//! A panel for trying the graphics settings while driving, hidden until its key
//! (`keys::Control::GraphicsSettings`, F2 unless bound to another) is pressed.
//!
//! Each line is one setting that costs frame time, with the key that steps it through its
//! values, and the frame time is shown underneath so that the difference can be read off
//! at once, with the longest frame and how many started late, which an average hides. Run
//! with `--no-vsync`, or everything under a refresh of the screen reads the same.
//!
//! Uses the `camera`, `environment` and `track` slices, whose settings these are. It only
//! writes their settings resources: each slice carries its own to what it has spawned.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

use crate::camera::CameraSettings;
use crate::environment::EnvironmentSettings;
use crate::game_state::GameState;
use crate::keys::{Control, KeyBindings};
use crate::track::TrackSettings;

pub struct GraphicsDebugPlugin;

impl Plugin for GraphicsDebugPlugin {
    fn build(&self, app: &mut App) {
        // A slice works alone, so it makes sure of the state it is built in. A second
        // `init_state` would log a warning.
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        if !app.world().contains_resource::<State<GameState>>() {
            app.init_state::<GameState>();
        }
        app.init_resource::<GraphicsDebug>()
            .init_resource::<KeyBindings>()
            .init_resource::<FrameTime>()
            .init_resource::<CameraSettings>()
            .init_resource::<EnvironmentSettings>()
            .init_resource::<TrackSettings>()
            .add_systems(OnEnter(GameState::Racing), spawn_panel)
            .add_systems(
                Update,
                (toggle_panel, change_settings, measure_frames, write_panel)
                    .chain()
                    .run_if(in_state(GameState::Racing)),
            );
    }
}

/// Whether the panel is showing. It stays as it was left from one race to the next.
#[derive(Resource, Default)]
pub struct GraphicsDebug {
    pub open: bool,
}

/// While the panel is open. They are not bindable: the panel says which does what.
const ANTIALIASING_KEY: KeyCode = KeyCode::F3;
const SHADOW_CASCADES_KEY: KeyCode = KeyCode::F4;
const SHADOW_DISTANCE_KEY: KeyCode = KeyCode::F5;
const ANISOTROPY_KEY: KeyCode = KeyCode::F6;

/// The values each key steps through. They include every default.
const SHADOW_CASCADES: [usize; 4] = [0, 1, 2, 4];
/// In metres.
const SHADOW_DISTANCES: [f32; 4] = [50.0, 100.0, 150.0, 300.0];
const ANISOTROPIES: [u16; 5] = [1, 2, 4, 8, 16];

/// How often the frame time on the panel is brought up to date. Any faster and it
/// can't be read.
const REFRESH: Duration = Duration::from_millis(500);

#[derive(Component)]
struct PanelText;

fn spawn_panel(mut commands: Commands, debug: Res<GraphicsDebug>) {
    commands.spawn((
        PanelText,
        DespawnOnExit(GameState::Racing),
        Text::default(),
        TextFont::from_font_size(16.0),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            left: px(16),
            // Under the key reference.
            top: px(40),
            padding: UiRect::all(px(8)),
            ..default()
        },
        visibility(debug.open),
    ));
}

fn visibility(open: bool) -> Visibility {
    if open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

fn toggle_panel(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<KeyBindings>,
    mut debug: ResMut<GraphicsDebug>,
    mut panels: Query<&mut Visibility, With<PanelText>>,
) {
    if !bindings.just_pressed(&keys, Control::GraphicsSettings) {
        return;
    }
    debug.open = !debug.open;
    for mut panel in &mut panels {
        *panel = visibility(debug.open);
    }
}

/// The value after `current` among `options`, going round. A value that isn't one of them,
/// from the command line say, goes to the first.
fn next<T: Copy + PartialEq>(options: &[T], current: T) -> T {
    let place = options.iter().position(|&option| option == current);
    options[place.map_or(0, |place| (place + 1) % options.len())]
}

/// The keys only work while the panel says what they do.
fn change_settings(
    keys: Res<ButtonInput<KeyCode>>,
    debug: Res<GraphicsDebug>,
    mut camera: ResMut<CameraSettings>,
    mut environment: ResMut<EnvironmentSettings>,
    mut track: ResMut<TrackSettings>,
) {
    if !debug.open {
        return;
    }
    if keys.just_pressed(ANTIALIASING_KEY) {
        camera.antialiasing = !camera.antialiasing;
    }
    if keys.just_pressed(SHADOW_CASCADES_KEY) {
        environment.shadow_cascades = next(&SHADOW_CASCADES, environment.shadow_cascades);
    }
    if keys.just_pressed(SHADOW_DISTANCE_KEY) {
        environment.shadow_distance = next(&SHADOW_DISTANCES, environment.shadow_distance);
    }
    if keys.just_pressed(ANISOTROPY_KEY) {
        track.anisotropy = next(&ANISOTROPIES, track.anisotropy);
    }
}

/// A frame this many times longer than the average started late. An average of 60 fps
/// can be made of frames of 10 ms and 23 ms, which is not what it looks like.
const LATE_FACTOR: f64 = 1.25;

/// Frame times over the last `REFRESH`, as the wall clock sees them.
#[derive(Resource, Default)]
struct FrameTime {
    frames: Vec<Duration>,
    elapsed: Duration,
    /// `None` until the first `REFRESH` is over.
    report: Option<FrameReport>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FrameReport {
    /// In seconds.
    average: f64,
    /// In seconds.
    longest: f64,
    late: usize,
    frames: usize,
}

impl FrameReport {
    fn of(frames: &[Duration]) -> Option<Self> {
        let total: f64 = frames.iter().map(Duration::as_secs_f64).sum();
        let average = total / frames.len() as f64;
        (total > 0.0).then(|| Self {
            average,
            longest: frames.iter().map(Duration::as_secs_f64).fold(0.0, f64::max),
            late: frames
                .iter()
                .filter(|frame| frame.as_secs_f64() > average * LATE_FACTOR)
                .count(),
            frames: frames.len(),
        })
    }
}

/// By the wall clock, as `diagnostics` does, so that both say the same.
fn measure_frames(mut frame_time: ResMut<FrameTime>, mut last_frame: Local<Option<Instant>>) {
    let now = Instant::now();
    let Some(frame) = last_frame.replace(now).map(|last| now.duration_since(last)) else {
        return;
    };
    let frame_time = &mut *frame_time;
    frame_time.frames.push(frame);
    frame_time.elapsed += frame;
    if frame_time.elapsed >= REFRESH {
        frame_time.report = FrameReport::of(&frame_time.frames);
        frame_time.frames.clear();
        frame_time.elapsed = Duration::ZERO;
    }
}

/// Only when there is something new to say, since laying text out costs frame time too.
fn write_panel(
    debug: Res<GraphicsDebug>,
    frame_time: Res<FrameTime>,
    camera: Res<CameraSettings>,
    environment: Res<EnvironmentSettings>,
    track: Res<TrackSettings>,
    mut text: Single<&mut Text, With<PanelText>>,
) {
    let new_average = frame_time.is_changed() && frame_time.frames.is_empty();
    let changed = debug.is_changed()
        || camera.is_changed()
        || environment.is_changed()
        || track.is_changed()
        || text.is_added();
    if debug.open && (new_average || changed) {
        text.0 = panel_text(&camera, &environment, &track, frame_time.report);
    }
}

fn panel_text(
    camera: &CameraSettings,
    environment: &EnvironmentSettings,
    track: &TrackSettings,
    frames: Option<FrameReport>,
) -> String {
    let antialiasing = if camera.antialiasing {
        "4x MSAA"
    } else {
        "off"
    };
    let cascades = match environment.shadow_cascades {
        0 => "no shadows".to_string(),
        cascades => cascades.to_string(),
    };
    let anisotropy = if track.mipmaps {
        format!("{}x anisotropic", track.anisotropy)
    } else {
        "off (no mipmaps)".to_string()
    };
    let frame = match frames {
        Some(report) => format!(
            "{:.1} ms  ({:.0} fps)\n\
             Longest  {:.1} ms,  {} of {} frames late",
            report.average * 1000.0,
            1.0 / report.average,
            report.longest * 1000.0,
            report.late,
            report.frames,
        ),
        None => "...".to_string(),
    };
    format!(
        "Graphics   (its key again hides)\n\
         F3  Antialiasing      {antialiasing}\n\
         F4  Shadow cascades   {cascades}\n\
         F5  Shadow distance   {:.0} m\n\
         F6  Ground sharpness  {anisotropy}\n\
         Frame  {frame}",
        environment.shadow_distance,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_steps_through_the_values_and_round_again() {
        assert_eq!(next(&ANISOTROPIES, 1), 2);
        assert_eq!(next(&ANISOTROPIES, 16), 1);
        assert_eq!(next(&SHADOW_DISTANCES, 150.0), 300.0);
        // One the keys never give, as a command line might.
        assert_eq!(next(&SHADOW_CASCADES, 3), 0);
    }

    #[test]
    fn every_default_is_one_of_the_values() {
        assert!(SHADOW_CASCADES.contains(&EnvironmentSettings::default().shadow_cascades));
        assert!(SHADOW_DISTANCES.contains(&EnvironmentSettings::default().shadow_distance));
        assert!(ANISOTROPIES.contains(&TrackSettings::default().anisotropy));
    }

    #[test]
    fn the_panel_says_what_is_set() {
        let text = panel_text(
            &CameraSettings {
                antialiasing: false,
                ..default()
            },
            &EnvironmentSettings {
                shadow_cascades: 0,
                shadow_distance: 100.0,
            },
            &TrackSettings::default(),
            FrameReport::of(&[
                Duration::from_millis(8),
                Duration::from_millis(8),
                Duration::from_millis(14),
            ]),
        );
        assert!(text.contains("Antialiasing      off"), "{text}");
        assert!(text.contains("no shadows"), "{text}");
        assert!(text.contains("100 m"), "{text}");
        assert!(text.contains("16x anisotropic"), "{text}");
        assert!(text.contains("10.0 ms  (100 fps)"), "{text}");
        assert!(
            text.contains("Longest  14.0 ms,  1 of 3 frames late"),
            "{text}"
        );
    }
}
