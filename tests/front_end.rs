//! The front end in a headless app: the real ui crate, the real slices, and no window.
//! The player is played by writing `PlayerDid`, as the ui crate's own input does.

use std::sync::Arc;
use std::time::Duration;

use bevy::ecs::resource::IsResource;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use monster_truck_rural_ruckus::camera::{CameraSettings, ChaseCameraPlugin};
use monster_truck_rural_ruckus::controls_help::{ControlsHelpPlugin, ControlsHelpSettings};
use monster_truck_rural_ruckus::dirt::DirtSettings;
use monster_truck_rural_ruckus::display::{DisplayPlugin, DisplaySettings, ScreenMode, Vsync};
use monster_truck_rural_ruckus::environment::{EnvironmentPlugin, EnvironmentSettings};
use monster_truck_rural_ruckus::front_end::{BaseGameChoice, FrontEndPlugin, FrontEndSettings};
use monster_truck_rural_ruckus::game_state::GameState;
use monster_truck_rural_ruckus::keys::{Control, KeyBindings};
use monster_truck_rural_ruckus::physics::GamePhysicsPlugin;
use monster_truck_rural_ruckus::race::{RaceClock, RacePause, RacePlugin, RaceSettings, Racer};
use monster_truck_rural_ruckus::scenery::SceneryPlugin;
use monster_truck_rural_ruckus::store::Store;
use monster_truck_rural_ruckus::track::{Track, TrackPlugin, TrackSettings, builtin_track};
use monster_truck_rural_ruckus::truck::{
    ChosenTruck, ComputerTrucks, PlayerTruck, SpeedUnits, Truck, TruckConfig, TruckData,
    TruckDisplay, TruckPlugin, TruckSetup,
};
use monster_truck_rural_ruckus::ui::{
    Action, Catalogue, Choices, Dials, Entry, FolderBrowser, PlayerDid, Screen, Settings, Turntable,
};
use monster_truck_rural_ruckus::water::WaterSettings;

const STEP: f64 = 1.0 / 120.0;

/// Folders with nothing in them, so that only the built-in truck and track are listed.
fn nowhere() -> FrontEndSettings {
    FrontEndSettings {
        trucks_folder: "no/such/trucks".into(),
        tracks_folder: "no/such/tracks".into(),
        builtin: true,
        base_game_folders: vec!["no/such/base".into()],
        ask_for_base_game: false,
    }
}

fn headless_app(settings: FrontEndSettings, store: Option<Arc<Store>>) -> App {
    let mut app = App::new();
    app.insert_resource(settings)
        .add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            GamePhysicsPlugin,
            DisplayPlugin,
            EnvironmentPlugin,
            TrackPlugin,
            SceneryPlugin,
            TruckPlugin,
            RacePlugin,
            ChaseCameraPlugin,
            ControlsHelpPlugin,
        ))
        // As `main.rs` does when it is given nothing to race.
        .insert_state(GameState::FrontEnd)
        .add_plugins(FrontEndPlugin { store })
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

fn player_does(app: &mut App, action: Action) {
    app.world_mut().write_message(PlayerDid(action));
    app.update();
}

/// Loading happens on another thread, so give it frames, and time, until it is done.
fn run_until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) {
    for _ in 0..2000 {
        app.update();
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("waited too long for {what}");
}

fn state(app: &App) -> GameState {
    *app.world().resource::<State<GameState>>().get()
}

/// `None` outside a race.
fn pause(app: &App) -> Option<RacePause> {
    app.world()
        .get_resource::<State<RacePause>>()
        .map(|pause| *pause.get())
}

fn count<F: bevy::ecs::query::QueryFilter>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<(), F>()
        .iter(app.world())
        .count()
}

/// The names of the trucks on the turntable, which ought to be one at most.
fn on_show(app: &mut App) -> Vec<String> {
    let turntable = app
        .world_mut()
        .query_filtered::<Entity, With<Turntable>>()
        .single(app.world())
        .expect("one turntable");
    app.world_mut()
        .query::<(&TruckDisplay, &ChildOf)>()
        .iter(app.world())
        .filter(|(_, parent)| parent.parent() == turntable)
        .map(|(display, _)| display.0.name.clone())
        .collect()
}

fn press(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    // There is no input plugin here to forget the press.
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.update();
}

#[test]
fn opens_with_a_truck_on_the_turntable_and_no_race() {
    let mut app = headless_app(nowhere(), None);
    run_until(&mut app, "the built-in truck", |app| {
        !on_show(app).is_empty()
    });

    assert_eq!(state(&app), GameState::FrontEnd);
    assert_eq!(count::<With<Truck>>(&mut app), 0);
    assert_eq!(on_show(&mut app), ["Built-in truck"]);
    // Standing on its wheels, which the truck slice builds the frame after.
    app.update();
    assert_eq!(
        count::<With<monster_truck_rural_ruckus::truck::Wheel>>(&mut app),
        4
    );

    // With both folders empty there is still something to race.
    let catalogue = app.world().resource::<Catalogue>();
    assert_eq!(catalogue.trucks.len(), 1);
    assert_eq!(catalogue.tracks[0].name, builtin_track().name);
}

#[test]
fn without_the_builtin_flag_only_archives_are_listed() {
    let app = headless_app(
        FrontEndSettings {
            builtin: false,
            ..nowhere()
        },
        None,
    );
    let catalogue = app.world().resource::<Catalogue>();
    assert!(catalogue.trucks.is_empty());
    assert!(catalogue.tracks.is_empty());
}

/// Community archives are often malformed. One that lists and then won't load is greyed
/// out with the reason, and isn't left on the turntable, or raced.
#[test]
fn an_archive_that_will_not_load_is_greyed_out() {
    let mut app = headless_app(nowhere(), None);
    run_until(&mut app, "the built-in truck", |app| {
        !on_show(app).is_empty()
    });
    app.world_mut()
        .resource_mut::<Catalogue>()
        .trucks
        .push(Entry {
            id: "no/such/trucks/gone.pod".into(),
            name: "Gone".into(),
            detail: "no/such/trucks/gone.pod".into(),
            available: true,
        });
    app.update();

    player_does(&mut app, Action::PickTruck(1));
    run_until(&mut app, "the load to fail", |app| {
        !app.world().resource::<Catalogue>().trucks[1].available
    });
    app.update();
    let entry = app.world().resource::<Catalogue>().trucks[1].clone();
    assert!(entry.detail.contains("gone.pod"), "{}", entry.detail);
    assert_ne!(entry.detail, "no/such/trucks/gone.pod");
    assert!(on_show(&mut app).is_empty());

    player_does(&mut app, Action::Go);
    app.update();
    assert_eq!(state(&app), GameState::FrontEnd);
}

/// Esc, and the choice of the pause dialog that is `downs` down from the top.
fn pause_and_choose(app: &mut App, downs: usize) {
    press(app, KeyCode::Escape);
    for _ in 0..downs {
        press(app, KeyCode::ArrowDown);
    }
    press(app, KeyCode::Enter);
}

/// The last choice of the pause dialog.
fn cancel_the_race(app: &mut App) {
    pause_and_choose(app, 3);
}

/// Restarting from the pause dialog builds the whole race again: new trucks, a new clock,
/// and nothing of the old race left over or doubled. So does Enter after the finish.
#[test]
fn restarting_builds_the_whole_race_again() {
    let mut app = headless_app(nowhere(), None);
    run_until(&mut app, "the built-in truck", |app| {
        !on_show(app).is_empty()
    });
    player_does(&mut app, Action::Go);
    for _ in 0..30 {
        app.update();
    }
    assert!(app.world().resource::<RaceClock>().tick > 0);
    let trucks = |app: &mut App| {
        let mut trucks: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<Truck>>()
            .iter(app.world())
            .collect();
        trucks.sort();
        trucks
    };
    let entities = |app: &mut App| count::<Without<IsResource>>(app);
    let old_trucks = trucks(&mut app);
    let before = entities(&mut app);

    // The second choice.
    pause_and_choose(&mut app, 1);
    assert_eq!(state(&app), GameState::Racing);
    assert_eq!(pause(&app), Some(RacePause::Running));
    assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    assert!(app.world().resource::<RaceClock>().tick < 5);
    let new_trucks = trucks(&mut app);
    assert_eq!(new_trucks.len(), old_trucks.len());
    assert!(new_trucks.iter().all(|truck| !old_trucks.contains(truck)));
    assert_eq!(entities(&mut app), before);

    // Enter does nothing until the player has finished.
    for _ in 0..30 {
        app.update();
    }
    press(&mut app, KeyCode::Enter);
    assert_eq!(trucks(&mut app), new_trucks);
    app.world_mut()
        .query_filtered::<&mut Racer, With<PlayerTruck>>()
        .single_mut(app.world_mut())
        .expect("the player's racer")
        .progress
        .finished = Some(100);
    press(&mut app, KeyCode::Enter);
    assert!(app.world().resource::<RaceClock>().tick < 5);
    let third_trucks = trucks(&mut app);
    assert_eq!(third_trucks.len(), new_trucks.len());
    assert!(third_trucks.iter().all(|truck| !new_trucks.contains(truck)));
    assert_eq!(entities(&mut app), before);
}

#[test]
fn go_races_what_was_chosen_and_cancelling_comes_back_to_it() {
    let mut app = headless_app(nowhere(), None);
    run_until(&mut app, "the built-in truck", |app| {
        !on_show(app).is_empty()
    });
    let entities = |app: &mut App| count::<Without<IsResource>>(app);

    player_does(&mut app, Action::MoreLaps);
    player_does(&mut app, Action::MoreLaps);
    player_does(&mut app, Action::SetDial(0, 4));
    player_does(&mut app, Action::SetDial(2, 0));
    player_does(&mut app, Action::SetDial(3, 3));
    // Three opponents to begin with, and one fewer.
    player_does(&mut app, Action::FewerOpponents);
    player_does(&mut app, Action::Show(Screen::Truck));
    let before = entities(&mut app);

    player_does(&mut app, Action::Go);
    app.update();
    assert_eq!(state(&app), GameState::Racing);
    assert_eq!(count::<With<Turntable>>(&mut app), 0);
    assert_eq!(count::<With<TruckDisplay>>(&mut app), 0);
    assert_eq!(app.world().resource::<RaceSettings>().laps, 5);
    assert_eq!(app.world().resource::<ChosenTruck>().name, "Built-in truck");
    assert_eq!(app.world().resource::<Track>().name, builtin_track().name);
    assert_eq!(
        *app.world().resource::<TruckSetup>(),
        TruckSetup {
            suspension: 1.0,
            gearing: 0.0,
            rear_steering: -1.0,
            grip: 0.5,
        }
    );
    // The computer drives as many trucks as were asked for, each in a colour of its own.
    let computer_trucks = &app.world().resource::<ComputerTrucks>().0;
    assert_eq!(computer_trucks.len(), 2);
    assert_ne!(computer_trucks[0].body_color, computer_trucks[1].body_color);
    assert_ne!(
        computer_trucks[0].body_color,
        TruckData::builtin().body_color
    );
    assert_eq!(count::<With<Truck>>(&mut app), 3);
    // And the truck that the player races is set up that way.
    let mut trucks = app
        .world_mut()
        .query_filtered::<&TruckConfig, (With<Truck>, With<PlayerTruck>)>();
    let config = trucks.single(app.world()).expect("a truck to race");
    assert!(config.spring > TruckConfig::default().spring * 1.5);
    assert_eq!(config.rear_steer_ratio, 0.0);

    // Esc pauses, and the clock stops with it.
    press(&mut app, KeyCode::Escape);
    assert_eq!(state(&app), GameState::Racing);
    assert_eq!(pause(&app), Some(RacePause::Paused));
    assert!(app.world().resource::<Time<Virtual>>().is_paused());
    let tick = app.world().resource::<RaceClock>().tick;
    app.update();
    assert_eq!(app.world().resource::<RaceClock>().tick, tick);

    // Esc again carries on.
    press(&mut app, KeyCode::Escape);
    assert_eq!(pause(&app), Some(RacePause::Running));
    assert!(!app.world().resource::<Time<Virtual>>().is_paused());

    cancel_the_race(&mut app);
    run_until(&mut app, "the truck to be back on show", |app| {
        !on_show(app).is_empty()
    });
    assert_eq!(state(&app), GameState::FrontEnd);
    assert_eq!(pause(&app), None);
    assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    assert_eq!(count::<With<Truck>>(&mut app), 0);
    assert_eq!(app.world().resource::<Choices>().laps, 5);
    assert_eq!(app.world().resource::<Dials>().0[0].step, 4);
    // Nothing of the race is left, and nothing of the front end was doubled.
    assert_eq!(entities(&mut app), before);
}

#[test]
fn exit_closes_the_game() {
    let mut app = headless_app(nowhere(), None);
    app.update();
    assert!(app.should_exit().is_none());
    player_does(&mut app, Action::Exit);
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}

/// Numbers that no dial could have written are brought back within range.
#[test]
fn remembered_numbers_are_kept_within_range() {
    let store = Arc::new(Store::in_memory());
    store
        .write(|batch| {
            batch
                .set_text("choice.truck", "trucks/long_gone.pod")
                .set_number("choice.laps", 5000.0)
                .set_number("choice.opponents", 5000.0)
                .set_number("setup.suspension", -40.0);
        })
        .unwrap();
    let app = headless_app(nowhere(), Some(store));
    let choices = app.world().resource::<Choices>();
    assert_eq!(
        (choices.truck, choices.laps),
        (0, monster_truck_rural_ruckus::ui::MOST_LAPS)
    );
    assert_eq!(
        choices.opponents,
        monster_truck_rural_ruckus::ui::MOST_OPPONENTS
    );
    assert_eq!(app.world().resource::<TruckSetup>().suspension, -1.0);
    assert_eq!(app.world().resource::<Dials>().0[0].step, 0);
}

/// The line of the options screen with this id.
fn option(app: &App, id: &str) -> usize {
    app.world()
        .resource::<Settings>()
        .0
        .iter()
        .position(|setting| setting.id == id)
        .unwrap_or_else(|| panic!("no option {id}"))
}

/// Its value by name.
fn set_option(app: &mut App, id: &str, value: &str) {
    let line = option(app, id);
    let values = &app.world().resource::<Settings>().0[line].values;
    let value = values
        .iter()
        .position(|each| each == value)
        .unwrap_or_else(|| panic!("{id} has no value {value}"));
    player_does(app, Action::SetSetting(line, value));
}

#[test]
fn an_option_goes_to_its_slice_at_once_and_is_remembered() {
    let store = Arc::new(Store::in_memory());
    let mut first = headless_app(nowhere(), Some(store.clone()));
    first.update();
    player_does(&mut first, Action::Show(Screen::Options));

    set_option(&mut first, "option.screen", "Windowed");
    set_option(&mut first, "option.vsync", "Strict");
    set_option(&mut first, "option.antialiasing", "Off");
    set_option(&mut first, "option.shadows", "Low");
    set_option(&mut first, "option.shadow_distance", "50 m");
    set_option(&mut first, "option.anisotropy", "4x");
    set_option(&mut first, "option.mipmaps", "Off");
    set_option(&mut first, "option.camera", "Far");
    set_option(&mut first, "option.smooth_terrain", "On");
    set_option(&mut first, "option.speed_units", "mph");
    set_option(&mut first, "option.controls_help", "Hide");
    set_option(&mut first, "option.splashes", "Off");
    set_option(&mut first, "option.dirt", "Off");

    let check = |app: &App| {
        let world = app.world();
        assert_eq!(
            *world.resource::<DisplaySettings>(),
            DisplaySettings {
                screen: ScreenMode::Windowed,
                vsync: Vsync::Strict,
            }
        );
        let camera = world.resource::<CameraSettings>();
        assert_eq!(
            camera.antialiasing,
            monster_truck_rural_ruckus::camera::Antialiasing::Off
        );
        assert!(camera.rig.distance > CameraSettings::default().rig.distance);
        let environment = world.resource::<EnvironmentSettings>();
        assert_eq!(
            (environment.shadow_cascades, environment.shadow_distance),
            (1, 50.0)
        );
        assert_eq!(
            *world.resource::<TrackSettings>(),
            TrackSettings {
                smooth_terrain: true,
                mipmaps: false,
                anisotropy: 4,
            }
        );
        assert_eq!(*world.resource::<SpeedUnits>(), SpeedUnits::Mph);
        assert!(!world.resource::<ControlsHelpSettings>().show);
        assert!(!world.resource::<WaterSettings>().splashes);
        assert!(!world.resource::<DirtSettings>().on);
    };
    check(&first);

    // Kept without GO being pressed, and set again next time.
    let mut second = headless_app(nowhere(), Some(store));
    second.update();
    check(&second);
    let line = option(&second, "option.shadows");
    assert_eq!(
        second.world().resource::<Settings>().0[line].chosen_value(),
        "Low"
    );

    // And all put back at once, from the options screen.
    player_does(&mut second, Action::Show(Screen::Options));
    player_does(&mut second, Action::RestoreDefaults);
    assert_eq!(
        *second.world().resource::<DisplaySettings>(),
        DisplaySettings::default()
    );
    assert_eq!(
        *second.world().resource::<TrackSettings>(),
        TrackSettings::default()
    );
    assert_eq!(
        *second.world().resource::<CameraSettings>(),
        CameraSettings::default()
    );
    assert_eq!(*second.world().resource::<SpeedUnits>(), SpeedUnits::Kmh);
    assert!(second.world().resource::<WaterSettings>().splashes);
    assert!(second.world().resource::<DirtSettings>().on);
}

/// The INTEGRATED GRAPHICS button beside RESTORE DEFAULTS turns down what costs most on a
/// processor's built-in graphics, and leaves the other lines as they were.
#[test]
fn integrated_graphics_is_a_preset_on_the_options_screen() {
    let mut app = headless_app(nowhere(), None);
    app.update();
    player_does(&mut app, Action::Show(Screen::Options));
    set_option(&mut app, "option.mipmaps", "Off");
    let preset = app
        .world()
        .resource::<monster_truck_rural_ruckus::ui::Presets>()
        .0
        .iter()
        .position(|preset| preset.label == "INTEGRATED GRAPHICS")
        .expect("the preset is offered");
    player_does(&mut app, Action::ApplyPreset(preset));

    let mut camera = CameraSettings::default();
    let mut environment = EnvironmentSettings::default();
    let mut track = TrackSettings {
        mipmaps: false,
        ..TrackSettings::default()
    };
    monster_truck_rural_ruckus::front_end::integrated_graphics(
        &mut camera,
        &mut environment,
        &mut track,
    );
    let world = app.world();
    assert_eq!(*world.resource::<CameraSettings>(), camera);
    assert_eq!(*world.resource::<EnvironmentSettings>(), environment);
    assert_eq!(*world.resource::<TrackSettings>(), track);
}

/// F2 changes the graphics in a race, and the command line can set values that no line of
/// the options screen has. The screen shows the nearest, and leaves them alone until the
/// player changes that line.
#[test]
fn a_setting_changed_elsewhere_is_shown_and_left_alone() {
    let mut app = headless_app(nowhere(), None);
    run_until(&mut app, "the built-in truck", |app| {
        !on_show(app).is_empty()
    });
    player_does(&mut app, Action::Go);
    app.update();
    assert_eq!(state(&app), GameState::Racing);
    app.world_mut()
        .resource_mut::<EnvironmentSettings>()
        .shadow_distance = 120.0;

    cancel_the_race(&mut app);
    assert_eq!(state(&app), GameState::FrontEnd);
    let line = option(&app, "option.shadow_distance");
    assert_eq!(
        app.world().resource::<Settings>().0[line].chosen_value(),
        "100 m"
    );

    // Another line changed, and this one kept as it was.
    set_option(&mut app, "option.antialiasing", "Off");
    assert_eq!(
        app.world().resource::<CameraSettings>().antialiasing,
        monster_truck_rural_ruckus::camera::Antialiasing::Off
    );
    assert_eq!(
        app.world()
            .resource::<EnvironmentSettings>()
            .shadow_distance,
        120.0
    );
}

/// A key binding waits for a key, binds it, and is remembered by the key's name. Esc stops
/// waiting and keeps the key there was. Every control has a line.
#[test]
fn a_key_is_bound_by_pressing_it_and_is_remembered() {
    let store = Arc::new(Store::in_memory());
    let mut first = headless_app(nowhere(), Some(store.clone()));
    first.update();
    player_does(&mut first, Action::Show(Screen::Controls));
    for control in Control::ALL {
        let line = option(&first, &format!("key.{}", snake(control.name())));
        assert!(!first.world().resource::<Settings>().0[line].keys.is_empty());
    }

    let line = option(&first, "key.change_view");
    player_does(&mut first, Action::Listen(line));
    set_option(&mut first, "key.change_view", "N");
    assert_eq!(
        first
            .world()
            .resource::<KeyBindings>()
            .key(Control::ChangeView),
        KeyCode::KeyN
    );

    let mut second = headless_app(nowhere(), Some(store));
    second.update();
    assert_eq!(
        second
            .world()
            .resource::<KeyBindings>()
            .key(Control::ChangeView),
        KeyCode::KeyN
    );
    // From the controls screen, where the keys are.
    player_does(&mut second, Action::Show(Screen::Controls));
    player_does(&mut second, Action::RestoreDefaults);
    assert_eq!(
        *second.world().resource::<KeyBindings>(),
        KeyBindings::default()
    );
}

/// A made-up CD in a folder of its own, laid out as MTM2's is: `cd/Shared`, `cd/English`
/// and `cd/French`, each with an archive with nothing in it, and `cd/Other`, with nothing
/// at all. Gone again when it is dropped.
struct MadeUpCd(std::path::PathBuf);

impl MadeUpCd {
    fn new(test: &str) -> Self {
        let root = std::env::temp_dir().join(format!("ruckus-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        // A header that says there are no files.
        let empty = vec![0; monster_truck_rural_ruckus::pod::ARCHIVE_HEADER_LENGTH];
        for folder in ["Shared", "English", "French"] {
            std::fs::create_dir_all(root.join("cd").join(folder)).unwrap();
            std::fs::write(root.join("cd").join(folder).join("EMPTY.POD"), &empty).unwrap();
        }
        std::fs::create_dir_all(root.join("cd/Other")).unwrap();
        Self(root)
    }

    fn folder(&self, name: &str) -> std::path::PathBuf {
        self.0.join("cd").join(name)
    }
}

impl Drop for MadeUpCd {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Where the folder browser is, and whether it is there.
fn browsing(app: &App) -> Option<String> {
    let browser = app.world().resource::<FolderBrowser>();
    browser.0.as_ref().map(|folder| folder.path.clone())
}

/// What the folder browser asks.
fn asking(app: &App) -> String {
    let browser = app.world().resource::<FolderBrowser>();
    browser
        .0
        .as_ref()
        .expect("a folder browsed")
        .heading
        .clone()
}

/// Goes into the subfolder with this name.
fn go_into(app: &mut App, name: &str) {
    let browser = app.world().resource::<FolderBrowser>();
    let folders = &browser.0.as_ref().expect("a folder browsed").folders;
    let index = folders
        .iter()
        .position(|folder| folder.name == name)
        .unwrap_or_else(|| panic!("no folder {name}"));
    player_does(app, Action::PickFolder(index));
}

fn remembered(store: &Store) -> BaseGameChoice {
    BaseGameChoice::remembered(store)
}

/// With nowhere to find the base game, the front end opens by asking for the Shared folder
/// and then the language's, beside it. Only a folder with archives in it can be chosen, so
/// not the CD itself, which has every language in it. Both are kept for next time.
#[test]
fn the_front_end_asks_where_the_base_game_is_and_remembers_it() {
    let cd = MadeUpCd::new("asks");
    let store = Arc::new(Store::in_memory());
    let settings = FrontEndSettings {
        base_game_folders: vec![cd.0.join("gone")],
        ask_for_base_game: true,
        ..nowhere()
    };
    let mut app = headless_app(settings.clone(), Some(store.clone()));
    app.update();
    // Where it was looked for has gone, so from the folder that it was in.
    assert_eq!(browsing(&app), Some(cd.0.display().to_string()));
    assert_eq!(asking(&app), "WHERE IS THE SHARED FOLDER?");
    // Browsing takes the keys that would change the screen.
    player_does(&mut app, Action::NextScreen);
    assert!(browsing(&app).is_some());

    go_into(&mut app, "cd");
    // The CD, with no archives of its own.
    player_does(&mut app, Action::ChooseFolder);
    assert_eq!(asking(&app), "WHERE IS THE SHARED FOLDER?");
    go_into(&mut app, "Other");
    player_does(&mut app, Action::ChooseFolder);
    assert_eq!(
        browsing(&app),
        Some(cd.folder("Other").display().to_string())
    );

    // Up, which comes back to the folder it was in, highlighted.
    player_does(&mut app, Action::FolderUp);
    let browser = app.world().resource::<FolderBrowser>().0.clone().unwrap();
    assert_eq!(browser.path, cd.0.join("cd").display().to_string());
    assert_eq!(browser.folders[browser.highlighted].name, "Other");
    assert!(!browser.can_choose, "{}", browser.found);

    go_into(&mut app, "Shared");
    player_does(&mut app, Action::ChooseFolder);
    // And now the language, from beside Shared.
    assert_eq!(asking(&app), "WHICH LANGUAGE?");
    assert_eq!(browsing(&app), Some(cd.0.join("cd").display().to_string()));
    go_into(&mut app, "French");
    player_does(&mut app, Action::ChooseFolder);
    assert_eq!(browsing(&app), None);

    let value = |app: &App, id: &str| {
        let line = option(app, id);
        app.world().resource::<Settings>().0[line]
            .chosen_value()
            .to_string()
    };
    assert_eq!(value(&app, "folder.base_shared"), "Shared");
    assert_eq!(value(&app, "folder.base_language"), "French");
    let kept = remembered(&store);
    assert_eq!(
        kept,
        BaseGameChoice {
            shared: Some(cd.folder("Shared")),
            language: Some(cd.folder("French")),
        }
    );
    assert!(kept.is_complete());
    assert_eq!(kept.folders(), [cd.folder("Shared"), cd.folder("French")]);
    // The screens are back.
    player_does(&mut app, Action::Show(Screen::Race));
    run_until(&mut app, "the built-in truck", |app| {
        !on_show(app).is_empty()
    });

    // Skipped, nothing is kept, and the language is not asked for.
    let store = Arc::new(Store::in_memory());
    let mut skipped = headless_app(settings.clone(), Some(store.clone()));
    skipped.update();
    player_does(&mut skipped, Action::LeaveFolder);
    assert_eq!(browsing(&skipped), None);
    assert_eq!(remembered(&store), BaseGameChoice::default());

    // With Shared kept, only the language is asked for.
    let store = Arc::new(Store::in_memory());
    store
        .write(|batch| {
            batch.set_text("folder.base_shared", cd.folder("Shared").to_str().unwrap());
        })
        .unwrap();
    let mut half = headless_app(settings, Some(store.clone()));
    half.update();
    assert_eq!(asking(&half), "WHICH LANGUAGE?");
    go_into(&mut half, "English");
    player_does(&mut half, Action::ChooseFolder);
    assert_eq!(browsing(&half), None);
    assert_eq!(remembered(&store).language, Some(cd.folder("English")));
}

/// The options screen's FILES lines open the same browser, which can be left with nothing
/// changed. Nothing asks where the base game is unless `main.rs` says to.
#[test]
fn the_base_game_can_be_changed_from_the_options_screen() {
    let cd = MadeUpCd::new("options");
    let settings = FrontEndSettings {
        base_game_folders: vec![cd.folder("Other")],
        ..nowhere()
    };
    let mut app = headless_app(settings, None);
    app.update();
    assert_eq!(browsing(&app), None);

    player_does(&mut app, Action::Show(Screen::Options));
    let line = option(&app, "folder.base_language");
    let value = |app: &App| {
        app.world().resource::<Settings>().0[line]
            .chosen_value()
            .to_string()
    };
    assert_eq!(value(&app), "Not chosen");
    player_does(&mut app, Action::OpenSetting(line));
    assert_eq!(asking(&app), "WHICH LANGUAGE?");
    assert_eq!(
        browsing(&app),
        Some(cd.folder("Other").display().to_string())
    );
    player_does(&mut app, Action::LeaveFolder);
    assert_eq!(browsing(&app), None);
    assert_eq!(value(&app), "Not chosen");

    // Enter on the line opens it too, and a folder chosen shows on the line. The other
    // folder is not asked for from here.
    player_does(&mut app, Action::Go);
    player_does(&mut app, Action::FolderUp);
    go_into(&mut app, "English");
    player_does(&mut app, Action::ChooseFolder);
    assert_eq!(browsing(&app), None);
    // Its name on the line, and the whole path in the detail.
    assert_eq!(value(&app), "English");
    let detail = app.world().resource::<Settings>().0[line].detail.clone();
    assert!(
        detail.starts_with(&cd.folder("English").display().to_string()),
        "{detail}"
    );
    let shared = option(&app, "folder.base_shared");
    assert_eq!(
        app.world().resource::<Settings>().0[shared].chosen_value(),
        "Not chosen"
    );
    assert_eq!(state(&app), GameState::FrontEnd);
}

/// The store's name for a control's line, from what the player sees it called.
fn snake(name: &str) -> String {
    match name {
        "Brake and reverse" => "reverse".into(),
        "Race again (after the finish)" => "race_again".into(),
        "Physics debug view" => "physics_debug".into(),
        name => name.to_lowercase().replace(' ', "_"),
    }
}

/// Blue is the application's standard colour (see AGENTS.md): the countdown's is the front
/// end's accent.
#[test]
fn the_race_speaks_in_the_front_ends_blue() {
    assert_eq!(
        monster_truck_rural_ruckus::race::BLUE,
        monster_truck_rural_ruckus::ui::Theme::default().accent
    );
}
