//! A race round a circuit: Monster Truck Madness 2's own event, laps around a track whose
//! checkpoints are driven through in order, as rules and a driver over the content model,
//! with nothing of the game's engine in it.
//!
//! The game keeps the entities and runs the systems; this crate decides. `gate` says when
//! a truck has driven through a checkpoint, `progress` is the rules of the race for one
//! truck (gates, laps, times, and who is ahead of whom), and `along` is how far a truck has
//! still to drive to its next gate along the course, which the race order compares. The
//! rest is the mind of a computer truck: `driver` works its controls out each step from the
//! course ahead, its place in the race and the checkpoint it has to drive through next;
//! `passing` is how it goes round what is in front of it, and `obstacles` the fixed scenery
//! on the road it treats as a truck stopped there, found once a race.
//!
//! Units are the game's: metres, seconds, metres a second, and race times in physics ticks.
//! Y is up. A position on the ground plane is world X and Z.

mod along;
mod driver;
mod gate;
mod obstacles;
mod passing;
mod progress;

pub use along::GatesAlong;
pub use driver::{
    Controls, Decision, Driver, FULL_LOCK_ANGLE, OnCourse, PROGRESS, Places, Race, Route, Skill,
    along_the_course, course_for, lead_driver, throttle_for,
};
pub use gate::{beside, crossed};
pub use progress::{Milestone, RaceProgress, position, standings};

/// What the tests of more than one module drive on.
#[cfg(test)]
pub(crate) mod fixtures {
    use bevy::math::Vec2;
    use content::track::Course;
    use content::truck::TruckConfig;

    /// A square of 300 m sides. The first side runs along +X from the origin, and the
    /// first corner turns towards +Z, which is to the right.
    pub(crate) fn course() -> Course {
        Course::new(
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(300.0, 0.0),
                Vec2::new(300.0, 300.0),
                Vec2::new(0.0, 300.0),
            ],
            14.0,
        )
    }

    /// What the built-in truck reaches from its middle, wheels included: 1.9 m across it
    /// and 2.75 m along it.
    pub(crate) fn built_in() -> Vec2 {
        crate::driver::reach(&TruckConfig::default())
    }
}
