//! Ground boxes as the game uses them: a mesh to draw and a collider to drive on.
//!
//! Both are built from `TrackData::ground_boxes`. The mesh draws each box's faces, less
//! the sides that a neighbouring box hides. The collider joins boxes that stand between
//! the same two heights, side by side, into one cuboid, so that a deck made of many cells
//! has no seams across it for a tire to catch on.

use std::collections::HashMap;

use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::{GroundBox, GroundCell};

/// How close two edges must be to count as the same, in metres.
const SAME: f32 = 0.001;

/// How far each mesh of ground boxes reaches along X and Z, in metres. Each is a mesh of
/// its own so that those off screen aren't drawn, as with the ground's chunks.
const CHUNK_METRES: f32 = 160.0;

/// One collider for all of a track's ground boxes. `None` for a track with none.
pub(super) fn build_collider(boxes: &[GroundBox]) -> Option<Collider> {
    let blocks: Vec<(Vec3, Quat, Collider)> = merged(boxes)
        .into_iter()
        .map(|block| {
            let middle = (block.min + block.max) / 2.0;
            let size = block.max - block.min;
            let height = block.top - block.bottom;
            (
                Vec3::new(middle.x, block.bottom + height / 2.0, middle.y),
                Quat::IDENTITY,
                // Avian's cuboid takes whole lengths, not half ones.
                Collider::cuboid(size.x, height, size.y),
            )
        })
        .collect();
    (!blocks.is_empty()).then(|| Collider::compound(blocks))
}

/// A box without textures, for the collider.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Block {
    min: Vec2,
    max: Vec2,
    bottom: f32,
    top: f32,
}

/// The boxes joined into as few blocks as a simple pass finds: first into runs along X,
/// then runs of the same length into rectangles along Z.
fn merged(boxes: &[GroundBox]) -> Vec<Block> {
    let blocks: Vec<Block> = boxes
        .iter()
        .map(|found| Block {
            min: found.min,
            max: found.max,
            bottom: found.bottom,
            top: found.top,
        })
        .collect();
    let same = |a: f32, b: f32| (a - b).abs() < SAME;
    let same_heights = |a: &Block, b: &Block| same(a.bottom, b.bottom) && same(a.top, b.top);

    let along_x = join(
        blocks,
        |a, b| (a.min.y, a.min.x).partial_cmp(&(b.min.y, b.min.x)),
        |run, next| {
            same_heights(run, next)
                && same(run.min.y, next.min.y)
                && same(run.max.y, next.max.y)
                && same(run.max.x, next.min.x)
        },
    );
    join(
        along_x,
        |a, b| (a.min.x, a.max.x, a.min.y).partial_cmp(&(b.min.x, b.max.x, b.min.y)),
        |run, next| {
            same_heights(run, next)
                && same(run.min.x, next.min.x)
                && same(run.max.x, next.max.x)
                && same(run.max.y, next.min.y)
        },
    )
}

/// Sorts the blocks and joins each to the one before it where `joins` says so.
fn join(
    mut blocks: Vec<Block>,
    order: impl Fn(&Block, &Block) -> Option<std::cmp::Ordering>,
    joins: impl Fn(&Block, &Block) -> bool,
) -> Vec<Block> {
    blocks.sort_by(|a, b| order(a, b).unwrap_or(std::cmp::Ordering::Equal));
    let mut joined: Vec<Block> = Vec::with_capacity(blocks.len());
    for block in blocks {
        match joined.last_mut() {
            Some(run) if joins(run, &block) => run.max = run.max.max(block.max),
            _ => joined.push(block),
        }
    }
    joined
}

/// Meshes of the ground boxes, a few chunks of them. With `textured`, the boxes that have
/// textures are drawn with them, for `TileMaterial`; the first list holds those, and the
/// second the rest, drawn plain.
pub(super) fn build_meshes(boxes: &[GroundBox], textured: bool) -> (Vec<Mesh>, Vec<Mesh>) {
    let key = |at: Vec2| {
        let snap = |metres: f32| (metres / SAME).round() as i64;
        (snap(at.x), snap(at.y))
    };
    let by_corner: HashMap<(i64, i64), &GroundBox> =
        boxes.iter().map(|found| (key(found.min), found)).collect();
    // Whether the box whose lowest corner is `at` covers the whole height of `side`.
    let hidden = |side: &GroundBox, at: Vec2| {
        by_corner
            .get(&key(at))
            .is_some_and(|other| other.bottom <= side.bottom + SAME && other.top >= side.top - SAME)
    };

    let mut chunks: HashMap<(i64, i64, bool), MeshBuilder> = HashMap::new();
    for found in boxes {
        let middle = (found.min + found.max) / 2.0;
        let chunk = (
            (middle.x / CHUNK_METRES).floor() as i64,
            (middle.y / CHUNK_METRES).floor() as i64,
        );
        let faces = found.faces.as_ref().filter(|_| textured);
        let mesh = chunks
            .entry((chunk.0, chunk.1, faces.is_some()))
            .or_default();

        let size = found.max - found.min;
        let (low, high) = (found.bottom, found.top);
        let (x0, x1, z0, z1) = (found.min.x, found.max.x, found.min.y, found.max.y);
        let corner = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
        let tile = |pick: fn(&super::BoxFaces) -> &GroundCell| faces.map(pick);

        // Each side as seen from outside: bottom left, bottom right, top left, top right.
        if !hidden(found, Vec2::new(x1, z0)) {
            let at = [
                corner(x1, low, z1),
                corner(x1, low, z0),
                corner(x1, high, z1),
                corner(x1, high, z0),
            ];
            mesh.push_face(at, Vec3::X, tile(|f| &f.plus_x));
        }
        if !hidden(found, Vec2::new(x0 - size.x, z0)) {
            let at = [
                corner(x0, low, z0),
                corner(x0, low, z1),
                corner(x0, high, z0),
                corner(x0, high, z1),
            ];
            mesh.push_face(at, Vec3::NEG_X, tile(|f| &f.minus_x));
        }
        if !hidden(found, Vec2::new(x0, z1)) {
            let at = [
                corner(x0, low, z1),
                corner(x1, low, z1),
                corner(x0, high, z1),
                corner(x1, high, z1),
            ];
            mesh.push_face(at, Vec3::Z, tile(|f| &f.plus_z));
        }
        if !hidden(found, Vec2::new(x0, z0 - size.y)) {
            let at = [
                corner(x1, low, z0),
                corner(x0, low, z0),
                corner(x1, high, z0),
                corner(x0, high, z0),
            ];
            mesh.push_face(at, Vec3::NEG_Z, tile(|f| &f.minus_z));
        }
        // The top and bottom in the ground's order: lowest, +X, +Z, far.
        let flat = |y: f32| {
            [
                corner(x0, y, z0),
                corner(x1, y, z0),
                corner(x0, y, z1),
                corner(x1, y, z1),
            ]
        };
        mesh.push_face(flat(high), Vec3::Y, tile(|f| &f.top));
        mesh.push_face(flat(low), Vec3::NEG_Y, tile(|f| &f.bottom));
    }

    let mut chunks: Vec<_> = chunks.into_iter().collect();
    // In a fixed order, so that the same track makes the same meshes.
    chunks.sort_by_key(|(key, _)| *key);
    let (mut with_textures, mut plain) = (Vec::new(), Vec::new());
    for ((_, _, has_textures), mesh) in chunks {
        if has_textures {
            with_textures.push(mesh.finish(true));
        } else {
            plain.push(mesh.finish(false));
        }
    }
    (with_textures, plain)
}

#[derive(Default)]
struct MeshBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    tiles: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    /// A rectangle, its corners in the order of `GroundCell::corners`, facing `normal`.
    fn push_face(&mut self, corners: [Vec3; 4], normal: Vec3, cell: Option<&GroundCell>) {
        let first = self.positions.len() as u32;
        for (index, corner) in corners.into_iter().enumerate() {
            self.positions.push(corner.to_array());
            self.normals.push(normal.to_array());
            if let Some(cell) = cell {
                self.uvs.push(cell.corners[index]);
                self.tiles.push([cell.tile as f32, 0.0]);
            }
        }
        let [a, b, c, d] = [first, first + 1, first + 2, first + 3];
        // Counter-clockwise seen from the front, whichever way the face looks.
        let (e1, e2) = (corners[1] - corners[0], corners[2] - corners[0]);
        if e1.cross(e2).dot(normal) > 0.0 {
            self.indices.extend_from_slice(&[a, b, c, c, b, d]);
        } else {
            self.indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    fn finish(self, textured: bool) -> Mesh {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_indices(Indices::U32(self.indices));
        if textured {
            mesh.with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.tiles)
        } else {
            mesh
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box on the square of side 10 whose lowest corner is (`x`, `z`).
    fn ground_box(x: f32, z: f32, bottom: f32, top: f32) -> GroundBox {
        GroundBox {
            min: Vec2::new(x, z),
            max: Vec2::new(x + 10.0, z + 10.0),
            bottom,
            top,
            faces: None,
        }
    }

    #[test]
    fn boxes_between_the_same_heights_join_into_one_block() {
        // A deck three wide and two long, and a taller box beside it.
        let mut boxes = Vec::new();
        for z in [0.0, 10.0] {
            for x in [0.0, 10.0, 20.0] {
                boxes.push(ground_box(x, z, 5.0, 6.0));
            }
        }
        boxes.push(ground_box(30.0, 0.0, 0.0, 6.0));
        let blocks = merged(&boxes);
        assert_eq!(blocks.len(), 2);
        assert!(blocks.contains(&Block {
            min: Vec2::ZERO,
            max: Vec2::new(30.0, 20.0),
            bottom: 5.0,
            top: 6.0
        }));

        // Every box is inside a block of its own heights.
        for found in &boxes {
            let middle = (found.min + found.max) / 2.0;
            assert!(blocks.iter().any(|block| {
                block.min.cmple(middle).all()
                    && block.max.cmpge(middle).all()
                    && block.bottom == found.bottom
                    && block.top == found.top
            }));
        }
    }

    #[test]
    fn sides_that_a_neighbour_hides_are_not_drawn() {
        // Two boxes side by side along X: each hides one side of the other.
        let (_, plain) = build_meshes(
            &[
                ground_box(0.0, 0.0, 0.0, 5.0),
                ground_box(10.0, 0.0, 0.0, 5.0),
            ],
            true,
        );
        let faces: usize = plain.iter().map(|mesh| mesh.count_vertices() / 4).sum();
        assert_eq!(faces, 2 * 6 - 2);

        // A shorter neighbour hides nothing of a taller box, which is still seen above it.
        let (_, plain) = build_meshes(
            &[
                ground_box(0.0, 0.0, 0.0, 5.0),
                ground_box(10.0, 0.0, 0.0, 3.0),
            ],
            true,
        );
        let faces: usize = plain.iter().map(|mesh| mesh.count_vertices() / 4).sum();
        assert_eq!(faces, 2 * 6 - 1);
    }

    #[test]
    fn faces_look_outwards() {
        let (_, plain) = build_meshes(&[ground_box(0.0, 0.0, 0.0, 5.0)], false);
        let mesh = &plain[0];
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("no indices");
        };
        let middle = Vec3::new(5.0, 2.5, 5.0);
        for triangle in indices.as_chunks::<3>().0 {
            let [a, b, c] = triangle.map(|index| Vec3::from(positions[index as usize]));
            let normal = (b - a).cross(c - a);
            assert!(normal.dot((a + b + c) / 3.0 - middle) > 0.0);
        }
    }
}
