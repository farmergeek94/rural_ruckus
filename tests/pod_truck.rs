//! Converts and draws a real Monster Truck Madness 2 truck. Such trucks can't be committed, so this
//! reads `trucks/99BFoot.pod` and passes trivially when it isn't there.

use std::path::Path;
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use monster_truck_rural_ruckus::base_game::BaseGame;
use monster_truck_rural_ruckus::physics::GamePhysicsPlugin;
use monster_truck_rural_ruckus::track::TrackPlugin;
use monster_truck_rural_ruckus::truck::{
    ChosenTruck, TruckData, TruckPlugin, TruckVisual, Wheel, load_pod, peek_pod, pod_holds_truck,
};

const STEP: f64 = 1.0 / 120.0;
const FOOT: f32 = 0.3048;

/// The archives in a folder of the repository and in every folder under it, leaving out
/// anything else there (the `.gitkeep` that keeps an empty folder in git).
fn pods(folder: &str) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut folders = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join(folder)];
    while let Some(folder) = folders.pop() {
        for path in std::fs::read_dir(folder)
            .into_iter()
            .flatten()
            .flatten()
            .map(|file| file.path())
        {
            if path.is_dir() {
                folders.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pod"))
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

fn truck_pod(file_name: &str) -> Option<TruckData> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("trucks")
        .join(file_name);
    path.exists()
        .then(|| load_pod(&path, &BaseGame::none()).unwrap())
}

/// The game minus rendering and windowing, advancing exactly one physics step per update.
fn headless_app(truck: TruckData) -> App {
    let mut app = App::new();
    app.insert_resource(ChosenTruck(truck))
        .add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            GamePhysicsPlugin,
            TrackPlugin,
            TruckPlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(Time::<Fixed>::from_seconds(STEP))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            STEP,
        )));
    // Avian makes some of its resources in `Plugin::finish`, which `App::update` never
    // calls (see `physics`).
    app.finish();
    app.cleanup();
    app
}

fn run(app: &mut App, seconds: f64) {
    for _ in 0..(seconds / STEP) as usize {
        app.update();
    }
}

#[test]
fn converts_to_the_games_conventions() {
    let Some(truck) = truck_pod("99BFoot.pod") else {
        return;
    };
    assert_eq!(truck.name, "Bigfoot 1999 Superduty");

    // A 72 inch tire.
    assert!((truck.config.wheel_radius - 3.0 * FOOT).abs() < 1e-4);
    // Front wheels first and at -Z, left wheels at -X, 9 ft apart.
    let [front_left, front_right, rear_left, rear_right] = truck.config.wheel_rest;
    assert!((front_right.x - front_left.x - 9.0 * FOOT).abs() < 1e-4);
    assert!((front_left.z + 6.61 * FOOT).abs() < 1e-4);
    assert!((rear_right.z - 6.38 * FOOT).abs() < 1e-4);
    assert_eq!(rear_left.y, front_right.y);
    assert_eq!(truck.collider_points.len(), 12);

    // Everything is in the archive: a body, both tires, an axle, and their paintwork.
    let looks = truck.looks.unwrap();
    let body = looks.body.unwrap();
    assert!(looks.left_tire.is_some() && looks.right_tire.is_some() && looks.axle.is_some());
    assert!(body.parts.iter().all(|part| part.texture.is_some()));
    assert!(
        looks
            .textures
            .iter()
            .all(|texture| texture.rgba.len() == texture.size * texture.size * 4)
    );
    // The body's nose is at -Z: it reaches 9.5 ft ahead of the origin and 10.8 behind.
    let reach = |pick: fn(f32, f32) -> f32, start: f32| {
        body.parts
            .iter()
            .flat_map(|part| &part.positions)
            .map(|position| position[2])
            .fold(start, pick)
    };
    assert!((reach(f32::min, f32::INFINITY) + 9.527 * FOOT).abs() < 0.01);
    assert!((reach(f32::max, f32::NEG_INFINITY) - 10.828 * FOOT).abs() < 0.01);
}

#[test]
fn tells_a_truck_from_a_track() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (truck, track) = (
        root.join("trucks/99BFoot.pod"),
        root.join("tracks/MyTrack.pod"),
    );
    if truck.exists() {
        assert!(pod_holds_truck(&truck));
    }
    if track.exists() {
        assert!(!pod_holds_truck(&track));
        assert!(load_pod(&track, &BaseGame::none()).is_err());
    }
    assert!(!pod_holds_truck(&root.join("trucks/no_such_truck.pod")));
}

/// A list of trucks reads each archive's name alone, which must be the name that loading
/// the whole truck gives, and must tell a truck from a track. Prints how long each takes:
/// `cargo test --test pod_truck peeking -- --nocapture`.
#[test]
fn peeking_gives_the_name_that_loading_does() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for file in pods("trucks") {
        let path = file;
        let started = std::time::Instant::now();
        let name = peek_pod(&path).unwrap().expect("a truck in trucks/");
        let peeked = started.elapsed();
        let truck = load_pod(&path, &BaseGame::none()).unwrap();
        println!(
            "{}: peeked in {peeked:?}, loaded in {:?}",
            path.display(),
            started.elapsed() - peeked
        );
        assert_eq!(name, truck.name);
    }
    for file in pods("tracks") {
        assert_eq!(peek_pod(&file), Ok(None));
    }
    assert!(peek_pod(&root.join("trucks/no_such_truck.pod")).is_err());
}

/// With somewhere to put textures, as there is in the real game, the truck's own models
/// are drawn: every mesh of the body, a tire model hanging from each wheel, an axle
/// between each pair, level with its hubs, and the bars, shocks and driveshaft that join
/// the axles to the body.
#[test]
fn draws_its_own_models_when_there_are_textures_to_draw_with() {
    let Some(truck) = truck_pod("99BFoot.pod") else {
        return;
    };
    let looks = truck.looks.clone().unwrap();
    let parts = |model: &Option<monster_truck_rural_ruckus::truck::TruckModel>| {
        model.as_ref().unwrap().parts.len()
    };
    let expected = parts(&looks.body)
        + 2 * parts(&looks.left_tire)
        + 2 * parts(&looks.right_tire)
        + 2 * parts(&looks.axle)
        + looks.axle_bars.links.len()
        + looks.shocks.links.len()
        + looks.driveshaft.links.len();
    // Bigfoot is an MTM2 truck: one set of bars, from each side of the body to each axle,
    // and two shocks at each wheel.
    assert_eq!(looks.axle_bars.links.len(), 4);
    assert_eq!(looks.shocks.links.len(), 8);

    let mut app = headless_app(truck);
    app.init_asset::<Image>();
    run(&mut app, 2.0);

    // The ground is drawn too, in chunks: count the truck's parts alone, which are all
    // under the `TruckVisual`.
    let mut meshes = app
        .world_mut()
        .query_filtered::<Entity, (With<Mesh3d>, With<MeshMaterial3d<StandardMaterial>>)>();
    let entities: Vec<Entity> = meshes.iter(app.world()).collect();
    let world = app.world();
    // The lamps' glows and beams are not the models'.
    let under_the_truck = |mut entity: Entity| loop {
        match world.get::<ChildOf>(entity) {
            Some(parent)
                if world
                    .get::<Name>(parent.0)
                    .is_some_and(|name| name.as_str() == "Lamp") =>
            {
                return false;
            }
            Some(parent) if world.get::<TruckVisual>(parent.0).is_some() => return true,
            Some(parent) => entity = parent.0,
            None => return false,
        }
    };
    let truck_parts: Vec<Entity> = entities
        .iter()
        .copied()
        .filter(|&entity| under_the_truck(entity))
        .collect();
    assert_eq!(truck_parts.len(), expected);
    let normal_maps = looks
        .textures
        .iter()
        .filter(|texture| texture.normal_map.is_some())
        .count();
    assert_eq!(
        app.world().resource::<Assets<Image>>().len(),
        looks.textures.len() + normal_maps
    );
    // Every material that has a texture has one that exists, with its mipmaps.
    let materials = app.world().resource::<Assets<StandardMaterial>>();
    let images = app.world().resource::<Assets<Image>>();
    let mut textured = 0;
    for &part in &truck_parts {
        let material = app
            .world()
            .get::<MeshMaterial3d<StandardMaterial>>(part)
            .unwrap();
        if let Some(texture) = &materials.get(&material.0).unwrap().base_color_texture {
            let image = images.get(texture).expect("a texture that isn't there");
            assert!(image.texture_descriptor.mip_level_count > 1);
            textured += 1;
        }
    }
    // The driveshaft has no texture, nor has a bar or shock whose texture is the base
    // game's.
    let untextured = [&looks.axle_bars, &looks.shocks, &looks.driveshaft]
        .iter()
        .filter(|kind| kind.texture.is_none())
        .map(|kind| kind.links.len())
        .sum::<usize>();
    assert_eq!(textured, expected - untextured);

    // The axles: the only children of the drawn truck that are neither wheels, meshes nor
    // lamps.
    let mut visuals = app
        .world_mut()
        .query_filtered::<Entity, With<TruckVisual>>();
    let visual = visuals.single(app.world()).unwrap();
    let mut wheels = app.world_mut().query::<(&Wheel, &Transform)>();
    let hub_height = wheels.iter(app.world()).next().unwrap().1.translation.y;
    let mut others = app
        .world_mut()
        .query_filtered::<(&ChildOf, &Transform, Option<&Name>), (Without<Wheel>, Without<Mesh3d>)>(
        );
    let axles: Vec<&Transform> = others
        .iter(app.world())
        .filter(|(parent, _, name)| {
            parent.parent() == visual && name.is_none_or(|name| name.as_str() != "Lamp")
        })
        .map(|(_, transform, _)| transform)
        .collect();
    assert_eq!(axles.len(), 2);
    for axle in axles {
        assert!(axle.translation.x.abs() < 1e-4, "{axle:?}");
        assert!((axle.translation.y - hub_height).abs() < 0.01, "{axle:?}");
        assert!(axle.rotation.angle_between(Quat::IDENTITY) < 0.01);
    }
}
