//! The weather slice in a headless app: the tires grip as the weather says, every truck
//! alike, F7 goes on to the next weather, and the trucks' lamps shine at dusk and at night. The sky, fog, rain and snow are only a look,
//! and an app that cannot draw gets none of them.
//!
//! How the weather looks, and how a wet truck drives, is checked by driving (see AGENTS.md).

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use monster_truck_rural_ruckus::track::{Track, TrackPlugin, builtin_track};
use monster_truck_rural_ruckus::truck::{
    ComputerTrucks, TireContacts, TruckConfig, TruckData, TruckLamps, TruckPlugin,
};
use monster_truck_rural_ruckus::weather::{TimeOfDay, Weather, WeatherPlugin, WeatherSettings};

fn headless_app(weather: Weather) -> App {
    headless_app_with(WeatherSettings {
        weather,
        ..default()
    })
}

fn headless_app_with(settings: WeatherSettings) -> App {
    let mut app = App::new();
    app.insert_resource(Track(builtin_track()))
        .insert_resource(ComputerTrucks(vec![TruckData::builtin().in_color(1)]))
        .insert_resource(settings)
        .add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            RapierPhysicsPlugin::<TireContacts>::default().in_fixed_schedule(),
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_plugins((TrackPlugin, TruckPlugin, WeatherPlugin));
    for _ in 0..3 {
        app.update();
    }
    app
}

/// Every truck's grip, and its handbrake grip.
fn grips(app: &mut App) -> Vec<(f32, f32)> {
    app.world_mut()
        .query::<&TruckConfig>()
        .iter(app.world())
        .map(|config| (config.grip, config.handbrake_grip))
        .collect()
}

#[test]
fn clear_weather_leaves_the_grip_alone() {
    let dry = TruckConfig::default();
    let grips = grips(&mut headless_app(Weather::Clear));
    assert_eq!(grips.len(), 2);
    for grip in grips {
        assert_eq!(grip, (dry.grip, dry.handbrake_grip));
    }
}

#[test]
fn every_truck_grips_less_in_the_rain_and_as_before_when_it_stops() {
    let dry = TruckConfig::default();
    let mut app = headless_app(Weather::Rain);
    let wet = grips(&mut app);
    assert_eq!(wet.len(), 2);
    for (grip, handbrake_grip) in wet {
        assert!(grip < dry.grip, "{grip}");
        assert!(handbrake_grip < dry.handbrake_grip, "{handbrake_grip}");
    }

    // Not less and less each frame: a share of the dry grip.
    let before = grips(&mut app);
    app.update();
    assert_eq!(grips(&mut app), before);

    app.world_mut().resource_mut::<WeatherSettings>().weather = Weather::Clear;
    app.update();
    for grip in grips(&mut app) {
        assert_eq!(grip, (dry.grip, dry.handbrake_grip));
    }
}

#[test]
fn f7_changes_the_weather() {
    let mut app = headless_app(Weather::Clear);
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.press(KeyCode::F7);
    app.update();
    assert_eq!(
        app.world().resource::<WeatherSettings>().weather,
        Weather::Overcast
    );
}

#[test]
fn random_weather_stays_random_for_the_next_race() {
    let app = headless_app_with(WeatherSettings {
        random: true,
        ..default()
    });
    let settings = *app.world().resource::<WeatherSettings>();
    assert!(settings.random);
    assert!(Weather::ALL.contains(&settings.weather));
}

#[test]
fn the_lamps_shine_at_dusk_and_at_night_only() {
    for time in TimeOfDay::ALL {
        let mut app = headless_app_with(WeatherSettings {
            time_of_day: time,
            ..default()
        });
        let lamps = *app.world().resource::<TruckLamps>();
        assert_eq!(lamps.lit, time != TimeOfDay::Day, "{time:?}");
        // The cones of the beams are seen only at night.
        assert_eq!(lamps.cones, time == TimeOfDay::Night, "{time:?}");
        // And every lamp of every truck with them.
        let shown: Vec<bool> = app
            .world_mut()
            .query_filtered::<(&Name, &Visibility), ()>()
            .iter(app.world())
            .filter(|(name, _)| name.as_str() == "Lamp")
            .map(|(_, visibility)| *visibility != Visibility::Hidden)
            .collect();
        assert!(!shown.is_empty());
        assert!(shown.iter().all(|&on| on == lamps.lit), "{time:?}");
    }
}
