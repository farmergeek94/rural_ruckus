//! Racing laps around the track: a countdown on the grid, checkpoint gates crossed in
//! order, lap counting and timing, and the results once the player has finished, plus the
//! gate markers, the race readout, a compass to the next checkpoint and a map of the track
//! with every truck on it on screen. Esc pauses the race (`pause`).
//!
//! Uses the `track` slice for the gates and start position, and the `truck` slice for
//! the trucks. Every truck becomes a `Racer` with a place on the starting grid: the
//! player's truck (`truck::PlayerTruck`) behind the others. The readout and the keys are
//! the player's.

mod along;
mod compass;
mod gate;
mod hud;
mod map;
mod markers;
mod pause;
mod progress;
mod results;
mod start;
mod systems;

use avian3d::prelude::PhysicsSystems;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

use crate::game_state::GameState;
use crate::keys::{Control, just_pressed};
use crate::track::TrackSystems;
use crate::truck::TruckSystems;

pub use gate::crossed;
pub use pause::{RaceCancelled, RacePause};
pub use progress::{Milestone, RaceProgress, position, standings};
pub use start::{BLUE, RaceStart};
pub use systems::BackToCheckpoint;

pub struct RacePlugin;

impl Plugin for RacePlugin {
    fn build(&self, app: &mut App) {
        // A slice works alone, so it makes sure of the state it is built in. A second
        // `init_state` would log a warning.
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        if !app.world().contains_resource::<State<GameState>>() {
            app.init_state::<GameState>();
        }
        app.add_sub_state::<RacePause>()
            .init_resource::<RaceSettings>()
            .init_resource::<crate::keys::KeyBindings>()
            .init_resource::<RaceClock>()
            // Which the race fills in, whether or not the trucks' plugin is there yet.
            .init_resource::<crate::truck::RepeatingWorld>()
            .init_resource::<RaceStart>()
            .init_resource::<pause::PauseMenu>()
            .init_resource::<map::MapShown>()
            .init_resource::<along::GatesAlong>()
            .add_message::<BackToCheckpoint>()
            .add_message::<RaceCancelled>()
            .add_systems(
                OnEnter(RacePause::Paused),
                (pause::stop_the_clock, pause::spawn_pause_dialog),
            )
            .add_systems(OnExit(RacePause::Paused), pause::start_the_clock)
            .add_systems(
                Update,
                (
                    pause::open_pause_dialog.run_if(in_state(RacePause::Running)),
                    (
                        pause::use_pause_dialog,
                        pause::give_up_on_capture,
                        pause::show_pause_dialog,
                    )
                        .chain()
                        .run_if(in_state(RacePause::Paused)),
                ),
            )
            .add_systems(
                OnEnter(GameState::Racing),
                (
                    (systems::reset_clock, start::begin_countdown).chain(),
                    systems::find_gates_along.after(TrackSystems::Prepare),
                    systems::tell_trucks_of_the_world.after(TrackSystems::Prepare),
                    markers::spawn_gate_markers.after(TrackSystems::Prepare),
                    hud::spawn_race_hud,
                    compass::spawn_compass,
                    // Painted from the track the race is run on.
                    map::spawn_map.after(TrackSystems::Prepare),
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
                    )
                        .before(TruckSystems::Place),
                    systems::race_again.run_if(pause::running),
                    markers::highlight_next_gate,
                    hud::update_race_hud,
                    start::show_count,
                    results::show_results,
                    // After the drawn truck has been placed for the frame.
                    compass::update_compass.after(TruckSystems::PlaceVisuals),
                    map::toggle_map
                        .run_if(pause::running)
                        .run_if(just_pressed(Control::Map)),
                    // A new racer gets its dot in the frame it joins.
                    (
                        map::add_dots.after(systems::enlist_trucks),
                        (map::place_dots, map::number_dots),
                    )
                        .chain()
                        .after(TruckSystems::PlaceVisuals),
                )
                    // Outside a race there are no gates, and before the first one no
                    // banner materials either.
                    .run_if(in_state(GameState::Racing)),
            )
            // After the physics step, so that a truck's position is this tick's.
            .add_systems(
                FixedPostUpdate,
                (systems::tick_clock, systems::track_progress)
                    .chain()
                    .after(PhysicsSystems::Writeback)
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
    /// Whether a front end takes the player back when the race is cancelled from the pause
    /// dialog: it reads `RaceCancelled`, and sets this. Without one, cancelling quits the
    /// game.
    pub front_end: bool,
}

impl Default for RaceSettings {
    fn default() -> Self {
        Self {
            laps: 3,
            front_end: false,
        }
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
    /// How far the truck has still to drive to its next gate, along the course, in metres,
    /// as of the last physics tick. Between two trucks that have driven through as many
    /// gates, the one with less to go is ahead.
    to_next_gate: f32,
}

/// For other slices to order their systems against this one.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum RaceSystems {
    /// Handles `BackToCheckpoint` messages, in `Update`. Write them from a system that
    /// runs before this set and the truck moves in the same frame.
    BackToCheckpoint,
}
