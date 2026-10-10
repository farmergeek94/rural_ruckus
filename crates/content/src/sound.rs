//! The engine's sounds as plain data: three loops, which the game's `sound` slice plays.

use std::sync::Arc;

/// The engine's three loops: idling, pulling away (M1) and running on (M2).
#[derive(Clone, Debug)]
pub struct EngineLoops {
    pub idle: Loop,
    pub m1: Loop,
    pub m2: Loop,
}

/// A sound that repeats without a break.
#[derive(Clone, Debug)]
pub struct Loop {
    /// Samples a second.
    pub sample_rate: u32,
    /// One channel, from -1 to 1.
    pub samples: Arc<[f32]>,
}
