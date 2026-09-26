//! On-screen speed readout.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use super::{Player, SpeedUnits};
use crate::game_state::GameState;

/// Metres per second to kilometres per hour, and to miles per hour.
const KMH_PER_MS: f32 = 3.6;
const MPH_PER_MS: f32 = 3600.0 / 1609.344;

#[derive(Component)]
pub(super) struct SpeedText;

pub(super) fn spawn_speedometer(mut commands: Commands, units: Res<SpeedUnits>) {
    commands.spawn((
        SpeedText,
        DespawnOnExit(GameState::Racing),
        Text::new(speed_text(0.0, *units)),
        TextFont::from_font_size(32.0),
        Node {
            position_type: PositionType::Absolute,
            left: px(16),
            bottom: px(12),
            ..default()
        },
    ));
}

pub(super) fn update_speedometer(
    truck: Single<&Velocity, Player>,
    units: Res<SpeedUnits>,
    mut text: Single<&mut Text, With<SpeedText>>,
) {
    text.0 = speed_text(truck.linear.length(), *units);
}

/// `speed` in metres per second.
fn speed_text(speed: f32, units: SpeedUnits) -> String {
    match units {
        SpeedUnits::Kmh => format!("{:.0} km/h", speed * KMH_PER_MS),
        SpeedUnits::Mph => format!("{:.0} mph", speed * MPH_PER_MS),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_speed_reads_in_either_unit() {
        assert_eq!(speed_text(10.0, SpeedUnits::Kmh), "36 km/h");
        // 100 km/h.
        assert_eq!(speed_text(27.778, SpeedUnits::Mph), "62 mph");
    }
}
