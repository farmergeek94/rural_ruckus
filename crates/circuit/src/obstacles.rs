//! Fixed scenery on the road, which a driver steers round as it does round a truck that is
//! stopped there: the outline of every fixed, solid object near the course at the heights a
//! truck would hit it, found once a race, and the stretch of it ahead of a driver each step.

use std::collections::HashSet;

use bevy::math::{Quat, Vec2, Vec3, Vec3Swizzles};
use content::track::{Course, Scenery, SceneryMotion};

/// Heights above the ground, in metres, at which a fixed object is cut to find where a truck
/// would hit it: its outline at a truck's bumpers and at its body. A part of a model above
/// both, such as the span of an arch, is over a truck and not in its way.
const OBSTACLE_SLICES: [f32; 2] = [0.5, 1.5];
/// Most metres between the points of an obstacle's outline that a driver steers round.
/// Closer is more points to check each step.
const OBSTACLE_SPACING: f32 = 1.0;
/// How far outside the edge of the course, in metres, a fixed object is still steered round:
/// a truck on a line near the edge reaches out past it.
const OBSTACLE_MARGIN: f32 = 2.0;

/// A point of the outline of a fixed object that reaches into the course, which the drivers
/// steer round as they do round a truck that is stopped there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Obstacle {
    /// How far along the course it is, in metres.
    pub(crate) along: f32,
    /// How far to the left of the course's centreline it is, in metres, as a driver's line is
    /// measured.
    pub(crate) across: f32,
    /// Where it is on the ground plane (world X and Z), in metres.
    pub(crate) position: Vec2,
}

/// Every point, `OBSTACLE_SPACING` apart, of the outline of each fixed, solid scenery object
/// at the heights a truck would hit it (`OBSTACLE_SLICES`), that is within `OBSTACLE_MARGIN`
/// of the edge of `course`, in order along it. Loose objects are left out: a truck knocks
/// them away, and so are ramps, which it drives up. Worked out once a race: it cuts every
/// triangle of every model near the course.
pub(crate) fn obstacles_on(scenery: &Scenery, course: &Course) -> Vec<Obstacle> {
    let band = course.width / 2.0 + OBSTACLE_MARGIN;
    let models = &scenery.models;
    // Where a point has been looked at already, on a grid `OBSTACLE_SPACING` across, so that
    // the two slices of a wall, the faces either side of an edge, and two objects that
    // touch, give one point.
    let mut seen = HashSet::new();
    let mut obstacles = Vec::new();
    for object in &scenery.objects {
        if !object.solid || object.ramp || object.motion != SceneryMotion::Fixed {
            continue;
        }
        let Some(model) = models.get(object.model) else {
            continue;
        };
        // Too far from the course for any of it to reach.
        let reach = model
            .positions
            .iter()
            .map(|&[x, _, z]| Vec2::new(x, z).length())
            .fold(0.0, f32::max);
        if course.nearest(object.position).distance > band + reach {
            continue;
        }
        // Placed as the scenery slice places it, with the object settled onto the slope.
        let rotation = Quat::from_rotation_y(object.yaw);
        let base = Vec3::Y * (object.height_above_ground - object.sunk_on_slope);
        let corner = |index: &u32| {
            let position = model.positions.get(*index as usize)?;
            Some(rotation * Vec3::from_array(*position) + base)
        };
        // Its outline where it reaches the course, a point to each cell of the grid.
        let mut cells = HashSet::new();
        let mut outline = Vec::new();
        for [a, b, c] in model.indices.as_chunks::<3>().0 {
            let (Some(a), Some(b), Some(c)) = (corner(a), corner(b), corner(c)) else {
                continue;
            };
            for height in OBSTACLE_SLICES {
                let Some((from, to)) = slice([a, b, c], height) else {
                    continue;
                };
                let steps = (from.distance(to) / OBSTACLE_SPACING).ceil().max(1.0) as usize;
                for step in 0..=steps {
                    let point = object.position + from.lerp(to, step as f32 / steps as f32);
                    let cell = (point / OBSTACLE_SPACING).round().as_ivec2();
                    if !cells.insert(cell) {
                        continue;
                    }
                    let nearest = course.nearest(point);
                    if nearest.distance <= band {
                        let along = course.distance_along(&nearest);
                        // As `Places::of` measures a truck.
                        let (middle, direction) = course.point_at(along);
                        outline.push((
                            cell,
                            Obstacle {
                                along,
                                across: -course.offset(middle, point).dot(direction.perp()),
                                position: point,
                            },
                        ));
                    }
                }
            }
        }
        obstacles.extend(
            outline
                .into_iter()
                .filter(|(cell, _)| seen.insert(*cell))
                .map(|(_, point)| point),
        );
    }
    obstacles.sort_by(|a, b| a.along.total_cmp(&b.along));
    obstacles
}

/// Where a triangle crosses the level `height`, on the ground plane (X and Z): the two ends
/// of the line it cuts there, or `None` where it is all above or all below.
fn slice(corners: [Vec3; 3], height: f32) -> Option<(Vec2, Vec2)> {
    let mut ends = (0..3).filter_map(|i| {
        let (p, q) = (corners[i], corners[(i + 1) % 3]);
        let (below_p, below_q) = (p.y - height, q.y - height);
        ((below_p < 0.0) != (below_q < 0.0)).then(|| p.lerp(q, below_p / (below_p - below_q)).xz())
    });
    Some((ends.next()?, ends.next()?))
}

/// The `obstacles`, in order along a course of `lap` metres, that are from `from` to `to`
/// metres along it, round the end of the lap and on. Found by a binary search for the first,
/// since a driver asks for these every step.
pub(crate) fn obstacles_between(
    obstacles: &[Obstacle],
    from: f32,
    to: f32,
    lap: f32,
) -> impl Iterator<Item = &Obstacle> {
    let start = from.rem_euclid(lap);
    let stretch = (to - from).min(lap);
    let first = obstacles.partition_point(|obstacle| obstacle.along < start);
    (0..obstacles.len())
        .map(move |i| &obstacles[(first + i) % obstacles.len()])
        .take_while(move |obstacle| (obstacle.along - start).rem_euclid(lap) <= stretch)
}

#[cfg(test)]
mod tests {
    use content::track::{SceneryModel, SceneryObject};

    use super::*;
    use crate::fixtures::course;

    #[test]
    fn a_triangle_is_cut_where_it_crosses_a_level() {
        // Standing up from the ground along X, 2 m high at its peak.
        let corners = [
            Vec3::ZERO,
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(2.0, 2.0, 0.0),
        ];
        let (from, to) = slice(corners, 1.0).expect("it crosses 1 m");
        let (low, high) = if from.x < to.x {
            (from, to)
        } else {
            (to, from)
        };
        assert!(low.distance(Vec2::new(1.0, 0.0)) < 1e-5, "{low}");
        assert!(high.distance(Vec2::new(3.0, 0.0)) < 1e-5, "{high}");
        // Above it and below it, nothing.
        assert_eq!(slice(corners, 3.0), None);
        assert_eq!(slice(corners, -1.0), None);
    }

    /// A block `size` metres across, along and high, standing `lift` metres off the ground,
    /// as a model of twelve triangles.
    fn block(size: Vec3, lift: f32) -> SceneryModel {
        let half = size / 2.0;
        let positions: Vec<[f32; 3]> = (0..8)
            .map(|i| {
                let pick = |bit: u32, half: f32| if i & bit == 0 { -half } else { half };
                [
                    pick(1, half.x),
                    lift + half.y + pick(2, half.y),
                    pick(4, half.z),
                ]
            })
            .collect();
        let faces: [[u32; 4]; 6] = [
            [0, 1, 3, 2],
            [4, 6, 7, 5],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 5, 7, 3],
        ];
        let indices = faces
            .iter()
            .flat_map(|&[a, b, c, d]| [a, b, c, a, c, d])
            .collect();
        SceneryModel {
            positions,
            indices,
            ..Default::default()
        }
    }

    #[test]
    fn what_reaches_into_the_road_at_a_trucks_height_is_an_obstacle() {
        // On the first side of the square course, which runs along +X from the origin: a
        // block standing beside it, a block and a ramp on it, a slab over it on high legs
        // that are not modelled, a loose block that a truck knocks away, and a block far off
        // in the field.
        let at = |x: f32, z: f32, model: usize, motion: SceneryMotion| SceneryObject {
            model,
            position: Vec2::new(x, z),
            height_above_ground: 0.0,
            yaw: 0.0,
            solid: true,
            motion,
            faces_camera: false,
            visible: true,
            ramp: false,
            sunk_on_slope: 0.0,
        };
        let scenery = Scenery {
            models: vec![
                block(Vec3::new(4.0, 2.0, 4.0), 0.0),
                block(Vec3::new(4.0, 1.0, 20.0), 4.0),
            ],
            objects: vec![
                at(100.0, 8.0, 0, SceneryMotion::Fixed),
                // On the road: a block to drive round, and a ramp to drive over.
                at(250.0, 1.0, 0, SceneryMotion::Fixed),
                SceneryObject {
                    ramp: true,
                    ..at(270.0, 0.0, 0, SceneryMotion::Fixed)
                },
                at(150.0, 0.0, 1, SceneryMotion::Fixed),
                at(200.0, 4.0, 0, SceneryMotion::Loose { mass: 50.0 }),
                at(150.0, 100.0, 0, SceneryMotion::Fixed),
            ],
            ..Default::default()
        };
        let course = course();
        let obstacles = obstacles_on(&scenery, &course);
        // Only the two blocks, the one beside the road only as far out as the 7 m edge of
        // the 14 m course and `OBSTACLE_MARGIN` past it: its near face is 6 m from the
        // middle, on the road, and its far face, 10 m off, is out of it.
        let beside =
            |&Vec2 { x, y }: &Vec2| (97.5..=102.5).contains(&x) && (5.5..=9.0).contains(&y);
        let on = |&Vec2 { x, y }: &Vec2| (247.5..=252.5).contains(&x) && (-1.5..=3.5).contains(&y);
        assert!(obstacles.iter().any(|obstacle| beside(&obstacle.position)));
        assert!(obstacles.iter().any(|obstacle| on(&obstacle.position)));
        for obstacle in &obstacles {
            let Vec2 { x, y } = obstacle.position;
            assert!(
                beside(&obstacle.position) || on(&obstacle.position),
                "{x}, {y}"
            );
            assert!((obstacle.along - x).abs() < 1e-3);
            // Along +X, +Z is to the right: as far to the right as it is out.
            assert!((obstacle.across + y).abs() < 1e-3, "{}", obstacle.across);
        }
        assert!(
            obstacles
                .windows(2)
                .all(|pair| pair[0].along <= pair[1].along)
        );
    }

    #[test]
    fn obstacles_are_found_between_two_places_along_the_course_and_round_the_lap() {
        let lap = 1200.0;
        let obstacles: Vec<Obstacle> = [10.0, 50.0, 400.0, 1190.0]
            .map(|along| Obstacle {
                along,
                across: 0.0,
                position: Vec2::ZERO,
            })
            .to_vec();
        let between = |from: f32, to: f32| -> Vec<f32> {
            obstacles_between(&obstacles, from, to, lap)
                .map(|obstacle| obstacle.along)
                .collect()
        };
        assert_eq!(between(0.0, 60.0), [10.0, 50.0]);
        assert_eq!(between(20.0, 45.0), Vec::<f32>::new());
        // Over the end of the lap, and from before its start.
        assert_eq!(between(1180.0, 1220.0), [1190.0, 10.0]);
        assert_eq!(between(-20.0, 20.0), [1190.0, 10.0]);
        // A stretch longer than the lap takes each one once.
        assert_eq!(between(0.0, 5000.0).len(), obstacles.len());
        assert_eq!(obstacles_between(&[], 0.0, 100.0, lap).count(), 0);
    }
}
