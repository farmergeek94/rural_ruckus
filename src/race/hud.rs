//! On-screen race readout: lap, next checkpoint and times.

use bevy::prelude::*;

use super::{RaceClock, RaceSettings, Racer, position};
use crate::game_state::GameState;
use crate::track::Track;
use crate::truck::PlayerTruck;

#[derive(Component)]
pub(super) struct RaceText;

pub(super) fn spawn_race_hud(mut commands: Commands) {
    commands.spawn((
        RaceText,
        DespawnOnExit(GameState::Racing),
        Text::default(),
        TextFont::from_font_size(22.0),
        TextLayout::justify(Justify::Right),
        Node {
            position_type: PositionType::Absolute,
            right: px(16),
            top: px(12),
            ..default()
        },
    ));
}

/// Shows the player's race, and where the player stands in it when it is against others.
pub(super) fn update_race_hud(
    clock: Res<RaceClock>,
    settings: Res<RaceSettings>,
    track: Res<Track>,
    time: Res<Time<Fixed>>,
    racer: Single<&Racer, With<PlayerTruck>>,
    others: Query<&Racer, Without<PlayerTruck>>,
    mut text: Single<&mut Text, With<RaceText>>,
) {
    let racer = *racer;
    let progress = &racer.progress;
    let tick_seconds = time.timestep().as_secs_f64();
    let show = |ticks: Option<u64>| match ticks {
        Some(ticks) => format_time(ticks as f64 * tick_seconds),
        None => "-:--.--".into(),
    };

    let headline = match (progress.finished, progress.lap) {
        (Some(total), _) => format!("Finished  {}", show(Some(total))),
        (None, 0) => "Cross the start line to begin".into(),
        (None, lap) => format!(
            "Lap {lap}/{}\n{}Time  {}",
            settings.laps,
            checkpoints_text(progress.next_gate, track.gates.len()),
            show(progress.current_lap_time(clock.tick)),
        ),
    };
    // Against the clock alone there is no place to be in.
    let place = if others.is_empty() {
        String::new()
    } else {
        let place = position(
            (progress, racer.to_next_gate),
            others
                .iter()
                .map(|other| (&other.progress, other.to_next_gate)),
            track.gates.len(),
        );
        format!("Position {place}/{}\n", others.iter().count() + 1)
    };
    text.0 = format!(
        "{place}{headline}\nLast  {}\nBest  {}",
        show(progress.last_lap),
        show(progress.best_lap),
    );
}

/// How many of the lap's checkpoints have been passed, and how many there are, as a line:
/// `Checkpoint 1/9` once the first after the start line has been passed. The start and
/// finish line is gate 0 and is not one of them. Empty on a track that has only that line.
fn checkpoints_text(next_gate: usize, gate_count: usize) -> String {
    let checkpoints = gate_count.saturating_sub(1);
    if checkpoints == 0 {
        return String::new();
    }
    // Gate 0 is next once every checkpoint has been passed.
    let passed = if next_gate == 0 {
        checkpoints
    } else {
        next_gate - 1
    };
    format!("Checkpoint {passed}/{checkpoints}\n")
}

/// Minutes, seconds and hundredths, like `1:07.25`.
pub(super) fn format_time(seconds: f64) -> String {
    let hundredths = (seconds * 100.0).round() as u64;
    format!(
        "{}:{:02}.{:02}",
        hundredths / 6000,
        hundredths / 100 % 60,
        hundredths % 100
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_times() {
        assert_eq!(format_time(0.0), "0:00.00");
        assert_eq!(format_time(7.256), "0:07.26");
        assert_eq!(format_time(67.25), "1:07.25");
        assert_eq!(format_time(59.999), "1:00.00");
        assert_eq!(format_time(754.5), "12:34.50");
    }

    #[test]
    fn checkpoints_are_counted_from_the_first_after_the_start_line() {
        // Ten gates: the start and finish line, and nine checkpoints.
        assert_eq!(checkpoints_text(1, 10), "Checkpoint 0/9\n");
        assert_eq!(checkpoints_text(2, 10), "Checkpoint 1/9\n");
        assert_eq!(checkpoints_text(9, 10), "Checkpoint 8/9\n");
        // All nine passed, and the finish line next.
        assert_eq!(checkpoints_text(0, 10), "Checkpoint 9/9\n");
        // A track with the line alone has no checkpoints to count.
        assert_eq!(checkpoints_text(0, 1), "");
        assert_eq!(checkpoints_text(0, 0), "");
    }
}
