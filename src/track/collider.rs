//! The physics side of the terrain: a heightfield.

use avian3d::parry::shape::{HeightFieldCellStatus, HeightFieldFlags, SharedShape};
use avian3d::parry::utils::Array2;
use avian3d::prelude::*;
use bevy::prelude::Vec3;

use super::HeightGrid;

pub fn build_collider(grid: &HeightGrid) -> Collider {
    let resolution = grid.resolution();

    // Parry takes the heights as a column-major array, with rows along Z and columns
    // along X.
    let mut heights = Vec::with_capacity(resolution * resolution);
    for col in 0..resolution {
        for row in 0..resolution {
            heights.push(grid.vertex(col, row));
        }
    }

    // The heights are already in metres, so the vertical scale is 1. Built through Parry
    // rather than `Collider::heightfield`, which has no way to choose the cells' diagonals.
    let mut shape = SharedShape::heightfield(
        Array2::new(resolution, resolution, heights),
        Vec3::new(grid.size(), 1.0, grid.size()),
    );

    // Parry splits a cell from its +X corner to its +Z corner unless told otherwise.
    let heightfield = shape
        .make_mut()
        .as_heightfield_mut()
        .expect("just built as a heightfield");
    for row in 0..resolution - 1 {
        for col in 0..resolution - 1 {
            if grid.splits_from_origin(col, row) {
                heightfield.set_cell_status(row, col, HeightFieldCellStatus::ZIGZAG_SUBDIVISION);
            }
        }
    }
    // A contact with the edge between two triangles, where the ground is flat, must not
    // push along the edge's own normal: without this flag it did, and a body sliding over
    // the ground met each edge as a wall. Measured on level ground with a loose panel of
    // the mass and size of Crazy '98's chevron signs (227 kg, 0.63 x 4.57 x 0.13 m),
    // bulldozed ahead of the built-in truck at 30 m/s: the panel's base met an edge with
    // a normal 13 degrees from level, was stopped dead by an impulse of 28 kN s, and was
    // kicked 13 m/s into the air; the truck then ran into it and was thrown 3 to 7 m. With
    // the flag the panel slides on, and over 32 such runs the truck never rose more than
    // 0.43 m. Headless, eight trucks for 40 s on Crazy '98: 5.26 ms a step with the flag
    // and 5.27 without. See `docs/avian.md`.
    heightfield.set_flags(HeightFieldFlags::FIX_INTERNAL_EDGES);
    Collider::from(shape)
}
