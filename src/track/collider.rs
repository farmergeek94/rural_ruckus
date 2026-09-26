//! The physics side of the terrain: a Rapier heightfield.

use bevy::prelude::*;
use bevy_rapier3d::geometry::shape_views::HeightFieldCellStatus;
use bevy_rapier3d::prelude::*;

use super::HeightGrid;

pub fn build_collider(grid: &HeightGrid) -> Collider {
    let resolution = grid.resolution();

    // Rapier expects column-major heights, with rows along Z and columns along X.
    let mut heights = Vec::with_capacity(resolution * resolution);
    for col in 0..resolution {
        for row in 0..resolution {
            heights.push(grid.vertex(col, row));
        }
    }

    // The heights are already in metres, so the vertical scale is 1.
    let mut collider = Collider::heightfield(
        heights,
        resolution,
        resolution,
        Vec3::new(grid.size(), 1.0, grid.size()),
    );

    // Rapier splits a cell from its +X corner to its +Z corner unless told otherwise.
    let mut heightfield = collider
        .as_heightfield_mut()
        .expect("just built as a heightfield");
    for row in 0..resolution - 1 {
        for col in 0..resolution - 1 {
            if grid.splits_from_origin(col, row) {
                heightfield.set_cell_status(row, col, HeightFieldCellStatus::ZIGZAG_SUBDIVISION);
            }
        }
    }
    collider
}
