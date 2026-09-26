//! Following a moving thing without ever catching it up with a jerk: a critically damped
//! spring, solved exactly.
//!
//! The follower is pulled towards its target by a spring and held back by a damper that is
//! just strong enough to stop it overshooting. Both work on the *difference* between the
//! follower and the target, in position and in velocity, so a target moving steadily is
//! followed with no lag at all, where a plain lag trails it by `speed / stiffness`.
//!
//! A frame is solved in closed form rather than stepped, so the answer is the same for one
//! long frame as for two short ones, and no frame is long enough to make it unstable.

use std::f32::consts::{PI, TAU};

/// Where a follower at `position` moving at `velocity` is after `seconds`, and how fast it
/// is going, given that its target is at `target` at the end of that time and has been
/// moving at `target_velocity` throughout.
///
/// `stiffness` is per second: the gap left by a sudden move of the target is down to about
/// a tenth after `4 / stiffness` seconds. Nothing but `seconds` has a unit of its own, so
/// this serves metres and radians alike.
pub fn follow(
    (position, velocity): (f32, f32),
    (target, target_velocity): (f32, f32),
    stiffness: f32,
    seconds: f32,
) -> (f32, f32) {
    // The gap at the start of the frame, the target having been `seconds` further back.
    let gap = position - (target - target_velocity * seconds);
    let (gap, gap_rate) = close((gap, velocity - target_velocity), stiffness, seconds);
    (target + gap, target_velocity + gap_rate)
}

/// The same for an angle in radians, going the short way round to a target that stands
/// still.
pub fn follow_angle(
    (angle, rate): (f32, f32),
    target: f32,
    stiffness: f32,
    seconds: f32,
) -> (f32, f32) {
    let (gap, rate) = close((wrap(angle - target), rate), stiffness, seconds);
    (wrap(target + gap), rate)
}

/// A gap and its rate of change, `seconds` later: the solution of
/// `gap'' = -2 * stiffness * gap' - stiffness^2 * gap`.
fn close((gap, rate): (f32, f32), stiffness: f32, seconds: f32) -> (f32, f32) {
    let decay = (-stiffness * seconds).exp();
    let k = rate + stiffness * gap;
    (
        (gap + k * seconds) * decay,
        (rate - k * stiffness * seconds) * decay,
    )
}

/// The same angle, between -pi and pi.
pub fn wrap(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f32 = 1.0 / 60.0;

    #[test]
    fn a_steady_target_is_followed_with_no_lag() {
        // Already on it and going its way: it stays on it, however fast that is.
        let (mut follower, speed) = ((0.0, 30.0), 30.0);
        for frame in 1..=600 {
            follower = follow(follower, (speed * FRAME * frame as f32, speed), 10.0, FRAME);
        }
        assert!((follower.0 - speed * 10.0).abs() < 1e-3, "{follower:?}");
        assert!((follower.1 - speed).abs() < 1e-4);
    }

    #[test]
    fn a_gap_closes_without_overshoot() {
        let mut follower = (-2.0, 0.0);
        let mut last_gap = -2.0;
        for _ in 0..600 {
            follower = follow(follower, (0.0, 0.0), 10.0, FRAME);
            // Never past the target, and never further away than it was.
            assert!(follower.0 <= 0.0 && follower.0 >= last_gap, "{follower:?}");
            last_gap = follower.0;
        }
        assert!(follower.0.abs() < 1e-4);

        // A tenth of it is left after about 4 / stiffness seconds.
        let (gap, _) = follow((-2.0, 0.0), (0.0, 0.0), 10.0, 0.4);
        assert!((0.15..0.25).contains(&-gap), "{gap}");
    }

    #[test]
    fn it_starts_gently() {
        // A plain lag is at full speed the moment its target moves. This is not.
        let (_, velocity) = follow((0.0, 0.0), (1.0, 0.0), 10.0, 0.001);
        assert!(velocity < 0.11, "{velocity}");
    }

    #[test]
    fn one_long_frame_is_two_short_ones() {
        let target_at = |seconds: f32| (3.0 + 12.0 * seconds, 12.0);
        let start = (1.0, -4.0);
        let once = follow(start, target_at(0.1), 7.0, 0.1);
        let twice = follow(
            follow(start, target_at(0.04), 7.0, 0.04),
            target_at(0.1),
            7.0,
            0.06,
        );
        assert!((once.0 - twice.0).abs() < 1e-5, "{once:?} {twice:?}");
        assert!((once.1 - twice.1).abs() < 1e-4, "{once:?} {twice:?}");
    }

    #[test]
    fn it_agrees_with_the_equation_stepped_finely() {
        let (stiffness, mut position, mut velocity) = (6.0_f32, 1.5_f32, -2.0_f32);
        let steps = 100_000;
        let dt = 0.5 / steps as f32;
        for _ in 0..steps {
            let acceleration = -2.0 * stiffness * velocity - stiffness * stiffness * position;
            velocity += acceleration * dt;
            position += velocity * dt;
        }
        let exact = follow((1.5, -2.0), (0.0, 0.0), stiffness, 0.5);
        assert!((exact.0 - position).abs() < 1e-3, "{exact:?} {position}");
        assert!((exact.1 - velocity).abs() < 1e-3, "{exact:?} {velocity}");
    }

    #[test]
    fn a_frame_of_no_length_changes_nothing() {
        assert_eq!(follow((1.0, 2.0), (5.0, 3.0), 10.0, 0.0), (1.0, 2.0));
    }

    #[test]
    fn angles_go_the_short_way_round() {
        // From just short of half a turn one way to just short of it the other.
        let (mut angle, mut rate) = (3.0, 0.0);
        let mut travelled = 0.0;
        for _ in 0..600 {
            let before = angle;
            (angle, rate) = follow_angle((angle, rate), -3.0, 5.0, FRAME);
            travelled += wrap(angle - before).abs();
        }
        assert!(wrap(angle + 3.0).abs() < 1e-3, "{angle}");
        assert!((travelled - (TAU - 6.0)).abs() < 1e-2, "{travelled}");
    }
}
