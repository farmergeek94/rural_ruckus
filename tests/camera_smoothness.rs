//! How smoothly the camera follows, against targets driven along scripted paths: no app, no
//! physics, no autopilot, and the same answer every time. Each figure asserted here is a
//! budget. A change to the camera that makes one worse fails, instead of being judged by eye.
//!
//! `cargo test --test camera_smoothness -- --nocapture` prints every figure.

use std::f32::consts::TAU;

use bevy::math::{Vec2, Vec3};
use monster_truck_rural_ruckus::camera::{Pose, Rig, RigConfig, Target};

/// What the driver of a scripted target is doing at a moment.
#[derive(Clone, Copy, Default)]
struct Controls {
    /// Along the way it faces, in m/s^2.
    acceleration: f32,
    /// In rad/s, positive to the left.
    turning: f32,
    /// Height above the road and its rate of change, in m and m/s: the body on its springs.
    height: f32,
    climbing: f32,
}

/// A target's whole journey, worked out once in steps of a millisecond.
struct Path {
    every_millisecond: Vec<Target>,
}

impl Path {
    fn new(speed: f32, seconds: f32, controls: impl Fn(f32) -> Controls) -> Self {
        const DT: f32 = 0.001;
        let (mut speed, mut heading, mut over_ground) = (speed, 0.0_f32, Vec2::ZERO);
        let every_millisecond = (0..=(seconds / DT) as usize + 1)
            .map(|step| {
                let now = controls(step as f32 * DT);
                // Yaw 0 faces -Z, and a turn to the left is positive.
                let facing = Vec3::new(-heading.sin(), 0.0, -heading.cos());
                let target = Target {
                    position: Vec3::new(over_ground.x, now.height, over_ground.y),
                    velocity: facing * speed + Vec3::Y * now.climbing,
                    facing,
                };
                over_ground += Vec2::new(facing.x, facing.z) * speed * DT;
                speed = (speed + now.acceleration * DT).max(0.0);
                heading += now.turning * DT;
                target
            })
            .collect();
        Self { every_millisecond }
    }

    fn at(&self, seconds: f32) -> Target {
        let place = seconds * 1000.0;
        let (from, to) = (
            self.every_millisecond[place as usize],
            self.every_millisecond[place as usize + 1],
        );
        let between = place.fract();
        Target {
            position: from.position.lerp(to.position, between),
            velocity: from.velocity.lerp(to.velocity, between),
            facing: from.facing.lerp(to.facing, between),
        }
    }
}

/// Frame lengths in seconds, filling `seconds`.
fn frames(seconds: f32, pattern: &[f32]) -> Vec<f32> {
    let mut frames = Vec::new();
    let mut total = 0.0;
    while total + pattern[frames.len() % pattern.len()] < seconds {
        let frame = pattern[frames.len() % pattern.len()];
        frames.push(frame);
        total += frame;
    }
    frames
}

const EVEN_60: [f32; 1] = [1.0 / 60.0];
/// As logged under vsync on the development machine: late frames and the short ones that
/// follow them, and the pair that the old clock took for a stall.
const UNEVEN: [f32; 8] = [0.0167, 0.023, 0.010, 0.0167, 0.030, 0.0094, 0.0167, 0.0113];

struct Frame {
    seconds: f32,
    length: f32,
    pose: Pose,
    target: Target,
}

impl Frame {
    fn forward(&self) -> Vec3 {
        (self.pose.aim - self.pose.eye).normalize()
    }

    /// Degrees between the middle of the picture and the point meant to be there.
    fn off_centre(&self, config: &RigConfig) -> f32 {
        let wanted = self.target.position + Vec3::Y * config.look_above;
        degrees_between(self.forward(), wanted - self.pose.eye)
    }
}

/// `Vec3::angle_between` goes by the dot product alone, which rounds anything under a
/// fiftieth of a degree to nothing or to a fiftieth. The cross product keeps small angles.
fn degrees_between(from: Vec3, to: Vec3) -> f32 {
    from.cross(to).length().atan2(from.dot(to)).to_degrees()
}

fn film(config: &RigConfig, path: &Path, frame_lengths: &[f32]) -> Vec<Frame> {
    let mut rig = Rig::new(config, &path.at(0.0));
    let mut seconds = 0.0;
    frame_lengths
        .iter()
        .map(|&length| {
            seconds += length;
            let target = path.at(seconds);
            let pose = rig.step(config, &target, length);
            Frame {
                seconds,
                length,
                pose,
                target,
            }
        })
        .collect()
}

fn worst(values: impl Iterator<Item = f32>) -> f32 {
    values.fold(0.0, f32::max)
}

/// The most the view's rate of turn changes from one frame to the next, in deg/s: a kink
/// in the camera's turning is a large one. Rates, not angles, so that frames of different
/// lengths can be compared.
fn worst_change_in_turn_rate(film: &[Frame]) -> f32 {
    let rates: Vec<f32> = film
        .windows(2)
        .map(|pair| degrees_between(pair[0].forward(), pair[1].forward()) / pair[1].length)
        .collect();
    worst(rates.windows(2).map(|pair| (pair[1] - pair[0]).abs()))
}

/// The most the eye's velocity changes from one frame to the next, in m/s.
fn worst_change_in_eye_velocity(film: &[Frame]) -> f32 {
    let velocities: Vec<Vec3> = film
        .windows(2)
        .map(|pair| (pair[1].pose.eye - pair[0].pose.eye) / pair[1].length)
        .collect();
    worst(velocities.windows(2).map(|pair| pair[1].distance(pair[0])))
}

/// When the target was last further than `degrees` from the middle of the picture.
fn last_off_centre(film: &[Frame], config: &RigConfig, degrees: f32) -> f32 {
    film.iter()
        .filter(|frame| frame.off_centre(config) > degrees)
        .map(|frame| frame.seconds)
        .fold(0.0, f32::max)
}

fn steady(_: f32) -> Controls {
    Controls::default()
}

#[test]
fn a_target_going_steadily_is_dead_centre_at_any_speed_and_any_frame_rate() {
    let config = RigConfig::default();
    for speed in [5.0, 18.0, 30.0] {
        let path = Path::new(speed, 10.0, steady);
        for pattern in [&EVEN_60[..], &[1.0 / 144.0], &[1.0 / 30.0], &UNEVEN] {
            let film = film(&config, &path, &frames(10.0, pattern));
            let off = worst(film.iter().map(|frame| frame.off_centre(&config)));
            assert!(off < 0.01, "{off} deg off centre at {speed} m/s");
            // The camera goes at the target's speed, whatever the frames are doing.
            let bump = worst_change_in_eye_velocity(&film);
            assert!(
                bump < 0.01,
                "eye velocity changed by {bump} m/s at {speed} m/s"
            );
        }
    }
}

#[test]
fn the_eye_keeps_its_distance() {
    let config = RigConfig::default();
    let path = Path::new(5.0, 10.0, |seconds| Controls {
        acceleration: if seconds < 4.0 { 5.0 } else { -10.0 },
        turning: 0.5,
        ..default_controls()
    });
    let arm = config.distance.hypot(config.height);
    for frame in film(&config, &path, &frames(10.0, &UNEVEN)) {
        let distance = frame.pose.eye.distance(frame.pose.aim);
        assert!((distance - arm).abs() < 1e-3, "{distance} m, not {arm}");
    }
}

fn default_controls() -> Controls {
    Controls::default()
}

#[test]
fn speeding_up_and_braking_hard_barely_move_the_target_in_the_picture() {
    let config = RigConfig::default();
    // 5 m/s^2 from 5 m/s for four seconds, then 10 m/s^2 of braking for two.
    let path = Path::new(5.0, 10.0, |seconds| Controls {
        acceleration: match seconds {
            s if s < 1.0 => 0.0,
            s if s < 5.0 => 5.0,
            s if s < 7.0 => -10.0,
            _ => 0.0,
        },
        ..default_controls()
    });
    for pattern in [&EVEN_60[..], &UNEVEN] {
        let film = film(&config, &path, &frames(10.0, pattern));
        let off = worst(film.iter().map(|frame| frame.off_centre(&config)));
        let kink = worst_change_in_eye_velocity(&film);
        println!("speed: {off:.3} deg off centre at worst, eye velocity changes {kink:.3} m/s");
        assert!(off < 0.15, "{off}");
        // Braking at 10 m/s^2 is 0.17 m/s a frame, and 0.3 over a frame of 30 ms. The
        // camera's speed changes no faster than the target's does.
        assert!(kink < 0.35, "{kink}");
    }
}

#[test]
fn a_bouncing_body_moves_in_the_picture_not_the_picture() {
    let config = RigConfig::default();
    // The body set bouncing on its springs at 1.5 Hz, 0.15 m, dying away, at 18 m/s, by
    // a bump a second in.
    let bounce = |seconds: f32| {
        let since = (seconds - 1.0).max(0.0);
        let (decay, phase) = ((-1.5 * since).exp(), TAU * 1.5 * since);
        if since == 0.0 {
            return Controls::default();
        }
        Controls {
            height: 0.15 * decay * phase.sin(),
            climbing: 0.15 * decay * (TAU * 1.5 * phase.cos() - 1.5 * phase.sin()),
            ..default_controls()
        }
    };
    let path = Path::new(18.0, 6.0, bounce);
    let film = film(&config, &path, &frames(6.0, &EVEN_60));

    // The eye is placed from the aim, so the picture doesn't pitch: it rides up and down.
    // How far, against the body's own travel, which a camera fastened to it would share?
    let rode = worst(
        film.iter()
            .map(|frame| (frame.pose.aim.y - config.look_above).abs()),
    );
    let body = worst(film.iter().map(|frame| frame.target.position.y.abs()));
    let kink = worst_change_in_eye_velocity(&film);
    let off = worst(film.iter().map(|frame| frame.off_centre(&config)));
    println!(
        "bounce: the camera rides {rode:.3} m of the body's {body:.3} m, its velocity changes \
         {kink:.3} m/s a frame, the body is {off:.3} deg off centre at worst"
    );
    assert!(rode < 0.4 * body, "{rode} of {body}");
    // The bump itself sets the body going at 1.4 m/s from one millisecond to the next.
    assert!(kink < 0.05, "{kink}");
    // And the body is never far from the middle for it.
    assert!(off < 0.6, "{off}");
}

#[test]
fn a_tap_of_the_steering_does_not_swing_the_camera() {
    let config = RigConfig::default();
    // 40 deg/s of turn for a quarter of a second: ten degrees, as a key tap gives.
    let path = Path::new(18.0, 6.0, |seconds| Controls {
        turning: if (1.0..1.25).contains(&seconds) {
            40.0_f32.to_radians()
        } else {
            0.0
        },
        ..default_controls()
    });
    for pattern in [&EVEN_60[..], &UNEVEN] {
        let film = film(&config, &path, &frames(6.0, pattern));
        let off = worst(film.iter().map(|frame| frame.off_centre(&config)));
        let kink = worst_change_in_turn_rate(&film);
        let settled = last_off_centre(&film, &config, 0.05) - 1.25;
        println!(
            "tap: {off:.3} deg off centre at worst, back within 0.05 deg {settled:.2} s after \
             it, turn rate changes {kink:.3} deg/s"
        );
        assert!(off < 0.4, "{off}");
        // The truck turned at 40 deg/s from one millisecond to the next. The view's rate
        // of turn never changes by more than this from one frame to the next.
        assert!(kink < 2.0, "{kink}");
    }
}

#[test]
fn a_long_turn_is_followed_round() {
    let config = RigConfig::default();
    let path = Path::new(15.0, 12.0, |seconds| Controls {
        turning: if seconds > 1.0 {
            30.0_f32.to_radians()
        } else {
            0.0
        },
        ..default_controls()
    });
    let film = film(&config, &path, &frames(12.0, &EVEN_60));
    let off = worst(film.iter().map(|frame| frame.off_centre(&config)));
    let last = &film[film.len() - 1];
    // Once round the bend is steady, the camera is a fixed angle behind the truck's tail.
    let behind = (last.pose.aim - last.pose.eye).with_y(0.0);
    let trailing = behind.angle_between(last.target.facing).to_degrees();
    println!(
        "turn: {off:.3} deg off centre at worst, the camera trails the tail by {trailing:.1} deg"
    );
    assert!(off < 0.5, "{off}");
    // Twice the rate of turn over the heading's stiffness: the price of a camera that a
    // tap of the steering doesn't swing. The truck is in the middle of the picture all the same.
    assert!(trailing < 25.0, "{trailing}");
}

#[test]
fn a_drop_off_a_ledge_is_followed_down_without_overshoot() {
    let config = RigConfig::default();
    // Level, then 2 m of free fall from the first second, then level again.
    let fall = (2.0_f32 * 2.0 / 9.81).sqrt();
    let path = Path::new(18.0, 8.0, move |seconds| {
        let falling = (seconds - 1.0).clamp(0.0, fall);
        Controls {
            height: -0.5 * 9.81 * falling * falling,
            climbing: if falling > 0.0 && falling < fall {
                -9.81 * falling
            } else {
                0.0
            },
            ..default_controls()
        }
    });
    let film = film(&config, &path, &frames(8.0, &EVEN_60));
    let lowest_aim = film
        .iter()
        .map(|frame| frame.pose.aim.y)
        .fold(f32::MAX, f32::min);
    let off = worst(film.iter().map(|frame| frame.off_centre(&config)));
    let settled = last_off_centre(&film, &config, 1.0) - 1.0;
    println!(
        "drop: {off:.2} deg off centre at worst, back within 1 deg {settled:.2} s after the \
         edge, aim went {:.3} m past",
        (config.look_above - 2.0) - lowest_aim
    );
    assert!(lowest_aim > config.look_above - 2.0 - 0.05, "{lowest_aim}");
    // It falls faster than the camera follows, which is the point: the truck drops in the
    // picture, and the picture comes down after it.
    assert!(off < 6.0, "{off}");
    assert!(settled < 1.5, "{settled}");
}

#[test]
fn frame_rate_makes_no_difference_to_where_the_camera_ends_up() {
    let config = RigConfig::default();
    let path = Path::new(10.0, 5.0, |seconds| Controls {
        acceleration: 3.0,
        turning: 0.4 * (seconds * 2.0).sin(),
        ..default_controls()
    });
    let end = |pattern: &[f32]| {
        let lengths = frames(4.0, pattern);
        let mut rig = Rig::new(&config, &path.at(0.0));
        let mut seconds = 0.0;
        for length in lengths {
            seconds += length;
            rig.step(&config, &path.at(seconds), length);
        }
        // To the same moment, whatever the last frame of the pattern left over.
        rig.step(&config, &path.at(4.0), 4.0 - seconds)
    };
    let reference = end(&[1.0 / 240.0]);
    for pattern in [&EVEN_60[..], &[1.0 / 30.0], &[1.0 / 144.0], &UNEVEN] {
        let pose = end(pattern);
        let apart = pose.eye.distance(reference.eye);
        println!("frame rate: {apart:.4} m from where 240 Hz ends up");
        // A frame is solved as if the target held its velocity through it, which one that
        // is speeding up and turning doesn't quite.
        assert!(apart < 0.05, "{apart}");
    }
}
