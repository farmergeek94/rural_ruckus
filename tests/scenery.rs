//! The scenery in a headless app: what kind of body each object is, that a loose object
//! stands where it is put until something touches it, and that a moving object comes back
//! onto the map at the other side.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use monster_truck_rural_ruckus::physics::GamePhysicsPlugin;
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

/// A fence panel standing on its edge, as The Graveyard's fences and gates are: 6 m long,
/// 5 m high and a centimetre thick, about its middle.
fn panel() -> SceneryModel {
    let mut model = cube();
    for corner in &mut model.positions {
        *corner = [corner[0] * 6.0, (corner[1] - 0.5) * 5.0, corner[2] * 0.01];
    }
    model.name = "PANEL.BIN".into();
    model
}

/// A tire lying flat, as a scrapyard stacks them: sixteen-sided, 1.2 m across and 0.3 m
/// high, standing on its bottom face.
fn tire() -> SceneryModel {
    const SIDES: usize = 16;
    let positions: Vec<[f32; 3]> = [0.0, 0.3]
        .iter()
        .flat_map(|&y| {
            (0..SIDES).map(move |side| {
                let angle = side as f32 / SIDES as f32 * std::f32::consts::TAU;
                [0.6 * angle.cos(), y, 0.6 * angle.sin()]
            })
        })
        .collect();
    let mut indices = Vec::new();
    for side in 0..SIDES as u32 {
        let next = (side + 1) % SIDES as u32;
        let (low, low_next, high, high_next) = (side, next, side + 16, next + 16);
        indices.extend([low, high, low_next, low_next, high, high_next]);
        if side > 0 && next > 0 {
            indices.extend([0, next, side, 16, side + 16, next + 16]);
        }
    }
    let count = positions.len();
    SceneryModel {
        name: "TIRE.BIN".into(),
        normals: vec![[0.0, 1.0, 0.0]; count],
        uvs: vec![[0.0, 0.0]; count],
        tiles: vec![0; count],
        positions,
        indices,
        ..default()
    }
}

/// The built-in track with three cubes on it, well away from the truck: one fixed, one
/// loose and one moving, 20 m from the edge of the map and heading off it at 10 m/s.
fn app() -> App {
    app_with(|_| Vec::new())
}

/// The same, with more objects, placed from the point the cubes are put about.
fn app_with(more: impl Fn(Vec2) -> Vec<Placed>) -> App {
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
        faces_camera: false,
        visible: true,
    };
    track.scenery = Scenery {
        tile_size: 8,
        tiles: vec![vec![128; 8 * 8 * 4]],
        texture_cycles: Vec::new(),
        models: vec![cube(), tire(), panel()],
        objects: [
            object(away, SceneryMotion::Fixed),
            object(away + Vec2::X * 5.0, SceneryMotion::Loose { mass: 50.0 }),
            object(
                Vec2::new(half - 20.0, away.y),
                SceneryMotion::Moving {
                    velocity: Vec3::X * 10.0,
                },
            ),
        ]
        .into_iter()
        .chain(more(away))
        .collect(),
    };

    let mut app = App::new();
    app.insert_resource(Track(track))
        .add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            GamePhysicsPlugin,
            TrackPlugin,
            SceneryPlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .insert_resource(Time::<Fixed>::from_seconds(STEP))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            STEP,
        )));
    // Avian makes some of its resources in `Plugin::finish`, which `App::update` never
    // calls (see `physics`).
    app.finish();
    app.cleanup();
    app.update();
    app
}

fn run(app: &mut App, seconds: f64) {
    for _ in 0..(seconds / STEP) as usize {
        app.update();
    }
}

fn bodies(app: &mut App) -> Vec<(RigidBody, Transform, bool)> {
    let mut query = app
        .world_mut()
        .query_filtered::<(&RigidBody, &Transform, Has<Sleeping>), With<SceneryObject>>();
    let mut bodies: Vec<_> = query
        .iter(app.world())
        .map(|(body, transform, sleeping)| (*body, *transform, sleeping))
        .collect();
    bodies.sort_by_key(|(body, ..)| format!("{body:?}"));
    bodies
}

fn body(app: &mut App, kind: RigidBody) -> (Transform, bool) {
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
        [RigidBody::Dynamic, RigidBody::Kinematic, RigidBody::Static]
    );
}

#[test]
fn a_loose_object_left_alone_stands_asleep_where_it_was_put() {
    let mut app = app();
    let (placed, _) = body(&mut app, RigidBody::Dynamic);
    run(&mut app, 2.0);
    let (now, sleeping) = body(&mut app, RigidBody::Dynamic);
    assert!(sleeping);
    // It is held where it was put until it is put to sleep.
    assert!(
        now.translation.distance(placed.translation) < 1e-4,
        "{} to {}",
        placed.translation,
        now.translation
    );
}

#[test]
fn a_moving_object_comes_back_onto_the_map_at_the_other_side() {
    let mut app = app();
    let (placed, _) = body(&mut app, RigidBody::Kinematic);
    let half = app.world().resource::<Track>().heights.size() / 2.0;
    run(&mut app, 1.0);
    let (on, _) = body(&mut app, RigidBody::Kinematic);
    assert!((on.translation.x - placed.translation.x - 10.0).abs() < 0.2);
    // 20 m from the edge at 10 m/s: across it after 2 s, and 10 m in from the other side
    // after 3 s. Its height and its way across the map are as they were.
    run(&mut app, 2.0);
    let (back, _) = body(&mut app, RigidBody::Kinematic);
    assert!(
        (back.translation.x - (10.0 - half)).abs() < 0.2,
        "{} on a map {} across",
        back.translation,
        half * 2.0
    );
    assert!((back.translation.y - placed.translation.y).abs() < 1e-3);
    assert!((back.translation.z - placed.translation.z).abs() < 1e-3);
}

/// A loose fence panel, a centimetre thick, stands where it was put until something
/// touches it. Let go in the steps before it was put to sleep, The Graveyard's gates fell
/// over, and slept lying on the ground.
#[test]
fn a_loose_panel_on_its_edge_stays_upright() {
    let mut app = app_with(|away| {
        vec![Placed {
            model: 2,
            position: away + Vec2::Y * 10.0,
            height_above_ground: 2.5,
            yaw: 0.3,
            solid: true,
            motion: SceneryMotion::Loose { mass: 900.0 },
            faces_camera: false,
            visible: true,
        }]
    });
    let panel = |app: &mut App| {
        bodies(app)
            .into_iter()
            .filter(|(body, ..)| *body == RigidBody::Dynamic)
            .max_by(|a, b| a.1.translation.y.total_cmp(&b.1.translation.y))
            .unwrap()
    };
    let (_, placed, _) = panel(&mut app);
    run(&mut app, 2.0);
    let (_, now, sleeping) = panel(&mut app);
    assert!(sleeping);
    assert!(
        now.rotation.angle_between(placed.rotation) < 1e-4,
        "turned {}°",
        now.rotation.angle_between(placed.rotation).to_degrees()
    );
    assert!(now.translation.distance(placed.translation) < 1e-4);
}

/// A stack of loose tires put down a little apart, as Scrapyard Run puts them, all sleeps.
/// Put to sleep before they met, they woke as they did, and rocked for the whole race;
/// with forty of them awake, the game ran at 3 frames a second.
#[test]
fn a_stack_of_loose_tires_falls_asleep() {
    let mut app = app_with(|away| {
        (0..3)
            .map(|level| Placed {
                model: 1,
                position: away - Vec2::X * 5.0,
                height_above_ground: 0.303 * level as f32,
                yaw: 0.0,
                solid: true,
                motion: SceneryMotion::Loose { mass: 50.0 },
                faces_camera: false,
                visible: true,
            })
            .collect()
    });
    run(&mut app, 2.0);
    let loose: Vec<bool> = bodies(&mut app)
        .into_iter()
        .filter(|(body, ..)| *body == RigidBody::Dynamic)
        .map(|(_, _, sleeping)| sleeping)
        .collect();
    assert_eq!(loose, [true; 4]);
}
