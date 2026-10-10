//! What a truck does to the water it drives through: where its tires break the surface,
//! and the ripples it leaves.
//!
//! A tire breaks the surface where the water is partway up its column (see `forces`),
//! neither below it nor over it. A tire that comes down into the water fast starts a big
//! ripple. And the truck leaves a trail of ripples behind it, one every few metres, which
//! spread and cross into a wake; standing in the water, it laps at it now and then.
//!
//! Each tire that breaks the surface pushes on the field of heights (`field`): it piles
//! the water up in front of it and draws it down behind, which the field spreads into a
//! wake, and churns up foam; a wheel that comes down into the water pushes it down, which
//! the field throws back up as a ring. And each is told to whatever throws spray
//! (`TireInWater`), so that the spray and the waves agree on where the water is broken.
//!
//! Where the truck is comes from how it is drawn (`TruckVisual`), so that the ripples and
//! the spray leave the tires where they are seen. How fast it goes comes from its body.

use avian3d::prelude::{AngularVelocity, LinearVelocity};
use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;

use super::field::{Disturbances, Source};
use super::forces::Column;
use super::ripples::{Ripple, Ripples};
use super::shore::swell_height;
use super::wind::Wind;
use super::{Plunge, TireInWater, WaterSettings};
use crate::track::Track;
use crate::truck::{TruckConfig, TruckVisual, TruckWheels, Wheel};

/// How fast a wheel must come down into the water to splash, in m/s.
pub const SPLASH_SPEED: f32 = 2.0;
/// How far the roof of a truck is above the middle of its chassis, in metres: the top of
/// the column of truck over each tire that the water can break on. Above it, the truck
/// is wholly under and churns nothing at the surface. The game's own, about a monster
/// truck's cab; the forces (`forces::HEIGHT_ABOVE_HUBS`) keep their own, shorter column.
const ROOF_ABOVE_CHASSIS: f32 = 1.8;
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
/// What a tire does to the field of heights. What pushes the water is the part of the
/// tire under it: the tire meets the water along a chord, `pushing_front` metres ahead of
/// the hub and as far behind, and the water is piled up ahead of that chord's leading
/// edge, which is where a source sits (`FRONT_SHARE` of the way from the hub to the
/// edge), and churned into foam there. How hard it piles the water up, in metres of
/// height each second for each m/s it goes over the ground, up to `MOST_BOW_SPEED`, for a
/// tire hub-deep; how far out from the leading edge that reaches, as a share of the
/// chord's half, or of the tire's half width if that is more; how much foam its tread
/// churns up each second for each m/s it goes, over the ground or faster where it spins
/// (`MOST_SPIN` more at the most), up to `MOST_CHURN`, again hub-deep. All of it scales
/// with how much of the tire is in the water, as the chord says: a tire that only wets
/// its bottom does little, and one with the water over its hub pushes with its whole
/// width.
const BOW_PUSH: f32 = 1.2;
const MOST_BOW_SPEED: f32 = 10.0;
const FRONT_SHARE: f32 = 0.8;
const BOW_REACH: f32 = 1.2;
const CHURN: f32 = 0.8;
const MOST_CHURN: f32 = 8.0;
const MOST_SPIN: f32 = 5.0;
/// What the truck's body does to the field once the water is up to it: how hard it piles
/// the water up in front of its bumper, in metres of height each second for each m/s it
/// goes, up to `MOST_BOW_SPEED`; how far ahead of the front hubs the bumper is, in wheel
/// radii; how far out from the middle of the bumper that reaches, as a share of the way
/// between its front hubs; and how far up its front tires the water must come
/// (`wet_share`) for the body to be in it at all, and for all of that.
const BODY_PUSH: f32 = 0.4;
const BUMPER_AHEAD: f32 = 1.0;
const BODY_REACH: f32 = 0.7;
/// How much foam the body churns up each second for each m/s it goes, up to `MOST_CHURN`.
const BODY_CHURN: f32 = 0.4;
const BODY_WET: [f32; 2] = [0.45, 0.8];
/// What a wheel coming down into the water does to the field: how far it pushes the
/// water down under it, in metres for each m/s it came down at, up to the most, and how
/// much foam it churns up at once for each m/s.
const PLUNGE_PUSH: f32 = 0.03;
const MOST_PLUNGE: f32 = 0.3;
const PLUNGE_FOAM: f32 = 0.25;

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
    bodies: Query<(
        &LinearVelocity,
        &AngularVelocity,
        &TruckConfig,
        &TruckWheels,
    )>,
    wheels: Query<&Wheel>,
    mut ripples: ResMut<Ripples>,
    mut disturbances: ResMut<Disturbances>,
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
        // The front wheels' hubs and how far up them the water comes, for the body.
        let mut fronts = Vec::new();
        for (index, rest) in config.wheel_rest.iter().enumerate() {
            let hub = transform.transform_point(*rest);
            let column = Column {
                bottom: hub - up * config.wheel_radius,
                top: transform.translation + up * ROOF_ABOVE_CHASSIS,
            };
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
            let tread = truck_wheels
                .0
                .get(index)
                .and_then(|wheel| wheels.get(*wheel).ok())
                .map_or(moving.dot(forward), Wheel::tread_speed);
            let how_wet = wet_share(hub.y, config.wheel_radius, level);
            if rest.z < 0.0 {
                fronts.push((hub, how_wet));
            }

            let plunge = (!was_wet && -moving.y > SPLASH_SPEED).then(|| {
                let down = -moving.y;
                ripples.add(Ripple {
                    center: hub.xz(),
                    born: now,
                    strength: (down * SPLASH_RIPPLE_PER_SPEED).min(MOST_SPLASH_RIPPLE),
                });
                // On the swell where it is drawn, as `water.wgsl` moves it.
                let swell = swell_height(hub.xz(), clock, wind.towards);
                Plunge {
                    speed: down,
                    under_hub: Vec3::new(hub.x, level + swell, hub.z),
                }
            });
            disturbances.0.push(tire_source(
                hub.xz(),
                hub.y - level,
                moving,
                forward.xz(),
                tread,
                config.wheel_radius,
                config.wheel_width,
                plunge.map(|plunge| plunge.speed),
            ));
            // With splashes off, nothing is thrown: the ripples and the wake go on.
            if settings.splashes {
                tires.write(TireInWater {
                    truck: visual,
                    wheel: index,
                    at: at_surface,
                    hub,
                    outwards,
                    forward,
                    moving,
                    tread,
                    wet: how_wet,
                    wheel_radius: config.wheel_radius,
                    wheel_width: config.wheel_width,
                    entered: !was_wet,
                    plunge,
                });
            }
        }

        if let Some(body) = body_source(&fronts, linear.0, forward.xz(), config.wheel_radius) {
            disturbances.0.push(body);
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
                strength: wake_strength(speed),
            });
            state.last_ripple = Some((center, now));
        }
    }
}

/// How far ahead of the hub a tire of `wheel_radius` whose hub stands `above` metres over
/// the water meets the water, in metres: half the chord the water cuts across its round.
/// With the water over the hub, the tire's whole width is in it, and it pushes with all
/// of that: the radius.
fn pushing_front(wheel_radius: f32, above: f32) -> f32 {
    if above <= 0.0 {
        wheel_radius
    } else {
        (wheel_radius.powi(2) - above.powi(2)).max(0.0).sqrt()
    }
}

/// What a tire whose hub is over `hub`, `above` metres over the water, going at `moving`
/// (or facing `forward` when it stands still) with its tread going round at `tread` m/s,
/// does to the field of heights: and if it came down into the water at `plunged` m/s,
/// that too.
#[allow(clippy::too_many_arguments)]
fn tire_source(
    hub: Vec2,
    above: f32,
    moving: Vec3,
    forward: Vec2,
    tread: f32,
    wheel_radius: f32,
    wheel_width: f32,
    plunged: Option<f32>,
) -> Source {
    let front = pushing_front(wheel_radius, above);
    // How much of the tire is in the water, from 0 to 1, as what it pushes with.
    let in_water = front / wheel_radius;
    let ground_speed = moving.xz().length();
    let ahead = moving.xz().normalize_or(forward.normalize_or(Vec2::NEG_Y));
    let spin = (tread.abs() - ground_speed).clamp(0.0, MOST_SPIN);
    let churn = (ground_speed + spin).min(MOST_CHURN);
    Source {
        at: hub + ahead * front * FRONT_SHARE,
        along: moving.xz(),
        radius: front.max(wheel_width / 2.0) * BOW_REACH,
        push: BOW_PUSH * ground_speed.min(MOST_BOW_SPEED) * in_water,
        lift: plunged.map_or(0.0, |down| -(PLUNGE_PUSH * down).min(MOST_PLUNGE)),
        foam: CHURN * churn * in_water,
        burst: plunged.map_or(0.0, |down| PLUNGE_FOAM * down),
    }
}

/// What a truck's body, over `fronts` (its front wheels' hubs, and how far up each the
/// water comes), going at `moving` (or facing `forward` when it stands still), with
/// wheels of `wheel_radius`, does to the field of heights: its bumper, a wheel's radius
/// ahead of the front hubs, piles the water up ahead of it, once the water is up to it.
/// `None` while it is clear of the water.
fn body_source(
    fronts: &[(Vec3, f32)],
    moving: Vec3,
    forward: Vec2,
    wheel_radius: f32,
) -> Option<Source> {
    let [(left, left_wet), (right, right_wet)] = fronts else {
        return None;
    };
    let wet = (left_wet + right_wet) / 2.0;
    let share = smoothstep(BODY_WET[0], BODY_WET[1], wet);
    if share <= 0.0 {
        return None;
    }
    let speed = moving.xz().length();
    let ahead = moving.xz().normalize_or(forward.normalize_or(Vec2::NEG_Y));
    Some(Source {
        at: (left.xz() + right.xz()) / 2.0 + ahead * wheel_radius * BUMPER_AHEAD,
        along: moving.xz(),
        radius: left.distance(*right) * BODY_REACH,
        push: BODY_PUSH * speed.min(MOST_BOW_SPEED) * share,
        lift: 0.0,
        foam: BODY_CHURN * speed.min(MOST_CHURN) * share,
        burst: 0.0,
    })
}

fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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
    fn a_tire_pushes_with_the_part_of_it_under_the_water() {
        // A tire of radius 1: touching, hub-deep, and under.
        assert_eq!(pushing_front(1.0, 1.0), 0.0);
        assert!((pushing_front(1.0, 0.6) - 0.8).abs() < 1e-5);
        assert_eq!(pushing_front(1.0, 0.0), 1.0);
        assert_eq!(pushing_front(1.0, -0.5), 1.0);
    }

    #[test]
    fn a_tire_pushes_harder_the_faster_and_deeper_it_goes_and_ahead_of_itself() {
        let source = |speed: f32, above: f32| {
            tire_source(
                Vec2::ZERO,
                above,
                Vec3::NEG_Z * speed,
                Vec2::NEG_Y,
                speed,
                1.0,
                0.6,
                None,
            )
        };
        assert_eq!(source(0.0, 0.0).push, 0.0);
        assert!(source(5.0, 0.0).push > source(2.0, 0.0).push);
        assert!(source(5.0, 0.0).push > source(5.0, 0.9).push);
        // Only so fast counts.
        assert_eq!(source(20.0, 0.0).push, source(MOST_BOW_SPEED, 0.0).push);
        assert_eq!(source(5.0, 0.0).lift, 0.0);
        // Ahead of the hub (-Z), at the leading edge of the water's chord across the tire:
        // further ahead the deeper the tire sits, up to the hub.
        assert!(source(5.0, 0.6).at.y < 0.0);
        assert!(source(5.0, 0.0).at.y < source(5.0, 0.6).at.y);
        assert_eq!(source(5.0, 0.0).at.y, source(5.0, -0.5).at.y);
        // A tire that only touches the water pushes nothing, and reaches as wide as it is.
        let touching = source(5.0, 1.0);
        assert_eq!(touching.push, 0.0);
        assert!((touching.radius - 0.3 * BOW_REACH).abs() < 1e-5);
        // Standing still, a spinning tire churns up foam, ahead of itself, and pushes no
        // water.
        let spinning = tire_source(
            Vec2::ZERO,
            0.0,
            Vec3::ZERO,
            Vec2::NEG_Y,
            20.0,
            1.0,
            0.6,
            None,
        );
        assert_eq!(spinning.push, 0.0);
        assert!(spinning.foam > 0.0 && spinning.at.y < 0.0);
        // Coming down pushes the water down, so that it is thrown back up as a ring.
        let plunged = tire_source(
            Vec2::ZERO,
            0.5,
            Vec3::NEG_Y * 4.0,
            Vec2::NEG_Y,
            0.0,
            1.0,
            0.6,
            Some(4.0),
        );
        assert!(plunged.lift < 0.0 && plunged.burst > 0.0);
    }

    #[test]
    fn the_body_pushes_the_water_once_it_is_up_to_it() {
        let fronts = |wet: f32| {
            [
                (Vec3::new(-1.0, 0.0, -1.5), wet),
                (Vec3::new(1.0, 0.0, -1.5), wet),
            ]
        };
        let moving = Vec3::NEG_Z * 6.0;
        let body_of = |wet: f32| body_source(&fronts(wet), moving, Vec2::NEG_Y, 0.8);
        // Only the tires in the water: the body is clear of it.
        assert!(body_of(0.3).is_none());
        let body = body_of(1.0).unwrap();
        // At the bumper, a wheel's radius ahead of the front hubs.
        assert_eq!(body.at, Vec2::new(0.0, -1.5 - 0.8 * BUMPER_AHEAD));
        assert!(body.push > 0.0 && body.radius > 1.0);
        // Deeper pushes harder.
        assert!(body_of(0.6).unwrap().push < body.push);
        // A truck with one wheel at the front, or none, has no body here to speak of.
        assert!(body_source(&fronts(1.0)[..1], moving, Vec2::NEG_Y, 0.8).is_none());
    }

    #[test]
    fn a_faster_wake_is_stronger_up_to_a_limit() {
        assert_eq!(wake_strength(0.0), WAKE_STRENGTH[0]);
        assert!(wake_strength(10.0) > wake_strength(2.0));
        assert_eq!(wake_strength(1000.0), WAKE_STRENGTH[1]);
    }
}
