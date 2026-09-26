//! 3D models (.BIN), and the animation control files that share their extension. See
//! `docs/formats/model.md`.
//!
//! A model is a stream of little-endian 32-bit records, each starting with a number that
//! says what it is: a vertex list, then texture names and the faces that use them, up to
//! an end marker. The Order and Jump records are stepped over by their length, and the
//! records after them read in turn: in the one model that has them, that reads every face
//! once (`tests/pod_real_tracks.rs`).
//!
//! An animation control file names the models that are its keyframes: see
//! `KeyframeAnimation`.

use super::PodError;

/// Model units per foot, at the usual magnification.
const UNITS_PER_FOOT: f32 = 256.0;
/// The magnification at which `UNITS_PER_FOOT` holds, and the only one seen so far.
const USUAL_MAGNIFY: i32 = 65536;
/// Texture coordinates are 16.16 fixed point numbers running from 0 to 255 across a
/// texture, whatever its size in pixels.
const TEXTURE_COORD_SPAN: f32 = (255 << 16) as f32;
const MAX_CORNERS: usize = 256;
/// The length of a stored normal.
const NORMAL_LENGTH: f32 = 65535.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    /// In feet, relative to the model's origin: X, up, Z, in MTM2's left-handed axes.
    /// Placed in a track with heading `psi`, the model's Z axis points along the heading
    /// and its X axis along (cos psi, -sin psi) on the ground.
    pub vertices: Vec<[f32; 3]>,
    /// A unit normal for each vertex, pointing out of the model, in the same axes. Only
    /// models meant to be shaded smoothly, such as trucks, have them: otherwise empty.
    pub vertex_normals: Vec<[f32; 3]>,
    pub faces: Vec<Face>,
    /// The animated textures that faces name by `Face::texture_cycle`, in the order the
    /// file gives them.
    pub texture_cycles: Vec<TextureCycle>,
    /// Set if reading stopped early at a record that isn't understood. What was read up
    /// to there is still good.
    pub incomplete: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    /// The record type, as written. See the constants in `face_type`.
    pub kind: i32,
    /// File name of the texture, for a textured face. An animated texture gives its
    /// first frame here.
    pub texture: Option<String>,
    /// For a textured face with an animated texture: an index into
    /// `Model::texture_cycles`.
    pub texture_cycle: Option<usize>,
    /// The colour record in force, as written. Only untextured faces use it.
    pub color: u32,
    /// The material record in force, for a face of type 64, the only type that takes one.
    pub material: Option<Material>,
    /// Three or more, in order round the face. Taken with the right-hand rule in the
    /// model's own axes, the order gives the side the face is seen from.
    pub corners: Vec<Corner>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Corner {
    /// Index into `Model::vertices`.
    pub vertex: usize,
    /// Position in the texture, (across, down) from its top left, where 0 to 1 spans it.
    /// Zero for untextured faces. Some files hold rubbish here, far outside that range.
    pub uv: [f32; 2],
}

/// An animated texture (record 29): the file names of its frames, and a number that the
/// game here takes as how long each is shown. See `docs/formats/model.md`.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureCycle {
    /// File names of the frames, in the order they are written. At least one.
    pub frames: Vec<String>,
    /// The record's rate number read as 16.16 fixed point. The game here shows each frame
    /// for this many seconds, which is its own reading: the true unit is **open**. Zero
    /// or less shows the first frame only.
    pub seconds_per_frame: f32,
}

/// Values of `Face::kind`.
pub mod face_type {
    /// Untextured, coloured by the last colour record.
    pub const FLAT: [i32; 2] = [5, 25];
    /// Textured, and see-through wherever the texture is pure black.
    pub const CUTOUT: [i32; 2] = [17, 51];
    /// Shaded smoothly, from the normals at its vertices, rather than as a flat facet.
    pub const SMOOTH: [i32; 2] = [41, 51];
    /// Textured, and drawn as the material record in force says (MTM2 Community Patch 3).
    pub const MATERIAL: i32 = 64;
}

/// What a material record (type 63) says. Only what the game uses is kept.
/// **Reference:** JSTruckViewer, `docs/BIN_HD_FORMAT.md`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    /// See `material_flag`.
    pub flags: u32,
    /// How opaque a blended face is, from 0 to 1.
    pub base_alpha: f32,
}

/// Bits of a material record's flags. **Reference:** JSTrackViewer,
/// `src/shared/mrgl-material.js`. Only those the game uses are here.
pub mod material_flag {
    /// See-through where the texture's alpha is under a half.
    pub const ALPHA_TEST: u32 = 0x0008;
    /// Seen from both sides, as leaves and fences are.
    pub const TWO_SIDED: u32 = 0x0080;
    /// Blended with what is behind it, as glass is, by `Material::base_alpha` and the
    /// texture's alpha. Alpha test takes precedence where both are set.
    pub const BLEND: u32 = 0x0004;
}

impl Face {
    /// Whether this face has holes in it: where its texture is pure black, for an 8-bit
    /// texture, or where its alpha is low, for a true-colour one.
    pub fn is_cutout(&self) -> bool {
        face_type::CUTOUT.contains(&self.kind) || self.has_material_flag(material_flag::ALPHA_TEST)
    }

    /// Whether the face is seen from behind as well as from in front.
    pub fn is_two_sided(&self) -> bool {
        self.has_material_flag(material_flag::TWO_SIDED)
    }

    /// How opaque the face is, for a face blended with what is behind it. `None` for a
    /// face that is opaque or cut out.
    pub fn blend_alpha(&self) -> Option<f32> {
        let material = self.material?;
        (material.flags & material_flag::BLEND != 0 && !self.is_cutout())
            .then_some(material.base_alpha)
    }

    fn has_material_flag(&self, flag: u32) -> bool {
        self.material
            .is_some_and(|material| material.flags & flag != 0)
    }

    /// Whether the face is shaded from `Model::vertex_normals`.
    pub fn is_smooth(&self) -> bool {
        face_type::SMOOTH.contains(&self.kind)
    }
}

// Record types. Those marked "reference" have not been seen in a real file here; their
// lengths come from JSTrackViewer's table, so that a model using one can still be read.
const END: i32 = 0;
const VERTEX_LIST: i32 = 2;
const NORMAL_LIST: i32 = 3;
const COLOR: i32 = 10;
const ORDER: i32 = 12;
const TEXTURE: i32 = 13;
const JUMP: i32 = 18;
const MAGNIFY: i32 = 20;
const TEXTURE_CYCLE: i32 = 29;
/// A texture record with a 64-byte name (reference).
const TEXTURE_64: i32 = 62;
/// A material, in force for the faces of type 64 that follow: flags, then 10 more ints, of
/// which the fourth is the base alpha, 16.16 fixed point.
const MATERIAL: i32 = 63;
/// 16.16 fixed point numbers in a material record.
const FIXED_ONE: f32 = 65536.0;
/// Faces with a vertex index and texture coordinates for each corner.
const MAPPED_FACES: [i32; 8] = [14, 17, 24, 34, 41, 51, 52, 64];
/// Faces with only a vertex index for each corner.
const UNMAPPED_FACES: [i32; 4] = [5, 6, 15, 25];
/// (type, length in 32-bit words including the type) for records that are skipped.
const SKIPPED: [(i32, usize); 8] = [
    (1, 4),
    (9, 8),
    (11, 2),
    (16, 5),
    (23, 3),
    (61, 2),
    (65, 1094),
    (66, 8),
];
/// What an animation control file starts with instead of `MAGNIFY`. **Reference:**
/// JSTrackViewer, `src/worker/bin-decoder.js`, which calls it MRGL_KEYFRAME.
const ANIMATION_CONTROL: i32 = 0x20;
/// The most frames an animation control file has room for: its names start at offset 24,
/// and the record is 344 bytes long (**reference**, JSTrackViewer).
const MAX_KEYFRAMES: usize = 20;
/// The width of a frame's name in an animation control file.
const KEYFRAME_NAME_WIDTH: usize = 16;

impl Model {
    pub fn parse(bytes: &[u8]) -> Result<Self, PodError> {
        let mut reader = Reader { bytes, offset: 0 };

        let first = reader.int("first record")?;
        if first == ANIMATION_CONTROL {
            return Err(PodError::Unsupported(
                "an animation control file, not a model".into(),
            ));
        }
        if first != MAGNIFY {
            return Err(PodError::Unsupported(format!(
                "a model that starts with record {first}"
            )));
        }
        let magnify = reader.int("magnification")?;
        let scale = USUAL_MAGNIFY as f32 / magnify.max(1) as f32 / UNITS_PER_FOOT;

        let mut model = Model {
            vertices: Vec::new(),
            vertex_normals: Vec::new(),
            faces: Vec::new(),
            texture_cycles: Vec::new(),
            incomplete: None,
        };
        let mut texture: Option<String> = None;
        let mut texture_cycle: Option<usize> = None;
        let mut color = 0;
        let mut material = None;

        while reader.remaining() >= 4 {
            let record = reader.int("record type")?;
            match record {
                END => return Ok(model),
                VERTEX_LIST => {
                    let _slot = reader.int("vertex list slot")?;
                    let count = reader.count("vertex count", 12)?;
                    let mut vertices = Vec::with_capacity(count);
                    for _ in 0..count {
                        let [x, up, z] = [(); 3].map(|()| reader.int("vertex"));
                        vertices.push([x? as f32 * scale, up? as f32 * scale, z? as f32 * scale]);
                    }
                    // Later lists are other levels of detail or frames. The first is the model.
                    if model.vertices.is_empty() {
                        model.vertices = vertices;
                    }
                }
                NORMAL_LIST => {
                    let _slot = reader.int("normal list slot")?;
                    let count = reader.count("normal count", 12)?;
                    let mut normals = Vec::with_capacity(count);
                    for _ in 0..count {
                        let [x, up, z] = [(); 3].map(|()| reader.int("normal"));
                        normals.push([x?, up?, z?].map(|part| part as f32 / NORMAL_LENGTH));
                    }
                    // One for each vertex of the list before it, or it is no use.
                    if model.vertex_normals.is_empty() && normals.len() == model.vertices.len() {
                        model.vertex_normals = normals;
                    }
                }
                TEXTURE => {
                    let _slot = reader.int("texture slot")?;
                    texture = Some(reader.name(16)?);
                    texture_cycle = None;
                }
                TEXTURE_64 => {
                    let _slot = reader.int("texture slot")?;
                    texture = Some(reader.name(64)?);
                    texture_cycle = None;
                }
                MATERIAL => {
                    let flags = reader.int("material flags")? as u32;
                    // Reflectivity, fresnel bias and fresnel strength come first.
                    reader.skip(12)?;
                    let base_alpha = reader.int("material base alpha")? as f32 / FIXED_ONE;
                    reader.skip(24)?;
                    material = Some(Material {
                        flags,
                        base_alpha: base_alpha.clamp(0.0, 1.0),
                    });
                }
                TEXTURE_CYCLE => {
                    let _slot = reader.int("texture cycle slot")?;
                    let count = reader.count("texture cycle length", 32)?;
                    // The ints either side of the rate are 0 in every file seen. What they
                    // are is open.
                    let _frame = reader.int("texture cycle frame")?;
                    let rate = reader.int("texture cycle rate")?;
                    reader.skip(8)?;
                    let mut frames = Vec::with_capacity(count);
                    for _ in 0..count {
                        frames.push(reader.name(32)?);
                    }
                    // With no frames the game would divide by zero; here the texture
                    // before it stays in force.
                    if let Some(first) = frames.first() {
                        texture = Some(first.clone());
                        model.texture_cycles.push(TextureCycle {
                            frames,
                            seconds_per_frame: rate as f32 / FIXED_ONE,
                        });
                        texture_cycle = Some(model.texture_cycles.len() - 1);
                    }
                }
                COLOR => color = reader.int("colour")? as u32,
                // Order and Jump move where the game draws from. Stepped over, as
                // JSTrackViewer steps over them; see `docs/formats/model.md`.
                ORDER => reader.skip(24)?,
                JUMP => reader.skip(4)?,
                MAGNIFY => reader.skip(4)?,
                _ if MAPPED_FACES.contains(&record) || UNMAPPED_FACES.contains(&record) => {
                    let mapped = MAPPED_FACES.contains(&record);
                    let count = reader.count("corner count", if mapped { 12 } else { 4 })?;
                    if count > MAX_CORNERS {
                        return Err(reader.bad("corner count", count));
                    }
                    // The face's normal and one more number, which the corners make redundant.
                    reader.skip(16)?;
                    let mut corners = Vec::with_capacity(count);
                    for _ in 0..count {
                        let vertex = reader.int("corner vertex")?;
                        let uv = if mapped {
                            [(); 2].map(|()| reader.int("texture coordinate"))
                        } else {
                            [Ok(0), Ok(0)]
                        };
                        let [across, down] = uv;
                        corners.push(Corner {
                            vertex: usize::try_from(vertex)
                                .ok()
                                .filter(|&vertex| vertex < model.vertices.len())
                                .ok_or_else(|| reader.bad("corner vertex", vertex))?,
                            uv: [
                                across? as f32 / TEXTURE_COORD_SPAN,
                                down? as f32 / TEXTURE_COORD_SPAN,
                            ],
                        });
                    }
                    model.faces.push(Face {
                        kind: record,
                        texture: if mapped { texture.clone() } else { None },
                        texture_cycle: texture_cycle.filter(|_| mapped),
                        color,
                        material: material.filter(|_| record == face_type::MATERIAL),
                        corners,
                    });
                }
                _ => match SKIPPED.iter().find(|(kind, _)| *kind == record) {
                    Some((_, words)) => reader.skip((words - 1) * 4)?,
                    None => {
                        model.incomplete = Some(format!(
                            "unknown record {record} at offset {}",
                            reader.offset - 4
                        ));
                        return Ok(model);
                    }
                },
            }
        }

        model.incomplete = Some("no end record".into());
        Ok(model)
    }

    /// Lowest and highest corner of the box round the model, in feet.
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let mut low = [f32::INFINITY; 3];
        let mut high = [f32::NEG_INFINITY; 3];
        for vertex in &self.vertices {
            for axis in 0..3 {
                low[axis] = low[axis].min(vertex[axis]);
                high[axis] = high[axis].max(vertex[axis]);
            }
        }
        (low, high)
    }

    /// File names of the textures the faces use, each once, in order of first use: every
    /// frame of an animated texture, since the game loads them all before it draws.
    pub fn textures(&self) -> Vec<&str> {
        let mut names: Vec<&str> = Vec::new();
        for face in &self.faces {
            let used: Vec<&str> = match face
                .texture_cycle
                .and_then(|index| self.texture_cycles.get(index))
            {
                Some(cycle) => cycle.frames.iter().map(String::as_str).collect(),
                None => face.texture.as_deref().into_iter().collect(),
            };
            for name in used {
                if !names.iter().any(|known| known.eq_ignore_ascii_case(name)) {
                    names.push(name);
                }
            }
        }
        names
    }
}

/// An animation control file: a .BIN that is not a model but names the models that are
/// the keyframes of one (**reference**: JSTrackViewer). How the frames are shown is the
/// game's own. See `docs/formats/model.md`.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyframeAnimation {
    /// File names of the models that are the frames, in order: from 1 to 20 of them. Each
    /// has as many vertices as the first.
    pub frames: Vec<String>,
    /// The file's rate number read as 16.16 fixed point. The game here takes this many
    /// seconds from one frame to the next, which is its own reading: the true unit is
    /// **open**.
    pub seconds_per_frame: f32,
}

impl KeyframeAnimation {
    /// Whether the bytes of a .BIN are an animation control file rather than a model.
    pub fn is_in(bytes: &[u8]) -> bool {
        bytes.get(..4) == Some(&ANIMATION_CONTROL.to_le_bytes())
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, PodError> {
        let mut reader = Reader { bytes, offset: 0 };
        let first = reader.int("first record")?;
        if first != ANIMATION_CONTROL {
            return Err(PodError::Unsupported(format!(
                "an animation control file that starts with record {first}"
            )));
        }
        let _unknown = reader.int("animation control")?;
        let count = reader.int("keyframe count")?;
        let count = usize::try_from(count)
            .ok()
            .filter(|count| (1..=MAX_KEYFRAMES).contains(count))
            .ok_or_else(|| reader.bad("keyframe count", count))?;
        let rate = reader.int("keyframe rate")?;
        // Frames that take no time can't be stepped through.
        if rate <= 0 {
            return Err(reader.bad("keyframe rate", rate));
        }
        // 0 in every file seen. What they are is open.
        reader.skip(8)?;
        let mut frames = Vec::with_capacity(count);
        for _ in 0..count {
            let name = reader.name(KEYFRAME_NAME_WIDTH)?;
            if name.is_empty() {
                return Err(reader.bad("keyframe name", "nothing"));
            }
            frames.push(name);
        }
        Ok(Self {
            frames,
            seconds_per_frame: rate as f32 / FIXED_ONE,
        })
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Reader<'_> {
    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn take(&mut self, length: usize, what: &str) -> Result<&[u8], PodError> {
        let end = self
            .offset
            .checked_add(length)
            .filter(|&end| end <= self.bytes.len());
        let Some(end) = end else {
            return Err(PodError::Truncated {
                what: what.to_string(),
                offset: self.offset,
                needed: length,
                available: self.remaining(),
            });
        };
        let taken = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(taken)
    }

    fn int(&mut self, what: &str) -> Result<i32, PodError> {
        let raw: [u8; 4] = self.take(4, what)?.try_into().expect("four bytes");
        Ok(i32::from_le_bytes(raw))
    }

    /// A count of things `item_size` bytes each, checked against what is left of the
    /// file, so that a nonsense count can't ask for gigabytes.
    fn count(&mut self, what: &str, item_size: usize) -> Result<usize, PodError> {
        let count = self.int(what)?;
        usize::try_from(count)
            .ok()
            .filter(|&count| count <= self.remaining() / item_size)
            .ok_or_else(|| self.bad(what, count))
    }

    fn skip(&mut self, length: usize) -> Result<(), PodError> {
        self.take(length, "skipped record").map(|_| ())
    }

    /// A fixed-width field holding a NUL-terminated name.
    fn name(&mut self, width: usize) -> Result<String, PodError> {
        let field = self.take(width, "name")?;
        let end = field.iter().position(|&byte| byte == 0).unwrap_or(width);
        Ok(field[..end].iter().map(|&byte| byte as char).collect())
    }

    fn bad(&self, field: &str, value: impl std::fmt::Display) -> PodError {
        PodError::BadValue {
            field: format!("{field} before offset {}", self.offset),
            line: 0,
            text: value.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ints(values: &[i32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    fn name(text: &str, width: usize) -> Vec<u8> {
        let mut field = text.as_bytes().to_vec();
        field.resize(width, 0);
        field
    }

    /// A square of 2 ft, textured, then the same square untextured in a colour.
    fn square() -> Vec<u8> {
        let full = 255 << 16;
        let mut bytes = ints(&[MAGNIFY, 65536, VERTEX_LIST, 0, 4]);
        bytes.extend(ints(&[
            -256, 0, -256, 256, 0, -256, 256, 0, 256, -256, 512, 256,
        ]));
        bytes.extend(ints(&[TEXTURE, 0]));
        bytes.extend(name("GRASS.RAW", 16));
        bytes.extend(ints(&[17, 4, 0, 65535, 0, 99]));
        bytes.extend(ints(&[0, 0, 0, 1, full, 0, 2, full, full, 3, 0, full / 2]));
        bytes.extend(ints(&[COLOR, 0x00ff_8040, 25, 3, 0, 65535, 0, 99, 0, 1, 2]));
        bytes.extend(ints(&[END]));
        bytes
    }

    #[test]
    fn reads_vertices_in_feet_and_faces_with_their_textures() {
        let model = Model::parse(&square()).unwrap();
        assert_eq!(model.incomplete, None);
        assert_eq!(model.vertices[0], [-1.0, 0.0, -1.0]);
        assert_eq!(model.vertices[3], [-1.0, 2.0, 1.0]);
        assert_eq!(model.bounds(), ([-1.0, 0.0, -1.0], [1.0, 2.0, 1.0]));
        assert_eq!(model.textures(), ["GRASS.RAW"]);

        let textured = &model.faces[0];
        assert!(textured.is_cutout());
        assert_eq!(textured.texture.as_deref(), Some("GRASS.RAW"));
        assert_eq!(textured.corners.len(), 4);
        assert_eq!(
            textured.corners[2],
            Corner {
                vertex: 2,
                uv: [1.0, 1.0]
            }
        );
        assert_eq!(textured.corners[3].uv, [0.0, 0.5]);

        let flat = &model.faces[1];
        assert_eq!(
            (flat.kind, flat.texture.as_ref(), flat.color),
            (25, None, 0x00ff_8040)
        );
        assert_eq!(flat.corners.len(), 3);
    }

    #[test]
    fn reads_a_normal_for_each_vertex_and_the_faces_after_them() {
        let mut bytes = ints(&[MAGNIFY, 65536, VERTEX_LIST, 0, 3]);
        bytes.extend(ints(&[0, 0, 0, 256, 0, 0, 0, 256, 0]));
        bytes.extend(ints(&[NORMAL_LIST, 0, 3]));
        bytes.extend(ints(&[0, 0, 65535, 0, -65535, 0, 0, 0, 0]));
        bytes.extend(ints(&[25, 3, 0, 0, 65535, 99, 0, 1, 2, END]));
        let model = Model::parse(&bytes).unwrap();
        assert_eq!(model.incomplete, None);
        assert_eq!(
            model.vertex_normals,
            [[0.0, 0.0, 1.0], [0.0, -1.0, 0.0], [0.0, 0.0, 0.0]]
        );
        assert_eq!(model.faces.len(), 1);

        // A list of the wrong length can't belong to the vertices, and is left out.
        let mut short = ints(&[MAGNIFY, 65536, VERTEX_LIST, 0, 1, 0, 0, 0]);
        short.extend(ints(&[NORMAL_LIST, 0, 2, 0, 0, 65535, 0, 0, 65535, END]));
        let model = Model::parse(&short).unwrap();
        assert_eq!(model.incomplete, None);
        assert!(model.vertex_normals.is_empty());
    }

    /// Laid out as in Baja Beach's models: a material record, then faces of type 64 that
    /// take it. A face of another type ignores it.
    #[test]
    fn faces_of_type_64_take_the_material_in_force() {
        let mut bytes = ints(&[MAGNIFY, 65536, VERTEX_LIST, 0, 3]);
        bytes.extend(ints(&[0, 0, 0, 256, 0, 0, 0, 256, 0]));
        bytes.extend(ints(&[TEXTURE_64, 0]));
        bytes.extend(name("PALMFAN.RAW", 64));
        bytes.extend(ints(&[
            MATERIAL, 0x8f, 0, 0, 0, 65536, 0, 0, 65536, 65536, 65536, 0,
        ]));
        bytes.extend(ints(&[64, 3, 0, 65535, 0, 99, 0, 0, 0, 1, 0, 0, 2, 0, 0]));
        bytes.extend(ints(&[24, 3, 0, 65535, 0, 99, 0, 0, 0, 1, 0, 0, 2, 0, 0]));
        bytes.extend(ints(&[END]));
        let model = Model::parse(&bytes).unwrap();
        assert_eq!(model.incomplete, None);

        let leaf = &model.faces[0];
        assert_eq!(leaf.texture.as_deref(), Some("PALMFAN.RAW"));
        assert_eq!(
            leaf.material,
            Some(Material {
                flags: 0x8f,
                base_alpha: 1.0
            })
        );
        // Blend and alpha test both: the alpha test wins.
        assert!(leaf.is_cutout() && leaf.is_two_sided());
        assert_eq!(leaf.blend_alpha(), None);

        let plain = &model.faces[1];
        assert_eq!(plain.material, None);
        assert!(!plain.is_cutout() && !plain.is_two_sided());
    }

    /// The GMC's glass: flags 0x167, base alpha 0.35.
    #[test]
    fn a_blended_material_says_how_opaque_its_faces_are() {
        let mut bytes = ints(&[MAGNIFY, 65536, VERTEX_LIST, 0, 3]);
        bytes.extend(ints(&[0, 0, 0, 256, 0, 0, 0, 256, 0]));
        bytes.extend(ints(&[TEXTURE, 0]));
        bytes.extend(name("GLASS.RAW", 16));
        let alpha = (0.35 * FIXED_ONE) as i32;
        bytes.extend(ints(&[
            MATERIAL, 0x167, 0, 0, 0, alpha, 0, 0, 65536, 65536, 65536, 0,
        ]));
        bytes.extend(ints(&[
            64, 3, 0, 65535, 0, 99, 0, 0, 0, 1, 0, 0, 2, 0, 0, END,
        ]));
        let model = Model::parse(&bytes).unwrap();
        let glass = &model.faces[0];
        assert!(!glass.is_cutout());
        assert!((glass.blend_alpha().unwrap() - 0.35).abs() < 1e-4);
    }

    /// A triangle of three vertices, as the prefix of a model.
    fn triangle() -> Vec<u8> {
        ints(&[
            MAGNIFY,
            65536,
            VERTEX_LIST,
            0,
            3,
            0,
            0,
            0,
            256,
            0,
            0,
            0,
            256,
            0,
        ])
    }

    /// A textured face (type 24) on the triangle.
    fn face() -> Vec<u8> {
        ints(&[24, 3, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 2, 0, 0])
    }

    /// Laid out as `88WRNSN1.BIN` in Alpine: two frames at 0xaaaa, two-thirds of a second
    /// each. Some files leave rubbish after the NUL that ends a name.
    #[test]
    fn animated_textures_give_their_frames_and_how_long_each_is_shown() {
        let mut bytes = triangle();
        bytes.extend(ints(&[TEXTURE_CYCLE, 0, 2, 0, 0xaaaa, 0, 0]));
        let mut rubbish = name("LIGHT1.RAW", 32);
        rubbish[20..24].copy_from_slice(&[0x1c, 0x3b, 0x88, 0x00]);
        bytes.extend(rubbish);
        bytes.extend(name("LIGHT2.RAW", 32));
        bytes.extend(face());
        bytes.extend(ints(&[TEXTURE, 0]));
        bytes.extend(name("PLAIN.RAW", 16));
        bytes.extend(face());
        bytes.extend(ints(&[END]));
        let model = Model::parse(&bytes).unwrap();
        assert_eq!(model.incomplete, None);

        let lit = &model.faces[0];
        assert_eq!(lit.texture.as_deref(), Some("LIGHT1.RAW"));
        assert_eq!(lit.texture_cycle, Some(0));
        assert!(!lit.is_cutout());
        let cycle = &model.texture_cycles[0];
        assert_eq!(cycle.frames, ["LIGHT1.RAW", "LIGHT2.RAW"]);
        assert!((cycle.seconds_per_frame - 2.0 / 3.0).abs() < 1e-4);

        // A plain texture ends the cycle.
        assert_eq!(model.faces[1].texture_cycle, None);
        assert_eq!(model.textures(), ["LIGHT1.RAW", "LIGHT2.RAW", "PLAIN.RAW"]);
    }

    /// Laid out as Alpine's helicopter: a Jump after the vertices and an Order before the
    /// end, stepped over by their lengths. The faces between are all read, once.
    #[test]
    fn order_and_jump_records_are_stepped_over() {
        let mut bytes = triangle();
        bytes.extend(ints(&[JUMP, 19148]));
        bytes.extend(ints(&[TEXTURE, 0]));
        bytes.extend(name("BODY.RAW", 16));
        bytes.extend(face());
        bytes.extend(ints(&[JUMP, 1192, TEXTURE_CYCLE, 0, 2, 0, 1024, 0, 0]));
        bytes.extend(name("ROTOR1.RAW", 32));
        bytes.extend(name("ROTOR2.RAW", 32));
        bytes.extend(face());
        bytes.extend(ints(&[
            JUMP, 36, ORDER, 0, 65535, 0, 61_244_529, -1156, -19140, END,
        ]));
        let model = Model::parse(&bytes).unwrap();
        assert_eq!(model.incomplete, None);
        let textures: Vec<_> = model
            .faces
            .iter()
            .map(|face| face.texture.as_deref())
            .collect();
        assert_eq!(textures, [Some("BODY.RAW"), Some("ROTOR1.RAW")]);
        assert_eq!(model.faces[1].texture_cycle, Some(0));
    }

    /// Laid out as `PUMPJACK.BIN` in the base game's Outback archive: 348 bytes, eight
    /// frames, rate 32768.
    fn pump_jack() -> Vec<u8> {
        let mut bytes = ints(&[ANIMATION_CONTROL, 0, 8, 32768, 0, 0]);
        for frame in 0..MAX_KEYFRAMES {
            let text = if frame < 8 {
                format!("pj{frame}.bin")
            } else {
                String::new()
            };
            bytes.extend(name(&text, KEYFRAME_NAME_WIDTH));
        }
        // Zeros to the end of the 344-byte record, then the end record.
        bytes.extend(ints(&[END]));
        bytes
    }

    #[test]
    fn an_animation_control_file_names_its_frames() {
        let bytes = pump_jack();
        assert_eq!(bytes.len(), 348);
        assert!(KeyframeAnimation::is_in(&bytes));
        assert!(!KeyframeAnimation::is_in(&square()));
        let animation = KeyframeAnimation::parse(&bytes).unwrap();
        assert_eq!(animation.frames.len(), 8);
        assert_eq!(animation.frames[0], "pj0.bin");
        assert_eq!(animation.frames[7], "pj7.bin");
        assert_eq!(animation.seconds_per_frame, 0.5);
    }

    #[test]
    fn a_bad_animation_control_file_is_an_error_not_a_panic() {
        let good = pump_jack();
        for length in 0..good.len() {
            let _ = KeyframeAnimation::parse(&good[..length]);
        }
        let with = |index: usize, value: i32| {
            let mut bytes = good.clone();
            bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
            KeyframeAnimation::parse(&bytes)
        };
        for count in [0, MAX_KEYFRAMES as i32 + 1, -1] {
            assert!(with(2, count).is_err(), "{count} frames");
        }
        assert!(with(3, 0).is_err());
        // A frame without a name.
        assert!(with(2, 9).is_err());
        assert!(KeyframeAnimation::parse(&square()).is_err());
    }

    #[test]
    fn a_smaller_magnification_makes_a_bigger_model() {
        let bytes = ints(&[MAGNIFY, 32768, VERTEX_LIST, 0, 1, 256, 512, 0, END]);
        assert_eq!(Model::parse(&bytes).unwrap().vertices, [[2.0, 4.0, 0.0]]);
    }

    #[test]
    fn stops_at_an_unknown_record_and_says_so() {
        let mut bytes = square();
        bytes.truncate(bytes.len() - 4);
        bytes.extend(ints(&[9999, 1, 2, 3]));
        let model = Model::parse(&bytes).unwrap();
        assert_eq!(model.faces.len(), 2);
        assert!(model.incomplete.unwrap().contains("9999"));
    }

    #[test]
    fn bad_input_is_an_error_not_a_panic() {
        let good = square();
        for length in 0..good.len() - 4 {
            // Any prefix either fails or is read as incomplete; it must not panic.
            if let Ok(model) = Model::parse(&good[..length]) {
                assert!(model.incomplete.is_some(), "at {length}");
            }
        }

        let bad_vertex = ints(&[
            MAGNIFY,
            65536,
            VERTEX_LIST,
            0,
            1,
            0,
            0,
            0,
            25,
            3,
            0,
            0,
            0,
            0,
            0,
            0,
            7,
            END,
        ]);
        assert!(matches!(
            Model::parse(&bad_vertex),
            Err(PodError::BadValue { .. })
        ));
        let huge_count = ints(&[MAGNIFY, 65536, VERTEX_LIST, 0, i32::MAX]);
        assert!(matches!(
            Model::parse(&huge_count),
            Err(PodError::BadValue { .. })
        ));
        let animation = ints(&[ANIMATION_CONTROL, 0, 2]);
        assert!(matches!(
            Model::parse(&animation),
            Err(PodError::Unsupported(_))
        ));
    }
}
