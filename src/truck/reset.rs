//! Recovery for a truck that has rolled over or got stuck, and moving a truck to a
//! new place on request.

use crate::keys::{Control, KeyBindings};
use avian3d::prelude::*;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;

use super::{Player, Truck, TruckConfig};

/// Asks for a truck to be put down somewhere: upright, facing `yaw` and standing still.
/// This is how other slices move a truck, for example onto the start line.
#[derive(Message, Clone, Debug)]
pub struct PlaceTruck {
    pub truck: Entity,
    /// A point on the ground. The truck is set down with its wheels just touching it.
    pub ground: Vec3,
    /// Rotation about Y in radians. 0 faces -Z.
    pub yaw: f32,
}

/// What moving a truck by hand writes: where it is, both as it is drawn and as the physics
/// has it, and how it moves.
type Moved = (
    &'static mut Transform,
    &'static mut Position,
    &'static mut Rotation,
    &'static mut LinearVelocity,
    &'static mut AngularVelocity,
);

/// Puts a truck at `transform`, standing still. The physics takes a new `Transform` over
/// only at the next step, and until then the forces worked out for that step (see
/// `drive`) would turn the truck about where it was: measured, a truck moved 200 m was
/// thrown end over end. So its pose in the physics is set at the same time.
fn teleport(moved: QueryItem<'_, '_, Moved>, to: Transform) {
    let (mut transform, mut position, mut rotation, mut linear, mut angular) = moved;
    *transform = to;
    position.0 = to.translation;
    rotation.0 = to.rotation;
    linear.0 = Vec3::ZERO;
    angular.0 = Vec3::ZERO;
}

pub(super) fn place_trucks(
    mut requests: MessageReader<PlaceTruck>,
    mut trucks: Query<(Moved, &TruckConfig), With<Truck>>,
) {
    for request in requests.read() {
        let Ok((moved, config)) = trucks.get_mut(request.truck) else {
            continue;
        };
        let to = Transform::from_translation(request.ground + Vec3::Y * config.standing_height())
            .with_rotation(Quat::from_rotation_y(request.yaw));
        teleport(moved, to);
    }
}

/// Puts the player's truck back on its wheels, a little above where it currently is.
pub(super) fn reset_truck(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<KeyBindings>,
    mut trucks: Query<Moved, Player>,
) {
    if !bindings.just_pressed(&keys, Control::FlipUpright) {
        return;
    }
    for moved in &mut trucks {
        let (yaw, _, _) = moved.0.rotation.to_euler(EulerRot::YXZ);
        let to = Transform::from_translation(moved.0.translation + Vec3::Y * 2.5)
            .with_rotation(Quat::from_rotation_y(yaw));
        teleport(moved, to);
    }
}
