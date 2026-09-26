//! Not a slice: which colliders touch which. Every slice may use it, as with `game_state`.
//!
//! Rapier lets two colliders touch when each is a member of a group the other's filter
//! names. A collider with no `CollisionGroups` is in every group and touches everything.
//! The two named here are not a filter between them any more -- a wheel touches the
//! ground, and which of those touches the solver keeps is `truck/contacts.rs`' to say,
//! not a group's -- but they are still how the two slices name each other: `truck/
//! contacts.rs` asks `GROUND` which contacts are the ground's. The `track` slice puts the
//! ground in `GROUND` and the `truck` slice puts wheels in `WHEELS`, and neither slice may
//! depend on the other, so the names live here.

use bevy_rapier3d::prelude::{CollisionGroups, Group};

/// The terrain.
pub const GROUND: Group = Group::GROUP_1;
/// A truck's wheels, as solid things for walls, scenery and other trucks to touch.
pub const WHEELS: Group = Group::GROUP_2;
/// The solid core inside each wheel, which touches the ground and nothing else.
pub const WHEEL_CORES: Group = Group::GROUP_3;

/// For the terrain: touches everything.
pub fn ground() -> CollisionGroups {
    CollisionGroups::new(GROUND, Group::ALL)
}

/// For a wheel: touches everything, the ground included. The ground under the tread is
/// the suspension's, and `truck/contacts.rs` drops those contacts so that the two do not
/// fight; what it leaves is the ground met anywhere else on the tire -- the top of a wheel
/// on a truck tipped over, a sidewall laid on a bank -- which nothing else would hold.
pub fn wheel() -> CollisionGroups {
    CollisionGroups::new(WHEELS, Group::ALL)
}

/// For a wheel's core: the ground alone. Walls, scenery and other trucks are the tire's.
pub fn wheel_core() -> CollisionGroups {
    CollisionGroups::new(WHEEL_CORES, GROUND)
}
