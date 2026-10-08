//! The graphics panel's keys, in a headless app: each reaches its slice's settings, and the
//! slice carries them to what it has already spawned.

use std::time::Duration;

use bevy::anti_alias::fxaa::Fxaa;
use bevy::image::ImageSampler;
use bevy::light::CascadeShadowConfig;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use monster_truck_rural_ruckus::camera::{CameraSettings, ChaseCameraPlugin};
use monster_truck_rural_ruckus::environment::{EnvironmentPlugin, EnvironmentSettings};
use monster_truck_rural_ruckus::graphics_debug::{GraphicsDebug, GraphicsDebugPlugin};
use monster_truck_rural_ruckus::physics::GamePhysicsPlugin;
use monster_truck_rural_ruckus::track::{
    TileMaterial, TileTextures, TrackPlugin, TrackSettings, tile_array,
};
use monster_truck_rural_ruckus::truck::TruckPlugin;

const STEP: f64 = 1.0 / 120.0;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        GamePhysicsPlugin,
        EnvironmentPlugin,
        TrackPlugin,
        TruckPlugin,
        ChaseCameraPlugin,
        GraphicsDebugPlugin,
    ))
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    // What a drawing app would have, so that there are textures to change.
    .init_asset::<Image>()
    .init_asset::<TileMaterial>()
    .init_resource::<ButtonInput<KeyCode>>()
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

/// One frame with `key` going down in it. Nothing clears the keys in a headless app.
fn press(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
    // And one for the slices to act on what changed.
    app.update();
}

#[test]
fn keys_do_nothing_until_the_panel_is_open() {
    let mut app = headless_app();
    for key in [KeyCode::F3, KeyCode::F4, KeyCode::F5, KeyCode::F6] {
        press(&mut app, key);
    }
    assert_eq!(
        *app.world().resource::<CameraSettings>(),
        CameraSettings::default()
    );
    assert_eq!(
        *app.world().resource::<EnvironmentSettings>(),
        EnvironmentSettings::default()
    );
    assert_eq!(app.world().resource::<TrackSettings>().anisotropy, 16);
}

#[test]
fn the_panel_shows_and_hides() {
    let mut app = headless_app();
    let mut panels = app
        .world_mut()
        .query_filtered::<(&Visibility, &Text), Without<Camera>>();
    let shown = |app: &App, panels: &mut QueryState<_, _>| -> Vec<(Visibility, String)> {
        panels
            .iter(app.world())
            .map(|(visibility, text): (&Visibility, &Text)| (*visibility, text.0.clone()))
            .filter(|(_, text)| text.contains("Graphics") || text.is_empty())
            .collect()
    };
    assert!(
        shown(&app, &mut panels)
            .iter()
            .all(|(visibility, _)| *visibility == Visibility::Hidden)
    );

    press(&mut app, KeyCode::F2);
    assert!(app.world().resource::<GraphicsDebug>().open);
    let open = shown(&app, &mut panels);
    assert!(
        open.iter().any(|(visibility, text)| {
            *visibility != Visibility::Hidden && text.contains("F3  Antialiasing      4x MSAA")
        }),
        "{open:?}"
    );

    press(&mut app, KeyCode::F2);
    assert!(!app.world().resource::<GraphicsDebug>().open);
}

#[test]
fn each_key_reaches_what_is_already_spawned() {
    let mut app = headless_app();

    // A texture of tiles and a material wearing it, as a textured track would have made.
    let settings = app.world().resource::<TrackSettings>().clone();
    let image = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(tile_array(4, &[vec![255; 4 * 4 * 4]], &settings));
    // Held on to, as the ground's entities would: a material nothing uses is dropped.
    let _material = app
        .world_mut()
        .resource_mut::<Assets<TileMaterial>>()
        .add(TileMaterial {
            base: StandardMaterial::default(),
            extension: TileTextures {
                tiles: image.clone(),
                ..default()
            },
        });

    press(&mut app, KeyCode::F2);

    // Antialiasing: 4x MSAA goes round to off, and on to FXAA, which takes one sample
    // and a pass over the picture.
    press(&mut app, KeyCode::F3);
    let mut cameras = app.world_mut().query::<(&Msaa, Has<Fxaa>)>();
    assert_eq!(cameras.single(app.world()).unwrap(), (&Msaa::Off, false));
    press(&mut app, KeyCode::F3);
    assert_eq!(cameras.single(app.world()).unwrap(), (&Msaa::Off, true));

    // Shadow distance: 300 m goes round to 50 m, with the four cascades it began with.
    press(&mut app, KeyCode::F5);
    let mut suns = app
        .world_mut()
        .query::<(&DirectionalLight, &CascadeShadowConfig)>();
    let (light, cascades) = suns.single(app.world()).unwrap();
    assert!(light.shadow_maps_enabled);
    assert_eq!(cascades.bounds.len(), 4);
    // The bounds are worked out as powers, so only nearly.
    assert!((cascades.bounds[3] - 50.0).abs() < 0.01);

    // Shadow cascades: 4 goes round to none.
    press(&mut app, KeyCode::F4);
    let (light, _) = suns.single(app.world()).unwrap();
    assert!(!light.shadow_maps_enabled);
    // And on to one.
    press(&mut app, KeyCode::F4);
    let (light, cascades) = suns.single(app.world()).unwrap();
    assert!(light.shadow_maps_enabled);
    assert_eq!(cascades.bounds.len(), 1);

    // Anisotropy: 16 goes round to 1.
    press(&mut app, KeyCode::F6);
    let images = app.world().resource::<Assets<Image>>();
    let ImageSampler::Descriptor(sampler) = &images.get(&image).unwrap().sampler else {
        panic!("no sampler");
    };
    assert_eq!(sampler.anisotropy_clamp, 1);
}
