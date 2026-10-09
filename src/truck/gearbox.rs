//! The gearbox: five forward gears and a reverse, changed by itself (`Transmission::Automatic`,
//! the default) or, on the player's truck, by the player (`Transmission::Manual`).
//!
//! The gearbox only scales the engine's push (`Gearbox::push`); it adds none. `drive` pushes
//! a tire with all of `engine_force` from a standstill, falling to nothing at `top_speed`,
//! and that tuning stands. Each gear reaches only its share of the top speed (`GEAR_TOPS`),
//! where the engine meets its rev limit and stops pushing; in a gear too high for the speed
//! the engine lugs and pushes less (`LUG_KNEE`), and lower still it stalls (`STALL_REVS`).
//! The automatic changes up before the limit and down before the engine lugs, so it pushes
//! as the one gear did before, but for a short pause at each change (`SHIFT_TIME`). A
//! manual driver who changes up late meets the limit, and one who changes up early lugs.
//!
//! Only the lowest gears (`STARTING_GEARS`) and reverse start the truck from a standstill,
//! with the clutch slipping. Throttle in a higher gear with the engine under `STALL_REVS`
//! stalls it: it pushes nothing until the throttle is let go or the gear is changed, and
//! then it runs again. The automatic never stalls, because it changes down first.
//!
//! Reverse goes in by itself in either mode, when the brake key is held with the truck at a
//! stop, and out again when the throttle is: the manual changes the forward gears only.
//!
//! Five forward gears are the user's choice. What gears MTM2 had is not confirmed.

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;

use super::{Autopilot, PlayerTruck, Transmission, Truck, TruckConfig, TruckInput};

/// How many forward gears there are.
pub const FORWARD_GEARS: u8 = 5;

/// The speed at which each forward gear meets the engine's rev limit, as a share of the
/// truck's top speed, first gear first. Closer together, the automatic changes more often
/// and a manual gear suits a narrower range of speeds. A first guess, which wants driving.
const GEAR_TOPS: [f32; FORWARD_GEARS as usize] = [0.25, 0.42, 0.6, 0.8, 1.0];

/// The same for reverse.
const REVERSE_TOP: f32 = GEAR_TOPS[0];

/// How far over the rev limit, as a share of it, the engine's push fades to nothing, so
/// that a truck at the limit holds its speed instead of jerking on and off it.
const LIMIT_FADE: f32 = 0.03;

/// How many of the lowest forward gears start the truck from a standstill, with the clutch
/// slipping, as reverse does: they never lug nor stall. Higher lets a manual driver pull
/// away in a higher gear.
const STARTING_GEARS: u8 = 2;

/// The engine speed, as a share of the rev limit, under which it lugs and pushes less, in
/// every gear above `STARTING_GEARS`: from all of its push here down to `LUG_FLOOR` of it at
/// `STALL_REVS`. It must stay under where the automatic changes up to (`SHIFT_UP` times the
/// ratio of two gears' tops: 0.64 from second to third) and where it changes down from
/// (`SHIFT_DOWN` likewise: 0.56 from third to second), so that the automatic never lugs.
/// Higher punishes a manual driver's early change more.
const LUG_KNEE: f32 = 0.45;
const LUG_FLOOR: f32 = 0.25;

/// The engine speed, as a share of the rev limit, under which the throttle stalls the
/// engine in a gear above `STARTING_GEARS`. In fifth, with the built-in truck's top speed
/// of 60 m/s, it is 12 m/s. Higher stalls a manual driver sooner.
const STALL_REVS: f32 = 0.2;

/// The engine speed, as a share of the rev limit, at which the automatic changes up.
/// Higher holds each gear longer.
const SHIFT_UP: f32 = 0.92;

/// The engine speed that the gear below would have, as a share of its rev limit, under
/// which the automatic changes down. It must be under `SHIFT_UP`, or the automatic would
/// change straight back up. Higher changes down sooner.
const SHIFT_DOWN: f32 = 0.8;

/// How long a change of gear takes, in seconds, while the engine pushes nothing. Longer
/// costs more speed at each change.
const SHIFT_TIME: f32 = 0.1;

/// How slowly the truck must go, in m/s, for reverse to go in, or to come out. As `drive`
/// has it, the throttle against the way the truck goes faster than this is the brakes.
const STOPPED: f32 = 0.5;

/// A gear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gear {
    Reverse,
    /// From 1 to `FORWARD_GEARS`.
    Forward(u8),
}

impl std::fmt::Display for Gear {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Gear::Reverse => write!(f, "R"),
            Gear::Forward(gear) => write!(f, "{gear}"),
        }
    }
}

/// On a truck: its gearbox, as it stands after the last physics step.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Gearbox {
    gear: Gear,
    /// The engine's speed, as a share of its rev limit, from 0 to 1.
    revs: f32,
    /// How long the change of gear under way has left to take, in seconds.
    shifting: f32,
    /// Whether the engine has stalled (see the module's notes).
    stalled: bool,
    /// How much of the engine's push reaches the wheels, from 0 to 1. `drive` reads it.
    pub(super) push: f32,
}

impl Default for Gearbox {
    fn default() -> Self {
        Self {
            gear: Gear::Forward(1),
            revs: 0.0,
            shifting: 0.0,
            stalled: false,
            push: 1.0,
        }
    }
}

impl Gearbox {
    /// The gear it is in.
    pub fn gear(&self) -> Gear {
        self.gear
    }

    /// The engine's speed, as a share of its rev limit, from 0 to 1.
    pub fn revs(&self) -> f32 {
        self.revs
    }

    /// Whether the engine has stalled: it pushes nothing, and does not turn.
    pub fn stalled(&self) -> bool {
        self.stalled
    }
}

/// Whether `gear` starts the truck from a standstill, with the clutch slipping.
fn starts(gear: Gear) -> bool {
    match gear {
        Gear::Reverse => true,
        Gear::Forward(gear) => gear <= STARTING_GEARS,
    }
}

/// Whether the engine is stalled after this step, from `stalled`, in `gear` at `speed`, in
/// m/s, with `throttle` from -1 to 1 (see the module's notes). The caller starts it again
/// when the gear is changed.
fn stalls(stalled: bool, gear: Gear, speed: f32, throttle: f32, top_speed: f32) -> bool {
    throttle > 0.0 && (stalled || (!starts(gear) && revs(gear, speed, top_speed) < STALL_REVS))
}

/// The speed at which `gear` meets the rev limit, in m/s, on a truck whose top speed is
/// `top_speed`.
fn gear_top(gear: Gear, top_speed: f32) -> f32 {
    match gear {
        Gear::Reverse => REVERSE_TOP * top_speed,
        Gear::Forward(gear) => GEAR_TOPS[usize::from(gear.clamp(1, FORWARD_GEARS)) - 1] * top_speed,
    }
}

/// The engine's speed in `gear` at `speed`, in m/s either way, as a share of its rev limit.
/// Over 1 the truck goes faster than the gear reaches, as down a hill.
fn revs(gear: Gear, speed: f32, top_speed: f32) -> f32 {
    let top = gear_top(gear, top_speed);
    if top > 0.0 { speed.abs() / top } else { 0.0 }
}

/// How much of the engine's push reaches the wheels in `gear` at `speed`, in m/s, from 0
/// to 1, while it runs: none past the rev limit, and less where the engine lugs. Under
/// `STALL_REVS` it is `LUG_FLOOR`, but the throttle stalls it there (`stalls`).
fn gear_push(gear: Gear, speed: f32, top_speed: f32) -> f32 {
    let revs = revs(gear, speed, top_speed);
    let limit = ((1.0 - revs) / LIMIT_FADE + 1.0).clamp(0.0, 1.0);
    let lug = if starts(gear) {
        1.0
    } else {
        let over_stall = (revs - STALL_REVS) / (LUG_KNEE - STALL_REVS);
        (LUG_FLOOR + (1.0 - LUG_FLOOR) * over_stall).clamp(LUG_FLOOR, 1.0)
    };
    limit * lug
}

/// The forward gear the automatic puts a truck in at `speed`, in m/s either way, from
/// `gear`: up past `SHIFT_UP` of each gear's limit, and down under `SHIFT_DOWN` of the
/// limit of the gear below. Several at once, if the speed calls for it.
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
    match gear {
        Gear::Forward(_) if into_reverse => Gear::Reverse,
        Gear::Reverse if out_of_reverse => Gear::Forward(1),
        Gear::Reverse => Gear::Reverse,
        Gear::Forward(gear) => Gear::Forward(match manual {
            Some(shifts) => {
                (i16::from(gear) + i16::from(shifts)).clamp(1, i16::from(FORWARD_GEARS)) as u8
            }
            None => automatic(gear, forward_speed, top_speed),
        }),
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

/// Puts each truck in its gear for the coming physics step, and works out how much of the
/// engine's push reaches its wheels. The manual is for the player's truck only, and only
/// while the player drives it: the computer's trucks, and the player's on `Autopilot`, are
/// automatic. Takes the changes of gear the driver asked for (`TruckInput::shifts`).
pub(super) fn change_gears(
    time: Res<Time>,
    transmission: Res<Transmission>,
    mut trucks: Query<GearedTruck, With<Truck>>,
) {
    let dt = time.delta_secs();
    for (transform, velocity, config, mut input, mut gearbox, player, autopilot) in &mut trucks {
        let shifts = std::mem::take(&mut input.shifts);
        let manual = (*transmission == Transmission::Manual && player && !autopilot)
            .then_some(shifts);
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
            gearbox.stalled = false;
        }
        gearbox.shifting = (gearbox.shifting - dt).max(0.0);
        gearbox.stalled = stalls(
            gearbox.stalled,
            gear,
            forward_speed,
            input.throttle,
            config.top_speed,
        );
        gearbox.revs = if gearbox.stalled {
            0.0
        } else {
            revs(gear, forward_speed, config.top_speed).min(1.0)
        };
        gearbox.push = if gearbox.shifting > 0.0 || gearbox.stalled {
            0.0
        } else {
            gear_push(gear, forward_speed, config.top_speed)
        };
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
        let down = (0..=steps).rev().map(|step| to * step as f32 / steps as f32);
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
    fn the_automatic_never_lugs_nor_meets_the_limit_below_top_speed() {
        for (speed, gear) in sweep(TOP * 0.99) {
            let push = gear_push(Gear::Forward(gear), speed, TOP);
            assert_eq!(push, 1.0, "gear {gear} at {speed} m/s pushes {push}");
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
    fn a_gear_stops_pushing_at_its_rev_limit() {
        let limit = GEAR_TOPS[0] * TOP;
        assert_eq!(gear_push(Gear::Forward(1), limit * 0.9, TOP), 1.0);
        assert_eq!(gear_push(Gear::Forward(1), limit * (1.0 + 2.0 * LIMIT_FADE), TOP), 0.0);
        assert_eq!(gear_push(Gear::Reverse, -limit * 1.1, TOP), 0.0);
    }

    #[test]
    fn a_high_gear_lugs_at_low_revs_and_a_starting_gear_does_not() {
        assert_eq!(gear_push(Gear::Forward(1), 0.0, TOP), 1.0);
        assert_eq!(gear_push(Gear::Forward(STARTING_GEARS), 0.0, TOP), 1.0);
        assert_eq!(gear_push(Gear::Reverse, 0.0, TOP), 1.0);
        let fifth = Gear::Forward(FORWARD_GEARS);
        let lugging = gear_push(fifth, TOP * (STALL_REVS + LUG_KNEE) / 2.0, TOP);
        assert!(LUG_FLOOR < lugging && lugging < 1.0, "{lugging}");
    }

    #[test]
    fn the_throttle_stalls_a_high_gear_from_a_standstill() {
        let fifth = Gear::Forward(FORWARD_GEARS);
        assert!(stalls(false, fifth, 0.0, 1.0, TOP));
        assert!(!stalls(false, Gear::Forward(STARTING_GEARS), 0.0, 1.0, TOP));
        assert!(!stalls(false, Gear::Reverse, 0.0, -1.0, TOP));
        // Not without the throttle, nor fast enough for the gear.
        assert!(!stalls(false, fifth, 0.0, 0.0, TOP));
        assert!(!stalls(false, fifth, TOP * 0.5, 1.0, TOP));
    }

    #[test]
    fn a_stalled_engine_stays_stalled_until_the_throttle_is_let_go() {
        let fifth = Gear::Forward(FORWARD_GEARS);
        // Rolling down a hill fast enough for the gear does not start it again.
        assert!(stalls(true, fifth, TOP * 0.5, 1.0, TOP));
        assert!(!stalls(true, fifth, 0.0, 0.0, TOP));
    }

    #[test]
    fn the_automatic_never_stalls() {
        for (speed, gear) in sweep(TOP * 0.99) {
            assert!(!stalls(false, Gear::Forward(gear), speed, 1.0, TOP), "{gear} at {speed}");
        }
    }

    #[test]
    fn the_manual_changes_one_gear_at_a_time_within_the_box() {
        let forward = |gear, shifts| next_gear(Gear::Forward(gear), 10.0, 1.0, Some(shifts), TOP);
        assert_eq!(forward(1, 1), Gear::Forward(2));
        assert_eq!(forward(3, -1), Gear::Forward(2));
        assert_eq!(forward(1, -1), Gear::Forward(1));
        assert_eq!(forward(FORWARD_GEARS, 1), Gear::Forward(FORWARD_GEARS));
        // Without asking, it stays in its gear, however fast the truck goes.
        assert_eq!(forward(1, 0), Gear::Forward(1));
    }

    #[test]
    fn reverse_goes_in_at_a_stop_and_out_on_the_throttle_in_either_mode() {
        for manual in [None, Some(0)] {
            let gear = |gear, speed, throttle| next_gear(gear, speed, throttle, manual, TOP);
            // Braking from speed: still in a forward gear.
            assert!(matches!(gear(Gear::Forward(3), 10.0, -1.0), Gear::Forward(_)));
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
    fn a_gear_reads_as_its_number_or_r() {
        assert_eq!(Gear::Forward(3).to_string(), "3");
        assert_eq!(Gear::Reverse.to_string(), "R");
    }
}
