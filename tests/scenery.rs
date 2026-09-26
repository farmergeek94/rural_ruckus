//! The scenery in a headless app: what kind of body each object is, that a loose object
//! stands where it is put until something touches it, and that a moving object comes back
//! onto the map at the other side.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_rapier3d::prelude::*;
use monster_truck_rural_ruckus::scenery::{SceneryObject, SceneryPlugin};
use monster_truck_rural_ruckus::track::{
    Scenery, SceneryModel, SceneryMotion, SceneryObject as Placed, Track, TrackPlugin,
    builtin_track, yaw_direction,
};

const STEP: f64 = 1.0 / 120.0;

/// A cube a metre a side, standing on its bottom face.
fn cube() -> SceneryModel {
    let corners: Vec<[f32; 3]> = (0..8)
        .map(|index| {
            let bit = |bit: usize| ((index >> bit) & 1) as f32;
            [bit(0) - 0.5, bit(1), bit(2) - 0.5]
        })
        .collect();
    // Six faces as two triangles each, wound outwards.
    let faces = [
        [0, 2, 6, 4],
        [1, 5, 7, 3],
        [0, 4, 5, 1],
        [2, 3, 7, 6],
        [0, 1, 3, 2],
        [4, 6, 7, 5],
    ];
    let indices = faces
        .iter()
        .flat_map(|[a, b, c, d]| [*a, *b, *c, *a, *c, *d])
        .collect();
    SceneryModel {
        name: "CUBE.BIN".into(),
        normals: vec![[0.0, 1.0, 0.0]; 8],
        uvs: vec![[0.0, 0.0]; 8],
        tiles: vec![0; 8],
        positions: corners,
        indices,
        ..default()
    }
}

/// The built-in track with three cubes on it, well away from the truck: one fixed, one
/// loose and one moving, 20 m from the edge of the map and heading off it at 10 m/s.
fn app() -> App {
    let mut track = builtin_track();
    let away = track.start.position - yaw_direction(track.start.yaw) * 60.0;
    let half = track.heights.size() / 2.0;
    let object = |position: Vec2, motion| Placed {
        model: 0,
        position,
        height_above_ground: 0.0,
        yaw: 0.0,
        solid: true,
        motion,
    };
    track.scenery = Scenery {
        tile_size: 8,
        tiles: vec![vec![128; 8 * 8 * 4]],
        texture_cycles: Vec::new(),
        models: vec![cube()],
        objects: vec![
            object(away, SceneryMotion::Fixed),
            object(away + Vec2::X * 5.0, SceneryMotion::Loose { mass: 50.0 }),
            object(
                Vec2::new(half - 20.0, away.y),
                SceneryMotion::Moving {
                    velocity: Vec3::X * 10.0,
                },
            ),
        ],
    };

    let mut app = App::new();
    app.insert_resource(Track(track))
        .add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule(),
            TrackPlugin,
            SceneryPlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .insert_resource(Time::<Fixed>::from_seconds(STEP))
        .insert_resource(TimestepMode::Fixed {
            dt: STEP as f32,
            substeps: 1,
        })
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            STEP,
        )));
    app.update();
    app
}

fn run(app: &mut App, seconds: f64) {
    for _ in 0..(seconds / STEP) as usize {
        app.update();
    }
}

fn bodies(app: &mut App) -> Vec<(RigidBody, Transform, Option<Sleeping>)> {
    let mut query = app
        .world_mut()
        .query_filtered::<(&RigidBody, &Transform, Option<&Sleeping>), With<SceneryObject>>();
    let mut bodies: Vec<_> = query
        .iter(app.world())
        .map(|(body, transform, sleeping)| (*body, *transform, sleeping.copied()))
        .collect();
    bodies.sort_by_key(|(body, ..)| format!("{body:?}"));
    bodies
}

fn body(app: &mut App, kind: RigidBody) -> (Transform, Option<Sleeping>) {
    let (_, transform, sleeping) = bodies(app)
        .into_iter()
        .find(|(body, ..)| *body == kind)
        .unwrap_or_else(|| panic!("no {kind:?} body"));
    (transform, sleeping)
}

#[test]
fn each_object_is_the_body_its_motion_says() {
    let mut app = app();
    let kinds: Vec<RigidBody> = bodies(&mut app)
        .into_iter()
        .map(|(body, ..)| body)
        .collect();
    assert_eq!(
        kinds,
        [
            RigidBody::Dynamic,
            RigidBody::Fixed,
            RigidBody::KinematicVelocityBased
        ]
    );
}

#[test]
fn a_loose_object_left_alone_stands_asleep_where_it_was_put() {
    let mut app = app();
    let (placed, _) = body(&mut app, RigidBody::Dynamic);
    run(&mut app, 2.0);
    let (now, sleeping) = body(&mut app, RigidBody::Dynamic);
    assert!(sleeping.unwrap().sleeping);
    // It settles onto the ground a little in the steps before it is put to sleep: by up to
    // 9 cm on Alpine's slopes.
    assert!(
        now.translation.distance(placed.translation) < 0.1,
        "{} to {}",
        placed.translation,
        now.translation
    );
}

#[test]
fn a_moving_object_comes_back_onto_the_map_at_the_other_side() {
    let mut app = app();
    let (placed, _) = body(&mut app, RigidBody::KinematicVelocityBased);
    let half = app.world().resource::<Track>().heights.size() / 2.0;
    run(&mut app, 1.0);
    let (on, _) = body(&mut app, RigidBody::KinematicVelocityBased);
    assert!((on.translation.x - placed.translation.x - 10.0).abs() < 0.2);
    // 20 m from the edge at 10 m/s: across it after 2 s, and 10 m in from the other side
    // after 3 s. Its height and its way across the map are as they were.
    run(&mut app, 2.0);
    let (back, _) = body(&mut app, RigidBody::KinematicVelocityBased);
    assert!(
        (back.translation.x - (10.0 - half)).abs() < 0.2,
        "{} on a map {} across",
        back.translation,
        half * 2.0
    );
    assert!((back.translation.y - placed.translation.y).abs() < 1e-3);
    assert!((back.translation.z - placed.translation.z).abs() < 1e-3);
}
