//! How far each fixed object must be lowered so that its foot touches sloping ground
//! (`TrackSettings::settle_scenery`). Monster Truck Madness 2's editor stands a model on its
//! lowest point at the ground under its origin only, so one with a broad, flat foot on a
//! slope floats at its downhill corners (`docs/formats/situation.md`).
//!
//! Worked out once, as a track is loaded, and kept in `SceneryObject::sunk_on_slope`: it
//! looks up the ground under every corner of every foot, up to 24 ms for a track with
//! 4000 objects in the dev build. Whether to use it is decided as each race begins.

use bevy::prelude::*;

use super::{HeightGrid, Scenery, SceneryModel, SceneryMotion, SceneryObject};

/// The corners of a model's foot are its vertices within this many metres of its lowest.
const FOOT_DEPTH: f32 = 0.25;
/// A foot whose every corner is more than this many metres above the ground was put there
/// on purpose, as a gantry over the road or a bridge is, and is not lowered. About a foot:
/// as far as the objects set by hand in the base game's tracks miss the ground.
const PUT_ABOVE_GROUND: f32 = 0.3;
/// The most an object is lowered, as a share of its height. Higher, a tree on a steep slope
/// whose lowest branch floats would lose its trunk in the ground: Alaska's and Graveyard's
/// float 5 to 8 m.
const MOST_SUNK: f32 = 0.1;

/// Fills in `sunk_on_slope` for every object of `scenery`. A loose or moving object is not
/// lowered: a loose one would be pushed out of the ground as it wakes, and a moving one
/// keeps its height whatever the ground under it does.
pub fn settle(scenery: &mut Scenery, heights: &HeightGrid) {
    let feet: Vec<Foot> = scenery.models.iter().map(Foot::of).collect();
    for object in &mut scenery.objects {
        object.sunk_on_slope = match object.motion {
            SceneryMotion::Fixed => feet[object.model].sink(object, heights),
            _ => 0.0,
        };
    }
}

/// The lowest part of a model, which stands on the ground.
struct Foot {
    /// Its corners, relative to the model's origin, in metres.
    corners: Vec<Vec3>,
    /// From the model's lowest point to its highest, in metres.
    height: f32,
}

impl Foot {
    fn of(model: &SceneryModel) -> Self {
        let (lowest, highest) = model
            .positions
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), p| {
                (low.min(p[1]), high.max(p[1]))
            });
        Self {
            corners: model
                .positions
                .iter()
                .map(|&p| Vec3::from(p))
                .filter(|corner| corner.y <= lowest + FOOT_DEPTH)
                .collect(),
            height: (highest - lowest).max(0.0),
        }
    }

    /// How far to lower `object` so that the corner of its foot highest over the ground
    /// touches it.
    fn sink(&self, object: &SceneryObject, heights: &HeightGrid) -> f32 {
        let ground = heights.height_at(object.position.x, object.position.y);
        let rotation = Quat::from_rotation_y(object.yaw);
        let origin = object
            .position
            .extend(ground + object.height_above_ground)
            .xzy();
        let (nearest, furthest) = self.corners.iter().fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(nearest, furthest), &corner| {
                let at = origin + rotation * corner;
                let above = at.y - heights.height_at(at.x, at.z);
                (nearest.min(above), furthest.max(above))
            },
        );
        if nearest > PUT_ABOVE_GROUND {
            return 0.0;
        }
        furthest.clamp(0.0, self.height * MOST_SUNK)
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::{FRAC_PI_4, SQRT_2};

    use super::*;

    /// Rises 1 m in every 5 along +X.
    fn slope() -> HeightGrid {
        HeightGrid::from_fn(65, 640.0, |x, _| x * 0.2)
    }

    /// A box `width` metres square and `height` high, standing on its origin.
    fn standing_box(width: f32, height: f32) -> SceneryModel {
        let half = width / 2.0;
        let positions = [0.0, height]
            .into_iter()
            .flat_map(|y| {
                [
                    [-half, y, -half],
                    [half, y, -half],
                    [-half, y, half],
                    [half, y, half],
                ]
            })
            .collect();
        SceneryModel {
            positions,
            ..default()
        }
    }

    fn standing_at(height_above_ground: f32, yaw: f32, motion: SceneryMotion) -> SceneryObject {
        SceneryObject {
            model: 0,
            position: Vec2::ZERO,
            height_above_ground,
            yaw,
            solid: true,
            motion,
            faces_camera: false,
            visible: true,
            ramp: false,
            sunk_on_slope: 0.0,
        }
    }

    fn fixed(height_above_ground: f32, yaw: f32) -> SceneryObject {
        standing_at(height_above_ground, yaw, SceneryMotion::Fixed)
    }

    #[test]
    fn a_foot_on_a_slope_is_lowered_until_its_downhill_corners_touch() {
        // Sidewinder Canyon's checkpoint pillar: 5.8 m across and 12.2 m high.
        let pillar = Foot::of(&standing_box(5.8, 12.2));
        assert_eq!(pillar.corners.len(), 4);
        let sink = pillar.sink(&fixed(0.0, 0.0), &slope());
        assert!((sink - 2.9 * 0.2).abs() < 1e-4, "sank {sink} m");
        // Turned by an eighth, its corner reaches further down the slope.
        let turned = pillar.sink(&fixed(0.0, FRAC_PI_4), &slope());
        assert!(
            (turned - 2.9 * SQRT_2 * 0.2).abs() < 1e-4,
            "sank {turned} m"
        );
        // On the level, nothing floats.
        let level = HeightGrid::from_fn(65, 640.0, |_, _| 3.0);
        assert_eq!(pillar.sink(&fixed(0.0, 0.0), &level), 0.0);
    }

    #[test]
    fn a_foot_is_lowered_by_no_more_than_a_tenth_of_its_height() {
        let slab = Foot::of(&standing_box(5.8, 2.0));
        let sink = slab.sink(&fixed(0.0, 0.0), &slope());
        assert!((sink - 0.2).abs() < 1e-4, "sank {sink} m");
    }

    #[test]
    fn a_foot_put_above_the_ground_stays_there() {
        let gantry = Foot::of(&standing_box(5.8, 3.5));
        // A gantry over the road, 7.3 m up, as the start lights stand.
        assert_eq!(gantry.sink(&fixed(7.3, 0.0), &slope()), 0.0);
        // A model with nothing to it has no foot.
        let nothing = Foot::of(&SceneryModel::default());
        assert_eq!(nothing.sink(&fixed(0.0, 0.0), &slope()), 0.0);
    }

    #[test]
    fn only_fixed_objects_are_lowered() {
        let mut scenery = Scenery {
            models: vec![standing_box(5.8, 12.2)],
            objects: vec![
                fixed(0.0, 0.0),
                standing_at(0.0, 0.0, SceneryMotion::Loose { mass: 50.0 }),
                standing_at(0.0, 0.0, SceneryMotion::Moving { velocity: Vec3::X }),
            ],
            ..default()
        };
        settle(&mut scenery, &slope());
        let sunk: Vec<f32> = scenery.objects.iter().map(|o| o.sunk_on_slope).collect();
        assert!((sunk[0] - 0.58).abs() < 1e-4, "{sunk:?}");
        assert_eq!(sunk[1..], [0.0, 0.0]);
    }
}
