//! How the water pushes on trucks: buoyancy and drag.
//!
//! Monster trucks drive through water, so it is never a wall and never sends a truck back:
//! a truck wades in, is slowed, and carries on along the bottom if it goes under. Each
//! truck is taken as four upright columns, one over each wheel, from the bottom of the tire
//! to the roof. The part of each column below the surface is pushed up (buoyancy) and held
//! back against the way that part of the truck moves (drag), at its middle. Both grow with
//! how deep the column is, so that a truck feels the water through its tires first, and a
//! truck that goes in nose first is turned level by it, as the front meets it first.
//!
//! The water lifts some of the truck's weight off its tires, so they grip less, and the
//! truck slides more easily as well as being slowed.
//!
//! Buoyancy and drag are the game's own. A Monster Truck Madness 2 track only says where
//! the water is, and how its trucks behaved in it has not been measured. The values below
//! are chosen against the truck's own figures and are for tuning by driving.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::track::Track;
use crate::truck::{Truck, TruckConfig};

/// How much of the truck's weight the water carries when it is wholly under, from 0 to 1.
/// At 1 a sunken truck would float, and at 0 the water would not lift it at all. Its
/// tires grip in proportion to the weight left on them, so higher also slides more.
const BUOYANCY: f32 = 0.6;
/// How hard the water holds back a truck that is wholly under, as a deceleration in m/s²
/// per m/s of speed, and per (m/s)² of speed. Together with the engine and the tires'
/// grip, these set how fast a truck can go through water: wholly under, about 7 m/s;
/// hub-deep, about 18 m/s; with only the bottoms of its tires wet, a little over half its
/// speed on dry ground. Higher is slower. The second is what makes a truck that meets
/// the water at speed slow down hard before it settles to wading.
const LINEAR_DRAG: f32 = 0.3;
const QUADRATIC_DRAG: f32 = 0.045;
/// How far above its hubs a truck reaches, in metres: the top of the columns the water
/// meets. With the tires below the hubs, this is how deep the water must be to cover it.
const HEIGHT_ABOVE_HUBS: f32 = 1.5;

/// For `buoyancy`, in m/s².
const GRAVITY: f32 = 9.81;

pub(super) fn push_trucks(
    track: Res<Track>,
    mut trucks: Query<
        (
            &Transform,
            &Velocity,
            &ReadMassProperties,
            &TruckConfig,
            &mut ExternalForce,
        ),
        With<Truck>,
    >,
) {
    let Some(level) = track.water_level else {
        return;
    };
    for (transform, velocity, mass_properties, config, mut external_force) in &mut trucks {
        let up = transform.up().as_vec3();
        let center_of_mass = transform.transform_point(mass_properties.local_center_of_mass);
        let share = config.mass / config.wheel_rest.len() as f32;
        for rest in config.wheel_rest {
            let hub = transform.transform_point(rest);
            let column = Column::over(hub, up, config.wheel_radius);
            let Some(depth) = column.under(level) else {
                continue;
            };
            let at = column.middle_under(depth);
            let moving = velocity.linear + velocity.angular.cross(at - center_of_mass);
            let force = buoyancy(share, depth) + drag(share, depth, moving);
            *external_force += ExternalForce::at_point(force, at, center_of_mass);
        }
    }
}

/// One of the four upright parts of a truck that the water meets, from the bottom of a
/// tire to the roof above it, in world space.
///
/// `splash` uses it too, to tell where a truck breaks the surface.
#[derive(Clone, Copy, Debug)]
pub(super) struct Column {
    pub(super) bottom: Vec3,
    pub(super) top: Vec3,
}

impl Column {
    /// The column over the wheel whose hub is at `hub`, on a truck whose up is `up`.
    pub(super) fn over(hub: Vec3, up: Vec3, wheel_radius: f32) -> Self {
        Self {
            bottom: hub - up * wheel_radius,
            top: hub + up * HEIGHT_ABOVE_HUBS,
        }
    }

    /// How much of the column is under water at `level`, from 0 to 1, going by the
    /// height of each end, however the truck leans. `None` where none of it is.
    pub(super) fn under(&self, level: f32) -> Option<f32> {
        let (low, high) = (self.bottom.y.min(self.top.y), self.bottom.y.max(self.top.y));
        if level <= low {
            return None;
        }
        if high <= low {
            // Lying flat on its side: all in or all out.
            return Some(1.0);
        }
        Some(((level - low) / (high - low)).min(1.0))
    }

    /// Where the water's push acts: the middle of the part under water.
    fn middle_under(&self, depth: f32) -> Vec3 {
        let (low, high) = if self.bottom.y <= self.top.y {
            (self.bottom, self.top)
        } else {
            (self.top, self.bottom)
        };
        low.lerp(high, depth / 2.0)
    }
}

/// The water's upward push on a column that carries `mass` kilograms of the truck and is
/// `depth` under, from 0 to 1, in newtons.
fn buoyancy(mass: f32, depth: f32) -> Vec3 {
    Vec3::Y * mass * GRAVITY * BUOYANCY * depth
}

/// The water's pull against a column that carries `mass` kilograms of the truck, is
/// `depth` under, from 0 to 1, and moves at `velocity`, in newtons.
fn drag(mass: f32, depth: f32, velocity: Vec3) -> Vec3 {
    let speed = velocity.length();
    -velocity * mass * depth * (LINEAR_DRAG + QUADRATIC_DRAG * speed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upright(bottom: f32, top: f32) -> Column {
        Column {
            bottom: Vec3::new(0.0, bottom, 0.0),
            top: Vec3::new(0.0, top, 0.0),
        }
    }

    #[test]
    fn a_column_is_as_far_under_as_the_water_is_up_it() {
        let column = upright(10.0, 12.0);
        assert_eq!(column.under(9.0), None);
        assert_eq!(column.under(10.0), None);
        assert_eq!(column.under(10.5), Some(0.25));
        assert_eq!(column.under(12.0), Some(1.0));
        assert_eq!(column.under(50.0), Some(1.0));
    }

    #[test]
    fn a_column_upside_down_is_measured_the_same() {
        let column = upright(12.0, 10.0);
        assert_eq!(column.under(10.5), Some(0.25));
        assert_eq!(column.middle_under(0.25), Vec3::new(0.0, 10.25, 0.0));
    }

    #[test]
    fn the_push_acts_halfway_down_what_is_under() {
        let column = upright(10.0, 12.0);
        assert_eq!(column.middle_under(0.5), Vec3::new(0.0, 10.5, 0.0));
        assert_eq!(column.middle_under(1.0), Vec3::new(0.0, 11.0, 0.0));
    }

    #[test]
    fn a_sunken_truck_still_rests_on_the_bottom() {
        let mass = 2000.0;
        let lift = buoyancy(mass, 1.0);
        assert!(lift.y > 0.0 && lift.y < mass * GRAVITY, "{lift}");
        assert_eq!(buoyancy(mass, 0.0), Vec3::ZERO);
    }

    #[test]
    fn drag_holds_back_whichever_way_the_truck_moves() {
        let velocity = Vec3::new(3.0, -1.0, -12.0);
        let force = drag(500.0, 0.5, velocity);
        assert!(force.dot(velocity) < 0.0);
        assert!(force.cross(velocity).length() < 1e-3);
        // Deeper and faster are both harder.
        assert!(drag(500.0, 1.0, velocity).length() > force.length());
        assert!(drag(500.0, 0.5, velocity * 2.0).length() > 2.0 * force.length());
    }

    /// The speeds the constants' comment promises: where the water's drag meets what the
    /// built-in truck's engine and tires can push with.
    #[test]
    fn a_truck_wades_at_the_speeds_it_is_tuned_for() {
        let config = TruckConfig::default();
        let wheels = config.wheel_rest.len() as f32;
        // What the truck can push itself along with at `speed`, as an acceleration: its
        // engine, which runs out of pull at top speed, or its tires' grip on the weight the
        // water leaves on them, whichever is less.
        let push = |speed: f32, depth: f32| {
            let engine = config.engine_force * wheels / config.mass
                * (1.0 - speed / config.top_speed).max(0.0);
            let grip = config.grip * GRAVITY * (1.0 - BUOYANCY * depth);
            engine.min(grip)
        };
        let top_speed = |depth: f32| {
            (0..1000)
                .map(|tenth| tenth as f32 / 10.0)
                .find(|&speed| {
                    drag(config.mass, depth, Vec3::X * speed).length() / config.mass
                        >= push(speed, depth)
                })
                .unwrap()
        };
        let under = top_speed(1.0);
        assert!((6.0..8.0).contains(&under), "wholly under: {under} m/s");
        // Hub-deep: the water up to the hubs, the tires' radius up a column that is that
        // and `HEIGHT_ABOVE_HUBS` tall.
        let hub_deep = top_speed(config.wheel_radius / (config.wheel_radius + HEIGHT_ABOVE_HUBS));
        assert!((16.0..20.0).contains(&hub_deep), "hub-deep: {hub_deep} m/s");
    }
}
