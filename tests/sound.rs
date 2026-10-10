//! The sound slice in a headless app: without Bevy's `AudioPlugin` it holds its settings
//! and nothing else, and the app runs.
//!
//! What the engines sound like is judged by ear, in the game (see AGENTS.md).

use bevy::prelude::*;
use monster_truck_rural_ruckus::sound::{SoundPlugin, SoundSettings};

/// The app as a test or a server has it: no window, no audio device.
fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app
}

fn systems_in_update(app: &App) -> usize {
    app.get_schedule(Update)
        .map_or(0, |schedule| schedule.systems_len())
}

#[test]
fn without_audio_the_slice_holds_its_settings_and_adds_no_systems() {
    let mut app = headless_app();
    let before = systems_in_update(&app);
    app.add_plugins(SoundPlugin);
    app.finish();
    app.cleanup();

    let settings = app.world().resource::<SoundSettings>();
    assert_eq!(*settings, SoundSettings::default());
    assert_eq!(settings.volume, 0.7);
    assert_eq!(systems_in_update(&app), before);

    for _ in 0..5 {
        app.update();
    }
}

#[test]
fn the_settings_can_be_changed_at_any_time() {
    let mut app = headless_app();
    app.add_plugins(SoundPlugin);
    app.finish();
    app.cleanup();
    app.update();

    app.world_mut().resource_mut::<SoundSettings>().volume = 0.0;
    app.update();

    let settings = app.world().resource::<SoundSettings>();
    assert_eq!(settings.volume, 0.0);
}
