//! A Summit Rumble instead of laps: on a track with `TrackData::rumble` zones, every racer
//! scores by the `rumble` crate's rules each physics tick after GO, for the session's
//! length (`RaceSettings::rumble_minutes`), and when the time is up every racer is
//! finished at once, so that the results come up and the race can be run again as a
//! circuit's can. A truck counts as on the summit, and in the scoring zone, by where its
//! middle is: which point of a truck MTM2 took is open.

use bevy::prelude::*;

use super::start::RaceStart;
use super::{RaceClock, RaceSettings, Racer};
use crate::track::Track;

/// A racer's score in a rumble. On every racer of a rumble, and on nothing else.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct RumbleScore(pub rumble::Score);

/// The session of the rumble being run. Only while one is.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct RumbleSession {
    /// How long the session lasts after GO, in physics ticks.
    pub duration: u64,
    /// Whether the time is up.
    pub over: bool,
}

impl RumbleSession {
    /// The ticks left to run at `tick` of the race clock, once the trucks have been let go
    /// at `go`; the whole session before that.
    pub fn remaining(&self, tick: u64, go: u64) -> u64 {
        self.duration.saturating_sub(tick.saturating_sub(go))
    }
}

/// Whether the race being run is a rumble.
pub fn is_rumble(track: Res<Track>) -> bool {
    track.rumble.is_some()
}

/// Begins the session.
pub(super) fn start_session(
    mut commands: Commands,
    settings: Res<RaceSettings>,
    time: Res<Time<Fixed>>,
) {
    let seconds = f64::from(settings.rumble_minutes.max(1)) * 60.0;
    commands.insert_resource(RumbleSession {
        duration: (seconds / time.timestep().as_secs_f64()).round() as u64,
        over: false,
    });
}

pub(super) fn end_session(mut commands: Commands) {
    commands.remove_resource::<RumbleSession>();
}

/// Gives each new racer its score.
pub(super) fn score_racers(mut commands: Commands, racers: Query<Entity, Added<Racer>>) {
    for racer in &racers {
        commands.entity(racer).insert(RumbleScore::default());
    }
}

/// Scores every racer for the tick, after GO, and ends the session when its time is up.
pub(super) fn score_rumble(
    track: Res<Track>,
    clock: Res<RaceClock>,
    start: Res<RaceStart>,
    time: Res<Time<Fixed>>,
    session: Option<ResMut<RumbleSession>>,
    mut racers: Query<(&Transform, &mut Racer, &mut RumbleScore)>,
) {
    let (Some(zones), Some(mut session)) = (&track.rumble, session) else {
        return;
    };
    if session.over || clock.tick < start.go {
        return;
    }
    let dt = time.timestep().as_secs_f32();
    for (transform, _, mut score) in &mut racers {
        let middle = transform.translation;
        let on_summit = rumble::on_summit(&zones.summit, middle, surface_under(&track, middle));
        let scoring = on_summit && zones.scoring.covers(middle.xz());
        score.0.step(on_summit, scoring, dt);
    }
    if session.remaining(clock.tick, start.go) == 0 {
        session.over = true;
        for (_, mut racer, _) in &mut racers {
            racer.progress.finished = Some(session.duration);
        }
    }
}

/// How much a truck's middle may be under the surface it stands on, in metres, for it to
/// be standing on that surface and not under it: a truck on a slope, or bottomed out.
const SUNK: f32 = 0.5;

/// The height of the surface a truck with its middle at `middle` stands on: the highest
/// of the ground there and the tops of the ground boxes there that is not above the
/// middle (less `SUNK`), or the ground when all are.
fn surface_under(track: &Track, middle: Vec3) -> f32 {
    let ground = track.heights.height_at(middle.x, middle.z);
    track
        .ground_boxes
        .iter()
        .filter(|b| {
            middle.x >= b.min.x && middle.x < b.max.x && middle.z >= b.min.y && middle.z < b.max.y
        })
        .map(|b| b.top)
        .chain(std::iter::once(ground))
        .filter(|&top| top <= middle.y + SUNK)
        .fold(ground.min(middle.y + SUNK), f32::max)
}

/// The place of a score among `others`, where 1 is the lead.
pub(super) fn rumble_position<'a>(
    of: &RumbleScore,
    others: impl IntoIterator<Item = &'a RumbleScore>,
) -> usize {
    rumble::position(&of.0, others.into_iter().map(|other| &other.0))
}

/// Whole points, as shown: the fraction of a second being scored is not.
pub(super) fn points_text(score: &RumbleScore) -> String {
    format!("{} pts", score.0.points.floor() as i64)
}
