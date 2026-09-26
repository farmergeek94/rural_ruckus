//! Detecting that a truck drove through a checkpoint gate.
//!
//! This is geometry on the truck's movement over one physics step rather than a physics
//! sensor: it cannot be tunnelled through at speed, it knows which way the truck was
//! going, and it needs no physics world to test.

use bevy::prelude::*;

use crate::track::Gate;

/// Whether moving in a straight line `from` one position `to` another passes through
/// the gate in its direction of travel. Height is ignored, so jumping through counts.
///
/// A movement that starts exactly on the line has already crossed it and doesn't count
/// again; one that ends exactly on the line does.
pub fn crossed(gate: &Gate, from: Vec3, to: Vec3) -> bool {
    let (from, to) = (from.xz() - gate.center, to.xz() - gate.center);

    // Signed distances in front of the gate line.
    let direction = gate.direction();
    let (before, after) = (from.dot(direction), to.dot(direction));
    if !(before < 0.0 && after >= 0.0) {
        return false;
    }

    // Where along the gate line the movement crossed it.
    let crossing = from.lerp(to, before / (before - after));
    crossing.dot(gate.across()).abs() <= gate.half_width
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use super::*;

    /// At (100, 50) on the ground plane, 20 m wide, driven through towards -Z.
    fn gate() -> Gate {
        Gate {
            center: Vec2::new(100.0, 50.0),
            yaw: 0.0,
            half_width: 10.0,
        }
    }

    fn at(x: f32, z: f32) -> Vec3 {
        Vec3::new(x, 0.0, z)
    }

    #[test]
    fn driving_through_forwards_counts() {
        assert!(crossed(&gate(), at(100.0, 51.0), at(100.0, 49.0)));
        // Diagonally, near one end.
        assert!(crossed(&gate(), at(105.0, 52.0), at(109.0, 49.0)));
    }

    #[test]
    fn height_is_ignored() {
        let (from, to) = (Vec3::new(100.0, 30.0, 51.0), Vec3::new(100.0, 35.0, 49.0));
        assert!(crossed(&gate(), from, to));
    }

    #[test]
    fn driving_through_backwards_does_not_count() {
        assert!(!crossed(&gate(), at(100.0, 49.0), at(100.0, 51.0)));
    }

    #[test]
    fn passing_outside_the_poles_does_not_count() {
        assert!(!crossed(&gate(), at(111.0, 51.0), at(111.0, 49.0)));
        assert!(!crossed(&gate(), at(89.0, 51.0), at(89.0, 49.0)));
        // Starts and ends outside, but in line with the gate's ends, on opposite sides.
        assert!(!crossed(&gate(), at(115.0, 51.0), at(125.0, 49.0)));
    }

    #[test]
    fn not_reaching_or_moving_alongside_does_not_count() {
        assert!(!crossed(&gate(), at(100.0, 55.0), at(100.0, 51.0)));
        assert!(!crossed(&gate(), at(95.0, 51.0), at(105.0, 51.0)));
        assert!(!crossed(&gate(), at(100.0, 49.0), at(100.0, 40.0)));
    }

    #[test]
    fn a_very_long_step_still_counts() {
        assert!(crossed(&gate(), at(100.0, 500.0), at(100.0, -500.0)));
    }

    #[test]
    fn stopping_on_the_line_counts_exactly_once() {
        let steps = [at(100.0, 51.0), at(100.0, 50.0), at(100.0, 49.0)];
        assert!(crossed(&gate(), steps[0], steps[1]));
        assert!(!crossed(&gate(), steps[1], steps[2]));
    }

    #[test]
    fn follows_the_gate_direction() {
        // Facing -X, as after a left turn.
        let gate = Gate {
            yaw: FRAC_PI_2,
            ..gate()
        };
        assert!(crossed(&gate, at(101.0, 50.0), at(99.0, 50.0)));
        assert!(!crossed(&gate, at(99.0, 50.0), at(101.0, 50.0)));
        assert!(!crossed(&gate, at(100.0, 51.0), at(100.0, 49.0)));
        // Its width now runs along Z.
        assert!(crossed(&gate, at(101.0, 58.0), at(99.0, 58.0)));
        assert!(!crossed(&gate, at(101.0, 62.0), at(99.0, 62.0)));
    }
}
