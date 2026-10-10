//! The gearbox: three forward gears, neutral and a reverse, changed by itself
//! (`Transmission::Automatic`, the default) or, on the player's truck, by the player
//! (`Transmission::Manual`).
//!
//! The gearbox chooses the gear, runs the shift timer, and decides how the clutch couples
//! the engine to the wheels (`ClutchMode`). The engine itself -- its speed, its torque, its
//! governors, the clutch's slip and the stall -- is in `engine`, which reads the gear and
//! the mode from here. Each gear meets the governed engine speed at its share of the top
//! speed (`GEAR_TOPS`); the automatic changes up before the governor and down before the
//! engine lugs, with a short pause at each change (`SHIFT_TIME`) while the clutch is open.
//!
//! Only the lowest gears (`STARTING_GEARS`) and reverse start the truck from a standstill:
//! in them, and in the automatic, the clutch is a torque converter (`ClutchMode::Converter`),
//! which slips as the engine revs and cannot stall it. A manual driver who presses the
//! throttle in a higher gear lets the clutch out (`ClutchMode::Manual`): the engine is
//! dragged to the wheels' speed, and stalls if that is too low. It runs again when the
//! throttle is let go or the gear is changed (see `engine`).
//!
//! Reverse goes in by itself in either mode, when the brake key is held with the truck at a
//! stop, and out again when the throttle is: the manual changes the forward gears and
//! neutral only. Neutral is below first, and only the manual uses it. In neutral the clutch
//! is open and the throttle revs the engine freely.
//!
//! Three forward gears are the user's choice (there were five at first). What gears MTM2
//! had is not confirmed; its `SOUND.POD` holds `2NDGEAR.WAV` and `3RDGEAR.WAV`, and no
//! sound for a fourth or fifth gear (see `docs/formats/sound.md`).

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;

use super::{Autopilot, PlayerTruck, Transmission, Truck, TruckConfig, TruckInput};

/// How many forward gears there are.
pub const FORWARD_GEARS: u8 = 3;

/// The speed at which each forward gear meets the engine's governed speed, as a share of
/// the truck's top speed, first gear first. Closer together, the automatic changes more
/// often and a manual gear suits a narrower range of speeds. A first guess, which wants
/// driving.
pub(super) const GEAR_TOPS: [f32; FORWARD_GEARS as usize] = [0.35, 0.65, 1.0];

/// The same for reverse.
const REVERSE_TOP: f32 = GEAR_TOPS[0];

/// How many of the lowest forward gears start the truck from a standstill, through the
/// torque converter, as reverse does: they never lug nor stall. Higher lets a manual driver
/// pull away in a higher gear.
const STARTING_GEARS: u8 = 2;

/// The engine speed, as a share of the governed speed, at which the automatic changes up.
/// Higher holds each gear longer. Coupled with the engine's `governor_band_rpm`: at 0.92 of
/// 3200 rpm the change is made at 2944 rpm, just under the band where the governor cuts the
/// fuel, so the automatic never meets the governor.
pub(super) const SHIFT_UP: f32 = 0.92;

/// The engine speed that the gear below would have, as a share of its governed speed,
/// under which the automatic changes down. It must be under `SHIFT_UP`, or the automatic
/// would change straight back up. Higher changes down sooner.
const SHIFT_DOWN: f32 = 0.8;

/// How long a change of gear takes, in seconds, while the clutch is open and the engine
/// pushes nothing. Longer costs more speed at each change.
const SHIFT_TIME: f32 = 0.1;

/// How slowly the truck must go, in m/s, for reverse to go in, or to come out. As `drive`
/// has it, the throttle against the way the truck goes faster than this is the brakes.
const STOPPED: f32 = 0.5;

/// A gear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gear {
    Reverse,
    Neutral,
    /// From 1 to `FORWARD_GEARS`.
    Forward(u8),
}

impl std::fmt::Display for Gear {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Gear::Reverse => write!(f, "R"),
            Gear::Neutral => write!(f, "N"),
            Gear::Forward(gear) => write!(f, "{gear}"),
        }
    }
}

/// How the clutch couples the engine to the wheels this step. `engine` reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum ClutchMode {
    /// Nothing couples them: neutral, or a change of gear under way.
    Open,
    /// A torque converter: it slips while the engine revs up from idle, holds once the
    /// engine is turning, and can never stall the engine. The automatic, every starting
    /// gear and reverse, and a high gear with no throttle (coasting).
    #[default]
    Converter,
    /// A driver's clutch, let out in a high gear with the throttle down: the engine is
    /// dragged to the wheels' speed, and stalls if that is too low.
    Manual,
}

/// On a truck: its gearbox, as it stands after the last physics step.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Gearbox {
    gear: Gear,
    /// How long the change of gear under way has left to take, in seconds.
    shifting: f32,
    /// How the clutch couples the engine to the wheels this step.
    pub(super) clutch_mode: ClutchMode,
}

impl Default for Gearbox {
    fn default() -> Self {
        Self {
            gear: Gear::Forward(1),
            shifting: 0.0,
            clutch_mode: ClutchMode::Converter,
        }
    }
}

impl Gearbox {
    /// The gear it is in.
    pub fn gear(&self) -> Gear {
        self.gear
    }

    /// Whether a change of gear is under way: the clutch is open and the engine lifts.
    pub fn shifting(&self) -> bool {
        self.shifting > 0.0
    }
}

/// Whether `gear` starts the truck from a standstill, through the converter.
fn starts(gear: Gear) -> bool {
    match gear {
        Gear::Reverse | Gear::Neutral => true,
        Gear::Forward(gear) => gear <= STARTING_GEARS,
    }
}

/// The speed at which `gear` meets the governed engine speed, in m/s, on a truck whose top
/// speed is `top_speed`. 0 in neutral, where nothing turns the engine with the wheels.
pub(super) fn gear_top(gear: Gear, top_speed: f32) -> f32 {
    match gear {
        Gear::Reverse => REVERSE_TOP * top_speed,
        Gear::Neutral => 0.0,
        Gear::Forward(gear) => GEAR_TOPS[usize::from(gear.clamp(1, FORWARD_GEARS)) - 1] * top_speed,
    }
}

/// The forward gear the automatic puts a truck in at `speed`, in m/s either way, from
/// `gear`: up past `SHIFT_UP` of each gear's top, and down under `SHIFT_DOWN` of the top of
/// the gear below. Several at once, if the speed calls for it.
fn automatic(gear: u8, speed: f32, top_speed: f32) -> u8 {
    let speed = speed.abs();
    let top = |gear: u8| gear_top(Gear::Forward(gear), top_speed);
    let mut gear = gear.clamp(1, FORWARD_GEARS);
    while gear < FORWARD_GEARS && speed > top(gear) * SHIFT_UP {
        gear += 1;
    }
    while gear > 1 && speed < top(gear - 1) * SHIFT_DOWN {
        gear -= 1;
    }
    gear
}

/// The gear a truck goes into from `gear`, going forwards at `forward_speed`, in m/s
/// (backwards negative), with `throttle` from -1 to 1. `manual` is how many gears the
/// driver asks to change by, up positive, or `None` if the gearbox changes by itself.
fn next_gear(
    gear: Gear,
    forward_speed: f32,
    throttle: f32,
    manual: Option<i8>,
    top_speed: f32,
) -> Gear {
    // Reverse goes in and out by itself (see the module's notes).
    let into_reverse = throttle < 0.0 && forward_speed < STOPPED;
    let out_of_reverse = throttle > 0.0 && forward_speed > -STOPPED;
    match (gear, manual) {
        (Gear::Forward(_) | Gear::Neutral, _) if into_reverse => Gear::Reverse,
        (Gear::Reverse, _) if out_of_reverse => Gear::Forward(1),
        (Gear::Reverse, _) => Gear::Reverse,
        // Neutral is 0, below first.
        (Gear::Forward(_) | Gear::Neutral, Some(shifts)) => {
            let from = match gear {
                Gear::Forward(gear) => i16::from(gear),
                _ => 0,
            };
            match (from + i16::from(shifts)).clamp(0, i16::from(FORWARD_GEARS)) {
                0 => Gear::Neutral,
                gear => Gear::Forward(gear as u8),
            }
        }
        (Gear::Forward(gear), None) => Gear::Forward(automatic(gear, forward_speed, top_speed)),
        (Gear::Neutral, None) => Gear::Forward(automatic(1, forward_speed, top_speed)),
    }
}

/// How the clutch couples the engine to the wheels in `gear`, with a change of gear under
/// way or not (`shifting`), on a manual or an automatic, with `throttle` from -1 to 1 (see
/// the module's notes). Only a manual driver can let the clutch out in a high gear: the
/// automatic, every starting gear and reverse, and a high gear with no throttle (coasting)
/// are the converter.
fn clutch_mode(gear: Gear, shifting: bool, manual: bool, throttle: f32) -> ClutchMode {
    // The throttle drives reverse the way it goes: backwards.
    let demand = match gear {
        Gear::Reverse => -throttle,
        _ => throttle,
    };
    if shifting || gear == Gear::Neutral {
        ClutchMode::Open
    } else if manual && !starts(gear) && demand > 0.0 {
        ClutchMode::Manual
    } else {
        ClutchMode::Converter
    }
}

/// What `change_gears` reads and writes of each truck.
type GearedTruck = (
    &'static Transform,
    &'static LinearVelocity,
    &'static TruckConfig,
    &'static mut TruckInput,
    &'static mut Gearbox,
    Has<PlayerTruck>,
    Has<Autopilot>,
);

/// Puts each truck in its gear for the coming physics step, runs the shift timer, and
/// decides how the clutch couples the engine to the wheels. The manual is for the player's
/// truck only, and only while the player drives it: the computer's trucks, and the
/// player's on `Autopilot`, are automatic. Takes the changes of gear the driver asked for
/// (`TruckInput::shifts`).
pub(super) fn change_gears(
    time: Res<Time>,
    transmission: Res<Transmission>,
    mut trucks: Query<GearedTruck, With<Truck>>,
) {
    let dt = time.delta_secs();
    for (transform, velocity, config, mut input, mut gearbox, player, autopilot) in &mut trucks {
        let shifts = std::mem::take(&mut input.shifts);
        let manual =
            (*transmission == Transmission::Manual && player && !autopilot).then_some(shifts);
        let forward_speed = velocity.0.dot(transform.forward().as_vec3());
        let gear = next_gear(
            gearbox.gear,
            forward_speed,
            input.throttle,
            manual,
            config.top_speed,
        );
        if gear != gearbox.gear {
            gearbox.gear = gear;
            gearbox.shifting = SHIFT_TIME;
        }
        gearbox.shifting = (gearbox.shifting - dt).max(0.0);
        gearbox.clutch_mode =
            clutch_mode(gear, gearbox.shifting(), manual.is_some(), input.throttle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOP: f32 = 60.0;

    /// The automatic's gear at each speed from a standstill up to `to`, in m/s, and back
    /// down, a little at a time, as a truck goes.
    fn sweep(to: f32) -> Vec<(f32, u8)> {
        let mut gear = 1;
        let steps = 600;
        let up = (0..=steps).map(|step| to * step as f32 / steps as f32);
        let down = (0..=steps)
            .rev()
            .map(|step| to * step as f32 / steps as f32);
        up.chain(down)
            .map(|speed| {
                gear = automatic(gear, speed, TOP);
                (speed, gear)
            })
            .collect()
    }

    #[test]
    fn the_automatic_goes_up_through_every_gear_and_back_down() {
        let gears = sweep(TOP);
        let highest = gears.iter().map(|(_, gear)| *gear).max();
        assert_eq!(highest, Some(FORWARD_GEARS));
        assert_eq!(gears.last().map(|(_, gear)| *gear), Some(1));
    }

    #[test]
    fn the_automatic_stays_under_the_governor_below_top_speed() {
        for (speed, gear) in sweep(TOP * 0.99) {
            let top = gear_top(Gear::Forward(gear), TOP);
            assert!(
                speed <= top,
                "gear {gear} at {speed} m/s is over its top {top}"
            );
        }
    }

    #[test]
    fn the_automatic_does_not_hunt_between_two_gears() {
        // Up from first to second, then a little slower: it stays in second.
        let up = GEAR_TOPS[0] * TOP * SHIFT_UP + 0.1;
        let gear = automatic(1, up, TOP);
        assert_eq!(gear, 2);
        assert_eq!(automatic(gear, up - 1.0, TOP), 2);
    }

    #[test]
    fn the_manual_changes_one_gear_at_a_time_within_the_box() {
        let forward = |gear, shifts| next_gear(Gear::Forward(gear), 10.0, 1.0, Some(shifts), TOP);
        assert_eq!(forward(1, 1), Gear::Forward(2));
        assert_eq!(forward(3, -1), Gear::Forward(2));
        assert_eq!(forward(1, -1), Gear::Neutral);
        assert_eq!(forward(FORWARD_GEARS, 1), Gear::Forward(FORWARD_GEARS));
        // Without asking, it stays in its gear, however fast the truck goes.
        assert_eq!(forward(1, 0), Gear::Forward(1));
    }

    #[test]
    fn reverse_goes_in_at_a_stop_and_out_on_the_throttle_in_either_mode() {
        for manual in [None, Some(0)] {
            let gear = |gear, speed, throttle| next_gear(gear, speed, throttle, manual, TOP);
            // Braking from speed: still in a forward gear.
            assert!(matches!(
                gear(Gear::Forward(3), 10.0, -1.0),
                Gear::Forward(_)
            ));
            assert_eq!(gear(Gear::Forward(1), 0.2, -1.0), Gear::Reverse);
            // Backing up, and coasting backwards: still in reverse.
            assert_eq!(gear(Gear::Reverse, -5.0, -1.0), Gear::Reverse);
            assert_eq!(gear(Gear::Reverse, -5.0, 0.0), Gear::Reverse);
            // The throttle brakes from speed backwards, and takes it out at a stop.
            assert_eq!(gear(Gear::Reverse, -5.0, 1.0), Gear::Reverse);
            assert_eq!(gear(Gear::Reverse, -0.2, 1.0), Gear::Forward(1));
        }
    }

    #[test]
    fn a_gear_reads_as_its_number_or_r_or_n() {
        assert_eq!(Gear::Forward(3).to_string(), "3");
        assert_eq!(Gear::Reverse.to_string(), "R");
        assert_eq!(Gear::Neutral.to_string(), "N");
    }

    #[test]
    fn neutral_is_below_first_in_the_manual_only() {
        let manual = |gear, shifts| next_gear(gear, 0.0, 0.0, Some(shifts), TOP);
        assert_eq!(manual(Gear::Forward(1), -1), Gear::Neutral);
        assert_eq!(manual(Gear::Neutral, -1), Gear::Neutral);
        assert_eq!(manual(Gear::Neutral, 1), Gear::Forward(1));
        assert_eq!(manual(Gear::Neutral, 0), Gear::Neutral);
        // The automatic leaves it for the gear the speed calls for.
        let automatic = next_gear(Gear::Neutral, TOP * 0.5, 1.0, None, TOP);
        assert!(
            matches!(automatic, Gear::Forward(gear) if gear > 1),
            "{automatic:?}"
        );
        // The brake at a stop puts reverse in from neutral too.
        assert_eq!(
            next_gear(Gear::Neutral, 0.0, -1.0, Some(0), TOP),
            Gear::Reverse
        );
    }

    #[test]
    fn the_clutch_is_open_in_neutral_and_during_a_change() {
        assert_eq!(
            clutch_mode(Gear::Neutral, false, true, 1.0),
            ClutchMode::Open
        );
        assert_eq!(
            clutch_mode(Gear::Neutral, false, false, 1.0),
            ClutchMode::Open
        );
        for gear in [
            Gear::Reverse,
            Gear::Forward(1),
            Gear::Forward(FORWARD_GEARS),
        ] {
            assert_eq!(
                clutch_mode(gear, true, true, 1.0),
                ClutchMode::Open,
                "{gear}"
            );
        }
    }

    #[test]
    fn only_a_manual_driver_on_the_throttle_in_a_high_gear_lets_the_clutch_out() {
        let high = Gear::Forward(STARTING_GEARS + 1);
        assert_eq!(clutch_mode(high, false, true, 1.0), ClutchMode::Manual);
        // The automatic is the converter in the same gear.
        assert_eq!(clutch_mode(high, false, false, 1.0), ClutchMode::Converter);
        // Coasting, or braking, in a high gear is the converter: the engine is turned by
        // the wheels and never stalls.
        assert_eq!(clutch_mode(high, false, true, 0.0), ClutchMode::Converter);
        assert_eq!(clutch_mode(high, false, true, -1.0), ClutchMode::Converter);
        // The starting gears and reverse are the converter even in the manual.
        for gear in 1..=STARTING_GEARS {
            assert_eq!(
                clutch_mode(Gear::Forward(gear), false, true, 1.0),
                ClutchMode::Converter
            );
        }
        assert_eq!(
            clutch_mode(Gear::Reverse, false, true, -1.0),
            ClutchMode::Converter
        );
    }

    #[test]
    fn a_fresh_gearbox_is_in_first_and_not_shifting() {
        let gearbox = Gearbox::default();
        assert_eq!(gearbox.gear(), Gear::Forward(1));
        assert!(!gearbox.shifting());
    }
}
