//! Following a path from the navigation field: where to steer at, and how fast to go.
//!
//! A driver steers at the point of the path a little way ahead of the point nearest it,
//! further the faster it goes (`Limits::look_ahead`), so that it cuts the grid's corners
//! smoothly. Its speed is what the bends within braking distance allow, worked out as the
//! game's circuit drivers do: a bend's radius sets the speed through it, and the brakes
//! set how fast the truck may be going now to be down to that by there. The end of the
//! path, the scoring zone, is a bend to be taken at `Limits::arrival`. A leap on the way
//! (`Waypoint::leap`) is the other way round: from `Limits::run_up` before its crest the
//! driver goes exactly as fast as the leap needs, whatever bends lie before it, and
//! from the crest to the landing it holds that speed and steers straight on, since
//! nothing it does in the air helps, and it has to stop where it lands: Arena Rumble's
//! platform is 39 m across. A truck in the hollow that a leap crosses is under the
//! flight, not on it: it is on its way back to the crest.

use bevy::math::Vec2;

use crate::Waypoint;

/// What a driver asks of its truck on a path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// The most it drives on a clear stretch, in m/s.
    pub cruise: f32,
    /// What it asks of its tires in a bend, in m/s².
    pub cornering: f32,
    /// The braking it counts on, in m/s².
    pub braking: f32,
    /// How slow it will go for the tightest bend, in m/s.
    pub slowest: f32,
    /// How fast it arrives at the end of the path, in m/s.
    pub arrival: f32,
    /// How many seconds up the path it steers at, and the least and most metres that is.
    pub look_ahead: f32,
    pub look: (f32, f32),
    /// From how far before a leap's crest it holds the speed the leap needs, in metres.
    pub run_up: f32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            cruise: 24.0,
            cornering: 7.0,
            braking: 5.0,
            slowest: 5.0,
            arrival: 7.0,
            look_ahead: 1.2,
            look: (8.0, 25.0),
            run_up: 60.0,
        }
    }
}

/// How far apart the path is sampled to measure its bends, and over how far a bend is
/// measured, in metres.
const STEP: f32 = 4.0;
const SPAN: f32 = 12.0;

/// What `follow` asks: where to steer at, and the speed to keep to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pursuit {
    pub aim: Vec2,
    pub speed: f32,
    /// How far ahead the crest of a leap is, in metres, when one is within
    /// `Limits::run_up`: `speed` is then what the leap needs.
    pub leap_in: Option<f32>,
}

/// How far below a crest a truck at it may be and still be taken as at it, in metres,
/// rather than in the hollow beyond it.
const AT_THE_CREST: f32 = 1.5;

/// What a truck at `position`, its middle `height` up, going at `speed` m/s does to
/// follow `path` (the places to drive through, in order). `None` for a path with no
/// places.
pub fn follow(
    path: &[Waypoint],
    position: Vec2,
    height: f32,
    speed: f32,
    limits: &Limits,
) -> Option<Pursuit> {
    let (segment, along) = nearest(path, position)?;
    // At a crest, from it to its landing: at the leap's speed, straight on. A truck under
    // the flight, in the hollow between the crest and the landing, is on the way to the
    // crest, not in the air: it is below the crest.
    let flying = segment + 1 < path.len()
        && path[segment + 1].leap > 0.0
        && along >= path[segment].at.distance(path[segment + 1].at) - 0.01
        && height >= path[segment + 1].height - AT_THE_CREST;
    if flying {
        return Some(Pursuit {
            aim: point_along(path, segment + 1, limits.look.1),
            speed: path[segment + 1].leap,
            leap_in: Some(0.0),
        });
    }
    let look = (speed.max(0.0) * limits.look_ahead).clamp(limits.look.0, limits.look.1);
    // Up to a crest, the aim is the crest itself, not a point out along the flight.
    let to_next = if segment + 1 < path.len() && path[segment + 1].leap > 0.0 {
        path[segment].at.distance(path[segment + 1].at) - along
    } else {
        f32::INFINITY
    };
    let aim = point_along(path, segment, along + look.min(to_next));
    let remaining = length_from(path, segment, along);
    let braking_distance = speed.max(0.0).powi(2) / (2.0 * limits.braking) + look;
    // The next leap on the way: how far to its crest, and what it needs.
    let leap = (segment + 1..path.len().saturating_sub(1))
        .find(|&i| path[i].leap > 0.0)
        .map(|i| (length_to(path, segment, along, i), path[i].leap));

    // The bend at each sample within braking distance, and the speed it allows now. What
    // lies beyond a crest is landed on, not driven round.
    let mut allowed = limits.cruise;
    let mut scan_to = braking_distance.min(remaining);
    if let Some((to_crest, _)) = leap {
        scan_to = scan_to.min(to_crest);
    }
    let mut distance = 0.0;
    while distance <= scan_to {
        let here = point_along(path, segment, along + distance);
        let ahead = point_along(path, segment, along + distance + SPAN);
        let before = point_along(path, segment, (along + distance - SPAN).max(along));
        let (into, out) = (here - before, ahead - here);
        if into.length() > 0.1 && out.length() > 0.1 {
            let bend = into.angle_to(out).abs();
            if bend > 1e-3 {
                let radius = SPAN / bend;
                let in_the_bend = (limits.cornering * radius).sqrt().max(limits.slowest);
                allowed =
                    allowed.min((in_the_bend.powi(2) + 2.0 * limits.braking * distance).sqrt());
            }
        }
        distance += STEP;
    }
    let mut leap_in = None;
    match leap {
        // On the run-up to a crest: just what the leap needs, whatever the bends before
        // it allow, and no more, since it has to stop after it lands.
        Some((to_crest, needs)) if to_crest <= limits.run_up => {
            allowed = needs;
            leap_in = Some(to_crest);
        }
        // The end of the path, taken at the arrival speed.
        _ if remaining <= braking_distance => {
            allowed =
                allowed.min((limits.arrival.powi(2) + 2.0 * limits.braking * remaining).sqrt());
        }
        _ => {}
    }
    Some(Pursuit {
        aim,
        speed: allowed.max(limits.slowest.min(limits.arrival)),
        leap_in,
    })
}

/// The piece of `path` nearest `position`, and how far along it the nearest point is. A
/// piece that is flown (from a place with a leap) is not driven, and is not it.
fn nearest(path: &[Waypoint], position: Vec2) -> Option<(usize, f32)> {
    if path.len() == 1 {
        return Some((0, 0.0));
    }
    path.windows(2)
        .enumerate()
        .filter(|(_, pair)| pair[0].leap == 0.0)
        .map(|(i, pair)| {
            let (a, b) = (pair[0].at, pair[1].at);
            let piece = b - a;
            let length = piece.length();
            let t = if length > 0.0 {
                ((position - a).dot(piece) / (length * length)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            ((position - (a + piece * t)).length(), (i, t * length))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, found)| found)
}

/// The point `distance` metres along `path` from the start of piece `segment`, held at the
/// path's end.
fn point_along(path: &[Waypoint], segment: usize, distance: f32) -> Vec2 {
    let mut left = distance.max(0.0);
    for i in segment..path.len().saturating_sub(1) {
        let (a, b) = (path[i].at, path[i + 1].at);
        let length = a.distance(b);
        if left <= length {
            return if length > 0.0 {
                a.lerp(b, left / length)
            } else {
                a
            };
        }
        left -= length;
    }
    path.last().expect("a path has places").at
}

/// How much of `path` is left from `along` metres into piece `segment`.
fn length_from(path: &[Waypoint], segment: usize, along: f32) -> f32 {
    length_to(path, segment, along, path.len().saturating_sub(1))
}

/// How far it is from `along` metres into piece `segment` to place `index` of `path`.
fn length_to(path: &[Waypoint], segment: usize, along: f32, index: usize) -> f32 {
    let rest: f32 = (segment..index)
        .map(|i| path[i].at.distance(path[i + 1].at))
        .sum();
    (rest - along).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn driven(places: impl IntoIterator<Item = Vec2>) -> Vec<Waypoint> {
        places
            .into_iter()
            .map(|at| Waypoint {
                at,
                height: 0.0,
                leap: 0.0,
            })
            .collect()
    }

    /// Straight along +X for 100 m, then a right-angle turn to +Z for 100 m.
    fn corner() -> Vec<Waypoint> {
        let mut path: Vec<Vec2> = (0..=25).map(|i| Vec2::new(i as f32 * 4.0, 0.0)).collect();
        path.extend((1..=25).map(|i| Vec2::new(100.0, i as f32 * 4.0)));
        driven(path)
    }

    #[test]
    fn on_a_straight_it_steers_ahead_and_cruises() {
        let limits = Limits::default();
        let got = follow(&corner(), Vec2::new(10.0, 3.0), 0.0, 10.0, &limits).unwrap();
        assert!(
            (got.aim.x - 22.0).abs() < 0.5 && got.aim.y.abs() < 0.01,
            "{got:?}"
        );
        assert_eq!(got.speed, limits.cruise);
    }

    #[test]
    fn it_slows_for_the_corner_and_more_the_nearer_it_is() {
        let limits = Limits::default();
        let far = follow(&corner(), Vec2::new(20.0, 0.0), 0.0, 24.0, &limits).unwrap();
        let near = follow(&corner(), Vec2::new(80.0, 0.0), 0.0, 24.0, &limits).unwrap();
        assert!(near.speed < far.speed, "{near:?} {far:?}");
        assert!(
            near.speed < 20.0 && near.speed >= limits.slowest,
            "{near:?}"
        );
    }

    #[test]
    fn it_arrives_at_the_end_slowly() {
        let limits = Limits::default();
        let path = driven((0..=10).map(|i| Vec2::new(i as f32 * 4.0, 0.0)));
        let got = follow(&path, Vec2::new(36.0, 0.0), 0.0, 20.0, &limits).unwrap();
        assert!(got.speed < 10.0, "{got:?}");
        assert_eq!(got.aim, Vec2::new(40.0, 0.0), "steers at the end");
        assert!(follow(&[], Vec2::ZERO, 0.0, 0.0, &limits).is_none());
    }

    #[test]
    fn a_leap_is_run_up_to_fast_and_flown_flat_out() {
        let limits = Limits::default();
        // Straight along +X: a crest at x = 40 that needs 18 m/s, landing at x = 52, and
        // the end 8 m on, which would otherwise mean arriving slowly.
        let mut path = driven((0..=10).map(|i| Vec2::new(i as f32 * 4.0, 0.0)));
        path[10].leap = 18.0;
        path.push(Waypoint {
            at: Vec2::new(52.0, 0.0),
            height: 0.0,
            leap: 0.0,
        });
        path.extend(driven([Vec2::new(56.0, 0.0), Vec2::new(60.0, 0.0)]));
        let run_up = follow(&path, Vec2::new(20.0, 0.0), 0.0, 5.0, &limits).unwrap();
        assert_eq!(run_up.speed, 18.0, "{run_up:?}");
        // With a tight bend just before the crest, still what the leap needs.
        let mut bent = path.clone();
        bent[8].at.y = 6.0;
        let run_up = follow(&bent, Vec2::new(20.0, 0.0), 0.0, 5.0, &limits).unwrap();
        assert_eq!(run_up.speed, 18.0, "{run_up:?}");
        let flying = follow(&path, Vec2::new(45.0, 0.0), 0.0, 18.0, &limits).unwrap();
        assert_eq!(flying.speed, 18.0, "{flying:?}");
        assert_eq!(flying.aim, Vec2::new(60.0, 0.0), "straight on");
        assert_eq!(flying.leap_in, Some(0.0));
        assert!(
            run_up.leap_in.is_some_and(|to_crest| to_crest > 20.0),
            "{run_up:?}"
        );
        // A truck in the hollow under the flight, 4 m to the side of the line, is on its
        // way to the crest, not flying: it steers at the crest and holds the leap's speed.
        let under = follow(&path, Vec2::new(46.0, 4.0), -5.0, 5.0, &limits).unwrap();
        assert!(under.aim.x <= 40.0 && under.aim.y == 0.0, "{under:?}");
        assert_eq!(under.speed, 18.0);
        // Landed, it slows for the end as usual.
        let landed = follow(&path, Vec2::new(53.0, 0.0), 0.0, 18.0, &limits).unwrap();
        assert!(landed.speed < 12.0, "{landed:?}");
        // Far from the crest, the bends before it still rule.
        let mut corner = corner();
        corner[40].leap = 18.0;
        let before = follow(&corner, Vec2::new(80.0, 0.0), 0.0, 24.0, &limits).unwrap();
        assert!(before.speed < 18.0, "{before:?}");
    }
}
