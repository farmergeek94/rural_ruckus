//! A picture of a track from above, drawn from its own data: the ground, the course over
//! it and the gates as ticks across the course, the start line marked, and water blue
//! over the ground it covers. It needs nothing a
//! track doesn't have, so it works for every track there is, and says more about a course
//! than a screenshot would. North (-Z) is up, and the picture is framed on the course: a
//! Monster Truck Madness 2 world is 2.5 km across and its course often fills a corner of it.
//!
//! Plain data in and bytes out. Whoever wants it on screen makes an image of it.

use bevy::prelude::*;

use super::TrackData;
use super::mesh::ground_color;

/// The course, the gates across it, and the start line, as red, green and blue.
pub const MAP_COURSE: [u8; 3] = [255, 200, 12];
pub const MAP_GATE: [u8; 3] = [40, 40, 40];
pub const MAP_START: [u8; 3] = [255, 255, 255];

/// The colour of water on the map, as red, green and blue. Near the colour the water is
/// drawn in (`water/surface.rs`), lightened, as a printed map's water is.
pub const MAP_WATER: [u8; 3] = [48, 112, 160];
/// How much of the water's colour covers the ground at the water's edge, and at
/// `OPAQUE_DEPTH` metres and deeper. In between it goes smoothly, so that shallows show
/// the ground through them and a lake reads as a lake.
const SHALLOW_WATER: f32 = 0.45;
const OPAQUE_DEPTH: f32 = 4.0;

/// The course is drawn at its own width, but never thinner than this many pixels, or the
/// course of a big track would vanish.
const THINNEST_COURSE: f32 = 2.5;
/// A gate's tick reaches this much beyond its own width, as a share of it, and is never
/// shorter than this many pixels from its middle to its end.
const TICK_OVERHANG: f32 = 0.6;
const SHORTEST_TICK: f32 = 4.0;
/// How much ground to show around the course, as a share of the course's own extent.
const MARGIN_AROUND_COURSE: f32 = 0.12;
/// Where the light that gives the ground its relief comes from. From the north west, as
/// on a printed map, and high, so that only steep ground is dark.
const LIGHT: Vec3 = Vec3::new(-0.45, 0.75, -0.45);
/// How far the height is sampled either side of a pixel to find its slope, in pixels.
const SLOPE_REACH: f32 = 0.75;

/// A square picture of the track, `size` pixels along each side, as red, green, blue and
/// alpha bytes, row by row from the top.
pub fn map_image(track: &TrackData, size: usize) -> Vec<u8> {
    let (middle, span) = frame(track);
    let mut map = Map {
        size,
        middle,
        span,
        pixels: vec![255; size * size * 4],
    };

    // Each tile of a textured track is one colour at this scale: its average.
    let tile_colors: Option<Vec<Vec3>> = track.ground.as_ref().map(|ground| {
        ground
            .tiles
            .iter()
            .map(|tile| average_color(tile))
            .collect()
    });

    let reach = map.span / size as f32 * SLOPE_REACH;
    let light = LIGHT.normalize();
    for row in 0..size {
        for col in 0..size {
            let at = map.to_world(Vec2::new(col as f32 + 0.5, row as f32 + 0.5));
            let height = |dx: f32, dz: f32| track.heights.height_at(at.x + dx, at.y + dz);
            let normal = Vec3::new(
                height(-reach, 0.0) - height(reach, 0.0),
                2.0 * reach,
                height(0.0, -reach) - height(0.0, reach),
            )
            .normalize();

            let color = match (&track.ground, &tile_colors) {
                (Some(ground), Some(colors)) => {
                    let cells = ground.cells_per_side();
                    let cell = |world: f32| {
                        let across = (world / track.heights.size() + 0.5).clamp(0.0, 1.0);
                        ((across * cells as f32) as usize).min(cells - 1)
                    };
                    colors[ground.cells[cell(at.y) * cells + cell(at.x)].tile]
                }
                _ => {
                    let shade = Color::from(ground_color(height(0.0, 0.0), normal, 0.0));
                    shade.to_srgba().to_vec3()
                }
            };
            // Relief: full brightness on the level, darker facing away from the light.
            let relief = 0.55 + 0.6 * normal.dot(light).clamp(0.0, 1.0);
            let mut color = color * relief;
            // The water is level, so it has no relief of its own.
            if let Some(level) = track.water_level {
                let depth = level - height(0.0, 0.0);
                if depth > 0.0 {
                    let cover =
                        SHALLOW_WATER + (1.0 - SHALLOW_WATER) * smoothstep(depth / OPAQUE_DEPTH);
                    color = color.lerp(rgb(MAP_WATER), cover);
                }
            }
            map.set(col, row, (color * 255.0).to_array().map(|c| c as u8));
        }
    }

    let pixel = map.span / size as f32;
    if let Some(course) = &track.course {
        let radius = (course.width / 2.0 / pixel).max(THINNEST_COURSE / 2.0);
        for (from, to) in course.segments() {
            map.line(from, to, radius, MAP_COURSE);
            // A piece that runs off the map comes back on at the other side.
            let back = track.heights.onto(to) - to;
            if back != Vec2::ZERO {
                map.line(from + back, to + back, radius, MAP_COURSE);
            }
        }
    }
    // The start line last, so that nothing is drawn over it.
    for (index, gate) in track.gates.iter().enumerate().rev() {
        let reach = (gate.half_width * (1.0 + TICK_OVERHANG)).max(SHORTEST_TICK * pixel);
        let (color, radius) = if index == 0 {
            (MAP_START, 1.5)
        } else {
            (MAP_GATE, 1.0)
        };
        let across = gate.across() * reach;
        map.line(gate.center - across, gate.center + across, radius, color);
    }

    map.pixels
}

/// The square of the world that `map_image` shows of a track, for whatever is put over the
/// picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapFrame {
    /// The place in the world (X and Z) at the middle of the picture, in metres.
    pub middle: Vec2,
    /// The length of each side of the picture in the world, in metres.
    pub span: f32,
}

impl MapFrame {
    /// Where `world` (X and Z) is on the picture: (0, 0) at its top left and (1, 1) at its
    /// bottom right. Outside that it is off the picture.
    pub fn place(&self, world: Vec2) -> Vec2 {
        (world - self.middle) / self.span + 0.5
    }
}

/// The square of the world that `map_image` shows of `track`.
pub fn map_frame(track: &TrackData) -> MapFrame {
    let (middle, span) = frame(track);
    MapFrame { middle, span }
}

/// The square of the world to show: its middle, and the metres along each side. Around
/// the course, the gates and the start where there is a course, and otherwise everything.
fn frame(track: &TrackData) -> (Vec2, f32) {
    let Some(course) = &track.course else {
        return (Vec2::ZERO, track.heights.size());
    };
    let points = course
        .centerline()
        .iter()
        .copied()
        .chain(track.gates.iter().map(|gate| gate.center))
        .chain([track.start.position]);
    let (low, high) = points.fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(low, high), point| (low.min(point), high.max(point)),
    );
    let world = track.heights.size();
    let extent = (high - low).max_element() + course.width;
    let span = (extent * (1.0 + 2.0 * MARGIN_AROUND_COURSE)).min(world);
    // Slid back inside the world where the margin would hang over its edge.
    let room = Vec2::splat((world - span) / 2.0);
    (low.midpoint(high).clamp(-room, room), span)
}

struct Map {
    size: usize,
    /// The place in the world at the middle of the picture, and the metres along each of
    /// its sides.
    middle: Vec2,
    span: f32,
    pixels: Vec<u8>,
}

impl Map {
    /// The picture's top left is its north west.
    fn to_world(&self, pixel: Vec2) -> Vec2 {
        self.middle + (pixel / self.size as f32 - 0.5) * self.span
    }

    fn to_pixel(&self, world: Vec2) -> Vec2 {
        ((world - self.middle) / self.span + 0.5) * self.size as f32
    }

    fn set(&mut self, col: usize, row: usize, color: [u8; 3]) {
        let index = (row * self.size + col) * 4;
        self.pixels[index..index + 3].copy_from_slice(&color);
    }

    /// A line between two places in the world, `radius` pixels thick either side.
    fn line(&mut self, from: Vec2, to: Vec2, radius: f32, color: [u8; 3]) {
        let (from, to) = (self.to_pixel(from), self.to_pixel(to));
        // A disc stamped every half pixel along it.
        let stamps = (from.distance(to) * 2.0).ceil().max(1.0) as usize;
        let reach = radius.ceil() as i32;
        for stamp in 0..=stamps {
            let centre = from.lerp(to, stamp as f32 / stamps as f32);
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let (col, row) = (centre.x as i32 + dx, centre.y as i32 + dy);
                    let middle = Vec2::new(col as f32 + 0.5, row as f32 + 0.5);
                    let inside = (0..self.size as i32).contains(&col)
                        && (0..self.size as i32).contains(&row);
                    if inside && middle.distance(centre) <= radius.max(0.71) {
                        self.set(col as usize, row as usize, color);
                    }
                }
            }
        }
    }
}

fn rgb(color: [u8; 3]) -> Vec3 {
    Vec3::from_array(color.map(f32::from)) / 255.0
}

/// 0 at 0 and below, 1 at 1 and above, and an S curve between.
fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The average of a tile's pixels, from 0 to 1. Averaged as the bytes stand, which is good
/// enough for a colour on a map. Holes count for nothing.
fn average_color(rgba: &[u8]) -> Vec3 {
    let (mut sum, mut count) = (Vec3::ZERO, 0.0);
    for pixel in rgba.as_chunks::<4>().0 {
        if pixel[3] > 0 {
            sum += Vec3::new(pixel[0] as f32, pixel[1] as f32, pixel[2] as f32);
            count += 1.0;
        }
    }
    sum / (count * 255.0_f32).max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::builtin_track;

    const SIZE: usize = 256;

    fn pixel_at(map: &[u8], track: &TrackData, world: Vec2) -> [u8; 3] {
        let (middle, span) = frame(track);
        let pixel = ((world - middle) / span + 0.5) * SIZE as f32;
        let index = (pixel.y as usize * SIZE + pixel.x as usize) * 4;
        [map[index], map[index + 1], map[index + 2]]
    }

    #[test]
    fn the_course_is_drawn_where_the_course_is() {
        let track = builtin_track();
        let map = map_image(&track, SIZE);
        assert_eq!(map.len(), SIZE * SIZE * 4);
        assert!(map.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 255));

        let course = track.course.as_ref().unwrap();
        let mut on_course = 0;
        let mut points = 0;
        let mut distance = 0.0;
        while distance < course.length() {
            let (point, _) = course.point_at(distance);
            // A gate's tick is drawn over the course, so only count points clear of them.
            let clear_of_gates = track
                .gates
                .iter()
                .all(|gate| gate.center.distance(point) > gate.half_width * 3.0);
            if clear_of_gates {
                points += 1;
                on_course += (pixel_at(&map, &track, point) == MAP_COURSE) as usize;
            }
            distance += 5.0;
        }
        assert!(points > 20);
        assert_eq!(on_course, points);

        // And nowhere else: a corner of the picture is a long way from any course.
        let (middle, span) = frame(&track);
        let corner = middle + Vec2::splat(span * 0.49);
        assert!(course.nearest(corner).distance > course.width * 2.0);
        let ground = pixel_at(&map, &track, corner);
        assert!(![MAP_COURSE, MAP_GATE, MAP_START].contains(&ground));
    }

    #[test]
    fn the_start_line_is_marked_and_the_other_gates_are_not() {
        let track = builtin_track();
        let map = map_image(&track, SIZE);
        assert_eq!(pixel_at(&map, &track, track.gates[0].center), MAP_START);
        assert!(track.gates.len() > 1);
        for gate in &track.gates[1..] {
            assert_eq!(pixel_at(&map, &track, gate.center), MAP_GATE);
        }
    }

    #[test]
    fn the_frame_places_things_where_the_picture_draws_them() {
        let track = builtin_track();
        let map = map_image(&track, SIZE);
        let frame = map_frame(&track);
        for (index, gate) in track.gates.iter().enumerate() {
            let pixel = (frame.place(gate.center) * SIZE as f32).as_uvec2();
            let at = (pixel.y as usize * SIZE + pixel.x as usize) * 4;
            let expected = if index == 0 { MAP_START } else { MAP_GATE };
            assert_eq!(map[at..at + 3], expected, "gate {index}");
        }
    }

    #[test]
    fn ground_under_the_water_is_water_coloured_and_the_deeper_the_more() {
        let dry = builtin_track();
        let (middle, span) = frame(&dry);
        // Every pixel's ground, lowest first, clear of the course and the gates.
        let dry_map = map_image(&dry, SIZE);
        let mut ground: Vec<(f32, Vec2)> = (0..SIZE * SIZE)
            .map(|index| {
                let pixel = Vec2::new((index % SIZE) as f32 + 0.5, (index / SIZE) as f32 + 0.5);
                let world = middle + (pixel / SIZE as f32 - 0.5) * span;
                (dry.heights.height_at(world.x, world.y), world)
            })
            .filter(|(_, world)| {
                ![MAP_COURSE, MAP_GATE, MAP_START].contains(&pixel_at(&dry_map, &dry, *world))
            })
            .collect();
        ground.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (lowest, deep) = ground[0];
        let (highest, above) = *ground.last().unwrap();

        let mut wet = dry.clone();
        wet.water_level = Some(lowest + OPAQUE_DEPTH + 1.0);
        assert!(wet.water_level.unwrap() < highest);
        let wet_map = map_image(&wet, SIZE);

        assert_eq!(pixel_at(&wet_map, &wet, deep), MAP_WATER);
        assert_eq!(
            pixel_at(&wet_map, &wet, above),
            pixel_at(&dry_map, &dry, above)
        );
        // A shallow place is water-coloured, but not as much as the deep one.
        let shallow_height = wet.water_level.unwrap() - OPAQUE_DEPTH / 4.0;
        let (_, shallow) = *ground
            .iter()
            .min_by(|a, b| {
                (a.0 - shallow_height)
                    .abs()
                    .total_cmp(&(b.0 - shallow_height).abs())
            })
            .unwrap();
        let (was, is) = (
            rgb(pixel_at(&dry_map, &dry, shallow)),
            rgb(pixel_at(&wet_map, &wet, shallow)),
        );
        let water = rgb(MAP_WATER);
        assert!(is.distance(water) < was.distance(water));
        assert_ne!(pixel_at(&wet_map, &wet, shallow), MAP_WATER);
    }

    #[test]
    fn a_textured_track_is_painted_in_its_tiles_colours() {
        let tile = [vec![200, 0, 0, 255], vec![0, 0, 200, 255]].concat();
        assert_eq!(average_color(&tile), Vec3::new(100.0, 0.0, 100.0) / 255.0);
        // A hole is no colour at all.
        let holed = [vec![200, 100, 0, 255], vec![9, 9, 9, 0]].concat();
        assert_eq!(average_color(&holed), Vec3::new(200.0, 100.0, 0.0) / 255.0);
    }
}
