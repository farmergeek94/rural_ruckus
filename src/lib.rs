//! The game lives in this library so that both the binary (`main.rs`) and the
//! integration tests in `tests/` can use it.
//!
//! Each module is a vertical slice: one feature, with its own plugin, components,
//! systems and UI. See AGENTS.md for the rules. `base_game`, `mipmaps`, `hd_texture`,
//! `particles`, `game_state`, `collision_groups`, `physics` and `keys` are the exceptions:
//! the base game's archives that tracks and trucks borrow from, two helpers shared by the
//! slices that have textures, one shared by the slices that throw things up, the state every
//! slice is built in, the names of what may touch what in the physics, how the physics
//! engine is set up, and which key does what.
//!
//! `pod`, `ui` and `store` are the three modules that know nothing of the game at all: the
//! reader for Monster Truck Madness 2 archives, the front end's screens, and what is
//! remembered between runs. The slices that use them (`track`, `truck`, `front_end`) do the
//! translating, and nothing goes the other way.

pub mod backdrop;
pub mod base_game;
pub mod camera;
pub mod collision_groups;
pub mod controls_help;
pub mod diagnostics;
pub mod dirt;
pub mod display;
pub mod environment;
pub mod footing;
pub mod frame_pacing;
pub mod front_end;
pub mod game_state;
pub mod graphics_debug;
pub mod hd_texture;
pub mod keys;
pub mod lighting;
pub mod mipmaps;
pub mod opponents;
pub mod particles;
pub mod physics;
pub mod physics_debug;
pub mod pod;
pub mod race;
pub mod scenery;
pub mod sky;
pub mod store;
pub mod track;
pub mod truck;
pub mod ui;
pub mod water;
pub mod weather;
