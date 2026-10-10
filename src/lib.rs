//! The game lives in this library so that both the binary (`main.rs`) and the
//! integration tests in `tests/` can use it.
//!
//! Each module is a vertical slice: one feature, with its own plugin, components,
//! systems and UI. See AGENTS.md for the rules. `base_game`, `game_state`,
//! `collision_groups`, `physics` and `keys` are the exceptions: the base game's archives
//! that tracks and trucks borrow from, the state every slice is built in, the names of
//! what may touch what in the physics, how the physics engine is set up, and which key
//! does what.
//!
//! The game is built against the `content` crate, its content model: a track, a truck and
//! the engine's sounds as plain data, which the `track`, `truck` and `sound` slices
//! re-export. The `pod` crate fills that model in from Monster Truck Madness 2 archives,
//! and only `main.rs`, `front_end.rs` and `base_game.rs` name it. `ui` and `store` are the
//! two modules here that know nothing of the game at all: the front end's screens, and
//! what is remembered between runs. `front_end` does the translating, and nothing goes the
//! other way.

pub mod backdrop;
pub mod base_game;
pub mod camera;
pub mod collision_groups;
pub mod controls_help;
pub mod diagnostics;
pub mod display;
pub mod environment;
pub mod footing;
pub mod frame_pacing;
pub mod front_end;
pub mod game_state;
pub mod graphics_debug;
pub mod keys;
pub mod lighting;
pub mod opponents;
pub mod particles;
pub mod physics;
pub mod physics_debug;
pub mod race;
pub mod scenery;
pub mod sky;
pub mod sound;
pub mod store;
pub mod track;
pub mod truck;
pub mod ui;
pub mod water;
pub mod weather;

/// Smaller copies of a texture, from the content model, for the slices that have textures.
pub use content::mipmaps;
