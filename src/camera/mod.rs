//! The chase camera, which keeps the truck in the middle of the picture, and the other
//! views: the cockpit, and looking aside or behind (`views`), with the truck's dashboard
//! in the cockpit (`dashboard`). On `truck::Autopilot`, as after the finish, the view
//! changes by itself, with a swing or a fade (`views`).
//!
//! This is the game's side of the camera, and the only part of it that knows the slices.
//! `chase` (with `rig` and `spring`) is the camera by itself: fastened to nothing, it
//! follows whatever it is told about smoothly and knows nothing of trucks, tracks, the physics
//! or `GameState`. Here it is told about the truck each frame, where the truck is drawn
//! and how fast its body is going, and about the ground under where the eye wants to be,
//! which it then keeps clear of.
//!
//! Uses the `truck` slice for the truck and the `track` slice for the ground.

mod chase;
mod dashboard;
mod rig;
mod spring;
mod views;

use avian3d::prelude::LinearVelocity;
use bevy::anti_alias::fxaa::Fxaa;
use bevy::camera::Hdr;
use bevy::post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::prelude::*;

use crate::game_state::GameState;
use crate::track::Track;
use crate::truck::{Player, TruckSystems, TruckVisual};

pub use chase::{CameraRigPlugin, CameraSystems, ChaseCamera};
pub use rig::{Pose, Rig, RigConfig, Target};
pub use spring::{follow, follow_angle, wrap};
pub use views::CameraView;

pub struct ChaseCameraPlugin;

impl Plugin for ChaseCameraPlugin {
    fn build(&self, app: &mut App) {
        // `TrackPlugin` and `TruckPlugin`, which this slice needs, have made sure of the state.
        if !app.is_plugin_added::<CameraRigPlugin>() {
            app.add_plugins(CameraRigPlugin);
        }
        app.init_resource::<CameraSettings>()
            .init_resource::<CameraView>()
            .init_resource::<views::Look>()
            .init_resource::<views::Director>()
            .init_resource::<crate::keys::KeyBindings>()
            .add_systems(
                OnEnter(GameState::Racing),
                (
                    spawn_camera,
                    views::reset_view,
                    views::spawn_fade,
                    dashboard::spawn_dashboard,
                ),
            )
            .add_systems(
                Update,
                (
                    // After the truck has been placed for this frame, or the camera would
                    // follow where it was drawn a frame ago.
                    follow_truck
                        .after(TruckSystems::PlaceVisuals)
                        .before(CameraSystems::Want),
                    tell_of_the_ground
                        .after(CameraSystems::Want)
                        .before(CameraSystems::Place),
                    apply_settings.run_if(resource_changed::<CameraSettings>),
                    (
                        views::change_view.before(CameraSystems::Want),
                        (views::direct_views, views::show_fade)
                            .chain()
                            .before(CameraSystems::Want),
                        // Part of placing the camera, so that what follows it sees the view.
                        views::place_view
                            .in_set(CameraSystems::Place)
                            .after(chase::place),
                        dashboard::show_dashboard.after(CameraSystems::Place),
                    )
                        .run_if(in_state(GameState::Racing)),
                ),
            );
    }
}

/// Choices about how the race is drawn. Insert it before adding `ChaseCameraPlugin`, or
/// change it while racing.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct CameraSettings {
    /// How jagged edges are smoothed, if at all.
    pub antialiasing: Antialiasing,
    /// Draws in high dynamic range, so that what is brighter than white (the trucks'
    /// lamps) spills a glow round it (bloom). Costs a few passes over the screen a frame.
    pub bloom: bool,
    /// Where the camera sits and how tightly it follows. Each number says what it does.
    pub rig: RigConfig,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            antialiasing: Antialiasing::Msaa,
            bloom: true,
            rig: RigConfig::default(),
        }
    }
}

impl CameraSettings {
    /// Four samples or one: the two counts every graphics card supports. FXAA works on
    /// the finished picture, so it takes one.
    fn msaa(&self) -> Msaa {
        match self.antialiasing {
            Antialiasing::Msaa => Msaa::Sample4,
            Antialiasing::Off | Antialiasing::Fxaa => Msaa::Off,
        }
    }
}

/// How jagged edges are smoothed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Antialiasing {
    Off,
    /// A pass over the finished picture that softens the edges it finds (FXAA). Costs
    /// little, and blurs fine detail a little.
    Fxaa,
    /// Four samples worked out for every pixel (MSAA). Sharp. The cost grows with the
    /// size of the screen, and is felt most on graphics built into the processor.
    #[default]
    Msaa,
}

impl Antialiasing {
    /// Every kind, from the cheapest up: the order the F2 panel steps through.
    pub const ALL: [Antialiasing; 3] = [Antialiasing::Off, Antialiasing::Fxaa, Antialiasing::Msaa];

    /// What the player sees it called.
    pub fn name(self) -> &'static str {
        match self {
            Antialiasing::Off => "Off",
            Antialiasing::Fxaa => "FXAA",
            Antialiasing::Msaa => "4x MSAA",
        }
    }

    /// The kind called `name` on the command line, in any case: `off`, `fxaa` or `msaa`.
    pub fn named(name: &str) -> Option<Antialiasing> {
        match name.to_ascii_lowercase().as_str() {
            "off" => Some(Antialiasing::Off),
            "fxaa" => Some(Antialiasing::Fxaa),
            "msaa" | "4x msaa" => Some(Antialiasing::Msaa),
            _ => None,
        }
    }
}

fn spawn_camera(mut commands: Commands, settings: Res<CameraSettings>) {
    let camera = commands
        .spawn((
            Camera3d::default(),
            settings.msaa(),
            DespawnOnExit(GameState::Racing),
            // Until the truck has been seen, which is before anything is drawn.
            Transform::from_xyz(0.0, 8.0, 14.0).looking_at(Vec3::ZERO, Vec3::Y),
            ChaseCamera::new(settings.rig.clone()),
        ))
        .id();
    if settings.bloom {
        commands.entity(camera).insert(glow());
    }
    if settings.antialiasing == Antialiasing::Fxaa {
        commands.entity(camera).insert(Fxaa::default());
    }
}

/// Only what is brighter than this, in linear colour after exposure, spills a glow. The
/// sun (`environment::SUNLIGHT`) at Bevy's default exposure lights a white surface to
/// about 1.3, and the lamps are drawn at 12 times their picture, so this keeps the lit
/// scene sharp and lets the lamps glow. Lower makes sunlit ground hazy; higher dims the
/// lamps' glow.
const GLOW_THRESHOLD: f32 = 2.0;

/// The race camera's bloom: a soft glow round only what is brighter than
/// `GLOW_THRESHOLD`. `Bloom::NATURAL` alone has no threshold, so it spreads a share of
/// every pixel over the picture and makes it all hazy. Stronger makes the glows bigger.
fn glow() -> Bloom {
    Bloom {
        intensity: 0.2,
        prefilter: BloomPrefilter {
            threshold: GLOW_THRESHOLD,
            // Eases pixels in over the top 20 % below the threshold, so that a glow has no
            // hard edge.
            threshold_softness: 0.2,
        },
        // Bevy's advice with a threshold: add the glow on, rather than blend the picture
        // with its blurred copy.
        composite_mode: BloomCompositeMode::Additive,
        ..Bloom::NATURAL
    }
}

#[allow(clippy::type_complexity)]
fn apply_settings(
    mut commands: Commands,
    settings: Res<CameraSettings>,
    mut cameras: Query<(Entity, &mut Msaa, &mut ChaseCamera, Has<Bloom>, Has<Fxaa>)>,
) {
    for (entity, mut msaa, mut camera, bloom, fxaa) in &mut cameras {
        if settings.bloom && !bloom {
            commands.entity(entity).insert(glow());
        } else if !settings.bloom && bloom {
            commands.entity(entity).remove::<(Bloom, Hdr)>();
        }
        let wants_fxaa = settings.antialiasing == Antialiasing::Fxaa;
        if wants_fxaa && !fxaa {
            commands.entity(entity).insert(Fxaa::default());
        } else if !wants_fxaa && fxaa {
            commands.entity(entity).remove::<Fxaa>();
        }
        msaa.set_if_neq(settings.msaa());
        if camera.config != settings.rig {
            camera.config = settings.rig.clone();
        }
    }
}

/// The player's truck: where it is drawn, which moves smoothly from frame to frame, and
/// how fast its body is really going, which a difference of drawn positions wouldn't say.
fn follow_truck(
    player: Single<(Entity, &LinearVelocity), Player>,
    visuals: Query<(&TruckVisual, &Transform)>,
    mut camera: Single<&mut ChaseCamera>,
) {
    let (player, velocity) = *player;
    // A handful of trucks at most.
    let Some((_, transform)) = visuals.iter().find(|(visual, _)| visual.truck == player) else {
        return;
    };
    let velocity = velocity.0;
    camera.target = Some(Target {
        position: transform.translation,
        velocity,
        facing: transform.forward().as_vec3(),
    });
}

/// So that hills don't get between the camera and the truck.
fn tell_of_the_ground(track: Res<Track>, mut camera: Single<&mut ChaseCamera>) {
    let eye = camera.wanted_eye();
    camera.ground = Some(track.heights.height_at(eye.x, eye.z));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_of_antialiasing_is_known_by_its_name() {
        for kind in Antialiasing::ALL {
            assert_eq!(Antialiasing::named(kind.name()), Some(kind), "{kind:?}");
        }
        assert_eq!(Antialiasing::named("MSAA"), Some(Antialiasing::Msaa));
        assert_eq!(Antialiasing::named("smaa"), None);
    }

    #[test]
    fn only_msaa_takes_more_than_one_sample_a_pixel() {
        let with = |antialiasing| {
            CameraSettings {
                antialiasing,
                ..default()
            }
            .msaa()
        };
        assert_eq!(with(Antialiasing::Msaa), Msaa::Sample4);
        assert_eq!(with(Antialiasing::Fxaa), Msaa::Off);
        assert_eq!(with(Antialiasing::Off), Msaa::Off);
    }
}
