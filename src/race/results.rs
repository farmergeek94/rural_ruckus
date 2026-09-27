//! The results: once the player's truck has finished, a table of every truck in the race,
//! in its order, with its race time and its best lap. The trucks still racing are listed
//! with the lap they are on, and the table keeps up with them as they finish. Enter races
//! again; Esc pauses, as it does in the race, and the pause dialog can leave it.
//!
//! What MTM2's results screen showed is not measured. This one is the game's own.

use bevy::prelude::*;

use super::hud::format_time;
use super::progress::standings;
use super::{RaceSettings, Racer};
use crate::game_state::GameState;
use crate::track::Track;
use crate::truck::{PlayerTruck, TruckName};

/// The table's columns, left to right: the place, the truck, the race time, and the best
/// lap. How wide each is, in pixels.
const COLUMNS: [f32; 4] = [70.0, 320.0, 140.0, 140.0];

#[derive(Component)]
pub(super) struct ResultsPanel;

/// On the text of column `0` of the table: each column is one text, a row a line, so that
/// the rows line up across them.
#[derive(Component)]
pub(super) struct ResultsColumn(usize);

pub(super) fn spawn_results(mut commands: Commands) {
    commands
        .spawn((
            ResultsPanel,
            DespawnOnExit(GameState::Racing),
            Visibility::Hidden,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                top: Val::Percent(18.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(px(24)),
                        row_gap: px(6),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
                ))
                .with_children(|panel| {
                    panel.spawn((Text::new("RACE RESULTS"), TextFont::from_font_size(30.0)));
                    panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            margin: UiRect::vertical(px(12)),
                            ..default()
                        })
                        .with_children(|table| {
                            for (column, width) in COLUMNS.into_iter().enumerate() {
                                table.spawn((
                                    ResultsColumn(column),
                                    Text::default(),
                                    TextFont::from_font_size(22.0),
                                    Node {
                                        width: px(width),
                                        ..default()
                                    },
                                ));
                            }
                        });
                    panel.spawn((
                        Text::new("Enter  race again        Esc  pause"),
                        TextFont::from_font_size(16.0),
                    ));
                });
        });
}

/// One line of the table.
struct Row {
    place: usize,
    name: String,
    result: String,
    best: String,
}

/// Shows the table once the player has finished, and keeps it up to date.
pub(super) fn show_results(
    settings: Res<RaceSettings>,
    track: Res<Track>,
    time: Res<Time<Fixed>>,
    racers: Query<(&Racer, &Transform, Option<&TruckName>, Has<PlayerTruck>)>,
    mut panel: Single<&mut Visibility, With<ResultsPanel>>,
    mut columns: Query<(&ResultsColumn, &mut Text)>,
) {
    let player_finished = racers
        .iter()
        .any(|(racer, _, _, player)| player && racer.progress.finished.is_some());
    panel.set_if_neq(if player_finished {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    if !player_finished {
        return;
    }

    let tick_seconds = time.timestep().as_secs_f64();
    let show = |ticks: Option<u64>| match ticks {
        Some(ticks) => format_time(ticks as f64 * tick_seconds),
        None => "-:--.--".into(),
    };
    let racers: Vec<_> = racers.iter().collect();
    let places: Vec<_> = racers
        .iter()
        .map(|(racer, transform, _, _)| {
            let to_next_gate = track
                .gates
                .get(racer.progress.next_gate)
                .map_or(0.0, |gate| gate.center.distance(transform.translation.xz()));
            (&racer.progress, to_next_gate)
        })
        .collect();
    let rows: Vec<Row> = standings(&places, track.gates.len())
        .into_iter()
        .enumerate()
        .map(|(at, index)| {
            let (racer, _, name, player) = racers[index];
            let progress = &racer.progress;
            let name = match (player, name) {
                (true, Some(name)) => format!("{} (you)", name.0),
                (true, None) => "You".into(),
                (false, Some(name)) => name.0.clone(),
                (false, None) => "Truck".into(),
            };
            let result = match (progress.finished, progress.lap) {
                (Some(total), _) => show(Some(total)),
                (None, 0) => "Not started".into(),
                (None, lap) => format!("Lap {lap}/{}", settings.laps),
            };
            Row {
                place: at + 1,
                name,
                result,
                best: show(progress.best_lap),
            }
        })
        .collect();

    let shown = table(&rows);
    for (column, mut text) in &mut columns {
        if text.0 != shown[column.0] {
            text.0 = shown[column.0].clone();
        }
    }
}

/// The table as the text of each column, under its heading, a row a line.
fn table(rows: &[Row]) -> [String; 4] {
    let mut columns = ["", "Truck", "Time", "Best lap"].map(String::from);
    for row in rows {
        let cells = [
            ordinal(row.place),
            row.name.clone(),
            row.result.clone(),
            row.best.clone(),
        ];
        for (column, cell) in columns.iter_mut().zip(cells) {
            column.push('\n');
            column.push_str(&cell);
        }
    }
    columns
}

/// 1st, 2nd, 3rd, 4th and so on.
fn ordinal(place: usize) -> String {
    let suffix = match (place % 10, place % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{place}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_are_written_as_ordinals() {
        let places: Vec<String> = [1, 2, 3, 4, 8, 11, 12, 13, 21, 22].map(ordinal).into();
        assert_eq!(
            places,
            [
                "1st", "2nd", "3rd", "4th", "8th", "11th", "12th", "13th", "21st", "22nd"
            ]
        );
    }

    #[test]
    fn the_table_lists_every_row_in_order() {
        let rows = [
            Row {
                place: 1,
                name: "Bigfoot".into(),
                result: "3:01.00".into(),
                best: "0:59.50".into(),
            },
            Row {
                place: 2,
                name: "Built-in truck (you)".into(),
                result: "Lap 3/3".into(),
                best: "1:01.20".into(),
            },
        ];
        let [places, names, results, best] = table(&rows);
        assert_eq!(places, "\n1st\n2nd");
        assert_eq!(names, "Truck\nBigfoot\nBuilt-in truck (you)");
        assert_eq!(results, "Time\n3:01.00\nLap 3/3");
        assert_eq!(best, "Best lap\n0:59.50\n1:01.20");
    }
}
