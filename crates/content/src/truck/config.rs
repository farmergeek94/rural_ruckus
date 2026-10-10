//! The truck's tuning values.

use bevy::prelude::*;

use super::EngineConfig;

/// Everything you'd tune to change how the truck feels.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct TruckConfig {
    /// The truck's weight, in kilograms.
    pub mass: f32,
    /// Centre of mass relative to the chassis centre. Lower is harder to roll over.
    pub center_of_mass: Vec3,

    /// Where each hub is with the truck standing still on level ground, relative to the
    /// chassis centre, in metres: front left, front right, rear left, rear right. The
    /// springs hold the truck here whatever their stiffness. It is partway into the
    /// travel: the body settles onto its springs, sinks from here on a bump or a landing,
    /// and can rise from here too, with the wheels hanging below it.
    pub wheel_rest: [Vec3; 4],
    pub wheel_radius: f32,
    /// How wide a tire is along its axle, in metres.
    pub wheel_width: f32,
    /// How far a hub can rise above its rest position before it meets the bump stop, in
    /// metres. Less is a truck that leans and dives less, and hits its stops on smaller
    /// bumps.
    pub suspension_bump: f32,
    /// How far a hub can hang below its rest position, in metres: the rest of the travel.
    /// It is what lets a wheel follow the ground down into a dip, and the body rise off
    /// its springs, without the tire leaving the ground. Once the springs have stretched
    /// back to their full length they push no more, and a wheel still lower hangs free.
    pub suspension_droop: f32,
    /// How far out from the middle of its axle each wheel's spring sits, as a fraction of
    /// how far out the wheel is. Each axle is a solid beam carrying both its wheels, and
    /// its springs sit on the beam, inboard of the wheels. So one wheel rising squeezes
    /// the other wheel's spring too, and the springs resist the body leaning by this
    /// squared: at 0.5, a quarter as much as springs at the wheels would. Lower leans
    /// more and shares a bump between the wheels more. 1 is springs at the wheels, and
    /// the wheels are independent.
    pub spring_spread: f32,
    /// Spring stiffness in N/m, per wheel. Higher is tighter: the body moves less over
    /// bumps and follows the ground more closely.
    pub spring: f32,
    /// Damper strength while the suspension is being squeezed, in N·s/m, per wheel. Lower
    /// lets the body sink further into a landing; higher passes more of a bump on to it.
    pub damper: f32,
    /// Damper strength while the suspension is stretching back out, in N·s/m, per wheel.
    /// This is what stops the springs throwing the truck back into the air after a
    /// landing. At about twice `sqrt(spring x mass / 4)` the body comes back up to its ride
    /// height once and stays there. Lower and it hops; much higher and it is slow to
    /// recover between bumps, and the tires are lightly loaded while it does.
    pub rebound_damper: f32,
    /// How much of the top of the travel the bump stop covers, in metres: a hub this far
    /// below its upper limit, or nearer, is pressing into it. Longer firms up big hits
    /// sooner, and leaves less of the travel soft.
    pub bump_stop_travel: f32,
    /// Stiffness of the bump stop, in N/m, per wheel, on top of the spring's, at the top of
    /// the travel: it comes in from nothing where the stop begins, and stiffens to this. It
    /// is rubber: what it stores as a wheel presses into it, it gives back as the wheel
    /// comes out, so a wheel driven hard into it comes back off it with a little upward push
    /// instead of stopping dead. Higher stops a wheel sooner, and pushes it back harder.
    pub bump_stop_spring: f32,
    /// How much of a hit the bump stop soaks up, in N·s/m, per wheel, at the top of the
    /// travel, coming in from nothing as the spring does. It acts only while a wheel presses
    /// into the stop, not while it comes back out, which is what lets the stop give a little
    /// back. Lower and the truck bounces off its stops.
    pub bump_stop_damper: f32,

    /// Peak drive force in N, per wheel: the locked first-gear pull at the engine's best
    /// speed (see `engine`). The pull tuning knob.
    pub engine_force: f32,
    /// Peak braking force in N, per wheel.
    pub brake_force: f32,
    /// The speed, in m/s, at which the top gear meets the engine's governed speed. Each
    /// lower gear meets it at its share of this (`gearbox::GEAR_TOPS`).
    pub top_speed: f32,
    /// The engine: its speeds, torque curve, governors, turbo and clutch. Every truck has
    /// the same one for now; a POD gives nothing of how a truck drives.
    pub engine: EngineConfig,
    /// Speed-proportional drag in N·s/m, per wheel.
    pub rolling_resistance: f32,

    /// Tire friction coefficient: total tire force is capped at `grip * load`.
    pub grip: f32,
    /// Friction coefficient of the rear tires while the handbrake is held.
    pub handbrake_grip: f32,
    /// How much of the sideways slip each tire tries to cancel per physics step (0 to 1).
    pub lateral_stiffness: f32,

    /// Front steering lock in radians, at low speed. At speed the lock is held to what the
    /// tires can hold (`drive::holdable_lock`), so this is how tight the truck turns where
    /// grip is not the limit.
    pub max_steer: f32,
    /// Rear wheels counter-steer by this fraction of the front angle (monster trucks
    /// steer all four wheels), at a standstill. It fades with speed to
    /// `drive::REAR_STEER_FLOOR` of it (`drive::REAR_STEER_FADE_SPEED`). 0 disables rear
    /// steering.
    pub rear_steer_ratio: f32,
    /// How far down from the hub towards the ground a tire's sideways grip acts, as a share
    /// of the tire's radius: 0 at the hub, 1 where the tire meets the ground, as a real
    /// tire's does. Higher leans the truck further out in a turn and tips it more readily:
    /// the grip pulls at the bottom of the truck while its weight carries on higher up.
    /// Drive and brakes stay at the hub, so that they pitch the truck no more than they did.
    /// Measured on flat ground with the built-in truck at full lock, throttle held, tip
    /// guard on: at 1 it leaned 13 to 19 degrees up to 25 m/s and 54 at 30; at 0.5, 11 to 15
    /// and 44; at 0 (the hub), 8 to 20 and, without the guard, 72. At 1 without the guard it
    /// rolled over at 30.
    pub cornering_lever: f32,
}

/// How the player has set the truck up in the garage, laid over whatever `TruckConfig` the
/// truck has. Each dial runs from -1 to 1, and 0 is the truck exactly as it was tuned, so
/// that every figure measured on it stands. Insert it before a race is entered.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct TruckSetup {
    /// -1 (soft) to 1 (stiff).
    pub suspension: f32,
    /// -1 (acceleration) to 1 (top speed).
    pub gearing: f32,
    /// -1 (off) to 1 (sharp).
    pub rear_steering: f32,
    /// Where the tires grip: -1 (at the hub: planted) to 1 (at the ground: leans and tips).
    pub grip: f32,
}

// What the dials' ends do. First guesses, which need driving: a dial should feel worth
// turning without making a truck that cannot be driven.
/// `spring` is multiplied by this with the suspension dial at its softest. Lower wallows more.
const SOFTEST_SPRING: f32 = 0.6;
/// And by this at its stiffest. Higher follows the ground more closely and skips over it sooner.
const STIFFEST_SPRING: f32 = 1.6;
/// `top_speed` is multiplied by this with the gearing dial at "top speed", and divided by it
/// at "acceleration", while `engine_force` goes the other way. Higher makes the dial matter more.
const GEARING_RANGE: f32 = 1.25;
/// `rear_steer_ratio` with the rear steering dial at its sharpest. At its lowest it is 0.
/// Higher turns tighter and is twitchier at speed.
const SHARPEST_REAR_STEER: f32 = 0.6;
/// `cornering_lever` with the grip dial at "planted": the hub, where cornering leans the
/// truck least.
const PLANTED_LEVER: f32 = 0.0;
/// And at "tippy": where the tire meets the ground, where it leans the truck most.
const TIPPY_LEVER: f32 = 1.0;

/// The places a computer's truck may have each dial at: the garage's five, so that it is
/// set up as a player could set it.
const DIAL_PLACES: [f32; 5] = [-1.0, -0.5, 0.0, 0.5, 1.0];

impl TruckSetup {
    /// A setup that `seed` picks, each dial at one of `DIAL_PLACES`, every place as likely
    /// as another. For the computer's trucks, so that no two race alike. The grip dial
    /// stays centred: the truck grips as it was tuned.
    pub fn random(seed: u64) -> Self {
        // Stirred (splitmix64), so that seeds close together pick different setups.
        let mut state = seed;
        let mut place = || {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut mixed = state;
            mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            mixed ^= mixed >> 31;
            DIAL_PLACES[(mixed % DIAL_PLACES.len() as u64) as usize]
        };
        Self {
            suspension: place(),
            gearing: place(),
            rear_steering: place(),
            grip: 0.0,
        }
    }
}

/// From `centre` towards `low` as `dial` goes to -1, and towards `high` as it goes to 1.
fn dial_between(dial: f32, low: f32, centre: f32, high: f32) -> f32 {
    let dial = dial.clamp(-1.0, 1.0);
    if dial < 0.0 {
        centre + (centre - low) * dial
    } else {
        centre + (high - centre) * dial
    }
}

/// How many of `TruckConfig::wheel_rest`, counting from the first, are front wheels.
pub const FRONT_WHEELS: usize = 2;

/// For working out how far the springs give under the truck's weight, in m/s².
const GRAVITY: f32 = 9.81;

impl TruckConfig {
    /// How far the springs are squeezed with the truck standing still on level ground,
    /// each carrying a quarter of the weight. In metres.
    pub fn sag(&self) -> f32 {
        self.mass * GRAVITY / 4.0 / self.spring
    }

    /// This truck as the garage has set it up. With every dial centred it is this truck.
    pub fn with_setup(&self, setup: &TruckSetup) -> Self {
        // Dampers go by the square root of the springs, which keeps the damping ratio: a
        // stiff truck is taut, and not bouncy.
        let stiffness = dial_between(setup.suspension, SOFTEST_SPRING, 1.0, STIFFEST_SPRING);
        // Force times top speed is the engine's power, which no gearbox changes.
        let gearing = GEARING_RANGE.powf(setup.gearing.clamp(-1.0, 1.0));
        Self {
            spring: self.spring * stiffness,
            damper: self.damper * stiffness.sqrt(),
            rebound_damper: self.rebound_damper * stiffness.sqrt(),
            top_speed: self.top_speed * gearing,
            engine_force: self.engine_force / gearing,
            rear_steer_ratio: dial_between(
                setup.rear_steering,
                0.0,
                self.rear_steer_ratio,
                SHARPEST_REAR_STEER,
            ),
            cornering_lever: dial_between(
                setup.grip,
                PLANTED_LEVER,
                self.cornering_lever,
                TIPPY_LEVER,
            ),
            ..self.clone()
        }
    }

    /// The top of each wheel's travel, where its hub meets the bump stop.
    pub fn wheel_mounts(&self) -> [Vec3; 4] {
        self.wheel_rest
            .map(|rest| rest + Vec3::Y * self.suspension_bump)
    }

    /// Chassis height at which the tires just reach the ground, which is its ride height.
    pub fn standing_height(&self) -> f32 {
        let lowest_hub = self
            .wheel_rest
            .iter()
            .map(|rest| rest.y)
            .fold(f32::INFINITY, f32::min);
        self.wheel_radius - lowest_hub
    }
}

impl Default for TruckConfig {
    fn default() -> Self {
        Self {
            // 12 000 lb, what a monster truck weighs. The truck was tuned at 2 000 kg, and every
            // value below that is a force (springs, dampers, bump stops, engine, brakes,
            // rolling resistance) is that tuning times 5 443 / 2 000, so that it rides and
            // drives as tuned: the same sag and body frequency, the same damping, the same
            // pull and braking for its weight. The figures in the comments below were
            // measured at 2 000 kg, with the values then (the ones here over 2.72).
            mass: 5_443.0,
            center_of_mass: Vec3::new(0.0, -0.5, 0.0),
            wheel_rest: [
                Vec3::new(-1.55, -0.65, -1.75),
                Vec3::new(1.55, -0.65, -1.75),
                Vec3::new(-1.55, -0.65, 1.75),
                Vec3::new(1.55, -0.65, 1.75),
            ],
            wheel_radius: 1.0,
            wheel_width: 0.7,
            // 0.8 m in all, 31 inches: the body sits a little under two thirds of the way
            // down it, as a monster truck's does.
            suspension_bump: 0.5,
            suspension_droop: 0.3,
            // The shocks of a Monster Truck Madness 2 truck stand about half as far out as
            // its wheels (see the `pod` crate's truck converter).
            spring_spread: 0.5,
            // Soft for a race truck, to ride the bumps at the speeds the extra power brings:
            // the body moves at about 1.08 Hz. Softer uses more of the travel over a bump or
            // into a landing, not more at rest, where the spring's preload holds the ride
            // height whatever it is. Damped heavily, so that it soaks a bump up rather than
            // bouncing off it: over three 0.5 m logs at 15 m/s on a spring of 18 000, dampers
            // two thirds as strong as these let the body heave at 0.59 m/s and pitch at 0.39
            // rad/s, with 1.1 g jolts, and half as strong again held it to 0.35 m/s and 0.29
            // rad/s, and 0.8 g. These are that, stiffened with the spring by its square
            // root. The price is that after a big landing it creeps back up to its ride
            // height rather than springing. (At 14 000
            // and 1 600 it floated at 0.85 Hz and bounced for four seconds.)
            spring: 62_600.0,
            damper: 13_995.0,
            rebound_damper: 31_100.0,
            // Over the top of the travel, so that a wheel is slowed well before it runs out
            // of room: at speed on Alpine, ordinary undulations used up all of 0.4 m, and a
            // tire with no travel left meets rising ground like a kerb and loses the truck
            // its speed. With this stop, 7 computer trucks for two minutes had 48 such
            // losses of over 1 m/s where they had had 212 (see `contacts` for the rest).
            bump_stop_travel: 0.2,
            bump_stop_spring: 680_000.0,
            bump_stop_damper: 54_400.0,
            // Enough to spin the tires off the line and pull hard to about 15 m/s, where
            // the falling pull drops under the grip. At 8 000 and 55 the pull and the rolling
            // resistance balanced at about 37 m/s on level ground; at these, a fifth more
            // force and a tenth more speed, that goes up by about a tenth, which is not yet
            // measured. (At 5 500 and 38 it was near the grip only at a standstill, and
            // balanced at 30 m/s.)
            engine_force: 26_400.0,
            brake_force: 19_050.0,
            top_speed: 60.0,
            engine: EngineConfig::default(),
            rolling_resistance: 109.0,
            grip: 1.1,
            handbrake_grip: 0.45,
            lateral_stiffness: 0.5,
            max_steer: 0.5,
            rear_steer_ratio: 0.35,
            cornering_lever: 0.6,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(suspension: f32, gearing: f32, rear_steering: f32) -> TruckSetup {
        TruckSetup {
            suspension,
            gearing,
            rear_steering,
            grip: 0.0,
        }
    }

    #[test]
    fn a_centred_setup_is_the_truck_as_it_was() {
        let config = TruckConfig::default();
        assert_eq!(config.with_setup(&TruckSetup::default()), config);
    }

    #[test]
    fn stiffer_springs_keep_the_damping_ratio() {
        let config = TruckConfig::default();
        let ratio = |config: &TruckConfig| config.rebound_damper / config.spring.sqrt();
        for dial in [-1.0, -0.5, 0.5, 1.0] {
            let set_up = config.with_setup(&setup(dial, 0.0, 0.0));
            assert!((ratio(&set_up) / ratio(&config) - 1.0).abs() < 1e-5);
            assert!((set_up.damper / set_up.rebound_damper - 0.45).abs() < 1e-5);
        }
        let spring = |dial| config.with_setup(&setup(dial, 0.0, 0.0)).spring / config.spring;
        assert!((spring(-1.0) - SOFTEST_SPRING).abs() < 1e-6);
        assert!((spring(1.0) - STIFFEST_SPRING).abs() < 1e-6);
        // And no further, whatever it is handed.
        assert_eq!(spring(7.0), spring(1.0));
    }

    #[test]
    fn gearing_trades_pull_for_top_speed() {
        let config = TruckConfig::default();
        let power = config.engine_force * config.top_speed;
        for dial in [-1.0, -0.5, 0.5, 1.0] {
            let set_up = config.with_setup(&setup(0.0, dial, 0.0));
            assert!((set_up.engine_force * set_up.top_speed / power - 1.0).abs() < 1e-5);
            assert_eq!(set_up.top_speed > config.top_speed, dial > 0.0);
        }
        let tall = config.with_setup(&setup(0.0, 1.0, 0.0));
        assert!((tall.top_speed / config.top_speed - GEARING_RANGE).abs() < 1e-5);
    }

    #[test]
    fn rear_steering_runs_from_none_to_its_sharpest() {
        let config = TruckConfig::default();
        let rear = |dial| config.with_setup(&setup(0.0, 0.0, dial)).rear_steer_ratio;
        assert_eq!(rear(-1.0), 0.0);
        assert_eq!(rear(0.0), config.rear_steer_ratio);
        assert_eq!(rear(1.0), SHARPEST_REAR_STEER);
        assert!(rear(-0.5) < rear(0.0) && rear(0.0) < rear(0.5));
    }

    #[test]
    fn the_grip_dial_moves_where_the_tires_grip_and_not_how_much() {
        let config = TruckConfig::default();
        let set_up = |dial| {
            config.with_setup(&TruckSetup {
                grip: dial,
                ..default()
            })
        };
        let lever = |dial| set_up(dial).cornering_lever;
        assert_eq!(lever(-1.0), PLANTED_LEVER);
        assert_eq!(lever(0.0), config.cornering_lever);
        assert_eq!(lever(1.0), TIPPY_LEVER);
        assert!(lever(-0.5) < lever(0.0) && lever(0.0) < lever(0.5));
        for dial in [-1.0, 1.0] {
            assert_eq!(set_up(dial).grip, config.grip);
            assert_eq!(set_up(dial).handbrake_grip, config.handbrake_grip);
        }
    }

    #[test]
    fn a_random_setup_puts_every_dial_where_the_garage_can() {
        let setups: Vec<_> = (0..200).map(TruckSetup::random).collect();
        for setup in &setups {
            for dial in [setup.suspension, setup.gearing, setup.rear_steering] {
                assert!(DIAL_PLACES.contains(&dial));
            }
            assert_eq!(setup.grip, 0.0);
        }
        // Every place turns up, on every dial, and the same seed picks the same setup.
        for place in DIAL_PLACES {
            assert!(setups.iter().any(|setup| setup.suspension == place));
            assert!(setups.iter().any(|setup| setup.gearing == place));
            assert!(setups.iter().any(|setup| setup.rear_steering == place));
        }
        assert_eq!(TruckSetup::random(7), TruckSetup::random(7));
    }
}
