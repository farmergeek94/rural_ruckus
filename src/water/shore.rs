//! Where the water meets the land, and what it does there.
//!
//! Two things come from the ground's heights, made once when the race starts:
//!
//! - A map of how deep the still water is over the ground, every `MAP_SPACING` metres,
//!   which `water.wgsl` reads. There it draws foam at the water's edge, lines
//!   of surf that roll in over shallow water, and foam where a truck's ripples break in the
//!   shallows. The water's edge moves up and down the ground with the swell.
//! - The points along the water's edge, each with which way is up the bank and how steep it
//!   is. Near the camera, the water breaks on a point when a crest of the swell comes in
//!   over it, and when a truck's ripple comes to it. Each break is told to whatever throws
//!   spray (`Surf`).
//!
//! Where the swell and the rings are comes from the same rules as `water.wgsl` draws them,
//! repeated here. A test checks that the two agree on their numbers.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, ShaderType, TextureDimension, TextureFormat};

use super::ripples::Ripples;
use super::wind::Wind;
use super::{Surf, SurfKind, WaterSettings};
use crate::track::{HeightGrid, Track};

/// The swell, as `water.wgsl` has it: each wave's angle to the wind (radians), length and
/// height (metres). A test checks that they match.
const SWELL_ANGLES: [f32; 3] = [0.0, 0.45, -0.6];
const SWELL_LENGTHS: [f32; 3] = [16.0, 10.5, 6.5];
const SWELL_HEIGHTS: [f32; 3] = [0.26, 0.15, 0.08];
/// The rings, as the field of heights spreads them (`field.wgsl`): how fast they go
/// (m/s, which a test checks), and the game's own guess at how long they take to fade
/// to a third (seconds) and how much they thin for each metre they spread. The field is
/// never read back, so where a ring has got to is reckoned here.
const RIPPLE_SPEED: f32 = 3.0;
const RIPPLE_FADE: f32 = 1.2;
const RIPPLE_THINNING: f32 = 0.25;
const GRAVITY: f32 = 9.81;

/// How far from the camera the water's breaking on the shore is told, in metres. Past
/// about 80 m the swell is drawn flat (`water.wgsl`'s `PATCH_EASE`), and spray there would
/// be too small to see.
const SURF_REACH: f32 = 70.0;
/// How high a crest of the swell must stand over the still water to break as it comes in,
/// in metres. The highest crests are about 0.5 m.
const SURF_CREST: f32 = 0.2;
/// How high a ring must be when it comes to the shore to break there, in metres.
const RING_BREAKS: f32 = 0.006;
/// How big a square of the shore's lookup is, in metres. Only the squares round the camera
/// are looked at each frame.
const BUCKET: f32 = 32.0;

/// How far apart the depth map's texels are, and the points of the shore, in metres, at
/// the most. A Monster Truck Madness 2 track's ground cells are nearly 10 m across, and one
/// texel for each of its vertices put the drawn water's edge metres off on a steep bank:
/// the map is sampled between them, and the ground's triangles are not. The map has at most
/// `MOST_TEXELS` along a side.
const MAP_SPACING: f32 = 2.5;
const MOST_TEXELS: usize = 4096;

/// How deep the still water is over the ground, on a square grid finer than the ground's:
/// what both the depth map and the points of the shore are made from.
struct Depths {
    /// How many samples there are along a side.
    resolution: usize,
    /// Where the first one is, along X and Z, in metres.
    origin: f32,
    /// How far apart they are, in metres.
    spacing: f32,
    /// Row by row: rows along Z and columns along X, as the ground's grid.
    depths: Vec<f32>,
}

impl Depths {
    fn new(heights: &HeightGrid, level: f32) -> Self {
        let size = heights.size();
        let resolution = ((size / MAP_SPACING).ceil() as usize + 1).clamp(2, MOST_TEXELS);
        let origin = -size / 2.0;
        let spacing = size / (resolution - 1) as f32;
        let mut depths = Vec::with_capacity(resolution * resolution);
        for row in 0..resolution {
            for col in 0..resolution {
                let [x, z] = [col, row].map(|index| origin + index as f32 * spacing);
                depths.push(level - heights.height_at(x, z));
            }
        }
        Self {
            resolution,
            origin,
            spacing,
            depths,
        }
    }

    fn at(&self, col: usize, row: usize) -> f32 {
        self.depths[row * self.resolution + col]
    }

    /// Where sample `index` is along X or Z, in metres.
    fn coord(&self, index: usize) -> f32 {
        self.origin + index as f32 * self.spacing
    }
}

/// Where the depth map lies, as `water.wgsl` reads it.
#[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct ShoreUniform {
    /// Where the first texel is, on the ground plane (world X and Z), in metres.
    origin: Vec2,
    /// How far apart the texels are, in metres.
    spacing: f32,
    /// How many texels there are along a side.
    resolution: f32,
    /// 1 where the ground repeats: past an edge of the map, the depth is that inside the
    /// other. 0 where the map ends at its edges.
    repeats: u32,
}

/// The map of how deep the still water at `level` is over the ground, in metres, and where
/// it lies. It holds 32-bit numbers, which the shader reads four at a time and blends
/// itself, since they cannot be filtered: eight bits a depth drew the surf's thin lines of
/// foam, which follow the depth, as a staircase.
pub(super) fn depth_map(heights: &HeightGrid, level: f32) -> (Image, ShoreUniform) {
    let depths = Depths::new(heights, level);
    let resolution = depths.resolution as u32;
    let image = Image::new(
        Extent3d {
            width: resolution,
            height: resolution,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        depths
            .depths
            .iter()
            .flat_map(|depth| depth.to_le_bytes())
            .collect(),
        TextureFormat::R32Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    let uniform = ShoreUniform {
        origin: Vec2::splat(depths.origin),
        spacing: depths.spacing,
        resolution: depths.resolution as f32,
        repeats: heights.repeats() as u32,
    };
    (image, uniform)
}

/// A point on the water's edge.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ShorePoint {
    /// Where it is, on the ground plane (world X and Z), in metres.
    at: Vec2,
    /// Which way is up the bank, of length 1.
    landward: Vec2,
    /// How steep the bank is, in metres of rise for each metre across.
    steepness: f32,
}

/// The points along the water's edge on this race's track, sorted into squares by where
/// they are. Only on a track with water, and only in an app that can draw.
#[derive(Resource)]
pub(super) struct Shore {
    /// Where the first square starts, along X and Z, in metres.
    origin: f32,
    per_side: usize,
    buckets: Vec<Vec<ShorePoint>>,
}

impl Shore {
    fn new(heights: &HeightGrid, level: f32) -> Self {
        let origin = -heights.size() / 2.0;
        let per_side = (heights.size() / BUCKET).ceil().max(1.0) as usize;
        let mut buckets = vec![Vec::new(); per_side * per_side];
        for point in shore_points(heights, &Depths::new(heights, level)) {
            let [col, row] = [point.at.x, point.at.y]
                .map(|along| (((along - origin) / BUCKET) as usize).min(per_side - 1));
            buckets[row * per_side + col].push(point);
        }
        Self {
            origin,
            per_side,
            buckets,
        }
    }

    /// The points within `reach` metres of `center`.
    fn near(&self, center: Vec2, reach: f32) -> impl Iterator<Item = &ShorePoint> {
        let bucket = |along: f32| ((along - self.origin) / BUCKET).floor() as isize;
        let last = self.per_side as isize - 1;
        let cols = bucket(center.x - reach).max(0)..=bucket(center.x + reach).min(last);
        let rows = bucket(center.y - reach).max(0)..=bucket(center.y + reach).min(last);
        rows.flat_map(move |row| cols.clone().map(move |col| (row, col)))
            .flat_map(move |(row, col)| &self.buckets[row as usize * self.per_side + col as usize])
            .filter(move |point| point.at.distance(center) <= reach)
    }
}

/// Where the still water meets the ground: one point on each edge between samples of
/// `depths` that runs from under the water to out of it.
fn shore_points(heights: &HeightGrid, depths: &Depths) -> Vec<ShorePoint> {
    let resolution = depths.resolution;
    let spacing = depths.spacing;
    let mut points = Vec::new();
    let mut add = |at: Vec2, along_edge: Vec2| {
        // Which way is up the bank, from the ground round the point rather than the edge
        // alone, which would only ever point along X or Z.
        let step = spacing / 2.0;
        let slope = Vec2::new(
            heights.height_at(at.x + step, at.y) - heights.height_at(at.x - step, at.y),
            heights.height_at(at.x, at.y + step) - heights.height_at(at.x, at.y - step),
        ) / spacing;
        let landward = slope.try_normalize().unwrap_or(along_edge);
        points.push(ShorePoint {
            at,
            landward,
            steepness: slope.length(),
        });
    };
    for row in 0..resolution {
        for col in 0..resolution {
            let here = depths.at(col, row);
            let corner = Vec2::new(depths.coord(col), depths.coord(row));
            for (next, along) in [
                (
                    (col + 1 < resolution).then(|| depths.at(col + 1, row)),
                    Vec2::X,
                ),
                (
                    (row + 1 < resolution).then(|| depths.at(col, row + 1)),
                    Vec2::Y,
                ),
            ] {
                let Some(there) = next else {
                    continue;
                };
                if (here > 0.0) == (there > 0.0) {
                    continue;
                }
                let share = here / (here - there);
                // From the wet end to the dry one.
                let landward = if here > 0.0 { along } else { -along };
                add(corner + along * share * spacing, landward);
            }
        }
    }
    points
}

/// One of the swell's waves: at a place `position` (world X and Z), at `time` on the
/// shader's clock, it stands `height * sin(travel · position - rate * time)` over the still
/// water.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwellWave {
    /// The way it rolls, on the ground plane, times how many radians it goes through for
    /// each metre that way.
    pub travel: Vec2,
    /// How many radians it goes through each second at any one place.
    pub rate: f32,
    /// How high its crests stand over the still water, in metres.
    pub height: f32,
}

/// The swell's waves, with the wind blowing `towards`. As `swell` in `water.wgsl`: each
/// rolls at the speed of a wave of its length on deep water.
pub(super) fn swell_waves(towards: Vec2) -> [SwellWave; 3] {
    std::array::from_fn(|wave| {
        let direction = Vec2::from_angle(SWELL_ANGLES[wave]).rotate(towards);
        let k = std::f32::consts::TAU / SWELL_LENGTHS[wave];
        SwellWave {
            travel: direction * k,
            rate: (GRAVITY * k).sqrt(),
            height: SWELL_HEIGHTS[wave],
        }
    })
}

/// How high the swell stands over the still water at `position`, `time` seconds in (as
/// `Time::elapsed_secs_wrapped`, which the shader is given), with the wind blowing
/// `towards`.
pub(super) fn swell_height(position: Vec2, time: f32, towards: Vec2) -> f32 {
    swell_waves(towards)
        .iter()
        .map(|wave| wave.height * (wave.travel.dot(position) - wave.rate * time).sin())
        .sum()
}

/// How high a ring of `strength` metres is, `age` seconds after it started, where it has
/// spread to. As `ring` in `water.wgsl`.
fn ring_height(strength: f32, age: f32) -> f32 {
    let radius = RIPPLE_SPEED * age;
    strength * (-age / RIPPLE_FADE).exp() / (1.0 + RIPPLE_THINNING * radius)
}

/// Makes the shore for a track with water.
pub(super) fn find_shore(mut commands: Commands, track: Res<Track>) {
    commands.remove_resource::<Shore>();
    if let Some(level) = track.water_level {
        commands.insert_resource(Shore::new(&track.heights, level));
    }
}

/// Finds where the swell and the trucks' ripples break against the shore near the camera,
/// and tells the spray.
#[allow(clippy::too_many_arguments)]
pub(super) fn find_surf(
    time: Res<Time>,
    track: Res<Track>,
    shore: Option<Res<Shore>>,
    wind: Res<Wind>,
    ripples: Res<Ripples>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    settings: Res<WaterSettings>,
    mut surf: MessageWriter<Surf>,
) {
    let (Some(shore), Some(level)) = (shore, track.water_level) else {
        return;
    };
    // Only spray is made of it.
    if !settings.splashes {
        return;
    }
    let Some(eye) = cameras
        .iter()
        .find(|(camera, _)| camera.is_active)
        .map(|(_, transform)| transform.translation().xz())
    else {
        return;
    };
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    let now = time.elapsed_secs_wrapped();
    let before = now - dt;
    let towards = wind.towards;
    for point in shore.near(eye, SURF_REACH) {
        let height = swell_height(point.at, now, towards);
        let was = swell_height(point.at, before, towards);
        if was < SURF_CREST && height >= SURF_CREST {
            surf.write(Surf {
                at: Vec3::new(point.at.x, level + height, point.at.y),
                // Along the shore as far as the points are apart, so that their spacing
                // doesn't show.
                width: MAP_SPACING,
                inwards: point.landward,
                steepness: point.steepness,
                kind: SurfKind::Swell,
            });
        }
    }

    let elapsed = time.elapsed_secs();
    for ripple in ripples.iter() {
        let age = elapsed - ripple.born;
        let height = ring_height(ripple.strength, age);
        if height < RING_BREAKS {
            continue;
        }
        let reached = RIPPLE_SPEED * age;
        let had_reached = RIPPLE_SPEED * (age - dt).max(0.0);
        let arrived = shore
            .near(ripple.center, reached)
            .filter(|point| point.at.distance(ripple.center) >= had_reached)
            .filter(|point| point.at.distance(eye) <= SURF_REACH);
        for point in arrived {
            let inwards = (point.at - ripple.center)
                .try_normalize()
                .unwrap_or(point.landward);
            surf.write(Surf {
                at: Vec3::new(point.at.x, level, point.at.y),
                width: 0.0,
                inwards,
                steepness: point.steepness,
                kind: SurfKind::Ripple { height },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ground that rises along X at `slope`, meeting water at level 0 at x = 0.
    fn bank(slope: f32) -> HeightGrid {
        HeightGrid::from_fn(33, 64.0, |x, _| x * slope)
    }

    #[test]
    fn the_shader_knows_the_numbers() {
        let shader = include_str!("../shaders/water.wgsl");
        let vec3 = |[a, b, c]: [f32; 3]| format!("vec3({a:?}, {b:?}, {c:?});");
        for line in [
            format!("const SWELL_ANGLES: vec3<f32> = {}", vec3(SWELL_ANGLES)),
            format!("const SWELL_LENGTHS: vec3<f32> = {}", vec3(SWELL_LENGTHS)),
            format!("const SWELL_HEIGHTS: vec3<f32> = {}", vec3(SWELL_HEIGHTS)),
        ] {
            assert!(shader.contains(&line), "water.wgsl lacks `{line}`");
        }
        let field = include_str!("../shaders/field.wgsl");
        let line = format!("const RIPPLE_SPEED: f32 = {RIPPLE_SPEED:?};");
        assert!(field.contains(&line), "field.wgsl lacks `{line}`");
    }

    #[test]
    fn the_map_holds_the_depth_over_the_ground() {
        let depths = Depths::new(&bank(0.5), 0.0);
        // Finer than the ground's grid, whose cells are 2 m here.
        assert_eq!(depths.resolution, 27);
        assert!(depths.spacing <= MAP_SPACING);
        assert_eq!(depths.depths.len(), 27 * 27);
        for col in 0..27 {
            let depth = -0.5 * depths.coord(col);
            assert!((depths.at(col, 5) - depth).abs() < 1e-4, "{col}");
        }
        let (image, uniform) = depth_map(&bank(0.5), 0.0);
        assert_eq!(image.data.as_ref().map(Vec::len), Some(27 * 27 * 4));
        assert_eq!(uniform.origin, Vec2::splat(-32.0));
    }

    #[test]
    fn the_shore_is_where_the_water_meets_the_ground() {
        let heights = bank(0.5);
        let points = shore_points(&heights, &Depths::new(&heights, 0.75));
        // One point on each row, where the ground is 0.75 m high.
        assert_eq!(points.len(), 27);
        for point in &points {
            assert!((point.at.x - 1.5).abs() < 1e-4, "{point:?}");
            assert!((point.landward - Vec2::X).length() < 1e-4, "{point:?}");
            assert!((point.steepness - 0.5).abs() < 1e-4, "{point:?}");
        }
    }

    #[test]
    fn dry_or_drowned_ground_has_no_shore() {
        let heights = bank(0.5);
        for level in [-100.0, 100.0] {
            assert!(shore_points(&heights, &Depths::new(&heights, level)).is_empty());
        }
    }

    #[test]
    fn the_shore_near_a_point_is_found_and_no_more() {
        let shore = Shore::new(&bank(0.5), 0.75);
        assert_eq!(shore.near(Vec2::new(1.5, 0.0), 5.0).count(), 5);
        assert_eq!(shore.near(Vec2::new(20.0, 0.0), 5.0).count(), 0);
        // Reaching past the track's edge.
        assert_eq!(shore.near(Vec2::new(1.5, 30.0), 10.0).count(), 5);
        assert_eq!(shore.near(Vec2::ZERO, 1000.0).count(), 27);
    }

    #[test]
    fn the_swell_rises_and_falls_within_its_height() {
        let highest: f32 = SWELL_HEIGHTS.iter().sum();
        let towards = Vec2::new(0.8, -0.6);
        let heights: Vec<f32> = (0..2000)
            .map(|step| swell_height(Vec2::new(3.0, -7.0), step as f32 * 0.05, towards))
            .collect();
        assert!(heights.iter().all(|height| height.abs() <= highest + 1e-4));
        // Its crests come in high enough to throw spray.
        assert!(heights.iter().any(|&height| height > SURF_CREST));
        assert!(heights.iter().any(|&height| height < -SURF_CREST));
    }

    #[test]
    fn a_ring_fades_as_it_spreads() {
        assert_eq!(ring_height(0.1, 0.0), 0.1);
        assert!(ring_height(0.1, 2.0) < ring_height(0.1, 1.0));
        // A fast truck's wake breaks on a shore close by, and not on one far off.
        assert!(ring_height(0.1, 0.7) >= RING_BREAKS);
        assert!(ring_height(0.1, 3.0) < RING_BREAKS);
    }
}
