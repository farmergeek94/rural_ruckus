//! The visual side of the terrain: a mesh, shaded by vertex colours or, for tracks with
//! ground textures, carrying the texture coordinates for them.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::{GroundCell, GroundTextures, HeightGrid, TrackData};

/// Terrain shaded by height and slope and tinted by `TrackData::surface`, for tracks with
/// no ground textures, in chunks like the textured ground's. Neighbouring cells of a chunk
/// share their vertices. Chunks don't, but the vertices they each have along a shared edge
/// are the same ones, normals and colours included, so no seam shows.
pub(super) fn build_shaded_meshes(track: &TrackData) -> Vec<Mesh> {
    let grid = &track.heights;
    let last = grid.resolution() - 1;
    // A finer grid (smoothed terrain) gets bigger chunks rather than more of them.
    let chunk_cells = CHUNK_CELLS.max(last.div_ceil(MOST_SHADED_CHUNKS));

    let mut meshes = Vec::new();
    for first_row in (0..last).step_by(chunk_cells) {
        for first_col in (0..last).step_by(chunk_cells) {
            let rows = chunk_cells.min(last - first_row);
            let cols = chunk_cells.min(last - first_col);
            meshes.push(shaded_chunk(track, first_col, first_row, cols, rows));
        }
    }
    meshes
}

/// The most chunks of shaded terrain along each side of the map.
const MOST_SHADED_CHUNKS: usize = 16;

/// `cols` x `rows` grid cells of shaded terrain, from vertex (`first_col`, `first_row`).
fn shaded_chunk(
    track: &TrackData,
    first_col: usize,
    first_row: usize,
    cols: usize,
    rows: usize,
) -> Mesh {
    let grid = &track.heights;
    let resolution = grid.resolution();

    let vertex_count = (cols + 1) * (rows + 1);
    let mut positions = Vec::with_capacity(vertex_count);
    let mut normals = Vec::with_capacity(vertex_count);
    let mut colors = Vec::with_capacity(vertex_count);

    for row in first_row..=first_row + rows {
        for col in first_col..=first_col + cols {
            let position = vertex_position(grid, col, row);
            let normal = vertex_normal(grid, col, row);
            let on_course = track.surface[row * resolution + col];
            colors.push(ground_color(position.y, normal, on_course).to_f32_array());
            positions.push(position.to_array());
            normals.push(normal.to_array());
        }
    }

    let across = cols as u32 + 1;
    let mut indices = Vec::with_capacity(cols * rows * 6);
    for row in 0..rows {
        for col in 0..cols {
            let v00 = row as u32 * across + col as u32;
            let v10 = v00 + 1; // +X
            let v01 = v00 + across; // +Z
            let v11 = v01 + 1;
            indices.extend_from_slice(&cell_triangles(
                grid,
                first_col + col,
                first_row + row,
                [v00, v10, v01, v11],
            ));
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// Ground cells along each side of one chunk of terrain. Each chunk is a mesh of its own,
/// so that the ones off screen, or too far away to cast a shadow that matters, are never
/// sent to be drawn. One mesh for the whole map is drawn in full every frame, several
/// times over: once for the view and once for each level of the sun's shadows.
const CHUNK_CELLS: usize = 16;

/// Terrain with a texture on every ground cell, for `TileMaterial`, in chunks. Each
/// ground cell has vertices of its own, since neighbours want different texture
/// coordinates at the corners they share. The normals are still those of the shared
/// corners, so the lighting stays smooth.
///
/// The first UV channel is the position within the cell's tile. The second carries the
/// number of the tile, which is its layer in the texture array.
pub(super) fn build_textured_meshes(grid: &HeightGrid, ground: &GroundTextures) -> Vec<Mesh> {
    let cells_per_side = ground.cells_per_side();
    assert_eq!(
        cells_per_side,
        grid.resolution() - 1,
        "one ground cell for every cell of the height grid"
    );

    let mut meshes = Vec::new();
    for chunk_row in (0..cells_per_side).step_by(CHUNK_CELLS) {
        for chunk_col in (0..cells_per_side).step_by(CHUNK_CELLS) {
            let mut chunk = ChunkBuilder::default();
            for row in chunk_row..(chunk_row + CHUNK_CELLS).min(cells_per_side) {
                for col in chunk_col..(chunk_col + CHUNK_CELLS).min(cells_per_side) {
                    chunk.push_cell(grid, &ground.cells[row * cells_per_side + col], col, row);
                }
            }
            meshes.push(chunk.finish());
        }
    }
    meshes
}

#[derive(Default)]
struct ChunkBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    tiles: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl ChunkBuilder {
    /// Adds the ground cell whose lowest corner is vertex (`col`, `row`).
    fn push_cell(&mut self, grid: &HeightGrid, cell: &GroundCell, col: usize, row: usize) {
        let first = self.positions.len() as u32;
        for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let (vertex_col, vertex_row) = (col + dx, row + dz);
            self.positions
                .push(vertex_position(grid, vertex_col, vertex_row).to_array());
            self.normals
                .push(vertex_normal(grid, vertex_col, vertex_row).to_array());
            self.uvs.push(cell.corners[dz * 2 + dx]);
            self.tiles.push([cell.tile as f32, 0.0]);
        }
        let corners = [first, first + 1, first + 2, first + 3];
        self.indices
            .extend_from_slice(&cell_triangles(grid, col, row, corners));
    }

    fn finish(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.tiles)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

fn vertex_position(grid: &HeightGrid, col: usize, row: usize) -> Vec3 {
    Vec3::new(grid.coord(col), grid.vertex(col, row), grid.coord(row))
}

/// From the height difference between neighbouring vertices, which are one cell apart
/// at the edge of the grid and two everywhere else.
fn vertex_normal(grid: &HeightGrid, col: usize, row: usize) -> Vec3 {
    let last = grid.resolution() - 1;
    let (west, east) = (col.saturating_sub(1), (col + 1).min(last));
    let (north, south) = (row.saturating_sub(1), (row + 1).min(last));
    let slope_x =
        (grid.vertex(east, row) - grid.vertex(west, row)) / (grid.coord(east) - grid.coord(west));
    let slope_z = (grid.vertex(col, south) - grid.vertex(col, north))
        / (grid.coord(south) - grid.coord(north));
    Vec3::new(-slope_x, 1.0, -slope_z).normalize()
}

/// The two triangles of a cell, given the vertex numbers of its corners in the order
/// lowest, +X, +Z, far. Counter-clockwise when seen from above, and split along the same
/// diagonal as the heightfield collider and `HeightGrid::height_at`.
fn cell_triangles(grid: &HeightGrid, col: usize, row: usize, corners: [u32; 4]) -> [u32; 6] {
    let [v00, v10, v01, v11] = corners;
    if grid.splits_from_origin(col, row) {
        [v00, v01, v11, v00, v11, v10]
    } else {
        [v00, v01, v10, v10, v01, v11]
    }
}

/// Placeholder shading for tracks without ground textures: grass, dirt on slopes, lighter
/// on hilltops, and packed earth on the course (`on_course` from 0 to 1).
pub(super) fn ground_color(height: f32, normal: Vec3, on_course: f32) -> LinearRgba {
    let grass = LinearRgba::rgb(0.13, 0.30, 0.08);
    let dirt = LinearRgba::rgb(0.30, 0.20, 0.10);
    let hilltop = LinearRgba::rgb(0.45, 0.42, 0.25);
    let course = LinearRgba::rgb(0.42, 0.31, 0.19);

    let steepness = smoothstep(0.02, 0.12, 1.0 - normal.y);
    let altitude = smoothstep(3.0, 8.0, height);
    grass
        .mix(&hilltop, altitude)
        .mix(&dirt, steepness)
        .mix(&course, on_course)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use bevy::mesh::VertexAttributeValues;

    use super::*;

    const CORNERS: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]];

    /// `side` x `side` ground cells, tiled 0, 1, 2...
    fn ground(side: usize) -> GroundTextures {
        GroundTextures {
            tile_size: 2,
            tiles: vec![vec![0; 16]; side * side],
            cells: (0..side * side)
                .map(|tile| GroundCell {
                    tile,
                    corners: CORNERS,
                })
                .collect(),
        }
    }

    fn bumpy(resolution: usize) -> HeightGrid {
        HeightGrid::from_vertex_fn(resolution, 10.0 * (resolution - 1) as f32, |col, row| {
            ((col * 7 + row * 3) % 5) as f32
        })
    }

    fn uvs(mesh: &Mesh, channel: bevy::mesh::MeshVertexAttribute) -> Vec<[f32; 2]> {
        match mesh.attribute(channel) {
            Some(VertexAttributeValues::Float32x2(values)) => values.clone(),
            _ => panic!("no such channel"),
        }
    }

    #[test]
    fn every_ground_cell_has_its_own_vertices() {
        let meshes = build_textured_meshes(&bumpy(3), &ground(2));
        assert_eq!(meshes.len(), 1);
        let mesh = &meshes[0];
        assert_eq!(mesh.count_vertices(), 4 * 4);
        assert_eq!(mesh.indices().unwrap().len(), 4 * 6);

        // Each cell's four vertices name its tile, and carry its corners.
        let named: Vec<f32> = uvs(mesh, Mesh::ATTRIBUTE_UV_1)
            .iter()
            .map(|tile| tile[0])
            .collect();
        assert_eq!(named[..8], [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]);
        assert_eq!(uvs(mesh, Mesh::ATTRIBUTE_UV_0)[4..8], CORNERS);
    }

    #[test]
    fn shaded_terrain_is_cut_into_chunks_that_meet() {
        let track = crate::track::builtin_track();
        let last = track.heights.resolution() - 1;
        let meshes = build_shaded_meshes(&track);

        // Every cell is drawn once, in a chunk small enough to be worth culling.
        let triangles: usize = meshes
            .iter()
            .map(|mesh| mesh.indices().unwrap().len() / 3)
            .sum();
        assert_eq!(triangles, last * last * 2);
        assert_eq!(meshes.len(), last.div_ceil(CHUNK_CELLS).pow(2));

        // The first chunk's last column of vertices is the second chunk's first.
        let positions = |mesh: &Mesh| match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(values)) => values.clone(),
            _ => panic!("no positions"),
        };
        let (west, east) = (positions(&meshes[0]), positions(&meshes[1]));
        let across = CHUNK_CELLS + 1;
        for row in 0..across {
            assert_eq!(west[row * across + CHUNK_CELLS], east[row * across]);
        }
    }

    #[test]
    fn terrain_is_cut_into_chunks() {
        // 40 ground cells a side makes 3 x 3 chunks: 16, 16 and 8 cells.
        let meshes = build_textured_meshes(&bumpy(41), &ground(40));
        assert_eq!(meshes.len(), 9);
        let vertices: Vec<usize> = meshes.iter().map(Mesh::count_vertices).collect();
        assert_eq!(vertices[0], 16 * 16 * 4);
        assert_eq!(vertices[2], 8 * 16 * 4);
        assert_eq!(vertices[8], 8 * 8 * 4);
        assert_eq!(vertices.iter().sum::<usize>(), 40 * 40 * 4);
    }
}
