//! A world that repeats, as a Monster Truck Madness 2 one does: a truck that goes over an
//! edge of the map comes back on at the other, going as it was.
//!
//! The truck is moved by whole widths of the map after a physics step, its pose in the
//! physics and its `Transform` together (see `reset`), and so are the two poses it is drawn
//! between (`PhysicsPose`), so that it is drawn going on smoothly. Nothing else about it
//! changes. Whoever follows it on screen is told by `TruckWrapped`, and moves the same way.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::Truck;
use super::interpolate::PhysicsPose;

/// How wide the world is where it repeats: along X and along Z, centred on the origin, in
/// metres, as `track::HeightGrid::repeating` has it. `None`, the default, is a world that
/// ends at its edges, where a truck that goes over one goes on. Set it before a race.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct RepeatingWorld(pub Option<f32>);

/// Says that a truck went over an edge of a world that repeats and was moved across to the
/// other: by `by`, in metres. Written after the physics step that took it over, and read in
/// `Update` of the same frame by whatever follows it on screen, which moves by as much.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct TruckWrapped {
    pub truck: Entity,
    pub by: Vec3,
}

pub(super) fn bring_back_on(
    world: Res<RepeatingWorld>,
    mut trucks: Query<(Entity, &mut Transform, &mut Position, &mut PhysicsPose), With<Truck>>,
    mut wrapped: MessageWriter<TruckWrapped>,
) {
    let Some(size) = world.0 else {
        return;
    };
    for (truck, mut transform, mut position, mut pose) in &mut trucks {
        let by = back_on(position.0.xz(), size);
        if by == Vec2::ZERO {
            continue;
        }
        let by = Vec3::new(by.x, 0.0, by.y);
        transform.translation += by;
        position.0 += by;
        pose.shift(by);
        wrapped.write(TruckWrapped { truck, by });
    }
}

/// How far to move something at `at` (world X and Z) to bring it back onto a map `size`
/// metres across, centred on the origin: by a whole map across each edge it is over, and
/// not at all while it is on the map.
fn back_on(at: Vec2, size: f32) -> Vec2 {
    let half = size / 2.0;
    let along = |world: f32| {
        if world < -half {
            size
        } else if world >= half {
            -size
        } else {
            0.0
        }
    };
    Vec2::new(along(at.x), along(at.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_truck_over_an_edge_is_moved_across_and_one_on_the_map_is_not() {
        assert_eq!(back_on(Vec2::new(0.0, 0.0), 100.0), Vec2::ZERO);
        assert_eq!(back_on(Vec2::new(-50.0, 49.9), 100.0), Vec2::ZERO);
        assert_eq!(back_on(Vec2::new(-50.1, 0.0), 100.0), Vec2::new(100.0, 0.0));
        assert_eq!(back_on(Vec2::new(50.0, 0.0), 100.0), Vec2::new(-100.0, 0.0));
        // Over a corner, across both edges.
        assert_eq!(
            back_on(Vec2::new(51.0, -52.0), 100.0),
            Vec2::new(-100.0, 100.0)
        );
    }
}
