//! The engine and the drivetrain, simulated: the physics decides what the engine does, and
//! the sound slice decides what that sounds like.
//!
//! The engine is a rotating mass with a speed (`Engine::rpm`). Each physics step the fuel
//! it is given makes torque on a curve (`EngineConfig::torque_curve`), less a mechanical
//! friction that is the engine braking off the throttle; the turbo's boost lags the fuel
//! and the speed (`spool_up_s`, `spool_down_s`) and scales the torque, which is the lag the
//! driver feels. Two governors set the fuel, as a diesel's do: the idle governor adds fuel
//! as the speed droops under idle, and the top governor cuts it over a band under the
//! governed speed (`max_rpm`), so the throttle asks for fuel, not for a speed, in gear;
//! with the clutch open (neutral, or a change of gear) an all-speed governor makes the
//! pedal set the speed instead, so the engine revs freely to where the pedal says.
//!
//! The clutch couples the engine to the wheels at a speed the gear sets
//! (`gearbox::ClutchMode`): a torque converter that slips as the engine revs up from idle
//! and cannot stall it, or a driver's clutch let out in a high gear, which drags the
//! engine to the wheels' speed and stalls it under `stall_rpm`. Either transmits at most
//! its capacity; within it the engine is locked to the wheels. The wheels' speed is taken
//! from the chassis (`forward_speed` through the gear's ratio), not from each wheel's own
//! tread speed: that is a kinematic rule for the visual (see `drive`), and read back
//! through the gear it made a limit cycle at every launch and spun the engine to three
//! times its limit over a jump. So wheelspin and wheels in the air do not yet rev the
//! engine; coupling it to each wheel's rotation is later work (see `docs/roadmap.md`).
//!
//! The model is calibrated to `TruckConfig::engine_force` and `top_speed`, which stay the
//! pull and speed tuning knobs: the peak torque is set so that the locked first-gear pull
//! at the curve's best speed is `engine_force` per wheel, and each gear meets the governed
//! speed at its `gearbox::GEAR_TOPS` share of `top_speed`. Both are read every step, because
//! `opponents` and the garage rewrite them.
//!
//! Explicit Euler on the speed, stable because the speed changes by at most
//! `peak torque x dt / inertia` a step (about 100 rpm for the built-in truck) and because
//! the lock test takes the stiff coupling to the wheels without a stiff spring: a slipping
//! step always lands on the same side of the wheels' speed, and then locks. The governors
//! are proportional loops whose per-step gain stays under 0.5 (a test checks it).
//!
//! `EngineConfig::step` is a pure function over plain data, and the system `run_engines` is
//! thin: a few dozen flops per truck per step, no allocation, no wheel lookups.

use std::f32::consts::TAU;

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;

use super::gearbox::{self, ClutchMode, Gear, Gearbox};
use super::{Truck, TruckConfig, TruckInput};

const RAD_PER_RPM: f32 = TAU / 60.0;
const RPM_PER_RAD: f32 = 60.0 / TAU;

/// All four wheels are driven, and the engine's torque is shared equally among them.
const DRIVEN_WHEELS: f32 = 4.0;

/// About the mean net torque share over the climb from idle to the governed speed at full
/// fuel: it turns `EngineConfig::free_rev_s` into the engine's inertia.
const CLIMB_TORQUE_SHARE: f32 = 0.8;

/// The least rotational inertia, in kg·m²: a guard against a config with no free rev, not
/// a tuning value.
const MIN_INERTIA: f32 = 1e-3;

/// The engine, as plain data. The default is a Cummins 5.9-like diesel, the engine the
/// user chose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineConfig {
    /// The speed the engine idles at, in rpm. Where the tachometer and the engine's sound
    /// start; higher is a busier idle.
    pub idle_rpm: f32,
    /// The governed speed, in rpm: the top governor cuts the fuel to hold it. Each gear
    /// meets it at its share of `top_speed` (`gearbox::GEAR_TOPS`).
    pub max_rpm: f32,
    /// The most the wheels can drive the engine to, in rpm, as down a hill in a low gear.
    /// The engine's sound clamps at the same speed.
    pub over_rev_rpm: f32,
    /// A running engine dragged under this speed, in rpm, stops. Higher stalls a manual
    /// driver sooner: in third, the built-in truck stalls under about 9 m/s at 500 rpm.
    pub stall_rpm: f32,
    /// How long the engine takes, in seconds, in neutral at full fuel, from idle to the
    /// governed speed. The rotational inertia is derived from it. Shorter is a lighter
    /// engine, which also makes the governors' loops stiffer (see the test on their gains).
    pub free_rev_s: f32,
    /// The torque at full fuel, as (rpm, share of the peak torque) points, piecewise linear
    /// between them and constant beyond the ends. The top falls so that the pull near
    /// `top_speed` fades instead of holding to the governor: a lower top point lowers the
    /// speed at which the pull meets the rolling resistance on flat ground.
    pub torque_curve: [(f32, f32); 6],
    /// The mechanical loss, as a share of the peak torque, at zero speed and at `max_rpm`,
    /// linear between; blended in by how far the fuel is lifted. The second figure is the
    /// engine braking: higher slows the truck harder off the throttle in gear.
    pub friction: (f32, f32),
    /// Over this band under `max_rpm`, in rpm, the fuel is cut progressively: a diesel's
    /// governor, not an ignition cut. Coupled with `gearbox::SHIFT_UP` (0.92 of the limit
    /// is 2944 rpm, just under the band): a narrower band, or a curve that falls sooner,
    /// moves the best shift point.
    pub governor_band_rpm: f32,
    /// The idle governor adds fuel in proportion as the speed droops under `idle_rpm`, all
    /// of it this far under, in rpm. Narrower holds the idle tighter, but with the per-step
    /// speed change at full fuel the loop oscillates (see the test on gains).
    pub idle_band_rpm: f32,
    /// Whether there is a turbo. Without one the boost is 0 and the torque the curve's.
    pub turbocharged: bool,
    /// How long the boost takes to follow its target going up, and coming down, in
    /// seconds: the time to close all but a third of the gap. The lag the driver feels
    /// when the throttle goes down.
    pub spool_up_s: f32,
    pub spool_down_s: f32,
    /// How much boost there is at the governed speed with no fuel, from 0 to 1: the exhaust
    /// still flows.
    pub boost_no_load: f32,
    /// How much of the curve's torque needs full boost, from 0 to 1: with no boost the
    /// engine makes 1 minus this of the curve. Higher is more turbo lag.
    pub boost_torque_share: f32,
    /// The torque the clutch or the converter transmits when fully engaged, as a share of
    /// the peak torque. Over 1, so that a locked engine stays locked under its own torque.
    pub clutch_capacity: f32,
    /// After every change of gear, the engagement ramps from 0 to 1 over this time, in
    /// seconds, so that the engine is brought to the new gear's speed over it and not in
    /// one dump. Longer is a softer change.
    pub clutch_engage_s: f32,
    /// The speed, in rpm, by which the converter is fully engaged pulling away. The engine
    /// settles lower, where its torque meets the capacity (see `engine`'s launch test).
    /// Lower pulls away at lower revs, with less wheelspin.
    pub launch_rpm: f32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            idle_rpm: 770.0,
            max_rpm: 3200.0,
            over_rev_rpm: 3840.0,
            stall_rpm: 500.0,
            free_rev_s: 0.25,
            torque_curve: [
                (0.0, 0.35),
                (770.0, 0.6),
                (1600.0, 1.0),
                (2400.0, 0.9),
                (3200.0, 0.35),
                (3500.0, 0.0),
            ],
            friction: (0.05, 0.15),
            governor_band_rpm: 250.0,
            idle_band_rpm: 120.0,
            turbocharged: true,
            spool_up_s: 0.9,
            spool_down_s: 0.4,
            boost_no_load: 0.25,
            boost_torque_share: 0.25,
            clutch_capacity: 1.3,
            clutch_engage_s: 0.25,
            launch_rpm: 1800.0,
        }
    }
}

/// What one step of the engine is given besides its own state: plain data, so that
/// `EngineConfig::step` is a pure function.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct EngineInputs {
    /// From -1 to 1, as `TruckInput::throttle`.
    pub(super) throttle: f32,
    pub(super) gear: Gear,
    /// Whether a change of gear is under way (`Gearbox::shifting`).
    pub(super) shifting: bool,
    pub(super) clutch_mode: ClutchMode,
    /// The chassis' speed along the truck's forward, in m/s.
    pub(super) forward_speed: f32,
    /// `TruckConfig::engine_force`, `top_speed` and `wheel_radius`, as they stand this
    /// step.
    pub(super) engine_force: f32,
    pub(super) top_speed: f32,
    pub(super) wheel_radius: f32,
}

/// On a truck: its engine, as it stands after the last physics step. Written by
/// `run_engines`; read by `drive`, the dashboard, the speedometer and the sound slice.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Engine {
    /// The crank's speed, in rad/s.
    omega: f32,
    revs: f32,
    boost: f32,
    boost_target: f32,
    boost_tau_s: f32,
    clutch: f32,
    /// The clutch's engagement ramp since the last change of gear, from 0 to 1.
    ramp: f32,
    running: bool,
    fueling: f32,
    load: f32,
    torque: f32,
    rpm_rate: f32,
    /// The gear of the last step, so that a change of gear is told.
    gear: Gear,
    /// The force each driven wheel is asked to push with, in N, along the truck's forward:
    /// negative in reverse, and when the engine brakes. `drive` reads it.
    pub(super) wheel_force: f32,
}

impl Default for Engine {
    /// `Engine::at_idle` for the default engine. A truck is spawned with `at_idle` for its
    /// own engine.
    fn default() -> Self {
        Self::at_idle(&EngineConfig::default())
    }
}

impl Engine {
    /// Running, at `config`'s idle, with no boost and the clutch disengaged, in first: the
    /// gear `Gearbox::default` is in, so that the first step sees no change of gear.
    pub fn at_idle(config: &EngineConfig) -> Self {
        Self {
            omega: config.idle_rpm * RAD_PER_RPM,
            revs: 0.0,
            boost: 0.0,
            boost_target: 0.0,
            boost_tau_s: config.spool_down_s,
            clutch: 0.0,
            ramp: 1.0,
            running: true,
            fueling: 0.0,
            load: 0.0,
            torque: 0.0,
            rpm_rate: 0.0,
            gear: Gear::Forward(1),
            wheel_force: 0.0,
        }
    }

    /// The engine's speed, in rpm. 0 while stalled.
    pub fn rpm(&self) -> f32 {
        self.omega * RPM_PER_RAD
    }

    /// The engine's speed as a share of its range: **0 at idle, 1 at the governed speed**
    /// (`EngineConfig::max_rpm`), which is what the tachometer's dial and the engine loops'
    /// pitch map. Clamped: under idle, and while stalled, it is 0; driven over the governor
    /// by the wheels, down a hill, it is 1.
    pub fn revs(&self) -> f32 {
        self.revs
    }

    /// Whether the engine runs. False while stalled: it makes no sound and pushes nothing
    /// until the throttle is let go or the gear is changed.
    pub fn running(&self) -> bool {
        self.running
    }

    /// The fuel the engine burns after the governors, from 0 to 1: what the combustion
    /// sound follows. About 0.14 at idle; 0 on the overrun.
    pub fn fueling(&self) -> f32 {
        self.fueling
    }

    /// The torque the clutch transmits to the wheels, as a share of the peak torque, from
    /// 0 to 1. Engine braking is 0.
    pub fn load(&self) -> f32 {
        self.load
    }

    /// The turbo's boost, from 0 to 1.
    pub fn boost(&self) -> f32 {
        self.boost
    }

    /// The boost the turbo spools towards this step, from 0 to 1, and the time constant it
    /// spools with, in seconds (`spool_up_s` if the target is above the boost, else
    /// `spool_down_s`). The sound carries the boost on between frames with them.
    pub fn boost_target(&self) -> f32 {
        self.boost_target
    }

    pub fn boost_tau_s(&self) -> f32 {
        self.boost_tau_s
    }

    /// The net torque at the crank, in N·m: negative off the throttle.
    pub fn torque(&self) -> f32 {
        self.torque
    }

    /// How far the clutch is engaged, from 0 (open) to 1.
    pub fn clutch(&self) -> f32 {
        self.clutch
    }

    /// How fast the speed changed over the last step, in rpm per second. The sound carries
    /// the speed on between frames with it.
    pub fn rpm_rate(&self) -> f32 {
        self.rpm_rate
    }
}

impl EngineConfig {
    /// The torque at full fuel at `rpm`, as a share of the peak torque: the curve.
    fn curve(&self, rpm: f32) -> f32 {
        let points = &self.torque_curve;
        if rpm <= points[0].0 {
            return points[0].1;
        }
        for pair in points.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            if rpm <= to.0 {
                let span = to.0 - from.0;
                let along = if span > 0.0 {
                    (rpm - from.0) / span
                } else {
                    1.0
                };
                return from.1 + (to.1 - from.1) * along;
            }
        }
        points[points.len() - 1].1
    }

    /// The mechanical loss at `rpm`, as a share of the peak torque.
    fn friction(&self, rpm: f32) -> f32 {
        let (still, fast) = self.friction;
        still + (fast - still) * rpm / self.max_rpm.max(1.0)
    }

    /// How far `rpm` is from idle to the governed speed, from 0 to 1.
    fn speed_share(&self, rpm: f32) -> f32 {
        ((rpm - self.idle_rpm) / (self.max_rpm - self.idle_rpm).max(1.0)).clamp(0.0, 1.0)
    }

    /// The boost the turbo spools towards at `rpm` with `fueling`, from 0 to 1.
    fn boost_target(&self, rpm: f32, fueling: f32) -> f32 {
        if !self.turbocharged {
            return 0.0;
        }
        self.speed_share(rpm) * (self.boost_no_load + (1.0 - self.boost_no_load) * fueling)
    }

    /// The boost after `dt` seconds of spooling from `boost` towards `target`, and the time
    /// constant it spooled with.
    fn spool(&self, boost: f32, target: f32, dt: f32) -> (f32, f32) {
        let tau = if target > boost {
            self.spool_up_s
        } else {
            self.spool_down_s
        };
        if !self.turbocharged || tau <= 0.0 {
            return (target, tau);
        }
        (target + (boost - target) * (-dt / tau).exp(), tau)
    }

    /// What `boost` multiplies the curve's torque by. Without a turbo, nothing.
    fn boost_factor(&self, boost: f32) -> f32 {
        if !self.turbocharged {
            return 1.0;
        }
        1.0 - self.boost_torque_share + self.boost_torque_share * boost
    }

    /// The net torque at full fuel at `rpm`, as a share of the peak torque, with the boost
    /// settled at its full-fuel target. The friction is blended in by the lifted fuel, so
    /// at full fuel it is the curve alone, times the boost.
    fn full_fuel_factor(&self, rpm: f32) -> f32 {
        self.curve(rpm) * self.boost_factor(self.boost_target(rpm, 1.0))
    }

    /// The speed, in rpm, at which the engine makes the most net torque at full fuel, and
    /// that torque as a share of the peak torque: the largest of `full_fuel_factor` at the
    /// curve's points and halfway between them (on each segment it is a parabola, so this
    /// is within a hair of the true peak).
    pub(super) fn peak(&self) -> (f32, f32) {
        let mut best = (self.idle_rpm, 0.0);
        let mut consider = |rpm: f32| {
            let factor = self.full_fuel_factor(rpm);
            if factor > best.1 {
                best = (rpm, factor);
            }
        };
        for pair in self.torque_curve.windows(2) {
            consider(pair[0].0);
            consider((pair[0].0 + pair[1].0) / 2.0);
        }
        consider(self.torque_curve[self.torque_curve.len() - 1].0);
        best
    }

    /// The engine's speed over the wheels' in `gear`, on a truck with `top_speed` and
    /// `wheel_radius`, so that the gear meets the governed speed at its share of the top
    /// speed. None in neutral, and on a truck with no top speed or no wheel radius, where
    /// nothing couples the engine to the wheels.
    fn ratio(&self, gear: Gear, top_speed: f32, wheel_radius: f32) -> Option<f32> {
        let top = gearbox::gear_top(gear, top_speed);
        (top > 0.0 && wheel_radius > 0.0).then(|| self.max_rpm * RAD_PER_RPM * wheel_radius / top)
    }

    /// The peak torque, in N·m: what makes the locked first-gear pull at the curve's best
    /// speed `engine_force` per wheel. At least 1 N·m, so that a truck with no engine force
    /// still has finite figures.
    pub(super) fn peak_torque(&self, engine_force: f32, top_speed: f32, wheel_radius: f32) -> f32 {
        let (_, factor) = self.peak();
        let ratio_1 = self
            .ratio(Gear::Forward(1), top_speed, wheel_radius)
            .unwrap_or(1.0);
        (engine_force * DRIVEN_WHEELS * wheel_radius / (ratio_1 * factor)).max(1.0)
    }

    /// The rotational inertia, in kg·m², that gives the free rev its time. For the
    /// built-in truck about 6 kg·m², which reflected through first gear is about a third
    /// of the truck's mass: a heavy flywheel, the price of game-scale torque with a
    /// believable free rev. At least `MIN_INERTIA`, so that a free rev of no time still
    /// gives finite figures.
    fn inertia(&self, peak_torque: f32) -> f32 {
        let range = (self.max_rpm - self.idle_rpm).max(1.0) * RAD_PER_RPM;
        (peak_torque * CLIMB_TORQUE_SHARE * self.free_rev_s / range).max(MIN_INERTIA)
    }

    /// The engine after one physics step of `dt` seconds from `engine`, given `inputs`.
    /// Pure. See the module's notes for the model.
    pub(super) fn step(&self, engine: Engine, inputs: &EngineInputs, dt: f32) -> Engine {
        let mut next = engine;
        if dt <= 0.0 {
            return next;
        }
        let idle = self.idle_rpm * RAD_PER_RPM;
        // The bands are divided by: at least 1 rpm, so that a config with none still gives
        // finite figures.
        let governor_band = self.governor_band_rpm.max(1.0);
        let idle_band = self.idle_band_rpm.max(1.0);
        let gear_changed = inputs.gear != engine.gear;
        next.gear = inputs.gear;

        // The throttle drives reverse the way it goes; while the gear changes the engine
        // lifts.
        let direction = if inputs.gear == Gear::Reverse {
            -1.0
        } else {
            1.0
        };
        let demand = if inputs.shifting {
            0.0
        } else {
            (direction * inputs.throttle).clamp(0.0, 1.0)
        };

        // A stalled engine starts again, at idle, when the throttle is let go or the gear
        // is changed. The starter is later work.
        if !engine.running && (demand <= 0.0 || gear_changed) {
            next.running = true;
            next.omega = idle;
        }
        let omega = next.omega;
        let rpm = omega * RPM_PER_RAD;

        let ratio = self.ratio(inputs.gear, inputs.top_speed, inputs.wheel_radius);
        let mode = if ratio.is_some() {
            inputs.clutch_mode
        } else {
            ClutchMode::Open
        };
        // The wheels' speed on the engine's side of the clutch. Rolling back down a hill
        // in a forward gear it is negative, and the engine never locks to it; a converter
        // takes it as stopped.
        let omega_wheels = ratio.map_or(0.0, |ratio| {
            let wheels = direction * inputs.forward_speed / inputs.wheel_radius * ratio;
            if mode == ClutchMode::Converter {
                wheels.max(0.0)
            } else {
                wheels
            }
        });

        let mut fueling = if !next.running {
            0.0
        } else {
            let demanded = if mode == ClutchMode::Open {
                // The all-speed governor: the pedal sets the speed.
                let asked = self.idle_rpm + demand * (self.max_rpm - self.idle_rpm);
                ((asked - rpm) / governor_band).clamp(0.0, 1.0)
            } else {
                demand
            };
            let idling = ((self.idle_rpm - rpm) / idle_band).clamp(0.0, 1.0);
            let governor = ((self.max_rpm - rpm) / governor_band).clamp(0.0, 1.0);
            demanded.max(idling) * governor
        };

        let boost_target = self.boost_target(rpm, fueling);
        let (boost, boost_tau_s) = self.spool(engine.boost, boost_target, dt);

        let peak = self.peak_torque(inputs.engine_force, inputs.top_speed, inputs.wheel_radius);
        let inertia = self.inertia(peak);
        // The friction blended by the fuel: the curve is the net torque at full fuel, and
        // the friction the closed-throttle drag. Subtracted whole, it halved the pull at
        // half throttle and made the neutral pedal a switch.
        let engine_torque = fueling * peak * self.curve(rpm) * self.boost_factor(boost)
            - (1.0 - fueling) * peak * self.friction(rpm);

        let ramp = if gear_changed {
            0.0
        } else if inputs.shifting {
            engine.ramp
        } else {
            (engine.ramp + dt / self.clutch_engage_s).min(1.0)
        };
        let engagement = match mode {
            ClutchMode::Open => 0.0,
            ClutchMode::Manual => ramp,
            ClutchMode::Converter => {
                let turning = omega.max(omega_wheels) * RPM_PER_RAD;
                let by_speed =
                    (turning - self.idle_rpm) / (self.launch_rpm - self.idle_rpm).max(1.0);
                ramp.min(by_speed.clamp(0.0, 1.0))
            }
        };

        let (mut omega_next, mut transmitted) = if !next.running {
            (0.0, 0.0)
        } else if engagement > 0.0 {
            // The torque that would lock the engine to the wheels this step. Within the
            // capacity the clutch transmits it and the engine turns with the wheels; over
            // it the clutch slips and transmits the capacity.
            let lock = engine_torque + inertia * (omega - omega_wheels) / dt;
            let capacity = engagement * self.clutch_capacity * peak;
            let mut transmitted = lock.clamp(-capacity, capacity);
            if mode == ClutchMode::Converter {
                // A converter cannot stall the engine: it drags it no lower than idle. The
                // limit stays within the capacity: with the engine far under idle and the
                // wheels turning fast (which no reachable state gives), it would otherwise
                // ask for more braking torque than the converter can pass.
                let no_lower_than_idle = engine_torque + inertia * (omega - idle) / dt;
                transmitted = transmitted.min(no_lower_than_idle.max(-capacity));
            }
            (
                omega + (engine_torque - transmitted) * dt / inertia,
                transmitted,
            )
        } else {
            (omega + engine_torque * dt / inertia, 0.0)
        };
        omega_next = omega_next.clamp(0.0, self.over_rev_rpm * RAD_PER_RPM);

        if next.running && mode == ClutchMode::Manual && omega_next < self.stall_rpm * RAD_PER_RPM {
            next.running = false;
            omega_next = 0.0;
            fueling = 0.0;
            transmitted = 0.0;
        }

        next.rpm_rate = (omega_next - omega) * RPM_PER_RAD / dt;
        next.omega = omega_next;
        next.revs = self.speed_share(omega_next * RPM_PER_RAD);
        next.boost = boost;
        next.boost_target = boost_target;
        next.boost_tau_s = boost_tau_s;
        next.ramp = ramp;
        next.clutch = engagement;
        next.fueling = fueling;
        next.torque = engine_torque;
        next.load = (transmitted / peak).clamp(0.0, 1.0);
        next.wheel_force = ratio.map_or(0.0, |ratio| {
            direction * transmitted * ratio / inputs.wheel_radius / DRIVEN_WHEELS
        });
        next
    }
}

/// What `run_engines` reads and writes of each truck.
type EnginedTruck = (
    &'static Transform,
    &'static LinearVelocity,
    &'static TruckConfig,
    &'static TruckInput,
    &'static Gearbox,
    &'static mut Engine,
);

/// Steps each truck's engine for the coming physics step, after `gearbox::change_gears`
/// has chosen its gear and before `drive` asks the wheels for its force.
pub(super) fn run_engines(time: Res<Time>, mut trucks: Query<EnginedTruck, With<Truck>>) {
    let dt = time.delta_secs();
    for (transform, velocity, config, input, gearbox, mut engine) in &mut trucks {
        let inputs = EngineInputs {
            throttle: input.throttle,
            gear: gearbox.gear(),
            shifting: gearbox.shifting(),
            clutch_mode: gearbox.clutch_mode,
            forward_speed: velocity.0.dot(transform.forward().as_vec3()),
            engine_force: config.engine_force,
            top_speed: config.top_speed,
            wheel_radius: config.wheel_radius,
        };
        *engine = config.engine.step(*engine, &inputs, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::super::FORWARD_GEARS;
    use super::gearbox::{GEAR_TOPS, SHIFT_UP};
    use super::*;

    const DT: f32 = 1.0 / 120.0;

    fn config() -> EngineConfig {
        EngineConfig::default()
    }

    /// Inputs for the built-in truck.
    fn inputs(gear: Gear, mode: ClutchMode, throttle: f32, forward_speed: f32) -> EngineInputs {
        let truck = TruckConfig::default();
        EngineInputs {
            throttle,
            gear,
            shifting: false,
            clutch_mode: mode,
            forward_speed,
            engine_force: truck.engine_force,
            top_speed: truck.top_speed,
            wheel_radius: truck.wheel_radius,
        }
    }

    /// An engine running at `rpm`, with its boost settled for `fueling` and the clutch
    /// ramp over, in `gear`.
    fn engine_at(rpm: f32, fueling: f32, gear: Gear) -> Engine {
        let config = config();
        Engine {
            omega: rpm * RAD_PER_RPM,
            boost: config.boost_target(rpm, fueling),
            gear,
            ..Engine::default()
        }
    }

    /// The chassis speed, in m/s, at which the wheels turn the engine at `rpm` in `gear`.
    fn speed_for(rpm: f32, gear: Gear) -> f32 {
        let truck = TruckConfig::default();
        let ratio = config()
            .ratio(gear, truck.top_speed, truck.wheel_radius)
            .expect("a gear");
        rpm * RAD_PER_RPM * truck.wheel_radius / ratio
    }

    /// Runs the engine for `seconds`, returning every state.
    fn run(mut engine: Engine, inputs: &EngineInputs, seconds: f32) -> Vec<Engine> {
        let steps = (seconds / DT).round() as usize;
        (0..steps)
            .map(|_| {
                engine = config().step(engine, inputs, DT);
                engine
            })
            .collect()
    }

    /// The wheel force, in N, with the engine locked to the wheels at `rpm` in `gear` at
    /// full throttle, boost settled.
    fn locked_force(gear: Gear, rpm: f32) -> f32 {
        let inputs = inputs(gear, ClutchMode::Manual, 1.0, speed_for(rpm, gear));
        let engine = config().step(engine_at(rpm, 1.0, gear), &inputs, DT);
        assert!(
            (engine.rpm() - rpm).abs() < 1.0,
            "{} not locked at {rpm}",
            engine.rpm()
        );
        engine.wheel_force
    }

    #[test]
    fn the_curve_is_linear_between_its_points_and_flat_beyond_them() {
        let config = config();
        assert_eq!(config.curve(-100.0), config.torque_curve[0].1);
        assert_eq!(config.curve(1600.0), 1.0);
        assert!((config.curve(2000.0) - 0.95).abs() < 1e-5);
        assert_eq!(config.curve(5000.0), 0.0);
    }

    #[test]
    fn the_free_rev_in_neutral_reaches_the_governor_in_time_and_never_passes_it() {
        let config = config();
        let inputs = inputs(Gear::Neutral, ClutchMode::Open, 1.0, 0.0);
        let states = run(Engine::default(), &inputs, 1.5 * config.free_rev_s + 1.0);
        let band = config.max_rpm - config.governor_band_rpm;
        let reached = states
            .iter()
            .position(|engine| engine.rpm() >= band)
            .expect("the engine reaches the governor's band");
        let time = (reached + 1) as f32 * DT;
        assert!(time <= 1.5 * config.free_rev_s, "{time} s to {band} rpm");
        for engine in &states {
            assert!(engine.rpm() <= config.max_rpm, "{} rpm", engine.rpm());
        }
        for engine in &states[states.len() / 2..] {
            assert!(
                engine.rpm() >= band,
                "{} rpm fell out of the band",
                engine.rpm()
            );
        }
        // The mean climb is about the configured free rev, with the boost lagging it.
        let mean_rate = states[..=reached]
            .iter()
            .map(|engine| engine.rpm_rate())
            .sum::<f32>()
            / (reached + 1) as f32;
        let expected = (config.max_rpm - config.idle_rpm) / config.free_rev_s;
        assert!(
            mean_rate > 0.5 * expected && mean_rate < 1.5 * expected,
            "{mean_rate} rpm/s against {expected}"
        );
    }

    #[test]
    fn the_idle_holds_in_neutral_and_in_gear_at_a_standstill() {
        let config = config();
        for inputs in [
            inputs(Gear::Neutral, ClutchMode::Open, 0.0, 0.0),
            inputs(Gear::Forward(1), ClutchMode::Converter, 0.0, 0.0),
        ] {
            let states = run(Engine::default(), &inputs, 2.0);
            for engine in &states {
                assert!(
                    (engine.rpm() - config.idle_rpm).abs() <= config.idle_band_rpm,
                    "{} rpm in {:?}",
                    engine.rpm(),
                    inputs.gear
                );
                assert!(engine.running());
                assert_eq!(engine.wheel_force, 0.0);
            }
            let fueling = states.last().unwrap().fueling();
            assert!((0.1..=0.25).contains(&fueling), "{fueling}");
        }
    }

    #[test]
    fn a_disturbed_idle_returns_without_oscillating() {
        let config = config();
        let inputs = inputs(Gear::Neutral, ClutchMode::Open, 0.0, 0.0);
        let low = engine_at(config.idle_rpm - 200.0, 0.0, Gear::Neutral);
        let states = run(low, &inputs, 2.0);
        let crossings = states
            .windows(2)
            .filter(|pair| {
                (pair[0].rpm() - config.idle_rpm).signum()
                    != (pair[1].rpm() - config.idle_rpm).signum()
            })
            .count();
        assert!(crossings <= 1, "{crossings} crossings of idle");
        let settled = states.last().unwrap().rpm();
        assert!(
            (settled - config.idle_rpm).abs() < 0.25 * config.idle_band_rpm,
            "{settled}"
        );
    }

    #[test]
    fn the_governors_per_step_gains_stay_under_a_half() {
        let config = config();
        let truck = TruckConfig::default();
        let peak = config.peak_torque(truck.engine_force, truck.top_speed, truck.wheel_radius);
        let step_rpm = peak * DT / config.inertia(peak) * RPM_PER_RAD;
        let idle_gain = config.full_fuel_factor(config.idle_rpm) * step_rpm / config.idle_band_rpm;
        let governor_gain = config.peak().1 * step_rpm / config.governor_band_rpm;
        assert!(
            idle_gain < 0.5,
            "idle gain {idle_gain} (step {step_rpm} rpm)"
        );
        assert!(
            governor_gain < 0.5,
            "governor gain {governor_gain} (step {step_rpm} rpm)"
        );
    }

    #[test]
    fn half_throttle_in_neutral_holds_the_middle_of_the_range() {
        let config = config();
        let inputs = inputs(Gear::Neutral, ClutchMode::Open, 0.5, 0.0);
        let settled = run(Engine::default(), &inputs, 2.0).last().unwrap().rpm();
        let middle = (config.idle_rpm + config.max_rpm) / 2.0;
        assert!(
            (settled - middle).abs() < 150.0,
            "{settled} rpm against {middle}"
        );
    }

    #[test]
    fn the_locked_first_gear_pull_at_the_curves_best_is_the_engine_force() {
        let truck = TruckConfig::default();
        let (best_rpm, _) = config().peak();
        let force = locked_force(Gear::Forward(1), best_rpm);
        assert!(
            (force / truck.engine_force - 1.0).abs() < 0.01,
            "{force} N per wheel at {best_rpm} rpm against {}",
            truck.engine_force
        );
        // The higher gears push the ratio of their gearing.
        let second = locked_force(Gear::Forward(2), best_rpm);
        assert!(
            (second / force - GEAR_TOPS[0] / GEAR_TOPS[1]).abs() < 0.01,
            "{second}"
        );
    }

    #[test]
    fn the_automatic_changes_up_before_the_pull_falls_and_keeps_most_of_it() {
        let config = config();
        for gear in 1..=FORWARD_GEARS {
            let before = locked_force(Gear::Forward(gear), SHIFT_UP * config.max_rpm);
            let at_top = locked_force(Gear::Forward(gear), 0.98 * config.max_rpm);
            assert!(
                before >= at_top,
                "gear {gear}: {before} before, {at_top} at the top"
            );
            if gear < FORWARD_GEARS {
                let tops = (
                    GEAR_TOPS[usize::from(gear) - 1],
                    GEAR_TOPS[usize::from(gear)],
                );
                let after_rpm = SHIFT_UP * config.max_rpm * tops.0 / tops.1;
                let after = locked_force(Gear::Forward(gear + 1), after_rpm);
                assert!(
                    after >= 0.6 * before,
                    "gear {gear}: {before} before, {after} after"
                );
            }
        }
    }

    #[test]
    fn a_launch_slips_the_converter_where_the_torque_meets_its_capacity() {
        let config = config();
        let truck = TruckConfig::default();
        let peak = config.peak_torque(truck.engine_force, truck.top_speed, truck.wheel_radius);
        let inputs = inputs(Gear::Forward(1), ClutchMode::Converter, 1.0, 0.0);
        let states = run(Engine::default(), &inputs, 3.0);
        for engine in &states {
            assert!(engine.running());
            assert!(
                engine.wheel_force >= 0.0,
                "{} N flipped",
                engine.wheel_force
            );
        }
        let settled = states.last().unwrap();
        assert!(
            (1100.0..=1700.0).contains(&settled.rpm()),
            "{} rpm",
            settled.rpm()
        );
        assert!(settled.wheel_force > 0.0);
        let by_capacity = settled.clutch() * config.clutch_capacity;
        let by_torque = settled.torque() / peak;
        assert!(
            (by_capacity - by_torque).abs() < 0.05,
            "{by_capacity} against {by_torque}"
        );
    }

    #[test]
    fn the_converter_never_stalls_coasting_to_a_stop() {
        let config = config();
        let gear = Gear::Forward(3);
        let mut engine = engine_at(2000.0, 0.0, gear);
        let steps = (5.0 / DT) as usize;
        for step in 0..steps {
            let speed = speed_for(2000.0, gear) * (1.0 - step as f32 / steps as f32);
            let inputs = inputs(gear, ClutchMode::Converter, 0.0, speed);
            engine = config.step(engine, &inputs, DT);
            assert!(engine.running(), "stalled at {speed} m/s");
        }
        assert!(
            (engine.rpm() - config.idle_rpm).abs() <= config.idle_band_rpm,
            "{}",
            engine.rpm()
        );
    }

    #[test]
    fn a_manual_driver_stalls_third_at_a_standstill_and_not_at_speed() {
        let config = config();
        let gear = Gear::Forward(3);
        let standing = inputs(gear, ClutchMode::Manual, 1.0, 0.0);
        let states = run(Engine::default(), &standing, 1.0);
        let stalled = states.iter().position(|engine| !engine.running());
        assert!(stalled.is_some(), "still running after a second");
        let stalled = *states.last().unwrap();
        assert!(!stalled.running() && stalled.rpm() == 0.0 && stalled.wheel_force == 0.0);
        assert_eq!(stalled.fueling(), 0.0);
        // Lifting the throttle starts it again, at idle.
        let lifted = inputs(gear, ClutchMode::Converter, 0.0, 0.0);
        let restarted = config.step(stalled, &lifted, DT);
        assert!(restarted.running());
        assert!((restarted.rpm() - config.idle_rpm).abs() < config.idle_band_rpm);
        // Fast enough for the gear, it pulls.
        let rolling = inputs(gear, ClutchMode::Manual, 1.0, 15.0);
        for engine in run(Engine::default(), &rolling, 2.0) {
            assert!(engine.running(), "stalled at 15 m/s");
        }
    }

    #[test]
    fn over_the_governor_downhill_the_wheels_turn_the_engine_and_it_brakes() {
        let config = config();
        let gear = Gear::Forward(2);
        let downhill = 1.1 * config.max_rpm;
        let inputs = inputs(gear, ClutchMode::Converter, 0.0, speed_for(downhill, gear));
        let settled = *run(Engine::default(), &inputs, 2.0).last().unwrap();
        assert!(
            (settled.rpm() / downhill - 1.0).abs() < 0.01,
            "{} rpm",
            settled.rpm()
        );
        assert_eq!(settled.fueling(), 0.0);
        assert_eq!(settled.revs(), 1.0);
        assert!(settled.wheel_force < 0.0, "{} N", settled.wheel_force);
        assert_eq!(settled.load(), 0.0);
    }

    #[test]
    fn the_engine_brakes_in_gear_off_the_throttle() {
        let gear = Gear::Forward(1);
        let inputs = inputs(gear, ClutchMode::Converter, 0.0, speed_for(2000.0, gear));
        let settled = *run(engine_at(2000.0, 0.0, gear), &inputs, 1.0)
            .last()
            .unwrap();
        // About `friction.1` of the peak torque through the gear: a few kN per wheel.
        assert!(settled.wheel_force < -1000.0, "{} N", settled.wheel_force);
    }

    #[test]
    fn reverse_pushes_backwards() {
        let inputs = inputs(Gear::Reverse, ClutchMode::Converter, -1.0, 0.0);
        let settled = *run(Engine::default(), &inputs, 1.0).last().unwrap();
        assert!(settled.running());
        assert!(settled.wheel_force < 0.0, "{} N", settled.wheel_force);
        assert!(settled.rpm() > config().idle_rpm);
    }

    #[test]
    fn the_boost_spools_up_under_load_and_down_when_the_throttle_lifts() {
        let config = config();
        let mut boost = 0.0;
        for _ in 0..120 {
            boost = config
                .spool(boost, config.boost_target(config.max_rpm, 1.0), DT)
                .0;
        }
        assert!(boost > 0.6, "{boost} after a second at full fuel");
        for _ in 0..120 {
            boost = config
                .spool(boost, config.boost_target(config.idle_rpm, 0.0), DT)
                .0;
        }
        assert!(boost < 0.1, "{boost} a second after lifting off");
        // Without a turbo there is none, and the torque is the curve's.
        let plain = EngineConfig {
            turbocharged: false,
            ..config
        };
        assert_eq!(plain.boost_target(config.max_rpm, 1.0), 0.0);
        assert_eq!(plain.full_fuel_factor(1600.0), 1.0);
    }

    #[test]
    fn a_shift_opens_the_clutch_and_ramps_it_back_in() {
        let config = config();
        let gear = Gear::Forward(2);
        // Fast enough in the new gear for the converter to be fully engaged by speed.
        let mut inputs = inputs(gear, ClutchMode::Open, 1.0, speed_for(1900.0, gear));
        inputs.shifting = true;
        // The gear changed this step: the ramp starts over.
        let shifting = config.step(engine_at(2900.0, 1.0, Gear::Forward(1)), &inputs, DT);
        assert_eq!(shifting.clutch(), 0.0);
        assert_eq!(shifting.wheel_force, 0.0);
        assert!(
            shifting.rpm() < 2900.0,
            "the engine lifts while the gear changes"
        );
        inputs.shifting = false;
        inputs.clutch_mode = ClutchMode::Converter;
        // The clutch's capacity is only a little over the engine's full-throttle torque,
        // so it pulls the engine down slowly until the ramp is well along: the engine
        // reaches the new gear's speed in about twice `clutch_engage_s`.
        let engaged = run(shifting, &inputs, config.clutch_engage_s * 3.0);
        let partway = engaged[engaged.len() / 6];
        assert!(
            partway.clutch() > 0.0 && partway.clutch() < 1.0,
            "{}",
            partway.clutch()
        );
        let settled = engaged.last().unwrap();
        assert_eq!(settled.clutch(), 1.0);
        assert!(
            (settled.rpm() - 1900.0).abs() < 1.0,
            "{} rpm",
            settled.rpm()
        );
    }

    #[test]
    fn no_state_makes_a_nan() {
        let truck = TruckConfig::default();
        let checked = sweep_is_finite(&config(), truck.wheel_radius);
        assert!(checked > 10_000);
    }

    #[test]
    fn no_state_makes_a_nan_with_a_degenerate_config() {
        // No free rev, no bands, the governed speed and the launch speed at idle, and no
        // wheel radius: every divisor the config feeds is zero.
        let config = EngineConfig {
            free_rev_s: 0.0,
            idle_band_rpm: 0.0,
            governor_band_rpm: 0.0,
            max_rpm: 770.0,
            launch_rpm: 770.0,
            ..config()
        };
        sweep_is_finite(&config, 0.0);
        sweep_is_finite(&config, TruckConfig::default().wheel_radius);
    }

    /// Steps `config` from every state in a sweep of speed, demand, wheel speed, gear,
    /// mode and running, and checks every output figure is finite. Returns how many states
    /// were checked.
    fn sweep_is_finite(config: &EngineConfig, wheel_radius: f32) -> usize {
        let truck = TruckConfig::default();
        let gears = [
            Gear::Reverse,
            Gear::Neutral,
            Gear::Forward(1),
            Gear::Forward(2),
            Gear::Forward(3),
        ];
        let modes = [ClutchMode::Open, ClutchMode::Converter, ClutchMode::Manual];
        let mut checked = 0;
        for gear in gears {
            for mode in modes {
                for rpm_step in 0..=16 {
                    let rpm = config.over_rev_rpm * rpm_step as f32 / 16.0;
                    for demand_step in -4..=4 {
                        let throttle = demand_step as f32 / 4.0;
                        for wheels_step in -4..=4 {
                            let omega_wheels = 100.0 * wheels_step as f32;
                            let ratio = config.ratio(gear, truck.top_speed, wheel_radius);
                            let speed =
                                ratio.map_or(0.0, |ratio| omega_wheels * wheel_radius / ratio);
                            for running in [true, false] {
                                let engine = Engine {
                                    running,
                                    ..engine_at(rpm, 0.5, gear)
                                };
                                let inputs = EngineInputs {
                                    wheel_radius,
                                    ..inputs(gear, mode, throttle, speed)
                                };
                                let next = config.step(engine, &inputs, DT);
                                for figure in [
                                    next.rpm(),
                                    next.revs(),
                                    next.fueling(),
                                    next.load(),
                                    next.boost(),
                                    next.boost_target(),
                                    next.boost_tau_s(),
                                    next.torque(),
                                    next.clutch(),
                                    next.rpm_rate(),
                                    next.wheel_force,
                                ] {
                                    assert!(
                                        figure.is_finite(),
                                        "{figure} from {engine:?} in {gear} {mode:?}"
                                    );
                                }
                                assert!(next.rpm() <= config.over_rev_rpm + 1e-3);
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
        checked
    }

    #[test]
    fn a_default_engine_idles_and_pushes_nothing() {
        let engine = Engine::default();
        assert!(engine.running());
        assert_eq!(engine.rpm().round(), EngineConfig::default().idle_rpm);
        assert_eq!(engine.revs(), 0.0);
        assert_eq!(engine.wheel_force, 0.0);
        assert_eq!(engine.clutch(), 0.0);
        // At its own engine's idle, whatever that is.
        let slow = EngineConfig {
            idle_rpm: 600.0,
            ..config()
        };
        assert_eq!(Engine::at_idle(&slow).rpm().round(), 600.0);
    }
}
