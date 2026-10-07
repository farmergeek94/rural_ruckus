//! Smooth motion on screen from physics that moves in steps.
//!
//! Physics runs a fixed number of times a second, and frames don't: a frame that came a
//! little early holds one physics step, the next one three. Drawn at its physics pose, the
//! truck would cover uneven distances from frame to frame, and the camera aiming at it
//! would shake the whole picture. So the truck that is drawn is a separate entity, placed
//! each frame between the last two physics poses by how far the frame falls between steps.
//! It is at most one step (8 ms) behind the physics, which nobody can see.
//!
//! The wheels hang from the drawn truck, and are placed the same way between their last two
//! poses (`WheelPose`): their suspension, steering and spin also move only at each physics
//! step.
//!
//! The physics body itself must not be moved for the sake of looks: the physics would take
//! that for a teleport.

use bevy::prelude::*;

use super::{Truck, TruckVisual};

/// A teleport (being put back on the course, say) is further than any truck drives in one
/// step, and must not be drawn as a streak across the map.
const TELEPORT_DISTANCE: f32 = 3.0;

/// Where the physics body was at the end of the last two physics steps.
#[derive(Component)]
pub(super) struct PhysicsPose {
    previous: Transform,
    current: Transform,
}

impl PhysicsPose {
    pub(super) fn at(transform: Transform) -> Self {
        Self {
            previous: transform,
            current: transform,
        }
    }

    /// Both poses moved `by`, as the truck is when it goes over the edge of a world that
    /// repeats (`wrap`): drawn going on as it was, not streaking across the map.
    pub(super) fn shift(&mut self, by: Vec3) {
        self.previous.translation += by;
        self.current.translation += by;
    }
}

pub(super) fn record_poses(mut trucks: Query<(&Transform, &mut PhysicsPose), With<Truck>>) {
    for (transform, mut pose) in &mut trucks {
        pose.previous = pose.current;
        pose.current = *transform;
    }
}

pub(super) fn place_visuals(
    time: Res<Time<Fixed>>,
    trucks: Query<&PhysicsPose>,
    mut visuals: Query<(&TruckVisual, &mut Transform)>,
) {
    let between = time.overstep_fraction();
    for (visual, mut transform) in &mut visuals {
        let Ok(pose) = trucks.get(visual.truck) else {
            continue;
        };
        let (from, to) = (pose.previous, pose.current);
        *transform = if from.translation.distance(to.translation) > TELEPORT_DISTANCE {
            to
        } else {
            Transform {
                translation: from.translation.lerp(to.translation, between),
                rotation: from.rotation.slerp(to.rotation, between),
                scale: to.scale,
            }
        };
    }
}

/// Where a wheel's pivot was in its drawn truck at the end of the last two physics steps:
/// `drive` sets it each step, and `place_wheels` draws the wheel between the two. Set
/// straight from each step, the suspension, steering and spin moved in steps that frames
/// caught unevenly, and the wheels, and the axles and shocks that follow them, shook
/// against a body that glided. On Alpine at 24 m/s a hub moves up and down by about 7 cm
/// faster than four times a second.
#[derive(Component)]
pub(super) struct WheelPose {
    previous: Transform,
    current: Transform,
}

impl WheelPose {
    pub(super) fn at(transform: Transform) -> Self {
        Self {
            previous: transform,
            current: transform,
        }
    }

    /// This step's pose, the last one becoming the one before.
    pub(super) fn step(&mut self, transform: Transform) {
        self.previous = self.current;
        self.current = transform;
    }
}

pub(super) fn place_wheels(
    time: Res<Time<Fixed>>,
    mut wheels: Query<(&WheelPose, &mut Transform)>,
) {
    let between = time.overstep_fraction();
    for (pose, mut transform) in &mut wheels {
        let (from, to) = (pose.previous, pose.current);
        *transform = Transform {
            translation: from.translation.lerp(to.translation, between),
            rotation: from.rotation.slerp(to.rotation, between),
            scale: to.scale,
        };
    }
}
