//! Scenery that moves: loose objects, knocked about by whatever hits them, and moving ones,
//! which go on at their velocity whatever they meet.
//!
//! A loose object is a body with its track's mass. It is put to sleep, so that it stands
//! where it was put, whatever the ground under it does, until something wakes it by
//! touching it. Not at once: the physics puts a body in with the others it touches (its
//! island) in its first step, and one made asleep is never put in; two such that touch
//! each other stop the game (measured: two of a track's rocks, in Avian 0.7). So it is put
//! to sleep a few steps after it is made (`settle`). Its collider is the hull round its model, since it must meet fixed scenery
//! as well as the ground and the trucks, and no contacts are found between two triangle
//! meshes. It can be moved fast by a heavy truck, so it is swept (CCD) to keep it from
//! going through the ground in one step. A tire that stands on it pushes it back (see
//! `truck/drive.rs`).
//!
//! A moving object is kinematic: nothing pushes it, and what is in its way is pushed. It
//! keeps its height, because every one found goes along a level line of ground and bridges
//! that runs right across the map (see `docs/formats/situation.md`), and when it runs off
//! one side of the map it comes back on the other, as the map's own edges do. Whether MTM2
//! does that too is open.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{SceneryObject, bouncy};
use crate::game_state::GameState;
use crate::track::Track;

/// How far under the ground beneath it a loose object may be before it is taken as lost
/// (fallen through the ground, or off the edge of the map) and removed, in metres. It
/// would otherwise fall for ever, and cost a body in every physics step.
const LOST_BELOW: f32 = 20.0;

/// A scenery object that goes on at its velocity.
#[derive(Component)]
pub(super) struct Moving;

/// A scenery object that stands until it is knocked over or away.
#[derive(Component)]
pub(super) struct Loose;

/// How many physics steps a loose object is left before it is put to sleep: past the
/// first, in which the physics puts it in with what it touches.
const SETTLE_STEPS: u8 = 3;

/// A loose object that is still to be put to sleep, in so many more physics steps.
#[derive(Component)]
pub(super) struct Settling(u8);

/// Spawns a loose object of `mass` kilograms, to be put to sleep.
pub(super) fn loose(
    commands: &mut Commands,
    transform: Transform,
    hull: Collider,
    mass: f32,
) -> Entity {
    commands
        .spawn((
            SceneryObject,
            Loose,
            DespawnOnExit(GameState::Racing),
            transform,
            RigidBody::Dynamic,
            // The density that gives the hull the track's mass, so that it turns as a
            // solid of that shape and mass would.
            ColliderDensity(mass / hull.mass(1.0).max(f32::EPSILON)),
            hull,
            bouncy(),
            Settling(SETTLE_STEPS),
            SweptCcd::default(),
        ))
        .id()
}

/// Spawns a moving object, going at `velocity` in metres per second.
pub(super) fn moving(
    commands: &mut Commands,
    transform: Transform,
    collider: Collider,
    velocity: Vec3,
) -> Entity {
    commands
        .spawn((
            SceneryObject,
            Moving,
            DespawnOnExit(GameState::Racing),
            transform,
            RigidBody::Kinematic,
            collider,
            bouncy(),
            LinearVelocity(velocity),
        ))
        .id()
}

/// Puts loose objects to sleep once the physics has them in its islands.
pub(super) fn settle(mut commands: Commands, mut objects: Query<(Entity, &mut Settling)>) {
    for (entity, mut settling) in &mut objects {
        settling.0 = settling.0.saturating_sub(1);
        if settling.0 == 0 {
            commands.queue(SleepBody(entity));
            commands.entity(entity).remove::<Settling>();
        }
    }
}

/// Brings a moving object that has run off the map back on at the other side.
pub(super) fn keep_moving(
    track: Res<Track>,
    mut objects: Query<&mut Transform, (With<Moving>, With<SceneryObject>)>,
) {
    let size = track.heights.size();
    for mut transform in &mut objects {
        let at = transform.translation.xz();
        let wrapped = wrap(at, size);
        if wrapped != at {
            transform.translation.x = wrapped.x;
            transform.translation.z = wrapped.y;
        }
    }
}

/// Removes loose objects that have fallen out of the world.
pub(super) fn remove_lost(
    mut commands: Commands,
    track: Res<Track>,
    objects: Query<(Entity, &Transform), With<Loose>>,
) {
    for (entity, transform) in &objects {
        let at = transform.translation;
        if at.y < track.heights.height_at(at.x, at.z) - LOST_BELOW {
            commands.entity(entity).despawn();
        }
    }
}

/// A point on the ground plane put back on a map `size` metres across, centred on the
/// origin, as if the map repeated on every side.
fn wrap(at: Vec2, size: f32) -> Vec2 {
    let half = Vec2::splat(size / 2.0);
    (at + half).rem_euclid(Vec2::splat(size)) - half
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_on_the_map_stays_where_it_is() {
        for at in [Vec2::ZERO, Vec2::new(-49.0, 49.0), Vec2::new(-50.0, 12.5)] {
            assert_eq!(wrap(at, 100.0), at);
        }
    }

    #[test]
    fn a_point_off_one_side_comes_back_on_the_other() {
        assert_eq!(wrap(Vec2::new(51.0, 0.0), 100.0), Vec2::new(-49.0, 0.0));
        assert_eq!(wrap(Vec2::new(0.0, -51.0), 100.0), Vec2::new(0.0, 49.0));
        assert_eq!(wrap(Vec2::new(50.0, -50.0), 100.0), Vec2::new(-50.0, -50.0));
        // However far off it has gone.
        assert_eq!(wrap(Vec2::new(-260.0, 0.0), 100.0), Vec2::new(40.0, 0.0));
    }
}
