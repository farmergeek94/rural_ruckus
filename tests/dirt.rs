//! The dirt slice in a headless app. Dirt is only a look, so an app that cannot draw gets
//! none, and the slice must not stop such an app from racing.
//!
//! How the dirt looks is checked by driving (see AGENTS.md).

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use monster_truck_rural_ruckus::dirt::{Dirt, DirtPlugin};
use monster_truck_rural_ruckus::track::{Track, TrackPlugin, builtin_track};
use monster_truck_rural_ruckus::truck::{TireContacts, TruckPlugin};

#[test]
fn a_headless_app_races_with_no_dirt() {
    let mut app = App::new();
    app.insert_resource(Track(builtin_track()))
        .add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            RapierPhysicsPlugin::<TireContacts>::default().in_fixed_schedule(),
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_plugins((TrackPlugin, TruckPlugin, DirtPlugin));
    for _ in 0..10 {
        app.update();
    }
    let dirt = app.world_mut().query::<&Dirt>().iter(app.world()).count();
    assert_eq!(dirt, 0);
}
