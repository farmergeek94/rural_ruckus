//! Tools for finding out why the game misbehaves. All off unless asked for.
//!
//! `--log-fps` prints, once a second: frame rate, frame time, how much of the world is being
//! drawn, and the worst frames of that second, which the averages hide.
//! `--autopilot` drives the truck round the course, so that a problem that only shows while
//! driving can be reproduced and measured without anyone at the keyboard. It is a
//! diagnostic, not an opponent: it presses the player's own keys.

use std::time::{Duration, Instant};

use bevy::diagnostic::{
    Diagnostic, DiagnosticPath, Diagnostics, FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin,
    RegisterDiagnostic,
};
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::scenery::SceneryObject;
use crate::track::Track;
use crate::truck::Player;

#[derive(Default)]
pub struct DiagnosticsPlugin {
    pub log_fps: bool,
    pub autopilot: bool,
}

/// Meshes that made it past culling this frame, of all the meshes there are.
const MESHES_IN_VIEW: DiagnosticPath = DiagnosticPath::const_new("meshes_in_view");
const MESHES: DiagnosticPath = DiagnosticPath::const_new("meshes");
/// The same for scenery alone.
const SCENERY_IN_VIEW: DiagnosticPath = DiagnosticPath::const_new("scenery_in_view");

/// A frame this long has missed a refresh of a 60 Hz screen, with a little to spare.
const LATE_FRAME: Duration = Duration::from_millis(20);

impl Plugin for DiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        if self.log_fps {
            app.add_plugins((
                FrameTimeDiagnosticsPlugin::default(),
                LogDiagnosticsPlugin::default(),
            ))
            .register_diagnostic(Diagnostic::new(MESHES_IN_VIEW))
            .register_diagnostic(Diagnostic::new(MESHES))
            .register_diagnostic(Diagnostic::new(SCENERY_IN_VIEW))
            .init_resource::<WorstFrames>()
            // Visibility is worked out in `PostUpdate`, so this reads the frame before's.
            .init_resource::<ViewMotion>()
            .add_systems(Update, (count_what_is_drawn, log_worst_frames))
            // After everything that moves the camera this frame.
            .add_systems(Last, log_view_motion);
        }
        if self.autopilot {
            // Before the truck reads the keys, in the same schedule.
            app.add_systems(PreUpdate, drive_the_course.after(bevy::input::InputSystems));
        }
    }
}

fn count_what_is_drawn(
    mut diagnostics: Diagnostics,
    meshes: Query<(&ViewVisibility, Has<SceneryObject>), With<Mesh3d>>,
) {
    let (mut all, mut in_view, mut scenery_in_view) = (0, 0, 0);
    for (visibility, is_scenery) in &meshes {
        all += 1;
        if visibility.get() {
            in_view += 1;
            if is_scenery {
                scenery_in_view += 1;
            }
        }
    }
    diagnostics.add_measurement(&MESHES, || all as f64);
    diagnostics.add_measurement(&MESHES_IN_VIEW, || in_view as f64);
    diagnostics.add_measurement(&SCENERY_IN_VIEW, || scenery_in_view as f64);
}

/// Frame times as the wall clock sees them, which is what the eye sees too.
#[derive(Resource)]
struct WorstFrames {
    last_frame: Instant,
    last_report: Instant,
    frames: Vec<Duration>,
}

impl Default for WorstFrames {
    fn default() -> Self {
        Self {
            last_frame: Instant::now(),
            last_report: Instant::now(),
            frames: Vec::new(),
        }
    }
}

fn log_worst_frames(mut worst: ResMut<WorstFrames>) {
    let now = Instant::now();
    let frame = now.duration_since(worst.last_frame);
    worst.last_frame = now;
    worst.frames.push(frame);

    if now.duration_since(worst.last_report) < Duration::from_secs(1) {
        return;
    }
    worst.last_report = now;
    worst.frames.sort();
    let millis = |frame: &Duration| frame.as_secs_f64() * 1000.0;
    let late = worst
        .frames
        .iter()
        .filter(|&&frame| frame > LATE_FRAME)
        .count();
    info!(
        "frames: {} this second, shortest {:.1} ms, median {:.1} ms, longest {:.1} ms, {late} over {:.0} ms",
        worst.frames.len(),
        millis(&worst.frames[0]),
        millis(&worst.frames[worst.frames.len() / 2]),
        millis(&worst.frames[worst.frames.len() - 1]),
        millis(&LATE_FRAME),
    );
    worst.frames.clear();
}

/// How the picture moved: what the eye judges smoothness by. A smooth turn sweeps the
/// view by nearly the same angle every frame. When that angle jumps about from one frame
/// to the next, the whole picture judders, whatever the frame rate says.
#[derive(Resource, Default)]
struct ViewMotion {
    last_forward: Option<Vec3>,
    last_position: Option<Vec3>,
    last_velocity: Option<Vec3>,
    last_turn: Option<f32>,
    last_report: Option<Instant>,
    /// Degrees the view turned in each frame.
    turns: Vec<f32>,
    turn_changes: Vec<f32>,
    /// Camera travel per frame, in metres, and velocity changes in metres per second.
    travel: Vec<f32>,
    velocity_changes: Vec<f32>,
}

/// The dot product alone rounds tiny turns to zero in f32. The cross product retains
/// those angles, which are precisely the ones the judder measurement needs to resolve.
fn turn_degrees(from: Vec3, to: Vec3) -> f32 {
    from.cross(to).length().atan2(from.dot(to)).to_degrees()
}

fn log_view_motion(
    mut motion: ResMut<ViewMotion>,
    time: Res<Time>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let forward = camera.forward().as_vec3();
    if let Some(last) = motion.last_forward {
        let turn = turn_degrees(last, forward);
        motion.turns.push(turn);
        if let Some(previous) = motion.last_turn {
            motion.turn_changes.push((turn - previous).abs());
        }
        motion.last_turn = Some(turn);
    }
    motion.last_forward = Some(forward);
    let position = camera.translation();
    if let Some(last) = motion.last_position {
        let displacement = position - last;
        motion.travel.push(displacement.length());
        // Normalise by the clock that moves the camera: a longer simulation frame
        // naturally travels further and should not be mistaken for a position jump.
        if time.delta_secs() > 0.0 {
            let velocity = displacement / time.delta_secs();
            if let Some(previous) = motion.last_velocity {
                motion.velocity_changes.push(velocity.distance(previous));
            }
            motion.last_velocity = Some(velocity);
        } else {
            motion.last_velocity = None;
        }
    }
    motion.last_position = Some(position);

    let now = Instant::now();
    let last_report = *motion.last_report.get_or_insert(now);
    if now.duration_since(last_report) < Duration::from_secs(1) || motion.turns.len() < 3 {
        return;
    }
    motion.last_report = Some(now);

    motion.turn_changes.sort_by(f32::total_cmp);
    let changes = &motion.turn_changes;
    let total: f32 = motion.turns.iter().sum();
    info!(
        "view: turned {total:.1} deg this second; change in turn from one frame to the next: \
         median {:.3}, 95th percentile {:.3}, worst {:.3} deg",
        changes[changes.len() / 2],
        changes[changes.len() * 95 / 100],
        changes[changes.len() - 1],
    );
    if !motion.velocity_changes.is_empty() {
        motion.velocity_changes.sort_by(f32::total_cmp);
        let changes = &motion.velocity_changes;
        let travel = motion.travel.iter().copied().fold(0.0, f32::max);
        info!(
            "view position: largest step {travel:.3} m; change in velocity from one frame to the next: \
             median {:.3}, 95th percentile {:.3}, worst {:.3} m/s",
            changes[changes.len() / 2],
            changes[changes.len() * 95 / 100],
            changes[changes.len() - 1],
        );
    }
    motion.turns.clear();
    motion.turn_changes.clear();
    motion.travel.clear();
    motion.velocity_changes.clear();
}

/// How far ahead along the course to aim, in metres.
const LOOK_AHEAD: f32 = 25.0;
/// In m/s: brisk, but slow enough to stay on a mountain road.
const CRUISE_SPEED: f32 = 18.0;

fn drive_the_course(
    track: Res<Track>,
    truck: Single<(&Transform, &Velocity), Player>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
) {
    let Some(course) = &track.course else {
        return;
    };
    let (transform, velocity) = *truck;
    let position = transform.translation.xz();
    let forward = transform.forward().xz().normalize_or_zero();

    // Aim at the point of the centreline a little further along than where we are.
    let travelled = course.distance_along(&course.nearest(position));
    let (aim, _) = course.point_at(travelled + LOOK_AHEAD);
    // Positive when the aim point is to the left.
    let turn = -forward.perp_dot((aim - position).normalize_or_zero());

    // The arrow keys, which drive whatever the player has bound (see `keys`).
    for key in [KeyCode::ArrowUp, KeyCode::ArrowLeft, KeyCode::ArrowRight] {
        keys.release(key);
    }
    if velocity.linear.length() < CRUISE_SPEED {
        keys.press(KeyCode::ArrowUp);
    }
    if turn > 0.04 {
        keys.press(KeyCode::ArrowLeft);
    } else if turn < -0.04 {
        keys.press(KeyCode::ArrowRight);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_turns_smaller_than_a_hundredth_of_a_degree() {
        for degrees in [0.001_f32, 0.005, 0.01, 0.02, 90.0, 180.0] {
            let from = Vec3::NEG_Z;
            let to = Quat::from_rotation_y(degrees.to_radians()) * from;
            let measured = turn_degrees(from, to);
            assert!((measured - degrees).abs() < 1e-5, "{degrees}: {measured}");
        }
        assert_eq!(turn_degrees(Vec3::NEG_Z, Vec3::NEG_Z), 0.0);
    }
}
