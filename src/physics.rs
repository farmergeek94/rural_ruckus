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

use avian3d::dynamics::solver::SolverConfig;
use avian3d::prelude::*;
use bevy::prelude::*;

use crate::truck::TireContacts;

/// How many times the solver works through each physics step. Avian's own default, and
/// what its solver is built round: it solves each contact once for each substep, where
/// Rapier went round each one several times in a single pass. More holds stacked and
/// stiff things steadier and costs more. Measured on Alpine's ground with eight trucks'
/// colliders, a step took 1.42 ms with 1 substep and 1.92 ms with 6.
const SUBSTEPS: u32 = 6;

/// How fast, in m/s, the solver pushes apart two bodies that overlap. Avian's default is
/// 4. At 0, a contact only stops two bodies from going further into each other, and never
/// throws one out of the other: two overlapping gate leaves on The Graveyard (JUNK.POD),
/// once woken, pushed each other 2.6 m apart.
const OVERLAP_SOLVE_SPEED: f32 = 0.0;

pub struct GamePhysicsPlugin;

impl Plugin for GamePhysicsPlugin {
    fn build(&self, app: &mut App) {
        // `TireContacts` hands a wheel's contacts under its tread to the suspension.
        app.add_plugins(PhysicsPlugins::default().with_collision_hooks::<TireContacts>())
            .insert_resource(SubstepCount(SUBSTEPS))
            .insert_resource(SolverConfig {
                max_overlap_solve_speed: OVERLAP_SOLVE_SPEED,
                ..default()
            });
    }
}
