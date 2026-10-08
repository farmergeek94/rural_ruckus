//! Sheets of water poured on the graphics card.
//!
//! A sheet is what a source pours out over time: a run of lips, one poured every
//! `Flow::every` seconds. A lip is a line of water that leaves the source all at once,
//! from its root to its crest, each point of it at its own speed, as water peels off a
//! tire from the tread out. Each point then flies as a particle of `pool` does (gravity,
//! and drag towards the air), so that the lip opens out, rises and pours back down. Two
//! lips one after the other are joined by a band of the sheet, so that the water is one
//! surface, not a scatter of drops.
//!
//! A set of sheets is one mesh of strips, and a storage buffer with one record for each
//! lip. Each strip is `rows` bands, a ring of lips: when a lip is poured, the CPU writes its
//! record once, and `sheet.wgsl` works out where every point of it is, every frame, from
//! the record and the time. A band is drawn only between two lips that one source poured
//! one after the other (`Row::follows`): the band from the newest lip round to the oldest,
//! and those across a pause in the pouring, put their corners at one point.
//!
//! Its picture moves with the water: along the sheet, it is placed by when the water was
//! poured, so that it flows out from the source. Across it, it goes from the root (left)
//! to the crest (right). As a lip ages, the thin parts of the picture tear away first
//! (`Flow::tear`), so that the sheet breaks into strands before it is gone. It fades out
//! where it sinks under `Flow::floor`, as it pours back into the water.
//!
//! Sheets are not sorted against each other, and the entity is kept at the camera, as a
//! pool of particles is. They are not lit; they are as bright as they are told (`light`).
//!
//! Knows nothing of the game: the `particles` slice spawns the sheets and pours into them.
//! Only for an app that can draw, and only after `pool::add`, whose uploads it shares.

use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::asset::io::embedded::EmbeddedAssetRegistry;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::ecs::system::SystemParam;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::encase::StorageBuffer;
use bevy::render::render_resource::{AsBindGroup, ShaderSize, ShaderType};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;

use super::pool::Uploads;

/// Where `sheet.wgsl` is among the embedded assets (see `add`).
const SHADER_PATH: &str = "embedded://monster_truck_rural_ruckus/shaders/sheet.wgsl";

/// How many times the gap between lips a source may leave and its sheet still be joined
/// across it: a frame that comes late does not tear the sheet.
const MOST_GAP: f32 = 3.0;
/// How early, as a share of the gap between lips, the next may come: frames do not come
/// at even times, and a lip that waits a whole frame more leaves an uneven sheet.
const EARLY: f32 = 0.75;

pub type SheetMaterial = ExtendedMaterial<StandardMaterial, SheetFlow>;

/// Makes sheets work in `app`. After `pool::add`.
pub fn add(app: &mut App) {
    app.world_mut()
        .resource_mut::<EmbeddedAssetRegistry>()
        .insert_asset(
            PathBuf::from("src/shaders/sheet.wgsl"),
            Path::new(SHADER_PATH.trim_start_matches("embedded://")),
            include_bytes!("../shaders/sheet.wgsl").as_slice(),
        );
    app.add_plugins(MaterialPlugin::<SheetMaterial>::default())
        // After every slice's `Update`, where they are poured.
        .add_systems(PostUpdate, draw);
}

/// Makes every set of sheets as bright as `level` says, from 0 (black) to 1 (its own
/// colours). Only a real change is sent.
pub fn light(level: f32, sheets: &Query<&Sheets>, materials: &mut Assets<SheetMaterial>) {
    let level = level.clamp(0.0, 1.0);
    let color = Color::linear_rgb(level, level, level);
    for sheets in sheets {
        if materials
            .get(&sheets.material)
            .is_some_and(|material| material.base.base_color != color)
            && let Some(mut material) = materials.get_mut(&sheets.material)
        {
            material.base.base_color = color;
        }
    }
}

/// How the water of a set of sheets moves and looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flow {
    /// How fast it falls, in m/s².
    pub gravity: f32,
    /// How much of its speed through the air it loses each second. Above 0.
    pub drag: f32,
    /// How fast the air goes, in m/s.
    pub air: Vec3,
    /// How long a lip lasts, in seconds.
    pub life: f32,
    /// How old a lip is when it starts to fade out, in seconds: it is clear at `life`.
    pub fade_from: f32,
    /// How often a source pours a lip, in seconds. Less is a smoother sheet, and more
    /// records sent.
    pub every: f32,
    /// The height it pours back down into, in metres, and how far under it it is gone.
    pub floor: f32,
    pub sink: f32,
    /// How much of the picture's thin water has torn away by the end of a lip's life, from
    /// 0 (none) to 1 (all of it). More breaks the sheet into strands sooner.
    pub tear: f32,
    /// How near what is behind it it starts to fade out, in metres, so that where a truck
    /// cuts through it there is no hard edge. At 0, it is cut off.
    pub soft: f32,
    /// How near the camera it starts to fade out, in metres.
    pub near_camera: f32,
    /// How many times the picture goes by along the sheet for each second of pouring.
    pub picture_rate: f32,
}

impl Flow {
    /// How many lips a strip holds: enough for a whole lip's life, and the one being
    /// poured over the oldest.
    fn rows(&self) -> usize {
        (self.life / self.every).ceil() as usize + 2
    }
}

/// One lip, as it is poured.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lip {
    /// Where its root and its crest start, in metres. The lip is the line between.
    pub root: Vec3,
    pub crest: Vec3,
    /// How fast its root and its crest go, in m/s. The points between go between.
    pub root_velocity: Vec3,
    pub crest_velocity: Vec3,
    /// How solid it is, from 0 (not there) to 1.
    pub strength: f32,
}

/// The water of a set of sheets, for `sheet.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SheetFlow {
    #[uniform(100)]
    flow: FlowUniform,
    /// One `Row` for each lip of each strip.
    #[storage(101, read_only)]
    rows: Handle<ShaderBuffer>,
}

impl MaterialExtension for SheetFlow {
    fn vertex_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }
}

/// As `Flow` in `sheet.wgsl`, where each is explained.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
struct FlowUniform {
    air: Vec3,
    gravity: f32,
    drag: f32,
    life: f32,
    fade_from: f32,
    floor: f32,
    sink: f32,
    tear: f32,
    soft: f32,
    near_camera: f32,
    picture_rate: f32,
    rows: u32,
}

impl FlowUniform {
    fn new(flow: Flow) -> Self {
        Self {
            air: flow.air,
            gravity: flow.gravity,
            drag: flow.drag,
            life: flow.life,
            fade_from: flow.fade_from,
            floor: flow.floor,
            sink: flow.sink,
            tear: flow.tear,
            soft: flow.soft,
            near_camera: flow.near_camera,
            picture_rate: flow.picture_rate,
            rows: flow.rows() as u32,
        }
    }
}

/// One lip as the shader reads it, as `Row` in `sheet.wgsl`: 64 bytes.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
struct Row {
    root: Vec3,
    /// When it was poured, on the shader's clock.
    poured: f32,
    crest: Vec3,
    /// When the lip before it in its sheet was poured, or -1 when it starts a sheet.
    follows: f32,
    root_velocity: Vec3,
    /// When it is gone, on the shader's clock.
    goes: f32,
    crest_velocity: Vec3,
    strength: f32,
}

/// One strip: whose sheet it holds, and where it is in its ring of lips.
#[derive(Clone, Copy, Debug)]
struct Strip {
    source: Option<u64>,
    /// The row the next lip goes in.
    next: usize,
    /// When its newest lip was poured, and when that lip is gone.
    poured: f32,
    goes: f32,
}

impl Strip {
    const EMPTY: Strip = Strip {
        source: None,
        next: 0,
        poured: f32::NEG_INFINITY,
        goes: f32::NEG_INFINITY,
    };

    /// Whether none of its lips is there `now`. After the clock goes back to 0, those
    /// poured before are gone.
    fn is_free(&self, now: f32) -> bool {
        now >= self.goes || now < self.poured
    }
}

/// A set of sheets, on the entity that draws them.
#[derive(Component)]
pub struct Sheets {
    flow: Flow,
    rows: usize,
    strips: Vec<Strip>,
    /// The lips poured since they were last sent to the buffer, and their rows in it.
    poured: Vec<(usize, Row)>,
    buffer: Handle<ShaderBuffer>,
    material: Handle<SheetMaterial>,
}

impl Sheets {
    /// Pours `lip` from `source` now (`pool::clock`), on the end of its sheet, if it is
    /// time for its next lip and there is room. A source is any number that stays the same
    /// while it pours. Call it every frame while the source pours: it keeps the time.
    /// Returns whether a lip was poured.
    pub fn pour(&mut self, source: u64, now: f32, lip: Lip) -> bool {
        let flow = self.flow;
        let own = self
            .strips
            .iter()
            .position(|strip| strip.source == Some(source) && !strip.is_free(now));
        let (strip, follows) = match own {
            Some(index) => {
                let since = now - self.strips[index].poured;
                if since < flow.every * EARLY {
                    return false;
                }
                let joined = since <= flow.every * MOST_GAP;
                (
                    index,
                    if joined {
                        self.strips[index].poured
                    } else {
                        -1.0
                    },
                )
            }
            None => {
                let Some(index) = self.strips.iter().position(|strip| strip.is_free(now)) else {
                    return false;
                };
                (index, -1.0)
            }
        };
        let goes = now + flow.life;
        let row = strip * self.rows + self.strips[strip].next;
        self.strips[strip] = Strip {
            source: Some(source),
            next: (self.strips[strip].next + 1) % self.rows,
            poured: now,
            goes,
        };
        self.poured.push((
            row,
            Row {
                root: lip.root,
                poured: now,
                crest: lip.crest,
                follows,
                root_velocity: lip.root_velocity,
                goes,
                crest_velocity: lip.crest_velocity,
                strength: lip.strength,
            },
        ));
        true
    }
}

/// Where a set of sheets' mesh, material and buffer are kept: what `sheets` needs.
#[derive(SystemParam)]
pub struct SheetAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<SheetMaterial>>,
    buffers: ResMut<'w, Assets<ShaderBuffer>>,
}

/// A set of `strips` sheets, each `columns` points across, whose water moves as `flow`
/// says, drawn with `picture`: a picture in white, which repeats along the sheet (its V)
/// and goes from root to crest across it (its U). Spawn it with what else the slice needs
/// on it.
pub fn sheets(
    assets: &mut SheetAssets,
    strips: usize,
    columns: usize,
    flow: Flow,
    picture: Handle<Image>,
) -> impl Bundle {
    let rows = flow.rows();
    let mesh = assets.meshes.add(strip_mesh(strips, rows, columns));
    // Made on the graphics card, where it starts as zeros: no lip goes after 0, so none is
    // drawn.
    let buffer = assets.buffers.add(ShaderBuffer::with_size(
        strips * rows * Row::SHADER_SIZE.get() as usize,
        RenderAssetUsages::default(),
    ));
    let material = assets.materials.add(SheetMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(picture),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            // Seen from either side.
            cull_mode: None,
            ..default()
        },
        extension: SheetFlow {
            flow: FlowUniform::new(flow),
            rows: buffer.clone(),
        },
    });
    (
        Sheets {
            flow,
            rows,
            strips: vec![Strip::EMPTY; strips],
            poured: Vec::new(),
            buffer,
            material: material.clone(),
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

/// A mesh of `strips` strips of `rows` bands, each band `columns` points across at its
/// older lip and again at its newer. Its first UV is how far across the lip a point is,
/// from 0 (the root) to 1 (the crest), and whether it is on the newer lip (1) or the older
/// (0); its second is which band it is, counted over all the strips. Its own points, not
/// shared with the next band, so that each band can be hidden on its own. It has colours,
/// all zero, only so that the standard material takes the colour the shader gives.
fn strip_mesh(strips: usize, rows: usize, columns: usize) -> Mesh {
    let columns = columns.max(2);
    let bands = strips * rows;
    let mut uvs = Vec::with_capacity(bands * 2 * columns);
    let mut band_of_point = Vec::with_capacity(bands * 2 * columns);
    let mut indices: Vec<u32> = Vec::with_capacity(bands * (columns - 1) * 6);
    for band in 0..bands {
        let first = (band * 2 * columns) as u32;
        for newer in 0..2 {
            for column in 0..columns {
                uvs.push([column as f32 / (columns - 1) as f32, newer as f32]);
                // Exact as a float up to 2^24 bands.
                band_of_point.push([band as f32, 0.0]);
            }
        }
        for column in 0..columns as u32 - 1 {
            let older = first + column;
            let newer = older + columns as u32;
            indices.extend_from_slice(&[older, older + 1, newer + 1, older, newer + 1, newer]);
        }
    }
    let points = bands * 2 * columns;
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; points])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, band_of_point)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; points])
    .with_inserted_indices(Indices::U32(indices))
}

/// Hands the lips poured this frame to the render world, and keeps each set of sheets at
/// the camera.
fn draw(
    mut uploads: ResMut<Uploads>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut sheets: Query<(&mut Sheets, &mut Transform)>,
) {
    let eye = cameras
        .iter()
        .find(|(camera, _)| camera.is_active)
        .map(|(_, transform)| transform.translation());
    let size = Row::SHADER_SIZE.get();
    for (mut sheets, mut transform) in &mut sheets {
        if let Some(eye) = eye {
            transform.translation = eye;
        }
        let buffer = sheets.buffer.id();
        for (row, record) in sheets.poured.drain(..) {
            let mut bytes = StorageBuffer::new(Vec::with_capacity(size as usize));
            bytes.write(&record).expect("a row fits in its own bytes");
            uploads.send(buffer, row as u64 * size, bytes.into_inner());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOW: Flow = Flow {
        gravity: 9.81,
        drag: 0.6,
        air: Vec3::ZERO,
        life: 0.5,
        fade_from: 0.3,
        every: 0.1,
        floor: 0.0,
        sink: 0.3,
        tear: 0.5,
        soft: 0.0,
        near_camera: 2.0,
        picture_rate: 1.0,
    };

    const LIP: Lip = Lip {
        root: Vec3::ZERO,
        crest: Vec3::X,
        root_velocity: Vec3::Y,
        crest_velocity: Vec3::new(2.0, 3.0, 0.0),
        strength: 1.0,
    };

    fn sheets(strips: usize) -> Sheets {
        Sheets {
            flow: FLOW,
            rows: FLOW.rows(),
            strips: vec![Strip::EMPTY; strips],
            poured: Vec::new(),
            buffer: Handle::default(),
            material: Handle::default(),
        }
    }

    #[test]
    fn a_strip_holds_a_whole_life_of_lips() {
        // 5 lips in a life, and room for the next over the oldest.
        assert_eq!(FLOW.rows(), 7);
    }

    #[test]
    fn a_source_pours_a_lip_each_time_its_gap_has_gone_by() {
        let mut sheets = sheets(2);
        assert!(sheets.pour(7, 10.0, LIP));
        assert!(!sheets.pour(7, 10.05, LIP));
        assert!(sheets.pour(7, 10.1, LIP));
        // In one strip, one row after the other, each lip on the one before.
        let poured: Vec<(usize, f32, f32)> = sheets
            .poured
            .iter()
            .map(|(row, record)| (*row, record.poured, record.follows))
            .collect();
        assert_eq!(poured, [(0, 10.0, -1.0), (1, 10.1, 10.0)]);
    }

    #[test]
    fn a_pause_starts_a_new_sheet() {
        let mut sheets = sheets(1);
        assert!(sheets.pour(7, 10.0, LIP));
        assert!(sheets.pour(7, 10.45, LIP));
        assert_eq!(sheets.poured[1].1.follows, -1.0);
    }

    #[test]
    fn each_source_has_its_own_strip_until_its_lips_are_gone() {
        let mut sheets = sheets(2);
        assert!(sheets.pour(1, 10.0, LIP));
        assert!(sheets.pour(2, 10.0, LIP));
        // No room for a third while the first two are there.
        assert!(!sheets.pour(3, 10.2, LIP));
        assert_eq!(sheets.poured[1].0, sheets.rows);
        // The first's lips are gone a life after its last: its strip is free.
        assert!(sheets.pour(3, 10.0 + FLOW.life, LIP));
        assert_eq!(sheets.poured[2].0, 1);
        assert_eq!(sheets.poured[2].1.follows, -1.0);
        // And after the clock goes back to 0.
        assert!(sheets.pour(4, 0.1, LIP));
    }

    #[test]
    fn a_strip_pours_round_its_ring() {
        let mut sheets = sheets(1);
        let rows = sheets.rows;
        for lip in 0..rows + 1 {
            assert!(sheets.pour(7, 10.0 + lip as f32 * FLOW.every, LIP));
        }
        assert_eq!(sheets.poured[rows].0, 0);
    }

    #[test]
    fn a_mesh_has_a_band_of_quads_for_each_row() {
        let mesh = strip_mesh(2, 3, 4);
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("no indices");
        };
        // 6 bands of 3 quads.
        assert_eq!(indices.len(), 6 * 3 * 6);
        assert_eq!(mesh.count_vertices(), 6 * 2 * 4);
        // Each band's quads use only its own points.
        for (quad, corners) in indices.chunks(6).enumerate() {
            let band = quad / 3;
            assert!(corners.iter().all(|corner| *corner as usize / 8 == band));
        }
    }

    #[test]
    fn the_shader_reads_a_row_and_the_flow_in_the_order_they_are_written() {
        assert_eq!(Row::SHADER_SIZE.get(), 64);
        let shader = include_str!("../shaders/sheet.wgsl");
        let in_order = |name: &str, fields: &[&str]| {
            let mut from = shader.find(&format!("struct {name} {{")).expect(name);
            for field in fields {
                let at = shader[from..]
                    .find(&format!("    {field}: "))
                    .unwrap_or_else(|| panic!("{field} out of order in sheet.wgsl"));
                from += at + 1;
            }
        };
        in_order(
            "Row",
            &[
                "root",
                "poured",
                "crest",
                "follows",
                "root_velocity",
                "goes",
                "crest_velocity",
                "strength",
            ],
        );
        in_order(
            "Flow",
            &[
                "air",
                "gravity",
                "drag",
                "life",
                "fade_from",
                "floor",
                "sink",
                "tear",
                "soft",
                "near_camera",
                "picture_rate",
                "rows",
            ],
        );
    }
}
