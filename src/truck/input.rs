//! Keyboard and gamepad input, reduced to what the driver wants the truck to do.
//!
//! A key is either down or up, so steering from the keyboard is wound on progressively
//! while the key is held and unwinds when it is let go, as Monster Truck Madness 2's
//! did. A tap is then a small correction and only a held key reaches full lock. A
//! gamepad stick is already progressive and passes straight through.

use bevy::prelude::*;

use super::PlayerTruck;
use crate::keys::{Control, KeyBindings};

/// How fast a held key winds the steering on, in locks per second: 1.5 reaches full
/// lock from centre in 0.67 s. Higher is twitchier, lower makes hairpins hard to catch.
const KEY_STEER_RATE: f32 = 1.5;
/// How fast the steering unwinds towards centre, in locks per second, both when the
/// key is let go and when the opposite key is pressed. Faster than winding on, so that
/// the truck straightens up promptly and a swerve can be caught.
const KEY_STEER_RETURN_RATE: f32 = 5.0;

/// Driver intent, consumed by `drive_truck`. `read_input` writes the player's, and
/// whoever drives another truck writes that one's.
#[derive(Component, Default)]
pub struct TruckInput {
    /// -1 (reverse) to 1 (forward).
    pub throttle: f32,
    /// -1 (right) to 1 (left).
    pub steer: f32,
    pub handbrake: bool,
}

/// Moves the keyboard's steering from `current` towards the `target` the keys ask for
/// (-1, 0 or 1), over `dt` seconds: at the return rate while heading back to centre, and
/// at the winding rate from there outwards.
fn ramp_key_steer(current: f32, target: f32, dt: f32) -> f32 {
    let mut steer = current;
    let mut dt = dt;

    // Unwind first, if the keys ask for the centre or for the other side of it.
    if steer != 0.0 && target * steer.signum() <= 0.0 {
        let unwound = (steer.abs() / KEY_STEER_RETURN_RATE).min(dt);
        steer -= steer.signum() * KEY_STEER_RETURN_RATE * unwound;
        dt -= unwound;
        // Less than a rounding error left of it: call that centred.
        if dt > 0.0 {
            steer = 0.0;
        }
    }

    // Then wind on with whatever time is left.
    let step = KEY_STEER_RATE * dt;
    steer + (target - steer).clamp(-step, step)
}

pub(super) fn read_input(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<KeyBindings>,
    gamepads: Query<&Gamepad>,
    mut inputs: Query<&mut TruckInput, With<PlayerTruck>>,
    // Where the keyboard's steering has got to, -1 (right) to 1 (left).
    mut key_steer: Local<f32>,
) {
    let key_axis = |negative: Control, positive: Control| {
        bindings.pressed(&keys, positive) as i8 as f32
            - bindings.pressed(&keys, negative) as i8 as f32
    };

    let mut throttle = key_axis(Control::Reverse, Control::Throttle);
    *key_steer = ramp_key_steer(
        *key_steer,
        key_axis(Control::SteerRight, Control::SteerLeft),
        time.delta_secs(),
    );
    let mut steer = *key_steer;
    let mut handbrake = bindings.pressed(&keys, Control::Handbrake);

    for gamepad in &gamepads {
        throttle += gamepad.get(GamepadButton::RightTrigger2).unwrap_or(0.0)
            - gamepad.get(GamepadButton::LeftTrigger2).unwrap_or(0.0);
        // Stick right is positive, but steering left is.
        steer -= gamepad.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
        handbrake |= gamepad.pressed(GamepadButton::South);
    }

    for mut input in &mut inputs {
        input.throttle = throttle.clamp(-1.0, 1.0);
        input.steer = steer.clamp(-1.0, 1.0);
        input.handbrake = handbrake;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One physics step, which is also one frame in the headless tests.
    const DT: f32 = 1.0 / 120.0;

    fn hold(mut steer: f32, target: f32, seconds: f32) -> f32 {
        for _ in 0..(seconds / DT).round() as usize {
            steer = ramp_key_steer(steer, target, DT);
        }
        steer
    }

    #[test]
    fn a_tap_steers_a_little_and_a_held_key_reaches_lock() {
        let tap = hold(0.0, 1.0, 0.1);
        assert!((tap - 0.1 * KEY_STEER_RATE).abs() < 1e-4, "{tap}");
        assert_eq!(hold(0.0, 1.0, 2.0), 1.0);
        assert_eq!(hold(0.0, -1.0, 2.0), -1.0);
    }

    #[test]
    fn letting_go_centres_faster_than_it_wound_on() {
        let after = hold(1.0, 0.0, 0.1);
        assert!((after - (1.0 - 0.1 * KEY_STEER_RETURN_RATE)).abs() < 1e-4);
        assert_eq!(hold(1.0, 0.0, 1.0), 0.0);
        assert_eq!(hold(-1.0, 0.0, 1.0), 0.0);
    }

    #[test]
    fn the_opposite_key_unwinds_quickly_then_winds_on() {
        // One step that crosses the centre spends what is left of it winding on.
        let crossed = ramp_key_steer(0.01, -1.0, DT);
        let left_over = DT - 0.01 / KEY_STEER_RETURN_RATE;
        assert!(
            (crossed + KEY_STEER_RATE * left_over).abs() < 1e-6,
            "{crossed}"
        );
        assert_eq!(hold(1.0, -1.0, 2.0), -1.0);
    }

    #[test]
    fn it_does_not_depend_on_the_frame_rate() {
        let coarse = ramp_key_steer(ramp_key_steer(0.6, -1.0, 0.1), -1.0, 0.1);
        let fine = hold(0.6, -1.0, 0.2);
        assert!((coarse - fine).abs() < 1e-3, "{coarse} vs {fine}");
    }
}
