//! The game's front end: choosing a truck, a track and a setup, and pressing GO.
//!
//! This module knows nothing about the game, as `pod` knows nothing about it. It shows
//! whatever plain data it is handed and says what the player did:
//!
//! | Item | Direction | Holds |
//! | --- | --- | --- |
//! | `Catalogue` | in | The trucks and tracks to list. |
//! | `Dials` | in and out | The garage's dials. The module draws and moves them, and doesn't know what they do. |
//! | `Choices` | in and out | Which truck and track, how many laps, and how many trucks the computer drives. |
//! | `Settings` | in and out | The options and advanced screens' settings, each a few named values. The module draws and changes them, and doesn't know what they do. |
//! | `TrackPreview` | in | A picture of the highlighted track. |
//! | `Turntable` | in | On the entity the module spawns and turns. Parent to it whatever is to be shown. |
//! | `FolderBrowser` | in and out | A folder the player is browsing, shown in place of the screens. The module writes which subfolder is highlighted. |
//! | `TruckHighlighted`, `TrackHighlighted`, `GoPressed`, `ExitPressed` | out | What the player just did. |
//! | `SettingOpened`, `FolderEntered`, `FolderUp`, `FolderChosen`, `FolderLeft` | out | The same, on a line that opens something and in a folder being browsed. The game changes `FolderBrowser` to suit. |
//! | `PlayerDid` | in | An `Action`, from the module's own input or from anyone else. |
//! | `FrontEndOpen` | in | The screens and the showroom exist while it is `Open`. |
//! | `LoadingScreen` | in | Something being got ready, drawn over everything, open or not, while it holds one. The player can do nothing while it is up. |
//!
//! `Catalogue`, `Choices`, `Dials` and `Settings` are read when the front end opens and
//! whenever the `Catalogue` or the `FolderBrowser` changes. From then on the module writes `Choices`, each
//! `Dial::step` and each `Setting::chosen`.
//!
//! Inside, one module per concern: `model` (the rules, a pure state machine), `input`
//! (bindings), `screens` (what is drawn, which reads the model and never changes it),
//! `theme` (every colour and size), `showroom` (the showroom behind it all) and `loading`
//! (the loading screen). Widgets are
//! `bevy_ui_widgets` buttons. What is highlighted lives in the model and not in
//! `bevy_input_focus`, so that a key, a stick and a click all go the same way.
//!
//! The plugin works in a headless app: the screens are plain components, and the showroom
//! is built without meshes where there is nowhere to keep them. It needs `StatesPlugin`,
//! and adds it if no one has.

mod input;
mod loading;
mod model;
mod screens;
mod showroom;
mod theme;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

pub use model::{Action, MOST_LAPS, MOST_OPPONENTS, Screen};
pub use showroom::Turntable;
pub use theme::Theme;

use model::{Browsing, DialSteps, FrontEnd, Happened, List, SettingValues};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        if !app.world().contains_resource::<State<FrontEndOpen>>() {
            app.init_state::<FrontEndOpen>();
        }
        app.init_resource::<Catalogue>()
            .init_resource::<Dials>()
            .init_resource::<Choices>()
            .init_resource::<Settings>()
            .init_resource::<Presets>()
            .init_resource::<TrackPreview>()
            .init_resource::<FolderBrowser>()
            .init_resource::<LoadingScreen>()
            .init_resource::<Theme>()
            .init_resource::<screens::OptionsScrolled>()
            .add_message::<PlayerDid>()
            .add_message::<TruckHighlighted>()
            .add_message::<TrackHighlighted>()
            .add_message::<GoPressed>()
            .add_message::<ExitPressed>()
            .add_message::<SettingOpened>()
            .add_message::<FolderEntered>()
            .add_message::<FolderUp>()
            .add_message::<FolderChosen>()
            .add_message::<FolderLeft>()
            .add_observer(input::widget_activated)
            .add_observer(showroom::dragged)
            .add_observer(showroom::let_go)
            .add_systems(OnEnter(FrontEndOpen::Open), showroom::spawn_showroom)
            .add_systems(
                OnExit(FrontEndOpen::Open),
                (forget_the_model, theme::forget_the_window),
            )
            .add_systems(
                Update,
                (
                    theme::fit_the_window,
                    input::read_input,
                    read_what_to_show,
                    apply_actions,
                    screens::redraw,
                    screens::scroll_options,
                    theme::show_hovering,
                    showroom::turn,
                )
                    .chain()
                    .in_set(UiSystems)
                    .run_if(in_state(FrontEndOpen::Open)),
            )
            // Once the line in hand has been laid out, and before it is drawn.
            .add_systems(
                PostUpdate,
                screens::keep_in_hand_in_view
                    .after(bevy::ui::UiSystems::Layout)
                    .run_if(in_state(FrontEndOpen::Open)),
            )
            // In `PostUpdate`, after whatever changed `LoadingScreen` in `Update`, so that the
            // change is drawn in that frame: the next may be the one a race is built in.
            .add_systems(
                PostUpdate,
                loading::redraw.before(bevy::ui::UiSystems::Prepare),
            );
    }
}

/// Everything the module does in `Update`. Write `PlayerDid` before it and read the
/// module's messages after it, and nothing waits a frame.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct UiSystems;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FrontEndOpen {
    #[default]
    Closed,
    Open,
}

/// Something to choose: a truck or a track.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    /// Means something to the game, and nothing to this module.
    pub id: String,
    pub name: String,
    /// A second line, shown smaller: where it came from, or why it can't be used.
    pub detail: String,
    /// False for something that is listed and cannot be chosen, which is drawn greyed out.
    pub available: bool,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct Catalogue {
    pub trucks: Vec<Entry>,
    pub tracks: Vec<Entry>,
}

/// One of the garage's dials.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dial {
    pub id: String,
    pub label: String,
    /// What its two ends are called.
    pub low_label: String,
    pub high_label: String,
    /// How many places it can stand at.
    pub steps: usize,
    /// Which one it stands at, from 0.
    pub step: usize,
}

impl Dial {
    /// Where it stands, from -1 at its low end to 1 at its high end.
    pub fn value(&self) -> f32 {
        if self.steps < 2 {
            return 0.0;
        }
        self.step as f32 / (self.steps - 1) as f32 * 2.0 - 1.0
    }

    /// Stands it at the step nearest to `value`, which runs from -1 to 1.
    pub fn set_value(&mut self, value: f32) {
        let top = self.steps.saturating_sub(1) as f32;
        self.step = ((value.clamp(-1.0, 1.0) + 1.0) / 2.0 * top).round() as usize;
    }
}

#[derive(Resource, Clone, Debug, Default)]
pub struct Dials(pub Vec<Dial>);

/// One line of the options screen: a few values to choose among, by name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Setting {
    /// Means something to the game, and nothing to this module.
    pub id: String,
    /// The heading it is shown under. Settings with the same one are shown together, in
    /// the order they come in.
    pub section: String,
    pub label: String,
    /// What each value is called, in order.
    pub values: Vec<String>,
    /// Which value is chosen, from 0.
    pub chosen: usize,
    /// Which value "restore defaults" chooses.
    pub default: usize,
    /// What it does, shown while it is highlighted.
    pub detail: String,
    /// For a key binding, the key each of `values` stands for. Such a line shows only the
    /// key it is bound to, and Enter or a click waits for the next key pressed, which Esc
    /// stops. Empty for any other line.
    pub keys: Vec<KeyCode>,
    /// A line that shows its one value, and that Enter or a click opens: the module says so
    /// with `SettingOpened`, and doesn't know what it opens.
    pub opens: bool,
    /// Shown on the ADVANCED screen rather than the options screen.
    pub advanced: bool,
    /// A line that stands for other settings, drawn as a slider: choosing value `i` sets
    /// each other setting to the value `levels[i]` names for it, in order (`None` leaves
    /// it), as a `Preset` does. The module keeps it at the first level that the others
    /// match, and at its last value, which is past the levels and can't be chosen, where
    /// they match none. Empty for any other line.
    pub levels: Vec<Vec<Option<usize>>>,
}

impl Setting {
    /// The name of the value that is chosen.
    pub fn chosen_value(&self) -> &str {
        self.values.get(self.chosen).map_or("", String::as_str)
    }
}

#[derive(Resource, Clone, Debug, Default)]
pub struct Settings(pub Vec<Setting>);

/// A set of values for the settings, chosen all at once by a button beside "restore
/// defaults" on the options screen: for each `Setting`, in order, which value it chooses,
/// or `None` to leave that setting as it is. Settings past the end are left as well.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preset {
    pub label: String,
    pub values: Vec<Option<usize>>,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct Presets(pub Vec<Preset>);

/// A folder being browsed: what is shown, and what can be done there. The module knows
/// nothing of files; the game reads the folder and fills this in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Folder {
    /// Over everything: what is being looked for.
    pub heading: String,
    /// Where it is.
    pub path: String,
    /// What is in it that matters, such as how many of what is looked for.
    pub found: String,
    /// How to tell the right folder, shown under the list.
    pub about: String,
    /// Its subfolders, which can be gone into.
    pub folders: Vec<Entry>,
    /// Which of `folders` is highlighted, from 0. The module writes it as the player moves.
    pub highlighted: usize,
    pub can_choose: bool,
    pub can_go_up: bool,
    /// What the button that stops browsing says, such as "CANCEL".
    pub leave: String,
}

/// The folder being browsed, which takes the place of the screens. `None` for none.
#[derive(Resource, Clone, Debug, Default)]
pub struct FolderBrowser(pub Option<Folder>);

/// Indices into the `Catalogue`, the number of laps, and how many trucks the computer drives.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Choices {
    pub truck: usize,
    pub track: usize,
    pub laps: u32,
    pub opponents: u32,
}

impl Default for Choices {
    fn default() -> Self {
        Self {
            truck: 0,
            track: 0,
            laps: 3,
            opponents: 3,
        }
    }
}

/// A picture of the highlighted track, for the race screen. `None` shows none.
#[derive(Resource, Default)]
pub struct TrackPreview(pub Option<Handle<Image>>);

/// Something being got ready, for the loading screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Loading {
    /// What is being got ready, shown large.
    pub title: String,
    /// A line under it, such as what goes with it.
    pub detail: String,
    /// What is being done now, under the bar.
    pub step: String,
}

/// The loading screen, which covers everything while it holds a view, whether the front end
/// is open or not. `None` shows none.
#[derive(Resource, Clone, Debug, Default)]
pub struct LoadingScreen(pub Option<Loading>);

/// Something the player did. The module's own input writes these, and anything else may.
#[derive(Message, Clone, Copy, Debug)]
pub struct PlayerDid(pub Action);

/// The truck at this index of the `Catalogue` is now the one highlighted. Also sent for
/// the one that is highlighted when the front end opens.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct TruckHighlighted(pub usize);

/// The same, for a track.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct TrackHighlighted(pub usize);

/// GO was pressed, with a truck and a track that can be raced. `Choices` says which.
#[derive(Message, Clone, Copy, Debug)]
pub struct GoPressed;

/// EXIT was pressed. The module does not close anything itself.
#[derive(Message, Clone, Copy, Debug)]
pub struct ExitPressed;

/// The setting at this index of `Settings`, one that `opens`, was opened.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct SettingOpened(pub usize);

/// The player went into the subfolder at this index of `Folder::folders`.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct FolderEntered(pub usize);

/// The player went up out of the folder being browsed.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct FolderUp;

/// The player chose the folder being browsed.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct FolderChosen;

/// The player stopped browsing, and chose nothing.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct FolderLeft;

/// The state machine behind the screens. Absent while the front end is closed.
#[derive(Resource)]
struct Model(FrontEnd);

fn forget_the_model(mut commands: Commands) {
    commands.remove_resource::<Model>();
}

#[expect(
    clippy::too_many_arguments,
    reason = "it reads everything the module is handed"
)]
fn read_what_to_show(
    mut commands: Commands,
    catalogue: Res<Catalogue>,
    choices: Res<Choices>,
    dials: Res<Dials>,
    settings: Res<Settings>,
    presets: Res<Presets>,
    browser: Res<FolderBrowser>,
    theme: Res<Theme>,
    model: Option<Res<Model>>,
    mut trucks: MessageWriter<TruckHighlighted>,
    mut tracks: MessageWriter<TrackHighlighted>,
) {
    if model.is_some() && !catalogue.is_changed() && !browser.is_changed() {
        return;
    }
    let list = |entries: &[Entry], chosen: usize| {
        List::new(
            entries.iter().map(|entry| entry.available).collect(),
            chosen,
        )
    };
    // A list that doesn't fit in view can be searched by name.
    let searched = |entries: &[Entry], chosen: usize| {
        let names: Vec<String> = entries.iter().map(|entry| entry.name.clone()).collect();
        list(entries, chosen).searched_by(&names, entries.len() > theme.list_rows)
    };
    let mut front_end = FrontEnd::new(
        searched(&catalogue.trucks, choices.truck),
        searched(&catalogue.tracks, choices.track),
        choices.laps,
        choices.opponents,
        dials
            .0
            .iter()
            .map(|dial| DialSteps {
                steps: dial.steps,
                step: dial.step,
            })
            .collect(),
        settings
            .0
            .iter()
            .map(|setting| SettingValues {
                values: setting.values.len(),
                chosen: setting.chosen,
                default: setting.default,
                listens: !setting.keys.is_empty(),
                opens: setting.opens,
                advanced: setting.advanced,
                levels: setting.levels.clone(),
            })
            .collect(),
    );
    front_end.presets = presets
        .0
        .iter()
        .map(|preset| preset.values.clone())
        .collect();
    front_end.browsing = browser.0.as_ref().map(|folder| Browsing {
        folders: list(&folder.folders, folder.highlighted),
        can_choose: folder.can_choose,
        can_go_up: folder.can_go_up,
    });
    if let Some(model) = model {
        front_end.screen = model.0.screen;
        front_end.setting_in_hand = model.0.setting_in_hand;
        // What was searched for, while the list still has a search field.
        for (list, old) in [
            (&mut front_end.trucks, &model.0.trucks),
            (&mut front_end.tracks, &model.0.tracks),
        ] {
            if list.searchable {
                list.search = old.search.clone();
            }
        }
    }
    // Whoever shows the truck and the track wants to know which, from the start.
    if !front_end.trucks.is_empty() {
        trucks.write(TruckHighlighted(front_end.trucks.highlighted));
    }
    if !front_end.tracks.is_empty() {
        tracks.write(TrackHighlighted(front_end.tracks.highlighted));
    }
    commands.insert_resource(Model(front_end));
}

#[expect(
    clippy::too_many_arguments,
    reason = "it writes everything the module hands out"
)]
fn apply_actions(
    mut did: MessageReader<PlayerDid>,
    model: Option<ResMut<Model>>,
    mut choices: ResMut<Choices>,
    mut dials: ResMut<Dials>,
    mut settings: ResMut<Settings>,
    mut trucks: MessageWriter<TruckHighlighted>,
    mut tracks: MessageWriter<TrackHighlighted>,
    mut go: MessageWriter<GoPressed>,
    mut browser: ResMut<FolderBrowser>,
    mut browsed: Browsed,
) {
    // Nothing is done while something is being got ready, and nothing waits for it to end.
    if browsed.loading.0.is_some() {
        did.clear();
        return;
    }
    // The model arrives with the next frame's commands, and what was done waits for it.
    let Some(mut model) = model else {
        return;
    };
    for PlayerDid(action) in did.read() {
        let before = (model.0.dial_in_hand, model.0.setting_in_hand);
        let happened = model.bypass_change_detection().0.apply(*action);
        if !happened.is_empty() || (model.0.dial_in_hand, model.0.setting_in_hand) != before {
            model.set_changed();
        }
        for happened in happened {
            match happened {
                Happened::ScreenChanged(_) | Happened::SearchChanged => {}
                Happened::TruckHighlighted(index) => {
                    choices.truck = index;
                    trucks.write(TruckHighlighted(index));
                }
                Happened::TrackHighlighted(index) => {
                    choices.track = index;
                    tracks.write(TrackHighlighted(index));
                }
                Happened::LapsChanged(laps) => choices.laps = laps,
                Happened::OpponentsChanged(opponents) => choices.opponents = opponents,
                Happened::DialChanged(index, step) => {
                    if let Some(dial) = dials.0.get_mut(index) {
                        dial.step = step;
                    }
                }
                Happened::SettingChanged(index, chosen) => {
                    if let Some(setting) = settings.0.get_mut(index) {
                        setting.chosen = chosen;
                    }
                }
                Happened::Go => {
                    go.write(GoPressed);
                }
                Happened::Exit => {
                    browsed.exit.write(ExitPressed);
                }
                Happened::SettingOpened(index) => {
                    browsed.opened.write(SettingOpened(index));
                }
                // Kept where the game can see it, without the game hearing of every move.
                Happened::FolderHighlighted(index) => {
                    if let Some(folder) = &mut browser.bypass_change_detection().0 {
                        folder.highlighted = index;
                    }
                }
                Happened::FolderEntered(index) => {
                    browsed.entered.write(FolderEntered(index));
                }
                Happened::FolderUp => {
                    browsed.up.write(FolderUp);
                }
                Happened::FolderChosen => {
                    browsed.chosen.write(FolderChosen);
                }
                Happened::FolderLeft => {
                    browsed.left.write(FolderLeft);
                }
            }
        }
    }
}

/// What `apply_actions` says about opening a line, browsing a folder, and exiting, and the
/// loading screen, which stops it.
#[derive(bevy::ecs::system::SystemParam)]
struct Browsed<'w> {
    loading: Res<'w, LoadingScreen>,
    opened: MessageWriter<'w, SettingOpened>,
    entered: MessageWriter<'w, FolderEntered>,
    up: MessageWriter<'w, FolderUp>,
    chosen: MessageWriter<'w, FolderChosen>,
    left: MessageWriter<'w, FolderLeft>,
    exit: MessageWriter<'w, ExitPressed>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dial_stands_between_minus_one_and_one() {
        let mut dial = Dial {
            steps: 5,
            step: 2,
            ..default()
        };
        assert_eq!(dial.value(), 0.0);
        dial.step = 0;
        assert_eq!(dial.value(), -1.0);
        dial.step = 4;
        assert_eq!(dial.value(), 1.0);

        dial.set_value(0.45);
        assert_eq!(dial.step, 3);
        dial.set_value(-7.0);
        assert_eq!(dial.step, 0);
        dial.set_value(0.0);
        assert_eq!(dial.step, 2);
    }
}
