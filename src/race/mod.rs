//! Racing laps around the track: a countdown on the grid, checkpoint gates crossed in
//! order, lap counting and timing, and the results once the player has finished, plus the
//! gate markers, the race readout and a compass to the next checkpoint on screen.
//!
//! Uses the `track` slice for the gates and start position, and the `truck` slice for
//! the trucks. Every truck becomes a `Racer` with a place on the starting grid: the
//! player's truck (`truck::PlayerTruck`) behind the others. The readout and the keys are
//! the player's.

mod compass;
mod gate;
mod hud;
mod markers;
mod progress;
mod results;
mod start;
mod systems;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::game_state::GameState;
use crate::track::TrackSystems;
use crate::truck::TruckSystems;

pub use gate::crossed;
pub use progress::{Milestone, RaceProgress, position, standings};
pub use start::{BLUE, RaceStart};
pub use systems::BackToCheckpoint;

pub struct RacePlugin;

impl Plugin for RacePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RaceSettings>()
            .init_resource::<crate::keys::KeyBindings>()
            .init_resource::<RaceClock>()
            .init_resource::<RaceStart>()
            .add_message::<BackToCheckpoint>()
            .add_systems(
                OnEnter(GameState::Racing),
                (
                    (systems::reset_clock, start::begin_countdown).chain(),
                    markers::spawn_gate_markers.after(TrackSystems::Prepare),
                    hud::spawn_race_hud,
                    compass::spawn_compass,
                    start::spawn_count,
                    results::spawn_results,
                ),
            )
            .add_systems(
                Update,
                (
                    // These move trucks, so the trucks slice must see their requests
                    // in the same frame: see `Racer::last_position`.
                    (
                        systems::enlist_trucks,
                        systems::back_to_checkpoint.in_set(RaceSystems::BackToCheckpoint),
                        systems::restart_race,
                    )
                        .before(TruckSystems::Place),
                    markers::highlight_next_gate,
                    hud::update_race_hud,
                    start::show_count,
                    results::show_results,
                    // After the drawn truck has been placed for the frame.
                    compass::update_compass.after(TruckSystems::PlaceVisuals),
                )
                    // Outside a race there are no gates, and before the first one no
                    // banner materials either.
                    .run_if(in_state(GameState::Racing)),
            )
            // After the physics step, so that a truck's position is this tick's.
            .add_systems(
                FixedUpdate,
                (systems::tick_clock, systems::track_progress)
                    .chain()
                    .after(PhysicsSet::Writeback)
                    .run_if(in_state(GameState::Racing)),
            )
            // Before the forces, so that a truck is let go in the step of GO.
            .add_systems(
                FixedUpdate,
                start::hold_at_start
                    .before(TruckSystems::Drive)
                    .run_if(in_state(GameState::Racing)),
            );
    }
}

#[derive(Resource)]
pub struct RaceSettings {
    pub laps: u32,
}

impl Default for RaceSettings {
    fn default() -> Self {
        Self { laps: 3 }
    }
}

/// Physics ticks since the race was entered. All race times are differences of this.
#[derive(Resource, Default)]
pub struct RaceClock {
    pub tick: u64,
}

/// A truck taking part in the race.
#[derive(Component, Default)]
pub struct Racer {
    pub progress: RaceProgress,
    /// Where the truck was at the end of the previous physics tick, to compare with
    /// where it is now. `None` means "wherever it is next tick": set when the truck is
    /// moved by hand, so that being carried across a gate isn't mistaken for driving
    /// through it.
    last_position: Option<Vec3>,
    /// Which place of the starting grid is this truck's. 0 is pole position.
    grid_place: usize,
}

/// For other slices to order their systems against this one.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum RaceSystems {
    /// Handles `BackToCheckpoint` messages, in `Update`. Write them from a system that
    /// runs before this set and the truck moves in the same frame.
    BackToCheckpoint,
}
