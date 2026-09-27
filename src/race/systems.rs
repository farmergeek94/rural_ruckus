//! The systems that run the race: following each truck's progress, and the keys that
//! put a truck back on the course.

use bevy::prelude::*;

use super::pause::running;
use super::start::count_down;
use super::{RaceClock, RacePause, RaceProgress, RaceSettings, RaceStart, Racer};
use crate::keys::{Control, KeyBindings};
use crate::track::{Track, TrackData, yaw_direction};
use crate::truck::{PlaceTruck, PlayerTruck, Truck};

/// Every race starts from nothing. Each `Racer` does too, being part of a new truck.
pub(super) fn reset_clock(mut clock: ResMut<RaceClock>) {
    clock.tick = 0;
}

pub(super) fn tick_clock(mut clock: ResMut<RaceClock>) {
    clock.tick += 1;
}

pub(super) fn track_progress(
    clock: Res<RaceClock>,
    settings: Res<RaceSettings>,
    track: Res<Track>,
    mut racers: Query<(&Transform, &mut Racer)>,
) {
    for (transform, mut racer) in &mut racers {
        let position = transform.translation;
        if let Some(last_position) = racer.last_position {
            racer.progress.advance(
                &track.gates,
                settings.laps,
                last_position,
                position,
                clock.tick,
            );
        }
        racer.last_position = Some(position);
    }
}

/// Every truck takes part: make it a racer and put it on the starting grid. The player
/// starts behind the trucks the computer drives, and so on pole position when alone.
pub(super) fn enlist_trucks(
    mut commands: Commands,
    track: Res<Track>,
    trucks: Query<(Entity, Has<PlayerTruck>), Unlisted>,
    racers: Query<(), With<Racer>>,
    mut place: MessageWriter<PlaceTruck>,
) {
    let mut trucks: Vec<(bool, Entity)> = trucks
        .iter()
        .map(|(truck, is_player)| (is_player, truck))
        .collect();
    // The player's last, and the rest in the order they were made.
    trucks.sort();
    let taken = racers.iter().count();
    for (index, &(_, truck)) in trucks.iter().enumerate() {
        let grid_place = taken + index;
        commands.entity(truck).insert(Racer {
            grid_place,
            ..default()
        });
        place.write(on_grid(truck, grid_place, &track));
    }
}

/// Asks for a racer's truck to be put back at the last checkpoint it drove through, or on
/// its place of the grid if it hasn't crossed the start line yet. This is what the
/// player's C key does, and how another slice gets a truck that is stuck going again.
#[derive(Message, Clone, Debug)]
pub struct BackToCheckpoint {
    pub truck: Entity,
}

/// A truck is not put down within this distance of another, in metres, but this much
/// further back from the checkpoint, as often as it takes and no more than `TRIES` times.
const CLEAR_OF_OTHERS: f32 = 7.0;
const TRIES: usize = 6;

/// C asks for the player's truck. Whoever drives the others asks for those.
#[expect(clippy::too_many_arguments, reason = "the keys and the whole race")]
pub(super) fn back_to_checkpoint(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<KeyBindings>,
    track: Res<Track>,
    mut requests: MessageReader<BackToCheckpoint>,
    player: Query<Entity, (With<Racer>, With<PlayerTruck>)>,
    mut racers: Query<&mut Racer>,
    trucks: Query<(Entity, &Transform), With<Truck>>,
    mut place: MessageWriter<PlaceTruck>,
    pause: Option<Res<State<RacePause>>>,
) {
    let mut asked: Vec<Entity> = requests.read().map(|request| request.truck).collect();
    // Another slice's requests are kept, but the key does nothing while paused.
    if running(pause) && bindings.just_pressed(&keys, Control::BackToCheckpoint) {
        asked.extend(&player);
    }
    asked.sort();
    asked.dedup();

    // Where trucks are, and where the ones put back in this frame are about to be.
    let mut taken: Vec<(Entity, Vec2)> = trucks
        .iter()
        .map(|(truck, transform)| (truck, transform.translation.xz()))
        .collect();
    for truck in asked {
        let Ok(mut racer) = racers.get_mut(truck) else {
            continue;
        };
        racer.last_position = None;
        let (mut position, yaw) = match racer.progress.last_gate(track.gates.len()) {
            Some(index) => (track.gates[index].center, track.gates[index].yaw),
            None => {
                let place = track.grid_place(racer.grid_place);
                (place.position, place.yaw)
            }
        };
        for _ in 0..TRIES {
            let clear = taken
                .iter()
                .all(|&(other, at)| other == truck || at.distance(position) > CLEAR_OF_OTHERS);
            if clear {
                break;
            }
            position -= yaw_direction(yaw) * CLEAR_OF_OTHERS;
        }
        taken.retain(|&(other, _)| other != truck);
        taken.push((truck, position));
        place.write(PlaceTruck {
            truck,
            ground: on_ground(&track, position),
            yaw,
        });
    }
}

/// Backspace abandons the race, and every truck starts again from its place on the grid,
/// with a new countdown. Once the player has finished, so does Enter: race again.
#[expect(
    clippy::too_many_arguments,
    reason = "the keys, the clock and the whole race"
)]
pub(super) fn restart_race(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<KeyBindings>,
    track: Res<Track>,
    clock: Res<RaceClock>,
    time: Res<Time<Fixed>>,
    mut start: ResMut<RaceStart>,
    mut racers: Query<(Entity, &mut Racer, Has<PlayerTruck>)>,
    mut place: MessageWriter<PlaceTruck>,
) {
    let finished = racers
        .iter()
        .any(|(_, racer, player)| player && racer.progress.finished.is_some());
    let again = finished && bindings.just_pressed(&keys, Control::RaceAgain);
    if !bindings.just_pressed(&keys, Control::RestartRace) && !again {
        return;
    }
    count_down(&mut start, clock.tick, time.timestep().as_secs_f32());
    for (truck, mut racer, _) in &mut racers {
        racer.progress = RaceProgress::default();
        racer.last_position = None;
        place.write(on_grid(truck, racer.grid_place, &track));
    }
}

/// A truck that isn't a racer yet.
type Unlisted = (With<Truck>, Without<Racer>);

fn on_grid(truck: Entity, grid_place: usize, track: &TrackData) -> PlaceTruck {
    let place = track.grid_place(grid_place);
    PlaceTruck {
        truck,
        ground: on_ground(track, place.position),
        yaw: place.yaw,
    }
}

fn on_ground(track: &TrackData, position: Vec2) -> Vec3 {
    Vec3::new(
        position.x,
        track.heights.height_at(position.x, position.y),
        position.y,
    )
}
