//! Not a slice: which colliders touch which. Every slice may use it, as with `game_state`.
//!
//! Avian lets two colliders touch when each is a member of a layer the other's filter
//! names. A collider with no `CollisionLayers` of its own is given the default: a member of
//! `LayerMask::DEFAULT` alone, touching everything. So none of the layers here is the
//! default one, or everything would be taken for the ground.
//!
//! The two named here are not a filter between them any more -- a wheel touches the
//! ground, and which of those touches the solver keeps is `truck/contacts.rs`' to say,
//! not a layer's -- but they are still how the two slices name each other: `truck/
//! contacts.rs` asks `GROUND` which contacts are the ground's. The `track` slice puts the
//! ground in `GROUND` and the `truck` slice puts wheels in `WHEELS`, and neither slice may
//! depend on the other, so the names live here.

use avian3d::prelude::{CollisionLayers, LayerMask};

/// The terrain.
pub const GROUND: LayerMask = LayerMask(1 << 1);
/// A truck's wheels, as solid things for walls, scenery and other trucks to touch.
pub const WHEELS: LayerMask = LayerMask(1 << 2);
/// The solid core inside each wheel, which touches the ground and nothing else.
pub const WHEEL_CORES: LayerMask = LayerMask(1 << 3);
/// The ground boxes, which are in `GROUND` as well. Their sides are upright, as no face of
/// the terrain is, and a tire meets one as a wall (see `truck/contacts.rs`).
pub const GROUND_BOXES: LayerMask = LayerMask(1 << 4);
/// A truck's body, so that the wheel cores of other trucks can touch it.
pub const TRUCK_BODIES: LayerMask = LayerMask(1 << 5);

/// For the terrain: touches everything.
pub fn ground() -> CollisionLayers {
    CollisionLayers::new(GROUND, LayerMask::ALL)
}

/// For the ground boxes: the ground, and touches everything.
pub fn ground_boxes() -> CollisionLayers {
    CollisionLayers::new(LayerMask(GROUND.0 | GROUND_BOXES.0), LayerMask::ALL)
}

/// For a wheel: touches everything, the ground included. The ground under the tread is
/// the suspension's, and `truck/contacts.rs` drops those contacts so that the two do not
/// fight; what it leaves is the ground met anywhere else on the tire -- the top of a wheel
/// on a truck tipped over, a sidewall laid on a bank -- which nothing else would hold.
pub fn wheel() -> CollisionLayers {
    CollisionLayers::new(WHEELS, LayerMask::ALL)
}

/// For a wheel's core: the ground, and the bodies of trucks. Walls, scenery and other
/// trucks' tires are the tire's. The tire's contacts with another truck are shaped for it
/// to ride up and over (see `truck/contacts.rs`), and without the core nothing held it out
/// of the other truck's body. The physics never lets two colliders of one body touch, so
/// a core never meets its own truck's body.
pub fn wheel_core() -> CollisionLayers {
    CollisionLayers::new(WHEEL_CORES, LayerMask(GROUND.0 | TRUCK_BODIES.0))
}

/// For a truck's body: touches everything.
pub fn truck_body() -> CollisionLayers {
    CollisionLayers::new(TRUCK_BODIES, LayerMask::ALL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_layer_is_the_one_every_collider_is_in_by_default() {
        for layer in [GROUND, WHEELS, WHEEL_CORES, GROUND_BOXES, TRUCK_BODIES] {
            assert_eq!(layer & LayerMask::DEFAULT, LayerMask::NONE);
        }
    }
}
