//! The front end's game side: fills the `ui` module's screens from the `truck`,
//! `track` and `race` slices, and acts on what the player does there. The `ui` module knows
//! nothing of the game and those slices know nothing of `ui`; this is the only code
//! that knows both. Nothing uses it.
//!
//! It lists the archives in `trucks/` and `tracks/`, the trucks and tracks in the base
//! game's archives (`base_game::BaseGame`, several to an archive), and the built-in truck
//! and track beside them only when `FrontEndSettings::builtin` asks for them (the `--builtin` flag),
//! puts the highlighted truck on the showroom's turntable, paints the highlighted track's
//! map, turns the garage's dials into a `truck::TruckSetup`, and on GO hands the choices to
//! the slices, the trucks that the computer drives among them (`truck::ComputerTrucks`),
//! and enters the race. Cancelling the race from its pause dialog (`race::RaceCancelled`)
//! comes back, with the choices kept. EXIT closes the game. What
//! was chosen is kept in a `store::Store` on GO and chosen again the next time.
//!
//! Where the base game's archives are is two folders the player chooses, `Shared` and one
//! language's, kept in the store. When the command line names no folder and the store
//! does not name both, with archives in them, the front end opens by asking for what is
//! missing (`FrontEndSettings::ask_for_base_game`), in a folder browser that the `ui`
//! module draws and this fills in. The two FILES lines of the options screen open the same
//! browser. `BaseGame` is opened again from the folders chosen, and the lists are made
//! again from it.
//!
//! The options screen, and the ADVANCED screen beside it, show settings that belong to
//! other slices: the window's (`display`), the graphics (`camera`, `environment`, `track`,
//! `water`, `dirt`, `backdrop`), the `weather`, and a few more. The options screen's
//! Quality line stands for the graphics that cost most, which are on the ADVANCED screen
//! with the physics, and sets them all at once. Each line of `OPTIONS` says which value of which slice's settings resource it stands for. A change
//! goes to that resource at once, and to the store, and what the store holds is set when
//! the game starts. A setting changed elsewhere (by F2 in a race, or on the command line)
//! is shown as it is, and is only written over when the player changes that line.
//!
//! Listing an archive reads its name alone (`peek_pod`, a millisecond or two). Loading a
//! whole truck takes 40 to 95 ms in a debug build and a track 150 ms, which is many frames,
//! so what is highlighted is loaded on a background task, the last truck staying on show
//! until the new one is ready, and the last few are kept so that walking back up a list is
//! instant.
//!
//! Needs `TrackPlugin`, `TruckPlugin` and `RacePlugin` added before it. Adds
//! `ui::UiPlugin` itself if no one has, and the settings resources of the other slices it
//! shows if their plugins have not. Works in a headless app.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};

use crate::base_game::{self, BaseGame};

use crate::backdrop::BackdropSettings;
use crate::camera::{Antialiasing, CameraSettings};
use crate::controls_help::ControlsHelpSettings;
use crate::dirt::DirtSettings;
use crate::display::{DisplaySettings, ScreenMode, Vsync};
use crate::environment::{EnvironmentSettings, Lighting};
use crate::game_state::GameState;
use crate::keys::{BINDABLE, Control, KeyBindings};
use crate::physics::PhysicsSettings;
use crate::race::{RaceCancelled, RaceSettings};
use crate::store::{self, Store};
use crate::track::{self, ChosenTrack, TrackData, TrackSettings};
use crate::truck::{
    self, ChosenTruck, ComputerTrucks, SpeedUnits, TruckData, TruckDisplay, TruckLooksSettings,
    TruckSetup,
};
use crate::ui::{
    self, Catalogue, Choices, Dial, Dials, Entry, ExitPressed, Folder, FolderBrowser, FolderChosen,
    FolderEntered, FolderLeft, FolderUp, FrontEndOpen, GoPressed, Setting, SettingOpened,
    TrackHighlighted, TrackPreview, TruckHighlighted, Turntable, UiPlugin, UiSystems,
};
use crate::water::WaterSettings;
use crate::weather::{TimeOfDay, Weather, WeatherSettings};

/// The catalogue id of the truck and the track that need no files. Every other id is the
/// path of an archive.
const BUILTIN: &str = "builtin";
/// Starts the id of a truck or track in the base game's archives: see `base_entry`.
const BASE_PREFIX: &str = "base:";

// Where each thing is kept in the store.
const KEY_TRUCK: &str = "choice.truck";
const KEY_TRACK: &str = "choice.track";
const KEY_LAPS: &str = "choice.laps";
const KEY_OPPONENTS: &str = "choice.opponents";
const KEY_SUSPENSION: &str = "setup.suspension";
const KEY_GEARING: &str = "setup.gearing";
const KEY_REAR_STEERING: &str = "setup.rear_steering";
const KEY_GRIP: &str = "setup.grip";

/// How many places each of the garage's dials can stand at. Odd, so that one is the middle.
const DIAL_STEPS: usize = 5;
/// How many loaded trucks are kept, so that going back to one is instant. Each is a few
/// megabytes of textures.
const TRUCKS_KEPT: usize = 6;
/// Pixels along each side of a track's map. Drawn at 320, so this is sharp at 4K.
const MAP_SIZE: usize = 512;

pub struct FrontEndPlugin {
    /// Where choices are remembered. `None` remembers nothing, and nothing fails.
    pub store: Option<Arc<Store>>,
}

/// Where the archives are looked for, and whether the built-in truck and track are listed
/// with them. Insert it before adding `FrontEndPlugin`.
#[derive(Resource, Clone, Debug)]
pub struct FrontEndSettings {
    pub trucks_folder: PathBuf,
    pub tracks_folder: PathBuf,
    /// Whether the built-in truck and track are offered, first in each list. They are for
    /// development: off, only the archives in the folders are.
    pub builtin: bool,
    /// Where the `BaseGame` that `main.rs` put in was opened from, for the options screen
    /// to show.
    pub base_game_folders: Vec<PathBuf>,
    /// Whether the front end opens by asking the player where the base game is.
    pub ask_for_base_game: bool,
}

impl Default for FrontEndSettings {
    /// Beside the working directory, as the command line's paths are, and archives alone.
    fn default() -> Self {
        Self {
            trucks_folder: "trucks".into(),
            tracks_folder: "tracks".into(),
            builtin: false,
            base_game_folders: vec![base_game::DEFAULT_FOLDER.into()],
            ask_for_base_game: false,
        }
    }
}

impl Plugin for FrontEndPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<UiPlugin>() {
            app.add_plugins(UiPlugin);
        }
        let settings = app
            .init_resource::<FrontEndSettings>()
            .world()
            .resource::<FrontEndSettings>()
            .clone();

        // `main.rs` puts the base game in before this; a test app may have none.
        let base = app
            .init_resource::<BaseGame>()
            .world()
            .resource::<BaseGame>()
            .clone();
        let catalogue = catalogue_for(&settings, &base);

        // What the slices say, unless something else was remembered.
        let mut choices = Choices {
            laps: app
                .world()
                .get_resource::<RaceSettings>()
                .map_or(Choices::default().laps, |race| race.laps),
            ..default()
        };
        // The race's pause dialog comes back here rather than quitting.
        if let Some(mut race) = app.world_mut().get_resource_mut::<RaceSettings>() {
            race.front_end = true;
        }
        let mut setup = app
            .world()
            .get_resource::<TruckSetup>()
            .copied()
            .unwrap_or_default();
        app.init_resource::<DisplaySettings>()
            .init_resource::<CameraSettings>()
            .init_resource::<EnvironmentSettings>()
            .init_resource::<ControlsHelpSettings>()
            .init_resource::<SpeedUnits>()
            .init_resource::<TruckLooksSettings>()
            .init_resource::<WaterSettings>()
            .init_resource::<DirtSettings>()
            .init_resource::<BackdropSettings>()
            .init_resource::<WeatherSettings>()
            .init_resource::<PhysicsSettings>()
            .init_resource::<KeyBindings>();
        let mut options = Options::in_world(app.world());
        let mut store = self.store.clone();
        if let Some(kept) = &store
            && let Err(error) = remember(kept, &catalogue, &mut choices, &mut setup)
                .and_then(|()| remember_options(kept, &mut options))
        {
            // Carry on with the defaults, and don't write over what couldn't be read.
            warn!("Choices are not remembered this time: {error}");
            store = None;
        }
        let base_folders = BaseGameFolders {
            opened_from: settings.base_game_folders.clone(),
            chosen: store
                .as_deref()
                .map(BaseGameChoice::remembered)
                .unwrap_or_default(),
        };
        let mut option_lines = ui::Settings(settings_for(&options));
        option_lines
            .0
            .extend(BaseFolder::ALL.map(|which| base_folder_line(which, &base_folders.chosen)));
        options.put_in_world(app.world_mut());

        let mut browsing = Browsing::default();
        let mut browser = FolderBrowser::default();
        if settings.ask_for_base_game {
            // Those not chosen yet, or gone, in turn.
            let mut wanted = BaseFolder::ALL
                .into_iter()
                .filter(|&which| !base_folders.chosen.usable(which));
            if let Some(which) = wanted.next() {
                let view;
                (browsing, view) =
                    Browsing::start(which, wanted.next(), "SKIP FOR NOW", &base_folders);
                browser.0 = Some(view);
            }
        }

        app.insert_resource(catalogue)
            .insert_resource(choices)
            .insert_resource(dials_for(&setup))
            .insert_resource(setup)
            .insert_resource(option_lines)
            .insert_resource(Saves(store))
            .insert_resource(browser)
            .insert_resource(browsing)
            .insert_resource(base_folders)
            .init_resource::<Showing>()
            .add_systems(OnEnter(GameState::FrontEnd), (open, show_the_options))
            .add_systems(OnExit(GameState::FrontEnd), close)
            .add_systems(
                Update,
                (
                    (
                        open_the_browser,
                        browse,
                        choose_the_base_game,
                        note_what_is_highlighted,
                        load_what_is_wanted,
                        show_the_truck,
                        show_the_map,
                        set_the_truck_up,
                        apply_the_options,
                        go,
                        exit,
                    )
                        .chain()
                        .after(UiSystems)
                        .run_if(in_state(GameState::FrontEnd)),
                    leave_the_race.run_if(in_state(GameState::Racing)),
                ),
            );
    }
}

#[derive(Resource)]
struct Saves(Option<Arc<Store>>);

/// The two folders of the base game. MTM2's CD has one `Shared` folder, and beside it one
/// folder for each language (`English`, `French`, `German`), whose archives have the same
/// names: only one language is wanted, so each folder is chosen on its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BaseFolder {
    #[default]
    Shared,
    Language,
}

impl BaseFolder {
    /// In the order they are asked for, and searched.
    const ALL: [BaseFolder; 2] = [BaseFolder::Shared, BaseFolder::Language];

    /// Where it is kept in the store. Also the id of its line on the options screen.
    fn key(self) -> &'static str {
        match self {
            BaseFolder::Shared => "folder.base_shared",
            BaseFolder::Language => "folder.base_language",
        }
    }

    fn label(self) -> &'static str {
        match self {
            BaseFolder::Shared => "Shared",
            BaseFolder::Language => "Language",
        }
    }

    fn heading(self) -> &'static str {
        match self {
            BaseFolder::Shared => "WHERE IS THE SHARED FOLDER?",
            BaseFolder::Language => "WHICH LANGUAGE?",
        }
    }

    fn about(self) -> &'static str {
        match self {
            BaseFolder::Shared => {
                "The Shared folder of Monster Truck Madness 2, on its CD or where it is \
                 installed. Tracks and trucks borrow from its archives."
            }
            BaseFolder::Language => {
                "The folder of one language of Monster Truck Madness 2: English, French or \
                 German, beside Shared. The base game's trucks are in it."
            }
        }
    }
}

/// The base game's folders that the player chose, as the store keeps them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BaseGameChoice {
    pub shared: Option<PathBuf>,
    pub language: Option<PathBuf>,
}

impl BaseGameChoice {
    /// What the store says. For `main.rs`, which opens the base game before the front end
    /// is built. A store that can't be read says nothing.
    pub fn remembered(store: &Store) -> Self {
        let folder = |which: BaseFolder| {
            store
                .get_text(which.key())
                .inspect_err(|error| warn!("{error}"))
                .ok()
                .flatten()
                .filter(|folder| !folder.is_empty())
                .map(PathBuf::from)
        };
        Self {
            shared: folder(BaseFolder::Shared),
            language: folder(BaseFolder::Language),
        }
    }

    /// Those chosen, to open the base game from.
    pub fn folders(&self) -> Vec<PathBuf> {
        [&self.shared, &self.language]
            .into_iter()
            .flatten()
            .cloned()
            .collect()
    }

    /// Whether both are chosen and hold archives. If not, the front end asks.
    pub fn is_complete(&self) -> bool {
        BaseFolder::ALL.into_iter().all(|which| self.usable(which))
    }

    fn get(&self, which: BaseFolder) -> Option<&PathBuf> {
        match which {
            BaseFolder::Shared => self.shared.as_ref(),
            BaseFolder::Language => self.language.as_ref(),
        }
    }

    fn set(&mut self, which: BaseFolder, folder: PathBuf) {
        match which {
            BaseFolder::Shared => self.shared = Some(folder),
            BaseFolder::Language => self.language = Some(folder),
        }
    }

    /// Chosen, and with archives in it. A CD that is not in has none.
    fn usable(&self, which: BaseFolder) -> bool {
        self.get(which)
            .is_some_and(|folder| !base_game::archives_in(folder).is_empty())
    }
}

/// Where the `BaseGame` in the world was opened from, and what the player chose.
#[derive(Resource)]
struct BaseGameFolders {
    opened_from: Vec<PathBuf>,
    chosen: BaseGameChoice,
}

/// The folder being browsed for the base game, if one is.
#[derive(Resource, Default)]
struct Browsing {
    /// Always absolute, so that there is a folder to go up to.
    at: Option<PathBuf>,
    /// Which of the base game's folders is looked for.
    which: BaseFolder,
    /// The one to ask for once this one is chosen, if it is still wanted then.
    then: Option<BaseFolder>,
    /// What the button that stops browsing says.
    leave: &'static str,
}

impl Browsing {
    /// Browsing for `which`, and what to show of where it starts.
    fn start(
        which: BaseFolder,
        then: Option<BaseFolder>,
        leave: &'static str,
        folders: &BaseGameFolders,
    ) -> (Self, Folder) {
        let at = start_folder(which, folders);
        let view = folder_view(&at, None, which, leave);
        let browsing = Self {
            at: Some(at),
            which,
            then,
            leave,
        };
        (browsing, view)
    }
}

/// The trucks and tracks to list: in the folders, and in the base game.
fn catalogue_for(settings: &FrontEndSettings, base: &BaseGame) -> Catalogue {
    let base_trucks = truck::peek_base(base)
        .into_iter()
        .map(|truck| base_entry(&truck.archive, &truck.file, truck.name))
        .collect();
    let base_tracks = track::peek_base(base)
        .into_iter()
        .map(|track| base_entry(&track.archive, &track.file, track.name))
        .collect();
    Catalogue {
        trucks: list_archives(
            &settings.trucks_folder,
            settings.builtin.then(|| TruckData::builtin().name),
            truck::peek_pod,
            "no truck in this archive",
            base_trucks,
        ),
        tracks: list_archives(
            &settings.tracks_folder,
            settings.builtin.then(|| track::builtin_track().name),
            track::peek_pod,
            "no track in this archive",
            base_tracks,
        ),
    }
}

/// The options screen's line that shows where one of the base game's folders is, and
/// opens the browser. After those of `OPTIONS`, which the systems that apply the options
/// walk in step with `ui::Settings`.
fn base_folder_line(which: BaseFolder, chosen: &BaseGameChoice) -> Setting {
    // The folder's name alone fits on the line, and says which language. The whole path is
    // in the detail, which is shown while the line is highlighted.
    let name = |folder: &PathBuf| {
        folder.file_name().map_or_else(
            || folder.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    };
    let (value, detail) = match chosen.get(which) {
        None => (
            "Not chosen".to_string(),
            format!("{} Enter or a click chooses it.", which.about()),
        ),
        Some(folder) if chosen.usable(which) => (
            name(folder),
            format!("{}. Enter or a click chooses another.", folder.display()),
        ),
        Some(folder) => (
            format!("{} (not found)", name(folder)),
            format!(
                "{} has no archives in it now. Enter or a click chooses another.",
                folder.display()
            ),
        ),
    };
    Setting {
        id: which.key().into(),
        section: FILES.into(),
        label: which.label().into(),
        values: vec![value],
        chosen: 0,
        default: 0,
        detail,
        keys: Vec::new(),
        opens: true,
        advanced: false,
        levels: Vec::new(),
    }
}

/// Where browsing for one of the base game's folders starts: where it is; else beside the
/// other one, as the two are on the CD; else where the base game was looked for. Or the
/// nearest folder above that which is there, or the working directory.
fn start_folder(which: BaseFolder, folders: &BaseGameFolders) -> PathBuf {
    let absolute = |path: &Path| std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let other = match which {
        BaseFolder::Shared => BaseFolder::Language,
        BaseFolder::Language => BaseFolder::Shared,
    };
    folders
        .chosen
        .get(which)
        .map(|folder| absolute(folder))
        .or_else(|| {
            let other = absolute(folders.chosen.get(other)?);
            other.parent().map(Path::to_path_buf)
        })
        .or_else(|| folders.opened_from.first().map(|folder| absolute(folder)))
        .and_then(|folder| {
            folder
                .ancestors()
                .find(|at| at.is_dir())
                .map(Path::to_path_buf)
        })
        .unwrap_or_else(|| absolute(Path::new(".")))
}

/// What the browser shows of the folder `at`, with the subfolder `highlight` highlighted
/// if it is there. A folder can be chosen when there are archives in it, and not only in
/// the folders in it: on the CD the folder above `Shared` holds every language's folder.
fn folder_view(at: &Path, highlight: Option<&OsString>, which: BaseFolder, leave: &str) -> Folder {
    let mut names: Vec<OsString> = Vec::new();
    let listed = std::fs::read_dir(at).map(|entries| {
        names = entries
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name())
            // Hidden folders are clutter here.
            .filter(|name| !name.to_string_lossy().starts_with('.'))
            .collect();
    });
    names.sort_by_key(|name| name.to_string_lossy().to_lowercase());
    let archives = base_game::archives_in(at).len();
    let found = match (listed, archives) {
        (Err(error), _) => format!("Cannot be read: {error}"),
        (Ok(()), 0) => "No archives (.pod) in here".into(),
        (Ok(()), 1) => "1 archive (.pod) in here".into(),
        (Ok(()), count) => format!("{count} archives (.pod) in here"),
    };
    Folder {
        heading: which.heading().into(),
        path: at.display().to_string(),
        found,
        about: which.about().into(),
        highlighted: highlight
            .and_then(|highlight| names.iter().position(|name| name == highlight))
            .unwrap_or(0),
        folders: names
            .iter()
            .map(|name| Entry {
                id: name.to_string_lossy().into_owned(),
                name: name.to_string_lossy().into_owned(),
                detail: at.join(name).display().to_string(),
                available: true,
            })
            .collect(),
        can_choose: archives > 0,
        can_go_up: at.parent().is_some(),
        leave: leave.into(),
    }
}

type Loading<T> = Option<(String, Task<Result<T, String>>)>;

/// What the showroom and the map are showing, what they ought to be, and what is on its
/// way. Everything is named by its catalogue id.
#[derive(Resource, Default)]
struct Showing {
    wanted_truck: Option<String>,
    wanted_track: Option<String>,
    /// The truck on the turntable, and the entity it is.
    truck_on_show: Option<(String, Entity)>,
    map_of: Option<String>,
    /// Loaded trucks, the latest last, and the one track.
    trucks: Vec<(String, TruckData)>,
    track: Option<(String, TrackData)>,
    loading_truck: Loading<TruckData>,
    loading_track: Loading<TrackData>,
}

impl Showing {
    fn truck(&self, id: &str) -> Option<&TruckData> {
        let (_, truck) = self.trucks.iter().find(|(kept, _)| kept == id)?;
        Some(truck)
    }

    fn keep_truck(&mut self, id: String, truck: TruckData) {
        self.trucks.retain(|(kept, _)| *kept != id);
        self.trucks.push((id, truck));
        if self.trucks.len() > TRUCKS_KEPT {
            self.trucks.remove(0);
        }
    }

    fn track(&self, id: &str) -> Option<&TrackData> {
        self.track
            .as_ref()
            .filter(|(kept, _)| kept == id)
            .map(|(_, track)| track)
    }
}

/// The built-in one, if it is offered, then every archive in the folder and every one of
/// the base game's (`from_base`), by name. An archive that can't be read is listed all the
/// same, greyed out, with the reason: community archives are often malformed, and the
/// player should see why one is missing.
fn list_archives(
    folder: &Path,
    builtin_name: Option<String>,
    peek: fn(&Path) -> Result<Option<String>, String>,
    holds_none: &str,
    from_base: Vec<Entry>,
) -> Vec<Entry> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|file| file.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pod"))
        })
        .collect();
    paths.sort();

    let mut entries: Vec<Entry> = builtin_name
        .map(|name| Entry {
            id: BUILTIN.into(),
            name,
            detail: "needs no files".into(),
            available: true,
        })
        .into_iter()
        .collect();
    let mut archives: Vec<Entry> = paths
        .iter()
        .map(|path| {
            let file = path.display().to_string();
            let stem = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            let (name, detail, available) = match peek(path) {
                // Not every track gives itself a name.
                Ok(Some(name)) if name.trim().is_empty() => (stem, file.clone(), true),
                Ok(Some(name)) => (name.trim().to_string(), file.clone(), true),
                Ok(None) => (stem, format!("{file}: {holds_none}"), false),
                // The error names the file already.
                Err(error) => (stem, error, false),
            };
            Entry {
                id: file,
                name,
                detail,
                available,
            }
        })
        .collect();
    archives.extend(from_base);
    archives.sort_by_key(|entry| entry.name.to_lowercase());
    entries.extend(archives);
    entries
}

fn dials_for(setup: &TruckSetup) -> Dials {
    let dial = |id: &str, label: &str, low: &str, high: &str, value: f32| {
        let mut dial = Dial {
            id: id.into(),
            label: label.into(),
            low_label: low.into(),
            high_label: high.into(),
            steps: DIAL_STEPS,
            step: 0,
        };
        dial.set_value(value);
        dial
    };
    Dials(vec![
        dial(
            KEY_SUSPENSION,
            "Suspension",
            "Soft",
            "Stiff",
            setup.suspension,
        ),
        dial(
            KEY_GEARING,
            "Gearing",
            "Acceleration",
            "Top speed",
            setup.gearing,
        ),
        dial(
            KEY_REAR_STEERING,
            "Rear steering",
            "Off",
            "Sharp",
            setup.rear_steering,
        ),
        dial(KEY_GRIP, "Grip", "Planted", "Tippy", setup.grip),
    ])
}

/// What was chosen last time, where it can still be had. A truck or a track that has gone
/// from its folder falls back to the built-in one, and a number out of range is clamped.
fn remember(
    store: &Store,
    catalogue: &Catalogue,
    choices: &mut Choices,
    setup: &mut TruckSetup,
) -> Result<(), store::StoreError> {
    let find = |entries: &[Entry], id: Option<String>| {
        id.and_then(|id| entries.iter().position(|entry| entry.id == id))
            .unwrap_or(0)
    };
    choices.truck = find(&catalogue.trucks, store.get_text(KEY_TRUCK)?);
    choices.track = find(&catalogue.tracks, store.get_text(KEY_TRACK)?);
    if let Some(laps) = store.get_number(KEY_LAPS)? {
        choices.laps = laps.clamp(1.0, ui::MOST_LAPS as f64) as u32;
    }
    if let Some(opponents) = store.get_number(KEY_OPPONENTS)? {
        choices.opponents = opponents.clamp(0.0, ui::MOST_OPPONENTS as f64) as u32;
    }
    for (key, dial) in [
        (KEY_SUSPENSION, &mut setup.suspension),
        (KEY_GEARING, &mut setup.gearing),
        (KEY_REAR_STEERING, &mut setup.rear_steering),
        (KEY_GRIP, &mut setup.grip),
    ] {
        if let Some(value) = store.get_number(key)? {
            *dial = (value as f32).clamp(-1.0, 1.0);
        }
    }
    Ok(())
}

/// Every setting the options screen shows, as the slices that own them hold them.
#[derive(Clone, Debug, PartialEq)]
struct Options {
    display: DisplaySettings,
    track: TrackSettings,
    camera: CameraSettings,
    environment: EnvironmentSettings,
    help: ControlsHelpSettings,
    speed: SpeedUnits,
    truck_looks: TruckLooksSettings,
    water: WaterSettings,
    dirt: DirtSettings,
    backdrop: BackdropSettings,
    weather: WeatherSettings,
    physics: PhysicsSettings,
    keys: KeyBindings,
}

impl Options {
    /// Every slice's own defaults.
    fn defaults() -> Self {
        Self {
            display: default(),
            track: default(),
            camera: default(),
            environment: default(),
            help: default(),
            speed: default(),
            truck_looks: default(),
            water: default(),
            dirt: default(),
            backdrop: default(),
            weather: default(),
            physics: default(),
            keys: default(),
        }
    }

    fn in_world(world: &World) -> Self {
        Self {
            display: *world.resource(),
            track: world.resource::<TrackSettings>().clone(),
            camera: world.resource::<CameraSettings>().clone(),
            environment: world.resource::<EnvironmentSettings>().clone(),
            help: *world.resource(),
            speed: *world.resource(),
            truck_looks: *world.resource(),
            water: *world.resource(),
            dirt: *world.resource(),
            backdrop: *world.resource(),
            weather: *world.resource(),
            physics: *world.resource(),
            keys: *world.resource(),
        }
    }

    fn put_in_world(self, world: &mut World) {
        world.insert_resource(self.display);
        world.insert_resource(self.track);
        world.insert_resource(self.camera);
        world.insert_resource(self.environment);
        world.insert_resource(self.help);
        world.insert_resource(self.speed);
        world.insert_resource(self.truck_looks);
        world.insert_resource(self.water);
        world.insert_resource(self.dirt);
        world.insert_resource(self.backdrop);
        world.insert_resource(self.weather);
        world.insert_resource(self.physics);
        world.insert_resource(self.keys);
    }
}

/// The same resources, for a system.
#[derive(SystemParam)]
struct OptionResources<'w> {
    display: ResMut<'w, DisplaySettings>,
    track: ResMut<'w, TrackSettings>,
    camera: ResMut<'w, CameraSettings>,
    environment: ResMut<'w, EnvironmentSettings>,
    help: ResMut<'w, ControlsHelpSettings>,
    speed: ResMut<'w, SpeedUnits>,
    truck_looks: ResMut<'w, TruckLooksSettings>,
    water: ResMut<'w, WaterSettings>,
    dirt: ResMut<'w, DirtSettings>,
    backdrop: ResMut<'w, BackdropSettings>,
    weather: ResMut<'w, WeatherSettings>,
    physics: ResMut<'w, PhysicsSettings>,
    keys: ResMut<'w, KeyBindings>,
}

impl OptionResources<'_> {
    fn get(&self) -> Options {
        Options {
            display: *self.display,
            track: self.track.clone(),
            camera: self.camera.clone(),
            environment: self.environment.clone(),
            help: *self.help,
            speed: *self.speed,
            truck_looks: *self.truck_looks,
            water: *self.water,
            dirt: *self.dirt,
            backdrop: *self.backdrop,
            weather: *self.weather,
            physics: *self.physics,
            keys: *self.keys,
        }
    }

    /// Only what is different, so that each slice carries only a real change.
    fn set(&mut self, options: Options) {
        self.display.set_if_neq(options.display);
        self.track.set_if_neq(options.track);
        self.camera.set_if_neq(options.camera);
        self.environment.set_if_neq(options.environment);
        self.help.set_if_neq(options.help);
        self.speed.set_if_neq(options.speed);
        self.truck_looks.set_if_neq(options.truck_looks);
        self.water.set_if_neq(options.water);
        self.dirt.set_if_neq(options.dirt);
        self.backdrop.set_if_neq(options.backdrop);
        self.weather.set_if_neq(options.weather);
        self.physics.set_if_neq(options.physics);
        self.keys.set_if_neq(options.keys);
    }
}

/// One line of the options screen, and which value of `Options` it stands for.
struct OptionLine {
    /// Where it is kept in the store, and its id on the screen.
    key: &'static str,
    section: &'static str,
    label: &'static str,
    detail: &'static str,
    /// What its values are called. The store keeps the name, so that the list can grow.
    values: &'static [&'static str],
    /// Which of `values` the options stand at. Where they stand between two, as a command
    /// line can put them, the nearest.
    get: fn(&Options) -> usize,
    set: fn(&mut Options, usize),
}

const DISPLAY: &str = "DISPLAY";
const GRAPHICS: &str = "GRAPHICS";
const GAME: &str = "GAME";
const PHYSICS: &str = "PHYSICS";
// The sections of the controls screen, whose lines are key bindings, which wait for a key
// (see `ui::Setting::keys`).
const DRIVING: &str = "DRIVING";
const RACE: &str = "RACE";
const CAMERA: &str = "CAMERA";
const GAME_KEYS: &str = "GAME";
const FILES: &str = "FILES";
/// These sections are on the ADVANCED screen, out of the way of the Quality line, which
/// sets the graphics all at once.
fn is_advanced(line: &OptionLine) -> bool {
    matches!(line.section, GRAPHICS | PHYSICS)
}
/// Only these lines' values are keys: a GAME section of the options screen has none.
fn binds_a_key(line: &OptionLine) -> bool {
    line.values == KEY_NAMES
}

/// What each key a control may be bound to is called, in `BINDABLE`'s order.
const KEY_NAMES: [&str; BINDABLE.len()] = {
    let mut names = [""; BINDABLE.len()];
    let mut index = 0;
    while index < BINDABLE.len() {
        names[index] = BINDABLE[index].1;
        index += 1;
    }
    names
};

/// The line of the options screen that binds `control`, kept in the store as `store`.
macro_rules! key_line {
    ($store:literal, $section:expr, $control:expr) => {
        OptionLine {
            key: $store,
            section: $section,
            label: $control.name(),
            detail: "Enter or a click, then the key to bind; Esc stops. The arrow keys drive as well.",
            values: &KEY_NAMES,
            get: |options| {
                let key = options.keys.key($control);
                BINDABLE.iter().position(|(bindable, _)| *bindable == key).unwrap_or(0)
            },
            set: |options, value| options.keys.bind($control, BINDABLE[value].0),
        }
    };
}

// The values that are numbers, in the order of their names. Each holds its slice's default.
const SCREEN_MODES: [ScreenMode; 3] = [
    ScreenMode::Fullscreen,
    ScreenMode::Borderless,
    ScreenMode::Windowed,
];
const VSYNCS: [Vsync; 3] = [Vsync::Off, Vsync::On, Vsync::Strict];
const SHADOW_CASCADES: [f32; 4] = [0.0, 1.0, 2.0, 4.0];
/// In metres.
const SHADOW_DISTANCES: [f32; 4] = [50.0, 100.0, 150.0, 300.0];
const ANISOTROPIES: [f32; 5] = [1.0, 2.0, 4.0, 8.0, 16.0];
/// Physics steps a second, and solver passes in each.
const PHYSICS_RATES: [f32; 3] = [60.0, 90.0, 120.0];
const SUBSTEPS: [f32; 2] = [4.0, 6.0];
/// In metres, the last all of it.
const SCENERY_DISTANCES: [f32; 4] = [150.0, 300.0, 600.0, f32::INFINITY];
/// `TruckLooksSettings::reflectance`: none, half of Bevy's default, and Bevy's default.
const TRUCK_SHINES: [f32; 3] = [0.0, 0.25, 0.5];
/// How far the camera sits from the truck, and how high, as a share of how it was tuned.
/// The same share of both, so that it looks down at the same angle.
const CAMERA_SCALES: [f32; 3] = [0.8, 1.0, 1.3];

/// Which of `values` is nearest to `value`.
fn nearest(values: &[f32], value: f32) -> usize {
    (0..values.len())
        .min_by(|&a, &b| {
            (values[a] - value)
                .abs()
                .total_cmp(&(values[b] - value).abs())
        })
        .unwrap_or(0)
}

/// Where `value` is among `values`, or the first.
fn place<T: PartialEq>(values: &[T], value: &T) -> usize {
    values.iter().position(|each| each == value).unwrap_or(0)
}

const OPTIONS: &[OptionLine] = &[
    OptionLine {
        key: "option.screen",
        section: DISPLAY,
        label: "Screen",
        detail: "Fullscreen shows frames soonest. Borderless is quicker to switch away from.",
        values: &["Fullscreen", "Borderless", "Windowed"],
        get: |options| place(&SCREEN_MODES, &options.display.screen),
        set: |options, value| options.display.screen = SCREEN_MODES[value],
    },
    OptionLine {
        key: "option.vsync",
        section: DISPLAY,
        label: "Vsync",
        detail: "Off shows each frame as soon as it is done, and tears. Strict holds a late frame for the next refresh, and never tears.",
        values: &["Off", "On", "Strict"],
        get: |options| place(&VSYNCS, &options.display.vsync),
        set: |options, value| options.display.vsync = VSYNCS[value],
    },
    OptionLine {
        key: QUALITY,
        section: DISPLAY,
        label: "Quality",
        detail: "Every graphics setting at once, from the fastest to the finest. Balanced suits graphics built into the processor. ADVANCED sets them one by one.",
        // One for each of `QUALITY_LEVELS`, and Custom where the graphics match none.
        values: &["Fastest", "Fast", "Balanced", "High", "Best", "Custom"],
        get: quality_of,
        // The ui module sets the lines a level names, as their own changes, and keeps this
        // line at the level they match: it stands for them, and holds nothing itself.
        set: |_, _| {},
    },
    OptionLine {
        key: "option.physics_rate",
        section: PHYSICS,
        label: "Physics rate",
        detail: "How many times a second the trucks and the world are worked out. Lower costs less time on the CPU, but the trucks were tuned at 120, and the suspension may bounce or shake lower down.",
        values: &["60 Hz", "90 Hz", "120 Hz"],
        get: |options| nearest(&PHYSICS_RATES, options.physics.rate as f32),
        set: |options, value| options.physics.rate = PHYSICS_RATES[value] as f64,
    },
    OptionLine {
        key: "option.substeps",
        section: PHYSICS,
        label: "Substeps",
        detail: "How many times the physics goes over each of its steps. Fewer costs less time on the CPU, and holds stiff springs and stacked things less steadily.",
        values: &["4", "6"],
        get: |options| nearest(&SUBSTEPS, options.physics.substeps as f32),
        set: |options, value| options.physics.substeps = SUBSTEPS[value] as u32,
    },
    OptionLine {
        key: "option.antialiasing",
        section: GRAPHICS,
        label: "Antialiasing",
        detail: "Smooths jagged edges. FXAA softens the finished picture and costs little. 4x MSAA is sharper, and costs most on graphics built into the processor.",
        values: &["Off", "FXAA", "4x MSAA"],
        get: |options| place(&Antialiasing::ALL, &options.camera.antialiasing),
        set: |options, value| options.camera.antialiasing = Antialiasing::ALL[value],
    },
    OptionLine {
        key: "option.bloom",
        section: GRAPHICS,
        label: "Glow",
        detail: "Lamps and other bright lights spill a glow round them (bloom). Costs a little on every frame.",
        values: &["Off", "On"],
        get: |options| options.camera.bloom as usize,
        set: |options, value| options.camera.bloom = value == 1,
    },
    OptionLine {
        key: "option.shadows",
        section: GRAPHICS,
        label: "Shadows",
        detail: "How finely the sun's shadows are drawn. Each step up draws the scene once more.",
        values: &["Off", "Low", "Medium", "High"],
        get: |options| nearest(&SHADOW_CASCADES, options.environment.shadow_cascades as f32),
        set: |options, value| {
            options.environment.shadow_cascades = SHADOW_CASCADES[value] as usize;
        },
    },
    OptionLine {
        key: "option.shadow_distance",
        section: GRAPHICS,
        label: "Shadow reach",
        detail: "How far from the camera shadows are drawn. Further spreads them thinner.",
        values: &["50 m", "100 m", "150 m", "300 m"],
        get: |options| nearest(&SHADOW_DISTANCES, options.environment.shadow_distance),
        set: |options, value| options.environment.shadow_distance = SHADOW_DISTANCES[value],
    },
    OptionLine {
        key: "option.lighting",
        section: GRAPHICS,
        label: "Lighting",
        detail: "How the world is lit. Off draws everything in its plain colours, with no light, shade or shadow: the least each pixel can cost. Simple lights the ground and the scenery by the sun, its shadows and the sky alone, with no shine and no lamps. Full is all of it.",
        values: &["Off", "Simple", "Full"],
        get: |options| place(&Lighting::ALL, &options.environment.lighting),
        set: |options, value| options.environment.lighting = Lighting::ALL[value],
    },
    OptionLine {
        key: "option.anisotropy",
        section: GRAPHICS,
        label: "Filtering",
        detail: "How sharp the ground stays as it goes into the distance. Needs mipmaps.",
        values: &["1x", "2x", "4x", "8x", "16x"],
        get: |options| nearest(&ANISOTROPIES, options.track.anisotropy as f32),
        set: |options, value| options.track.anisotropy = ANISOTROPIES[value] as u16,
    },
    OptionLine {
        key: "option.scenery_distance",
        section: GRAPHICS,
        label: "Scenery distance",
        detail: "How far away trees, signs and buildings are drawn. A track can have thousands, and each one drawn costs time on every frame, however small. Solid ones stay solid beyond it.",
        values: &["150 m", "300 m", "600 m", "All"],
        get: |options| {
            // All is the last, and the nearest of the rest stands for any other distance.
            let distance = options.track.scenery_distance;
            let all = SCENERY_DISTANCES.len() - 1;
            if distance.is_finite() {
                nearest(&SCENERY_DISTANCES[..all], distance)
            } else {
                all
            }
        },
        set: |options, value| options.track.scenery_distance = SCENERY_DISTANCES[value],
    },
    OptionLine {
        key: "option.decorations",
        section: GRAPHICS,
        label: "Decorations",
        detail: "The trees, bushes and signs that trucks drive through. Off draws fewer things on every frame. What trucks can hit is always drawn.",
        values: &["Off", "On"],
        get: |options| options.track.decorations as usize,
        set: |options, value| options.track.decorations = value == 1,
    },
    OptionLine {
        key: "option.mipmaps",
        section: GRAPHICS,
        label: "Mipmaps",
        detail: "Stops textures shimmering in the distance. From the next race.",
        values: &["Off", "On"],
        get: |options| options.track.mipmaps as usize,
        set: |options, value| options.track.mipmaps = value == 1,
    },
    OptionLine {
        key: "option.truck_shine",
        section: GRAPHICS,
        label: "Truck shine",
        detail: "How much the trucks reflect the sky and the sun. Off shows the tire tread best.",
        values: &["Off", "Low", "Full"],
        get: |options| nearest(&TRUCK_SHINES, options.truck_looks.reflectance),
        set: |options, value| options.truck_looks.reflectance = TRUCK_SHINES[value],
    },
    OptionLine {
        key: "option.splashes",
        section: GRAPHICS,
        label: "Splashes",
        detail: "The spray and splashes that trucks and the shore throw up. The ripples stay.",
        values: &["Off", "On"],
        get: |options| options.water.splashes as usize,
        set: |options, value| options.water.splashes = value == 1,
    },
    OptionLine {
        key: "option.dirt",
        section: GRAPHICS,
        label: "Dirt",
        detail: "The clods and dust that trucks throw up as they drive.",
        values: &["Off", "On"],
        get: |options| options.dirt.on as usize,
        set: |options, value| options.dirt.on = value == 1,
    },
    OptionLine {
        key: "option.backdrop",
        section: GRAPHICS,
        label: "Backdrop",
        detail: "The distant hills a track draws round its horizon. Off, the sky reaches down to the ground.",
        values: &["Off", "On"],
        get: |options| options.backdrop.on as usize,
        set: |options, value| options.backdrop.on = value == 1,
    },
    OptionLine {
        key: "option.camera",
        section: GAME,
        label: "Camera",
        detail: "How far behind the truck the camera sits.",
        values: &["Near", "Normal", "Far"],
        get: |options| {
            let tuned = CameraSettings::default().rig;
            nearest(&CAMERA_SCALES, options.camera.rig.distance / tuned.distance)
        },
        set: |options, value| {
            let tuned = CameraSettings::default().rig;
            let rig = &mut options.camera.rig;
            rig.distance = tuned.distance * CAMERA_SCALES[value];
            rig.height = tuned.height * CAMERA_SCALES[value];
        },
    },
    OptionLine {
        key: "option.weather",
        section: GAME,
        label: "Weather",
        detail: "Rain and snow make the ground slippery, and snow freezes the water to ice. Random picks anew for each race. F7 changes it in a race.",
        // Random first, and then `Weather::ALL` in its order.
        values: &[
            "Random", "Clear", "Overcast", "Fog", "Rain", "Storm", "Snow",
        ],
        get: |options| {
            if options.weather.random {
                0
            } else {
                1 + place(&Weather::ALL, &options.weather.weather)
            }
        },
        set: |options, value| {
            options.weather.random = value == 0;
            if value > 0 {
                options.weather.weather = Weather::ALL[value - 1];
            }
        },
    },
    OptionLine {
        key: "option.time_of_day",
        section: GAME,
        label: "Time of day",
        detail: "At dusk and at night the trucks' lamps are on. F8 changes it in a race.",
        values: &["Day", "Dusk", "Night"],
        get: |options| place(&TimeOfDay::ALL, &options.weather.time_of_day),
        set: |options, value| options.weather.time_of_day = TimeOfDay::ALL[value],
    },
    OptionLine {
        key: "option.smooth_terrain",
        section: GAME,
        label: "Smooth ground",
        detail: "Lights the ground as if the creases that Monster Truck Madness 2 has were rounded off. The shape of the ground does not change. From the next race.",
        values: &["Off", "On"],
        get: |options| options.track.smooth_terrain as usize,
        set: |options, value| options.track.smooth_terrain = value == 1,
    },
    OptionLine {
        key: "option.speed_units",
        section: GAME,
        label: "Speed",
        detail: "What the speedometer reads in.",
        values: &["km/h", "mph"],
        get: |options| (options.speed == SpeedUnits::Mph) as usize,
        set: |options, value| {
            options.speed = if value == 1 {
                SpeedUnits::Mph
            } else {
                SpeedUnits::Kmh
            };
        },
    },
    OptionLine {
        key: "option.controls_help",
        section: GAME,
        label: "Key help",
        detail: "The line of keys across the top of the screen in a race. From the next race.",
        values: &["Hide", "Show"],
        get: |options| options.help.show as usize,
        set: |options, value| options.help.show = value == 1,
    },
    // In `Control::ALL`'s order.
    key_line!("key.throttle", DRIVING, Control::Throttle),
    key_line!("key.reverse", DRIVING, Control::Reverse),
    key_line!("key.steer_left", DRIVING, Control::SteerLeft),
    key_line!("key.steer_right", DRIVING, Control::SteerRight),
    key_line!("key.handbrake", DRIVING, Control::Handbrake),
    key_line!("key.flip_upright", DRIVING, Control::FlipUpright),
    key_line!("key.back_to_checkpoint", RACE, Control::BackToCheckpoint),
    key_line!("key.race_again", RACE, Control::RaceAgain),
    key_line!("key.change_view", CAMERA, Control::ChangeView),
    key_line!("key.look_left", CAMERA, Control::LookLeft),
    key_line!("key.look_right", CAMERA, Control::LookRight),
    key_line!("key.look_back", CAMERA, Control::LookBack),
    key_line!("key.next_weather", GAME_KEYS, Control::NextWeather),
    key_line!("key.next_time_of_day", GAME_KEYS, Control::NextTimeOfDay),
    key_line!("key.physics_debug", GAME_KEYS, Control::PhysicsDebug),
    key_line!(
        "key.graphics_settings",
        GAME_KEYS,
        Control::GraphicsSettings
    ),
];

/// Graphics for a graphics processor built into the CPU, which shares the computer's
/// memory: what costs most on one turned down, and the rest left as it is. Four samples
/// a pixel give way to FXAA, a pass over the finished picture, and the glow goes, since
/// it draws the picture in high dynamic range: both cost most where memory is slowest.
/// The sun's shadows keep two of their four maps, over 100 m, and the ground's filtering
/// keeps a quarter of its sharpness. The ground and the scenery are lit the simple way
/// (`lighting.rs`): drawing them unlit was measured to save a great deal on such a GPU. Scenery is drawn to 300 m, which measured on Snake
/// River Canyon (`--no-vsync`, no shadows, HD Graphics 620 at 1366 x 768) took the
/// meshes drawn from about 1400 to 290 and the frame from 21 ms to the screen's 16.7.
/// The rest are chosen, not measured: measure with the F2 panel and `--no-vsync` (see
/// `docs/smoothness.md`) before changing them. It is the Balanced level of the options
/// screen's Quality line, and `--integrated-graphics` starts a race with it.
pub fn integrated_graphics(
    camera: &mut CameraSettings,
    environment: &mut EnvironmentSettings,
    track: &mut TrackSettings,
) {
    camera.antialiasing = Antialiasing::Fxaa;
    camera.bloom = false;
    environment.shadow_cascades = 2;
    environment.shadow_distance = 100.0;
    environment.lighting = Lighting::Simple;
    track.anisotropy = 4;
    track.scenery_distance = 300.0;
}

/// The options screen's Quality line, which stands for the graphics lines that cost most.
const QUALITY: &str = "option.quality";

/// The lines each level of Quality sets: those that cost time on every frame. The rest of
/// the graphics (mipmaps, which save time, and the trucks' shine, which is a look) and the
/// physics are left as the player has them.
const QUALITY_LINES: [&str; 11] = [
    "option.antialiasing",
    "option.bloom",
    "option.shadows",
    "option.shadow_distance",
    "option.lighting",
    "option.anisotropy",
    "option.scenery_distance",
    "option.decorations",
    "option.splashes",
    "option.dirt",
    "option.backdrop",
];

/// Quality's levels, fastest first, each as it changes the defaults. Balanced is
/// `integrated_graphics`, and Best is the defaults.
const QUALITY_LEVELS: [fn(&mut Options); 5] = [
    // Fastest: everything that can go, goes, the lighting with it.
    |options| {
        options.environment.lighting = Lighting::Off;
        options.camera.antialiasing = Antialiasing::Off;
        options.camera.bloom = false;
        options.environment.shadow_cascades = 0;
        options.environment.shadow_distance = 50.0;
        options.track.anisotropy = 1;
        options.track.scenery_distance = 150.0;
        options.track.decorations = false;
        options.water.splashes = false;
        options.dirt.on = false;
        options.backdrop.on = false;
    },
    // Fast: the simple lighting, one shadow map near the truck, and the dirt and the hills
    // back.
    |options| {
        options.environment.lighting = Lighting::Simple;
        options.camera.antialiasing = Antialiasing::Fxaa;
        options.camera.bloom = false;
        options.environment.shadow_cascades = 1;
        options.environment.shadow_distance = 50.0;
        options.track.anisotropy = 2;
        options.track.scenery_distance = 150.0;
        options.track.decorations = false;
        options.water.splashes = false;
    },
    |options| {
        integrated_graphics(
            &mut options.camera,
            &mut options.environment,
            &mut options.track,
        );
    },
    // High: the defaults, with the scenery drawn to 600 m and less filtering.
    |options| {
        options.track.anisotropy = 8;
        options.track.scenery_distance = 600.0;
    },
    |_| {},
];

/// For each level of Quality, the value it sets each line of `OPTIONS` to, where it sets
/// that line at all.
fn quality_levels() -> Vec<Vec<Option<usize>>> {
    QUALITY_LEVELS
        .iter()
        .map(|level| {
            let mut options = Options::defaults();
            level(&mut options);
            OPTIONS
                .iter()
                .map(|line| {
                    QUALITY_LINES
                        .contains(&line.key)
                        .then(|| (line.get)(&options))
                })
                .collect()
        })
        .collect()
}

/// The first level of Quality whose lines all stand where `options` do, or Custom, past
/// the levels, where none does: as the ui module works it out.
fn quality_of(options: &Options) -> usize {
    quality_levels()
        .iter()
        .position(|level| {
            OPTIONS
                .iter()
                .zip(level)
                .all(|(line, value)| value.is_none_or(|value| (line.get)(options) == value))
        })
        .unwrap_or(QUALITY_LEVELS.len())
}

/// The options screen's lines, standing where `options` do.
fn settings_for(options: &Options) -> Vec<Setting> {
    let defaults = Options::defaults();
    OPTIONS
        .iter()
        .map(|line| Setting {
            keys: if binds_a_key(line) {
                BINDABLE.iter().map(|(key, _)| *key).collect()
            } else {
                Vec::new()
            },
            id: line.key.into(),
            section: line.section.into(),
            label: line.label.into(),
            values: line.values.iter().map(|&value| value.into()).collect(),
            chosen: (line.get)(options),
            default: (line.get)(&defaults),
            detail: line.detail.into(),
            opens: false,
            advanced: is_advanced(line),
            levels: if line.key == QUALITY {
                quality_levels()
            } else {
                Vec::new()
            },
        })
        .collect()
}

/// What was set last time, by the name of its value. A name that is no longer one of the
/// values is let go.
fn remember_options(store: &Store, options: &mut Options) -> Result<(), store::StoreError> {
    for line in OPTIONS {
        if let Some(name) = store.get_text(line.key)?
            && let Some(value) = line.values.iter().position(|&value| value == name)
        {
            (line.set)(options, value);
        }
    }
    Ok(())
}

/// The options screen is brought up to date whenever the front end opens: F2 may have
/// changed the graphics in a race.
fn show_the_options(resources: OptionResources, mut settings: ResMut<ui::Settings>) {
    let options = resources.get();
    let settings = settings.bypass_change_detection();
    for (setting, line) in settings.0.iter_mut().zip(OPTIONS) {
        setting.chosen = (line.get)(&options);
    }
}

/// The ui module changes the settings and doesn't know what they do. This does, and keeps
/// what was changed.
fn apply_the_options(
    settings: Res<ui::Settings>,
    mut resources: OptionResources,
    saves: Res<Saves>,
) {
    if !settings.is_changed() {
        return;
    }
    let mut options = resources.get();
    // Only the lines the player changed. The rest may stand between two values.
    let changed: Vec<(&OptionLine, &Setting)> = OPTIONS
        .iter()
        .zip(&settings.0)
        .filter(|(line, setting)| (line.get)(&options) != setting.chosen)
        .collect();
    if changed.is_empty() {
        return;
    }
    for (line, setting) in &changed {
        (line.set)(&mut options, setting.chosen.min(line.values.len() - 1));
    }
    resources.set(options);

    // A few lines, once for each press of a key.
    if let Some(store) = &saves.0 {
        let written = store.write(|batch| {
            for (line, setting) in &changed {
                batch.set_text(line.key, setting.chosen_value());
            }
        });
        if let Err(error) = written {
            warn!("Options were not saved: {error}");
        }
    }
}

/// A base game line on the options screen opens the browser, where that folder is.
fn open_the_browser(
    mut opened: MessageReader<SettingOpened>,
    lines: Res<ui::Settings>,
    folders: Res<BaseGameFolders>,
    mut browser: ResMut<FolderBrowser>,
    mut browsing: ResMut<Browsing>,
) {
    let which = opened.read().find_map(|SettingOpened(index)| {
        let id = &lines.0.get(*index)?.id;
        BaseFolder::ALL.into_iter().find(|which| which.key() == id)
    });
    if let Some(which) = which {
        let view;
        (*browsing, view) = Browsing::start(which, None, "CANCEL", &folders);
        browser.0 = Some(view);
    }
}

/// Into a folder, up out of one, or out of the browser.
fn browse(
    mut entered: MessageReader<FolderEntered>,
    mut up: MessageReader<FolderUp>,
    mut left: MessageReader<FolderLeft>,
    mut browser: ResMut<FolderBrowser>,
    mut browsing: ResMut<Browsing>,
) {
    let (which, leave) = (browsing.which, browsing.leave);
    let Some(at) = &mut browsing.at else {
        entered.clear();
        up.clear();
        left.clear();
        return;
    };
    let mut highlight = None;
    let mut moved = false;
    for FolderEntered(index) in entered.read() {
        let name = browser
            .0
            .as_ref()
            .and_then(|folder| folder.folders.get(*index))
            .map(|entry| entry.id.clone());
        if let Some(name) = name {
            at.push(name);
            highlight = None;
            moved = true;
        }
    }
    for FolderUp in up.read() {
        if let Some(parent) = at.parent().map(Path::to_path_buf) {
            // Back where the player came from.
            highlight = at.file_name().map(OsString::from);
            *at = parent;
            moved = true;
        }
    }
    if left.read().count() > 0 {
        browser.0 = None;
        browsing.at = None;
    } else if moved {
        browser.0 = Some(folder_view(at, highlight.as_ref(), which, leave));
    }
}

/// The folder chosen is where that part of the base game is from now on: it is kept, the
/// base game is opened again from both folders, and the lists are made again. What is
/// highlighted stays so where it can. Then the other folder is asked for, if it is to be.
#[expect(
    clippy::too_many_arguments,
    reason = "the base game is under everything"
)]
fn choose_the_base_game(
    mut commands: Commands,
    mut chosen: MessageReader<FolderChosen>,
    mut browser: ResMut<FolderBrowser>,
    mut browsing: ResMut<Browsing>,
    (mut base, mut folders): (ResMut<BaseGame>, ResMut<BaseGameFolders>),
    settings: Res<FrontEndSettings>,
    (mut catalogue, mut choices, mut lines): (
        ResMut<Catalogue>,
        ResMut<Choices>,
        ResMut<ui::Settings>,
    ),
    mut showing: ResMut<Showing>,
    saves: Res<Saves>,
) {
    if chosen.read().count() == 0 {
        return;
    }
    browser.0 = None;
    let Some(at) = browsing.at.take() else {
        return;
    };
    let which = browsing.which;

    if let Some(store) = &saves.0 {
        let written = match at.to_str() {
            Some(folder) => store.write(|batch| {
                batch.set_text(which.key(), folder);
            }),
            None => Err(store::StoreError {
                store: "saves".into(),
                what: format!("{} is not a name that can be kept", at.display()),
            }),
        };
        if let Err(error) = written {
            warn!("Where the base game is was not saved: {error}");
        }
    }
    folders.chosen.set(which, at);
    *base = BaseGame::open(&folders.chosen.folders());
    for line in &mut lines.0 {
        if let Some(which) = BaseFolder::ALL
            .into_iter()
            .find(|which| which.key() == line.id)
        {
            *line = base_folder_line(which, &folders.chosen);
        }
    }

    // Whatever was loaded borrowed from the last base game, or found nothing to borrow.
    if let Some((_, entity)) = showing.truck_on_show.take() {
        commands.entity(entity).despawn();
    }
    *showing = Showing::default();

    let new = catalogue_for(&settings, &base);
    let remembered = |key: &str| {
        saves
            .0
            .as_ref()
            .and_then(|store| store.get_text(key).ok().flatten())
    };
    // What was chosen last time, if it has only now come back; else what is highlighted,
    // which may only stand for it until then.
    let find = |old: &[Entry], chosen: usize, new: &[Entry], key: &str| {
        let highlighted = old.get(chosen).map(|entry| entry.id.clone());
        let remembered = remembered(key);
        let returned = remembered
            .clone()
            .filter(|id| !old.iter().any(|entry| entry.id == *id));
        [returned, highlighted, remembered]
            .into_iter()
            .flatten()
            .find_map(|id| new.iter().position(|entry| entry.id == id))
            .unwrap_or(0)
    };
    choices.truck = find(&catalogue.trucks, choices.truck, &new.trucks, KEY_TRUCK);
    choices.track = find(&catalogue.tracks, choices.track, &new.tracks, KEY_TRACK);
    *catalogue = new;

    if let Some(next) = browsing.then.filter(|&next| !folders.chosen.usable(next)) {
        let view;
        (*browsing, view) = Browsing::start(next, None, browsing.leave, &folders);
        browser.0 = Some(view);
    }
}

fn open(mut open: ResMut<NextState<FrontEndOpen>>, mut showing: ResMut<Showing>) {
    open.as_mut().set_if_neq(FrontEndOpen::Open);
    // The turntable is new, and bare.
    showing.truck_on_show = None;
}

fn close(mut open: ResMut<NextState<FrontEndOpen>>) {
    open.as_mut().set_if_neq(FrontEndOpen::Closed);
}

fn note_what_is_highlighted(
    catalogue: Res<Catalogue>,
    mut trucks: MessageReader<TruckHighlighted>,
    mut tracks: MessageReader<TrackHighlighted>,
    mut showing: ResMut<Showing>,
) {
    // Something that can't be had is nothing to show.
    let id = |entries: &[Entry], index: usize| {
        entries
            .get(index)
            .filter(|entry| entry.available)
            .map(|entry| entry.id.clone())
    };
    if let Some(TruckHighlighted(index)) = trucks.read().last() {
        showing.wanted_truck = id(&catalogue.trucks, *index);
    }
    if let Some(TrackHighlighted(index)) = tracks.read().last() {
        showing.wanted_track = id(&catalogue.tracks, *index);
    }
}

fn load_truck(id: &str, base: &BaseGame) -> Result<TruckData, String> {
    match (id, in_base(id)) {
        (BUILTIN, _) => Ok(TruckData::builtin()),
        (_, Some((archive, file))) => truck::load_base(base, archive, file),
        (path, None) => truck::load_pod(Path::new(path), base),
    }
}

fn load_track(id: &str, base: &BaseGame) -> Result<TrackData, String> {
    match (id, in_base(id)) {
        (BUILTIN, _) => Ok(track::builtin_track()),
        (_, Some((archive, file))) => track::load_base(base, archive, file),
        (path, None) => track::load_pod(Path::new(path), base),
    }
}

/// A truck or track of the base game's, one of several in its archive. Its id names the
/// archive and the file in it: `base:<archive path>#<file>`. A file in an archive has no
/// `#` in its name, and the plain path of a truck or track in `trucks/` or `tracks/`
/// never starts with `base:`.
fn base_entry(archive: &Path, file: &str, name: Result<String, String>) -> Entry {
    let archive_name = archive
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let detail = format!("base game: {archive_name}");
    let (name, detail, available) = match name {
        Ok(name) if name.trim().is_empty() => (file.to_string(), detail, true),
        Ok(name) => (name.trim().to_string(), detail, true),
        Err(error) => (file.to_string(), error, false),
    };
    Entry {
        id: format!("{BASE_PREFIX}{}#{file}", archive.display()),
        name,
        detail,
        available,
    }
}

/// The archive and file that the id of a base game truck or track names.
fn in_base(id: &str) -> Option<(&Path, &str)> {
    let (archive, file) = id.strip_prefix(BASE_PREFIX)?.rsplit_once('#')?;
    Some((Path::new(archive), file))
}

/// One load of each kind at a time. A load that is no longer wanted is let finish, and
/// kept: the player may well come back to it.
fn load_what_is_wanted(
    mut showing: ResMut<Showing>,
    mut catalogue: ResMut<Catalogue>,
    base: Res<BaseGame>,
) {
    let showing = showing.bypass_change_detection();
    // An archive that listed and won't load is greyed out, with the reason.
    let failed = |entries: &mut Vec<Entry>, id: &str, error: String| {
        warn!("{error}");
        if let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) {
            entry.available = false;
            entry.detail = error;
        }
    };

    if let Some((id, result)) = finished(&mut showing.loading_truck) {
        match result {
            Ok(truck) => showing.keep_truck(id, truck),
            Err(error) => failed(&mut catalogue.trucks, &id, error),
        }
    }
    if let Some((id, result)) = finished(&mut showing.loading_track) {
        match result {
            Ok(track) => showing.track = Some((id, track)),
            Err(error) => failed(&mut catalogue.tracks, &id, error),
        }
    }

    let tasks = AsyncComputeTaskPool::get();
    if let Some(id) = showing.wanted_truck.clone()
        && showing.loading_truck.is_none()
        && showing.truck(&id).is_none()
        && is_available(&catalogue.trucks, &id)
    {
        let (load, base) = (id.clone(), base.clone());
        showing.loading_truck = Some((id, tasks.spawn(async move { load_truck(&load, &base) })));
    }
    if let Some(id) = showing.wanted_track.clone()
        && showing.loading_track.is_none()
        && showing.track(&id).is_none()
        && is_available(&catalogue.tracks, &id)
    {
        let (load, base) = (id.clone(), base.clone());
        showing.loading_track = Some((id, tasks.spawn(async move { load_track(&load, &base) })));
    }
}

/// The id and the outcome of a load that has finished, which is then no longer loading.
fn finished<T>(loading: &mut Loading<T>) -> Option<(String, Result<T, String>)> {
    let (_, task) = loading.as_mut()?;
    let result = block_on(poll_once(task))?;
    let (id, _) = loading.take()?;
    Some((id, result))
}

fn is_available(entries: &[Entry], id: &str) -> bool {
    entries
        .iter()
        .any(|entry| entry.id == id && entry.available)
}

/// Puts the wanted truck on the turntable once it is loaded, in place of the last one,
/// which stays until then.
fn show_the_truck(
    mut commands: Commands,
    mut showing: ResMut<Showing>,
    turntable: Option<Single<Entity, With<Turntable>>>,
) {
    let Some(turntable) = turntable else {
        return;
    };
    let showing = showing.bypass_change_detection();
    let on_show = showing.truck_on_show.as_ref().map(|(id, _)| id);
    if on_show == showing.wanted_truck.as_ref() {
        return;
    }
    let wanted = showing.wanted_truck.clone();
    let truck = wanted.as_ref().and_then(|id| showing.truck(id)).cloned();
    // Nothing is wanted, or what is wanted is ready: either way the last one goes.
    if wanted.is_some() && truck.is_none() {
        return;
    }
    if let Some((_, entity)) = showing.truck_on_show.take() {
        commands.entity(entity).despawn();
    }
    if let (Some(id), Some(truck)) = (wanted, truck) {
        let entity = commands
            .spawn((
                Name::new("Truck on show"),
                // Its tires on the turntable, whatever its size.
                Transform::from_xyz(0.0, truck.config.standing_height(), 0.0),
                TruckDisplay(truck),
                ChildOf(*turntable),
            ))
            .id();
        showing.truck_on_show = Some((id, entity));
    }
}

fn show_the_map(
    mut showing: ResMut<Showing>,
    mut preview: ResMut<TrackPreview>,
    // Absent in a headless app, which has nowhere to show a map.
    images: Option<ResMut<Assets<Image>>>,
) {
    let showing = showing.bypass_change_detection();
    if showing.map_of == showing.wanted_track {
        return;
    }
    let map = match &showing.wanted_track {
        None => None,
        Some(id) => match showing.track(id) {
            Some(track) => Some(track::map_image(track, MAP_SIZE)),
            // On its way. The last map stays until then.
            None => return,
        },
    };
    showing.map_of = showing.wanted_track.clone();
    let Some(mut images) = images else {
        return;
    };
    preview.0 = map.map(|rgba| {
        images.add(Image::new(
            Extent3d {
                width: MAP_SIZE as u32,
                height: MAP_SIZE as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        ))
    });
}

/// The ui crate moves the dials and doesn't know what they do. This does.
fn set_the_truck_up(dials: Res<Dials>, mut setup: ResMut<TruckSetup>) {
    if !dials.is_changed() {
        return;
    }
    let mut set_up = *setup;
    for dial in &dials.0 {
        match dial.id.as_str() {
            KEY_SUSPENSION => set_up.suspension = dial.value(),
            KEY_GEARING => set_up.gearing = dial.value(),
            KEY_REAR_STEERING => set_up.rear_steering = dial.value(),
            KEY_GRIP => set_up.grip = dial.value(),
            _ => {}
        }
    }
    if *setup != set_up {
        *setup = set_up;
    }
}

/// Hands the choices to the slices, remembers them, and enters the race.
#[expect(clippy::too_many_arguments, reason = "it is where everything meets")]
fn go(
    mut commands: Commands,
    mut pressed: MessageReader<GoPressed>,
    choices: Res<Choices>,
    mut catalogue: ResMut<Catalogue>,
    setup: Res<TruckSetup>,
    showing: Res<Showing>,
    base: Res<BaseGame>,
    saves: Res<Saves>,
    mut race: ResMut<RaceSettings>,
    mut game: ResMut<NextState<GameState>>,
    mut open: ResMut<NextState<FrontEndOpen>>,
) {
    if pressed.read().count() == 0 {
        return;
    }
    let (Some(truck_entry), Some(track_entry)) = (
        catalogue.trucks.get(choices.truck).cloned(),
        catalogue.tracks.get(choices.track).cloned(),
    ) else {
        return;
    };

    // Whatever was highlighted long enough is loaded already. GO straight after moving
    // waits for the rest here, where a track is about to be built anyway.
    let truck = match showing.truck(&truck_entry.id) {
        Some(truck) => Ok(truck.clone()),
        None => load_truck(&truck_entry.id, &base),
    };
    let track = match showing.track(&track_entry.id) {
        Some(track) => Ok(track.clone()),
        None => load_track(&track_entry.id, &base),
    };
    let (truck, track) = match (truck, track) {
        (Ok(truck), Ok(track)) => (truck, track),
        (truck, track) => {
            // Stay, and say why on the entry that failed.
            let catalogue = &mut *catalogue;
            for (entries, id, error) in [
                (&mut catalogue.trucks, &truck_entry.id, truck.err()),
                (&mut catalogue.tracks, &track_entry.id, track.err()),
            ] {
                if let (Some(error), Some(entry)) =
                    (error, entries.iter_mut().find(|entry| entry.id == *id))
                {
                    warn!("{error}");
                    entry.available = false;
                    entry.detail = error;
                }
            }
            return;
        }
    };

    commands.insert_resource(ComputerTrucks(opponents(
        choices.opponents as usize,
        choices.truck,
        &truck,
        &catalogue,
        &showing,
        &base,
    )));
    commands.insert_resource(ChosenTruck(truck));
    commands.insert_resource(ChosenTrack(track));
    race.laps = choices.laps;

    // One transaction, and only here: never per frame, and never while racing.
    if let Some(store) = &saves.0 {
        let written = store.write(|batch| {
            batch
                .set_text(KEY_TRUCK, &truck_entry.id)
                .set_text(KEY_TRACK, &track_entry.id)
                .set_number(KEY_LAPS, choices.laps as f64)
                .set_number(KEY_OPPONENTS, choices.opponents as f64)
                .set_number(KEY_SUSPENSION, setup.suspension as f64)
                .set_number(KEY_GEARING, setup.gearing as f64)
                .set_number(KEY_REAR_STEERING, setup.rear_steering as f64)
                .set_number(KEY_GRIP, setup.grip as f64);
        });
        if let Err(error) = written {
            warn!("Choices were not saved: {error}");
        }
    }

    game.as_mut().set_if_neq(GameState::Racing);
    // Together with it, so that no frame is drawn with neither a race nor a showroom.
    open.as_mut().set_if_neq(FrontEndOpen::Closed);
}

/// EXIT closes the game. What was chosen is already in the store: it is written on GO and
/// as each option changes.
fn exit(mut pressed: MessageReader<ExitPressed>, mut exit: MessageWriter<AppExit>) {
    if pressed.read().count() > 0 {
        exit.write(AppExit::Success);
    }
}

/// The trucks that the computer drives: the ones listed after the player's, round and
/// round the list, so that a race shows off what is in `trucks/`. One that won't load is
/// the player's truck again, and each has its number's colour, which tells apart the ones
/// that are drawn as the built-in box.
fn opponents(
    count: usize,
    players: usize,
    players_truck: &TruckData,
    catalogue: &Catalogue,
    showing: &Showing,
    base: &BaseGame,
) -> Vec<TruckData> {
    let available: Vec<&Entry> = catalogue
        .trucks
        .iter()
        .cycle()
        .skip(players + 1)
        .take(catalogue.trucks.len())
        .filter(|entry| entry.available)
        .collect();
    // Each archive is read once, however many of its truck there are.
    let mut loaded: Vec<(&str, TruckData)> = Vec::new();
    (0..count)
        .map(|index| {
            let truck = match available.get(index % available.len().max(1)) {
                None => players_truck.clone(),
                Some(entry) => {
                    if !loaded.iter().any(|(id, _)| *id == entry.id) {
                        let truck = showing
                            .truck(&entry.id)
                            .cloned()
                            .or_else(|| {
                                load_truck(&entry.id, base)
                                    .inspect_err(|error| warn!("{error}"))
                                    .ok()
                            })
                            .unwrap_or_else(|| players_truck.clone());
                        loaded.push((&entry.id, truck));
                    }
                    let (_, truck) = loaded
                        .iter()
                        .find(|(id, _)| *id == entry.id)
                        .expect("just loaded");
                    truck.clone()
                }
            };
            truck.in_color(index + 1)
        })
        .collect()
}

/// Cancelling the race from its pause dialog comes back to the front end.
fn leave_the_race(
    mut cancelled: MessageReader<RaceCancelled>,
    mut game: ResMut<NextState<GameState>>,
    mut open: ResMut<NextState<FrontEndOpen>>,
) {
    if cancelled.read().count() > 0 {
        game.as_mut().set_if_neq(GameState::FrontEnd);
        open.as_mut().set_if_neq(FrontEndOpen::Open);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_base_game_id_names_its_archive_and_file() {
        let entry = base_entry(
            Path::new("base/Shared/JUNK.POD"),
            "WORLD\\GRAVEY.SIT",
            Ok("The Graveyard".into()),
        );
        assert_eq!(entry.name, "The Graveyard");
        assert_eq!(entry.detail, "base game: JUNK.POD");
        assert_eq!(
            in_base(&entry.id),
            Some((Path::new("base/Shared/JUNK.POD"), "WORLD\\GRAVEY.SIT"))
        );
    }

    #[test]
    fn quality_has_a_value_for_each_level_and_custom_past_them() {
        let line = OPTIONS.iter().find(|line| line.key == QUALITY).unwrap();
        assert_eq!(line.values.len(), QUALITY_LEVELS.len() + 1);
        assert_eq!(line.values[QUALITY_LEVELS.len()], "Custom");
        assert!(!is_advanced(line));
        // Every line it sets is a graphics line, on the advanced screen, and each is there.
        for key in QUALITY_LINES {
            let line = OPTIONS.iter().find(|line| line.key == key).unwrap();
            assert_eq!(line.section, GRAPHICS, "{key}");
        }
    }

    #[test]
    fn the_defaults_are_the_best_quality_and_integrated_graphics_balanced() {
        let defaults = Options::defaults();
        assert_eq!(QUALITY_LEVELS.len() - 1, quality_of(&defaults));
        let mut integrated = defaults.clone();
        integrated_graphics(
            &mut integrated.camera,
            &mut integrated.environment,
            &mut integrated.track,
        );
        assert_eq!(quality_of(&integrated), 2);
        // A line changed by hand is no level.
        integrated.dirt.on = false;
        assert_eq!(quality_of(&integrated), QUALITY_LEVELS.len());
    }

    #[test]
    fn each_level_of_quality_is_its_own() {
        let levels = quality_levels();
        for (index, level) in levels.iter().enumerate() {
            let mut options = Options::defaults();
            QUALITY_LEVELS[index](&mut options);
            assert_eq!(quality_of(&options), index);
            assert!(levels[..index].iter().all(|earlier| earlier != level));
        }
    }

    #[test]
    fn a_path_in_the_tracks_folder_is_not_a_base_game_id() {
        assert_eq!(in_base("tracks/My#Track.pod"), None);
        assert_eq!(in_base(BUILTIN), None);
    }

    #[test]
    fn a_base_game_file_that_cannot_be_read_is_listed_greyed_out() {
        let entry = base_entry(
            Path::new("TRUCK2.POD"),
            "TRUCK\\X.TRK",
            Err("broken".into()),
        );
        assert!(!entry.available);
        assert_eq!(entry.name, "TRUCK\\X.TRK");
        assert_eq!(entry.detail, "broken");
    }
}
