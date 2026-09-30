//! The dashboard, laid over the cockpit view: the player's truck's `truck::Dashboard`, its
//! picture for the way the player looks, and looking ahead its steering wheel and the
//! needles of its dials. See `docs/formats/cockpit.md`.
//!
//! It is UI, drawn over the 3D view, so the scene's light and fog never touch it: it reads
//! as well at night as by day, as a backlit dashboard does. Its picture is stretched over
//! the whole window, whatever the window's shape, and the road shows through its see-through
//! pixels. Everything on it is placed as a share of the layout's screen (640 x 480), so
//! that it stretches with the picture: across by the window's width over 640, down by its
//! height over 480. The picture's pixels become a texture each as they are; the needles are
//! the game's own thin bars, not MTM2's needle model, and follow their dials as stretched.
//!
//! Hidden in every other view, and for a truck without a dashboard. Drawn only, and only
//! when the app can draw.

use avian3d::prelude::LinearVelocity;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::views::{CameraView, Look};
use crate::game_state::GameState;
use crate::truck::{
    ChosenTruck, Dashboard, DashboardPicture, Dial, Player, TruckConfig, TruckInput,
};

/// How thick a needle is, in the dashboard's pixels. The game's own.
const NEEDLE_WIDTH: f32 = 2.5;
/// The needles' colour: the application's blue, which stands out on the black dials.
const NEEDLE_COLOR: Color = Color::srgb(0.2, 0.5, 0.96);
/// How far round a needle can go from its zero, in radians: a little past the dials' last
/// marks (100 mph is 265 degrees round, 9,000 rpm 235), and never all the way round. The
/// game's own.
const MOST_SWEEP: f32 = 300.0 * std::f32::consts::PI / 180.0;
/// The tachometer: the truck has no engine speed, so the needle stands at `IDLE_RPM` and
/// goes up to `TOP_RPM` with the truck's speed, as a share of its top speed, as if in one
/// gear. The game's own, not MTM2's.
const IDLE_RPM: f32 = 1000.0;
const TOP_RPM: f32 = 7000.0;
/// Where the horizon is put in the dashboard's 3D window, as a share of the way down it,
/// when the truck stands level: higher shows more of the road in front. The game's own.
const HORIZON_IN_WINDOW: f32 = 0.4;
/// Under the rest of the race's UI, which is at -1 and above, and under the fade between
/// views (`views::FADE_LAYER`), which darkens the dashboard with the 3D view.
const UNDER_THE_REST: i32 = -3;

/// The dashboard's parts, and what they are drawn from.
#[derive(Component)]
pub(super) struct DashboardRoot {
    dashboard: std::sync::Arc<Dashboard>,
    /// The texture of each of `Dashboard::views`.
    views: [Option<Handle<Image>>; 4],
    /// The texture of each of the steering wheel's frames, in its order.
    wheel: Vec<Handle<Image>>,
}

#[derive(Component)]
pub(super) struct Background;

#[derive(Component)]
pub(super) struct Wheel;

#[derive(Component, Clone, Copy)]
pub(super) enum Needle {
    Speed,
    Revs,
}

/// Builds the dashboard of the truck the player drives, hidden until the cockpit view.
pub(super) fn spawn_dashboard(
    mut commands: Commands,
    chosen: Res<ChosenTruck>,
    images: Option<ResMut<Assets<Image>>>,
) {
    let (Some(dashboard), Some(mut images)) = (chosen.dashboard.clone(), images) else {
        return;
    };
    let mut texture = |picture: &DashboardPicture| images.add(texture(picture));
    let views = dashboard
        .views
        .each_ref()
        .map(|view| view.as_ref().map(&mut texture));
    let wheel: Vec<Handle<Image>> = dashboard
        .steering_wheel
        .iter()
        .flat_map(|wheel| &wheel.frames)
        .map(|(_, picture)| texture(picture))
        .collect();

    let screen = dashboard.screen;
    let parts = |parent: &mut ChildSpawnerCommands| {
        parent.spawn((
            Background,
            ImageNode {
                image_mode: NodeImageMode::Stretch,
                ..default()
            },
            placed(Rect::from_corners(Vec2::ZERO, screen), screen),
        ));
        if let Some(steering_wheel) = &dashboard.steering_wheel {
            parent.spawn((
                Wheel,
                ImageNode {
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
                placed(steering_wheel.rect, screen),
            ));
        }
        for (needle, dial) in [
            (Needle::Speed, dashboard.speedometer),
            (Needle::Revs, dashboard.tachometer),
        ] {
            let Some(dial) = dial else {
                continue;
            };
            // A bar twice the needle's length, turned about its middle, which is the
            // dial's middle, with only its upper half coloured: straight up at no turn.
            let half = Vec2::new(NEEDLE_WIDTH / 2.0, dial.radius);
            let bar = Rect::from_corners(dial.center - half, dial.center + half);
            parent.spawn((
                needle,
                placed(bar, screen),
                UiTransform::default(),
                children![(
                    Node {
                        width: percent(100),
                        height: percent(50),
                        ..default()
                    },
                    BackgroundColor(NEEDLE_COLOR),
                )],
            ));
        }
    };

    commands
        .spawn((
            DashboardRoot {
                dashboard: dashboard.clone(),
                views,
                wheel,
            },
            DespawnOnExit(GameState::Racing),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(UNDER_THE_REST),
            Visibility::Hidden,
        ))
        .with_children(parts);
}

/// Shows the dashboard in the cockpit, with the picture for the way the player looks, and
/// looking ahead moves its wheel and needles.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn show_dashboard(
    view: Res<CameraView>,
    look: Res<Look>,
    root: Option<Single<(&DashboardRoot, &ComputedNode, &mut Visibility)>>,
    mut background: Query<&mut ImageNode, (With<Background>, Without<Wheel>)>,
    mut wheel: Query<(&mut ImageNode, &mut Visibility), (With<Wheel>, Without<DashboardRoot>)>,
    mut needles: Query<
        (&Needle, &mut UiTransform, &mut Visibility),
        (Without<Wheel>, Without<DashboardRoot>),
    >,
    player: Single<(&LinearVelocity, &TruckInput, &TruckConfig), Player>,
) {
    let Some(root) = root else {
        return;
    };
    let (root, computed, mut visibility) = root.into_inner();
    let picture = root.views[look_index(*look)].clone();
    let shown = *view == CameraView::Cockpit && picture.is_some();
    visibility.set_if_neq(if shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    if !shown {
        return;
    }

    let widened = widening(computed.size(), root.dashboard.screen);
    for mut image in &mut background {
        if let Some(picture) = &picture
            && image.image != *picture
        {
            image.image = picture.clone();
        }
    }

    let ahead = *look == Look::Ahead;
    let seen = if ahead {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let (velocity, input, config) = *player;
    let dashboard = &root.dashboard;
    for (mut image, mut visibility) in &mut wheel {
        visibility.set_if_neq(seen);
        let Some(steering_wheel) = &dashboard.steering_wheel else {
            continue;
        };
        let turns: Vec<f32> = steering_wheel
            .frames
            .iter()
            .map(|(turn, _)| *turn)
            .collect();
        if let Some(frame) = root.wheel.get(wheel_frame(&turns, input.steer))
            && image.image != *frame
        {
            image.image = frame.clone();
        }
    }
    let speed = velocity.0.length();
    for (needle, mut transform, mut visibility) in &mut needles {
        visibility.set_if_neq(seen);
        let (dial, reading) = match needle {
            Needle::Speed => (dashboard.speedometer, speed),
            Needle::Revs => (dashboard.tachometer, revs(speed, config.top_speed)),
        };
        if let Some(dial) = dial {
            let (angle, length) = stretched_needle(needle_angle(&dial, reading), widened);
            transform.rotation = Rot2::radians(angle);
            transform.scale = Vec2::new(1.0, length);
        }
    }
}

/// How far to tip the cockpit's eye down, in radians, so that a level horizon is
/// `HORIZON_IN_WINDOW` of the way down the dashboard's 3D window, for a camera that sees
/// `fov` radians from the top of the window to the bottom. MTM2 drew the road into that
/// window alone, so its horizon was in the window's middle; here the road fills the
/// window, whose middle is lower than the window's. The picture is as high as the window,
/// so this holds whatever the window's shape.
pub(super) fn eye_pitch(dashboard: &Dashboard, fov: f32) -> f32 {
    let Some(window) = dashboard.window else {
        return 0.0;
    };
    let middle = dashboard.screen.y / 2.0;
    if middle <= 0.0 {
        return 0.0;
    }
    let horizon = window.min.y + window.height() * HORIZON_IN_WINDOW;
    ((middle - horizon) / middle * (fov / 2.0).tan()).atan()
}

/// Which of `Dashboard::views` a look shows.
fn look_index(look: Look) -> usize {
    match look {
        Look::Ahead => 0,
        Look::Left => 1,
        Look::Right => 2,
        Look::Back => 3,
    }
}

/// A node over `rect` of a dashboard `screen` pixels big, placed as shares of it.
fn placed(rect: Rect, screen: Vec2) -> Node {
    let share = |pixels: f32, of: f32| percent(pixels / of * 100.0);
    Node {
        position_type: PositionType::Absolute,
        left: share(rect.min.x, screen.x),
        top: share(rect.min.y, screen.y),
        width: share(rect.width(), screen.x),
        height: share(rect.height(), screen.y),
        ..default()
    }
}

/// How much more a picture of a dashboard with a `screen` (640 x 480) is stretched across
/// than down, to cover a window `window` big: 1 for a 4:3 window, 4/3 for a 16:9 one.
fn widening(window: Vec2, screen: Vec2) -> f32 {
    let (across, down) = (window.x / screen.x, window.y / screen.y);
    if across > 0.0 && down > 0.0 && across.is_finite() && down.is_finite() {
        across / down
    } else {
        1.0
    }
}

/// A needle pointing `angle` radians clockwise from straight up on its dial, as it is drawn
/// on that dial stretched `widening` times more across than down: which way it points on
/// the screen, clockwise from straight up, and how long it is there, as a share of its
/// length straight up. Its tip stays on the stretched dial, so it points at the same mark.
fn stretched_needle(angle: f32, widening: f32) -> (f32, f32) {
    // The tip, as (across, up) from the middle of an unstretched dial of radius 1.
    let (across, up) = (angle.sin() * widening, angle.cos());
    (across.atan2(up), across.hypot(up))
}

/// Where a dial's needle points at `reading`, in radians clockwise from straight up.
fn needle_angle(dial: &Dial, reading: f32) -> f32 {
    let sweep = (dial.per_unit * reading.max(0.0)).clamp(-MOST_SWEEP, MOST_SWEEP);
    dial.zero + sweep
}

/// The steering wheel's frame nearest to `steer` (-1 right to 1 left), of frames turned as
/// far as `turns`, in the same measure. The frame turned furthest is a full turn of the
/// wheel.
fn wheel_frame(turns: &[f32], steer: f32) -> usize {
    turns
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (*a - steer).abs().total_cmp(&(*b - steer).abs()))
        .map_or(0, |(frame, _)| frame)
}

/// The engine speed the tachometer shows at `speed`, in metres a second, for a truck whose
/// top speed is `top_speed`.
fn revs(speed: f32, top_speed: f32) -> f32 {
    let share = if top_speed > 0.0 {
        (speed / top_speed).clamp(0.0, 1.0)
    } else {
        0.0
    };
    IDLE_RPM + (TOP_RPM - IDLE_RPM) * share
}

fn texture(picture: &DashboardPicture) -> Image {
    Image::new(
        Extent3d {
            width: picture.width,
            height: picture.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        picture.rgba.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picture_is_stretched_to_the_window() {
        let screen = Vec2::new(640.0, 480.0);
        assert_eq!(widening(Vec2::new(1280.0, 960.0), screen), 1.0);
        assert!((widening(Vec2::new(1920.0, 1080.0), screen) - 4.0 / 3.0).abs() < 1e-6);
        assert!((widening(Vec2::new(800.0, 1000.0), screen) - 0.6).abs() < 1e-6);
        assert_eq!(widening(Vec2::ZERO, screen), 1.0);
    }

    #[test]
    fn a_stretched_needle_still_points_at_its_mark() {
        use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};
        // Unstretched, nothing changes.
        let (angle, length) = stretched_needle(1.0, 1.0);
        assert!((angle - 1.0).abs() < 1e-6 && (length - 1.0).abs() < 1e-6);
        // Straight up, down and across keep their way, and across grows longer.
        assert!(stretched_needle(0.0, 2.0).0.abs() < 1e-6);
        assert!((stretched_needle(PI, 2.0).0.abs() - PI).abs() < 1e-6);
        let (angle, length) = stretched_needle(FRAC_PI_2, 2.0);
        assert!((angle - FRAC_PI_2).abs() < 1e-6 && (length - 2.0).abs() < 1e-6);
        // Half-way between up and across leans towards across, to the stretched mark.
        let (angle, length) = stretched_needle(FRAC_PI_4, 2.0);
        assert!((angle - 2f32.atan()).abs() < 1e-6);
        assert!((length - (2.5f32).sqrt()).abs() < 1e-5);
        // Up and to the left, 300 degrees round, stays up and to the left.
        let (angle, _) = stretched_needle(300f32.to_radians(), 4.0 / 3.0);
        assert!(angle.sin() < 0.0 && angle.cos() > 0.0);
    }

    #[test]
    fn a_needle_turns_clockwise_from_its_zero_and_never_laps() {
        let dial = Dial {
            center: Vec2::ZERO,
            radius: 30.0,
            zero: 300f32.to_radians(),
            per_unit: 2f32.to_radians(),
        };
        assert!((needle_angle(&dial, 0.0) - dial.zero).abs() < 1e-6);
        assert!((needle_angle(&dial, 30.0) - 360f32.to_radians()).abs() < 1e-5);
        assert_eq!(needle_angle(&dial, -5.0), dial.zero);
        assert!((needle_angle(&dial, 1000.0) - dial.zero - MOST_SWEEP).abs() < 1e-5);
    }

    #[test]
    fn the_wheel_shows_the_frame_nearest_the_steering() {
        let turns = [-1.0, -0.5, 0.0, 0.5, 1.0];
        assert_eq!(wheel_frame(&turns, 0.0), 2);
        assert_eq!(wheel_frame(&turns, 0.1), 2);
        assert_eq!(wheel_frame(&turns, 0.4), 3);
        assert_eq!(wheel_frame(&turns, 1.0), 4);
        assert_eq!(wheel_frame(&turns, -0.9), 0);
        assert_eq!(wheel_frame(&[], 0.3), 0);
    }

    #[test]
    fn the_engine_idles_at_rest_and_revs_with_speed() {
        assert_eq!(revs(0.0, 30.0), IDLE_RPM);
        assert_eq!(revs(30.0, 30.0), TOP_RPM);
        assert_eq!(revs(60.0, 30.0), TOP_RPM);
        assert_eq!(revs(10.0, 0.0), IDLE_RPM);
    }

    #[test]
    fn the_eye_tips_down_to_put_the_horizon_in_the_window() {
        let mut dashboard = Dashboard {
            screen: Vec2::new(640.0, 480.0),
            window: Some(Rect::new(0.0, 88.0, 640.0, 328.0)),
            views: [None, None, None, None],
            speedometer: None,
            tachometer: None,
            steering_wheel: None,
        };
        let fov = std::f32::consts::FRAC_PI_4;
        let pitch = eye_pitch(&dashboard, fov);
        // Horizon 184 pixels down, 56 above the middle of 480.
        let horizon = 240.0 - 240.0 * pitch.tan() / (fov / 2.0).tan();
        assert!((horizon - 184.0).abs() < 1e-3, "{horizon}");
        assert!(pitch > 0.0);
        dashboard.window = None;
        assert_eq!(eye_pitch(&dashboard, fov), 0.0);
    }

    #[test]
    fn a_part_is_placed_as_shares_of_the_screen() {
        let node = placed(
            Rect::new(160.0, 240.0, 480.0, 480.0),
            Vec2::new(640.0, 480.0),
        );
        assert_eq!(node.left, percent(25));
        assert_eq!(node.top, percent(50));
        assert_eq!(node.width, percent(50));
        assert_eq!(node.height, percent(50));
    }
}
