//! On-screen reminder of the controls, with the keys they are bound to (`keys`).

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

use crate::game_state::GameState;
use crate::keys::{Control, KeyBindings};

pub struct ControlsHelpPlugin;

impl Plugin for ControlsHelpPlugin {
    fn build(&self, app: &mut App) {
        // A slice works alone, so it makes sure of the state it is built in. A second
        // `init_state` would log a warning.
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        if !app.world().contains_resource::<State<GameState>>() {
            app.init_state::<GameState>();
        }
        app.init_resource::<ControlsHelpSettings>()
            .init_resource::<KeyBindings>()
            .add_systems(OnEnter(GameState::Racing), spawn_controls_help);
    }
}

/// Whether the reference is shown. Read as each race begins.
/// Every control and its key, and Esc, which is not bindable.
fn reference(bindings: &KeyBindings) -> String {
    let key = |control| bindings.key_name(control);
    let mut parts = vec![
        format!(
            "{}/{} throttle",
            key(Control::Throttle),
            key(Control::Reverse)
        ),
        format!(
            "{}/{} steer",
            key(Control::SteerLeft),
            key(Control::SteerRight)
        ),
    ];
    parts.extend(
        Control::ALL
            .into_iter()
            .skip(4)
            .map(|control| format!("{} {}", key(control), control.name().to_lowercase())),
    );
    parts.push("Esc front end".into());
    parts.join("   ")
}

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ControlsHelpSettings {
    pub show: bool,
}

impl Default for ControlsHelpSettings {
    fn default() -> Self {
        Self { show: true }
    }
}

fn spawn_controls_help(
    mut commands: Commands,
    settings: Res<ControlsHelpSettings>,
    bindings: Res<KeyBindings>,
) {
    if !settings.show {
        return;
    }
    commands.spawn((
        DespawnOnExit(GameState::Racing),
        Text::new(reference(&bindings)),
        TextFont::from_font_size(14.0),
        Node {
            position_type: PositionType::Absolute,
            left: px(16),
            top: px(12),
            // Wraps before it reaches the race readout in the other corner.
            max_width: Val::Percent(60.0),
            ..default()
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reference_names_the_keys_that_are_bound() {
        let mut bindings = KeyBindings::default();
        let text = reference(&bindings);
        assert!(
            text.starts_with("W/S throttle   A/D steer   Space handbrake"),
            "{text}"
        );
        assert!(text.contains("V change view"));
        bindings.bind(Control::ChangeView, KeyCode::KeyN);
        assert!(reference(&bindings).contains("N change view"));
    }
}
