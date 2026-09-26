//! Recovery for a truck that has rolled over or got stuck, and moving a truck to a
//! new place on request.

use crate::keys::{Control, KeyBindings};
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

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

pub(super) fn place_trucks(
    mut requests: MessageReader<PlaceTruck>,
    mut trucks: Query<(&mut Transform, &mut Velocity, &TruckConfig), With<Truck>>,
) {
    for request in requests.read() {
        let Ok((mut transform, mut velocity, config)) = trucks.get_mut(request.truck) else {
            continue;
        };
        transform.translation = request.ground + Vec3::Y * config.standing_height();
        transform.rotation = Quat::from_rotation_y(request.yaw);
        *velocity = Velocity::zero();
    }
}

/// Puts the player's truck back on its wheels, a little above where it currently is.
pub(super) fn reset_truck(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<KeyBindings>,
    mut trucks: Query<(&mut Transform, &mut Velocity), Player>,
) {
    if !bindings.just_pressed(&keys, Control::FlipUpright) {
        return;
    }
    for (mut transform, mut velocity) in &mut trucks {
        let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        transform.rotation = Quat::from_rotation_y(yaw);
        transform.translation.y += 2.5;
        *velocity = Velocity::zero();
    }
}
