//! The particles slice in a headless app. Particles are only a look, so an app that cannot
//! draw gets none, and the slice must not stop such an app from racing.
//!
//! How the dirt and the spray look is checked by driving (see AGENTS.md).

use bevy::prelude::*;
use monster_truck_rural_ruckus::particles::{Dirt, ParticlesPlugin};
use monster_truck_rural_ruckus::physics::GamePhysicsPlugin;
use monster_truck_rural_ruckus::track::{Track, TrackPlugin, builtin_track};
use monster_truck_rural_ruckus::truck::TruckPlugin;

#[test]
fn a_headless_app_races_with_no_dirt() {
    let mut app = App::new();
    app.insert_resource(Track(builtin_track()))
        .add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            GamePhysicsPlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_plugins((TrackPlugin, TruckPlugin, ParticlesPlugin));
    // Avian makes some of its resources in `Plugin::finish`, which `App::update` never
    // calls (see `physics`).
    app.finish();
    app.cleanup();
    for _ in 0..10 {
        app.update();
    }
    let dirt = app.world_mut().query::<&Dirt>().iter(app.world()).count();
    assert_eq!(dirt, 0);
}
