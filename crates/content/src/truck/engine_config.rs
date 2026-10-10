//! The engine as plain data: the numbers that `truck::engine` in the game simulates
//! from. The simulation stays in the game; a loader only fills these in.

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
