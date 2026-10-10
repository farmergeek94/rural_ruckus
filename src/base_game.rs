//! The base game's archives on disk, as a resource: `pod::BaseGame`, which `main.rs`
//! opens (`--base-game=FOLDER`, else the `Shared` and language folders the player chose,
//! else `base/`) and `front_end` hands to the loaders.
//!
//! Not a slice: with `main.rs` and `front_end.rs`, the only code that names the `pod`
//! crate. The `sound` slice asks it for the engine's loops, so that it needs no loader.

use std::ops::Deref;
use std::path::PathBuf;

use bevy::prelude::*;
use content::sound::EngineLoops;

pub use pod::archives_in;

/// Where the base archives are looked for when the command line names no folder: beside
/// `tracks/` and `trucks/`, and git ignores it as it does them. Its subfolders are searched
/// too, so that it can hold links to the `Shared` and `English` folders of an MTM2 CD.
pub const DEFAULT_FOLDER: &str = "base";

/// The base game's archives. Cheap to clone: the clones share what has been read.
#[derive(Resource, Clone, Default)]
pub struct BaseGame(pod::BaseGame);

impl BaseGame {
    /// No base archives: tracks and trucks have only the files they carry.
    pub fn none() -> Self {
        Self(pod::BaseGame::none())
    }

    /// The archives in each folder and in its subfolders: see `pod::BaseGame::open`.
    pub fn open(folders: &[PathBuf]) -> Self {
        Self(pod::BaseGame::open(folders))
    }

    /// The engine's three loops, from the base game's sounds, or why they are missing.
    pub fn engine_loops(&self) -> Result<EngineLoops, String> {
        pod::sound::engine_loops(&self.0)
    }
}

impl Deref for BaseGame {
    type Target = pod::BaseGame;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
