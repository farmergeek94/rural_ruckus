//! A compass at the top of the screen whose needle points to the player's next
//! checkpoint.
//!
//! The needle is turned against the heading of the drawn truck, not the camera: up is
//! straight ahead of the truck, so the needle says which way to steer. The drawn truck
//! moves smoothly from frame to frame, where the body moves in physics steps.

use bevy::prelude::*;

use super::Racer;
use crate::game_state::GameState;
use crate::track::Track;
use crate::truck::{PlayerTruck, TruckVisual};

/// Width and height of the compass dial, in pixels.
const DIAL_SIZE: f32 = 72.0;
/// Length of the needle from the middle of the dial to the base of its head, in pixels.
const NEEDLE_LENGTH: f32 = 22.0;
/// Width of the needle, in pixels.
const NEEDLE_WIDTH: f32 = 5.0;
/// Side of the square that, turned by 45 degrees, makes the needle's head, in pixels.
const HEAD_SIZE: f32 = 14.0;

const DIAL_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.45);
const RIM_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.8);
const NEEDLE_COLOR: Color = Color::srgb(1.0, 0.75, 0.1);

/// The whole compass, hidden when there is no checkpoint to go to.
#[derive(Component)]
pub(super) struct Compass;

/// The part of the compass that turns: the needle and its head.
#[derive(Component)]
pub(super) struct CompassNeedle;

pub(super) fn spawn_compass(mut commands: Commands) {
    // A row the width of the screen, so that the dial sits in the middle of it.
    commands
        .spawn((
            Compass,
            DespawnOnExit(GameState::Racing),
            Node {
                position_type: PositionType::Absolute,
                top: px(12),
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                Node {
                    width: px(DIAL_SIZE),
                    height: px(DIAL_SIZE),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(DIAL_COLOR),
                BorderColor::all(RIM_COLOR),
            ))
            .with_children(|dial| {
                // Fills the dial, so that it turns about the dial's middle.
                dial.spawn((
                    CompassNeedle,
                    Node {
                        position_type: PositionType::Absolute,
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    },
                ))
                .with_children(|needle| {
                    let middle = (DIAL_SIZE - 4.0) / 2.0;
                    needle.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(middle - NEEDLE_WIDTH / 2.0),
                            top: px(middle - NEEDLE_LENGTH),
                            width: px(NEEDLE_WIDTH),
                            height: px(NEEDLE_LENGTH),
                            ..default()
                        },
                        BackgroundColor(NEEDLE_COLOR),
                    ));
                    needle.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(middle - HEAD_SIZE / 2.0),
                            top: px(middle - NEEDLE_LENGTH - HEAD_SIZE / 2.0),
                            width: px(HEAD_SIZE),
                            height: px(HEAD_SIZE),
                            ..default()
                        },
                        UiTransform::from_rotation(Rot2::degrees(45.0)),
                        BackgroundColor(NEEDLE_COLOR),
                    ));
                });
            });
        });
}

/// Turns the needle towards the player's next checkpoint.
pub(super) fn update_compass(
    track: Res<Track>,
    player: Single<(Entity, &Racer), With<PlayerTruck>>,
    visuals: Query<(&TruckVisual, &Transform)>,
    mut compass: Single<&mut Visibility, With<Compass>>,
    mut needle: Single<&mut UiTransform, With<CompassNeedle>>,
) {
    let (player, racer) = *player;
    let gate = track.gates.get(racer.progress.next_gate);
    // A handful of trucks at most.
    let visual = visuals.iter().find(|(visual, _)| visual.truck == player);
    let (Some(gate), Some((_, transform)), None) = (gate, visual, racer.progress.finished) else {
        compass.set_if_neq(Visibility::Hidden);
        return;
    };
    compass.set_if_neq(Visibility::Inherited);
    let forward = transform.forward().as_vec3().xz();
    let bearing = bearing(transform.translation.xz(), forward, gate.center);
    needle.rotation = Rot2::radians(bearing);
}

/// Angle from straight ahead to `target`, as seen from `position` facing `forward` (all
/// world X and Z), in radians: positive to the right, negative to the left, within ±π.
/// A needle turned clockwise by this points at the target.
fn bearing(position: Vec2, forward: Vec2, target: Vec2) -> f32 {
    let to_target = target - position;
    // The truck's right on the ground plane: -Z ahead has +X on the right.
    let right = Vec2::new(-forward.y, forward.x);
    to_target.dot(right).atan2(to_target.dot(forward))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    const AHEAD: Vec2 = Vec2::new(0.0, -1.0);

    #[test]
    fn straight_ahead_is_zero() {
        assert!(bearing(Vec2::ZERO, AHEAD, Vec2::new(0.0, -50.0)).abs() < 1e-6);
    }

    #[test]
    fn right_is_positive_and_left_negative() {
        // A truck with no rotation faces -Z and has +X on its right.
        let right = bearing(Vec2::ZERO, AHEAD, Vec2::new(10.0, 0.0));
        let left = bearing(Vec2::ZERO, AHEAD, Vec2::new(-10.0, 0.0));
        assert!((right - FRAC_PI_2).abs() < 1e-6);
        assert!((left + FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn behind_is_half_a_turn() {
        let behind = bearing(Vec2::ZERO, AHEAD, Vec2::new(0.0, 10.0));
        assert!((behind.abs() - PI).abs() < 1e-6);
    }

    #[test]
    fn follows_the_truck_round() {
        // Facing +X, with the target further along +X from somewhere else.
        let facing_x = Vec2::new(1.0, 0.0);
        let target = Vec2::new(20.0, 5.0);
        assert!(bearing(Vec2::new(5.0, 5.0), facing_x, target).abs() < 1e-6);
        // Facing +X, +Z is on the right.
        let right = bearing(Vec2::ZERO, facing_x, Vec2::new(0.0, 10.0));
        assert!((right - FRAC_PI_2).abs() < 1e-6);
    }
}
