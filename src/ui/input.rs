//! Turns keys, the gamepad, the mouse wheel and clicks on widgets into `Action`s. The only
//! module that knows a binding.
//!
//! | | Keys | Gamepad |
//! | --- | --- | --- |
//! | Move, and change a setting | arrows, WASD, the wheel | d-pad, left stick |
//! | Next and previous screen | Tab or E, Shift+Tab or Q | the shoulder buttons |
//! | Fewer and more opponents | - and + | West and North |
//! | Easier and harder | [ and ] | |
//! | Accept (next screen, and GO from the garage; the next value of a setting) | | South |
//! | GO | Enter | Start |
//! | Bind a key (on a key binding) | Enter, then the key; Esc to stop | South |
//! | Turn the truck | drag it | right stick (see `showroom`) |
//! | Search a long list of trucks or tracks | /, or click the search field | |
//!
//! While the search field takes the keys, what is typed goes into it, Backspace takes
//! the last character off, the up and down arrows walk what it finds, Enter keeps the
//! search and gives the keys back, and Esc empties it and gives them back.
//!
//! While a folder is browsed, the arrows move and go in (right) and up (left), and:
//!
//! | | Keys | Gamepad |
//! | --- | --- | --- |
//! | Go into the highlighted folder | Enter | South |
//! | Go up | Backspace | West |
//! | Choose this folder | Space | North |
//! | Stop browsing | Esc | East |

use bevy::input::keyboard::Key;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::model::Action;
use super::{Model, PlayerDid, Settings};

/// How long a direction is held before it starts repeating, in seconds, and how long
/// between repeats. A long list is walked by holding a key down.
const REPEAT_AFTER: f32 = 0.4;
const REPEAT_EVERY: f32 = 0.08;
/// How far a stick has to be pushed to count as a direction, from 0 to 1.
const STICK_PUSHED: f32 = 0.6;

/// What activating a widget does.
#[derive(Component, Clone, Copy)]
pub(super) struct Does(pub(super) Action);

pub(super) fn widget_activated(
    activate: On<Activate>,
    widgets: Query<&Does>,
    mut did: MessageWriter<PlayerDid>,
) {
    if let Ok(does) = widgets.get(activate.entity) {
        did.write(PlayerDid(does.0));
    }
}

/// The direction being held, and for how long.
#[derive(Default)]
pub(super) struct Held {
    direction: Option<Action>,
    seconds: f32,
}

impl Held {
    /// Whether to act on `direction` this frame: at once when it is first held, and then
    /// over and over after a pause.
    fn acts(&mut self, direction: Option<Action>, dt: f32) -> bool {
        if direction != self.direction {
            *self = Self {
                direction,
                seconds: 0.0,
            };
            return direction.is_some();
        }
        if direction.is_none() {
            return false;
        }
        let before = self.seconds;
        self.seconds += dt;
        let repeats = |seconds: f32| ((seconds - REPEAT_AFTER) / REPEAT_EVERY).floor().max(-1.0);
        repeats(self.seconds) > repeats(before)
    }
}

#[expect(clippy::too_many_arguments, reason = "it reads every kind of input")]
pub(super) fn read_input(
    time: Res<Time>,
    model: Option<Res<Model>>,
    settings: Res<Settings>,
    // Absent in a headless app, which drives the front end with `PlayerDid` itself.
    keys: Option<Res<ButtonInput<KeyCode>>>,
    // What the keys mean in the player's layout, for what is typed.
    typed: Option<Res<ButtonInput<Key>>>,
    wheel: Option<Res<AccumulatedMouseScroll>>,
    gamepads: Query<&Gamepad>,
    mut held: Local<Held>,
    mut did: MessageWriter<PlayerDid>,
) {
    // A key binding waiting for a key takes the next one, and nothing else happens.
    if let Some(model) = &model
        && model.0.listening
    {
        *held = Held::default();
        let Some(keys) = &keys else {
            return;
        };
        let in_hand = model.0.setting_in_hand;
        if keys.just_pressed(KeyCode::Escape) {
            did.write(PlayerDid(Action::StopListening));
        } else if let Some(setting) = settings.0.get(in_hand)
            && let Some(value) = keys
                .get_just_pressed()
                .find_map(|pressed| setting.keys.iter().position(|key| key == pressed))
        {
            did.write(PlayerDid(Action::SetSetting(in_hand, value)));
        }
        return;
    }

    // The wheel moves whatever list is up, a row for each notch.
    if let Some(wheel) = wheel {
        if wheel.delta.y > 0.0 {
            did.write(PlayerDid(Action::Up));
        } else if wheel.delta.y < 0.0 {
            did.write(PlayerDid(Action::Down));
        }
    }

    // A search field taking the keys takes all of them but the arrows up and down. The
    // gamepad goes on as ever: it has nothing to type with.
    if let Some(model) = &model
        && model.0.typing
        && let Some(keys) = &keys
    {
        if let Some(typed) = &typed {
            for key in typed.get_just_pressed() {
                match key {
                    Key::Character(text) => {
                        for character in text.chars().filter(|character| !character.is_control()) {
                            did.write(PlayerDid(Action::Type(character)));
                        }
                    }
                    Key::Space => {
                        did.write(PlayerDid(Action::Type(' ')));
                    }
                    _ => {}
                }
            }
        }
        if keys.just_pressed(KeyCode::Backspace) {
            did.write(PlayerDid(Action::Erase));
        }
        if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
            did.write(PlayerDid(Action::StopTyping));
        }
        if keys.just_pressed(KeyCode::Escape) {
            did.write(PlayerDid(Action::ClearSearch));
        }
        let direction = if keys.pressed(KeyCode::ArrowUp) {
            Some(Action::Up)
        } else if keys.pressed(KeyCode::ArrowDown) {
            Some(Action::Down)
        } else {
            None
        };
        if held.acts(direction, time.delta_secs())
            && let Some(direction) = direction
        {
            did.write(PlayerDid(direction));
        }
        return;
    }

    let browsing = model
        .as_ref()
        .is_some_and(|model| model.0.browsing.is_some());
    let mut direction = None;
    let mut point = |pressed: bool, action: Action| {
        if pressed {
            direction = Some(action);
        }
    };

    if let Some(keys) = &keys {
        point(
            keys.any_pressed([KeyCode::ArrowUp, KeyCode::KeyW]),
            Action::Up,
        );
        point(
            keys.any_pressed([KeyCode::ArrowDown, KeyCode::KeyS]),
            Action::Down,
        );
        point(
            keys.any_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]),
            Action::Less,
        );
        point(
            keys.any_pressed([KeyCode::ArrowRight, KeyCode::KeyD]),
            Action::More,
        );

        let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
        if keys.just_pressed(KeyCode::KeyE) || (keys.just_pressed(KeyCode::Tab) && !shift) {
            did.write(PlayerDid(Action::NextScreen));
        }
        if keys.just_pressed(KeyCode::KeyQ) || (keys.just_pressed(KeyCode::Tab) && shift) {
            did.write(PlayerDid(Action::PreviousScreen));
        }
        if keys.any_just_pressed([KeyCode::Minus, KeyCode::NumpadSubtract]) {
            did.write(PlayerDid(Action::FewerOpponents));
        }
        // The key that has + on it, which is = without Shift.
        if keys.any_just_pressed([KeyCode::Equal, KeyCode::NumpadAdd]) {
            did.write(PlayerDid(Action::MoreOpponents));
        }
        if keys.just_pressed(KeyCode::BracketLeft) {
            did.write(PlayerDid(Action::Easier));
        }
        if keys.just_pressed(KeyCode::BracketRight) {
            did.write(PlayerDid(Action::Harder));
        }
        if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
            did.write(PlayerDid(Action::Go));
        }
        if keys.any_just_pressed([KeyCode::Slash, KeyCode::NumpadDivide]) && !browsing {
            did.write(PlayerDid(Action::Search));
        }
        if browsing {
            for (key, action) in [
                (KeyCode::Backspace, Action::FolderUp),
                (KeyCode::Space, Action::ChooseFolder),
                (KeyCode::Escape, Action::LeaveFolder),
            ] {
                if keys.just_pressed(key) {
                    did.write(PlayerDid(action));
                }
            }
        }
    }

    for gamepad in &gamepads {
        let stick = gamepad.left_stick();
        point(
            gamepad.pressed(GamepadButton::DPadUp) || stick.y > STICK_PUSHED,
            Action::Up,
        );
        point(
            gamepad.pressed(GamepadButton::DPadDown) || stick.y < -STICK_PUSHED,
            Action::Down,
        );
        point(
            gamepad.pressed(GamepadButton::DPadLeft) || stick.x < -STICK_PUSHED,
            Action::Less,
        );
        point(
            gamepad.pressed(GamepadButton::DPadRight) || stick.x > STICK_PUSHED,
            Action::More,
        );

        let (west, north) = if browsing {
            (Action::FolderUp, Action::ChooseFolder)
        } else {
            (Action::FewerOpponents, Action::MoreOpponents)
        };
        for (button, action) in [
            (GamepadButton::RightTrigger, Action::NextScreen),
            (GamepadButton::LeftTrigger, Action::PreviousScreen),
            (GamepadButton::West, west),
            (GamepadButton::North, north),
            (GamepadButton::East, Action::LeaveFolder),
            (GamepadButton::South, Action::Accept),
            (GamepadButton::Start, Action::Go),
        ] {
            if gamepad.just_pressed(button) {
                did.write(PlayerDid(action));
            }
        }
    }

    if held.acts(direction, time.delta_secs())
        && let Some(direction) = direction
    {
        did.write(PlayerDid(direction));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_direction_acts_at_once_and_then_repeats() {
        let mut held = Held::default();
        let frame = 1.0 / 60.0;
        assert!(held.acts(Some(Action::Down), frame));

        // Nothing more until the pause is over, then one every `REPEAT_EVERY`.
        let mut acted = 0;
        for _ in 0..((REPEAT_AFTER - 0.05) / frame) as usize {
            acted += held.acts(Some(Action::Down), frame) as usize;
        }
        assert_eq!(acted, 0);
        for _ in 0..60 {
            acted += held.acts(Some(Action::Down), frame) as usize;
        }
        let expected = (1.0 - 0.05) / REPEAT_EVERY;
        assert!((acted as f32 - expected).abs() <= 1.5, "{acted}");

        // Letting go stops it, and another direction acts at once.
        assert!(!held.acts(None, frame));
        assert!(!held.acts(None, frame));
        assert!(held.acts(Some(Action::Up), frame));
        assert!(held.acts(Some(Action::Down), frame));
    }
}
