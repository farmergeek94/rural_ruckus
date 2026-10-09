use bevy::prelude::*;
use monster_truck_rural_ruckus::{
    backdrop, base_game, camera, controls_help, diagnostics, display, environment, footing,
    frame_pacing, front_end, game_state, graphics_debug, lighting, opponents, particles, physics,
    physics_debug, race, scenery, sky, store, track, truck, water, weather,
};

/// Where what the player chose is remembered, beside the working directory as `tracks/` and
/// `trucks/` are.
const SAVES: &str = "saves/ruckus.redb";

fn main() {
    let mut app = App::new();

    // `cargo run` opens the front end, where a truck and a track are chosen.
    // `cargo run -- tracks/MyTrack.pod` goes straight to a race on a Monster Truck Madness 2
    // track, and `cargo run -- tracks/MyTrack.pod trucks/MyTruck.pod` in one of its trucks,
    // so that everything measured on a race can be measured as it always was. The built-in
    // truck and track are for development, and only `--builtin` offers them: in the front
    // end's lists, as the truck when only a track is named, and with `--race`, which goes
    // straight to a race in the built-in truck on the built-in track. `--smooth-terrain`
    // lights the ground as if it were rounded off, `--no-blend-ground` leaves the ground's
    // textures meeting edge to edge, as MTM2 draws them,
    // `--no-mipmaps` draws textures without their smaller copies,
    // `--log-fps` prints frame times and how much is drawn, once a second, and `--autopilot`
    // drives the course by itself. See `diagnostics.rs`. `--unlit` draws without lighting,
    // and `--simple-lighting` with the cheap lighting (`lighting.rs`). `--antialiasing=fxaa` (off, fxaa
    // or msaa; `--no-antialiasing` is off),
    // `--shadow-cascades=2` (0 for no shadows), `--shadow-distance=100`, `--anisotropy=4` and
    // `--scenery-distance=200` (in metres; all scenery is drawn without it) start with the
    // graphics settings that F2 changes while racing, and
    // `--integrated-graphics` with the set of them for a graphics processor built into the
    // CPU (`front_end::integrated_graphics`, the options screen's Quality at Balanced); an
    // option after it on the command line wins over it. `--opponents=3` races
    // against that many trucks driven by the computer, which are copies of the player's.
    // `--difficulty=hard` (easy, normal or hard) is how hard they are to beat. `--manual`
    // has the player change the forward gears.
    // `--weather=rain` races in that weather (clear, overcast, fog, rain, storm or snow), and
    // `--weather=random` in one picked at random, at a time of day picked at random, and
    // `--time=night` at that time of day (day, dusk or night), unless the weather is random. `--no-backdrop` leaves out the distant
    // hills a track draws round its horizon, and `--no-decorations` the scenery that trucks
    // drive through. `--no-settle-scenery` stands each object as the track puts it, though
    // its foot floats over a slope. `--flat-water` draws the water as a plain flat plane,
    // and `--no-water-reflections` mirrors only the sky in it. `--base-game=FOLDER` looks for the base game's
    // archives, which tracks and trucks borrow from, in FOLDER and its subfolders, in place
    // of the Shared and language folders chosen in the front end (or `base/`, if none was);
    // it may be given more than once.
    // In the front end, what was set on its options screen is set again over these.
    let mut settings = track::TrackSettings::default();
    let mut camera_settings = camera::CameraSettings::default();
    let mut environment_settings = environment::EnvironmentSettings::default();
    let mut weather_settings = weather::WeatherSettings::default();
    let mut opponents_settings = opponents::OpponentsSettings::default();
    let mut backdrop_settings = backdrop::BackdropSettings::default();
    let mut water_settings = water::WaterSettings::default();
    let mut log_fps = false;
    let mut autopilot = false;
    // Draws frames as fast as they come instead of waiting for the screen, so that
    // `--log-fps` shows how much time there is to spare.
    let mut vsync = true;
    // Waits for the screen strictly. Plain vsync (`AutoVsync`) prefers `FifoRelaxed` where
    // there is one, which shows a frame that just missed a refresh at once, and can tear
    // the top of the picture when frames start late. `Fifo` holds it for the next refresh
    // and never tears, at the price of that frame staying up for two. `--no-vsync` wins.
    let mut fifo = false;
    // Whether anything on the command line says what to race.
    let mut race_at_once = false;
    // Whether the built-in truck and track may be used at all.
    let mut builtin = false;
    let mut opponents = 0;

    // Opened first, because it says where the base game is, unless the command line does.
    let saves = store::Store::open(SAVES.as_ref());
    let remembered = saves
        .as_ref()
        .map(front_end::BaseGameChoice::remembered)
        .unwrap_or_default();

    // Read first, because the archives named after it borrow from it, in whatever order.
    let mut base_folders: Vec<std::path::PathBuf> = std::env::args()
        .skip(1)
        .filter_map(|argument| argument.strip_prefix("--base-game=").map(Into::into))
        .collect();
    let base_named = !base_folders.is_empty();
    if !base_named {
        base_folders = remembered.folders();
    }
    if base_folders.is_empty() {
        base_folders.push(base_game::DEFAULT_FOLDER.into());
    }
    let base = base_game::BaseGame::open(&base_folders);
    // The front end asks for the Shared and the language folder until the player has chosen
    // both. One that has gone, such as a CD that is not in, is asked for again.
    let ask_for_base_game = !base_named && !remembered.is_complete();
    if base.archive_count() == 0 {
        eprintln!(
            "No base game archives found in {}: parts that tracks and trucks borrow from the base game will be missing.",
            base_folders
                .iter()
                .map(|folder| folder.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    for argument in std::env::args_os().skip(1) {
        match argument.to_str() {
            Some("--smooth-terrain") => settings.smooth_terrain = true,
            Some("--no-blend-ground") => settings.blend_ground = false,
            Some("--no-mipmaps") => settings.mipmaps = false,
            Some("--no-settle-scenery") => settings.settle_scenery = false,
            Some("--log-fps") => log_fps = true,
            Some("--autopilot") => autopilot = true,
            Some("--unlit") => environment_settings.lighting = environment::Lighting::Off,
            Some("--simple-lighting") => {
                environment_settings.lighting = environment::Lighting::Simple;
            }
            Some("--no-vsync") => vsync = false,
            Some("--fifo") => fifo = true,
            Some("--race") => race_at_once = true,
            Some("--manual") => {
                app.insert_resource(truck::Transmission::Manual);
            }
            Some("--builtin") => builtin = true,
            Some("--no-antialiasing") => camera_settings.antialiasing = camera::Antialiasing::Off,
            Some(flag) if flag.starts_with("--antialiasing=") => {
                let name = flag.split_once('=').map_or("", |(_, name)| name);
                camera_settings.antialiasing =
                    camera::Antialiasing::named(name).unwrap_or_else(|| {
                        eprintln!("{flag}: the antialiasing is off, fxaa or msaa");
                        std::process::exit(2);
                    });
            }
            Some("--integrated-graphics") => front_end::integrated_graphics(
                &mut camera_settings,
                &mut environment_settings,
                &mut settings,
            ),
            Some("--no-backdrop") => backdrop_settings.on = false,
            Some("--no-decorations") => settings.decorations = false,
            Some("--flat-water") => water_settings.flat = true,
            Some("--no-water-reflections") => water_settings.reflections = false,
            Some(flag) if flag.starts_with("--base-game=") => {}
            Some(flag) if flag.starts_with("--shadow-cascades=") => {
                environment_settings.shadow_cascades = number(flag);
            }
            Some(flag) if flag.starts_with("--shadow-distance=") => {
                environment_settings.shadow_distance = number(flag);
            }
            Some(flag) if flag.starts_with("--anisotropy=") => {
                settings.anisotropy = number(flag);
            }
            Some(flag) if flag.starts_with("--scenery-distance=") => {
                settings.scenery_distance = number(flag);
            }
            Some(flag) if flag.starts_with("--weather=") => {
                let name = flag.split_once('=').map_or("", |(_, name)| name);
                if name.eq_ignore_ascii_case("random") {
                    weather_settings.random = true;
                } else {
                    weather_settings.weather = weather::Weather::named(name).unwrap_or_else(|| {
                        let names: Vec<&str> = weather::Weather::ALL.map(|w| w.name()).to_vec();
                        eprintln!(
                            "{flag}: the weather is Random or one of {}",
                            names.join(", ")
                        );
                        std::process::exit(2);
                    });
                }
            }
            Some(flag) if flag.starts_with("--time=") => {
                let name = flag.split_once('=').map_or("", |(_, name)| name);
                weather_settings.time_of_day =
                    weather::TimeOfDay::named(name).unwrap_or_else(|| {
                        eprintln!("{flag}: the time of day is Day, Dusk or Night");
                        std::process::exit(2);
                    });
            }
            Some(flag) if flag.starts_with("--opponents=") => {
                race_at_once = true;
                opponents = number(flag);
            }
            Some(flag) if flag.starts_with("--difficulty=") => {
                let name = flag.split_once('=').map_or("", |(_, name)| name);
                opponents_settings.difficulty = match name.to_ascii_lowercase().as_str() {
                    "easy" => opponents::Difficulty::Easy,
                    "normal" => opponents::Difficulty::Normal,
                    "hard" => opponents::Difficulty::Hard,
                    _ => {
                        eprintln!("{flag}: the difficulty is Easy, Normal or Hard");
                        std::process::exit(2);
                    }
                };
            }
            Some(flag) if flag.starts_with("--") => {
                eprintln!(
                    "Unknown option {flag}. Options: --race --builtin --smooth-terrain --no-blend-ground --no-mipmaps --log-fps --no-vsync --fifo --autopilot --unlit --simple-lighting \
                     --no-antialiasing --antialiasing=OFF|FXAA|MSAA --integrated-graphics --no-backdrop --no-decorations --flat-water --no-water-reflections --shadow-cascades=N --shadow-distance=METRES --anisotropy=N --scenery-distance=METRES --opponents=N --difficulty=EASY|NORMAL|HARD --manual --weather=NAME --time=DAY|DUSK|NIGHT --base-game=FOLDER"
                );
                std::process::exit(2);
            }
            // An archive is a truck if it holds a truck file, and a track otherwise.
            _ if truck::pod_holds_truck(argument.as_ref()) => {
                race_at_once = true;
                match truck::load_pod(argument.as_ref(), &base) {
                    Ok(data) => {
                        app.insert_resource(truck::ChosenTruck(data));
                    }
                    Err(error) => {
                        eprintln!("Cannot load truck {error}");
                        std::process::exit(1);
                    }
                }
            }
            _ => match track::load_pod(argument.as_ref(), &base) {
                Ok(data) => {
                    race_at_once = true;
                    app.insert_resource(track::Track(data));
                }
                Err(error) => {
                    eprintln!("Cannot load track {error}");
                    std::process::exit(1);
                }
            },
        }
    }
    // The slices fall back to the built-in truck and track when nothing is chosen, which
    // is only for development: without `--builtin`, a race needs an archive for each.
    if race_at_once && !builtin {
        let track_named = app.world().contains_resource::<track::Track>();
        let truck_named = app.world().contains_resource::<truck::ChosenTruck>();
        for (named, what, folder) in [
            (track_named, "track", "tracks"),
            (truck_named, "truck", "trucks"),
        ] {
            if !named {
                eprintln!(
                    "No {what} named. Name a {folder}/*.pod archive, or pass --builtin to race the built-in {what}."
                );
                std::process::exit(2);
            }
        }
    }
    if opponents > 0 {
        let player = app
            .world()
            .get_resource::<truck::ChosenTruck>()
            .map_or_else(truck::TruckData::builtin, |chosen| chosen.0.clone());
        let trucks = (1..=opponents).map(|number| player.clone().in_color(number));
        app.insert_resource(truck::ComputerTrucks(trucks.collect()));
    }
    let display_settings = display::DisplaySettings {
        vsync: match (vsync, fifo) {
            (false, _) => display::Vsync::Off,
            (true, false) => display::Vsync::On,
            (true, true) => display::Vsync::Strict,
        },
        ..default()
    };
    app.insert_resource(settings)
        .insert_resource(camera_settings)
        .insert_resource(environment_settings)
        .insert_resource(weather_settings)
        .insert_resource(opponents_settings)
        .insert_resource(backdrop_settings)
        .insert_resource(water_settings)
        .insert_resource(display_settings)
        .insert_resource(base);
    let front_end_settings = front_end::FrontEndSettings {
        builtin,
        base_game_folders: base_folders,
        ask_for_base_game,
        ..default()
    };

    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Monster Truck Rural Ruckus".into(),
                present_mode: display_settings.present_mode(),
                mode: display_settings.window_mode(),
                ..default()
            }),
            ..default()
        }),
        // Physics runs on Bevy's fixed timestep so that handling doesn't change
        // with the frame rate.
        physics::GamePhysicsPlugin,
    ))
    // One plugin per slice.
    .add_plugins((
        display::DisplayPlugin,
        (environment::EnvironmentPlugin, lighting::LightingPlugin),
        track::TrackPlugin,
        scenery::SceneryPlugin,
        (backdrop::BackdropPlugin, sky::SkyPlugin),
        water::WaterPlugin,
        particles::ParticlesPlugin,
        weather::WeatherPlugin,
        // Nested because a tuple of plugins tops out at 15.
        (truck::TruckPlugin, footing::FootingPlugin),
        race::RacePlugin,
        opponents::OpponentsPlugin,
        camera::ChaseCameraPlugin,
        controls_help::ControlsHelpPlugin,
        physics_debug::PhysicsDebugPlugin,
        graphics_debug::GraphicsDebugPlugin,
    ));
    if !race_at_once {
        // In place of the `Racing` the slices began with. Nothing has run yet.
        // Remembering choices must never stop the game: without a store it forgets them.
        let store = saves
            .inspect_err(|error| eprintln!("Choices will not be remembered: {error}"))
            .ok();
        app.insert_state(game_state::GameState::FrontEnd)
            .insert_resource(front_end_settings)
            .add_plugins(front_end::FrontEndPlugin {
                store: store.map(std::sync::Arc::new),
            });
    } else {
        // Let go of the file, so that another race can be started from the command line
        // while this one runs.
        drop(saves);
    }
    // The screen shows frames evenly only when it is waited for. Without vsync each is
    // shown as soon as it is done, and the clock's own measurements are the true ones.
    // `display` switches it as vsync is switched on the options screen.
    app.insert_resource(frame_pacing::FramePacing { on: vsync })
        .add_plugins(frame_pacing::FramePacingPlugin);
    app.add_plugins(diagnostics::DiagnosticsPlugin { log_fps, autopilot });
    app.run();
}

/// The number after the `=` of an option like `--anisotropy=4`.
fn number<T: std::str::FromStr>(flag: &str) -> T {
    let value = flag.split_once('=').map_or("", |(_, value)| value);
    value.parse().unwrap_or_else(|_| {
        eprintln!("{flag}: \"{value}\" is not a number this option takes");
        std::process::exit(2);
    })
}
