//! Not a slice: how the physics engine (Avian) is set up. `main.rs` adds it, and so does
//! every test app that runs physics, so that a test steps the world as the game does.
//!
//! Avian steps the world in `FixedPostUpdate`, after `FixedUpdate`. So a system that applies
//! forces for a step goes in `FixedUpdate`, where it runs before the step with no ordering
//! needed; one that reads what the step did goes in `FixedPostUpdate`,
//! `.after(PhysicsSystems::Writeback)`.
//!
//! A headless app must call `App::finish` and `App::cleanup` once its plugins are added,
//! as `App::run` does: Avian makes some of its resources only then.
//!
//! The step's schedules can run on one thread (`one_thread`, `PhysicsSettings::
//! threaded_schedules`, `--one-thread-schedules`). Bevy's multi-threaded executor hands
//! every system to the task pool and waits for it: at 120 steps a second, with a few dozen
//! small systems in each of the step's schedules and six substeps, the hand-offs and the
//! waiting were most of the step in the dev build. Measured on the development machine (16 cores, the `powersave`
//! governor, which wakes a thread slowly), Baja Beach with eight trucks, dev build,
//! headless, the step alone: 3.3 ms with the multi-threaded executor and 1.1 ms on one
//! thread. With one truck the multi-threaded step still cost 3.3 ms: the time was never
//! the work. Stack samples agreed: in three of every four, no thread was computing.
//! Avian's own parallel iteration (its `parallel` feature) spreads the solver's work the
//! same way and cost 0.3 ms of that 1.1 in the dev build. Both are kept on by default for
//! the release build, where the work is worth spreading: the user's decision.

use avian3d::dynamics::solver::SolverConfig;
use avian3d::prelude::*;
use bevy::ecs::schedule::{ScheduleLabel, SingleThreadedExecutor};
use bevy::prelude::*;

use crate::truck::decide_tire_contacts;

/// Choices about how finely the world is simulated, which trade how the trucks handle for
/// time on the CPU. Insert it before adding `GamePhysicsPlugin`, or change it at any time,
/// in a race too, except `threaded_schedules`, which the plugin reads once as it is added.
/// The defaults are what the trucks were tuned on.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct PhysicsSettings {
    /// Physics steps per second. A short step keeps the stiff suspension springs stable.
    /// What is drawn is interpolated between steps (see `truck/interpolate.rs`), so this
    /// needn't match the display. Each step costs about the same, so 60 costs half of 120.
    pub rate: f64,
    /// How many times the solver works through each physics step. Avian's own default is
    /// 6, and its solver is built round it: it solves each contact once for each substep,
    /// where Rapier went round each one several times in a single pass. More holds stacked
    /// and stiff things steadier and costs more. Measured on Alpine's ground with eight
    /// trucks' colliders, a step took 1.42 ms with 1 substep and 1.92 ms with 6.
    pub substeps: u32,
    /// Whether the step's schedules keep Bevy's multi-threaded executor, which hands each
    /// system to the task pool, instead of running on one thread (see the module's doc;
    /// `--one-thread-schedules` turns it off). Read once, when `GamePhysicsPlugin` is
    /// added.
    pub threaded_schedules: bool,
}

impl Default for PhysicsSettings {
    fn default() -> Self {
        Self {
            rate: 120.0,
            substeps: 6,
            threaded_schedules: true,
        }
    }
}

/// How fast, in m/s, the solver pushes apart two bodies that overlap. Avian's default is
/// 4. At 0, a contact only stops two bodies from going further into each other, and never
/// throws one out of the other: two overlapping gate leaves on The Graveyard (JUNK.POD),
/// once woken, pushed each other 2.6 m apart.
const OVERLAP_SOLVE_SPEED: f32 = 0.0;

pub struct GamePhysicsPlugin;

impl Plugin for GamePhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PhysicsPlugins::default())
            // Hands a wheel's contacts under its tread to the suspension, and shapes the
            // rest, before the solver sees them.
            .add_systems(
                PhysicsSchedule,
                decide_tire_contacts.in_set(NarrowPhaseSystems::Last),
            )
            .insert_resource(SolverConfig {
                max_overlap_solve_speed: OVERLAP_SOLVE_SPEED,
                ..default()
            })
            .init_resource::<PhysicsSettings>()
            // Before the fixed steps of the frame, which read both.
            .add_systems(
                First,
                apply_settings.run_if(resource_changed::<PhysicsSettings>),
            );
        // Every schedule of a fixed step: Bevy's five, and Avian's step and substeps.
        if !app.world().resource::<PhysicsSettings>().threaded_schedules {
            one_thread(app, FixedFirst);
            one_thread(app, FixedPreUpdate);
            one_thread(app, FixedUpdate);
            one_thread(app, FixedPostUpdate);
            one_thread(app, FixedLast);
            one_thread(app, PhysicsSchedule);
            one_thread(app, SubstepSchedule);
        }
    }
}

/// Runs the schedule on the thread that runs the step, one system after another (see the
/// module's doc). The systems that spread their own work over the task pool still do.
fn one_thread(app: &mut App, label: impl ScheduleLabel) {
    app.edit_schedule(label, |schedule| {
        schedule.set_executor(SingleThreadedExecutor::default());
    });
}

fn apply_settings(
    settings: Res<PhysicsSettings>,
    mut fixed: ResMut<Time<Fixed>>,
    mut substeps: ResMut<SubstepCount>,
) {
    fixed.set_timestep_hz(settings.rate);
    substeps.0 = settings.substeps.max(1);
}
