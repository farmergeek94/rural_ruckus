//! A score over every truck in a Summit Rumble: its points, in the application's blue
//! as the countdown's numbers are, so that the player sees how everyone stands while they
//! fight. Each is UI text placed over the drawn truck through the race camera each frame.
//!
//! The number is drawn at its largest and scaled down by how far the truck is from the
//! eye, within limits, so that it reads as part of the scene. Scaling the node, rather
//! than the font, keeps one font atlas and lays the text out once (Bevy makes an atlas
//! for every font size). It is placed in `PostUpdate` before the UI is laid out, from the
//! camera's `Transform` of the frame (the camera is placed in `Update`, and its
//! `GlobalTransform` is only worked out after the UI), and moved by its `UiTransform`, so
//! that moving it costs no layout. Without a camera, as in a headless app, nothing is
//! shown.

use bevy::prelude::*;

use super::rumble::RumbleScore;
use super::{BLUE, Racer};
use crate::game_state::GameState;
use crate::truck::{SeenFromInside, TruckVisual};

/// How far over the drawn truck's middle the number sits, straight up, in metres: just
/// over its roof. Straight up, not the truck's up, so that it stays over a truck on its
/// side.
const ABOVE: f32 = 2.1;
/// How big the number is, in pixels, over a truck `AT` metres from the eye, and the least
/// and the most it is at any distance. It is drawn at `LARGEST` and scaled down.
const SIZE: f32 = 40.0;
const AT: f32 = 15.0;
const SMALLEST: f32 = 16.0;
const LARGEST: f32 = 64.0;
/// The box the number is centred in, in pixels, unscaled: room for any score, and a
/// line of the largest text.
const WIDTH: f32 = 240.0;
const HEIGHT: f32 = LARGEST * 1.25;
const SHADOW: Color = Color::srgba(0.0, 0.0, 0.0, 0.6);

/// The number over one truck.
#[derive(Component)]
pub struct ScoreLabel {
    pub truck: Entity,
    /// The whole points on show.
    shown: i64,
}

/// Gives each new racer its number, hidden until it is placed.
pub(super) fn add_labels(mut commands: Commands, racers: Query<Entity, Added<Racer>>) {
    for truck in &racers {
        commands.spawn((
            ScoreLabel { truck, shown: 0 },
            DespawnOnExit(GameState::Racing),
            Text::new("0"),
            TextFont::from_font_size(LARGEST),
            TextColor(BLUE),
            TextShadow {
                offset: Vec2::splat(3.0),
                color: SHADOW,
            },
            TextLayout::justify(Justify::Center),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: px(WIDTH),
                height: px(HEIGHT),
                ..default()
            },
            Visibility::Hidden,
            UiTransform::default(),
            // Under the readout and the results: part of the scene, not of the screen.
            GlobalZIndex(-1),
        ));
    }
}

/// Puts each number over its drawn truck, as the race camera sees it, scaled by its
/// distance. A truck the camera cannot see, or is inside, has none.
pub(super) fn place_labels(
    cameras: Query<(&Camera, &Transform), With<Camera3d>>,
    visuals: Query<(&TruckVisual, &Transform, Has<SeenFromInside>)>,
    mut labels: Query<(&ScoreLabel, &mut UiTransform, &mut Visibility)>,
) {
    let eye = cameras
        .iter()
        .find(|(camera, _)| camera.is_active)
        .map(|(camera, transform)| (camera, GlobalTransform::from(*transform)));
    for (label, mut transform, mut visibility) in &mut labels {
        let seen = eye.as_ref().and_then(|(camera, eye)| {
            // A handful of trucks at most.
            let (_, truck, inside) = visuals
                .iter()
                .find(|(visual, _, _)| visual.truck == label.truck)?;
            if inside {
                return None;
            }
            let over = truck.translation + Vec3::Y * ABOVE;
            camera.world_to_viewport_with_depth(eye, over).ok()
        });
        let Some(at) = seen else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        visibility.set_if_neq(Visibility::Inherited);
        let scale = (SIZE * AT / at.z).clamp(SMALLEST, LARGEST) / LARGEST;
        transform.scale = Vec2::splat(scale);
        // The box is scaled about its middle: its bottom edge, scaled, on the point.
        transform.translation = Val2::px(at.x - WIDTH / 2.0, at.y - HEIGHT * (1.0 + scale) / 2.0);
    }
}

/// Puts each truck's whole points in its number, when they change.
pub(super) fn number_labels(
    scores: Query<&RumbleScore>,
    mut labels: Query<(&mut ScoreLabel, &mut Text)>,
) {
    for (mut label, mut text) in &mut labels {
        let Ok(score) = scores.get(label.truck) else {
            continue;
        };
        let points = score.0.points.floor() as i64;
        if label.shown != points {
            label.shown = points;
            text.0 = points.to_string();
        }
    }
}
