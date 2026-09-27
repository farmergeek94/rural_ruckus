//! Objects that always face the camera (`track::SceneryObject::faces_camera`): flat
//! pictures of trees and palms. Each turns about Y only, so that it stands upright, after
//! the camera has moved, so that it is never a frame behind. Its own `yaw` is replaced.
//!
//! Which side of the picture faces the camera is the model's +Z, as in JSTrackViewer. Both
//! sides are drawn, so the other choice would only show the picture the other way round.
//! See `docs/formats/situation.md`.

use bevy::prelude::*;

use crate::camera::ChaseCamera;

/// On a scenery object that turns to face the camera.
#[derive(Component)]
pub(super) struct FacesCamera;

/// The rotation about Y that turns +Z from `at` towards `eye`, on the ground plane. None
/// when the eye is straight above or below, where any way round will do.
fn yaw_towards(at: Vec3, eye: Vec3) -> Option<f32> {
    let to_eye = eye - at;
    if to_eye.x.abs() < f32::EPSILON && to_eye.z.abs() < f32::EPSILON {
        return None;
    }
    Some(to_eye.x.atan2(to_eye.z))
}

/// Turns every object that faces the camera to face it.
pub(super) fn face_the_camera(
    cameras: Query<&Transform, (With<ChaseCamera>, Without<FacesCamera>)>,
    mut objects: Query<&mut Transform, With<FacesCamera>>,
) {
    let Ok(eye) = cameras.single() else {
        return;
    };
    for mut transform in &mut objects {
        if let Some(yaw) = yaw_towards(transform.translation, eye.translation) {
            let rotation = Quat::from_rotation_y(yaw);
            if transform.rotation != rotation {
                transform.rotation = rotation;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plus_z_turns_to_the_eye_on_the_ground_plane() {
        let at = Vec3::new(3.0, 1.0, -2.0);
        for eye in [
            Vec3::new(10.0, 5.0, 4.0),
            Vec3::new(-7.0, -2.0, 1.0),
            Vec3::new(3.0, 20.0, -9.0),
        ] {
            let facing = Quat::from_rotation_y(yaw_towards(at, eye).unwrap()) * Vec3::Z;
            let flat = (eye - at).with_y(0.0).normalize();
            assert!(facing.abs_diff_eq(flat, 1e-5), "{eye}");
        }
    }

    #[test]
    fn an_eye_straight_above_leaves_it_as_it_is() {
        assert_eq!(yaw_towards(Vec3::ZERO, Vec3::Y * 10.0), None);
    }
}
