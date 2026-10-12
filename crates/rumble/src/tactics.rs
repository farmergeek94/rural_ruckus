//! What a computer truck does on the summit: whom to push off, how to hit it, how fast it
//! may go without going over the rim itself, where to wait, and when to get out of the way
//! of a truck that is about to hit it. Pure functions over what the truck sees. The game's
//! own: Monster Truck Madness 2 drove no computer trucks in a rumble.

use bevy::math::Vec2;

/// The truck that is deciding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Me {
    pub position: Vec2,
    /// A unit vector along its nose.
    pub forward: Vec2,
    /// Along its nose, in m/s.
    pub speed: f32,
    /// Whether it is earning points: on the summit and in the scoring zone.
    pub scoring: bool,
}

impl Me {
    fn velocity(&self) -> Vec2 {
        self.forward * self.speed
    }
}

/// Another truck, as the tactics see it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rival {
    pub position: Vec2,
    pub velocity: Vec2,
    pub on_summit: bool,
    /// Whether it is earning points.
    pub scoring: bool,
    /// Its score.
    pub points: f32,
}

/// What a driver is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    /// Off the summit: driving up it.
    Climb,
    /// On the summit, after rival number `0`.
    Fight(usize),
    /// On the summit with no one to push: waiting in the scoring zone.
    Hold,
    /// Getting out of the way of a truck about to hit it.
    Dodge(Dodge),
}

/// Which way to get out of the way: a unit vector on the ground plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dodge {
    pub direction: Vec2,
}

/// How near a rival has to be for its nearness to count for much, in metres.
const NEAR: f32 = 15.0;
/// What hitting a rival from the inside is worth, so that it goes outward; what a rival
/// that is earning points is worth; what a rival near the rim is worth.
const OUTWARD_WORTH: f32 = 0.8;
const SCORING_WORTH: f32 = 0.5;
const RIM_WORTH: f32 = 0.5;
/// What the rival with the most points is worth, over everything else together, so that
/// the leader is the one pushed off; and how far behind the leader, in points, a rival is
/// still worth a share of that: ten seconds of scoring.
const LEADER_WORTH: f32 = 3.0;
const LEAD_SPAN: f32 = 100.0;
/// How far inside a rival, on the summit's middle side, a driver aims, in metres: past
/// its middle, so that the hit carries it outward.
const INSIDE: f32 = 1.5;
/// How fast a driver counts on closing a gap when it leads a moving rival, in m/s, at
/// least.
const CLOSING: f32 = 8.0;
/// A truck that will pass within this of the driver's middle is going to hit it, in
/// metres; one closing slower than this, in m/s, is not worth dodging; and one that will
/// hit sooner than this, in seconds, is dodged now.
const MISS: f32 = 3.5;
const DODGE_SPEED: f32 = 7.0;
const DODGE_TIME: f32 = 1.2;
/// How near the middle of the scoring zone a holding driver keeps, in metres, and how far
/// ahead it steers while it creeps round there.
const HOLD_RADIUS: f32 = 6.0;
const CREEP: f32 = 6.0;
/// How far to either side of a driver's nose a rival may be and still be hit square, in
/// metres: about a truck's width.
const LANE: f32 = 2.5;
/// How far to either side of a driver's nose a rival may be and still be the one it is
/// pushing, in metres: wider than `LANE`, since a push lands on a corner of it too.
const PUSH_LANE: f32 = 3.5;
/// A driver that is not earning points fights only a rival within this, in metres;
/// otherwise it goes back to the scoring zone. A fight drifts, and on Arena Rumble it
/// drifted the whole field into a corner of the platform, outside the zone, where it
/// stood for a minute.
const ENGAGE: f32 = 12.0;

/// The rival worth going after, from `rivals`, for a driver `me` on a summit whose middle
/// is `centre` and whose rim is about `reach` metres from it: the one with the most
/// points, or near them, before all else; then the nearest, the more for one it can hit
/// from the inside, one that is earning points, and one near the rim. `None` when no
/// rival is on the summit.
pub fn choose_target(me: &Me, rivals: &[Rival], centre: Vec2, reach: f32) -> Option<usize> {
    let up = |rival: &&Rival| rival.on_summit;
    let top = rivals
        .iter()
        .filter(up)
        .map(|rival| rival.points)
        .fold(f32::NEG_INFINITY, f32::max);
    rivals
        .iter()
        .enumerate()
        .filter(|(_, rival)| up(rival))
        .map(|(index, rival)| {
            let to_rival = rival.position - me.position;
            let near = 1.0 / (1.0 + to_rival.length() / NEAR);
            let out = (rival.position - centre).normalize_or_zero();
            let outward = to_rival.normalize_or_zero().dot(out).max(0.0);
            let rim = ((rival.position - centre).length() / reach.max(1.0)).min(1.0);
            let leading = 1.0 - ((top - rival.points) / LEAD_SPAN).min(1.0);
            let worth = LEADER_WORTH * leading
                + near
                + OUTWARD_WORTH * outward
                + SCORING_WORTH * f32::from(rival.scoring)
                + RIM_WORTH * rim;
            (index, worth)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(index, _)| index)
}

/// Where a driver `me` steers at to hit `rival` so that it goes outward from `centre`: a
/// point just inside it, where it will be by the time the driver gets there.
pub fn attack_aim(me: &Me, rival: &Rival, centre: Vec2) -> Vec2 {
    let gap = rival.position - me.position;
    let lead = gap.length() / me.speed.max(CLOSING);
    let there = rival.position + rival.velocity * lead;
    let out = (there - centre).normalize_or_zero();
    there - out * INSIDE
}

/// The fastest a driver may go now, in m/s, and still stop in `run_off` metres less a
/// `margin`, braking at `braking` m/s².
pub fn edge_speed(run_off: f32, braking: f32, margin: f32) -> f32 {
    (2.0 * braking * (run_off - margin).max(0.0)).sqrt()
}

/// How far ahead of `me`, along its nose, `rival` is, when it is within `LANE` of the
/// nose's line and ahead: where it will be hit square. `None` when it is not.
pub fn ahead_in_lane(me: &Me, rival: &Rival) -> Option<f32> {
    let gap = rival.position - me.position;
    let ahead = gap.dot(me.forward);
    (ahead > 0.0 && gap.perp_dot(me.forward).abs() <= LANE).then_some(ahead)
}

/// How far ahead of `me`, along its nose, `rival` is while `me` is pushing it: ahead,
/// within `contact` metres (about a truck's length and a little, so that the two touch
/// or are about to), and within `PUSH_LANE` of the nose's line. `None` when it is not.
pub fn pushing(me: &Me, rival: &Rival, contact: f32) -> Option<f32> {
    let gap = rival.position - me.position;
    let ahead = gap.dot(me.forward);
    (ahead > 0.0 && ahead <= contact && gap.perp_dot(me.forward).abs() <= PUSH_LANE)
        .then_some(ahead)
}

/// The fastest a driver may go at a rival `to_rival` metres straight ahead of it, in m/s,
/// with `run_off` metres before its own rim: a hit between equals leaves it with half its
/// speed, so it may arrive at twice what it could stop from in what is left of the run
/// after the hit, and never less than `edge_speed` lets it go anyway.
pub fn charge_speed(run_off: f32, to_rival: f32, braking: f32, margin: f32) -> f32 {
    edge_speed(run_off, braking, margin).max(2.0 * edge_speed(run_off - to_rival, braking, margin))
}

/// A truck about to hit `me`, if there is one: a rival closing at more than `DODGE_SPEED`
/// that will pass within `MISS` metres within `DODGE_TIME` seconds. The way out is across
/// its path, to the side the driver is already on.
pub fn threat(me: &Me, rivals: &[Rival]) -> Option<Dodge> {
    rivals
        .iter()
        .filter_map(|rival| {
            let relative = rival.velocity - me.velocity();
            let gap = me.position - rival.position;
            let closing = relative.dot(gap.normalize_or_zero());
            if closing < DODGE_SPEED {
                return None;
            }
            let speed2 = relative.length_squared();
            let when = relative.dot(gap) / speed2;
            if !(0.0..=DODGE_TIME).contains(&when) {
                return None;
            }
            let miss = gap - relative * when;
            if miss.length() > MISS {
                return None;
            }
            let across = relative.perp().normalize_or_zero();
            let side = if miss.dot(across) >= 0.0 { 1.0 } else { -1.0 };
            Some((
                when,
                Dodge {
                    direction: across * side,
                },
            ))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, dodge)| dodge)
}

/// Where a driver with no one to push steers at: the middle of the scoring zone, `centre`,
/// from further than `HOLD_RADIUS` away; nearer than that, a short way towards the nearest
/// rival, so that its nose meets whoever comes. `None` when it is near the middle and
/// alone: then it stops.
pub fn hold_aim(me: &Me, rivals: &[Rival], centre: Vec2) -> Option<Vec2> {
    if me.position.distance(centre) > HOLD_RADIUS {
        return Some(centre);
    }
    let nearest = rivals.iter().min_by(|a, b| {
        a.position
            .distance_squared(me.position)
            .total_cmp(&b.position.distance_squared(me.position))
    })?;
    let towards = (nearest.position - me.position).normalize_or_zero();
    Some(me.position + towards * CREEP)
}

/// What a driver does: climbs while it is off the summit; on it, dodges a truck about to
/// hit it when it `dodges` at all, else fights the rival worth going after, if it is
/// earning points or the rival is within `ENGAGE`, else holds the scoring zone.
pub fn decide(
    me: &Me,
    on_summit: bool,
    rivals: &[Rival],
    centre: Vec2,
    reach: f32,
    dodges: bool,
) -> Mode {
    if !on_summit {
        return Mode::Climb;
    }
    if dodges && let Some(dodge) = threat(me, rivals) {
        return Mode::Dodge(dodge);
    }
    match choose_target(me, rivals, centre, reach) {
        Some(target) if me.scoring || rivals[target].position.distance(me.position) <= ENGAGE => {
            Mode::Fight(target)
        }
        _ => Mode::Hold,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn me_at(x: f32, z: f32, forward: Vec2, speed: f32) -> Me {
        Me {
            position: Vec2::new(x, z),
            forward,
            speed,
            scoring: true,
        }
    }

    fn rival_at(x: f32, z: f32, velocity: Vec2, on_summit: bool) -> Rival {
        Rival {
            position: Vec2::new(x, z),
            velocity,
            on_summit,
            scoring: on_summit,
            points: 0.0,
        }
    }

    #[test]
    fn a_rival_just_ahead_and_near_the_nose_line_is_being_pushed() {
        let me = me_at(0.0, 0.0, Vec2::NEG_Y, 3.0);
        let contact = 6.0;
        // A truck's length ahead, a little to one side: pushed.
        let ahead = rival_at(2.0, -4.0, Vec2::ZERO, true);
        assert_eq!(pushing(&me, &ahead, contact), Some(4.0));
        // Too far ahead, too far aside, or behind: not.
        assert_eq!(pushing(&me, &rival_at(0.0, -8.0, Vec2::ZERO, true), contact), None);
        assert_eq!(pushing(&me, &rival_at(4.0, -4.0, Vec2::ZERO, true), contact), None);
        assert_eq!(pushing(&me, &rival_at(0.0, 4.0, Vec2::ZERO, true), contact), None);
    }

    #[test]
    fn the_leader_is_the_target_before_a_nearer_rival() {
        let me = me_at(0.0, 0.0, Vec2::X, 5.0);
        let centre = Vec2::new(-20.0, 0.0);
        let leader = Rival {
            points: 500.0,
            ..rival_at(-30.0, 10.0, Vec2::ZERO, true)
        };
        let near = rival_at(12.0, 0.0, Vec2::ZERO, true);
        assert_eq!(choose_target(&me, &[near, leader], centre, 30.0), Some(1));
        // A second of scoring behind: the easier hit.
        let close_behind = Rival {
            points: 490.0,
            ..near
        };
        assert_eq!(
            choose_target(&me, &[close_behind, leader], centre, 30.0),
            Some(0)
        );
        // The leader off the summit is no target.
        let down = Rival {
            on_summit: false,
            ..leader
        };
        assert_eq!(choose_target(&me, &[near, down], centre, 30.0), Some(0));
    }

    #[test]
    fn the_target_is_on_the_summit_and_best_hit_from_the_inside() {
        let me = me_at(0.0, 0.0, Vec2::X, 5.0);
        let rivals = [
            // Off the summit, however near.
            rival_at(3.0, 0.0, Vec2::ZERO, false),
            // On the summit, outward from the driver: hit from the inside.
            rival_at(12.0, 0.0, Vec2::ZERO, true),
            // On the summit, the same way off, but between the driver and the middle.
            rival_at(-12.0, 0.0, Vec2::ZERO, true),
        ];
        let centre = Vec2::new(-20.0, 0.0);
        assert_eq!(choose_target(&me, &rivals, centre, 30.0), Some(1));
        assert_eq!(choose_target(&me, &rivals[..1], centre, 30.0), None);
    }

    #[test]
    fn the_hit_is_aimed_just_inside_a_moving_rival() {
        let me = me_at(0.0, 0.0, Vec2::X, 10.0);
        let rival = rival_at(20.0, 0.0, Vec2::new(0.0, 5.0), true);
        let centre = Vec2::new(10.0, -10.0);
        let aim = attack_aim(&me, &rival, centre);
        // Led 2 s along its way, then pulled back towards the middle.
        let there = Vec2::new(20.0, 10.0);
        let inward = (centre - there).normalize();
        assert!((aim - (there + inward * INSIDE)).length() < 1e-3, "{aim}");
    }

    #[test]
    fn the_rim_caps_the_speed() {
        assert_eq!(edge_speed(2.0, 5.0, 3.0), 0.0);
        let far = edge_speed(43.0, 5.0, 3.0);
        assert!((far - 20.0).abs() < 1e-3, "{far}");
    }

    #[test]
    fn a_rival_in_the_lane_may_be_charged_at_twice_the_stopping_speed() {
        let me = me_at(0.0, 0.0, Vec2::X, 10.0);
        assert_eq!(
            ahead_in_lane(&me, &rival_at(12.0, 2.0, Vec2::ZERO, true)),
            Some(12.0)
        );
        assert!(ahead_in_lane(&me, &rival_at(12.0, 4.0, Vec2::ZERO, true)).is_none());
        assert!(ahead_in_lane(&me, &rival_at(-5.0, 0.0, Vec2::ZERO, true)).is_none());
        // 12 m to the rival, 23 m to the rim: 11 m left after the hit, less 3, at 5 m/s²
        // stops from 8.9, so it may hit at 17.9; alone it could only do 14.1.
        let charge = charge_speed(23.0, 12.0, 5.0, 3.0);
        assert!((charge - 17.9).abs() < 0.1, "{charge}");
        // A rival at the very rim is hit no faster than the rim allows.
        assert!((charge_speed(23.0, 22.0, 5.0, 3.0) - 14.1).abs() < 0.1);
    }

    #[test]
    fn a_truck_about_to_hit_is_dodged_to_the_side_already_taken() {
        let me = me_at(0.0, 0.0, Vec2::X, 0.0);
        // Coming from the +X side at 12 m/s, 1 m to the driver's +Z side.
        let charging = rival_at(10.0, 1.0, Vec2::new(-12.0, 0.0), true);
        let dodge = threat(&me, &[charging]).expect("a threat");
        assert!(dodge.direction.y < -0.99, "{dodge:?}");
        // The same truck going slowly, or going away, is no threat.
        assert!(threat(&me, &[rival_at(10.0, 1.0, Vec2::new(-5.0, 0.0), true)]).is_none());
        assert!(threat(&me, &[rival_at(10.0, 1.0, Vec2::new(12.0, 0.0), true)]).is_none());
        // One that will pass wide of the driver is not either.
        assert!(threat(&me, &[rival_at(10.0, 6.0, Vec2::new(-12.0, 0.0), true)]).is_none());
    }

    #[test]
    fn holding_goes_to_the_middle_and_then_faces_the_nearest() {
        let centre = Vec2::ZERO;
        let far = me_at(20.0, 0.0, Vec2::X, 0.0);
        assert_eq!(hold_aim(&far, &[], centre), Some(centre));
        let there = me_at(1.0, 0.0, Vec2::X, 0.0);
        assert_eq!(hold_aim(&there, &[], centre), None);
        let rivals = [
            rival_at(1.0, 30.0, Vec2::ZERO, false),
            rival_at(60.0, 0.0, Vec2::ZERO, false),
        ];
        let aim = hold_aim(&there, &rivals, centre).unwrap();
        assert!((aim - Vec2::new(1.0, CREEP)).length() < 1e-3, "{aim}");
    }

    #[test]
    fn the_mode_follows_the_situation() {
        let me = me_at(0.0, 0.0, Vec2::X, 0.0);
        let centre = Vec2::ZERO;
        assert_eq!(decide(&me, false, &[], centre, 30.0, true), Mode::Climb);
        assert_eq!(decide(&me, true, &[], centre, 30.0, true), Mode::Hold);
        let charging = rival_at(10.0, 1.0, Vec2::new(-12.0, 0.0), true);
        assert!(matches!(
            decide(&me, true, &[charging], centre, 30.0, true),
            Mode::Dodge(_)
        ));
        assert_eq!(
            decide(&me, true, &[charging], centre, 30.0, false),
            Mode::Fight(0)
        );
        // Earning nothing, it fights only a rival near it, and otherwise goes back to the
        // zone.
        let outside = Me {
            scoring: false,
            ..me
        };
        let far = rival_at(20.0, 0.0, Vec2::ZERO, true);
        assert_eq!(
            decide(&outside, true, &[far], centre, 30.0, false),
            Mode::Hold
        );
        assert_eq!(
            decide(&outside, true, &[charging], centre, 30.0, false),
            Mode::Fight(0)
        );
    }
}
