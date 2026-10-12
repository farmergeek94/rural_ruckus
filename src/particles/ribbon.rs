//! Ribbons laid on the ground, drawn on the graphics card: the tracks that tires leave.
//!
//! A ribbon is a chain of pieces, each a flat four-sided patch between two edges: the
//! last edge of one piece is the first edge of the next, so that the chain is one strip.
//! Whoever lays a piece gives its four corners, each already on the ground, so that the
//! strip follows the ground's slopes, and how far along the ribbon each edge is, so that
//! a picture repeats evenly along it. The pieces of every ribbon are one mesh of four
//! corners each, one for each slot, and a storage buffer with one record a slot, as the
//! particles are (`pool`): the CPU writes a slot once when its piece is laid, and
//! `ribbons.wgsl` places and fades it from then on, so that the CPU's work goes with how
//! many pieces are laid, not with how many lie on the ground. The mesh never changes.
//!
//! The slots are a ring: the piece laid longest ago is the one written over when every
//! slot is full, so that the oldest track goes first. A piece fades out as it ages
//! (`Look::fade_from`, `Look::life`), and in the distance (`Look::seen_to`).
//!
//! Each piece multiplies the ground it lies on by its picture and its colour
//! (`AlphaMode::Multiply`), so that a track darkens sand, grass or snow alike and keeps
//! the ground's own colour and shading, as pressed ground is. The fog hides the ground
//! under a piece as it hides the ground, so the shader fades a piece with the fog's
//! contrast there, which it reads from the view's fog. The pieces are drawn before the
//! particles in the air over them (`FLAT_SORT`), and after the opaque ground, which they
//! are tested against by depth: whoever lays them lifts them a little off it.
//!
//! The shader's clock (`globals.time`) goes back to 0 every hour, when the pieces laid
//! are lost.
//!
//! Knows nothing of the game: the `particles` slice spawns the ribbons and lays into
//! them. Only for an app that can draw.

use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::asset::io::embedded::EmbeddedAssetRegistry;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::ecs::system::SystemParam;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::encase::StorageBuffer;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderSize, ShaderType, SpecializedMeshPipelineError,
};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;

use super::pool::{Upload, Uploads};

/// Where `ribbons.wgsl` is among the embedded assets (see `add`).
const SHADER_PATH: &str = "embedded://monster_truck_rural_ruckus/shaders/ribbons.wgsl";

/// How much further off than the particles the ribbons are sorted, in metres, so that
/// they are drawn before them: under the dust and the spray in the air over them. As the
/// particles that lie flat are (`pool::FLAT_SORT`). Bevy adds a material's depth bias to
/// its depth, but only the whole part, which is none.
const FLAT_SORT: f32 = -0.01;

/// Makes ribbons work in `app`: their material, and what writes them to their meshes. The
/// particles' uploads (`pool::add`) carry the slots to the graphics card, so `pool::add`
/// must have been called.
pub fn add(app: &mut App) {
    app.world_mut()
        .resource_mut::<EmbeddedAssetRegistry>()
        .insert_asset(
            PathBuf::from("src/shaders/ribbons.wgsl"),
            Path::new(SHADER_PATH.trim_start_matches("embedded://")),
            include_bytes!("../shaders/ribbons.wgsl").as_slice(),
        );
    app.add_plugins(MaterialPlugin::<RibbonMaterial>::default())
        // After every slice's `Update`, where pieces are laid, so that they are drawn in
        // the frame they are laid in.
        .add_systems(PostUpdate, draw);
}

/// How the pieces of one set of ribbons look and last.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// How long a piece lasts, in seconds, if it isn't written over first.
    pub life: f32,
    /// How old a piece is when it starts to fade out, in seconds: it is gone at `life`.
    pub fade_from: f32,
    /// How many metres of ribbon one height of the picture covers: the picture repeats
    /// along the ribbon every this far.
    pub period: f32,
    /// How far from the camera a piece starts to fade out, in metres, and where it is
    /// gone, so that the far ground is not peppered with specks.
    pub seen_to: f32,
    pub gone_at: f32,
}

/// One piece of ribbon, as it is laid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    /// Its four corners, in metres, each on the ground: the left and right of its first
    /// edge, then the left and right of its last. The next piece of the same ribbon
    /// starts from this one's last edge.
    pub corners: [Vec3; 4],
    /// How far along the ribbon its first edge and its last edge are, in metres, by which
    /// the picture repeats.
    pub along: [f32; 2],
    /// How much of the ground's colour the ribbon leaves at its darkest, in red, green
    /// and blue, from 0 (black) to 1 (all of it); and how strong the piece is, from 0
    /// (not there) to 1 (as dark as that).
    pub color: LinearRgba,
}

/// As `Look` in `ribbons.wgsl`.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
struct LookUniform {
    life: f32,
    fade_from: f32,
    period: f32,
    seen_to: f32,
    gone_at: f32,
}

/// One piece as the shader reads it, as `Slot` in `ribbons.wgsl`: 96 bytes, in the buffer
/// at `slot * Slot::SHADER_SIZE`.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
struct Slot {
    a: Vec4,
    b: Vec4,
    c: Vec4,
    d: Vec4,
    color: Vec4,
    /// When it was laid and when it goes, on the shader's clock.
    life: Vec2,
    padding: Vec2,
}

impl Slot {
    fn new(piece: &Piece, life: [f32; 2]) -> Self {
        let [a, b, c, d] = piece.corners;
        let [first, last] = piece.along;
        Self {
            a: a.extend(first),
            b: b.extend(first),
            c: c.extend(last),
            d: d.extend(last),
            color: piece.color.to_vec4(),
            life: Vec2::from(life),
            padding: Vec2::ZERO,
        }
    }
}

/// How the pieces of a set of ribbons look, for `ribbons.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct RibbonMaterial {
    #[uniform(0)]
    look: LookUniform,
    /// One `Slot` for each piece of the mesh.
    #[storage(1, read_only)]
    pieces: Handle<ShaderBuffer>,
    /// How dark the ribbon is at each point, in the red channel, from 0 (not at all) to
    /// 1: across the ribbon from left to right, and along it down the picture, which
    /// repeats.
    #[texture(2)]
    #[sampler(3)]
    picture: Handle<Image>,
}

impl Material for RibbonMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Multiply
    }

    fn depth_bias(&self) -> f32 {
        FLAT_SORT
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Seen from either side: a piece on a slope is seen from below from under the
        // slope, and whoever lays it may wind it either way.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// A set of ribbons, on the entity that draws them.
#[derive(Component)]
pub struct Ribbons {
    /// Per slot: when its piece was laid and when it goes, on the shader's clock.
    lives: Vec<[f32; 2]>,
    /// The slot the next piece goes in: the slots are taken in turn, round and round.
    next: usize,
    /// How long a piece lasts, in seconds.
    life: f32,
    /// The pieces laid since they were last sent to the buffer, and their slots.
    laid: Vec<(usize, Piece, [f32; 2])>,
    buffer: Handle<ShaderBuffer>,
}

impl Ribbons {
    /// Lays `piece` now (`clock(time)`), in the next slot round, over whatever was there.
    pub fn lay(&mut self, now: f32, piece: Piece) {
        let slot = self.next;
        self.next = (slot + 1) % self.lives.len();
        let life = [now, now + self.life];
        self.lives[slot] = life;
        self.laid.push((slot, piece, life));
    }

    /// How many pieces are on the ground now (`clock(time)`).
    #[cfg(test)]
    fn lying(&self, now: f32) -> usize {
        self.lives
            .iter()
            .filter(|[laid, goes]| now >= *laid && now < *goes)
            .count()
    }
}

/// The time on the shader's clock, in seconds, for `Ribbons::lay`.
pub fn clock(time: &Time) -> f32 {
    time.elapsed_secs_wrapped()
}

/// Where a set of ribbons' mesh, material and buffer are kept: what `ribbons` needs.
#[derive(SystemParam)]
pub struct RibbonAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<RibbonMaterial>>,
    buffers: ResMut<'w, Assets<ShaderBuffer>>,
}

/// A set of ribbons of up to `slots` pieces that look as `look` says, drawn with
/// `picture` (see `RibbonMaterial::picture`). Spawn it with what else the slice needs on
/// it.
pub fn ribbons(
    assets: &mut RibbonAssets,
    slots: usize,
    look: Look,
    picture: Handle<Image>,
) -> impl Bundle {
    let mesh = assets.meshes.add(quads(slots));
    // Made on the graphics card, where it starts as zeros: every slot empty. Nothing is
    // kept on the CPU; `pool::upload` writes the slots as pieces are laid.
    let buffer = assets.buffers.add(ShaderBuffer::with_size(
        slots * Slot::SHADER_SIZE.get() as usize,
        RenderAssetUsages::default(),
    ));
    let material = assets.materials.add(RibbonMaterial {
        look: LookUniform {
            life: look.life,
            fade_from: look.fade_from,
            period: look.period,
            seen_to: look.seen_to,
            gone_at: look.gone_at,
        },
        pieces: buffer.clone(),
        picture,
    });
    (
        Ribbons {
            lives: vec![[0.0, 0.0]; slots],
            next: 0,
            life: look.life,
            laid: Vec::new(),
            buffer,
        },
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::default(),
        // Placed by the shader, so never outside the view as a whole.
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
    )
}

/// A mesh of `slots` four-sided pieces, of four corners each, which the shader places
/// by their vertex numbers: it has positions, all zero, only because a mesh must. It
/// never changes, so it is only on the graphics card.
fn quads(slots: usize) -> Mesh {
    let indices: Vec<u32> = (0..slots as u32)
        .flat_map(|quad| {
            let first = quad * 4;
            [first, first + 2, first + 1, first, first + 3, first + 2]
        })
        .collect();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; slots * 4])
    .with_inserted_indices(Indices::U32(indices))
}

/// Hands the pieces laid this frame to the render world, and keeps each set of ribbons
/// at the camera, so that it is sorted with what is nearest (see `pool`).
fn draw(
    mut uploads: ResMut<Uploads>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut ribbons: Query<(&mut Ribbons, &mut Transform)>,
) {
    let eye = cameras
        .iter()
        .find(|(camera, _)| camera.is_active)
        .map(|(_, transform)| transform.translation());
    for (mut ribbons, mut transform) in &mut ribbons {
        if let Some(eye) = eye {
            transform.translation = eye;
        }
        if ribbons.laid.is_empty() {
            continue;
        }
        let laid = std::mem::take(&mut ribbons.laid);
        let buffer = ribbons.buffer.id();
        uploads
            .0
            .extend(runs(&laid).into_iter().map(|(offset, bytes)| Upload {
                buffer,
                offset,
                bytes,
            }));
    }
}

/// `laid` as runs of neighbouring slots, each as where it starts in the buffer, in bytes,
/// and its records. Slots are taken in turn, so what is laid in one frame is mostly one
/// run; a run that reaches the end of the ring is cut there.
fn runs(laid: &[(usize, Piece, [f32; 2])]) -> Vec<(u64, Vec<u8>)> {
    let mut sorted: Vec<_> = laid.iter().collect();
    // Stable, so that of a slot written twice in one frame, the later one wins.
    sorted.sort_by_key(|(slot, ..)| *slot);
    let size = Slot::SHADER_SIZE.get();
    let mut runs: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut next_slot = None;
    for (slot, piece, life) in sorted {
        let mut bytes = StorageBuffer::new(Vec::with_capacity(size as usize));
        bytes
            .write(&Slot::new(piece, *life))
            .expect("a slot fits in its own bytes");
        let bytes = bytes.into_inner();
        match runs.last_mut() {
            Some((_, run)) if next_slot == Some(*slot + 1) => {
                let at = run.len() - size as usize;
                run[at..].copy_from_slice(&bytes);
            }
            Some((_, run)) if next_slot == Some(*slot) => run.extend_from_slice(&bytes),
            _ => runs.push((*slot as u64 * size, bytes)),
        }
        next_slot = Some(*slot + 1);
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ribbons(slots: usize) -> Ribbons {
        Ribbons {
            lives: vec![[0.0, 0.0]; slots],
            next: 0,
            life: 10.0,
            laid: Vec::new(),
            buffer: Handle::default(),
        }
    }

    fn piece(along: f32) -> Piece {
        Piece {
            corners: [Vec3::ZERO, Vec3::X, Vec3::Z, Vec3::new(1.0, 0.0, 1.0)],
            along: [along, along + 1.0],
            color: LinearRgba::WHITE,
        }
    }

    #[test]
    fn the_slots_are_a_ring_and_the_oldest_piece_is_written_over() {
        let mut ribbons = ribbons(3);
        for n in 0..3 {
            ribbons.lay(n as f32, piece(n as f32));
        }
        assert_eq!(ribbons.lying(2.5), 3);
        // A fourth goes where the first was.
        ribbons.lay(3.0, piece(3.0));
        let slots: Vec<usize> = ribbons.laid.iter().map(|(slot, ..)| *slot).collect();
        assert_eq!(slots, [0, 1, 2, 0]);
        assert_eq!(ribbons.lives[0], [3.0, 13.0]);
        // Pieces go when their life is up.
        assert_eq!(ribbons.lying(11.5), 2);
        assert_eq!(ribbons.lying(13.5), 0);
    }

    #[test]
    fn a_slot_holds_the_corners_with_how_far_along_each_edge_is() {
        let slot = Slot::new(&piece(4.0), [1.0, 2.0]);
        assert_eq!(slot.a, Vec4::new(0.0, 0.0, 0.0, 4.0));
        assert_eq!(slot.b, Vec4::new(1.0, 0.0, 0.0, 4.0));
        assert_eq!(slot.c, Vec4::new(0.0, 0.0, 1.0, 5.0));
        assert_eq!(slot.d, Vec4::new(1.0, 0.0, 1.0, 5.0));
        assert_eq!(slot.life, Vec2::new(1.0, 2.0));
    }

    #[test]
    fn neighbouring_slots_are_sent_as_one_run() {
        let size = Slot::SHADER_SIZE.get();
        let laid = |slot: usize| (slot, piece(0.0), [1.0, 2.0]);
        let runs = runs(&[laid(5), laid(3), laid(4), laid(9)]);
        let placed: Vec<(u64, usize)> = runs.iter().map(|(at, bytes)| (*at, bytes.len())).collect();
        assert_eq!(
            placed,
            [(3 * size, 3 * size as usize), (9 * size, size as usize)]
        );
    }

    #[test]
    fn the_shader_reads_a_slot_and_the_look_in_the_order_they_are_written() {
        // WGSL lays out a struct as encase does, so the fields in the same order give the
        // same bytes.
        assert_eq!(Slot::SHADER_SIZE.get(), 96);
        let shader = include_str!("../shaders/ribbons.wgsl");
        let in_order = |name: &str, fields: &[&str]| {
            let mut from = shader.find(name).unwrap_or_else(|| panic!("{name}"));
            for field in fields {
                let at = shader[from..]
                    .find(&format!("    {field}: "))
                    .unwrap_or_else(|| panic!("{field} out of order in ribbons.wgsl"));
                from += at + 1;
            }
        };
        in_order(
            "struct Slot {",
            &["a", "b", "c", "d", "color", "life", "padding"],
        );
        in_order(
            "struct Look {",
            &["life", "fade_from", "period", "seen_to", "gone_at"],
        );
    }

    #[test]
    fn the_mesh_has_four_corners_a_piece_in_the_order_the_shader_takes_them() {
        let mesh = quads(2);
        assert_eq!(mesh.count_vertices(), 8);
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("U32 indices");
        };
        assert_eq!(indices.len(), 12);
        // The second piece's corners are vertices 4 to 7, which the shader reads as slot
        // 1 (vertex / 4), corners 0 to 3 (vertex % 4).
        assert!(indices[6..].iter().all(|index| (4..8).contains(index)));
    }
}
