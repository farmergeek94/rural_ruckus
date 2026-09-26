//! What the front end is showing, as a pure state machine, in the manner of the game's
//! `race/progress.rs`: an `Action` goes in, and out comes a list of what happened. No Bevy
//! types, so every rule is a unit test.

/// The most laps a race can be set to.
pub const MOST_LAPS: u32 = 99;
/// The most trucks the computer can drive in a race. With the player's that is eight, which
/// is how many places a Monster Truck Madness 2 starting grid has.
pub const MOST_OPPONENTS: u32 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Screen {
    Truck,
    Race,
    Garage,
    Options,
    /// The key bindings: the settings that wait for a key.
    Controls,
}

impl Screen {
    /// In the order of their tabs.
    pub const ALL: [Screen; 5] = [
        Screen::Truck,
        Screen::Race,
        Screen::Garage,
        Screen::Options,
        Screen::Controls,
    ];
    /// The screens that lead to GO, in order, which `Accept` walks. The options are aside
    /// from them.
    pub const TO_GO: [Screen; 3] = [Screen::Truck, Screen::Race, Screen::Garage];

    pub fn title(self) -> &'static str {
        match self {
            Screen::Truck => "TRUCK",
            Screen::Race => "RACE",
            Screen::Garage => "GARAGE",
            Screen::Options => "OPTIONS",
            Screen::Controls => "CONTROLS",
        }
    }

    /// Whether the settings are on this screen: the key bindings on the controls screen,
    /// and the rest on the options screen.
    pub fn shows_settings(self) -> bool {
        matches!(self, Screen::Options | Screen::Controls)
    }
}

/// Something the player did, whatever they did it with. `Up`, `Down`, `Less` and `More`
/// mean whatever the screen that is up makes of them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    NextScreen,
    PreviousScreen,
    Show(Screen),
    Up,
    Down,
    Less,
    More,
    PickTruck(usize),
    PickTrack(usize),
    /// One lap fewer or more, from whichever screen.
    FewerLaps,
    MoreLaps,
    /// One computer-driven truck fewer or more, from whichever screen.
    FewerOpponents,
    MoreOpponents,
    /// Turn dial number `0` to step `1`.
    SetDial(usize, usize),
    /// Set setting number `0` to its value number `1`.
    SetSetting(usize, usize),
    /// Every setting back to its default.
    RestoreDefaults,
    /// Setting number `0`, a key binding, waits for a key.
    Listen(usize),
    /// Stop waiting for a key, and keep the one there was.
    StopListening,
    /// Setting number `0`, one that opens something, is opened.
    OpenSetting(usize),
    /// In a folder being browsed: go into folder number `0` of the list.
    PickFolder(usize),
    /// In a folder being browsed: go up to the folder it is in.
    FolderUp,
    /// In a folder being browsed: this is the one.
    ChooseFolder,
    /// Stop browsing, and choose nothing.
    LeaveFolder,
    /// A gamepad's "yes": on to the next screen, and GO from the last one. On the options
    /// screen, the next value of the highlighted setting, round and round.
    Accept,
    Go,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Happened {
    ScreenChanged(Screen),
    TruckHighlighted(usize),
    TrackHighlighted(usize),
    LapsChanged(u32),
    OpponentsChanged(u32),
    /// Dial number `0` now stands at step `1`.
    DialChanged(usize, usize),
    /// Setting number `0` now has its value number `1`.
    SettingChanged(usize, usize),
    /// Setting number `0`, one that opens something, was opened.
    SettingOpened(usize),
    FolderHighlighted(usize),
    FolderEntered(usize),
    FolderUp,
    FolderChosen,
    FolderLeft,
    Go,
}

/// A list the player walks up and down. `available[i]` is false for an entry that is
/// shown but cannot be chosen, such as an archive that could not be read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct List {
    pub highlighted: usize,
    pub available: Vec<bool>,
}

impl List {
    pub fn new(available: Vec<bool>, highlighted: usize) -> Self {
        let highlighted = if highlighted < available.len() {
            highlighted
        } else {
            0
        };
        Self {
            highlighted,
            available,
        }
    }

    pub fn len(&self) -> usize {
        self.available.len()
    }

    pub fn is_empty(&self) -> bool {
        self.available.is_empty()
    }

    /// Whether what is highlighted can be raced.
    pub fn chosen_is_available(&self) -> bool {
        self.available.get(self.highlighted) == Some(&true)
    }

    /// Moves by one, wrapping round at either end. Says whether anything moved.
    fn step(&mut self, down: bool) -> bool {
        let len = self.len();
        if len < 2 {
            return false;
        }
        self.highlighted = (self.highlighted + if down { 1 } else { len - 1 }) % len;
        true
    }

    fn pick(&mut self, index: usize) -> bool {
        if index >= self.len() || index == self.highlighted {
            return false;
        }
        self.highlighted = index;
        true
    }
}

/// A dial as the model sees it: `steps` places to stand, and which one it is at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DialSteps {
    pub steps: usize,
    pub step: usize,
}

/// A setting as the model sees it: how many values it has, which one is chosen, and which
/// one is its default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettingValues {
    pub values: usize,
    pub chosen: usize,
    pub default: usize,
    /// A key binding: rather than stepping through its values, it waits for a key.
    pub listens: bool,
    /// Rather than stepping through its values, it opens something.
    pub opens: bool,
}

/// A folder being browsed, as the model sees it: its subfolders, and what can be done in it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Browsing {
    pub folders: List,
    pub can_choose: bool,
    pub can_go_up: bool,
}

impl Browsing {
    /// While a folder is browsed, only what browsing does happens.
    fn apply(&mut self, action: Action) -> Vec<Happened> {
        let happened = match action {
            Action::Up | Action::Down => self
                .folders
                .step(action == Action::Down)
                .then_some(Happened::FolderHighlighted(self.folders.highlighted)),
            Action::More | Action::Accept | Action::Go => self
                .folders
                .chosen_is_available()
                .then_some(Happened::FolderEntered(self.folders.highlighted)),
            // A click goes in at once.
            Action::PickFolder(index) => {
                let moved = self.folders.pick(index);
                if self.folders.highlighted != index {
                    None
                } else if self.folders.chosen_is_available() {
                    Some(Happened::FolderEntered(index))
                } else {
                    moved.then_some(Happened::FolderHighlighted(index))
                }
            }
            Action::Less | Action::FolderUp => self.can_go_up.then_some(Happened::FolderUp),
            Action::ChooseFolder => self.can_choose.then_some(Happened::FolderChosen),
            Action::LeaveFolder => Some(Happened::FolderLeft),
            _ => None,
        };
        happened.into_iter().collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrontEnd {
    pub screen: Screen,
    pub trucks: List,
    pub tracks: List,
    pub laps: u32,
    /// How many trucks the computer drives. None is a race against the clock.
    pub opponents: u32,
    pub dials: Vec<DialSteps>,
    /// Which dial `Less` and `More` turn, on the garage screen.
    pub dial_in_hand: usize,
    pub settings: Vec<SettingValues>,
    /// Which setting `Less` and `More` change, on the options screen.
    pub setting_in_hand: usize,
    /// Whether the key binding in hand is waiting for a key.
    pub listening: bool,
    /// A folder being browsed, shown in place of the screen that is up. `None` for none.
    pub browsing: Option<Browsing>,
}

impl FrontEnd {
    pub fn new(
        trucks: List,
        tracks: List,
        laps: u32,
        opponents: u32,
        dials: Vec<DialSteps>,
        settings: Vec<SettingValues>,
    ) -> Self {
        Self {
            screen: Screen::Truck,
            trucks,
            tracks,
            laps: laps.clamp(1, MOST_LAPS),
            opponents: opponents.min(MOST_OPPONENTS),
            dials: dials
                .into_iter()
                .map(|dial| DialSteps {
                    steps: dial.steps.max(1),
                    step: dial.step.min(dial.steps.max(1) - 1),
                })
                .collect(),
            dial_in_hand: 0,
            settings: settings
                .into_iter()
                .map(|setting| {
                    let values = setting.values.max(1);
                    SettingValues {
                        values,
                        chosen: setting.chosen.min(values - 1),
                        default: setting.default.min(values - 1),
                        listens: setting.listens,
                        opens: setting.opens,
                    }
                })
                .collect(),
            setting_in_hand: 0,
            listening: false,
            browsing: None,
        }
    }

    /// GO is lit once there is a truck and a track that can be raced.
    pub fn can_go(&self) -> bool {
        self.trucks.chosen_is_available() && self.tracks.chosen_is_available()
    }

    pub fn apply(&mut self, action: Action) -> Vec<Happened> {
        if let Some(browsing) = &mut self.browsing {
            return browsing.apply(action);
        }
        let mut happened = Vec::new();
        match action {
            Action::NextScreen | Action::PreviousScreen => {
                let at = Screen::ALL
                    .iter()
                    .position(|&screen| screen == self.screen)
                    .unwrap_or(0);
                let step = if action == Action::NextScreen {
                    1
                } else {
                    Screen::ALL.len() - 1
                };
                self.show(Screen::ALL[(at + step) % Screen::ALL.len()], &mut happened);
            }
            Action::Show(screen) => self.show(screen, &mut happened),
            Action::Up | Action::Down => {
                let down = action == Action::Down;
                match self.screen {
                    Screen::Truck => {
                        if self.trucks.step(down) {
                            happened.push(Happened::TruckHighlighted(self.trucks.highlighted));
                        }
                    }
                    Screen::Race => {
                        if self.tracks.step(down) {
                            happened.push(Happened::TrackHighlighted(self.tracks.highlighted));
                        }
                    }
                    // Dials don't wrap: there are few of them and they are all in view.
                    Screen::Garage if down => {
                        self.dial_in_hand =
                            (self.dial_in_hand + 1).min(self.dials.len().saturating_sub(1));
                    }
                    Screen::Garage => self.dial_in_hand = self.dial_in_hand.saturating_sub(1),
                    // Nor do settings, for the same reason.
                    Screen::Options | Screen::Controls => {
                        if let Some(next) = self.next_on_screen(down) {
                            self.setting_in_hand = next;
                        }
                    }
                }
            }
            Action::Less | Action::More => {
                let more = action == Action::More;
                match self.screen {
                    Screen::Truck => {}
                    Screen::Race => self.change_laps(more, &mut happened),
                    Screen::Garage => {
                        if let Some(dial) = self.dials.get(self.dial_in_hand) {
                            let step = if more {
                                dial.step + 1
                            } else {
                                dial.step.saturating_sub(1)
                            };
                            self.set_dial(self.dial_in_hand, step, &mut happened);
                        }
                    }
                    Screen::Options => {
                        if let Some(setting) = self.settings.get(self.setting_in_hand) {
                            let chosen = step_within(setting.chosen, more, setting.values);
                            self.set_setting(self.setting_in_hand, chosen, &mut happened);
                        }
                    }
                    // A key binding waits for a key instead.
                    Screen::Controls => {}
                }
            }
            Action::PickTruck(index) => {
                if self.trucks.pick(index) {
                    happened.push(Happened::TruckHighlighted(index));
                }
            }
            Action::PickTrack(index) => {
                if self.tracks.pick(index) {
                    happened.push(Happened::TrackHighlighted(index));
                }
            }
            Action::FewerLaps => self.change_laps(false, &mut happened),
            Action::MoreLaps => self.change_laps(true, &mut happened),
            Action::FewerOpponents => self.change_opponents(false, &mut happened),
            Action::MoreOpponents => self.change_opponents(true, &mut happened),
            Action::SetDial(dial, step) => {
                if dial < self.dials.len() {
                    self.dial_in_hand = dial;
                }
                self.set_dial(dial, step, &mut happened);
            }
            Action::SetSetting(setting, chosen) => {
                if setting < self.settings.len() {
                    self.setting_in_hand = setting;
                }
                self.listening = false;
                self.set_setting(setting, chosen, &mut happened);
            }
            Action::Listen(setting) => {
                if self
                    .settings
                    .get(setting)
                    .is_some_and(|setting| setting.listens)
                {
                    self.setting_in_hand = setting;
                    self.listening = true;
                }
            }
            Action::StopListening => self.listening = false,
            Action::OpenSetting(setting) => self.open_setting(setting, &mut happened),
            // Browsing, which is dealt with above.
            Action::PickFolder(_)
            | Action::FolderUp
            | Action::ChooseFolder
            | Action::LeaveFolder => {}
            // On a key binding, Enter and a gamepad's "yes" wait for a key.
            Action::Accept | Action::Go
                if self.screen == Screen::Controls
                    && self
                        .settings
                        .get(self.setting_in_hand)
                        .is_some_and(|setting| setting.listens) =>
            {
                self.listening = true;
            }
            // Those on the screen that is up.
            Action::RestoreDefaults => {
                for index in 0..self.settings.len() {
                    if self.on_screen(index) {
                        self.set_setting(index, self.settings[index].default, &mut happened);
                    }
                }
            }
            // On a line that opens something, Enter and a gamepad's "yes" open it.
            Action::Accept | Action::Go
                if self.screen == Screen::Options
                    && self
                        .settings
                        .get(self.setting_in_hand)
                        .is_some_and(|setting| setting.opens) =>
            {
                self.open_setting(self.setting_in_hand, &mut happened);
            }
            Action::Accept if self.screen == Screen::Options => {
                if let Some(setting) = self.settings.get(self.setting_in_hand) {
                    let chosen = (setting.chosen + 1) % setting.values;
                    self.set_setting(self.setting_in_hand, chosen, &mut happened);
                }
            }
            Action::Accept if self.screen != Screen::TO_GO[Screen::TO_GO.len() - 1] => {
                let at = Screen::TO_GO
                    .iter()
                    .position(|&screen| screen == self.screen)
                    .unwrap_or(0);
                self.show(Screen::TO_GO[at + 1], &mut happened);
            }
            Action::Accept | Action::Go => {
                if self.can_go() {
                    happened.push(Happened::Go);
                }
            }
        }
        happened
    }

    fn show(&mut self, screen: Screen, happened: &mut Vec<Happened>) {
        if screen != self.screen {
            self.screen = screen;
            self.listening = false;
            // The line in hand is one on the screen, if it has any.
            if screen.shows_settings()
                && !self.on_screen(self.setting_in_hand)
                && let Some(first) = (0..self.settings.len()).find(|&index| self.on_screen(index))
            {
                self.setting_in_hand = first;
            }
            happened.push(Happened::ScreenChanged(screen));
        }
    }

    /// Whether setting `index` is on the screen that is up.
    pub fn on_screen(&self, index: usize) -> bool {
        self.settings.get(index).is_some_and(|setting| {
            if setting.listens {
                self.screen == Screen::Controls
            } else {
                self.screen == Screen::Options
            }
        })
    }

    /// The setting after the one in hand, or before it, on the same screen. `None` at the
    /// end: they don't wrap.
    fn next_on_screen(&self, down: bool) -> Option<usize> {
        let at = self.setting_in_hand;
        if down {
            (at + 1..self.settings.len()).find(|&index| self.on_screen(index))
        } else {
            (0..at).rev().find(|&index| self.on_screen(index))
        }
    }

    fn open_setting(&mut self, index: usize, happened: &mut Vec<Happened>) {
        if self
            .settings
            .get(index)
            .is_some_and(|setting| setting.opens)
        {
            self.setting_in_hand = index;
            happened.push(Happened::SettingOpened(index));
        }
    }

    fn change_laps(&mut self, more: bool, happened: &mut Vec<Happened>) {
        let laps = if more { self.laps + 1 } else { self.laps - 1 }.clamp(1, MOST_LAPS);
        if laps != self.laps {
            self.laps = laps;
            happened.push(Happened::LapsChanged(laps));
        }
    }

    fn change_opponents(&mut self, more: bool, happened: &mut Vec<Happened>) {
        let opponents = if more {
            (self.opponents + 1).min(MOST_OPPONENTS)
        } else {
            self.opponents.saturating_sub(1)
        };
        if opponents != self.opponents {
            self.opponents = opponents;
            happened.push(Happened::OpponentsChanged(opponents));
        }
    }

    /// A dial stops at its ends.
    fn set_dial(&mut self, index: usize, step: usize, happened: &mut Vec<Happened>) {
        let Some(dial) = self.dials.get_mut(index) else {
            return;
        };
        let step = step.min(dial.steps - 1);
        if step != dial.step {
            dial.step = step;
            happened.push(Happened::DialChanged(index, step));
        }
    }
}

/// One step up or down from `at`, stopping at 0 and at `len - 1`.
fn step_within(at: usize, up: bool, len: usize) -> usize {
    if up {
        (at + 1).min(len.saturating_sub(1))
    } else {
        at.saturating_sub(1)
    }
}

impl FrontEnd {
    /// A setting stops at its ends.
    fn set_setting(&mut self, index: usize, chosen: usize, happened: &mut Vec<Happened>) {
        let Some(setting) = self.settings.get_mut(index) else {
            return;
        };
        let chosen = chosen.min(setting.values - 1);
        if chosen != setting.chosen {
            setting.chosen = chosen;
            happened.push(Happened::SettingChanged(index, chosen));
        }
    }
}

/// Where to break groups of `heights` rows into two columns, so that the taller column is
/// as short as it can be: the groups before the index returned go in the first column.
/// Groups keep their order, so that reading down one column and then the next is the order
/// they are walked in.
pub fn split_in_two(heights: &[usize]) -> usize {
    let total: usize = heights.iter().sum();
    let mut first = 0;
    let mut best = (total, 0);
    for (index, height) in heights.iter().enumerate() {
        first += height;
        let taller = first.max(total - first);
        // A tie goes to the first column, so that one group is never alone on the right.
        if taller <= best.0 {
            best = (taller, index + 1);
        }
    }
    best.1
}

/// Which rows of a list of `len` to show when there is room for `rows`, so that the
/// highlighted one is in view and, where it can be, in the middle.
pub fn visible_rows(highlighted: usize, len: usize, rows: usize) -> std::ops::Range<usize> {
    if len <= rows {
        return 0..len;
    }
    let first = highlighted.saturating_sub(rows / 2).min(len - rows);
    first..first + rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn front_end(trucks: usize, tracks: usize) -> FrontEnd {
        FrontEnd::new(
            List::new(vec![true; trucks], 0),
            List::new(vec![true; tracks], 0),
            3,
            2,
            vec![DialSteps { steps: 5, step: 2 }; 3],
            vec![
                SettingValues {
                    values: 2,
                    chosen: 1,
                    default: 1,
                    listens: false,
                    opens: false,
                },
                SettingValues {
                    values: 4,
                    chosen: 3,
                    default: 3,
                    listens: false,
                    opens: false,
                },
            ],
        )
    }

    #[test]
    fn tabs_cycle_both_ways() {
        let mut model = front_end(2, 2);
        assert_eq!(
            model.apply(Action::NextScreen),
            [Happened::ScreenChanged(Screen::Race)]
        );
        model.apply(Action::NextScreen);
        model.apply(Action::NextScreen);
        model.apply(Action::NextScreen);
        assert_eq!(
            model.apply(Action::NextScreen),
            [Happened::ScreenChanged(Screen::Truck)]
        );
        assert_eq!(
            model.apply(Action::PreviousScreen),
            [Happened::ScreenChanged(Screen::Controls)]
        );
        // Showing the screen that is already up is nothing happening.
        assert_eq!(model.apply(Action::Show(Screen::Controls)), []);
    }

    #[test]
    fn a_list_wraps_and_reports_what_is_highlighted() {
        let mut model = front_end(3, 2);
        assert_eq!(model.apply(Action::Up), [Happened::TruckHighlighted(2)]);
        assert_eq!(model.apply(Action::Down), [Happened::TruckHighlighted(0)]);
        assert_eq!(
            model.apply(Action::PickTruck(1)),
            [Happened::TruckHighlighted(1)]
        );
        assert_eq!(model.apply(Action::PickTruck(1)), []);
        assert_eq!(model.apply(Action::PickTruck(9)), []);

        // The same keys walk the tracks once the race screen is up.
        model.apply(Action::Show(Screen::Race));
        assert_eq!(model.apply(Action::Down), [Happened::TrackHighlighted(1)]);
        assert_eq!(model.trucks.highlighted, 1);

        // A list of one has nowhere to go.
        let mut lonely = front_end(1, 1);
        assert_eq!(lonely.apply(Action::Down), []);
    }

    #[test]
    fn laps_stay_in_range() {
        let mut model = front_end(1, 1);
        model.apply(Action::Show(Screen::Race));
        assert_eq!(model.apply(Action::More), [Happened::LapsChanged(4)]);
        for _ in 0..200 {
            model.apply(Action::MoreLaps);
        }
        assert_eq!(model.laps, MOST_LAPS);
        assert_eq!(model.apply(Action::More), []);
        for _ in 0..200 {
            model.apply(Action::Less);
        }
        assert_eq!(model.laps, 1);
        assert_eq!(model.apply(Action::FewerLaps), []);

        // And whatever it is handed to begin with.
        let wild = FrontEnd::new(
            List::default(),
            List::default(),
            0,
            0,
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(wild.laps, 1);
    }

    #[test]
    fn opponents_stay_in_range_from_whichever_screen() {
        let mut model = front_end(1, 1);
        assert_eq!(
            model.apply(Action::MoreOpponents),
            [Happened::OpponentsChanged(3)]
        );
        for _ in 0..20 {
            model.apply(Action::MoreOpponents);
        }
        assert_eq!(model.opponents, MOST_OPPONENTS);
        assert_eq!(model.apply(Action::MoreOpponents), []);
        for _ in 0..20 {
            model.apply(Action::FewerOpponents);
        }
        // None at all is a race against the clock.
        assert_eq!(model.opponents, 0);
        assert_eq!(model.apply(Action::FewerOpponents), []);

        let wild = FrontEnd::new(
            List::default(),
            List::default(),
            3,
            500,
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(wild.opponents, MOST_OPPONENTS);
    }

    #[test]
    fn a_dial_stops_at_its_ends() {
        let mut model = front_end(1, 1);
        model.apply(Action::Show(Screen::Garage));
        model.apply(Action::Down);
        assert_eq!(model.dial_in_hand, 1);
        assert_eq!(model.apply(Action::More), [Happened::DialChanged(1, 3)]);
        assert_eq!(model.apply(Action::More), [Happened::DialChanged(1, 4)]);
        assert_eq!(model.apply(Action::More), []);
        assert_eq!(
            model.apply(Action::SetDial(0, 0)),
            [Happened::DialChanged(0, 0)]
        );
        assert_eq!(model.dial_in_hand, 0);
        assert_eq!(model.apply(Action::Less), []);
        assert_eq!(
            model.apply(Action::SetDial(2, 40)),
            [Happened::DialChanged(2, 4)]
        );
        assert_eq!(model.apply(Action::SetDial(7, 1)), []);

        // Moving between dials stops at the first and the last.
        for _ in 0..9 {
            model.apply(Action::Down);
        }
        assert_eq!(model.dial_in_hand, 2);
        for _ in 0..9 {
            model.apply(Action::Up);
        }
        assert_eq!(model.dial_in_hand, 0);
    }

    #[test]
    fn go_needs_a_truck_and_a_track_that_can_be_raced() {
        assert_eq!(front_end(1, 1).apply(Action::Go), [Happened::Go]);
        assert_eq!(front_end(1, 0).apply(Action::Go), []);
        assert_eq!(front_end(0, 1).apply(Action::Go), []);

        // An archive that couldn't be read is listed, and can't be raced.
        let mut model = FrontEnd::new(
            List::new(vec![true], 0),
            List::new(vec![true, false], 1),
            3,
            0,
            Vec::new(),
            Vec::new(),
        );
        assert!(!model.can_go());
        assert_eq!(model.apply(Action::Go), []);
        model.apply(Action::PickTrack(0));
        assert_eq!(model.apply(Action::Go), [Happened::Go]);
    }

    #[test]
    fn accepting_walks_the_screens_and_then_goes() {
        let mut model = front_end(1, 1);
        assert_eq!(
            model.apply(Action::Accept),
            [Happened::ScreenChanged(Screen::Race)]
        );
        assert_eq!(
            model.apply(Action::Accept),
            [Happened::ScreenChanged(Screen::Garage)]
        );
        assert_eq!(model.apply(Action::Accept), [Happened::Go]);
    }

    #[test]
    fn a_setting_stops_at_its_ends_and_accept_goes_round() {
        let mut model = front_end(1, 1);
        model.apply(Action::Show(Screen::Options));
        assert_eq!(model.apply(Action::More), []);
        assert_eq!(model.apply(Action::Less), [Happened::SettingChanged(0, 0)]);
        assert_eq!(model.apply(Action::Less), []);
        // Accept is a gamepad's way to flick an on and off setting.
        assert_eq!(
            model.apply(Action::Accept),
            [Happened::SettingChanged(0, 1)]
        );
        assert_eq!(
            model.apply(Action::Accept),
            [Happened::SettingChanged(0, 0)]
        );

        // Moving between settings stops at the first and the last.
        for _ in 0..9 {
            model.apply(Action::Down);
        }
        assert_eq!(model.setting_in_hand, 1);
        assert_eq!(model.apply(Action::Less), [Happened::SettingChanged(1, 2)]);
        for _ in 0..9 {
            model.apply(Action::Up);
        }
        assert_eq!(model.setting_in_hand, 0);

        assert_eq!(
            model.apply(Action::SetSetting(1, 40)),
            [Happened::SettingChanged(1, 3)]
        );
        assert_eq!(model.setting_in_hand, 1);
        assert_eq!(model.apply(Action::SetSetting(7, 0)), []);
        // The options are not on the way to GO.
        assert_eq!(model.apply(Action::Go), [Happened::Go]);
    }

    #[test]
    fn defaults_are_restored_all_at_once() {
        let mut model = front_end(1, 1);
        // Those of the screen that is up, and none elsewhere.
        assert_eq!(model.apply(Action::RestoreDefaults), []);
        model.apply(Action::Show(Screen::Options));
        model.apply(Action::SetSetting(0, 0));
        model.apply(Action::SetSetting(1, 0));
        assert_eq!(
            model.apply(Action::RestoreDefaults),
            [
                Happened::SettingChanged(0, 1),
                Happened::SettingChanged(1, 3)
            ]
        );
        assert_eq!(model.apply(Action::RestoreDefaults), []);
    }

    #[test]
    fn columns_are_balanced_and_keep_their_order() {
        assert_eq!(split_in_two(&[3, 6, 5]), 2);
        assert_eq!(split_in_two(&[5, 5]), 1);
        assert_eq!(split_in_two(&[4]), 1);
        assert_eq!(split_in_two(&[]), 0);
        assert_eq!(split_in_two(&[1, 1, 1, 1]), 2);
    }

    #[test]
    fn a_remembered_choice_that_has_gone_falls_back_to_the_first() {
        assert_eq!(List::new(vec![true; 3], 7).highlighted, 0);
        assert_eq!(List::new(vec![true; 3], 2).highlighted, 2);
    }

    #[test]
    fn the_highlighted_row_is_always_in_view() {
        assert_eq!(visible_rows(0, 5, 12), 0..5);
        assert_eq!(visible_rows(0, 40, 12), 0..12);
        assert_eq!(visible_rows(20, 40, 12), 14..26);
        assert_eq!(visible_rows(39, 40, 12), 28..40);
        for highlighted in 0..40 {
            let rows = visible_rows(highlighted, 40, 12);
            assert!(rows.contains(&highlighted) && rows.len() == 12);
        }
    }

    #[test]
    fn a_key_binding_waits_for_a_key_until_one_is_set_or_it_is_stopped() {
        let mut model = front_end(1, 1);
        model.settings[1].listens = true;
        model.apply(Action::Show(Screen::Options));

        // Not on a line that is not a key binding.
        model.apply(Action::Listen(0));
        assert!(!model.listening);
        // Enter there is GO, as ever.
        assert_eq!(model.apply(Action::Go), [Happened::Go]);

        // The key binding is on the controls screen, and only it.
        model.apply(Action::Show(Screen::Controls));
        assert_eq!(model.setting_in_hand, 1);
        assert!(!model.on_screen(0));
        model.apply(Action::Up);
        assert_eq!(model.setting_in_hand, 1);

        model.apply(Action::Listen(1));
        assert!(model.listening);
        assert_eq!(model.setting_in_hand, 1);
        assert_eq!(
            model.apply(Action::SetSetting(1, 0)),
            [Happened::SettingChanged(1, 0)]
        );
        assert!(!model.listening);

        // Enter on a key binding waits rather than going.
        assert!(model.apply(Action::Go).is_empty());
        assert!(model.listening);
        model.apply(Action::StopListening);
        assert!(!model.listening);
        assert_eq!(model.settings[1].chosen, 0);
    }

    #[test]
    fn a_line_that_opens_something_says_so_and_changes_nothing() {
        let mut model = front_end(1, 1);
        model.settings[0].opens = true;
        model.apply(Action::Show(Screen::Options));
        assert_eq!(model.apply(Action::Accept), [Happened::SettingOpened(0)]);
        assert_eq!(model.apply(Action::Go), [Happened::SettingOpened(0)]);
        assert_eq!(
            model.apply(Action::OpenSetting(0)),
            [Happened::SettingOpened(0)]
        );
        // Not a line that doesn't open.
        assert_eq!(model.apply(Action::OpenSetting(1)), []);
        // Enter elsewhere on the options screen is GO, as ever.
        model.apply(Action::Down);
        assert_eq!(model.apply(Action::Go), [Happened::Go]);
    }

    #[test]
    fn browsing_a_folder_takes_every_action_until_it_ends() {
        let mut model = front_end(2, 2);
        model.browsing = Some(Browsing {
            folders: List::new(vec![true, false, true], 0),
            can_choose: false,
            can_go_up: true,
        });
        // What the screens do does not happen.
        assert_eq!(model.apply(Action::NextScreen), []);
        assert_eq!(model.apply(Action::MoreOpponents), []);
        assert_eq!(model.screen, Screen::Truck);

        assert_eq!(model.apply(Action::Up), [Happened::FolderHighlighted(2)]);
        assert_eq!(model.apply(Action::Accept), [Happened::FolderEntered(2)]);
        assert_eq!(model.apply(Action::More), [Happened::FolderEntered(2)]);
        // A folder that can't be read is shown, and can't be gone into.
        assert_eq!(
            model.apply(Action::PickFolder(1)),
            [Happened::FolderHighlighted(1)]
        );
        assert_eq!(model.apply(Action::Go), []);
        assert_eq!(
            model.apply(Action::PickFolder(0)),
            [Happened::FolderEntered(0)]
        );
        assert_eq!(model.apply(Action::PickFolder(9)), []);
        assert_eq!(model.apply(Action::Less), [Happened::FolderUp]);
        // Only a folder with something in it can be chosen.
        assert_eq!(model.apply(Action::ChooseFolder), []);
        model.browsing.as_mut().unwrap().can_choose = true;
        assert_eq!(model.apply(Action::ChooseFolder), [Happened::FolderChosen]);
        assert_eq!(model.apply(Action::LeaveFolder), [Happened::FolderLeft]);

        // At the top there is nowhere to go up to, and an empty folder nothing to go into.
        model.browsing = Some(Browsing::default());
        assert_eq!(model.apply(Action::FolderUp), []);
        assert_eq!(model.apply(Action::Accept), []);
    }
}
