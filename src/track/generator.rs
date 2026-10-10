//! The built-in track, generated in code. It stands in for tracks loaded from content
//! packs, and is the only place that knows this track is procedural: the rest of the
//! game sees a `TrackData` like any other.

use bevy::prelude::*;

use super::direction_yaw;
use super::{Course, Footing, Gate, HeightGrid, Scenery, StartPosition, TrackData};

/// Side length of the (square) terrain, in metres.
const SIZE: f32 = 512.0;
/// Number of vertices along each side. 257 gives 2 m cells.
const RESOLUTION: usize = 257;

/// Full width of the course, in metres. The truck is just under 4 m wide.
const COURSE_WIDTH: f32 = 14.0;
/// Distance over which the flattened course blends back into the hills, in metres.
const SHOULDER: f32 = 8.0;
/// Spacing of the centreline points sampled from the spline, in metres.
const CENTERLINE_SPACING: f32 = 4.0;
/// Width of the soft edge between the course surface and the grass, in metres.
const COURSE_EDGE: f32 = 2.0;

const GATE_COUNT: usize = 6;
const GATE_HALF_WIDTH: f32 = 10.0;
/// How far past the start position the start/finish line is, in metres.
const START_LINE_DISTANCE: f32 = 10.0;

/// Places on the starting grid, pole position included, as a Monster Truck Madness 2 grid has.
const GRID_PLACES: usize = 8;
/// How far behind the place in front each place of the grid is, in metres. The truck is
/// under 6 m long.
const GRID_SPACING: f32 = 7.0;
/// How far to the side of the centreline the places behind pole position are, in metres,
/// left and right by turns. The truck is just under 4 m wide.
const GRID_STAGGER: f32 = 3.0;

/// The course passes through these points on the ground plane (X, Z), in order, and
/// then returns to the first. It starts at the origin heading -Z. The first three are
/// in line so that the start straight, with the ramp on it, really is straight.
const CONTROL_POINTS: [Vec2; 14] = [
    Vec2::new(0.0, 0.0),
    Vec2::new(0.0, -60.0),
    Vec2::new(0.0, -120.0),
    Vec2::new(-40.0, -190.0),
    Vec2::new(-120.0, -210.0),
    Vec2::new(-195.0, -160.0),
    Vec2::new(-200.0, -70.0),
    Vec2::new(-130.0, -10.0),
    Vec2::new(-150.0, 70.0),
    Vec2::new(-200.0, 140.0),
    Vec2::new(-150.0, 205.0),
    Vec2::new(-60.0, 190.0),
    Vec2::new(-10.0, 130.0),
    Vec2::new(0.0, 60.0),
];

pub fn builtin_track() -> TrackData {
    let course = Course::new(
        sample_closed_spline(&CONTROL_POINTS, CENTERLINE_SPACING),
        COURSE_WIDTH,
    );
    let heights = course_heights(&course);

    let half_width = course.width / 2.0;
    let surface = course
        .nearest_per_vertex(RESOLUTION, SIZE, half_width + COURSE_EDGE)
        .into_iter()
        .map(|nearest| match nearest {
            Some(nearest) => {
                1.0 - smoothstep(half_width, half_width + COURSE_EDGE, nearest.distance)
            }
            None => 0.0,
        })
        .collect();

    let lap_length = course.length();
    let gates = (0..GATE_COUNT)
        .map(|index| {
            let distance = START_LINE_DISTANCE + lap_length * index as f32 / GATE_COUNT as f32;
            let (center, direction) = course.point_at(distance);
            Gate {
                center,
                yaw: direction_yaw(direction),
                half_width: GATE_HALF_WIDTH,
            }
        })
        .collect();

    let (start, start_direction) = course.point_at(0.0);
    let grid = (1..GRID_PLACES)
        .map(|place| {
            let (center, direction) = course.point_at(-GRID_SPACING * place as f32);
            let side = if place % 2 == 1 { -1.0 } else { 1.0 };
            StartPosition {
                position: center + direction.perp() * GRID_STAGGER * side,
                yaw: direction_yaw(direction),
            }
        })
        .collect();
    TrackData {
        name: "Rolling Hills".into(),
        heights,
        surface,
        ground: None,
        // One cell for the whole track: it is all dirt.
        footing: vec![Footing::Loose],
        scenery: Scenery::default(),
        backdrop: None,
        skies: Default::default(),
        ground_boxes: Vec::new(),
        water_level: None,
        course: Some(course),
        other_courses: Vec::new(),
        gates,
        start: StartPosition {
            position: start,
            yaw: direction_yaw(start_direction),
        },
        grid,
    }
}

/// The hills, with the ground flattened across the course so it has no side slope.
fn course_heights(course: &Course) -> HeightGrid {
    let half_width = course.width / 2.0;
    let nearest = course.nearest_per_vertex(RESOLUTION, SIZE, half_width + SHOULDER);
    let mut nearest = nearest.into_iter();

    // `from_fn` visits vertices in the same row-by-row order as `nearest_per_vertex`.
    HeightGrid::from_fn(RESOLUTION, SIZE, |x, z| {
        let mut height = hills(x, z);
        if let Some(nearest) = nearest.next().expect("one entry per vertex") {
            // Across the course, use the height of the hills under the centreline.
            let a = course.centerline()[nearest.segment];
            let b = course.centerline()[(nearest.segment + 1) % course.centerline().len()];
            let on_course = hills(a.x, a.y).lerp(hills(b.x, b.y), nearest.t);
            let blend = smoothstep(half_width, half_width + SHOULDER, nearest.distance);
            height = on_course.lerp(height, blend);
        }
        height + ramp(x, z)
    })
}

fn hills(x: f32, z: f32) -> f32 {
    let hills = 6.0 * (x * 0.02).sin() * (z * 0.025).cos()
        + 2.5 * (x * 0.07 + 1.3).sin() * (z * 0.06).sin()
        + 0.6 * (x * 0.21).sin() * (z * 0.19).cos();

    // Keep the spawn area flat, blending into the hills further out.
    let distance = Vec2::new(x, z).length();
    hills * smoothstep(15.0, 60.0, distance)
}

/// A kicker ramp on the start straight, straight ahead of the start position.
fn ramp(x: f32, z: f32) -> f32 {
    const START_Z: f32 = -40.0;
    const LENGTH: f32 = 16.0;
    const HALF_WIDTH: f32 = 8.0;
    const HEIGHT: f32 = 4.0;

    let along = (START_Z - z) / LENGTH;
    if x.abs() <= HALF_WIDTH && (0.0..=1.0).contains(&along) {
        along * HEIGHT
    } else {
        0.0
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Points roughly `spacing` metres apart along the closed Catmull-Rom spline through
/// `points`. The result is a loop too: its last point is one step before its first.
fn sample_closed_spline(points: &[Vec2], spacing: f32) -> Vec<Vec2> {
    let count = points.len();
    let mut samples = Vec::new();
    for i in 0..count {
        let p0 = points[(i + count - 1) % count];
        let p1 = points[i];
        let p2 = points[(i + 1) % count];
        let p3 = points[(i + 2) % count];

        let steps = (p1.distance(p2) / spacing).ceil().max(1.0) as usize;
        for step in 0..steps {
            let t = step as f32 / steps as f32;
            samples.push(
                0.5 * (2.0 * p1
                    + (p2 - p0) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t * t * t),
            );
        }
    }
    samples
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::yaw_direction;

    #[test]
    fn spline_passes_through_its_control_points() {
        let samples = sample_closed_spline(&CONTROL_POINTS, CENTERLINE_SPACING);
        for point in CONTROL_POINTS {
            assert!(samples.contains(&point), "missing {point}");
        }
    }

    #[test]
    fn builtin_track_is_valid() {
        let track = builtin_track();
        let grid = &track.heights;
        let half_size = grid.size() / 2.0;
        let course = track
            .course
            .as_ref()
            .expect("the built-in track has a course");
        assert_eq!(track.surface.len(), grid.resolution() * grid.resolution());

        for row in 0..grid.resolution() {
            for col in 0..grid.resolution() {
                assert!(grid.vertex(col, row).is_finite());
            }
        }

        // The course is a sensible length and keeps clear of the edge of the terrain.
        let lap_length = course.length();
        assert!(
            (800.0..1500.0).contains(&lap_length),
            "lap is {lap_length} m"
        );
        for point in course.centerline() {
            assert!(
                point.abs().max_element() < half_size - 30.0,
                "{point} near the edge"
            );
        }

        // Gates sit on the centreline, face along it, and come in lap order.
        assert_eq!(track.gates.len(), GATE_COUNT);
        let mut previous_distance = -1.0;
        for gate in &track.gates {
            let nearest = course.nearest(gate.center);
            assert!(nearest.distance < 0.01, "gate off the centreline: {gate:?}");

            let a = course.centerline()[nearest.segment];
            let b = course.centerline()[(nearest.segment + 1) % course.centerline().len()];
            assert!(gate.direction().dot((b - a).normalize()) > 0.99);
            assert!(gate.half_width * 2.0 >= course.width);

            let distance: f32 = course
                .segments()
                .take(nearest.segment)
                .map(|(a, b)| a.distance(b))
                .sum::<f32>()
                + a.distance(gate.center);
            assert!(distance > previous_distance, "gates out of order");
            previous_distance = distance;
        }

        // The truck starts at the origin facing -Z, a little behind the start line.
        assert_eq!(track.start.position, Vec2::ZERO);
        assert!((yaw_direction(track.start.yaw) - Vec2::NEG_Y).length() < 1e-5);
        let start_line = &track.gates[0];
        let behind = (track.start.position - start_line.center).dot(start_line.direction());
        assert!(
            (-20.0..-5.0).contains(&behind),
            "start is {behind} m past the line"
        );
    }

    #[test]
    fn the_grid_lines_up_behind_pole_position_on_the_course() {
        let track = builtin_track();
        let course = track.course.as_ref().unwrap();
        assert_eq!(track.grid.len(), GRID_PLACES - 1);

        let places: Vec<_> = (0..GRID_PLACES)
            .map(|place| track.grid_place(place))
            .collect();
        let ahead = yaw_direction(track.start.yaw);
        for (index, place) in places.iter().enumerate() {
            // With room for the truck's 3.8 m between the course's edges.
            let off_centre = course.nearest(place.position).distance;
            assert!(off_centre + 1.9 < course.width / 2.0, "place {index}");
            assert!(yaw_direction(place.yaw).dot(ahead) > 0.95, "place {index}");
            for other in &places[..index] {
                assert!((other.position - place.position).dot(ahead) > 0.0);
                // Further apart than a truck is long, corner to corner.
                assert!(
                    other.position.distance(place.position) > 6.5,
                    "place {index}"
                );
            }
        }
    }

    #[test]
    fn ground_is_level_across_the_course_and_flat_at_the_start() {
        let track = builtin_track();
        for offset in [-10.0, -3.0, 0.0, 3.0, 10.0] {
            assert_eq!(track.heights.height_at(offset, offset), 0.0);
        }

        // Half way round, well away from the flat start area and the ramp.
        let course = track.course.as_ref().unwrap();
        let (center, direction) = course.point_at(course.length() / 2.0);
        let side = direction.perp() * (COURSE_WIDTH / 2.0 - 2.0);
        let (left, right) = (center + side, center - side);
        let difference =
            track.heights.height_at(left.x, left.y) - track.heights.height_at(right.x, right.y);
        assert!(difference.abs() < 0.3, "side slope of {difference} m");
    }

    #[test]
    fn the_built_in_track_is_dirt() {
        let track = builtin_track();
        assert!(track.loose_at(-track.heights.size() / 4.0, 0.0));
    }
}
