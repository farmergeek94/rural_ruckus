//! What keeps the rain and snow off: for each square metre round the camera, the height
//! of the highest solid that does not move above it, such as a bridge, a roof or the
//! ground. `fall.wgsl` draws no drop below it, so none falls under a bridge or a roof, and
//! rain still falls beyond its edge.
//!
//! Each square metre (`COVER_CELL`) is found once, by a ray cast straight down onto the
//! bodies that never move (`RigidBody::Static`): trucks, and what they knock about or what
//! moves on its own, give no cover. What does not move does not change, so a square is
//! never cast again while the camera stays near it. Only the squares that the camera has
//! just come near are cast, the nearest first and no more than `RAYS_PER_FRAME` in a frame.
//! A square not yet found has no cover, as before there was any.
//!
//! The squares are kept in a ring, `COVER_CELLS` across, laid over the world like tiles:
//! world square (x, z) is in place (x, z) of the ring, each taken round. As the camera
//! moves, the squares it leaves behind make room for those it comes to. Each place says
//! which world square it holds, so that the shader never takes one square's cover for
//! another's.
//!
//! Measured headless in the dev build: a ray costs 0.6 to 1.4 µs on Alpine Mountains,
//! Monte Carlo and Route 77, so the whole ring (2304 rays) costs 1.3 to 3.2 ms, and the
//! squares that a camera at speed comes to in a frame, 0.03 to 0.07 ms.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::render::storage::ShaderBuffer;

use crate::camera::ChaseCamera;

/// The side of a square of cover, in metres.
pub(super) const COVER_CELL: f32 = 1.0;
/// How many squares the ring has along each side. It must reach past the box of rain
/// round the camera (`fall::FALL_BOX`) on every side.
pub(super) const COVER_CELLS: usize = 48;
/// The most squares cast in one frame: at most 0.15 to 0.35 ms (see above). The whole
/// ring is found in 9 or 10 frames, nearest first.
const RAYS_PER_FRAME: usize = 256;
/// The height that each ray starts from, in metres: above anything on a track. A height
/// map's steps go up to 255 x 2 ft, 155 m (`docs/formats/terrain.md`).
const RAY_FROM: f32 = 2000.0;
/// How far a ray goes down, in metres: to well below the lowest ground.
const RAY_LENGTH: f32 = 4000.0;

/// The squares of cover round the camera, on the mesh of rain or snow, so that new
/// weather, or a new race, starts with none found.
#[derive(Component)]
pub(super) struct Cover {
    places: Vec<Place>,
    /// The square the camera was over when every square round it was last found.
    found_round: Option<IVec2>,
    /// The ring as the shader reads it.
    buffer: Handle<ShaderBuffer>,
}

/// One place of the ring.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Place {
    /// Which world square it holds, or none yet.
    square: Option<IVec2>,
    /// The height of the highest solid above it that does not move, in metres, or
    /// `f32::MIN` where there is none.
    cover: f32,
}

impl Cover {
    /// No square found, and the buffer the shader reads that from.
    pub(super) fn new(buffers: &mut Assets<ShaderBuffer>) -> Self {
        let places = vec![
            Place {
                square: None,
                cover: f32::MIN,
            };
            COVER_CELLS * COVER_CELLS
        ];
        let buffer = buffers.add(ShaderBuffer::from(for_shader(&places)));
        Self {
            places,
            found_round: None,
            buffer,
        }
    }

    pub(super) fn buffer(&self) -> Handle<ShaderBuffer> {
        self.buffer.clone()
    }
}

/// Finds the cover of the squares that the camera has come near, and gives the shader the
/// ring when any has changed.
pub(super) fn find_cover(
    cameras: Query<&Transform, With<ChaseCamera>>,
    mut covers: Query<&mut Cover>,
    spatial: SpatialQuery,
    colliders: Query<&ColliderOf>,
    bodies: Query<&RigidBody>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
    let Ok(eye) = cameras.single() else {
        return;
    };
    let centre = square_at(eye.translation.xz());
    let never_moves = |collider: Entity| {
        colliders
            .get(collider)
            .ok()
            .and_then(|of| bodies.get(of.body).ok())
            .is_some_and(|body| *body == RigidBody::Static)
    };
    for mut cover in &mut covers {
        if cover.found_round == Some(centre) {
            continue;
        }
        let wanted = squares_to_find(&cover.places, centre);
        for &square in wanted.iter().take(RAYS_PER_FRAME) {
            let middle = (square.as_vec2() + 0.5) * COVER_CELL;
            let hit = spatial.cast_ray_predicate(
                Vec3::new(middle.x, RAY_FROM, middle.y),
                Dir3::NEG_Y,
                RAY_LENGTH,
                true,
                &SpatialQueryFilter::default(),
                &never_moves,
            );
            cover.places[place_of(square)] = Place {
                square: Some(square),
                cover: hit.map_or(f32::MIN, |hit| RAY_FROM - hit.distance),
            };
        }
        if wanted.len() <= RAYS_PER_FRAME {
            cover.found_round = Some(centre);
        }
        if !wanted.is_empty()
            && let Some(mut buffer) = buffers.get_mut(&cover.buffer)
        {
            buffer.set_data(for_shader(&cover.places));
        }
    }
}

/// The world square under a point on the ground plane (world X and Z).
fn square_at(at: Vec2) -> IVec2 {
    (at / COVER_CELL).floor().as_ivec2()
}

/// Where world square `square` is kept in the ring.
fn place_of(square: IVec2) -> usize {
    let ring = COVER_CELLS as i32;
    let [x, z] = [square.x, square.y].map(|along| along.rem_euclid(ring) as usize);
    z * COVER_CELLS + x
}

/// The squares round `centre` that the ring does not hold yet, nearest first. The ring
/// covers `COVER_CELLS` squares along each side, from half of them before `centre`.
fn squares_to_find(places: &[Place], centre: IVec2) -> Vec<IVec2> {
    let half = COVER_CELLS as i32 / 2;
    let mut wanted: Vec<IVec2> = (-half..COVER_CELLS as i32 - half)
        .flat_map(|z| (-half..COVER_CELLS as i32 - half).map(move |x| centre + IVec2::new(x, z)))
        .filter(|&square| places[place_of(square)].square != Some(square))
        .collect();
    wanted.sort_by_key(|&square| (square - centre).length_squared());
    wanted
}

/// The ring as `fall.wgsl` reads it: for each place, the cover's height, the world
/// square's X and Z, and 1 if it is found (0 if not).
fn for_shader(places: &[Place]) -> Vec<Vec4> {
    places
        .iter()
        .map(|place| match place.square {
            Some(square) => Vec4::new(place.cover, square.x as f32, square.y as f32, 1.0),
            None => Vec4::ZERO,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weather::fall::FALL_BOX;

    fn found_round(centre: IVec2) -> Vec<Place> {
        let mut places = vec![
            Place {
                square: None,
                cover: f32::MIN,
            };
            COVER_CELLS * COVER_CELLS
        ];
        for square in squares_to_find(&places, centre) {
            places[place_of(square)] = Place {
                square: Some(square),
                cover: 0.0,
            };
        }
        places
    }

    #[test]
    fn the_ring_reaches_past_the_box_of_rain() {
        // From the camera's square, the ring reaches at least half of it less one square
        // either way.
        let reach = (COVER_CELLS as f32 / 2.0 - 1.0) * COVER_CELL;
        assert!(reach >= FALL_BOX.x / 2.0 && reach >= FALL_BOX.z / 2.0);
    }

    #[test]
    fn every_square_round_the_camera_has_a_place_of_its_own() {
        let places = found_round(IVec2::new(-7, 130));
        assert!(places.iter().all(|place| place.square.is_some()));
        assert!(squares_to_find(&places, IVec2::new(-7, 130)).is_empty());
    }

    #[test]
    fn a_step_along_finds_one_new_row_only() {
        let places = found_round(IVec2::ZERO);
        let wanted = squares_to_find(&places, IVec2::new(1, 0));
        assert_eq!(wanted.len(), COVER_CELLS);
        assert!(
            wanted
                .iter()
                .all(|square| square.x == COVER_CELLS as i32 / 2)
        );
    }

    #[test]
    fn the_nearest_squares_are_found_first() {
        let empty = vec![
            Place {
                square: None,
                cover: f32::MIN,
            };
            COVER_CELLS * COVER_CELLS
        ];
        let centre = IVec2::new(3, -4);
        let wanted = squares_to_find(&empty, centre);
        assert_eq!(wanted.len(), COVER_CELLS * COVER_CELLS);
        assert_eq!(wanted[0], centre);
        let distances: Vec<i32> = wanted
            .iter()
            .map(|&square| (square - centre).length_squared())
            .collect();
        assert!(distances.is_sorted());
    }

    /// A fixed roof gives cover, and a body that can move does not. Run headless, with the
    /// physics, as in the game.
    #[test]
    fn a_fixed_roof_gives_cover_and_a_box_that_can_move_does_not() {
        use std::time::Duration;

        use bevy::time::TimeUpdateStrategy;

        use crate::physics::GamePhysicsPlugin;

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            GamePhysicsPlugin,
        ))
        .init_asset::<ShaderBuffer>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 120.0,
        )))
        .add_systems(Update, find_cover);
        app.finish();
        app.cleanup();
        let world = app.world_mut();
        world.spawn((
            ChaseCamera::new(default()),
            Transform::from_xyz(0.5, 2.0, 0.5),
        ));
        // A roof 10 m square, its top 6.25 m up, over the camera.
        world.spawn((
            RigidBody::Static,
            Collider::cuboid(10.0, 0.5, 10.0),
            Transform::from_xyz(0.0, 6.0, 0.0),
        ));
        // A crate 15 m off, held up so that it does not fall while the test runs.
        world.spawn((
            RigidBody::Dynamic,
            Collider::cuboid(2.0, 2.0, 2.0),
            Transform::from_xyz(15.0, 6.0, 0.0),
            GravityScale(0.0),
        ));
        let cover = Cover::new(&mut world.resource_mut::<Assets<ShaderBuffer>>());
        let cover = world.spawn(cover).id();
        for _ in 0..20 {
            app.update();
        }

        let cover = app.world().get::<Cover>(cover).unwrap();
        let at = |x: i32, z: i32| cover.places[place_of(IVec2::new(x, z))];
        let roof = at(0, 0);
        assert_eq!(roof.square, Some(IVec2::ZERO));
        assert!((roof.cover - 6.25).abs() < 1e-3, "{roof:?}");
        assert!((at(-4, 4).cover - 6.25).abs() < 1e-3);
        // Off the roof, under the crate, and in the open.
        assert_eq!(at(15, 0).cover, f32::MIN);
        assert_eq!(at(20, -10).cover, f32::MIN);
        // Every square round the camera found, and the shader told.
        assert!(cover.places.iter().all(|place| place.square.is_some()));
        let buffers = app.world().resource::<Assets<ShaderBuffer>>();
        let sent = buffers.get(&cover.buffer).unwrap().data.as_ref().unwrap();
        assert_eq!(sent.len(), COVER_CELLS * COVER_CELLS * 16);
    }

    #[test]
    fn the_shader_is_told_which_square_each_place_holds() {
        let places = [
            Place {
                square: Some(IVec2::new(-3, 9)),
                cover: 12.5,
            },
            Place {
                square: None,
                cover: f32::MIN,
            },
        ];
        assert_eq!(
            for_shader(&places),
            [Vec4::new(12.5, -3.0, 9.0, 1.0), Vec4::ZERO]
        );
    }
}
