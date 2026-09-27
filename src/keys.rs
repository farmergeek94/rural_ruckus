//! Not a slice: which key does each thing in a race. Every slice may use it, as with
//! `game_state`, and asks it rather than naming a key itself, so that the player can bind
//! each thing to any key on the front end's options screen.
//!
//! The arrow keys always drive as well, whatever the bindings: they are what a player
//! reaches for first. Esc is not bindable: it pauses the race, and on the options screen it
//! stops waiting for a key.

use bevy::prelude::*;

/// Something a key does in a race.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Control {
    Throttle,
    Reverse,
    SteerLeft,
    SteerRight,
    Handbrake,
    FlipUpright,
    BackToCheckpoint,
    RestartRace,
    /// Once the player has finished: race again.
    RaceAgain,
    ChangeView,
    LookLeft,
    LookRight,
    LookBack,
    NextWeather,
    NextTimeOfDay,
    PhysicsDebug,
    GraphicsSettings,
}

impl Control {
    /// Every control, in the order the options screen lists them.
    pub const ALL: [Control; 17] = [
        Control::Throttle,
        Control::Reverse,
        Control::SteerLeft,
        Control::SteerRight,
        Control::Handbrake,
        Control::FlipUpright,
        Control::BackToCheckpoint,
        Control::RestartRace,
        Control::RaceAgain,
        Control::ChangeView,
        Control::LookLeft,
        Control::LookRight,
        Control::LookBack,
        Control::NextWeather,
        Control::NextTimeOfDay,
        Control::PhysicsDebug,
        Control::GraphicsSettings,
    ];

    fn index(self) -> usize {
        Control::ALL
            .iter()
            .position(|&control| control == self)
            .expect("every control is in ALL")
    }

    /// What the player sees it called.
    pub const fn name(self) -> &'static str {
        match self {
            Control::Throttle => "Throttle",
            Control::Reverse => "Brake and reverse",
            Control::SteerLeft => "Steer left",
            Control::SteerRight => "Steer right",
            Control::Handbrake => "Handbrake",
            Control::FlipUpright => "Flip upright",
            Control::BackToCheckpoint => "Back to checkpoint",
            Control::RestartRace => "Restart race",
            Control::RaceAgain => "Race again (after the finish)",
            Control::ChangeView => "Change view",
            Control::LookLeft => "Look left",
            Control::LookRight => "Look right",
            Control::LookBack => "Look back",
            Control::NextWeather => "Next weather",
            Control::NextTimeOfDay => "Next time of day",
            Control::PhysicsDebug => "Physics debug view",
            Control::GraphicsSettings => "Graphics settings",
        }
    }

    /// The key it has until the player binds another.
    pub fn default_key(self) -> KeyCode {
        match self {
            Control::Throttle => KeyCode::KeyW,
            Control::Reverse => KeyCode::KeyS,
            Control::SteerLeft => KeyCode::KeyA,
            Control::SteerRight => KeyCode::KeyD,
            Control::Handbrake => KeyCode::Space,
            Control::FlipUpright => KeyCode::KeyR,
            Control::BackToCheckpoint => KeyCode::KeyC,
            Control::RestartRace => KeyCode::Backspace,
            Control::RaceAgain => KeyCode::Enter,
            Control::ChangeView => KeyCode::KeyV,
            Control::LookLeft => KeyCode::KeyQ,
            Control::LookRight => KeyCode::KeyE,
            Control::LookBack => KeyCode::KeyB,
            Control::NextWeather => KeyCode::F7,
            Control::NextTimeOfDay => KeyCode::F8,
            Control::PhysicsDebug => KeyCode::F1,
            Control::GraphicsSettings => KeyCode::F2,
        }
    }

    /// The arrow key that does it too, whatever it is bound to.
    fn arrow(self) -> Option<KeyCode> {
        match self {
            Control::Throttle => Some(KeyCode::ArrowUp),
            Control::Reverse => Some(KeyCode::ArrowDown),
            Control::SteerLeft => Some(KeyCode::ArrowLeft),
            Control::SteerRight => Some(KeyCode::ArrowRight),
            _ => None,
        }
    }
}

/// The key bound to each control. Insert it before adding the plugins, or change it at
/// any time.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyBindings([KeyCode; Control::ALL.len()]);

impl Default for KeyBindings {
    fn default() -> Self {
        Self(Control::ALL.map(Control::default_key))
    }
}

impl KeyBindings {
    /// The key bound to `control`.
    pub fn key(&self, control: Control) -> KeyCode {
        self.0[control.index()]
    }

    pub fn bind(&mut self, control: Control, key: KeyCode) {
        self.0[control.index()] = key;
    }

    /// Whether `control`'s key, or its arrow key, is held down.
    pub fn pressed(&self, keys: &ButtonInput<KeyCode>, control: Control) -> bool {
        keys.pressed(self.key(control)) || control.arrow().is_some_and(|key| keys.pressed(key))
    }

    /// Whether `control`'s key, or its arrow key, went down this frame.
    pub fn just_pressed(&self, keys: &ButtonInput<KeyCode>, control: Control) -> bool {
        keys.just_pressed(self.key(control))
            || control.arrow().is_some_and(|key| keys.just_pressed(key))
    }

    /// What the player sees `control`'s key called.
    pub fn key_name(&self, control: Control) -> &'static str {
        key_name(self.key(control))
    }
}

/// A run condition: whether `control`'s key went down this frame.
pub fn just_pressed(
    control: Control,
) -> impl Fn(Res<KeyBindings>, Res<ButtonInput<KeyCode>>) -> bool {
    move |bindings, keys| bindings.just_pressed(&keys, control)
}

/// Every key a control may be bound to, with what the player sees it called, in the order
/// the options screen goes through them. The front end keeps a binding by that name, so
/// the list may grow but a name must not change.
pub const BINDABLE: [(KeyCode, &str); 76] = [
    (KeyCode::KeyA, "A"),
    (KeyCode::KeyB, "B"),
    (KeyCode::KeyC, "C"),
    (KeyCode::KeyD, "D"),
    (KeyCode::KeyE, "E"),
    (KeyCode::KeyF, "F"),
    (KeyCode::KeyG, "G"),
    (KeyCode::KeyH, "H"),
    (KeyCode::KeyI, "I"),
    (KeyCode::KeyJ, "J"),
    (KeyCode::KeyK, "K"),
    (KeyCode::KeyL, "L"),
    (KeyCode::KeyM, "M"),
    (KeyCode::KeyN, "N"),
    (KeyCode::KeyO, "O"),
    (KeyCode::KeyP, "P"),
    (KeyCode::KeyQ, "Q"),
    (KeyCode::KeyR, "R"),
    (KeyCode::KeyS, "S"),
    (KeyCode::KeyT, "T"),
    (KeyCode::KeyU, "U"),
    (KeyCode::KeyV, "V"),
    (KeyCode::KeyW, "W"),
    (KeyCode::KeyX, "X"),
    (KeyCode::KeyY, "Y"),
    (KeyCode::KeyZ, "Z"),
    (KeyCode::Digit0, "0"),
    (KeyCode::Digit1, "1"),
    (KeyCode::Digit2, "2"),
    (KeyCode::Digit3, "3"),
    (KeyCode::Digit4, "4"),
    (KeyCode::Digit5, "5"),
    (KeyCode::Digit6, "6"),
    (KeyCode::Digit7, "7"),
    (KeyCode::Digit8, "8"),
    (KeyCode::Digit9, "9"),
    (KeyCode::F1, "F1"),
    (KeyCode::F2, "F2"),
    (KeyCode::F3, "F3"),
    (KeyCode::F4, "F4"),
    (KeyCode::F5, "F5"),
    (KeyCode::F6, "F6"),
    (KeyCode::F7, "F7"),
    (KeyCode::F8, "F8"),
    (KeyCode::F9, "F9"),
    (KeyCode::F10, "F10"),
    (KeyCode::F11, "F11"),
    (KeyCode::F12, "F12"),
    (KeyCode::Space, "Space"),
    (KeyCode::Enter, "Enter"),
    (KeyCode::Backspace, "Backspace"),
    (KeyCode::Tab, "Tab"),
    (KeyCode::ShiftLeft, "Left Shift"),
    (KeyCode::ShiftRight, "Right Shift"),
    (KeyCode::ControlLeft, "Left Ctrl"),
    (KeyCode::ControlRight, "Right Ctrl"),
    (KeyCode::AltLeft, "Left Alt"),
    (KeyCode::AltRight, "Right Alt"),
    (KeyCode::ArrowUp, "Up"),
    (KeyCode::ArrowDown, "Down"),
    (KeyCode::ArrowLeft, "Left"),
    (KeyCode::ArrowRight, "Right"),
    (KeyCode::Insert, "Insert"),
    (KeyCode::Delete, "Delete"),
    (KeyCode::Home, "Home"),
    (KeyCode::End, "End"),
    (KeyCode::PageUp, "Page Up"),
    (KeyCode::PageDown, "Page Down"),
    (KeyCode::Minus, "-"),
    (KeyCode::Equal, "="),
    (KeyCode::BracketLeft, "["),
    (KeyCode::BracketRight, "]"),
    (KeyCode::Semicolon, ";"),
    (KeyCode::Quote, "'"),
    (KeyCode::Comma, ","),
    (KeyCode::Period, "."),
];

/// What the player sees `key` called.
pub fn key_name(key: KeyCode) -> &'static str {
    BINDABLE
        .iter()
        .find(|(bindable, _)| *bindable == key)
        .map_or("?", |(_, name)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_key_is_bindable_and_named_once() {
        for control in Control::ALL {
            assert_ne!(key_name(control.default_key()), "?", "{control:?}");
        }
        let mut names: Vec<&str> = BINDABLE.iter().map(|(_, name)| *name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), BINDABLE.len());
        assert!(!BINDABLE.iter().any(|(key, _)| *key == KeyCode::Escape));
    }

    #[test]
    fn a_control_answers_to_its_key_and_its_arrow() {
        let mut bindings = KeyBindings::default();
        bindings.bind(Control::Throttle, KeyCode::KeyI);
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyW);
        assert!(!bindings.pressed(&keys, Control::Throttle));
        keys.press(KeyCode::KeyI);
        assert!(bindings.pressed(&keys, Control::Throttle));
        keys.release_all();
        keys.press(KeyCode::ArrowUp);
        assert!(bindings.pressed(&keys, Control::Throttle));
        assert_eq!(bindings.key_name(Control::Throttle), "I");
    }
}
