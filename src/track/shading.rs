//! Smooth shading: the ground lit as if its creases were rounded off, while its shape,
//! and what trucks drive on, stay the flat triangles of the height grid.
//!
//! A normal map of the whole track is worked out once, as the race is built: which way a
//! smooth surface through the grid's heights faces (`HeightGrid::smooth_normal`), several
//! times across every cell. The ground's shader (`tiles.wgsl`) reads it at each pixel's
//! place on the ground and lights the pixel by it, in place of the normal of the triangle
//! the pixel is on. Nothing is done on the CPU while racing. Hill outlines and the creases
//! against the sky keep their straight edges: only the light and shade are rounded off.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::HeightGrid;
use crate::mipmaps::data_mip_chain;

/// How many normals the map has along each ground cell, at least. Between them the
/// graphics card blends. Four follows the curve of a 10 m cell closely enough that no
/// facet shows in the light. Fewer and the old creases start to show again as bands.
const NORMALS_PER_CELL: usize = 4;

/// The largest side of the map, in texels, to keep a track with a very fine grid from
/// asking for a huge texture. 2048 is 16 MB of normals.
const LARGEST_MAP: usize = 2048;

/// The normal map of `grid`'s smooth surface, covering the whole track: see `texels`.
pub(super) fn normal_map(grid: &HeightGrid) -> Image {
    let side = map_side(grid);
    let chain = data_mip_chain(&texels(grid, side), side);
    let levels = chain.len();
    // Ground that repeats is lit beyond each edge as it is inside the other. The texels
    // cover the map evenly, so the map repeats exactly with it.
    let beyond_edge = if grid.repeats() {
        ImageAddressMode::Repeat
    } else {
        ImageAddressMode::ClampToEdge
    };
    // Built field by field, because `Image::new` takes only the full-size level.
    let mut image = Image {
        data: Some(chain.concat()),
        asset_usage: RenderAssetUsages::default(),
        sampler: ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: beyond_edge,
            address_mode_v: beyond_edge,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            ..default()
        }),
        ..default()
    };
    image.texture_descriptor.size = Extent3d {
        width: side as u32,
        height: side as u32,
        depth_or_array_layers: 1,
    };
    image.texture_descriptor.dimension = TextureDimension::D2;
    image.texture_descriptor.mip_level_count = levels as u32;
    // Directions, not colours: read as the numbers they are.
    image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    image
}

/// Texels along each side of the map: a power of two, so that it has a full chain of
/// mipmaps, with at least `NORMALS_PER_CELL` across every cell.
fn map_side(grid: &HeightGrid) -> usize {
    ((grid.resolution() - 1) * NORMALS_PER_CELL)
        .next_power_of_two()
        .min(LARGEST_MAP)
}

/// `side` x `side` normals, row by row (rows along Z, columns along X), each as red,
/// green and blue bytes for X, Y and Z from -1 at 0 to 1 at 255, and an unused alpha.
/// Texel (`i`, `j`) is the normal at the middle of its square of the track, so that the
/// shader finds a place's normal at `xz / size + 0.5`, as a texture's coordinates run.
fn texels(grid: &HeightGrid, side: usize) -> Vec<u8> {
    let place = |index: usize| ((index as f32 + 0.5) / side as f32 - 0.5) * grid.size();
    let byte = |value: f32| ((value * 0.5 + 0.5) * 255.0).round() as u8;
    let mut texels = Vec::with_capacity(side * side * 4);
    for row in 0..side {
        for col in 0..side {
            let normal = grid.smooth_normal(place(col), place(row));
            texels.extend([byte(normal.x), byte(normal.y), byte(normal.z), 255]);
        }
    }
    texels
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normal_at(texels: &[u8], side: usize, col: usize, row: usize) -> Vec3 {
        let at = (row * side + col) * 4;
        let value = |byte: u8| byte as f32 / 255.0 * 2.0 - 1.0;
        Vec3::new(
            value(texels[at]),
            value(texels[at + 1]),
            value(texels[at + 2]),
        )
    }

    #[test]
    fn a_monster_truck_madness_2_track_gets_four_normals_a_cell() {
        let grid = HeightGrid::from_fn(257, 2497.0, |_, _| 0.0);
        assert_eq!(map_side(&grid), 1024);
        // A finer grid is kept to a sensible size.
        let fine = HeightGrid::from_fn(1025, 2497.0, |_, _| 0.0);
        assert_eq!(map_side(&fine), LARGEST_MAP);
    }

    #[test]
    fn the_map_holds_every_level_of_its_mipmaps() {
        // 4 cells, so 16 texels a side, halving down to 1.
        let image = normal_map(&HeightGrid::from_fn(5, 40.0, |x, _| 0.1 * x));
        assert_eq!((image.width(), image.height()), (16, 16));
        assert_eq!(image.texture_descriptor.mip_level_count, 5);
        let texels = 16 * 16 + 8 * 8 + 4 * 4 + 2 * 2 + 1;
        assert_eq!(image.data.as_ref().unwrap().len(), texels * 4);
    }

    #[test]
    fn level_ground_faces_straight_up() {
        let grid = HeightGrid::from_fn(5, 40.0, |_, _| 3.0);
        let texels = texels(&grid, 16);
        assert!(texels.chunks(4).all(|texel| texel == [128, 255, 128, 255]));
    }

    #[test]
    fn each_texel_holds_the_normal_at_its_middle() {
        let grid = HeightGrid::from_fn(9, 80.0, |x, z| 3.0 * (0.1 * x).sin() + 0.4 * z);
        let side = 32;
        let texels = texels(&grid, side);
        // The track runs from -40 to 40 m, so each texel is 2.5 m across.
        for (col, row) in [(5, 9), (20, 3), (16, 16)] {
            let (x, z) = (
                col as f32 * 2.5 + 1.25 - 40.0,
                row as f32 * 2.5 + 1.25 - 40.0,
            );
            let expected = grid.smooth_normal(x, z);
            // Kept to within a byte's rounding.
            assert!(
                normal_at(&texels, side, col, row).distance(expected) < 0.01,
                "texel ({col}, {row})"
            );
        }
        // Facing downhill, away from +Z, on the slope along Z.
        assert!(normal_at(&texels, side, 16, 16).z < -0.3);
    }
}
