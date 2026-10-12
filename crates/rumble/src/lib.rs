//! A Summit Rumble: Monster Truck Madness 2's "King of the Hill" event, as rules and
//! tactics over the content model, with nothing of the game's engine in it.
//!
//! The game keeps the entities and runs the systems; this crate decides. `score` is the
//! scoring, as MTM2's own help file states it (`docs/formats/situation.md`, "Summit
//! Rumbles"). The rest is the game's own, since MTM2 drove no computer trucks in a rumble:
//! `nav` finds the way up the summit over the ground and the ground boxes, round every
//! wall and rim, as a field of next steps worked out once a race; `pursuit` follows such a
//! path at a speed its bends allow; `tactics` picks whom to push off, how, and when to get
//! out of the way.
//!
//! Units are the game's: metres, seconds, metres a second. Y is up. A position on the
//! ground plane is world X and Z.

mod nav;
mod pursuit;
mod score;
mod tactics;

pub use nav::{NavField, NavParams, Waypoint};
pub use pursuit::{Limits, Pursuit, follow};
pub use score::{
    BOOTED, Event, OFF_SUMMIT_PER_SECOND, ON_ZONE_PER_SECOND, Score, UNDER_SUMMIT, on_summit,
    position, standings,
};
pub use tactics::{
    Dodge, Me, Mode, Rival, ahead_in_lane, attack_aim, charge_speed, choose_target, decide,
    edge_speed, hold_aim, pushing, threat,
};
