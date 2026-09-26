//! Builds a race, takes it down and builds it again, in a headless app: the second race
//! must be the first one over again, with nothing left behind in between.

use std::time::Duration;

use bevy::ecs::resource::IsResource;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_rapier3d::prelude::*;
use monster_truck_rural_ruckus::camera::ChaseCameraPlugin;
use monster_truck_rural_ruckus::controls_help::ControlsHelpPlugin;
use monster_truck_rural_ruckus::environment::EnvironmentPlugin;
use monster_truck_rural_ruckus::game_state::GameState;
use monster_truck_rural_ruckus::race::{RaceClock, RacePlugin};
use monster_truck_rural_ruckus::scenery::SceneryPlugin;
use monster_truck_rural_ruckus::track::TrackPlugin;
use monster_truck_rural_ruckus::truck::{TireContacts, Truck, TruckPlugin};

const STEP: f64 = 1.0 / 120.0;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        RapierPhysicsPlugin::<TireContacts>::default().in_fixed_schedule(),
        EnvironmentPlugin,
        TrackPlugin,
        SceneryPlugin,
        TruckPlugin,
        RacePlugin,
        ChaseCameraPlugin,
        ControlsHelpPlugin,
    ))
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .init_resource::<ButtonInput<KeyCode>>()
    .insert_resource(Time::<Fixed>::from_seconds(STEP))
    .insert_resource(TimestepMode::Fixed {
        dt: STEP as f32,
        substeps: 1,
    })
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        STEP,
    )));
    app
}

fn run(app: &mut App, seconds: f64) {
    for _ in 0..(seconds / STEP) as usize {
        app.update();
    }
}

fn enter(app: &mut App, state: GameState) {
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(state);
    app.update();
}

fn truck(app: &mut App) -> Option<Transform> {
    let mut trucks = app.world_mut().query_filtered::<&Transform, With<Truck>>();
    trucks.single(app.world()).ok().copied()
}

/// Everything in the world that isn't a resource, which Bevy keeps as entities too.
fn entities(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<Entity, Without<IsResource>>()
        .iter(app.world())
        .count()
}

#[test]
fn a_race_can_be_taken_down_and_built_again() {
    let mut app = headless_app();
    app.update();
    let built = entities(&mut app);
    run(&mut app, 3.0);
    assert!(truck(&mut app).is_some(), "a truck to race");
    assert_eq!(entities(&mut app), built, "a race grew while it ran");
    assert!(app.world().resource::<RaceClock>().tick > 300);

    enter(&mut app, GameState::FrontEnd);
    assert!(truck(&mut app).is_none());
    let outside = entities(&mut app);
    assert!(outside < built);

    enter(&mut app, GameState::Racing);
    // The clock started again, and has counted this one update's step at most.
    assert!(app.world().resource::<RaceClock>().tick <= 1);
    assert_eq!(entities(&mut app), built);
    run(&mut app, 3.0);
    let first = truck(&mut app).expect("a truck to race again");

    // Built once more, it is the same race to the last bit. The race the app starts in
    // is not compared: Bevy's first update runs no physics step, so that truck's first
    // step comes after it is placed and before the ground is in the physics world, and it
    // dips 0.4 mm and settles a millimetre away from where a rebuilt one does.
    enter(&mut app, GameState::FrontEnd);
    assert_eq!(entities(&mut app), outside);
    enter(&mut app, GameState::Racing);
    assert_eq!(entities(&mut app), built);
    run(&mut app, 3.0);
    let second = truck(&mut app).expect("a truck to race a third time");
    assert!(
        first.translation.distance(second.translation) < 1e-3,
        "{} and then {}",
        first.translation,
        second.translation
    );

    // Leaving twice leaves the same nothing behind.
    enter(&mut app, GameState::FrontEnd);
    assert_eq!(entities(&mut app), outside);
}
