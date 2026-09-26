//! The water slice in a headless app: a track with water gets a surface at its level, a
//! dry one gets none, the surface goes with the race, and in snow the water is solid ice.
//!
//! How water drives is physics, and is checked by driving (see AGENTS.md).

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_rapier3d::prelude::*;
use monster_truck_rural_ruckus::game_state::GameState;
use monster_truck_rural_ruckus::track::{Track, TrackPlugin, builtin_track};
use monster_truck_rural_ruckus::truck::{TireContacts, TruckPlugin};
use monster_truck_rural_ruckus::water::{WaterPlugin, WaterSurface};
use monster_truck_rural_ruckus::weather::{Weather, WeatherSettings};

const STEP: f64 = 1.0 / 120.0;

fn headless_app(water_level: Option<f32>) -> App {
    headless_app_in(water_level, Weather::Clear)
}

fn headless_app_in(water_level: Option<f32>, weather: Weather) -> App {
    let mut app = App::new();
    app.insert_resource(Track(builtin_track()))
        .insert_resource(WeatherSettings {
            weather,
            ..default()
        })
        .add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            RapierPhysicsPlugin::<TireContacts>::default().in_fixed_schedule(),
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
    app.world_mut().resource_mut::<Track>().0.water_level = water_level;
    app.add_plugins((TrackPlugin, TruckPlugin, WaterPlugin));
    app.update();
    app
}

fn surfaces(app: &mut App) -> Vec<Transform> {
    app.world_mut()
        .query_filtered::<&Transform, With<WaterSurface>>()
        .iter(app.world())
        .copied()
        .collect()
}

#[test]
fn a_track_with_water_has_a_surface_at_its_level() {
    let mut app = headless_app(Some(12.5));
    let surfaces = surfaces(&mut app);
    assert_eq!(surfaces.len(), 1);
    assert_eq!(surfaces[0].translation.y, 12.5);
}

#[test]
fn a_dry_track_has_no_water() {
    let mut app = headless_app(None);
    assert!(surfaces(&mut app).is_empty());
}

#[test]
fn the_water_goes_with_the_race() {
    let mut app = headless_app(Some(12.5));
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::FrontEnd);
    app.update();
    assert!(surfaces(&mut app).is_empty());
}

/// The tops of the slabs of ice that are solid.
fn solid_ice(app: &mut App) -> Vec<f32> {
    app.world_mut()
        .query_filtered::<(&Name, &GlobalTransform, &Collider), Without<ColliderDisabled>>()
        .iter(app.world())
        .filter(|(name, _, _)| name.as_str() == "Ice slab")
        .filter_map(|(_, at, collider)| {
            let slab = collider.as_cuboid()?;
            Some(at.translation().y + slab.half_extents().y)
        })
        .collect()
}

fn step(app: &mut App) {
    for _ in 0..3 {
        app.update();
    }
}

#[test]
fn snow_freezes_the_water_into_ice_at_its_level() {
    let mut app = headless_app_in(Some(12.5), Weather::Snow);
    step(&mut app);
    assert_eq!(solid_ice(&mut app), [12.5]);
}

#[test]
fn water_is_not_solid_until_it_freezes_and_thaws_again() {
    let mut app = headless_app(Some(12.5));
    step(&mut app);
    assert!(solid_ice(&mut app).is_empty());

    app.world_mut().resource_mut::<WeatherSettings>().weather = Weather::Snow;
    step(&mut app);
    assert_eq!(solid_ice(&mut app), [12.5]);

    app.world_mut().resource_mut::<WeatherSettings>().weather = Weather::Rain;
    step(&mut app);
    assert!(solid_ice(&mut app).is_empty());
}

#[test]
fn a_dry_track_has_no_ice_in_snow() {
    let mut app = headless_app_in(None, Weather::Snow);
    step(&mut app);
    assert!(solid_ice(&mut app).is_empty());
}
