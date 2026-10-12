//! Laps round a circuit: on a track without `TrackData::rumble` zones, every racer's
//! progress through the checkpoint gates is followed each physics tick by the `circuit`
//! crate's rules, and how far it has still to drive to its next gate along the course,
//! which the race order compares between racers through as many gates. Where each gate is
//! along the course is found once a race.

use bevy::prelude::*;

pub use ::circuit::{Milestone, RaceProgress, position, standings};

use super::{RaceClock, RaceSettings, Racer};
use crate::track::Track;

/// Whether the race being run is round a circuit: laps and checkpoints, not a rumble.
pub fn is_circuit(track: Res<Track>) -> bool {
    track.rumble.is_none()
}

/// Where the gates of the race's track are along its course.
#[derive(Resource, Default)]
pub(super) struct GatesAlong(::circuit::GatesAlong);

pub(super) fn find_gates_along(mut commands: Commands, track: Res<Track>) {
    commands.insert_resource(GatesAlong(::circuit::GatesAlong::new(&track)));
}

pub(super) fn track_progress(
    clock: Res<RaceClock>,
    settings: Res<RaceSettings>,
    track: Res<Track>,
    gates_along: Res<GatesAlong>,
    mut racers: Query<(&Transform, &mut Racer)>,
) {
    for (transform, mut racer) in &mut racers {
        let position = transform.translation;
        if let Some(last_position) = racer.last_position {
            let (from, to) = match track.gates.get(racer.progress.next_gate) {
                Some(gate) => ::circuit::beside(gate, &track.heights, last_position, position),
                None => (last_position, position),
            };
            racer
                .progress
                .advance(&track.gates, settings.laps, from, to, clock.tick);
        }
        racer.last_position = Some(position);
        racer.to_next_gate =
            gates_along
                .0
                .to_next_gate(&track, racer.progress.next_gate, position.xz());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::builtin_track;

    #[test]
    fn the_builtin_tracks_gates_go_round_its_course_once() {
        let track = builtin_track();
        let along = ::circuit::GatesAlong::new(&track);
        let course = track.course.as_ref().unwrap();
        let lap = course.length();
        let along = along.along();
        let count = along.len();
        assert_eq!(count, track.gates.len());
        let turns: f32 = (0..count)
            .map(|i| (along[(i + 1) % count] - along[i]).rem_euclid(lap))
            .sum::<f32>()
            / lap;
        assert!((turns - 1.0).abs() < 1e-3);
    }

    #[test]
    fn without_a_course_the_distance_is_a_straight_line() {
        let mut track = builtin_track();
        track.course = None;
        let along = ::circuit::GatesAlong::new(&track);
        let gate = track.gates[1].center;
        let d = along.to_next_gate(&track, 1, gate + Vec2::new(3.0, 4.0));
        assert!((d - 5.0).abs() < 1e-4);
    }
}
