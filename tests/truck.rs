//! The truck slice in a headless app: a truck put down somewhere by `PlaceTruck`, as the
//! race does on the grid and at a checkpoint, stands where it was put.
//!
//! How a truck drives is checked by driving (see AGENTS.md).

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use monster_truck_rural_ruckus::physics::GamePhysicsPlugin;
use monster_truck_rural_ruckus::track::{Track, TrackPlugin, builtin_track};
use monster_truck_rural_ruckus::truck::{PlaceTruck, PlayerTruck, TruckConfig, TruckPlugin};

const STEP: f64 = 1.0 / 120.0;

fn headless_app() -> App {
    let mut app = App::new();
    app.insert_resource(Track(builtin_track()))
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
    app.update();
    app
}

/// The physics takes a moved `Transform` over only at the next step, and the tires' forces
/// are worked out before it. Moved without its pose in the physics, a truck put down 200 m
/// from where it stood was turned about its old place.
#[test]
fn a_truck_put_down_far_away_stands_where_it_was_put() {
    let mut app = headless_app();
    let (truck, standing) = {
        let world = app.world_mut();
        let mut trucks = world.query_filtered::<(Entity, &TruckConfig), With<PlayerTruck>>();
        let (truck, config) = trucks.single(world).unwrap();
        (truck, config.standing_height())
    };
    let track = app.world().resource::<Track>();
    let (x, z) = (40.0, 200.0);
    let ground = Vec3::new(x, track.heights.height_at(x, z), z);
    app.world_mut().write_message(PlaceTruck {
        truck,
        ground,
        yaw: 0.0,
    });
    // Measured with the physics' pose left behind: it rose a metre and leant 37 degrees
    // before it came down again. It is watched all the way, not only where it ends up.
    for _ in 0..(2.0 / STEP) as usize {
        app.update();
        let at = *app.world().get::<Transform>(truck).unwrap();
        // It may roll a little, on a slope with no brakes on.
        assert!(
            at.translation.xz().distance(ground.xz()) < 2.0,
            "{} for {}",
            at.translation,
            ground
        );
        assert!(
            (at.translation.y - (ground.y + standing)).abs() < 0.3,
            "{at:?}"
        );
        assert!(at.up().y > 0.99, "{at:?}");
    }
}
