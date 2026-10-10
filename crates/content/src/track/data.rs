//! A track as plain data. Everything the game knows about a track comes from a
//! `TrackData`, whether it was generated in code or loaded from an archive.
//!
//! Only plain types live here: no entities, asset handles or resources.

use bevy::prelude::*;

use super::{Course, HeightGrid};

#[derive(Clone, Debug)]
pub struct TrackData {
    pub name: String,
    /// The ground. Where it repeats (`HeightGrid::repeats`), as a Monster Truck Madness 2
    /// world does, so does everything on it, and the way from one place to another is
    /// `HeightGrid::offset`.
    pub heights: HeightGrid,
    /// How much of the ground at each vertex of `heights` is course rather than open
    /// country, from 0 to 1, in the same row-by-row order. It only tints the terrain,
    /// and will give way to real ground textures.
    pub surface: Vec<f32>,
    /// Textures for the ground, where the track has them. Without them the terrain is
    /// shaded by height and slope, and tinted by `surface`.
    pub ground: Option<GroundTextures>,
    /// The things standing around the track. Empty for a track that has none.
    pub scenery: Scenery,
    /// What is drawn round the horizon, behind everything else. `None` for a track that
    /// has none.
    pub backdrop: Option<Backdrop>,
    /// The pictures of the sky, behind the backdrop. None at all for the built-in track.
    pub skies: Skies,
    /// What the ground is underfoot in each ground cell: a square of cells covering the
    /// track, row by row like the height grid's vertices, and as many as there are
    /// `GroundTextures::cells`. Empty for a track that doesn't say, where all of the
    /// ground is `Footing::Unnamed`; one cell for a track whose ground is all one. Ask
    /// `footing_at`.
    pub footing: Vec<Footing>,
    /// Solid blocks on the ground's grid: bridges, the roofs of tunnels, walls. Part of
    /// the ground, and like it they don't move when the ground is rounded off. Empty for
    /// a track that has none.
    pub ground_boxes: Vec<GroundBox>,
    /// The height of the surface of the water that fills the ground's hollows, in metres.
    /// One level for the whole track, which a course may run under. `None` for a dry
    /// track.
    pub water_level: Option<f32>,
    /// The route around the track, where one is known.
    pub course: Option<Course>,
    /// Other routes around the track, in the track's own order, each `None` where the
    /// track leaves it out. A Monster Truck Madness 2 track has four: `[Course 1]` to
    /// `[Course 4]`, which its computer trucks follow, and of which the second is often
    /// the shortest, by shortcuts (situation.md, "Extra courses"). The corners that a
    /// track leaves out between their straight pieces are put back as curves, which keep
    /// to the middle of the road. Empty for a track that has no others.
    pub other_courses: Vec<Option<Course>>,
    /// Checkpoints in the order they must be crossed. Gate 0 is the start/finish line.
    pub gates: Vec<Gate>,
    /// Pole position: where the first truck waits for the race to begin.
    pub start: StartPosition,
    /// The other places on the starting grid, in the track's own order. A Monster Truck
    /// Madness 2 track has seven, and not always behind pole position.
    pub grid: Vec<StartPosition>,
}

/// How far behind the last place of the grid each place that the track doesn't have is
/// put, in metres: a truck's length and room to get going.
const EXTRA_PLACE_SPACING: f32 = 8.0;

impl TrackData {
    /// Place `index` of the starting grid, where 0 is pole position. A race with more
    /// trucks than the track has places for lines the rest up behind the last place.
    pub fn grid_place(&self, index: usize) -> StartPosition {
        let places = || std::iter::once(&self.start).chain(&self.grid);
        if let Some(place) = places().nth(index) {
            return place.clone();
        }
        let last = places().last().expect("there is always pole position");
        let behind = (index - self.grid.len()) as f32 * EXTRA_PLACE_SPACING;
        StartPosition {
            position: last.position - yaw_direction(last.yaw) * behind,
            yaw: last.yaw,
        }
    }

    /// What the ground is underfoot at `x`, `z` (world X and Z, in metres).
    pub fn footing_at(&self, x: f32, z: f32) -> Footing {
        let cells = self.footing.len().isqrt();
        if cells == 0 {
            return Footing::Unnamed;
        }
        let Vec2 { x, y: z } = self.heights.onto(Vec2::new(x, z));
        let cell = |world: f32| {
            let across = (world / self.heights.size() + 0.5).clamp(0.0, 1.0);
            ((across * cells as f32) as usize).min(cells - 1)
        };
        self.footing[cell(z) * cells + cell(x)]
    }

    /// Whether tires throw up the ground at `x`, `z` (world X and Z, in metres): loose
    /// ground, as the track's texture types say. Where the track doesn't name its ground,
    /// whatever looks loose (`looks_loose`), judged at that point of its texture, so that
    /// a texture that is half grass and half road is loose only on its grass half.
    pub fn loose_at(&self, x: f32, z: f32) -> bool {
        let Vec2 { x, y: z } = self.heights.onto(Vec2::new(x, z));
        match self.footing_at(x, z) {
            Footing::Loose => true,
            Footing::Firm | Footing::Ice => false,
            Footing::Unnamed => self
                .ground
                .as_ref()
                .and_then(|ground| ground.color_at(x, z, self.heights.size()))
                .is_some_and(looks_loose),
        }
    }
}

/// The least that a loose texel's red, green and blue differ, from 0 to 255: dirt, sand and
/// grass are coloured, and road, rock and snow grey. Measured on the ground that the
/// tracks' texture types don't name (`docs/formats/texture_types.md`): at 12, 97 to 100 %
/// of Baja Beach's sand, Snake River's gravel and every track's grass is loose, and 1 to
/// 10 % of the asphalt. Lower takes more asphalt for loose; higher, less of Lands
/// Between's grey-green ground (38 % at 12).
const LOOSE_COLORFULNESS: u8 = 12;

/// Whether a texel of ground of `color` (sRGB, 0 to 255, as on screen) looks loose: it is
/// coloured, and not blue, as water and ice are. The game's own rule: MTM2 says nothing of
/// ground its texture types don't name.
fn looks_loose([red, green, blue]: [u8; 3]) -> bool {
    let brightest = red.max(green).max(blue);
    let darkest = red.min(green).min(blue);
    brightest - darkest >= LOOSE_COLORFULNESS && blue < red.max(green)
}

/// What the ground is underfoot, as the track's texture types say: for how well tires grip
/// it, and whether they throw it up (`docs/formats/texture_types.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Footing {
    /// Dirt, mud, sand, grass and rocky ground. Tires grip it as well as they can, and
    /// throw it up.
    Loose,
    /// Road, water, rock, metal and railway track. Tires grip it as well as they can, and
    /// throw nothing up.
    Firm,
    /// Ice: tires slip on it.
    Ice,
    /// Ground the track doesn't name. Tires grip it as well as they can; whether they
    /// throw it up is judged by its colour (`TrackData::loose_at`).
    #[default]
    Unnamed,
}

/// Square textures painted on the ground, one per ground cell.
#[derive(Clone, Debug)]
pub struct GroundTextures {
    /// Pixels along each side of a tile.
    pub tile_size: usize,
    /// Each tile's pixels as red, green, blue and alpha bytes, row by row from the top.
    pub tiles: Vec<Vec<u8>>,
    /// A square of ground cells covering the track, row by row like the height grid's
    /// vertices. A ground cell is one cell of the height grid, or a whole number of them
    /// along each side if the height grid has been made finer.
    pub cells: Vec<GroundCell>,
}

impl GroundTextures {
    pub fn cells_per_side(&self) -> usize {
        self.cells.len().isqrt()
    }

    /// The colour of the texel drawn at `x`, `z` (world X and Z, in metres) on a track
    /// `size` metres across, in sRGB from 0 to 255. Found as the terrain's mesh finds it:
    /// the cell's corners give the texture coordinates, spread across the cell. `None`
    /// where there is no texture.
    pub fn color_at(&self, x: f32, z: f32, size: f32) -> Option<[u8; 3]> {
        let cells = self.cells_per_side();
        if cells == 0 {
            return None;
        }
        // Which cell, and how far across it.
        let place = |world: f32| {
            let across = (world / size + 0.5).clamp(0.0, 1.0) * cells as f32;
            let cell = (across as usize).min(cells - 1);
            (cell, (across - cell as f32).clamp(0.0, 1.0))
        };
        let ((col, u), (row, v)) = (place(x), place(z));
        let cell = self.cells.get(row * cells + col)?;
        let [lowest, plus_x, plus_z, far] = cell.corners.map(Vec2::from);
        let coords = lowest.lerp(plus_x, u).lerp(plus_z.lerp(far, u), v);
        let last = self.tile_size as f32 - 1.0;
        let texel = (coords * self.tile_size as f32)
            .floor()
            .clamp(Vec2::ZERO, Vec2::splat(last));
        let index = (texel.y as usize * self.tile_size + texel.x as usize) * 4;
        let rgba = self.tiles.get(cell.tile)?.get(index..index + 4)?;
        Some([rgba[0], rgba[1], rgba[2]])
    }
}

#[derive(Clone, Debug)]
pub struct GroundCell {
    /// Index into `GroundTextures::tiles`.
    pub tile: usize,
    /// Where in the tile each corner of the cell falls, as (across, down) from the
    /// tile's top left, from 0 to 1. The corners are in the order: lowest, +X, +Z, far.
    pub corners: [[f32; 2]; 4],
}

/// A solid block with its sides along X and Z, standing between two heights.
#[derive(Clone, Debug)]
pub struct GroundBox {
    /// The corner with the lowest X and Z, on the ground plane (world X and Z), in metres.
    pub min: Vec2,
    /// The corner with the highest X and Z.
    pub max: Vec2,
    /// Height of the bottom, in metres. It may be under the ground.
    pub bottom: f32,
    /// Height of the top, in metres.
    pub top: f32,
    /// A texture for each face, from `GroundTextures::tiles`. `None` where the ground has
    /// no textures, or the track doesn't say.
    pub faces: Option<BoxFaces>,
}

/// The textures on the faces of a `GroundBox`. For the top and the bottom, the corners of
/// each `GroundCell` are in the ground's order: lowest, +X, +Z, far. For a side they are
/// as seen from outside: bottom left, bottom right, top left, top right.
#[derive(Clone, Debug)]
pub struct BoxFaces {
    pub plus_x: GroundCell,
    pub minus_x: GroundCell,
    pub plus_z: GroundCell,
    pub minus_z: GroundCell,
    pub top: GroundCell,
    pub bottom: GroundCell,
}

/// Everything standing around a track: a few models, each set down in many places.
#[derive(Clone, Debug, Default)]
pub struct Scenery {
    /// Pixels along each side of a tile.
    pub tile_size: usize,
    /// The models' textures, as red, green, blue and alpha bytes, row by row from the
    /// top. Alpha 0 is a hole to see through. For `track::tile_array`.
    pub tiles: Vec<Vec<u8>>,
    /// The animated textures, which `SceneryModel::texture_cycles` names from 1. Each
    /// frame is a tile of `tiles`, and a cycle's frames are consecutive tiles.
    pub texture_cycles: Vec<TextureCycle>,
    pub models: Vec<SceneryModel>,
    pub objects: Vec<SceneryObject>,
}

/// An animated texture: tiles shown one after the other, round and round, each for the
/// same time, all on one clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureCycle {
    /// The index into `Scenery::tiles` of the first frame. The others follow it.
    pub first_tile: u32,
    /// How many frames there are. At least 1.
    pub frames: u32,
    /// How long each frame is shown, in seconds. Greater than 0.
    pub seconds_per_frame: f32,
}

/// How a model's vertices move, from keyframe to keyframe, round and round.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keyframes {
    /// How long the model takes from one keyframe to the next, in seconds. Greater than 0.
    pub seconds_per_frame: f32,
    /// Where each keyframe puts every vertex of the mesh, in metres, in the order of
    /// `SceneryModel::positions`. The first is those positions. At least 2.
    pub frames: Vec<Vec<[f32; 3]>>,
    /// The mesh's faces, each a run of consecutive vertices, for working out their
    /// normals again as they move.
    pub faces: Vec<KeyframedFace>,
}

/// A face of a model that moves by keyframes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyframedFace {
    /// Its first vertex in `SceneryModel::positions`.
    pub first: u32,
    /// How many corners, and so vertices, it has, in order round it.
    pub corners: u32,
}

/// A track's own sky, and the skies of other weather and times of day. Each is `None`
/// where the track has none.
#[derive(Clone, Debug, Default)]
pub struct Skies {
    /// Clear weather by day: the sky the track names.
    pub own: Option<SkyPicture>,
    pub cloudy: Option<SkyPicture>,
    pub dusk: Option<SkyPicture>,
    pub night: Option<SkyPicture>,
}

/// A picture of the sky, laid round the horizon: its top row at the top of the sky, its
/// bottom row at the horizon, and its left and right edges meeting.
#[derive(Clone, Debug)]
pub struct SkyPicture {
    /// Pixels along each side.
    pub size: usize,
    /// Red, green, blue and alpha bytes, row by row from the top.
    pub rgba: Vec<u8>,
}

/// Models drawn round the horizon: distant hills, a skyline. They stand round the camera
/// wherever it goes, so they never come nearer, and are drawn behind everything else.
#[derive(Clone, Debug, Default)]
pub struct Backdrop {
    /// Pixels along each side of a tile.
    pub tile_size: usize,
    /// The models' textures, as for `Scenery::tiles`.
    pub tiles: Vec<Vec<u8>>,
    /// In the track's order. Each is placed with its origin at the camera, and is the size
    /// it was made; where it is drawn scales it up without changing how it looks.
    pub models: Vec<SceneryModel>,
}

/// A model as a triangle mesh in the game's axes and metres, relative to its own origin.
/// Every corner of every face has a vertex of its own, as the faces are flat-shaded and
/// differently textured.
#[derive(Clone, Debug, Default)]
pub struct SceneryModel {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Position within the tile, (across, down) from its top left, from 0 to 1.
    pub uvs: Vec<[f32; 2]>,
    /// For each vertex, the index into `Scenery::tiles` of its face's texture: for an
    /// animated texture, of its first frame.
    pub tiles: Vec<u32>,
    /// For each vertex, which of `Scenery::texture_cycles` its face's texture is, counting
    /// from 1, or 0 for a texture that stays still. Empty where every texture stays still.
    pub texture_cycles: Vec<u32>,
    /// Three per triangle, counter-clockwise seen from the front.
    pub indices: Vec<u32>,
    /// How the model moves, for one that moves by keyframes. It is solid as it stands in
    /// its first keyframe.
    pub keyframes: Option<Keyframes>,
}

#[derive(Clone, Debug)]
pub struct SceneryObject {
    /// Index into `Scenery::models`.
    pub model: usize,
    /// On the ground plane (world X and Z), in metres.
    pub position: Vec2,
    /// Height of the model's origin above the ground under it, in metres. Kept relative
    /// so that the object stays on the ground if the ground is reshaped.
    pub height_above_ground: f32,
    /// Rotation about Y in radians.
    pub yaw: f32,
    /// Whether trucks collide with it.
    pub solid: bool,
    pub motion: SceneryMotion,
    /// Whether it turns about Y to face the camera, whatever its `yaw`. Such a model is a
    /// flat picture of a tree or a palm, drawn on both sides, which faces +Z.
    pub faces_camera: bool,
    /// Whether it is drawn. A ramp that a track gives by its size alone is solid but
    /// unseen: the solid shape under a model that trucks drive through.
    pub visible: bool,
    /// Whether it is a ramp, which trucks drive up and over rather than round: a box of the
    /// track file's Ramps section (situation.md, "Ramps").
    pub ramp: bool,
    /// How far below `height_above_ground` it stands when it is lowered onto sloping
    /// ground (`TrackSettings::settle_scenery`), in metres. Worked out as the track is
    /// loaded (`settle`).
    pub sunk_on_slope: f32,
}

/// Whether a scenery object stays where it is put.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SceneryMotion {
    /// Nothing moves it.
    #[default]
    Fixed,
    /// Stands where it is put until something knocks it over or away.
    Loose {
        /// In kilograms.
        mass: f32,
    },
    /// Goes on at this velocity, in metres per second along the world's axes, whatever it
    /// meets. Nothing moves it off its way.
    Moving { velocity: Vec3 },
}

/// A checkpoint: a line across the course that must be crossed in one direction.
#[derive(Clone, Debug)]
pub struct Gate {
    /// Middle of the gate on the ground plane (world X and Z), in metres.
    pub center: Vec2,
    /// Direction of travel through the gate, as a rotation about Y in radians.
    /// 0 faces -Z, like a truck with no rotation.
    pub yaw: f32,
    /// Half the drivable width of the gate, in metres.
    pub half_width: f32,
}

impl Gate {
    /// Unit vector of the direction of travel, on the ground plane (X, Z).
    pub fn direction(&self) -> Vec2 {
        yaw_direction(self.yaw)
    }

    /// Unit vector along the gate line, on the ground plane (X, Z).
    pub fn across(&self) -> Vec2 {
        self.direction().perp()
    }
}

/// Where a truck waits for the race to begin.
#[derive(Clone, Debug)]
pub struct StartPosition {
    /// On the ground plane (world X and Z), in metres.
    pub position: Vec2,
    /// Rotation about Y in radians. 0 faces -Z.
    pub yaw: f32,
}

/// The ground-plane (X, Z) direction faced by something rotated `yaw` about Y.
pub fn yaw_direction(yaw: f32) -> Vec2 {
    Vec2::new(-yaw.sin(), -yaw.cos())
}

/// The inverse of `yaw_direction`.
pub fn direction_yaw(direction: Vec2) -> f32 {
    (-direction.x).atan2(-direction.y)
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use super::*;

    /// A flat, empty track, 100 m across, with pole position at its origin.
    fn flat_track() -> TrackData {
        TrackData {
            name: "Flat".into(),
            heights: HeightGrid::from_fn(8, 100.0, |_, _| 0.0),
            surface: Vec::new(),
            ground: None,
            scenery: Scenery::default(),
            backdrop: None,
            skies: Skies::default(),
            footing: Vec::new(),
            ground_boxes: Vec::new(),
            water_level: None,
            course: None,
            other_courses: Vec::new(),
            gates: Vec::new(),
            start: StartPosition {
                position: Vec2::ZERO,
                yaw: 0.0,
            },
            grid: Vec::new(),
        }
    }

    #[test]
    fn a_grid_that_runs_out_carries_on_behind_its_last_place() {
        let place = |z: f32| StartPosition {
            position: Vec2::new(0.0, z),
            yaw: 0.0,
        };
        let track = TrackData {
            grid: vec![place(6.0)],
            ..flat_track()
        };
        assert_eq!(track.grid_place(0).position, track.start.position);
        assert_eq!(track.grid_place(1).position, Vec2::new(0.0, 6.0));
        // Facing -Z, so behind is +Z.
        let third = track.grid_place(2);
        assert_eq!(third.position, Vec2::new(0.0, 6.0 + EXTRA_PLACE_SPACING));
        assert_eq!(third.yaw, 0.0);
        assert_eq!(
            track.grid_place(4).position,
            Vec2::new(0.0, 6.0 + 3.0 * EXTRA_PLACE_SPACING)
        );
    }

    /// Colours read off real tracks' ground textures (`docs/formats/texture_types.md`).
    #[test]
    fn sand_dirt_and_grass_look_loose_and_road_snow_and_water_do_not() {
        // Baja Beach's sand, Snake River's gravel, and grass.
        assert!(looks_loose([213, 209, 184]));
        assert!(looks_loose([149, 133, 127]));
        assert!(looks_loose([70, 80, 30]));
        // Tight Corners' and Route 77's asphalt, snow, and water.
        assert!(!looks_loose([27, 29, 26]));
        assert!(!looks_loose([85, 79, 74]));
        assert!(!looks_loose([186, 186, 186]));
        assert!(!looks_loose([40, 120, 160]));
    }

    #[test]
    fn unnamed_ground_is_loose_where_its_texture_looks_it() {
        // One cell with a tile of two texels: grey on its -X half, sand on its +X half.
        let grey_and_sand = vec![40, 40, 40, 255, 213, 209, 184, 255];
        let track = |footing: Vec<Footing>| {
            let mut track = flat_track();
            track.ground = Some(GroundTextures {
                tile_size: 2,
                tiles: vec![[grey_and_sand.clone(), grey_and_sand.clone()].concat()],
                cells: vec![GroundCell {
                    tile: 0,
                    corners: [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]],
                }],
            });
            track.footing = footing;
            track
        };
        let quarter = flat_track().heights.size() / 4.0;
        let (grey, sand) = (-quarter, quarter);

        let unnamed = track(vec![Footing::Unnamed]);
        assert!(!unnamed.loose_at(grey, 0.0));
        assert!(unnamed.loose_at(sand, 0.0));
        // A track that names its ground is taken at its word.
        assert!(!track(vec![Footing::Firm]).loose_at(sand, 0.0));
        assert!(track(vec![Footing::Loose]).loose_at(grey, 0.0));
    }

    #[test]
    fn yaw_matches_bevy_rotations() {
        for yaw in [0.0, 0.4, FRAC_PI_2, -2.0, 3.0] {
            let forward = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
            let direction = yaw_direction(yaw);
            assert!((direction - forward.xz()).length() < 1e-6, "yaw {yaw}");
            assert!((direction_yaw(direction) - yaw).abs() < 1e-5, "yaw {yaw}");
        }
    }
}
