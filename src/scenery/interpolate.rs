//! Smooth motion on screen for scenery that moves, as `truck/interpolate.rs` gives the
//! trucks (whose notes say why): what is drawn is a separate entity, placed each frame
//! between the body's last two physics poses. The body itself is never moved for looks.

use bevy::prelude::*;

use super::SceneryObject;
use crate::game_state::GameState;

/// Further than anything moves in one physics step: a moving object brought back onto the
/// map at the other side, which must not be drawn as a streak across it.
const TELEPORT_DISTANCE: f32 = 10.0;

/// Where a body was at the end of the last two physics steps.
#[derive(Component)]
pub(super) struct PhysicsPose {
    previous: Transform,
    current: Transform,
}

/// What is drawn of a body that moves.
#[derive(Component)]
pub(super) struct SceneryVisual {
    body: Entity,
}

/// Draws `body`, which starts at `transform`.
pub(super) fn draw(
    commands: &mut Commands,
    body: Entity,
    transform: Transform,
    name: Name,
    drawn: impl Bundle,
) -> Entity {
    commands.entity(body).insert(PhysicsPose {
        previous: transform,
        current: transform,
    });
    commands
        .spawn((
            SceneryObject,
            SceneryVisual { body },
            DespawnOnExit(GameState::Racing),
            name,
            transform,
            drawn,
        ))
        .id()
}

pub(super) fn record_poses(mut bodies: Query<(&Transform, &mut PhysicsPose)>) {
    for (transform, mut pose) in &mut bodies {
        pose.previous = pose.current;
        pose.current = *transform;
    }
}

/// Places what is drawn, and takes it away once its body has gone.
pub(super) fn place_visuals(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    bodies: Query<&PhysicsPose>,
    mut visuals: Query<(Entity, &SceneryVisual, &mut Transform)>,
) {
    let between = time.overstep_fraction();
    for (visual, drawn, mut transform) in &mut visuals {
        let Ok(pose) = bodies.get(drawn.body) else {
            commands.entity(visual).despawn();
            continue;
        };
        let (from, to) = (pose.previous, pose.current);
        if from == to {
            // Asleep, which most are: leave it be, and Bevy with it.
            if *transform != to {
                *transform = to;
            }
            continue;
        }
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
