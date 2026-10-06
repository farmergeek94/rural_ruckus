//! How far a racer has still to drive to its next gate, measured along the course rather
//! than in a straight line, so that the race order says who is further round the track.
//!
//! Where each gate is along the course is found once a race. The nearest piece of the
//! course to a gate is not always the right one: where a course crosses itself, or runs
//! beside itself, another piece can be a metre nearer (measured: `tightcorners.pod` and
//! `landsbetween.pod` each have such gates). So every piece near a gate is a candidate, and
//! the candidates are chosen that take the gates round the course once, in order.
//!
//! A racer is looked for only on the stretch of course between its last gate and its next
//! one, which is where it is racing, so that a road that doubles back can't mislead it.

use bevy::prelude::*;

use crate::track::{Course, Nearest, TrackData};

/// How much further from a gate than its nearest piece of course another piece may be and
/// still be a candidate for where the gate is, as a share of the course's width. The right
/// piece is up to 11 m further than the nearest on `landsbetween.pod`, whose course is
/// 19.5 m wide.
const CANDIDATE_REACH: f32 = 1.0;

/// How far along the course each gate is, in metres from its first point. Empty where the
/// track has no course or no gates: then distances are straight lines.
#[derive(Resource, Default)]
pub(super) struct GatesAlong(Vec<f32>);

impl GatesAlong {
    pub(super) fn new(track: &TrackData) -> Self {
        match &track.course {
            Some(course) if !track.gates.is_empty() => {
                let centers: Vec<Vec2> = track.gates.iter().map(|gate| gate.center).collect();
                Self(gates_along(course, &centers))
            }
            _ => Self(Vec::new()),
        }
    }

    /// Metres from `position` (world X and Z) to gate `next` of `track`: along the course
    /// from the point of it nearest `position`, or in a straight line without a course.
    pub(super) fn to_next_gate(&self, track: &TrackData, next: usize, position: Vec2) -> f32 {
        let Some(gate) = track.gates.get(next) else {
            return 0.0;
        };
        match (&track.course, self.0.get(next)) {
            (Some(course), Some(&to)) => {
                let count = self.0.len();
                let from = self.0[(next + count - 1) % count];
                along_to(course, from, to, position)
            }
            _ => gate.center.distance(position),
        }
    }
}

/// Metres along `course` from `position` to the point `to` metres along it, for a racer
/// that is on the stretch from `from` to `to` metres along it. The stretch is the whole lap
/// when the two are the same: a course with one gate.
fn along_to(course: &Course, from: f32, to: f32, position: Vec2) -> f32 {
    let lap = course.length();
    let mut stretch = (to - from).rem_euclid(lap);
    if stretch == 0.0 {
        stretch = lap;
    }
    let here = course.distance_along(&course.nearest_within(position, to - stretch, to));
    let ahead = (to - here).rem_euclid(lap);
    // The search takes pieces whole, so a racer can be found a little past its gate. It has
    // not driven through it, so it is at it, not most of a lap away.
    if ahead > stretch + (lap - stretch) / 2.0 {
        0.0
    } else {
        ahead
    }
}

/// How far along `course` each of `gates` (world X and Z) is, in metres.
///
/// Of every way of choosing a candidate for each gate, the one that goes round the course
/// the fewest times, and then has its gates nearest their candidates. A way goes round once
/// more each time a gate is earlier along the course than the gate before it, the last
/// gate's next being the first.
fn gates_along(course: &Course, gates: &[Vec2]) -> Vec<f32> {
    let candidates: Vec<Vec<Candidate>> =
        gates.iter().map(|&gate| candidates(course, gate)).collect();
    // How much a way costs: how many times it goes round, and the metres from its gates to
    // their candidates.
    let cheaper = |a: &(u32, f32), b: &(u32, f32)| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1));
    let step = |from: f32, to: f32| u32::from(to < from);

    let mut best: Option<((u32, f32), Vec<f32>)> = None;
    for first in &candidates[0] {
        // The cheapest way from `first` to each candidate of the gate reached so far. A few
        // candidates for each of a few dozen gates, once a race.
        let mut ways = vec![((0, first.off), vec![first.along])];
        for gate in &candidates[1..] {
            ways = gate
                .iter()
                .map(|candidate| {
                    let ((rounds, off), way) = ways
                        .iter()
                        .map(|((rounds, off), way)| {
                            let before = *way.last().expect("a way has a gate");
                            (
                                (rounds + step(before, candidate.along), off + candidate.off),
                                way,
                            )
                        })
                        .min_by(|a, b| cheaper(&a.0, &b.0))
                        .expect("every gate has a candidate");
                    let mut way = way.clone();
                    way.push(candidate.along);
                    ((rounds, off), way)
                })
                .collect();
        }
        for ((rounds, off), way) in ways {
            let last = *way.last().expect("a way has a gate");
            let cost = (rounds + step(last, first.along), off);
            if best
                .as_ref()
                .is_none_or(|(best, _)| cheaper(&cost, best).is_lt())
            {
                best = Some((cost, way));
            }
        }
    }
    best.map_or_else(Vec::new, |(_, way)| way)
}

/// A place on the course that a gate may be at.
struct Candidate {
    /// How far along the course it is, in metres.
    along: f32,
    /// How far the gate is from it, in metres.
    off: f32,
}

/// The nearest point of each piece of `course` that is near `gate`.
fn candidates(course: &Course, gate: Vec2) -> Vec<Candidate> {
    let reach = course.nearest(gate).distance + course.width * CANDIDATE_REACH;
    (0..course.centerline().len())
        .map(|segment| {
            let nearest = nearest_on_piece(course, segment, gate);
            Candidate {
                along: course.distance_along(&nearest),
                off: nearest.distance,
            }
        })
        .filter(|candidate| candidate.off <= reach)
        .collect()
}

/// The point of piece `segment` of `course` nearest `position`.
fn nearest_on_piece(course: &Course, segment: usize, position: Vec2) -> Nearest {
    let points = course.centerline();
    let (a, b) = (points[segment], points[(segment + 1) % points.len()]);
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 {
        ((position - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Nearest {
        distance: (a + ab * t).distance(position),
        segment,
        t,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::builtin_track;

    /// A square loop 100 m a side, round from the origin: -Z, then +X, then +Z, then -X.
    fn square() -> Course {
        Course::new(
            vec![
                Vec2::ZERO,
                Vec2::new(0.0, -100.0),
                Vec2::new(100.0, -100.0),
                Vec2::new(100.0, 0.0),
            ],
            10.0,
        )
    }

    #[test]
    fn a_gate_on_the_course_is_as_far_along_as_it_is() {
        let gates = [Vec2::new(0.0, -50.0), Vec2::new(100.0, -50.0)];
        let along = gates_along(&square(), &gates);
        assert!((along[0] - 50.0).abs() < 1e-3, "{along:?}");
        assert!((along[1] - 250.0).abs() < 1e-3, "{along:?}");
    }

    #[test]
    fn where_the_course_crosses_itself_the_gates_go_round_once() {
        // A figure of eight, crossing itself at the origin: first going -Z, 50 m along, and
        // then going -X, 250 m along. 400 m in all.
        let course = Course::new(
            vec![
                Vec2::new(0.0, 50.0),
                Vec2::new(0.0, -50.0),
                Vec2::new(50.0, -50.0),
                Vec2::new(50.0, 0.0),
                Vec2::new(-50.0, 0.0),
                Vec2::new(-50.0, 50.0),
            ],
            10.0,
        );
        // Each gate at the crossing is half a metre nearer the other piece through it.
        let gates = [
            Vec2::new(0.0, 40.0),
            Vec2::new(0.5, 0.0),
            Vec2::new(50.0, -25.0),
            Vec2::new(0.0, 0.5),
            Vec2::new(-50.0, 25.0),
        ];
        let along = gates_along(&course, &gates);
        let expected = [10.0, 50.0, 175.0, 250.0, 325.0];
        for (along, expected) in along.iter().zip(expected) {
            assert!((along - expected).abs() < 1.0, "{along} for {expected}");
        }
    }

    #[test]
    fn the_distance_to_a_gate_is_along_the_course() {
        let course = square();
        // From part way up the first side to a gate part way down the third: round the
        // corners, not across the square.
        let to = 250.0;
        let from = 0.0;
        let d = along_to(&course, from, to, Vec2::new(3.0, -20.0));
        assert!((d - 230.0).abs() < 1e-3, "{d}");
        // A racer just past its gate, which it has not driven through, is at it.
        let d = along_to(&course, from, to, Vec2::new(100.0, -45.0));
        assert_eq!(d, 0.0);
        // One gate: the whole lap is the stretch.
        let d = along_to(&course, 50.0, 50.0, Vec2::new(0.0, -40.0));
        assert!((d - 10.0).abs() < 1e-3, "{d}");
        let d = along_to(&course, 50.0, 50.0, Vec2::new(0.0, -60.0));
        assert!((d - 390.0).abs() < 1e-3, "{d}");
    }

    #[test]
    fn the_builtin_tracks_gates_go_round_its_course_once() {
        let track = builtin_track();
        let along = GatesAlong::new(&track);
        let course = track.course.as_ref().unwrap();
        let lap = course.length();
        let count = along.0.len();
        assert_eq!(count, track.gates.len());
        let turns: f32 = (0..count)
            .map(|i| (along.0[(i + 1) % count] - along.0[i]).rem_euclid(lap))
            .sum::<f32>()
            / lap;
        assert!((turns - 1.0).abs() < 1e-3);
    }

    #[test]
    fn without_a_course_the_distance_is_a_straight_line() {
        let mut track = builtin_track();
        track.course = None;
        let along = GatesAlong::new(&track);
        let gate = track.gates[1].center;
        let d = along.to_next_gate(&track, 1, gate + Vec2::new(3.0, 4.0));
        assert!((d - 5.0).abs() < 1e-4);
    }
}
