//! On-screen speed readout, with the gear beside it.

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;

use super::{Engine, Gear, Gearbox, Player, SpeedUnits};
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
        Text::new(speed_text(0.0, Gear::Forward(1), *units)),
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
    truck: Single<(&LinearVelocity, &Gearbox, &Engine), Player>,
    units: Res<SpeedUnits>,
    mut text: Single<&mut Text, With<SpeedText>>,
) {
    let (velocity, gearbox, engine) = *truck;
    let mut shown = speed_text(velocity.0.length(), gearbox.gear(), *units);
    if !engine.running() {
        shown.push_str(" stalled");
    }
    // Only when it changes, so that the text is not laid out again every frame.
    if text.0 != shown {
        text.0 = shown;
    }
}

/// `speed` in metres per second.
fn speed_text(speed: f32, gear: Gear, units: SpeedUnits) -> String {
    let speed = match units {
        SpeedUnits::Kmh => format!("{:.0} km/h", speed * KMH_PER_MS),
        SpeedUnits::Mph => format!("{:.0} mph", speed * MPH_PER_MS),
    };
    format!("{speed}   Gear {gear}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_speed_reads_in_either_unit() {
        let first = Gear::Forward(1);
        assert_eq!(speed_text(10.0, first, SpeedUnits::Kmh), "36 km/h   Gear 1");
        // 100 km/h.
        assert_eq!(
            speed_text(27.778, Gear::Forward(4), SpeedUnits::Mph),
            "62 mph   Gear 4"
        );
        assert_eq!(
            speed_text(2.0, Gear::Reverse, SpeedUnits::Mph),
            "4 mph   Gear R"
        );
    }
}
