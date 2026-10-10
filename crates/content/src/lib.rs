//! The content model of Monster Truck Rural Ruckus: everything the game knows about a
//! track (`track::TrackData`), a truck (`truck::TruckData`) and the engine's sounds
//! (`sound::EngineLoops`), as plain data.
//!
//! The game is built against this model and nothing else: a loader, such as the `pod`
//! crate for Monster Truck Madness 2 archives, fills it in, and gameplay cannot tell
//! where its content came from. A loader for another source produces the same types.
//! When a source holds something the model cannot, extend the model; never make the game
//! reach into a loader's types.
//!
//! Units and axes are the game's: SI units (metres, seconds, kilograms, newtons), Y up,
//! and a truck drives towards -Z. A loader converts its source's conventions once.
//!
//! Only plain types live here: no entities, asset handles, meshes or images. The vectors
//! are Bevy's (`Vec2`, `Vec3`, `Quat`, `Rect`), re-exported so that a loader needs no
//! other dependency to fill the model in. `truck::TruckConfig` is a `Component` and
//! `truck::TruckSetup` a `Resource`, because the game keeps them on entities as they are.

pub mod mipmaps;
pub mod sound;
pub mod track;
pub mod truck;

pub use bevy::math::{Quat, Rect, Vec2, Vec3};

/// How many animated textures (`track::TextureCycle`) one of the game's materials can
/// show. Past that many, a texture shows its first frame only. The most any track seen
/// has is 3.
pub const MAX_TEXTURE_CYCLES: usize = 31;
