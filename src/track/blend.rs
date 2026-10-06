//! Blended ground: where two ground cells meet, their tiles fade into each other and blur
//! a little, instead of meeting in a hard line. A departure from Monster Truck Madness 2,
//! which draws every cell's tile edge to edge.
//!
//! The ground's shader (`tiles.wgsl`) does the work. Near an edge of its cell, a pixel
//! also reads the tile of the cell across that edge, mirrored back across it so that the
//! two meet in the same colours from both sides, and fades between the two: half and half
//! at the edge. Near the edge it reads both from smaller, softer copies of the tiles (their
//! mipmaps), so the line between them is blurred as well as faded. At a corner it mixes
//! all four cells.
//!
//! What the shader needs to read another cell's tile is put in a map with two texels per
//! ground cell (`cell_map`), once as the race is built. Nothing is done on the CPU while
//! racing. Physics, the footing and the track map still see whole cells.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::GroundTextures;

/// How far into a cell, from each edge, its neighbour's tile is faded in, as a share of
/// the cell's side. The two tiles are half and half at the edge, and the neighbour's
/// fades to nothing here. 0.25 of a 10 m MTM2 cell makes a change of ground 5 m wide.
/// More softens the ground further and makes more pixels read more than one tile.
pub(super) const BLEND_WIDTH: f32 = 0.25;

/// Everything the shader needs about the ground's cells, as a texture with two texels per
/// cell, side by side along X, and a row of them for every row of cells. The first texel
/// holds the tile and where the cell's lowest corner falls in it, and an unused 0. The
/// second holds how far through the tile one cell's side along X, and then along Z,
/// moves. Read texel by texel (`textureLoad`), never sampled.
pub(super) fn cell_map(ground: &GroundTextures) -> Image {
    let cells = ground.cells_per_side();
    let mut texels: Vec<f32> = Vec::with_capacity(cells * cells * 8);
    for cell in &ground.cells {
        let [lowest, plus_x, plus_z, _] = cell.corners.map(Vec2::from);
        let (along_x, along_z) = (plus_x - lowest, plus_z - lowest);
        texels.extend([cell.tile as f32, lowest.x, lowest.y, 0.0]);
        texels.extend([along_x.x, along_x.y, along_z.x, along_z.y]);
    }
    let bytes = texels.iter().flat_map(|value| value.to_le_bytes()).collect();
    Image::new(
        Extent3d {
            width: (cells * 2) as u32,
            height: cells as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytes,
        TextureFormat::Rgba32Float,
        RenderAssetUsages::RENDER_WORLD,
    )
}

#[cfg(test)]
mod tests {
    use super::super::GroundCell;
    use super::*;

    #[test]
    fn each_cell_has_its_tile_and_its_mapping_in_two_texels() {
        // Turned a quarter: across X runs down the tile, across Z runs back along it.
        let turned = [[1.0, 0.0], [1.0, 1.0], [0.0, 0.0], [0.0, 1.0]];
        let ground = GroundTextures {
            tile_size: 1,
            tiles: vec![vec![0; 4]; 8],
            cells: vec![
                GroundCell {
                    tile: 7,
                    corners: turned,
                };
                4
            ],
        };
        let image = cell_map(&ground);
        assert_eq!((image.width(), image.height()), (4, 2));
        let data = image.data.as_ref().unwrap();
        let texel = |index: usize| -> [f32; 4] {
            std::array::from_fn(|i| {
                let at = (index * 4 + i) * 4;
                f32::from_le_bytes(data[at..at + 4].try_into().unwrap())
            })
        };
        assert_eq!(texel(0), [7.0, 1.0, 0.0, 0.0]);
        assert_eq!(texel(1), [0.0, 1.0, -1.0, 0.0]);
    }
}
