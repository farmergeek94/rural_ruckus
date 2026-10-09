//! Turns a Monster Truck Madness 2 track, as the `pod` crate reads it, into `TrackData`.
//!
//! This is the one place where MTM2's conventions are converted to the game's, and the
//! reasons for each rule are in `docs/formats/`:
//!
//! - MTM2 works in feet, with the world's corner at the origin. The game works in
//!   metres, with the world centred on the origin. The world repeats, and a position
//!   off the map is where it would be a map's width over.
//! - MTM2's Z axis runs the other way. Flipping it turns a heading of `psi`, measured
//!   from +Z, into a yaw of `-psi` about Y.
//! - A texture's type (.TTY) says what kind of surface it is, by its hundreds: ice is 8
//!   and 9, and dirt, mud, sand, grass and rocky ground (`Footing::Loose`) are 2 and 4
//!   to 7. A texture with no type is `Footing::Unnamed`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use crate::base_game::BaseGame;
use crate::hd_texture;
use crate::pod::{self, FEET_PER_CELL, FEET_PER_HEIGHT_STEP, PodArchive};

use super::data::{direction_yaw, yaw_direction};
use super::{
    BoxFaces, Course, Diagonals, Footing, Gate, GroundBox, GroundCell, GroundTextures, HeightGrid,
    StartPosition, TrackData,
};

const METRES_PER_FOOT: f32 = 0.3048;

/// The texture types that are ice. Measured by looking at every texture of these types in
/// the archives seen (`docs/formats/texture_types.md`): all ice, and no ice of any other
/// type.
const ICE_TYPES: std::ops::RangeInclusive<u32> = 800..=999;
/// The texture types that are loose ground, which tires throw up: dirt (2), mud (4), sand
/// (5), grass (6) and rocky ground (7). Measured by looking at the textures of every type on
/// the ground of the base game's tracks (`docs/formats/texture_types.md`). The others are
/// road (1), water (3), metal (10), rock (12) and railway track (14).
const LOOSE_HUNDREDS: [u32; 5] = [2, 4, 5, 6, 7];

/// Half the width of a checkpoint whose model isn't in the archive, in feet. Roads are
/// two terrain cells wide, so one cell either side of the middle spans the road.
const CHECKPOINT_HALF_WIDTH_FEET: f32 = FEET_PER_CELL;

/// How far apart two places of the starting grid must be, in metres: a truck is 3.8 m
/// across its tires. A place nearer than this to one before it is skipped, and the truck
/// that would have had it lines up behind the last place (`TrackData::grid_place`). Lands
/// Between puts its eight places side by side 10 ft (3.05 m) apart, where the trucks would
/// start inside each other and be thrown apart.
const PLACE_CLEARANCE: f32 = 4.0;

/// The name of the track in an archive on disk, for a list of tracks: the archive's
/// directory and its track file, and none of its ground, models or textures. `Ok(None)`
/// for an archive with no track in it, and an error for one that can't be read. A track
/// that gives itself no name comes back as an empty one.
pub fn peek_pod(path: &Path) -> Result<Option<String>, String> {
    let describe = |error: &dyn std::fmt::Display| format!("{}: {error}", path.display());
    let bytes = std::fs::read(path).map_err(|error| describe(&error))?;
    let archive = PodArchive::parse(bytes).map_err(|error| describe(&error))?;
    Ok(pod::Track::name_in(&archive))
}

/// A track in one of the base game's archives.
pub struct BaseTrack {
    /// The archive it is in.
    pub archive: PathBuf,
    /// Its track file in that archive, such as `WORLD\\JUNK.SIT`.
    pub file: String,
    /// The name it gives itself, or why its track file can't be read.
    pub name: Result<String, String>,
}

/// Every track in the base game's archives, for a list of tracks: each track file is
/// read on its own, and nothing else of its archive.
pub fn peek_base(base: &BaseGame) -> Vec<BaseTrack> {
    let mut found = Vec::new();
    for (archive, entries) in base.directories() {
        for entry in pod::Track::files_in(entries) {
            let name = base
                .read_one(archive, entry)
                .map(|bytes| pod::Track::name_from(&bytes))
                .map_err(|error| format!("{}: {error}", entry.name));
            found.push(BaseTrack {
                archive: archive.to_path_buf(),
                file: entry.name.clone(),
                name,
            });
        }
    }
    found
}

/// Reads one track of the base game's archives, as [`peek_base`] lists it.
pub fn load_base(base: &BaseGame, archive: &Path, file: &str) -> Result<TrackData, String> {
    let describe = |error: &dyn std::fmt::Display| format!("{} {file}: {error}", archive.display());
    let pod_archive = base
        .archive(archive)
        .ok_or_else(|| describe(&"is not a base archive that can be read"))?;
    let track = pod::Track::from_file(pod_archive, file, base).map_err(|error| describe(&error))?;
    track_from_pod(&track).map_err(|error| describe(&error))
}

/// Reads the first track in a POD archive on disk, with what it borrows from `base`.
pub fn load_pod(path: &Path, base: &BaseGame) -> Result<TrackData, String> {
    let describe = |error: &dyn std::fmt::Display| format!("{}: {error}", path.display());
    let bytes = std::fs::read(path).map_err(|error| describe(&error))?;
    let archive = PodArchive::parse(bytes).map_err(|error| describe(&error))?;
    let track = pod::Track::from_archives(&archive, base).map_err(|error| describe(&error))?;
    track_from_pod(&track).map_err(|error| describe(&error))
}

pub fn track_from_pod(track: &pod::Track) -> Result<TrackData, String> {
    let cells = track.heightmap.size();
    if !cells.is_multiple_of(2) {
        // Flipping Z would swap which cells of the chessboard are which.
        return Err(format!("a heightmap {cells} cells wide is not supported"));
    }
    let world_feet = cells as f32 * FEET_PER_CELL;
    // The world repeats, and MTM2's own tracks place checkpoints and scenery up to half a
    // map beyond its edges: each is where it would be a map's width over (situation.md).
    // Whatever is on the map, its far edge included, stays where it is.
    let on_the_map = |feet: f32| {
        if (0.0..=world_feet).contains(&feet) {
            feet
        } else {
            feet.rem_euclid(world_feet)
        }
    };
    let to_ground = |feet: [f32; 3]| {
        let (x, z) = (on_the_map(feet[0]), on_the_map(feet[2]));
        Vec2::new(x - world_feet / 2.0, world_feet / 2.0 - z) * METRES_PER_FOOT
    };

    // The world repeats, so one extra row and column of vertices closes the far edges
    // with the heights of the near ones. Rows count the other way because Z is flipped.
    let resolution = cells + 1;
    let heights =
        HeightGrid::from_vertex_fn(resolution, world_feet * METRES_PER_FOOT, |col, row| {
            track.heightmap.feet(col, cells - row) * METRES_PER_FOOT
        })
        // MTM2 splits its even cells from the origin corner. With Z flipped those are
        // our odd cells, and that diagonal becomes our +X to +Z one: exactly what
        // `Checkerboard` gives the odd cells (see `docs/formats/terrain.md`).
        .with_diagonals(Diagonals::Checkerboard)
        // And beyond each edge is the other: Monte Carlo's course drives off the map on the
        // west and on again from the east, and off on the south and on from the north.
        .repeating();

    // Without the ground textures, at least show where they change: whatever isn't the
    // most common texture is taken to be course. A vertex is shaded by how many of the
    // four cells around it are.
    let mut surface = vec![0.0; resolution * resolution];
    if let Some(map) = &track.texture_map {
        let open_ground = map.most_common_texture();
        for row in 0..resolution {
            for col in 0..resolution {
                // Cell indices are offset by the map's size so that stepping back one
                // wraps round instead of going below zero.
                let (cell_col, cell_row) = (col + cells, cells - row + cells);
                let course_cells = [(0, 0), (1, 0), (0, 1), (1, 1)]
                    .into_iter()
                    .filter(|(dx, dz)| {
                        map.cell(cell_col - dx, cell_row - dz).texture != open_ground
                    })
                    .count();
                surface[row * resolution + col] = course_cells as f32 / 4.0;
            }
        }
    }

    let pole_position = track
        .situation
        .vehicles
        .first()
        .ok_or("the track has no starting grid")?;
    let grid_place = |vehicle: &pod::Vehicle| StartPosition {
        position: to_ground(vehicle.position),
        yaw: -vehicle.psi,
    };

    let course: Vec<(Vec2, Vec2)> = track
        .situation
        .course
        .iter()
        .map(|segment| (to_ground(segment.start), to_ground(segment.end)))
        .collect();

    let mut gates: Vec<Gate> = track
        .situation
        .checkpoints()
        .map(|checkpoint| {
            let center = to_ground(checkpoint.position);
            let authored = yaw_direction(-checkpoint.angles[2]);
            Gate {
                center,
                yaw: direction_yaw(travel_direction(center, authored, &course)),
                half_width: checkpoint_half_width_feet(track, checkpoint) * METRES_PER_FOOT,
            }
        })
        .collect();
    // MTM2 lists the finish line last. Here it comes first, as gate 0. A track may have no
    // checkpoints at all (`tground.pod`), and rotating none panics.
    if !gates.is_empty() {
        gates.rotate_right(1);
    }

    let (ground, ground_boxes) = match ground_textures(track) {
        Some((mut tiles, cells)) => {
            let boxes = ground_boxes(track, to_ground, Some(&mut tiles));
            (tiles.finish(cells), boxes)
        }
        None => (None, ground_boxes(track, to_ground, None)),
    };

    let course = course_from_segments(&course, &heights, false);
    let other_courses = track
        .situation
        .extra_courses
        .iter()
        .map(|segments| {
            let segments: Vec<(Vec2, Vec2)> = segments
                .iter()
                .map(|segment| (to_ground(segment.start), to_ground(segment.end)))
                .collect();
            course_from_segments(&segments, &heights, true)
        })
        .collect();
    let mut scenery = super::pod_scenery::scenery_from_pod(track, to_ground);
    super::settle::settle(&mut scenery, &heights);
    Ok(TrackData {
        name: track.situation.name.clone(),
        heights,
        surface,
        ground,
        footing: footing(track),
        ground_boxes,
        // MTM2's arenas write a height of 0 for no water.
        water_level: track
            .level
            .water_feet()
            .filter(|&feet| feet > 0.0)
            .map(|feet| feet * METRES_PER_FOOT),
        scenery,
        backdrop: super::pod_scenery::backdrop_from_pod(track),
        skies: skies_from_pod(&track.skies),
        course,
        other_courses,
        gates,
        start: grid_place(pole_position),
        grid: clear_places(
            grid_place(pole_position),
            track.situation.vehicles[1..].iter().map(grid_place),
        ),
    })
}

/// The places after pole position, in order, without any too near pole position or a place
/// kept before it.
fn clear_places(
    pole_position: StartPosition,
    places: impl Iterator<Item = StartPosition>,
) -> Vec<StartPosition> {
    let mut kept = vec![pole_position];
    for place in places {
        let clear = kept
            .iter()
            .all(|other| other.position.distance(place.position) >= PLACE_CLEARANCE);
        if clear {
            kept.push(place);
        }
    }
    kept.split_off(1)
}

/// A checkpoint is as wide as its model, which is long along its X axis: a trigger box,
/// or a banner over the road. A checkpoint with no model is an invisible box, and its
/// `width` is its whole size along that same axis (situation.md).
fn checkpoint_half_width_feet(track: &pod::Track, checkpoint: &pod::SituationBox) -> f32 {
    let name = match &checkpoint.shape {
        pod::BoxShape::Model(name) => name,
        pod::BoxShape::Dimensions([_, width, _]) => return width / 2.0,
    };
    match track.models.get(&name.to_ascii_uppercase()) {
        Some(model) => {
            let (low, high) = model.bounds();
            low[0].abs().max(high[0].abs())
        }
        None => CHECKPOINT_HALF_WIDTH_FEET,
    }
}

/// Width given to a POD track's course, in feet: roads are two terrain cells wide.
const COURSE_WIDTH_FEET: f32 = 2.0 * FEET_PER_CELL;

/// Most metres between the points of a corner that is put back into a course.
const CORNER_STEP: f32 = 5.0;
/// A corner is put back only where the lines of its two pieces meet no further from them
/// than this many times the gap between the pieces. Pieces that are nearly in line meet far
/// away, and a curve out to there would leave the road.
const CORNER_REACH: f32 = 2.0;

/// The route round the track, from the straight pieces that computer trucks follow.
/// The corners between them aren't in the file, so each piece is joined to the next, and
/// the last back to the first, the short way across an edge of the `ground` where that is
/// shorter. Joined straight, or, with `round_corners`, by the corner that was left out
/// (`corner`).
///
/// The main course is joined straight: the race, the map and the checkpoints' directions
/// are measured on it, and the computer's drivers were tuned on it. On the extra courses a
/// straight join cuts across the grass and through what stands there (situation.md,
/// "Corners of the extra courses").
fn course_from_segments(
    segments: &[(Vec2, Vec2)],
    ground: &HeightGrid,
    round_corners: bool,
) -> Option<Course> {
    let mut centerline: Vec<Vec2> = Vec::with_capacity(segments.len() * 2);
    for (index, &(start, end)) in segments.iter().enumerate() {
        let (next_start, next_end) = segments[(index + 1) % segments.len()];
        let corner = match round_corners {
            true => corner((start, end), (next_start, next_end), ground),
            false => Vec::new(),
        };
        for point in [start, end].into_iter().chain(corner) {
            // Pieces that do meet would otherwise leave a segment of no length, and so
            // would a point on one edge of the map and the same point on the other.
            if centerline
                .last()
                .is_none_or(|&last| ground.offset(last, point).length() > 0.01)
            {
                centerline.push(point);
            }
        }
    }
    (centerline.len() >= 3).then(|| {
        let width = COURSE_WIDTH_FEET * METRES_PER_FOOT;
        match ground.repeats() {
            true => Course::repeating(centerline, width, ground.size()),
            false => Course::new(centerline, width),
        }
    })
}

/// The points of the corner left out between a straight `piece` and the `next`, not counting
/// the ends of the pieces: a curve that leaves the one along it and meets the other along
/// it (a quadratic Bezier curve whose middle control point is where the pieces' lines
/// meet). Each piece's end is where the road's bend begins (situation.md, "Corners of the
/// extra courses"). None where the lines meet behind either piece, as in a jog to one
/// side, or too far off (`CORNER_REACH`): those are joined straight.
fn corner(piece: (Vec2, Vec2), next: (Vec2, Vec2), ground: &HeightGrid) -> Vec<Vec2> {
    let along = ground.offset(piece.0, piece.1).normalize_or_zero();
    let next_along = ground.offset(next.0, next.1).normalize_or_zero();
    // From the end of the piece to the start of the next, the short way.
    let gap = ground.offset(piece.1, next.0);
    let turn = along.perp_dot(next_along);
    if turn.abs() < 1e-3 || gap.length() < CORNER_STEP {
        return Vec::new();
    }
    // The end of the piece goes `ahead` along it, and the start of the next `behind` back
    // along that, to where the two lines meet.
    let ahead = gap.perp_dot(next_along) / turn;
    let behind = -gap.perp_dot(along) / turn;
    let reach = CORNER_REACH * gap.length();
    if ahead <= 0.0 || behind <= 0.0 || ahead > reach || behind > reach {
        return Vec::new();
    }
    let control = along * ahead;
    // Out to where the lines meet and back is never shorter than the curve.
    let steps = ((ahead + behind) / CORNER_STEP).ceil() as usize;
    (1..steps)
        .map(|step| {
            let t = step as f32 / steps as f32;
            let point = 2.0 * (1.0 - t) * t * control + t * t * gap;
            ground.onto(piece.1 + point)
        })
        .collect()
}

/// The track's ground textures, if it carries its own, and the tiles they are made into,
/// which the ground boxes will add theirs to. Tracks that borrow the base game's textures
/// don't have them, and get none rather than a patchwork.
fn ground_textures(track: &pod::Track) -> Option<(TileSet<'_>, Vec<GroundCell>)> {
    let map = track.texture_map.as_ref()?;
    let mut tiles = TileSet {
        track,
        tile_of_texture: HashMap::new(),
        tiles: Vec::new(),
        tile_size: None,
    };
    let cells_per_side = map.size();
    let mut cells = Vec::with_capacity(cells_per_side * cells_per_side);

    // Our rows count the other way because Z is flipped, which also turns the distance
    // `v` along a cell's Z side into `1 - v`.
    for row in 0..cells_per_side {
        for col in 0..cells_per_side {
            let cell = map.cell(col, cells_per_side - 1 - row);
            cells.push(flat_face(cell, tiles.tile(cell.texture)?));
        }
    }
    Some((tiles, cells))
}

/// What the ground is underfoot in each cell, in the same order as `ground_textures`'s
/// cells. Empty for a track without a texture map or texture types.
fn footing(track: &pod::Track) -> Vec<Footing> {
    let (Some(map), Some(types)) = (&track.texture_map, &track.texture_types) else {
        return Vec::new();
    };
    let of_texture: Vec<Footing> = track
        .ground_texture_names
        .iter()
        .map(|name| match types.class_of(name) {
            Some(class) if ICE_TYPES.contains(&class) => Footing::Ice,
            Some(class) if LOOSE_HUNDREDS.contains(&(class / 100)) => Footing::Loose,
            Some(_) => Footing::Firm,
            None => Footing::Unnamed,
        })
        .collect();
    let cells_per_side = map.size();
    let mut cells = Vec::with_capacity(cells_per_side * cells_per_side);
    // Rows count the other way because Z is flipped, as for the textures.
    for row in 0..cells_per_side {
        for col in 0..cells_per_side {
            let texture = map.cell(col, cells_per_side - 1 - row).texture as usize;
            cells.push(of_texture.get(texture).copied().unwrap_or_default());
        }
    }
    cells
}

/// A texture laid flat, as on the ground, corners in the order lowest, +X, +Z, far.
fn flat_face(cell: pod::TextureCell, tile: usize) -> GroundCell {
    let corners = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
        .map(|(u, v)| cell.texture_coords(u, 1.0 - v).into());
    GroundCell { tile, corners }
}

/// A texture stood upright on the side of a ground box, corners in the order bottom left,
/// bottom right, top left, top right, as seen from outside. Which way up and which way
/// round is not known (see `docs/formats/ground_boxes.md`).
fn side_face(cell: pod::TextureCell, tile: usize) -> GroundCell {
    let corners = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
        .map(|(u, v)| cell.texture_coords(u, v).into());
    GroundCell { tile, corners }
}

/// The ground's textures as tiles, each made once, numbered in order of first use.
struct TileSet<'a> {
    track: &'a pod::Track,
    tile_of_texture: HashMap<u16, usize>,
    tiles: Vec<Vec<u8>>,
    tile_size: Option<usize>,
}

impl TileSet<'_> {
    /// The tile for a texture of the list, or `None` if the track doesn't carry it or it
    /// is a different size from the others.
    fn tile(&mut self, texture: u16) -> Option<usize> {
        if let Some(&tile) = self.tile_of_texture.get(&texture) {
            return Some(tile);
        }
        let (size, rgba) = self.image(texture as usize)?;
        if *self.tile_size.get_or_insert(size) != size {
            return None;
        }
        self.tiles.push(rgba);
        self.tile_of_texture.insert(texture, self.tiles.len() - 1);
        Some(self.tiles.len() - 1)
    }

    /// A texture's size and pixels: from its PNG where the archive has one, as the
    /// patched game does, and otherwise from its .RAW and its own palette, or the track's
    /// where it has none. A PNG that can't be decoded falls back to the .RAW.
    fn image(&self, texture: usize) -> Option<(usize, Vec<u8>)> {
        let track = self.track;
        let hd = track
            .ground_hd_textures
            .get(texture)
            .and_then(Option::as_ref);
        if let Some(hd) = hd
            && let Some(rgba) = hd_texture::decode(hd)
        {
            // The ground is opaque: whatever alpha the file has means nothing here.
            return Some((hd.size(), opaque(rgba)));
        }
        let image = track.ground_textures.get(texture)?.as_ref()?;
        let own_palette = track
            .ground_texture_names
            .get(texture)
            .and_then(|name| track.own_palettes.get(&name.to_ascii_uppercase()));
        let palette = own_palette.or(track.palette.as_ref())?;
        Some((image.size(), image.to_rgba(palette)))
    }

    fn finish(self, cells: Vec<GroundCell>) -> Option<GroundTextures> {
        Some(GroundTextures {
            tile_size: self.tile_size?,
            tiles: self.tiles,
            cells,
        })
    }
}

/// The track's ground boxes, one per cell, covered in tiles from `tiles` where there are
/// any. A box whose textures can't all be found is left plain.
fn ground_boxes(
    track: &pod::Track,
    to_ground: impl Fn([f32; 3]) -> Vec2,
    mut tiles: Option<&mut TileSet>,
) -> Vec<GroundBox> {
    let Some(boxes) = &track.ground_boxes else {
        return Vec::new();
    };
    boxes
        .iter()
        .map(|(col, row, found)| {
            // The cell's corners, in feet. Z is flipped, so the corner with the lowest Z
            // here is the one with the highest there.
            let (x, z) = (col as f32 * FEET_PER_CELL, row as f32 * FEET_PER_CELL);
            let height = |steps: u8| steps as f32 * FEET_PER_HEIGHT_STEP * METRES_PER_FOOT;
            let faces = match (found.faces, tiles.as_deref_mut()) {
                (Some(faces), Some(tiles)) => box_faces(&faces, tiles),
                _ => None,
            };
            GroundBox {
                min: to_ground([x, 0.0, z + FEET_PER_CELL]),
                max: to_ground([x + FEET_PER_CELL, 0.0, z]),
                bottom: height(found.lower.min(found.upper)),
                top: height(found.upper.max(found.lower)),
                faces,
            }
        })
        .collect()
}

/// The same pixels, with every one fully opaque.
fn opaque(mut rgba: Vec<u8>) -> Vec<u8> {
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    rgba
}

/// A box's faces in our axes: with Z flipped, the side MTM2 has facing +Z faces -Z here.
fn box_faces(faces: &[pod::TextureCell; 6], tiles: &mut TileSet) -> Option<BoxFaces> {
    let mut side = |face: usize| Some(side_face(faces[face], tiles.tile(faces[face].texture)?));
    let (plus_x, minus_x) = (side(pod::face::EAST)?, side(pod::face::WEST)?);
    let (plus_z, minus_z) = (side(pod::face::SOUTH)?, side(pod::face::NORTH)?);
    let mut flat = |face: usize| Some(flat_face(faces[face], tiles.tile(faces[face].texture)?));
    Some(BoxFaces {
        plus_x,
        minus_x,
        plus_z,
        minus_z,
        top: flat(pod::face::TOP)?,
        bottom: flat(pod::face::BOTTOM)?,
    })
}

/// Which way trucks drive through a checkpoint. Its authored heading gives the line it
/// lies on, but track makers didn't always point it the right way along that line, so
/// the course the computer trucks follow has the casting vote when it passes nearby: the
/// heading is turned round where the course near it runs against it, and nowhere near it
/// runs with it. A course can pass a checkpoint twice, once each way: Lands Between's goes
/// through its first checkpoint eastwards, as the file's heading says, and passes it again
/// westwards 2.7 m from its middle, nearer than the first time (4.3 m).
fn travel_direction(center: Vec2, authored: Vec2, course: &[(Vec2, Vec2)]) -> Vec2 {
    let pieces: Vec<(f32, Vec2)> = course
        .iter()
        .map(|&(start, end)| {
            let along = end - start;
            let t = ((center - start).dot(along) / along.length_squared()).clamp(0.0, 1.0);
            (
                center.distance(start + along * t),
                along.normalize_or_zero(),
            )
        })
        .collect();
    let Some(nearest) = pieces.iter().map(|piece| piece.0).min_by(f32::total_cmp) else {
        return authored;
    };
    // As near as the nearest, give or take half the width of the course. Where the course
    // passes both ways, the pass the heading agrees with is up to 7.2 m further than the
    // nearest (Lands Between's fourth checkpoint); in the drag arenas Tacoma Dome and Trans
    // World Dome, where the heading is the wrong way, the course runs with it 15.5 m further.
    let near = || {
        pieces
            .iter()
            .filter(move |piece| piece.0 <= nearest + COURSE_WIDTH_FEET * METRES_PER_FOOT / 2.0)
            .map(|piece| piece.1)
    };
    // Only a course that runs roughly along the authored heading can flip it; one that
    // crosses it says nothing about which way is forwards.
    let against = near().any(|direction| authored.dot(direction) < -0.5);
    let with = near().any(|direction| authored.dot(direction) > 0.5);
    if against && !with {
        -authored
    } else {
        authored
    }
}

/// The skies, coloured as MTM2 colours a sky (see `pod::sky_rgba`).
fn skies_from_pod(skies: &pod::Skies) -> super::Skies {
    let picture = |sky: &Option<pod::Sky>| {
        sky.as_ref().map(|sky| super::SkyPicture {
            size: sky.texture.size(),
            rgba: pod::sky_rgba(&sky.texture, &sky.palette),
        })
    };
    super::Skies {
        own: picture(&skies.own),
        cloudy: picture(&skies.cloudy),
        dusk: picture(&skies.dusk),
        night: picture(&skies.night),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flat ground 1 km across, which repeats as MTM2's does.
    fn repeating_ground() -> HeightGrid {
        HeightGrid::from_fn(9, 1000.0, |_, _| 0.0).repeating()
    }

    #[test]
    fn a_corner_left_out_is_put_back_as_a_curve_along_both_pieces() {
        let ground = repeating_ground();
        // Along +X to (0, 0), then from (40, 40) along +Z: the lines meet at (40, 0).
        let piece = (Vec2::new(-100.0, 0.0), Vec2::ZERO);
        let next = (Vec2::new(40.0, 40.0), Vec2::new(40.0, 140.0));
        let points = corner(piece, next, &ground);
        assert!(points.len() >= 10, "{points:?}");
        // Inside the sharp corner and outside the straight line across it, so on a road
        // that bends round there.
        let middle = points[points.len() / 2];
        let chord = Vec2::new(20.0, 20.0);
        let sharp = Vec2::new(40.0, 0.0);
        assert!(
            middle.distance(chord) > 5.0 && middle.distance(sharp) > 5.0,
            "{middle}"
        );
        // It leaves the one piece along it, and meets the other along it.
        assert!(points[0].y.abs() < 1.0 && points[0].x > 0.0);
        assert!((points[points.len() - 1].x - 40.0).abs() < 1.0);
        // No step longer than CORNER_STEP.
        let path: Vec<Vec2> = [piece.1]
            .into_iter()
            .chain(points)
            .chain([next.0])
            .collect();
        assert!(
            path.windows(2)
                .all(|pair| pair[0].distance(pair[1]) <= CORNER_STEP)
        );
    }

    #[test]
    fn pieces_that_jog_or_run_on_in_line_are_joined_straight() {
        let ground = repeating_ground();
        let piece = (Vec2::new(-100.0, 0.0), Vec2::ZERO);
        // A jog to one side: the lines meet behind the end of the piece.
        let jog = (Vec2::new(40.0, 40.0), Vec2::new(140.0, 90.0));
        assert!(corner(piece, jog, &ground).is_empty());
        // In line, and nearly so: they meet nowhere, or far off.
        let in_line = (Vec2::new(40.0, 0.0), Vec2::new(140.0, 0.0));
        assert!(corner(piece, in_line, &ground).is_empty());
        let nearly = (Vec2::new(40.0, 2.0), Vec2::new(140.0, 4.0));
        assert!(corner(piece, nearly, &ground).is_empty());
    }

    #[test]
    fn a_corner_across_an_edge_of_the_map_goes_the_short_way() {
        let ground = repeating_ground();
        // The same corner as above, with the next piece written a map's width over.
        let piece = (Vec2::new(-100.0, 0.0), Vec2::ZERO);
        let next = (Vec2::new(40.0, 40.0), Vec2::new(40.0, 140.0));
        let over = (next.0 + Vec2::X * 1000.0, next.1 + Vec2::X * 1000.0);
        let near = corner(piece, next, &ground);
        let far = corner(piece, over, &ground);
        assert_eq!(near.len(), far.len());
        assert!(near.iter().zip(&far).all(|(a, b)| a.distance(*b) < 1e-3));
    }

    #[test]
    fn the_course_decides_which_way_a_checkpoint_faces() {
        let course = [(Vec2::new(0.0, 100.0), Vec2::new(0.0, -100.0))];
        let (north, east) = (Vec2::NEG_Y, Vec2::X);
        let center = Vec2::new(3.0, 20.0);

        assert_eq!(travel_direction(center, north, &course), north);
        assert_eq!(travel_direction(center, -north, &course), north);
        // Nothing to go on: keep what the track says.
        assert_eq!(travel_direction(center, east, &course), east);
        assert_eq!(travel_direction(center, -north, &[]), -north);
    }

    #[test]
    fn a_checkpoint_the_course_passes_both_ways_keeps_its_own_heading() {
        // Northwards past it 5 m to the west, and back southwards 3 m to the east.
        let course = [
            (Vec2::new(-5.0, 100.0), Vec2::new(-5.0, -100.0)),
            (Vec2::new(3.0, -100.0), Vec2::new(3.0, 100.0)),
        ];
        let (north, south) = (Vec2::NEG_Y, Vec2::Y);
        assert_eq!(travel_direction(Vec2::ZERO, north, &course), north);
        assert_eq!(travel_direction(Vec2::ZERO, south, &course), south);
    }
    #[test]
    fn grid_places_too_near_another_are_skipped() {
        let place = |x: f32| StartPosition {
            position: Vec2::new(x, 0.0),
            yaw: 0.0,
        };
        // Lands Between's row: side by side, 10 ft apart.
        let spacing = 10.0 * METRES_PER_FOOT;
        let kept = clear_places(place(0.0), (1..8).map(|i| place(i as f32 * spacing)));
        let xs: Vec<f32> = kept.iter().map(|place| place.position.x).collect();
        assert_eq!(xs, [2.0 * spacing, 4.0 * spacing, 6.0 * spacing]);
        // Places far enough apart are all kept.
        assert_eq!(
            clear_places(place(0.0), [place(5.0), place(10.0)].into_iter()).len(),
            2
        );
    }
}
