//! Turns the models and objects of a Monster Truck Madness 2 track into `Scenery`, and
//! the models of its backdrop into a `Backdrop`.
//!
//! The conventions, and how each was measured, are in `docs/formats/model.md`. Briefly: a
//! model's vertices are (X, up, Z) in feet in MTM2's left-handed axes, so Z is flipped
//! here like everything else. Flipping an axis turns every face inside out, so the
//! corners of each face are taken in the opposite order.
//!
//! An animated texture becomes a run of consecutive tiles, one for each frame, which the
//! scenery's material steps through. A model that moves by keyframes becomes the mesh of
//! its first frame, with where every other frame puts each of the mesh's vertices. The
//! backdrop's textures stay on their first frame.

use std::collections::{BTreeSet, HashMap, HashSet};

use bevy::prelude::*;

use crate::hd_texture;
use crate::mipmaps::mip_chain;
use crate::pod::{self, BoxShape, box_type};

use super::{
    Backdrop, KeyframedFace, Keyframes, MAX_TEXTURE_CYCLES, Scenery, SceneryModel, SceneryMotion,
    SceneryObject, TextureCycle,
};

const METRES_PER_FOOT: f32 = 0.3048;

/// A box's mass is written in slugs (see `docs/formats/situation.md`).
const KILOGRAMS_PER_SLUG: f32 = 14.593_903;

/// Box types that trucks drive straight through: checkpoints, and two that MTM2's
/// editors call "drive through" and "always face the camera".
const NOT_SOLID: [i32; 3] = [box_type::CHECKPOINT, 7, FACES_CAMERA];

/// The box type that "always faces the camera" (see `docs/formats/situation.md`).
const FACES_CAMERA: i32 = 8;

/// Checkpoints whose model is the editor's arrow-covered trigger box are invisible in the
/// game. Other checkpoint models, such as banners over the road, are there to be seen.
const INVISIBLE_MODEL_PREFIX: &str = "CKBOX";

/// Stands in for a texture that isn't in the archive.
const MISSING_TEXTURE_GREY: [u8; 4] = [128, 128, 128, 255];

/// The size of a tile when the archive holds no model texture to take it from. Every tile
/// is then one colour, so any size draws the same; a small one costs little memory.
const TILE_SIZE_WITHOUT_TEXTURES: usize = 8;

/// `to_ground` takes a position in MTM2 feet to the game's ground plane.
pub(super) fn scenery_from_pod(
    track: &pod::Track,
    to_ground: impl Fn([f32; 3]) -> Vec2,
) -> Scenery {
    let mut tiles = Tiles {
        track,
        tile_size: tile_size(track, track.models.values()),
        pixels: Vec::new(),
        known: HashMap::new(),
        cycles: Some(Vec::new()),
        known_cycles: HashMap::new(),
    };
    let mut models = Vec::new();
    let mut model_numbers = HashMap::new();
    let mut objects = Vec::new();

    for situation_box in &track.situation.boxes {
        let BoxShape::Model(name) = &situation_box.shape else {
            continue;
        };
        let name = name.to_ascii_uppercase();
        let Some(model) = track.models.get(&name) else {
            continue;
        };
        let is_checkpoint = situation_box.kind == box_type::CHECKPOINT;
        if is_checkpoint && name.starts_with(INVISIBLE_MODEL_PREFIX) {
            continue;
        }

        let number = *model_numbers.entry(name.clone()).or_insert_with(|| {
            let animated = track.animated_models.get(&name);
            models.push(convert_model(&name, model, animated, &mut tiles));
            models.len() - 1
        });
        let [x, height, z] = situation_box.position;
        let solid = !NOT_SOLID.contains(&situation_box.kind);
        objects.push(SceneryObject {
            model: number,
            position: to_ground(situation_box.position),
            height_above_ground: (height - track.heightmap.ground_feet(x, z)) * METRES_PER_FOOT,
            yaw: -situation_box.angles[2],
            solid,
            motion: motion(situation_box, solid),
            faces_camera: situation_box.kind == FACES_CAMERA,
        });
    }

    Scenery {
        tile_size: tiles.tile_size,
        texture_cycles: tiles.cycles.unwrap_or_default(),
        tiles: tiles.pixels,
        models,
        objects,
    }
}

/// A moving box goes along its velocity, and a solid box with a mass can be knocked about.
/// Anything else stays put: "0.000000 mass means unmoveable" (the Traxx editor's notes).
fn motion(situation_box: &pod::SituationBox, solid: bool) -> SceneryMotion {
    let [x, up, z] = situation_box.velocity;
    // Z is flipped, as for positions.
    let velocity = Vec3::new(x, up, -z) * METRES_PER_FOOT;
    if situation_box.kind == box_type::MOVING && velocity != Vec3::ZERO {
        SceneryMotion::Moving { velocity }
    } else if solid && situation_box.mass > 0.0 {
        SceneryMotion::Loose {
            mass: situation_box.mass * KILOGRAMS_PER_SLUG,
        }
    } else {
        SceneryMotion::Fixed
    }
}

/// The track's backdrop, in its own tiles: its textures are larger than the scenery's
/// (256 pixels a side, where the scenery's are 64), and would lose their detail made to fit
/// the scenery's.
pub(super) fn backdrop_from_pod(track: &pod::Track) -> Option<Backdrop> {
    if track.backdrop_models.is_empty() {
        return None;
    }
    let mut tiles = Tiles {
        track,
        tile_size: tile_size(track, track.backdrop_models.values()),
        pixels: Vec::new(),
        known: HashMap::new(),
        cycles: None,
        known_cycles: HashMap::new(),
    };
    // In the track's order, which says which is drawn over which. A name can come twice.
    let mut models = Vec::new();
    let mut seen = HashSet::new();
    for name in &track.situation.backdrops {
        let name = name.to_ascii_uppercase();
        if let Some(model) = track.backdrop_models.get(&name)
            && seen.insert(name.clone())
        {
            models.push(convert_model(&name, model, None, &mut tiles));
        }
    }
    Some(Backdrop {
        tile_size: tiles.tile_size,
        tiles: tiles.pixels,
        models,
    })
}

fn convert_model(
    name: &str,
    model: &pod::Model,
    animated: Option<&pod::AnimatedModel>,
    tiles: &mut Tiles,
) -> SceneryModel {
    let mut mesh = SceneryModel {
        name: name.to_string(),
        ..default()
    };
    // For each vertex of the mesh, the vertex of the model it is.
    let mut sources = Vec::new();
    let mut faces = Vec::new();
    let written = written_faces(model);
    for face in &model.faces {
        let cycle = face
            .texture_cycle
            .and_then(|index| model.texture_cycles.get(index))
            .and_then(|cycle| tiles.cycle(cycle, face.is_cutout()));
        let (tile, cycle) = match (cycle, &face.texture) {
            (Some(cycle), _) => cycle,
            (None, Some(texture)) => (tiles.textured(texture, face.is_cutout()), 0),
            (None, None) => (tiles.flat(face.color), 0),
        };

        // Reversed, because flipping Z turns the face inside out.
        let reversed: Vec<&pod::Corner> = face.corners.iter().rev().collect();
        let corners: Vec<(Vec3, [f32; 2])> = reversed
            .iter()
            .map(|corner| {
                // A few files hold rubbish for texture coordinates; keep it on the tile.
                let uv = corner.uv.map(|along| {
                    if along.is_finite() {
                        along.clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                });
                (to_game(model.vertices[corner.vertex]), uv)
            })
            .collect();
        let mut add = |corners: &[(Vec3, [f32; 2])], vertices: Vec<usize>| {
            faces.push(KeyframedFace {
                first: mesh.positions.len() as u32,
                corners: corners.len() as u32,
            });
            sources.extend(vertices);
            add_face(&mut mesh, corners, tile, cycle);
        };
        let vertices: Vec<usize> = reversed.iter().map(|corner| corner.vertex).collect();
        add(&corners, vertices.clone());
        // The back is drawn as a face of its own, since the material culls back faces:
        // unless the model has its back already, as a face written the other way round.
        // Drawn twice, the two backs fought for the same pixels, and flickered.
        if face.is_two_sided() && !written.contains(&corners_of(model, face.corners.iter().rev())) {
            let back: Vec<_> = corners.into_iter().rev().collect();
            add(&back, vertices.into_iter().rev().collect());
        }
    }
    mesh.keyframes = animated
        .filter(|animated| animated.frames.len() > 1 && animated.animation.seconds_per_frame > 0.0)
        .map(|animated| Keyframes {
            seconds_per_frame: animated.animation.seconds_per_frame,
            frames: animated
                .frames
                .iter()
                .map(|frame| {
                    sources
                        .iter()
                        .map(|&vertex| to_game(frame[vertex]).to_array())
                        .collect()
                })
                .collect(),
            faces,
        });
    mesh
}

/// A position in a model's own axes, in feet, in the game's axes and metres.
fn to_game([x, up, z]: [f32; 3]) -> Vec3 {
    Vec3::new(x, up, -z) * METRES_PER_FOOT
}

/// Every face of the model, as `corners_of` gives it.
fn written_faces(model: &pod::Model) -> HashSet<Vec<[u32; 3]>> {
    model
        .faces
        .iter()
        .map(|face| corners_of(model, face.corners.iter()))
        .collect()
}

/// Where a face's corners are, in the order given, starting from the lowest, so that a
/// face compares equal to itself however its corners are numbered and wherever its list
/// of them starts. By position rather than by vertex, since a file can hold one point
/// more than once.
fn corners_of<'a>(
    model: &pod::Model,
    corners: impl Iterator<Item = &'a pod::Corner>,
) -> Vec<[u32; 3]> {
    let points: Vec<[u32; 3]> = corners
        .map(|corner| model.vertices[corner.vertex].map(f32::to_bits))
        .collect();
    let first = (0..points.len())
        .min_by_key(|&index| points[index])
        .unwrap_or(0);
    (0..points.len())
        .map(|step| points[(first + step) % points.len()])
        .collect()
}

/// One face, seen from the side its corners wind anticlockwise round. `cycle` is the
/// animated texture it shows, counting from 1, or 0 for a still one.
fn add_face(mesh: &mut SceneryModel, corners: &[(Vec3, [f32; 2])], tile: u32, cycle: u32) {
    // Summed over every edge (Newell's method) rather than taken from the first three
    // corners, which in some faces lie in a line.
    let normal = (0..corners.len())
        .map(|index| {
            let (here, next) = (corners[index].0, corners[(index + 1) % corners.len()].0);
            here.cross(next)
        })
        .sum::<Vec3>()
        .normalize_or_zero();

    let first = mesh.positions.len() as u32;
    for (position, uv) in corners {
        mesh.positions.push(position.to_array());
        mesh.normals.push(normal.to_array());
        mesh.uvs.push(*uv);
        mesh.tiles.push(tile);
        mesh.texture_cycles.push(cycle);
    }
    // A fan from the first corner. The game's own faces are flat and convex.
    for second in 1..corners.len() as u32 - 1 {
        mesh.indices
            .extend([first, first + second, first + second + 1]);
    }
}

/// The size most of the textures of `models` have, which the others are made to fit: the
/// larger, where two sizes are as common. A texture's PNG counts in place of its .RAW.
fn tile_size<'a>(track: &pod::Track, models: impl Iterator<Item = &'a pod::Model>) -> usize {
    let mut count_of_size = HashMap::new();
    let names = models
        .flat_map(pod::Model::textures)
        .map(str::to_ascii_uppercase)
        .filter(|name| {
            track.model_textures.contains_key(name) || track.model_hd_textures.contains_key(name)
        })
        .collect::<BTreeSet<_>>();
    for name in &names {
        let size = match track.model_hd_textures.get(name) {
            Some(hd) => hd.size(),
            None => track.model_textures[name].size(),
        };
        *count_of_size.entry(size).or_insert(0) += 1;
    }
    count_of_size
        .into_iter()
        .max_by_key(|&(size, count)| (count, size))
        .map_or(TILE_SIZE_WITHOUT_TEXTURES, |(size, _)| size)
}

/// Whether a texture of one size can be made to fit a tile of another: only powers of two
/// scale evenly.
fn fits(size: usize, tile_size: usize) -> bool {
    size == tile_size || (size.is_power_of_two() && tile_size.is_power_of_two())
}

/// The tiles made so far, and how to make more.
struct Tiles<'a> {
    track: &'a pod::Track,
    tile_size: usize,
    pixels: Vec<Vec<u8>>,
    known: HashMap<TileKey, u32>,
    /// The animated textures made so far, or `None` where they are shown still.
    cycles: Option<Vec<TextureCycle>>,
    /// What each is, by its frames' names, whether it is a cutout, and the bits of how
    /// long each frame is shown, as a float can't be a key: its number, from 1.
    known_cycles: HashMap<(Vec<String>, bool, u32), u32>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum TileKey {
    Texture { name: String, cutout: bool },
    Flat(u32),
}

impl Tiles<'_> {
    /// The tile for a texture. A texture used both plainly and with its black cut out
    /// becomes two tiles.
    fn textured(&mut self, name: &str, cutout: bool) -> u32 {
        let name = name.to_ascii_uppercase();
        let key = TileKey::Texture {
            name: name.clone(),
            cutout,
        };
        self.tile(key, |tiles| tiles.texture_pixels(&name, cutout))
    }

    /// The first tile of an animated texture, and its number, from 1: its frames are that
    /// tile and the ones after it. `None` where it is shown still, as its first frame: in
    /// the backdrop, for a cycle of one frame or of no time, or past the most one material
    /// can show.
    fn cycle(&mut self, cycle: &pod::TextureCycle, cutout: bool) -> Option<(u32, u32)> {
        let frames: Vec<String> = cycle
            .frames
            .iter()
            .map(|name| name.to_ascii_uppercase())
            .collect();
        let key = (frames.clone(), cutout, cycle.seconds_per_frame.to_bits());
        if let Some(&number) = self.known_cycles.get(&key) {
            let made = self.cycles.as_ref()?;
            return Some((made[number as usize - 1].first_tile, number));
        }
        let made = self.cycles.as_ref()?;
        if frames.len() < 2
            || cycle.seconds_per_frame.is_nan()
            || cycle.seconds_per_frame <= 0.0
            || made.len() >= MAX_TEXTURE_CYCLES
        {
            return None;
        }
        let first_tile = self.pixels.len() as u32;
        for name in &frames {
            let pixels = self.texture_pixels(name, cutout);
            self.pixels.push(pixels);
        }
        let made = self.cycles.as_mut()?;
        made.push(TextureCycle {
            first_tile,
            frames: frames.len() as u32,
            seconds_per_frame: cycle.seconds_per_frame,
        });
        let number = made.len() as u32;
        self.known_cycles.insert(key, number);
        Some((first_tile, number))
    }

    /// A texture's pixels, made to fit a tile, or grey if there is no such texture.
    fn texture_pixels(&self, name: &str, cutout: bool) -> Vec<u8> {
        self.image(name, cutout)
            .filter(|&(size, _)| fits(size, self.tile_size))
            .map(|(size, rgba)| fit(rgba, size, self.tile_size))
            .unwrap_or_else(|| MISSING_TEXTURE_GREY.repeat(self.tile_size * self.tile_size))
    }

    /// A texture's size and pixels: from its PNG where the archive has one, as the
    /// patched game does, and otherwise from its .RAW and its own palette, or the track's
    /// where it has none.
    fn image(&self, name: &str, cutout: bool) -> Option<(usize, Vec<u8>)> {
        let track = self.track;
        if let Some(hd) = track.model_hd_textures.get(name)
            && let Some(mut rgba) = hd_texture::decode(hd)
        {
            // A PNG brings its own alpha, which a cutout face keeps. **Reference:**
            // JSTrackViewer tests a cutout face's texture against half alpha, and draws
            // every other face opaque.
            if !cutout {
                for pixel in rgba.as_chunks_mut::<4>().0 {
                    pixel[3] = 255;
                }
            }
            return Some((hd.size(), rgba));
        }
        let texture = track.model_textures.get(name)?;
        let own_palette = track.own_palettes.get(name);
        let mut rgba = texture.to_rgba(own_palette.or(track.palette.as_ref())?);
        if cutout {
            // MTM2 has no alpha. On a cutout face, pure black is a hole. It stays
            // black so that smooth filtering doesn't draw a bright fringe round it.
            for pixel in rgba.as_chunks_mut::<4>().0 {
                if pixel[..3] == [0, 0, 0] {
                    pixel[3] = 0;
                }
            }
        }
        Some((texture.size(), rgba))
    }

    /// A tile of one colour, for an untextured face. The colour is a Windows COLORREF:
    /// red in the lowest byte.
    fn flat(&mut self, color: u32) -> u32 {
        let [red, green, blue, _] = color.to_le_bytes();
        self.tile(TileKey::Flat(color & 0x00ff_ffff), |tiles| {
            [red, green, blue, 255].repeat(tiles.tile_size * tiles.tile_size)
        })
    }

    fn tile(&mut self, key: TileKey, pixels: impl FnOnce(&Self) -> Vec<u8>) -> u32 {
        if let Some(&tile) = self.known.get(&key) {
            return tile;
        }
        let made = pixels(self);
        self.pixels.push(made);
        let tile = self.pixels.len() as u32 - 1;
        self.known.insert(key, tile);
        tile
    }
}

/// A square RGBA tile of `from` pixels a side, made `to` pixels a side. Both must be
/// powers of two. Made smaller by the mipmaps' averaging, in linear light; made larger by
/// repeating each pixel, which adds nothing and takes nothing away.
fn fit(rgba: Vec<u8>, from: usize, to: usize) -> Vec<u8> {
    if from == to {
        return rgba;
    }
    if from > to {
        return mip_chain(&rgba, from)
            .into_iter()
            .nth((from / to).trailing_zeros() as usize)
            .expect("a power of two halves down to any smaller one");
    }
    let factor = to / from;
    let mut larger = Vec::with_capacity(to * to * 4);
    for y in 0..to {
        for x in 0..to {
            let pixel = ((y / factor) * from + x / factor) * 4;
            larger.extend_from_slice(&rgba[pixel..pixel + 4]);
        }
    }
    larger
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_a_tile_to_a_larger_or_smaller_size() {
        // 2 x 2: red, green / blue, white.
        let tile = [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255; 4],
        ]
        .concat();
        let larger = fit(tile.clone(), 2, 4);
        assert_eq!(larger.len(), 4 * 4 * 4);
        assert_eq!(&larger[..8], [255, 0, 0, 255, 255, 0, 0, 255]);
        assert_eq!(&larger[(3 * 4 + 3) * 4..], [255; 4]);

        let smaller = fit(tile.clone(), 2, 1);
        assert_eq!(smaller, mip_chain(&tile, 2)[1]);
        assert_eq!(fit(tile.clone(), 2, 2), tile);
    }

    fn square(corners: [usize; 4]) -> pod::Face {
        pod::Face {
            kind: pod::face_type::MATERIAL,
            texture: None,
            texture_cycle: None,
            color: 0,
            material: Some(pod::Material {
                flags: pod::material_flag::TWO_SIDED,
                base_alpha: 1.0,
            }),
            corners: corners
                .into_iter()
                .map(|vertex| pod::Corner {
                    vertex,
                    uv: [0.0; 2],
                })
                .collect(),
        }
    }

    fn model(faces: Vec<pod::Face>) -> pod::Model {
        pod::Model {
            vertices: vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            vertex_normals: Vec::new(),
            faces,
            texture_cycles: Vec::new(),
            incomplete: None,
        }
    }

    /// Baja Beach's trees, huts and ship write both sides of each leaf and plank, and
    /// call both two-sided.
    #[test]
    fn a_face_whose_back_is_written_is_not_given_another() {
        let both = model(vec![square([0, 1, 2, 3]), square([2, 1, 0, 3])]);
        let written = written_faces(&both);
        for face in &both.faces {
            assert!(written.contains(&corners_of(&both, face.corners.iter().rev())));
        }
        // A lone two-sided face has no back of its own.
        let lone = model(vec![square([0, 1, 2, 3])]);
        let face = &lone.faces[0];
        assert!(!written_faces(&lone).contains(&corners_of(&lone, face.corners.iter().rev())));
    }

    #[test]
    fn a_face_is_the_same_wherever_its_corners_start() {
        let one = model(vec![square([0, 1, 2, 3]), square([2, 3, 0, 1])]);
        let [a, b] = [0, 1].map(|index| corners_of(&one, one.faces[index].corners.iter()));
        assert_eq!(a, b);
        // But not turned round.
        assert_ne!(a, corners_of(&one, one.faces[0].corners.iter().rev()));
    }
}
