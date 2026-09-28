//! The views the player can pick in a race: the chase camera, or the cockpit, from the
//! driver's seat. And in either, a look to the left, the right or behind, while a key is
//! held, to see the trucks round and behind the player's.
//!
//! The chase camera goes on following the truck whatever the view, so that it is settled
//! behind it when the player comes back to it. A view is laid over what it has placed, after
//! `CameraSystems::Place`:
//!
//! - Looking aside or back from the chase camera swings the eye round the truck, at the same
//!   distance and height, and keeps it clear of the ground.
//! - The cockpit is fixed to the drawn truck (`TruckVisual`), which is where the truck is
//!   between its physics steps, and so moves as smoothly as the truck does. It leans and
//!   pitches with the truck. The player's own truck is hidden in it: from inside, its body
//!   would fill the picture.
//!
//! Once the player's truck is on `truck::Autopilot`, as after the finish, the keys choose no
//! more: a new view is picked at random every `SHOW_EACH_VIEW` seconds (`direct_views`), as
//! a television picture of the race would change shots. It does not cut to it. The chase
//! camera swings round the truck to look another way, on a spring, so that it eases off and
//! eases in. Between the chase camera and the cockpit the picture fades to black and back
//! (`Fade`): a swing from one to the other would go through the truck's body.
//!
//! MTM2 has an in-cab view whose dashboard the truck's `Instrument Cluster` names, which
//! `dashboard` lays over this view. Where the eye sits here is the game's own: near the
//! front of the body and just under its roof. With a dashboard, it is tipped down a little,
//! so that the road is in the dashboard's window (`dashboard::eye_pitch`).

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use super::ChaseCamera;
use super::dashboard::eye_pitch;
use super::spring::follow_angle;
use crate::game_state::GameState;
use crate::keys::{Control, KeyBindings};
use crate::track::Track;
use crate::truck::{Autopilot, ChosenTruck, Player, SeenFromInside, TruckVisual};

/// Changes between the chase camera and the cockpit, as `Control::ChangeView`'s key does.
const VIEW_BUTTON: GamepadButton = GamepadButton::North;
/// How far the right stick must be pushed to look that way, from 0 to 1.
const STICK_LOOK: f32 = 0.5;

/// Where the eye sits in the cockpit: how far under the top of the body, in metres, and
/// how far forward from its middle, as a share of the way to its front. Higher up sees
/// more of the course over the bumps; further forward sees less of the bonnet.
const COCKPIT_BELOW_ROOF: f32 = 0.35;
const COCKPIT_FORWARD: f32 = 0.35;

/// On autopilot, how long each view is shown before another is picked, in seconds. Lower
/// changes shots more often.
const SHOW_EACH_VIEW: f32 = 7.0;
/// On autopilot, how quickly the chase camera swings round to look another way, per
/// second: it is most of the way round after `4 / SWING_STIFFNESS` seconds. Higher swings
/// faster.
const SWING_STIFFNESS: f32 = 2.5;
/// On autopilot, how long the picture takes to fade to black, and again to come back, in
/// seconds, between the chase camera and the cockpit.
const FADE_TIME: f32 = 0.4;
/// Over the 3D view and the dashboard (`dashboard::UNDER_THE_REST`), and under the rest of
/// the race's UI, which is at -1 and above.
const FADE_LAYER: i32 = -2;

/// The views picked from on autopilot: the chase camera looking each way, and the cockpit
/// looking ahead. From the cockpit, a look aside shows little but the inside of the cab.
const SHOTS: [(CameraView, Look); 5] = [
    (CameraView::Chase, Look::Ahead),
    (CameraView::Chase, Look::Left),
    (CameraView::Chase, Look::Right),
    (CameraView::Chase, Look::Back),
    (CameraView::Cockpit, Look::Ahead),
];

/// Which view the player's camera shows. Not remembered between races.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CameraView {
    /// Behind and above the truck.
    #[default]
    Chase,
    /// From the driver's seat.
    Cockpit,
}

/// Which way the player is looking, from the way the truck faces. Kept up to date in
/// `CameraSystems::Place`, for the dashboard.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Look {
    #[default]
    Ahead,
    Left,
    Right,
    Back,
}

impl Look {
    /// How far round from ahead, in radians, about the truck's up: left is positive.
    fn angle(self) -> f32 {
        match self {
            Look::Ahead => 0.0,
            Look::Left => std::f32::consts::FRAC_PI_2,
            Look::Right => -std::f32::consts::FRAC_PI_2,
            Look::Back => std::f32::consts::PI,
        }
    }

    /// The way the keys and a stick ask to look. `stick` is the right stick, right and up
    /// positive.
    fn asked(left: bool, right: bool, back: bool, stick: Vec2) -> Look {
        let left = left || stick.x < -STICK_LOOK;
        let right = right || stick.x > STICK_LOOK;
        let back = back || stick.y < -STICK_LOOK;
        match (left, right, back) {
            (_, _, true) | (true, true, _) => Look::Back,
            (true, false, false) => Look::Left,
            (false, true, false) => Look::Right,
            (false, false, false) => Look::Ahead,
        }
    }
}

/// On autopilot: the view picked, how far the camera has got to it, and how long until
/// another is picked.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub(super) struct Director {
    /// The view picked, which the camera swings or fades to. `None` until the first pick.
    shot: Option<(CameraView, Look)>,
    /// How far round from ahead the chase camera has swung, in radians (left is positive),
    /// and how fast it is swinging, in radians per second.
    swung: (f32, f32),
    /// How dark the picture is, from 0 (clear) to 1 (black).
    dark: f32,
    /// In seconds. At 0 or below, a view is picked at once.
    next_shot_in: f32,
    /// Stirred at every pick, for the next one.
    seed: u64,
}

/// Over the whole window: black, as dark as `Director::dark` says.
#[derive(Component)]
pub(super) struct Fade;

/// Its key, or the gamepad's top button, changes the view. Not on autopilot, where
/// `direct_views` chooses.
pub(super) fn change_view(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<KeyBindings>,
    gamepads: Query<&Gamepad>,
    mut view: ResMut<CameraView>,
    mut camera: Single<&mut ChaseCamera>,
    player: Single<Has<Autopilot>, Player>,
) {
    if *player {
        return;
    }
    let asked = bindings.just_pressed(&keys, Control::ChangeView)
        || gamepads
            .iter()
            .any(|gamepad| gamepad.just_pressed(VIEW_BUTTON));
    if !asked {
        return;
    }
    *view = match *view {
        CameraView::Chase => CameraView::Cockpit,
        CameraView::Cockpit => CameraView::Chase,
    };
    // Back to the chase camera, it starts again settled behind the truck.
    camera.snap();
}

/// A new race starts with the chase camera, looking ahead.
pub(super) fn reset_view(
    time: Res<Time<bevy::time::Real>>,
    mut view: ResMut<CameraView>,
    mut looking: ResMut<Look>,
    mut director: ResMut<Director>,
) {
    *view = CameraView::Chase;
    *looking = Look::Ahead;
    // From the clock, so that each race shows its own order of views.
    *director = Director {
        seed: time.elapsed().as_nanos() as u64,
        ..default()
    };
}

/// On autopilot, picks another view at random when the one shown has had its time (the
/// first at once), and takes the camera there: a swing round the truck, or to and from the
/// cockpit a fade through black, changing over while the picture is black.
pub(super) fn direct_views(
    time: Res<Time>,
    player: Single<Has<Autopilot>, Player>,
    mut view: ResMut<CameraView>,
    mut director: ResMut<Director>,
    mut camera: Single<&mut ChaseCamera>,
) {
    if !*player {
        return;
    }
    let dt = time.delta_secs();
    director.next_shot_in -= dt;
    if director.next_shot_in <= 0.0 {
        director.next_shot_in = SHOW_EACH_VIEW;
        director.seed = stir(director.seed);
        let current = director.shot.unwrap_or((*view, Look::Ahead));
        director.shot = Some(next_shot(current, director.seed));
    }
    let Some((wanted, look)) = director.shot else {
        return;
    };
    if *view != wanted {
        director.dark = (director.dark + dt / FADE_TIME).min(1.0);
        if director.dark >= 1.0 {
            *view = wanted;
            // Unseen in the dark, it is already looking the new way.
            director.swung = (look.angle(), 0.0);
            if wanted == CameraView::Chase {
                // Back to the chase camera, it starts again settled behind the truck.
                camera.snap();
            }
        }
    } else {
        director.dark = (director.dark - dt / FADE_TIME).max(0.0);
        director.swung = follow_angle(director.swung, look.angle(), SWING_STIFFNESS, dt);
    }
}

pub(super) fn spawn_fade(mut commands: Commands) {
    commands.spawn((
        Fade,
        DespawnOnExit(GameState::Racing),
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(Color::NONE),
        GlobalZIndex(FADE_LAYER),
    ));
}

/// After `direct_views`.
pub(super) fn show_fade(
    director: Res<Director>,
    mut fade: Single<&mut BackgroundColor, With<Fade>>,
) {
    let colour = BackgroundColor(Color::BLACK.with_alpha(director.dark));
    fade.set_if_neq(colour);
}

/// One of `SHOTS` other than `current`, which `roll` picks.
fn next_shot(current: (CameraView, Look), roll: u64) -> (CameraView, Look) {
    let others: Vec<_> = SHOTS.into_iter().filter(|shot| *shot != current).collect();
    others[(roll % others.len() as u64) as usize]
}

/// A number that looks random, from `state` (splitmix64), so that seeds close together
/// give numbers far apart.
fn stir(state: u64) -> u64 {
    let mut mixed = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}

/// Lays the view and the look over where the chase camera has put the eye (on autopilot,
/// as far round as `direct_views` has swung it), and in the cockpit tells the truck slice
/// that the player's truck is seen from inside, which hides its body and keeps its lamps
/// lit (`truck::SeenFromInside`).
#[allow(clippy::too_many_arguments)]
pub(super) fn place_view(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<KeyBindings>,
    gamepads: Query<&Gamepad>,
    view: Res<CameraView>,
    mut looking: ResMut<Look>,
    director: Res<Director>,
    track: Res<Track>,
    chosen: Res<ChosenTruck>,
    player: Single<(Entity, &Collider, Has<Autopilot>), Player>,
    visuals: Query<(Entity, &TruckVisual, &Transform, Has<SeenFromInside>), Without<ChaseCamera>>,
    camera: Single<(&ChaseCamera, &mut Transform, Option<&Projection>)>,
) {
    let (player, collider, autopilot) = *player;
    let Some((drawn, _, truck, inside)) = visuals
        .iter()
        .find(|(_, visual, _, _)| visual.truck == player)
    else {
        return;
    };
    let cockpit = *view == CameraView::Cockpit;
    if cockpit && !inside {
        commands.entity(drawn).insert(SeenFromInside);
    } else if !cockpit && inside {
        commands.entity(drawn).remove::<SeenFromInside>();
    }

    let stick = gamepads.iter().fold(Vec2::ZERO, |sum, gamepad| {
        sum + Vec2::new(
            gamepad.get(GamepadAxis::RightStickX).unwrap_or(0.0),
            gamepad.get(GamepadAxis::RightStickY).unwrap_or(0.0),
        )
    });
    // On autopilot the cockpit only looks ahead, and the chase camera goes by `swung`.
    let look = if autopilot {
        Look::Ahead
    } else {
        Look::asked(
            bindings.pressed(&keys, Control::LookLeft),
            bindings.pressed(&keys, Control::LookRight),
            bindings.pressed(&keys, Control::LookBack),
            stick,
        )
    };
    looking.set_if_neq(look);
    let angle = if autopilot {
        director.swung.0
    } else {
        look.angle()
    };

    let (chase, mut transform, projection) = camera.into_inner();
    if cockpit {
        let body = collider.raw.compute_local_aabb();
        let eye = cockpit_eye(body.mins, body.maxs);
        let pitch = match (&chosen.dashboard, projection) {
            (Some(dashboard), Some(Projection::Perspective(perspective))) => {
                eye_pitch(dashboard, perspective.fov)
            }
            _ => 0.0,
        };
        *transform = cockpit_pose(truck, eye, look, pitch);
    } else if angle != 0.0 {
        let aim = truck.translation + Vec3::Y * chase.config.look_above;
        let eye = swing(transform.translation, aim, angle);
        let ground = track.heights.height_at(eye.x, eye.z);
        let eye = eye.with_y(eye.y.max(ground + chase.config.clearance));
        *transform = Transform::from_translation(eye).looking_at(aim, Vec3::Y);
    }
}

/// Where the driver's eye is in a body that reaches from `mins` to `maxs`, in the body's
/// own space, whose front is towards -Z.
fn cockpit_eye(mins: Vec3, maxs: Vec3) -> Vec3 {
    let middle = (mins.z + maxs.z) / 2.0;
    Vec3::new(
        (mins.x + maxs.x) / 2.0,
        maxs.y - COCKPIT_BELOW_ROOF,
        middle + (mins.z - middle) * COCKPIT_FORWARD,
    )
}

/// The camera in the cockpit of a truck drawn at `truck`, with its eye at `eye` in the
/// truck's own space, looking `look`, tipped down by `pitch` radians.
fn cockpit_pose(truck: &Transform, eye: Vec3, look: Look, pitch: f32) -> Transform {
    Transform {
        translation: truck.transform_point(eye),
        rotation: truck.rotation
            * Quat::from_rotation_y(look.angle())
            * Quat::from_rotation_x(-pitch),
        scale: Vec3::ONE,
    }
}

/// The chase camera's eye at `eye`, swung `angle` radians round `aim` (left is positive,
/// as `Look::angle`): behind the truck to look ahead, beside it on the right to look left,
/// and in front of it to look back.
fn swing(eye: Vec3, aim: Vec3, angle: f32) -> Vec3 {
    aim + Quat::from_rotation_y(angle) * (eye - aim)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_the_stick_ask_which_way_to_look() {
        let none = Vec2::ZERO;
        assert_eq!(Look::asked(false, false, false, none), Look::Ahead);
        assert_eq!(Look::asked(true, false, false, none), Look::Left);
        assert_eq!(Look::asked(false, true, false, none), Look::Right);
        assert_eq!(Look::asked(false, false, true, none), Look::Back);
        assert_eq!(Look::asked(true, true, false, none), Look::Back);
        assert_eq!(Look::asked(false, false, false, Vec2::X), Look::Right);
        assert_eq!(Look::asked(false, false, false, -Vec2::Y), Look::Back);
        assert_eq!(Look::asked(false, false, false, Vec2::X * 0.2), Look::Ahead);
    }

    #[test]
    fn each_view_picked_on_autopilot_is_another_and_any_other_can_come() {
        for current in SHOTS {
            let picked: Vec<_> = (0..200)
                .map(|seed| next_shot(current, stir(seed)))
                .collect();
            assert!(!picked.contains(&current));
            for shot in SHOTS.into_iter().filter(|shot| *shot != current) {
                assert!(picked.contains(&shot), "{shot:?} after {current:?}");
            }
        }
    }

    #[test]
    fn looking_left_from_the_chase_camera_puts_it_on_the_right() {
        // A truck facing -Z, with the camera behind it.
        let aim = Vec3::ZERO;
        let eye = Vec3::new(0.0, 3.0, 10.0);
        let left = swing(eye, aim, Look::Left.angle());
        assert!(left.abs_diff_eq(Vec3::new(10.0, 3.0, 0.0), 1e-4), "{left}");
        let back = swing(eye, aim, Look::Back.angle());
        assert!(back.z < -9.9, "{back}");
        assert_eq!(swing(eye, aim, Look::Ahead.angle()), eye);
    }

    #[test]
    fn the_cockpit_looks_the_way_the_truck_faces_and_round() {
        let truck = Transform::from_xyz(5.0, 1.0, 0.0);
        let eye = cockpit_eye(Vec3::new(-1.0, -0.5, -2.0), Vec3::new(1.0, 1.5, 2.0));
        assert_eq!(
            eye,
            Vec3::new(0.0, 1.5 - COCKPIT_BELOW_ROOF, -2.0 * COCKPIT_FORWARD)
        );
        let ahead = cockpit_pose(&truck, eye, Look::Ahead, 0.0);
        assert!(ahead.forward().as_vec3().abs_diff_eq(Vec3::NEG_Z, 1e-5));
        assert!(ahead.translation.abs_diff_eq(truck.translation + eye, 1e-5));
        let left = cockpit_pose(&truck, eye, Look::Left, 0.0);
        assert!(left.forward().as_vec3().abs_diff_eq(Vec3::NEG_X, 1e-5));
        let back = cockpit_pose(&truck, eye, Look::Back, 0.0);
        assert!(back.forward().as_vec3().abs_diff_eq(Vec3::Z, 1e-5));
        // Tipped down, it still looks the way it turned.
        let down = cockpit_pose(&truck, eye, Look::Left, 0.1)
            .forward()
            .as_vec3();
        assert!(down.y < 0.0 && down.x < -0.99, "{down}");
    }
}
