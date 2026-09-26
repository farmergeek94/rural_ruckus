//! The route around a track: a closed centreline with a width.
//!
//! How far round the loop each piece of the centreline starts is worked out once, when the
//! course is made, because the computer's drivers ask where they are on it many times a
//! physics step. So the centreline can only be read, not changed.

use bevy::prelude::*;

use super::height_grid::grid_coord;

#[derive(Clone, Debug)]
pub struct Course {
    /// Points along the middle of the course on the ground plane (world X and Z), in
    /// metres. The course is a loop: the last point joins back to the first.
    centerline: Vec<Vec2>,
    /// How far round the loop from the first point each piece of the centreline starts,
    /// in metres, and after them the length of the lap.
    starts: Vec<f32>,
    /// Full drivable width in metres.
    pub width: f32,
}

/// The closest point of the centreline to some position.
#[derive(Clone, Copy, Debug)]
pub struct Nearest {
    /// Distance from the position to the centreline, in metres.
    pub distance: f32,
    /// The closest point lies between centreline points `segment` and `segment + 1`...
    pub segment: usize,
    /// ...this far along, from 0 to 1.
    pub t: f32,
}

impl Course {
    /// The course along `centerline`, `width` metres wide. The centreline needs at least
    /// one point.
    pub fn new(centerline: Vec<Vec2>, width: f32) -> Self {
        let count = centerline.len();
        let mut starts = Vec::with_capacity(count + 1);
        let mut start = 0.0;
        starts.push(start);
        for i in 0..count {
            start += centerline[i].distance(centerline[(i + 1) % count]);
            starts.push(start);
        }
        Self {
            centerline,
            starts,
            width,
        }
    }

    /// Points along the middle of the course on the ground plane (world X and Z), in
    /// metres. The course is a loop: the last point joins back to the first.
    pub fn centerline(&self) -> &[Vec2] {
        &self.centerline
    }

    /// Every straight piece of the centreline, including the one closing the loop.
    pub fn segments(&self) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
        let count = self.centerline.len();
        (0..count).map(move |i| (self.centerline[i], self.centerline[(i + 1) % count]))
    }

    /// Length of one lap along the centreline, in metres.
    pub fn length(&self) -> f32 {
        self.starts[self.centerline.len()]
    }

    /// The ends of piece `segment` of the centreline.
    fn segment(&self, segment: usize) -> (Vec2, Vec2) {
        let count = self.centerline.len();
        (
            self.centerline[segment],
            self.centerline[(segment + 1) % count],
        )
    }

    /// Position and direction of travel `distance` metres along the centreline from
    /// its first point. Wraps around, so any distance is valid.
    pub fn point_at(&self, distance: f32) -> (Vec2, Vec2) {
        let count = self.centerline.len();
        let wanted = distance.rem_euclid(self.length());
        // The first piece that ends at or past it.
        let segment = self.starts[1..].partition_point(|&end| end < wanted);
        if segment >= count {
            // Only reachable through rounding on the very last segment.
            let (a, b) = self.segment(0);
            return (a, (b - a).normalize());
        }
        let (a, b) = self.segment(segment);
        let direction = (b - a) / a.distance(b);
        (a + direction * (wanted - self.starts[segment]), direction)
    }

    /// `point_at` for `count` places `step` metres apart, the first `from` metres along
    /// the centreline.
    pub fn sample_ahead(&self, from: f32, step: f32, count: usize) -> Vec<(Vec2, Vec2)> {
        (0..count)
            .map(|i| self.point_at(from + step * i as f32))
            .collect()
    }

    /// The closest point of the centreline to `position`.
    pub fn nearest(&self, position: Vec2) -> Nearest {
        self.segments()
            .enumerate()
            .map(|(segment, (a, b))| nearest_on_segment(position, a, b, segment))
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
            .expect("a course has points")
    }

    /// How far along the centreline from its first point a `nearest` is, in metres.
    pub fn distance_along(&self, nearest: &Nearest) -> f32 {
        let start = self.starts[nearest.segment];
        start + (self.starts[nearest.segment + 1] - start) * nearest.t
    }

    /// `nearest`, looking only at the stretch of the centreline from `from` to `to` metres
    /// along it, which wraps round the loop as `point_at` does. For following something
    /// that is known to be about there: where a course doubles back beside itself, the
    /// closest point of all can be on the wrong stretch. Pieces are taken whole, so the
    /// result can lie a little outside the stretch.
    pub fn nearest_within(&self, position: Vec2, from: f32, to: f32) -> Nearest {
        let lap = self.length();
        let stretch = to - from;
        self.segments()
            .enumerate()
            .filter_map(|(segment, (a, b))| {
                let start = self.starts[segment];
                let length = self.starts[segment + 1] - start;
                let begins_in_stretch = (start - from).rem_euclid(lap) <= stretch;
                let holds_the_start = (from - start).rem_euclid(lap) < length;
                (begins_in_stretch || holds_the_start)
                    .then(|| nearest_on_segment(position, a, b, segment))
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
            // A stretch always holds the piece it starts in. NaN distances find none.
            .unwrap_or_else(|| self.nearest(position))
    }

    /// `nearest` for every vertex of a height grid at once, row by row, for vertices
    /// within `reach` metres of the centreline. Much cheaper than asking per vertex,
    /// because each segment only visits the vertices around it.
    pub(super) fn nearest_per_vertex(
        &self,
        resolution: usize,
        size: f32,
        reach: f32,
    ) -> Vec<Option<Nearest>> {
        let mut result: Vec<Option<Nearest>> = vec![None; resolution * resolution];
        let cells = (resolution - 1) as f32;
        let index_range = |low: f32, high: f32| {
            let to_index = |world: f32| ((world / size + 0.5) * cells).clamp(0.0, cells);
            to_index(low - reach).floor() as usize..=to_index(high + reach).ceil() as usize
        };

        for (segment, (a, b)) in self.segments().enumerate() {
            for row in index_range(a.y.min(b.y), a.y.max(b.y)) {
                for col in index_range(a.x.min(b.x), a.x.max(b.x)) {
                    let vertex = Vec2::new(
                        grid_coord(col, resolution, size),
                        grid_coord(row, resolution, size),
                    );
                    let candidate = nearest_on_segment(vertex, a, b, segment);
                    let slot = &mut result[row * resolution + col];
                    let closer = slot.is_none_or(|best| candidate.distance < best.distance);
                    if candidate.distance <= reach && closer {
                        *slot = Some(candidate);
                    }
                }
            }
        }
        result
    }
}

fn nearest_on_segment(position: Vec2, a: Vec2, b: Vec2, segment: usize) -> Nearest {
    let along = b - a;
    let t = ((position - a).dot(along) / along.length_squared()).clamp(0.0, 1.0);
    Nearest {
        distance: position.distance(a + along * t),
        segment,
        t,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 100 m square, driven anticlockwise on paper (X right, Z up).
    fn square() -> Course {
        Course::new(
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(100.0, 0.0),
                Vec2::new(100.0, 100.0),
                Vec2::new(0.0, 100.0),
            ],
            10.0,
        )
    }

    #[test]
    fn length_includes_the_closing_segment() {
        assert_eq!(square().length(), 400.0);
    }

    #[test]
    fn point_at_walks_the_loop_and_wraps() {
        let course = square();
        let (position, direction) = course.point_at(150.0);
        assert_eq!(position, Vec2::new(100.0, 50.0));
        assert_eq!(direction, Vec2::Y);

        let (position, direction) = course.point_at(400.0 + 350.0);
        assert_eq!(position, Vec2::new(0.0, 50.0));
        assert_eq!(direction, Vec2::NEG_Y);
    }

    #[test]
    fn sample_ahead_agrees_with_point_at_round_the_loop_and_over_its_end() {
        let course = square();
        // Off the corners, where either neighbouring piece is a fair answer.
        let samples = course.sample_ahead(372.5, 15.0, 12);
        assert_eq!(samples.len(), 12);
        for (i, (position, direction)) in samples.iter().enumerate() {
            let (wanted, wanted_direction) = course.point_at(372.5 + 15.0 * i as f32);
            assert!(position.distance(wanted) < 1e-3, "sample {i}: {position}");
            assert_eq!(*direction, wanted_direction, "sample {i}");
        }
    }

    #[test]
    fn nearest_finds_the_closest_segment() {
        let nearest = square().nearest(Vec2::new(30.0, 104.0));
        assert_eq!(nearest.segment, 2);
        assert!((nearest.distance - 4.0).abs() < 1e-5);
        assert!((nearest.t - 0.7).abs() < 1e-5);
    }

    #[test]
    fn distance_along_is_the_inverse_of_point_at() {
        let course = square();
        for distance in [0.0, 30.0, 150.0, 399.0] {
            let (position, _) = course.point_at(distance);
            let along = course.distance_along(&course.nearest(position));
            assert!((along - distance).abs() < 1e-3, "{distance}: {along}");
        }
    }

    #[test]
    fn nearest_within_keeps_to_its_stretch() {
        let course = square();
        // Just inside the first side, and nearer to the last one.
        let position = Vec2::new(1.0, 3.0);
        assert_eq!(course.nearest(position).segment, 3);
        assert_eq!(course.nearest_within(position, 0.0, 50.0).segment, 0);
        // A stretch over the end of the loop takes in both.
        assert_eq!(course.nearest_within(position, 380.0, 430.0).segment, 3);
        assert_eq!(course.nearest_within(position, -20.0, 30.0).segment, 3);
        // One that begins part of the way along a piece still holds that piece.
        assert_eq!(course.nearest_within(position, 150.0, 160.0).segment, 1);
    }

    #[test]
    fn per_vertex_agrees_with_nearest() {
        let course = square();
        let (resolution, size, reach) = (41, 400.0, 25.0);
        let per_vertex = course.nearest_per_vertex(resolution, size, reach);
        for row in 0..resolution {
            for col in 0..resolution {
                let vertex = Vec2::new(
                    grid_coord(col, resolution, size),
                    grid_coord(row, resolution, size),
                );
                let expected = course.nearest(vertex);
                match per_vertex[row * resolution + col] {
                    Some(found) => assert!((found.distance - expected.distance).abs() < 1e-4),
                    None => assert!(expected.distance > reach, "missed vertex {vertex}"),
                }
            }
        }
    }
}
