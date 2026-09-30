//! The physics side of the terrain: a heightfield.

use avian3d::parry::shape::{HeightFieldCellStatus, SharedShape};
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
    Collider::from(shape)
}
