//! The axles that are drawn between the wheels, and the bars, shocks and driveshaft that
//! join them to the body. Looks only: the physics knows wheels and a chassis, and nothing
//! of axles.

use bevy::prelude::*;

use super::Wheel;

/// A solid axle, drawn from the middle of the line between its two wheels and tilting
/// as one of them rises. A child of the `TruckVisual`, like the wheels.
#[derive(Component)]
pub(super) struct Axle {
    pub(super) left: Entity,
    pub(super) right: Entity,
}

pub(super) fn place_axles(
    wheels: Query<&Transform, With<Wheel>>,
    mut axles: Query<(&Axle, &mut Transform), Without<Wheel>>,
) {
    for (axle, mut transform) in &mut axles {
        let (Ok(left), Ok(right)) = (wheels.get(axle.left), wheels.get(axle.right)) else {
            continue;
        };
        let (translation, rotation) = axle_pose(left.translation, right.translation);
        transform.translation = translation;
        transform.rotation = rotation;
    }
}

/// A bar, shock or half driveshaft from the body to an axle, drawn as a cylinder one metre
/// long along Y and stretched to reach. A child of the `TruckVisual`, like the wheels. It
/// follows the hubs itself, so that a truck with no axle model still has its links.
#[derive(Component)]
pub(super) struct LinkToAxle {
    /// The axle's hubs.
    pub(super) left: Entity,
    pub(super) right: Entity,
    /// See `truck::AxleLink`.
    pub(super) body_end: Vec3,
    pub(super) axle_end: Vec3,
}

pub(super) fn place_axle_links(
    wheels: Query<&Transform, With<Wheel>>,
    mut links: Query<(&LinkToAxle, &mut Transform), Without<Wheel>>,
) {
    for (link, mut transform) in &mut links {
        let (Ok(left), Ok(right)) = (wheels.get(link.left), wheels.get(link.right)) else {
            continue;
        };
        let (middle, rotation) = axle_pose(left.translation, right.translation);
        *transform = link_transform(link.body_end, middle + rotation * link.axle_end);
    }
}

/// A cylinder one metre long along Y, stretched and turned to run from `from` to `to`.
fn link_transform(from: Vec3, to: Vec3) -> Transform {
    let along = to - from;
    Transform {
        translation: from.midpoint(to),
        rotation: Quat::from_rotation_arc(Vec3::Y, along.normalize_or(Vec3::Y)),
        scale: Vec3::new(1.0, along.length(), 1.0),
    }
}

/// Where an axle is, given its hubs, in the truck's axes. The model's X axis runs from
/// the left hub to the right one. It doesn't turn with the steering: the hubs do that.
fn axle_pose(left_hub: Vec3, right_hub: Vec3) -> (Vec3, Quat) {
    let along = right_hub - left_hub;
    (
        left_hub.midpoint(right_hub),
        Quat::from_rotation_z(along.y.atan2(along.x)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_axle_runs_from_hub_to_hub() {
        let (left, right) = (Vec3::new(-1.5, -0.2, 2.0), Vec3::new(1.5, 0.4, 2.0));
        let (middle, rotation) = axle_pose(left, right);
        assert_eq!(middle, Vec3::new(0.0, 0.1, 2.0));
        let end = middle + rotation * Vec3::X * left.distance(right) / 2.0;
        assert!(end.distance(right) < 1e-5, "{end}");

        let (_, level) = axle_pose(Vec3::new(-1.5, 0.0, 0.0), Vec3::new(1.5, 0.0, 0.0));
        assert!(level.angle_between(Quat::IDENTITY) < 1e-6);
    }

    #[test]
    fn a_link_runs_from_end_to_end() {
        let (from, to) = (Vec3::new(0.4, -0.8, 0.0), Vec3::new(0.6, -1.0, 1.9));
        let transform = link_transform(from, to);
        // The cylinder's ends are half a metre either side of its middle, along Y.
        let end = |y: f32| transform.transform_point(Vec3::new(0.0, y, 0.0));
        assert!(end(-0.5).distance(from) < 1e-5, "{}", end(-0.5));
        assert!(end(0.5).distance(to) < 1e-5, "{}", end(0.5));
    }
}
