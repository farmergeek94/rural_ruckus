//! A chase camera that is fastened to nothing. It is told, every frame, where something is,
//! how fast it is going and which way it faces, and it keeps that thing in the middle of the
//! picture: following it up, down and sideways, and always bringing it back to the centre
//! smoothly, with no jump in the camera's own speed or turning.
//!
//! It knows nothing of what it follows, or of the game: it is the camera by itself, which
//! `camera/mod.rs` fastens to the truck. The rules are `Rig`, a state machine over plain
//! data with no Bevy in it but its vectors, tested against scripted paths in
//! `tests/camera_smoothness.rs`. `CameraRigPlugin` is a thin wrapper that runs a `Rig` for
//! every entity with a `ChaseCamera` and writes its `Transform`.
//!
//! Whoever follows something with it does this each frame, in `Update`:
//!
//! 1. before `CameraSystems::Want`, write `ChaseCamera::target`;
//! 2. between `CameraSystems::Want` and `CameraSystems::Place`, if there is ground, read
//!    `ChaseCamera::wanted_eye` and write `ChaseCamera::ground` with its height there.

use bevy::prelude::*;

use super::rig::{Rig, RigConfig, Target};

/// Runs a `Rig` for every `ChaseCamera`. `camera::ChaseCameraPlugin` adds it.
pub struct CameraRigPlugin;
impl Plugin for CameraRigPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(Update, (CameraSystems::Want, CameraSystems::Place).chain())
            .add_systems(
                Update,
                (
                    want.in_set(CameraSystems::Want),
                    place.in_set(CameraSystems::Place),
                ),
            );
    }
}

/// For the game to order its systems against.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CameraSystems {
    /// Follows the target for the frame, and works out where the eye wants to be.
    Want,
    /// Places the camera, clear of the ground it has been told of.
    Place,
}

/// Put this beside a `Camera3d`, with a `Transform`, and keep its `target` up to date.
#[derive(Component, Debug, Default)]
pub struct ChaseCamera {
    /// May be changed at any time: it is read every frame.
    pub config: RigConfig,
    /// What to follow. While there is none the camera stays where it is.
    pub target: Option<Target>,
    /// The height of the ground under `wanted_eye`, in metres, for this frame. `None`
    /// for no ground at all.
    pub ground: Option<f32>,
    wanted_eye: Vec3,
    rig: Option<Rig>,
}

impl ChaseCamera {
    pub fn new(config: RigConfig) -> Self {
        Self {
            config,
            ..default()
        }
    }

    /// Where the eye wants to be this frame, before the ground has had its say. Good
    /// from `CameraSystems::Want` on.
    pub fn wanted_eye(&self) -> Vec3 {
        self.wanted_eye
    }

    /// Goes straight to the target next frame, instead of following it there.
    pub fn snap(&mut self) {
        self.rig = None;
    }
}

fn want(time: Res<Time>, mut cameras: Query<&mut ChaseCamera>) {
    for mut camera in &mut cameras {
        let camera = &mut *camera;
        let Some(target) = &camera.target else {
            continue;
        };
        let rig = camera
            .rig
            .get_or_insert_with(|| Rig::new(&camera.config, target));
        camera.wanted_eye = rig.want(&camera.config, target, time.delta_secs());
    }
}

pub(super) fn place(time: Res<Time>, mut cameras: Query<(&mut ChaseCamera, &mut Transform)>) {
    for (mut camera, mut transform) in &mut cameras {
        let camera = &mut *camera;
        let Some(rig) = &mut camera.rig else {
            continue;
        };
        let pose = rig.place(&camera.config, camera.ground.take(), time.delta_secs());
        *transform = Transform::from_translation(pose.eye).looking_at(pose.aim, Vec3::Y);
    }
}
