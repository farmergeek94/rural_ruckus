//! Whether a race is on. Not a slice: the one piece of state every slice shares, as
//! `content::mipmaps` is the one shared helper.
//!
//! Everything a race is made of is spawned on entering `Racing`, carrying
//! `DespawnOnExit(GameState::Racing)`, so that leaving a race leaves nothing behind and
//! another can be built. `Racing` is the default: an app that says nothing, as the
//! headless tests don't, races at once, as the game always did. Only `main.rs`, given
//! nothing to race on, starts in `FrontEnd`.
//!
//! Each slice's plugin makes sure the state exists (see `TruckPlugin::build`), so that a
//! slice still works alone.

use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    /// Choosing a truck and a track. Nothing of a race exists.
    FrontEnd,
    #[default]
    Racing,
}
