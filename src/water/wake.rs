//! What a truck does to the water it drives through: where its tires break the surface,
//! and the ripples it leaves.
//!
//! A tire breaks the surface where the water is partway up its column (see `forces`),
//! neither below it nor over it. A tire that comes down into the water fast starts a big
//! ripple. And the truck leaves a trail of ripples behind it, one every few metres, which
//! spread and cross into a wake; standing in the water, it laps at it now and then.
//!
//! Each tire that breaks the surface is told to whatever throws spray (`TireInWater`), so
//! that the spray and the ripples agree on where the water is broken.
//!
//! Where the truck is comes from how it is drawn (`TruckVisual`), so that the ripples and
//! the spray leave the tires where they are seen. How fast it goes comes from its body.

use avian3d::prelude::{AngularVelocity, LinearVelocity};
use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;

use super::forces::Column;
use super::ripples::{Ripple, Ripples};
use super::shore::swell_height;
use super::wind::Wind;
use super::{Plunge, TireInWater, WaterSettings};
use crate::track::Track;
use crate::truck::{TruckConfig, TruckVisual, TruckWheels, Wheel};

/// How fast a wheel must come down into the water to splash, in m/s.
pub const SPLASH_SPEED: f32 = 2.0;
/// How far a truck goes between the ripples it leaves, in metres, and how soon after
/// one it may leave the next at the least, in seconds. The two keep a fast truck from
/// using up every ripple the shader has.
const WAKE_SPACING: f32 = 2.0;
const WAKE_INTERVAL: f32 = 0.15;
/// How often a truck standing in the water laps at it, in seconds.
const LAPPING_INTERVAL: f32 = 1.2;
/// How strong a truck's ripples are, in metres: the least, standing, and how much more for
/// each m/s it goes, up to the most.
const WAKE_STRENGTH: [f32; 2] = [0.015, 0.1];
const WAKE_STRENGTH_PER_SPEED: f32 = 0.006;
/// How strong a splash's ripple is for each m/s the wheel came down at, and the most.
const SPLASH_RIPPLE_PER_SPEED: f32 = 0.03;
const MOST_SPLASH_RIPPLE: f32 = 0.25;

/// What `wade` remembers about a truck from one frame to the next.
#[derive(Default)]
pub(super) struct Wading {
    /// Per wheel: whether its tire was in the water.
    wet: [bool; 4],
    /// Where it left its last ripple, and how long ago.
    last_ripple: Option<(Vec2, f32)>,
}

/// Finds where each truck's tires break the surface, tells the spray, and leaves the
/// truck's ripples.
#[allow(clippy::too_many_arguments)]
pub(super) fn wade(
    time: Res<Time>,
    track: Res<Track>,
    wind: Res<Wind>,
    settings: Res<WaterSettings>,
    trucks: Query<(Entity, &Transform, &TruckVisual)>,
    bodies: Query<(&LinearVelocity, &AngularVelocity, &TruckConfig, &TruckWheels)>,
    wheels: Query<&Wheel>,
    mut ripples: ResMut<Ripples>,
    mut tires: MessageWriter<TireInWater>,
    mut wading: Local<EntityHashMap<Wading>>,
) {
    let Some(level) = track.water_level else {
        return;
    };
    let now = time.elapsed_secs();
    let clock = time.elapsed_secs_wrapped();

    // Trucks from a race that is over are forgotten.
    wading.retain(|visual, _| trucks.contains(*visual));

    for (visual, transform, truck) in &trucks {
        let Ok((linear, angular, config, truck_wheels)) = bodies.get(truck.truck) else {
            continue;
        };
        let state = wading.entry(visual).or_default();
        let up = transform.up().as_vec3();
        let right = transform.right().as_vec3();
        let forward = transform.forward().as_vec3();
        let mut breaking = Vec::new();
        for (index, rest) in config.wheel_rest.iter().enumerate() {
            let hub = transform.transform_point(*rest);
            let column = Column::over(hub, up, config.wheel_radius);
            let was_wet = state.wet[index];
            let wet = breaks_the_surface(&column, level);
            state.wet[index] = wet;
            if !wet {
                continue;
            }
            // Where the tire meets the water, on the outside of the tire.
            let outwards = right * rest.x.signum();
            let at_surface = Vec3::new(hub.x, level, hub.z) + outwards * config.wheel_width / 2.0;
            breaking.push(at_surface.xz());
            let moving = linear.0 + angular.0.cross(hub - transform.translation);

            let plunge = (!was_wet && -moving.y > SPLASH_SPEED).then(|| {
                let down = -moving.y;
                ripples.add(Ripple {
                    center: hub.xz(),
                    born: now,
                    born_on_clock: clock,
                    strength: (down * SPLASH_RIPPLE_PER_SPEED).min(MOST_SPLASH_RIPPLE),
                });
                // On the swell where it is drawn, as `water.wgsl` moves it.
                let swell = swell_height(hub.xz(), clock, wind.towards);
                Plunge {
                    speed: down,
                    under_hub: Vec3::new(hub.x, level + swell, hub.z),
                }
            });
            // With splashes off, nothing is thrown: the ripples and the wake go on.
            if settings.splashes {
                let tread = truck_wheels
                    .0
                    .get(index)
                    .and_then(|wheel| wheels.get(*wheel).ok())
                    .map_or(moving.dot(forward), Wheel::tread_speed);
                tires.write(TireInWater {
                    truck: visual,
                    wheel: index,
                    at: at_surface,
                    hub,
                    outwards,
                    forward,
                    moving,
                    tread,
                    wet: wet_share(hub.y, config.wheel_radius, level),
                    wheel_radius: config.wheel_radius,
                    wheel_width: config.wheel_width,
                    entered: !was_wet,
                    plunge,
                });
            }
        }

        if breaking.is_empty() {
            state.last_ripple = None;
            continue;
        }
        let center = breaking.iter().sum::<Vec2>() / breaking.len() as f32;
        let speed = linear.0.xz().length();
        let due = match state.last_ripple {
            None => true,
            Some((last, when)) => ripple_due(center.distance(last), now - when),
        };
        if due {
            ripples.add(Ripple {
                center,
                born: now,
                born_on_clock: clock,
                strength: wake_strength(speed),
            });
            state.last_ripple = Some((center, now));
        }
    }
}

/// Whether the water is partway up this column: the tire is in it, and the top is out.
fn breaks_the_surface(column: &Column, level: f32) -> bool {
    column.under(level).is_some_and(|depth| depth < 1.0)
}

/// How far up a tire whose hub is `hub_height` high, `radius` metres in radius, water at
/// `level` comes, from 0 (at its bottom) to 1 (over its top). Taken upright: a tire in the
/// water leans little.
fn wet_share(hub_height: f32, radius: f32, level: f32) -> f32 {
    ((level - (hub_height - radius)) / (2.0 * radius)).clamp(0.0, 1.0)
}

/// Whether a truck that has gone `travelled` metres in `since` seconds since its last
/// ripple leaves another.
fn ripple_due(travelled: f32, since: f32) -> bool {
    (travelled >= WAKE_SPACING && since >= WAKE_INTERVAL) || since >= LAPPING_INTERVAL
}

/// How strong a ripple a truck going at `speed` m/s leaves.
fn wake_strength(speed: f32) -> f32 {
    (WAKE_STRENGTH[0] + speed * WAKE_STRENGTH_PER_SPEED).min(WAKE_STRENGTH[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(bottom: f32, top: f32) -> Column {
        Column {
            bottom: Vec3::Y * bottom,
            top: Vec3::Y * top,
        }
    }

    #[test]
    fn only_a_tire_at_the_surface_breaks_it() {
        assert!(!breaks_the_surface(&column(1.0, 3.0), 0.5));
        assert!(breaks_the_surface(&column(1.0, 3.0), 1.5));
        // Wholly under, it is below the surface, not breaking it.
        assert!(!breaks_the_surface(&column(1.0, 3.0), 4.0));
    }

    #[test]
    fn a_tire_is_as_wet_as_the_water_is_high_up_it() {
        assert_eq!(wet_share(1.0, 1.0, -1.0), 0.0);
        assert_eq!(wet_share(1.0, 1.0, 0.0), 0.0);
        assert_eq!(wet_share(1.0, 1.0, 1.0), 0.5);
        assert_eq!(wet_share(1.0, 1.0, 5.0), 1.0);
    }

    #[test]
    fn a_truck_leaves_ripples_by_distance_and_laps_when_still() {
        assert!(!ripple_due(0.5, 0.2));
        assert!(ripple_due(WAKE_SPACING, 0.2));
        // However far it has gone, not too soon after the last.
        assert!(!ripple_due(50.0, WAKE_INTERVAL / 2.0));
        assert!(ripple_due(0.0, LAPPING_INTERVAL));
    }

    #[test]
    fn a_faster_wake_is_stronger_up_to_a_limit() {
        assert_eq!(wake_strength(0.0), WAKE_STRENGTH[0]);
        assert!(wake_strength(10.0) > wake_strength(2.0));
        assert_eq!(wake_strength(1000.0), WAKE_STRENGTH[1]);
    }
}
