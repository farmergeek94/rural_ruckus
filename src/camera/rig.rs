//! The camera's rules, as a state machine over plain data: told each frame where its target
//! is, it says where the eye is and what it looks at.
//!
//! Two things are smoothed, each by a critically damped spring (see `spring`): the **aim**,
//! the point in the middle of the picture, which follows the target; and the **heading**,
//! the direction over the ground that the camera sits behind the aim in, which follows the
//! way the target faces. The eye is not smoothed. It is placed: so far behind the aim along
//! the heading, and so far above it. It is therefore always the same distance away, and it
//! moves as smoothly as the two things it is placed from.

use bevy::math::{Vec2, Vec3, Vec3Swizzles};

use super::spring::{follow, follow_angle};

/// What the camera is told about the thing it follows, every frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    /// In metres.
    pub position: Vec3,
    /// In m/s. Knowing it is what lets the camera keep up with no lag. Give the real
    /// velocity, from the physics, not a difference of positions: that has every wobble
    /// of the clock in it.
    pub velocity: Vec3,
    /// The way it faces. Only its direction over the ground is used, and when it has
    /// none (pointing straight up, say) the camera keeps the heading it had.
    pub facing: Vec3,
}

/// How the camera behaves. Stiffnesses are per second: a sudden gap is down to about a
/// tenth after `4 / stiffness` seconds. Higher follows more tightly and passes more of the
/// target's shaking on to the picture; lower floats.
#[derive(Clone, Debug, PartialEq)]
pub struct RigConfig {
    /// From the point looked at back to the eye, over the ground, in metres.
    pub distance: f32,
    /// The eye's height above the point looked at, in metres.
    pub height: f32,
    /// The point looked at is this far above the target's position, in metres.
    pub look_above: f32,
    /// How tightly the aim follows the target over the ground. Tight: this is what keeps
    /// the target in the middle of the picture from side to side.
    pub aim_stiffness: f32,
    /// How tightly the aim follows the target's height. Loose, so that a body bouncing on
    /// its springs moves in the picture instead of shaking the whole picture.
    pub aim_height_stiffness: f32,
    /// How long the target's vertical speed must last before the camera goes along with
    /// it, in seconds. A long climb is followed with no lag; a bounce, which is over
    /// sooner than this, is mostly left to the spring.
    pub climb_settling: f32,
    /// How soon the camera stops going along with a vertical speed that has ended, in
    /// seconds. Short, or the camera carries on down after a truck that has landed.
    pub climb_letting_go: f32,
    /// How tightly the heading follows the way the target faces. Loose, so that a flick
    /// of the steering doesn't swing the camera round on its long arm.
    pub heading_stiffness: f32,
    /// How far above the ground the eye is kept, in metres.
    pub clearance: f32,
    /// How quickly the eye rises to clear ground that has come up under it, and how
    /// slowly it comes back down afterwards.
    pub lift_stiffness: f32,
    pub settle_stiffness: f32,
    /// A target that is suddenly further than this from the point looked at, in metres,
    /// has been moved by hand, and the camera goes straight to it.
    pub snap_distance: f32,
}

impl Default for RigConfig {
    fn default() -> Self {
        Self {
            distance: 13.0,
            height: 3.5,
            look_above: 1.5,
            aim_stiffness: 10.0,
            aim_height_stiffness: 4.0,
            climb_settling: 0.5,
            climb_letting_go: 0.1,
            heading_stiffness: 2.5,
            clearance: 2.0,
            lift_stiffness: 8.0,
            settle_stiffness: 2.0,
            snap_distance: 40.0,
        }
    }
}

/// Where the camera is and the point in the middle of its picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub eye: Vec3,
    pub aim: Vec3,
}

#[derive(Clone, Debug)]
pub struct Rig {
    aim: Vec3,
    aim_velocity: Vec3,
    /// The target's vertical speed, as far as the camera goes along with it.
    climb: f32,
    /// Rotation about Y in radians. 0 sits behind a target facing -Z.
    heading: f32,
    heading_rate: f32,
    /// What the heading is making for: the last direction the target faced that was one.
    wanted_heading: f32,
    /// How far the eye is held above where it would otherwise be, to clear the ground.
    lift: f32,
    lift_rate: f32,
}

/// Rotation about Y of something facing `facing`, where 0 faces -Z. `None` when it faces
/// no way over the ground.
fn heading_of(facing: Vec3) -> Option<f32> {
    let flat = facing.xz();
    (flat.length_squared() > 1e-4).then(|| (-flat.x).atan2(-flat.y))
}

impl Rig {
    /// A camera already settled behind `target`, and going its way.
    pub fn new(config: &RigConfig, target: &Target) -> Self {
        let heading = heading_of(target.facing).unwrap_or(0.0);
        Self {
            aim: target.position + Vec3::Y * config.look_above,
            aim_velocity: target.velocity,
            climb: target.velocity.y,
            heading,
            heading_rate: 0.0,
            wanted_heading: heading,
            lift: 0.0,
            lift_rate: 0.0,
        }
    }

    /// Moves the camera `by`, with everything it has followed: for a target that has been
    /// moved as far, as in a world that repeats, and is to be seen going on as it was.
    pub fn shift(&mut self, by: Vec3) {
        self.aim += by;
    }

    /// Follows `target` for a frame of `seconds`, and says where the eye wants to be,
    /// before anything has been said about the ground. Follow it with `place`.
    pub fn want(&mut self, config: &RigConfig, target: &Target, seconds: f32) -> Vec3 {
        let wanted_aim = target.position + Vec3::Y * config.look_above;
        if self.aim.distance(wanted_aim) > config.snap_distance {
            *self = Self::new(config, target);
        }

        // Over the ground the camera goes along with the target's velocity as it is. A
        // truck's is smooth, and any lag here is the target leaving the middle of the
        // picture. Its vertical speed is taken up only as it lasts.
        let taking_up =
            target.velocity.y * self.climb >= 0.0 && target.velocity.y.abs() > self.climb.abs();
        let settling = if taking_up {
            config.climb_settling
        } else {
            config.climb_letting_go
        };
        let settled = if settling > 0.0 {
            1.0 - (-seconds / settling).exp()
        } else {
            1.0
        };
        self.climb += (target.velocity.y - self.climb) * settled;
        let going = Vec3::new(target.velocity.x, self.climb, target.velocity.z);

        for (axis, stiffness) in [
            (0, config.aim_stiffness),
            (1, config.aim_height_stiffness),
            (2, config.aim_stiffness),
        ] {
            (self.aim[axis], self.aim_velocity[axis]) = follow(
                (self.aim[axis], self.aim_velocity[axis]),
                (wanted_aim[axis], going[axis]),
                stiffness,
                seconds,
            );
        }

        if let Some(heading) = heading_of(target.facing) {
            self.wanted_heading = heading;
        }
        (self.heading, self.heading_rate) = follow_angle(
            (self.heading, self.heading_rate),
            self.wanted_heading,
            config.heading_stiffness,
            seconds,
        );

        self.unlifted_eye(config)
    }

    /// The camera's pose for the frame, given the height of the ground under where the
    /// eye wants to be, if there is any. The eye is lifted clear of it softly, so that a
    /// crease in the ground never puts a corner in the camera's path.
    pub fn place(&mut self, config: &RigConfig, ground: Option<f32>, seconds: f32) -> Pose {
        let unlifted = self.unlifted_eye(config);
        let needed = ground.map_or(0.0, |ground| {
            (ground + config.clearance - unlifted.y).max(0.0)
        });
        let stiffness = if needed > self.lift {
            config.lift_stiffness
        } else {
            config.settle_stiffness
        };
        (self.lift, self.lift_rate) = follow(
            (self.lift, self.lift_rate),
            (needed, 0.0),
            stiffness,
            seconds,
        );

        let mut eye = unlifted + Vec3::Y * self.lift;
        // The last resort, for ground that came up faster than the lift: never under it.
        if let Some(ground) = ground {
            eye.y = eye.y.max(ground + config.clearance * 0.25);
        }
        Pose { eye, aim: self.aim }
    }

    /// Both at once, for a camera with no ground to think about.
    pub fn step(&mut self, config: &RigConfig, target: &Target, seconds: f32) -> Pose {
        self.want(config, target, seconds);
        self.place(config, None, seconds)
    }

    fn unlifted_eye(&self, config: &RigConfig) -> Vec3 {
        let behind = Vec2::new(self.heading.sin(), self.heading.cos()) * config.distance;
        self.aim + Vec3::new(behind.x, config.height, behind.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(position: Vec3, velocity: Vec3) -> Target {
        Target {
            position,
            velocity,
            facing: Vec3::NEG_Z,
        }
    }

    #[test]
    fn sits_behind_and_above_a_target_facing_minus_z() {
        let config = RigConfig::default();
        let mut rig = Rig::new(&config, &target(Vec3::ZERO, Vec3::ZERO));
        let pose = rig.step(&config, &target(Vec3::ZERO, Vec3::ZERO), 1.0 / 60.0);
        assert_eq!(pose.aim, Vec3::new(0.0, 1.5, 0.0));
        assert!(
            pose.eye.distance(Vec3::new(0.0, 5.0, 13.0)) < 1e-5,
            "{pose:?}"
        );
    }

    #[test]
    fn headings_follow_the_games_convention() {
        // Yaw 0 faces -Z and a quarter turn to the left faces -X.
        assert_eq!(heading_of(Vec3::NEG_Z), Some(0.0));
        let left = heading_of(Vec3::NEG_X).unwrap();
        assert!((left - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        // Pitch is ignored, and straight up faces no way at all.
        assert_eq!(heading_of(Vec3::new(0.0, 0.7, -0.7)), Some(0.0));
        assert_eq!(heading_of(Vec3::Y), None);
    }

    #[test]
    fn a_target_moved_by_hand_is_gone_straight_to() {
        let config = RigConfig::default();
        let mut rig = Rig::new(&config, &target(Vec3::ZERO, Vec3::ZERO));
        let far = Target {
            position: Vec3::new(500.0, 20.0, -300.0),
            velocity: Vec3::ZERO,
            facing: Vec3::X,
        };
        let pose = rig.step(&config, &far, 1.0 / 60.0);
        assert_eq!(pose.aim, far.position + Vec3::Y * 1.5);
        // Behind it, which for a target facing +X is towards -X.
        assert!(pose.eye.distance(far.position + Vec3::new(-13.0, 5.0, 0.0)) < 1e-4);
    }

    #[test]
    fn a_target_moved_with_the_camera_is_followed_as_if_it_had_not_been() {
        let config = RigConfig::default();
        let (mut stayed, mut moved) = {
            let start = target(Vec3::ZERO, Vec3::new(20.0, 0.0, -10.0));
            (Rig::new(&config, &start), Rig::new(&config, &start))
        };
        let by = Vec3::new(-2500.0, 0.0, 2500.0);
        for frame in 1..=30 {
            let seconds = frame as f32 / 60.0;
            let going = target(
                Vec3::new(20.0, 0.0, -10.0) * seconds,
                Vec3::new(20.0, 3.0, -10.0),
            );
            let pose = stayed.step(&config, &going, 1.0 / 60.0);
            // Over the edge of the world half way, and moved across.
            if frame == 15 {
                moved.shift(by);
            }
            let offset = if frame >= 15 { by } else { Vec3::ZERO };
            let shifted = Target {
                position: going.position + offset,
                ..going
            };
            let moved_pose = moved.step(&config, &shifted, 1.0 / 60.0);
            assert!(moved_pose.eye.distance(pose.eye + offset) < 1e-2, "{frame}");
            assert!(moved_pose.aim.distance(pose.aim + offset) < 1e-2, "{frame}");
        }
    }

    #[test]
    fn the_eye_is_lifted_clear_of_the_ground_and_let_down_again() {
        let config = RigConfig::default();
        let still = target(Vec3::ZERO, Vec3::ZERO);
        let mut rig = Rig::new(&config, &still);

        // A bank 10 m high under the eye, which wants to be at 5 m.
        let mut last = 5.0;
        for _ in 0..120 {
            rig.want(&config, &still, 1.0 / 60.0);
            let eye = rig.place(&config, Some(10.0), 1.0 / 60.0).eye;
            assert!(
                eye.y >= last - 1e-4,
                "it only rises: {} then {}",
                last,
                eye.y
            );
            assert!(eye.y >= 10.0 + 0.5 - 1e-4, "never in the bank: {}", eye.y);
            last = eye.y;
        }
        assert!((last - 12.0).abs() < 0.01, "{last}");

        // The bank gone, it comes down, more slowly than it went up.
        rig.want(&config, &still, 0.25);
        let after = rig.place(&config, Some(0.0), 0.25).eye.y;
        assert!(after > 9.0 && after < 12.0, "{after}");
    }
}
