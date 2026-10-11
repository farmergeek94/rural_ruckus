//! Which of a wheel collider's contacts the physics solves, and how the tread drives the
//! ones it does.
//!
//! A wheel collider is fastened to the chassis, so what it touches, it touches rigidly:
//! right for a wall, a rail, another truck or the side of a parked car, which the tire
//! should stop against. But a rigid contact under the tread would hold the hub where it is
//! and let nothing compress. The sweep in `drive` already finds whatever is under the
//! tread and lets the suspension stand the tire on it, so a contact that pushes the wheel
//! up on its tread is dropped here and left to the sweep. `decide_tire_contacts` is shown
//! every pair of colliders one of which carries `ActiveCollisionHooks::MODIFY_CONTACTS`,
//! which the wheel colliders and their cores do (see `spawn`), with all their contact
//! manifolds, and each manifold is decided on its own: against the ground a tire has one
//! for each triangle it touches.
//!
//! What the suspension takes is what the sweep stands the tire on, by the sweep's own
//! rules, so that a touch is the suspension's or the collider's and never both or neither
//! -- unless the springs are shut (`WheelCollider::bottomed`): then the tire has squashed
//! as far as it goes and is simply solid, so the contact is kept, and it is what holds the
//! truck up. The terrain's push is the suspension's wherever it holds the tire up as the
//! truck stands (`drive::UPWARD`), since the terrain has no edges. Scenery's and a ground
//! box's only where it also touches the tire from under (`drive::UNDER`) and the sweep
//! took it this step (`WheelCollider::stands_on`): the top of a kerb under the tread is
//! the suspension's, and the same kerb's edge against the front of the tire, or an edge
//! the sweep refused to lift the tire onto (`drive::EDGE_RAMP`), is the collider's, which
//! climbs it. Left to neither, a wall's edge against a tire whose wheel hung at full droop
//! let the tire sink into the wall until its frictionless core held it, and the truck
//! crept through the wall at 0.3 m/s. A tire landing on a top from the air is the sweep's
//! before it is the collider's: the sweep reaches the whole travel below a hanging tire,
//! which is further than a step takes it. Another truck's, and light loose
//! scenery's (`drive::LIGHT_LOOSE`), never: the sweep does not look at either (see
//! `drive`), so every contact a tire has with them is kept.
//!
//! The ground has no underside, so it never pushes a wheel or its core down
//! (`ground_holds_down`): the terrain is a field of heights, and a push down is from under
//! one of its triangles, met by a tire sunk into a slope. Everything else a wheel collider
//! touches, it touches as it is: the upright side of a ground box, which is a wall although
//! the box is ground; the face of a kerb or a step; a rail; another truck, tread to tread
//! or tread to body; the tops of its own tires, which a truck tipped over lies on. The
//! terrain too, where it does not hold the tire up as the truck stands (`drive::UPWARD`):
//! a bank met by the side of the tire, or by the tread with the truck on its side.
//!
//! **Every kept contact is driven by the tread** (`drive_contact`, below). The tire grips
//! what it touches with twice its own grip (`CLIMB_GRIP` of `TruckConfig::grip`, which
//! `spawn` gives the collider as its friction), and another tire with twice that again
//! (`TIRE_ON_TIRE`). Tread on tread, only the tire that goes into the other the faster
//! pushes, and so climbs (`rides_up`); the other holds and rolls. Across the tread it does not slide: that is the tire's sideways grip.
//! Along the tread it rolls freely, as a wheel on an axle does, and is pushed round by
//! what the driver asks of it (`WheelCollider`): the engine's force through the gear, or
//! the brakes', which are the throttle against the way the tread rolls, or `Held` or the
//! handbrake. The sidewall neither rolls nor drives (`drive::on_the_tread`): a tire lying on
//! its side holds like a block, where rolling freely it let a truck on its side creep
//! along on its wheels like castors, and spin them under throttle to 18 m/s. So a tire
//! pressed against a wall climbs it under throttle, by the tread's
//! grip on the wall, and rolls back down it off the throttle; a tire on top of another
//! truck drives on it as on the ground; and a truck on its roof, with its tires' tops on
//! the ground, drives on them -- backwards, as it did in Monster Truck Madness 2, since
//! the tops of the tires go the other way. The push is told to the solver as how much
//! faster the tread goes than the surface, which is how far the force takes a quarter of
//! the truck in one step: friction carries a tire to whatever speed its tread is said to
//! go within one step, if the push on the face allows, so a tread told to go at a climbing
//! speed outright threw the truck up at all of it at once. The engine's push fades out as
//! the tread nears the truck's top speed, since the engine only knows the speed of the
//! chassis and not of a tread that drives the truck backwards: without that, a truck on
//! its roof under full throttle reached 76 m/s in 3 s.
//! Climbing, the push stops once the truck rises faster than `CLIMB_SPEED`, so that a
//! truck walks up a wall at a crawl and does not run up it, and fades as its nose comes up
//! (`CLIMB_PITCH_FULL`), so that it does not walk up a wall taller than it can get over.
//! Measured headless with the built-in truck driven at ground-box walls of 0.5 to 3 m at
//! full throttle, from a crawl and at 4, 8 and 15 m/s, head on and at 30 and 50 degrees:
//! it rolls over a 0.5 m kerb at any speed as it did; a wall of 1 m or more stops it, and
//! it then climbs: walls of 1 to 1.5 m in 8 or 9 runs of 12 each, 2 m walls in 3 (head on,
//! up to 8 m/s), and no 3 m wall; no run tips the truck over. Before, the
//! suspension rolled it over every wall up to 1.5 m, throwing it up to 5.9 m into the
//! air, and 2 m walls bounced it back at 2 m/s.
//!
//! An edge of scenery or of a ground box that the front of the tire meets -- the top of a
//! wall between seven tenths of the tire's radius high and a little over the hub -- pushes
//! the tire back and up. Met rigidly, that up threw the truck over the wall: measured, the
//! built-in truck at 18 m/s into a 1 m wall rose 2.1 m and came down 13 m/s harder. Its
//! push is turned level here (`meet_edge`), so that the edge stops the truck as a wall
//! does, and the tread then climbs it; once the tire is far enough over the edge the
//! suspension takes it (`drive::UNDER`).
//!
//! Nothing bounces: tires and bodies have no restitution (see `spawn`), so a truck that
//! hits a wall or another truck stops, and what rocks it back is its own suspension. The
//! physics still gives a body back about a twentieth of the speed it meets a wall at:
//! measured, a cube with no bounce and no friction came off a wall at 1.0 m/s after
//! hitting it at 20, and the built-in truck at 1.9 m/s.
//!
//! A tire with its springs shut pushes straight up off the ground (`stand_it_up`), not
//! back along it: where the ground rises ahead of such a tire, a solid tire pushed back
//! along the slope was stopped as a wheel is by a kerb.
//!
//! `physics::GamePhysicsPlugin` runs `decide_tire_contacts`. An app that sets up the
//! physics without it keeps every contact as the physics makes it, and its trucks stand
//! on their wheel colliders instead of their springs.

use avian3d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::tasks::{ComputeTaskPool, TaskPool};

use super::drive::{AXLE_ALONG_X, LIGHT_LOOSE, UNDER, UPWARD, carries, on_the_tread};
use super::{Truck, TruckConfig, WheelCollider, WheelCore};
use crate::collision_groups::{GROUND, GROUND_BOXES};

#[derive(SystemParam)]
pub struct TireContacts<'w, 's> {
    /// Each wheel collider: where `drive` put it on its truck, its truck, and what `drive`
    /// says of it this step.
    wheels: Query<'w, 's, (&'static Transform, &'static ChildOf, &'static WheelCollider)>,
    /// Where each truck is and how it moves, as the physics has it in this step.
    trucks: Query<
        'w,
        's,
        (
            &'static Position,
            &'static Rotation,
            &'static LinearVelocity,
            &'static AngularVelocity,
            &'static ComputedCenterOfMass,
        ),
        With<Truck>,
    >,
    layers: Query<'w, 's, &'static CollisionLayers>,
    /// A core is its wheel collider's child, and so the truck's through the physics.
    cores: Query<'w, 's, &'static ColliderOf, With<WheelCore>>,
    /// The body each collider is part of, and what kind of body that is.
    bodies: Query<'w, 's, &'static ColliderOf>,
    kinds: Query<'w, 's, (&'static RigidBody, &'static ComputedMass)>,
    /// Each truck's tuning: its mass and its tires' size and grip.
    configs: Query<'w, 's, &'static TruckConfig, With<Truck>>,
    /// The physics' clock, for how long a step is.
    time: Res<'w, Time>,
}

/// How much more a tread grips what it climbs -- a wall, an edge, another truck -- than
/// the ground, as a multiple of `TruckConfig::grip`. The user asked for twice.
const CLIMB_GRIP: f32 = 2.0;
/// And how much more again tread on tread grips, as two tires meet: twice, the user
/// asked, as there are two tires touching.
const TIRE_ON_TIRE: f32 = 2.0;

/// Tread on tread, the tire that goes into the other the faster is the one that climbs
/// (`rides_up`), and the other only holds and rolls. This is how much faster, in m/s, the
/// second collider's tire must go into the first's to be the one: under it the first
/// rides up, so that two tires evenly matched do not take turns.
const RIDER_MARGIN: f32 = 0.5;

/// Whether the first of two tires that meet is the one that climbs the other, from how
/// fast each goes into the other, in m/s (see `RIDER_MARGIN`): the user wants whichever
/// tire moves into the collision the faster to be the one that climbs.
fn rides_up(first_into: f32, second_into: f32) -> bool {
    second_into <= first_into + RIDER_MARGIN
}

/// The friction of a contact between treads of these grips: the greatest of them, raised
/// for climbing (`CLIMB_GRIP`), and again where two treads meet (`TIRE_ON_TIRE`).
fn climb_friction(grips: impl ExactSizeIterator<Item = f32>) -> f32 {
    let both = if grips.len() > 1 { TIRE_ON_TIRE } else { 1.0 };
    CLIMB_GRIP * both * grips.fold(0.0, f32::max)
}

/// How fast a truck may be rising, in m/s, for the throttle to go on carrying a tire up a
/// face it climbs: above it the tread only holds. Higher runs up walls faster. A first
/// guess, which wants driving.
const CLIMB_SPEED: f32 = 1.5;

/// How far up the truck's forward may point for the throttle to carry a tire up a face at
/// full strength, and where that has faded to nothing, in radians. Between them it fades
/// evenly. A truck whose nose is up this far is climbing something taller than it can get
/// over, and its tires hold on it rather than walk it further up. Higher lets a truck go
/// up taller walls, and go over on its back from one sooner. Measured: without the fade
/// the built-in truck crawled up a 3 m wall in 5 s.
const CLIMB_PITCH_FULL: f32 = 35.0 * std::f32::consts::PI / 180.0;
const CLIMB_PITCH_NONE: f32 = 55.0 * std::f32::consts::PI / 180.0;

/// How much of straight up the push of an edge may have for it to be turned level (see
/// `meet_edge`), as the cosine of its lean from upright: 0.87 is 30 degrees. A push more
/// upright than that is the top of something, which holds the tire up as it is.
const EDGE_UPRIGHT: f32 = 0.87;

/// A tire's tread where a contact touches it: what `drive_contact` needs of it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Tread {
    /// Which way the tread goes round at the touch when the wheel rolls forwards, in the
    /// world, flattened onto the contact's plane: under the tire it points backwards, and
    /// at the front of the tire down.
    along: Vec3,
    /// Across the tread, in the contact's plane.
    across: Vec3,
    /// How much faster the driver asks the tread to go this step than it rolls over the
    /// surface now, in m/s: forwards positive.
    push: f32,
    /// How much of the touch is on the tread rather than the sidewall, from 0 to 1: how
    /// much it rolls and drives.
    rolls: f32,
    /// The tire's grip.
    grip: f32,
}

/// The speed along the tread, in m/s, under which throttle against the way it rolls is
/// taken for driving the other way, and not for the brakes: as `drive` has it.
const ROLLING: f32 = 0.5;

/// How much faster the driver asks a tread to go this step than it rolls over what it
/// touches at `rolling` m/s (forwards positive), in m/s, from the `throttle`, the force the
/// engine gives the wheel (`engine`, newtons, forwards positive), whether the brakes are
/// held on (`brakes_on`), and the truck's tuning: the brakes' force, its top speed and its
/// mass, of which a tire has a quarter. Over `dt` seconds. The brakes slow the tread
/// towards rolling with the surface and no further, and so does the engine off the
/// throttle, whichever way the tread rolls: the engine's braking is worked out for the
/// chassis going forwards, and pushed a truck lying on its side along at 0.2 m/s. On the
/// throttle the engine's push fades to nothing at the top speed (see the module's notes).
fn asked_of_the_tread(
    rolling: f32,
    throttle: f32,
    engine: f32,
    brakes_on: bool,
    config: &TruckConfig,
    dt: f32,
) -> f32 {
    let per_force = dt / (config.mass / 4.0);
    let braking = throttle * rolling < 0.0 && rolling.abs() > ROLLING;
    if brakes_on || braking {
        let strength = if brakes_on { 1.0 } else { throttle.abs() };
        let most = strength * config.brake_force * per_force;
        -rolling.clamp(-most, most)
    } else if throttle == 0.0 {
        let most = engine.abs() * per_force;
        -rolling.clamp(-most, most)
    } else {
        engine * per_force * (1.0 - rolling.abs() / config.top_speed).clamp(0.0, 1.0)
    }
}

impl TireContacts<'_, '_> {
    /// Whether a wheel collider that is `pushed` this way, in the world, by `other`, at
    /// these points in its own frame (X along the axle, Y up, -Z forwards), would rather be
    /// carried by its suspension (see the module's notes).
    fn suspension_takes(&self, wheel: Entity, other: Entity, pushed: Vec3, at: &[Vec3]) -> bool {
        let Ok((_, child_of, state)) = self.wheels.get(wheel) else {
            return false;
        };
        let (Ok((_, rotation, ..)), Ok(config)) =
            (self.trucks.get(child_of.0), self.configs.get(child_of.0))
        else {
            return false;
        };
        // Solid once the springs are shut: what holds the truck up.
        if state.bottomed || self.is_truck_part(other) || self.is_loose(other) {
            return false;
        }
        let up = rotation.0 * Vec3::Y;
        if self.is_terrain(other) {
            // Wherever it holds the tire up. The top of a tire on a truck tipped over, and
            // a bank beside it, the collider meets.
            return pushed.dot(up) > UPWARD;
        }
        // Scenery and the ground boxes: as the sweep stands the tire on them, from under,
        // and only where it does.
        let under = at
            .iter()
            .any(|point| -point.y >= UNDER * config.wheel_radius);
        carries(pushed, up, false) && under && state.stands_on == Some(other)
    }

    /// Whether this collider is part of light loose scenery: a body that can be knocked
    /// about, lighter than `LIGHT_LOOSE`, and not a truck.
    fn is_loose(&self, collider: Entity) -> bool {
        let body = self.bodies.get(collider).map_or(collider, |of| of.body);
        !self.trucks.contains(body)
            && self
                .kinds
                .get(body)
                .is_ok_and(|(kind, mass)| kind.is_dynamic() && mass.value() < LIGHT_LOOSE)
    }

    /// Whether this collider is part of a truck: its body, a wheel collider or a core.
    fn is_truck_part(&self, collider: Entity) -> bool {
        self.trucks.contains(collider)
            || self.wheels.contains(collider)
            || self.cores.contains(collider)
    }

    /// The truck a wheel's collider or core is part of.
    fn truck_of(&self, wheel: Entity) -> Option<Entity> {
        self.wheels
            .get(wheel)
            .map(|(_, child_of, _)| child_of.0)
            .or_else(|_| self.cores.get(wheel).map(|of| of.body))
            .ok()
    }

    /// Whether this collider is a wheel whose springs are shut, and so solid.
    fn bottomed(&self, collider: Entity) -> bool {
        self.wheels
            .get(collider)
            .is_ok_and(|(_, _, wheel)| wheel.bottomed)
    }

    /// Whether this collider is the ground: the terrain or the ground boxes.
    fn is_ground(&self, collider: Entity) -> bool {
        self.layers
            .get(collider)
            .is_ok_and(|layers| layers.memberships.has_all(GROUND))
    }

    /// Whether this collider is the terrain: the ground, and not the ground boxes.
    fn is_terrain(&self, collider: Entity) -> bool {
        self.layers.get(collider).is_ok_and(|layers| {
            layers.memberships.has_all(GROUND) && !layers.memberships.has_all(GROUND_BOXES)
        })
    }

    /// Whether this collider is the ground boxes, and pushes a tire this way, in the world,
    /// with an upright side: a wall, where the rest of the ground is a floor or a slope.
    fn is_box_wall(&self, collider: Entity, pushed: Vec3) -> bool {
        pushed.y.abs() <= UPWARD
            && self
                .layers
                .get(collider)
                .is_ok_and(|layers| layers.memberships.has_all(GROUND_BOXES))
    }

    /// Whether `ground` is the terrain and `part` a wheel collider or core that it pushes
    /// this way, in the world, downward: from under one of its triangles, met by a tire
    /// sunk into a slope, which is no underside at all. Kept, and turned straight down for
    /// a tire whose springs are shut (`stand_it_up`), it stopped a tire that rose out of
    /// the ground at 7 m/s in one step, a jolt of 38 g. Measured over rough ground at 15
    /// and 25 m/s, the hardest jolt of a run was 57 g on average, and is 32 g. Not the
    /// ground boxes, whose upright sides are walls.
    fn ground_holds_down(&self, ground: Entity, part: Entity, pushed: Vec3) -> bool {
        (self.wheels.contains(part) || self.cores.contains(part))
            && self.is_terrain(ground)
            && pushed.y < 0.0
    }

    /// Where a contact touches a wheel collider, in the wheel's own frame (X along the
    /// axle, Y up, -Z forwards), from `touches`: each point on the wheel's surface, from
    /// its truck's centre of mass, in the world. Nothing for any other collider.
    fn on_the_wheel(&self, collider: Entity, touches: impl Iterator<Item = Vec3>) -> Vec<Vec3> {
        let Ok((wheel_at, child_of, _)) = self.wheels.get(collider) else {
            return Vec::new();
        };
        let Ok((_, rotation, _, _, centre)) = self.trucks.get(child_of.0) else {
            return Vec::new();
        };
        // `drive` keeps the collider turned by the steering and onto its axle (the
        // cylinder's axis is Y); the wheel's own frame has the steering alone.
        let steered = wheel_at.rotation * AXLE_ALONG_X.inverse();
        touches
            .map(|touch| {
                let in_the_truck = centre.0 + rotation.0.inverse() * touch;
                steered.inverse() * (in_the_truck - wheel_at.translation)
            })
            .collect()
    }

    /// How fast the body `collider` is part of goes at `point`, in the world, if it is a
    /// truck. The ground and scenery stand still, or near enough.
    fn moving_at(&self, collider: Entity, point: Vec3) -> Option<Vec3> {
        let truck = if self.trucks.contains(collider) {
            collider
        } else {
            self.truck_of(collider)?
        };
        let (position, rotation, linear, angular, centre) = self.trucks.get(truck).ok()?;
        let centre = position.0 + rotation.0 * centre.0;
        Some(linear.0 + angular.0.cross(point - centre))
    }

    /// The tread of wheel collider `wheel` where a contact whose normal is `normal` touches
    /// it at `point`, in the world, halfway between the two surfaces `depth` metres apart,
    /// and the two colliders' relative velocity there is `relative`, the first's less the
    /// second's; `is_first` says whether the wheel is the first. Nothing for a collider
    /// that is not a wheel, or a touch on the axle itself.
    #[allow(clippy::too_many_arguments)]
    fn tread(
        &self,
        wheel: Entity,
        point: Vec3,
        normal: Vec3,
        depth: f32,
        relative: Vec3,
        is_first: bool,
    ) -> Option<Tread> {
        let (wheel_at, child_of, state) = self.wheels.get(wheel).ok()?;
        let (position, rotation, ..) = self.trucks.get(child_of.0).ok()?;
        let config = self.configs.get(child_of.0).ok()?;
        // `drive` keeps the collider turned by the steering and onto its axle; the wheel's
        // own frame has the steering alone, with its axle along X.
        let steered = rotation.0 * wheel_at.rotation * AXLE_ALONG_X.inverse();
        let axle = steered * Vec3::X;
        let hub = position.0 + rotation.0 * wheel_at.translation;
        // Where the touch is on the tire's own surface: the point is given halfway between
        // the two surfaces (see `keep`).
        let sign = if is_first { 1.0 } else { -1.0 };
        let on_the_tire = point + sign * normal * depth / 2.0 - hub;
        let (along, across) = tread_frame(axle, on_the_tire, normal)?;
        // On the tread, or on the sidewall, as `drive` has it.
        let rolls = on_the_tread(normal, axle);
        // How fast the tread rolls over the surface now, forwards positive: the speed it
        // must go round at for nothing to slide.
        let rolling = -sign * relative.dot(along);
        let mut push = rolls
            * asked_of_the_tread(
            rolling,
            state.throttle,
            state.engine_force,
            state.brakes_on,
            config,
            self.time.delta_secs(),
        );
        // Climbing, only until the truck rises at `CLIMB_SPEED`, and less the further its
        // nose is up (`CLIMB_PITCH_FULL`).
        if -along.y * push > 0.0 {
            let rising = self.moving_at(wheel, point).map_or(0.0, |moving| moving.y);
            let pitch = (rotation.0 * Vec3::NEG_Z).y.clamp(-1.0, 1.0).asin() * push.signum();
            let fade = ((CLIMB_PITCH_NONE - pitch) / (CLIMB_PITCH_NONE - CLIMB_PITCH_FULL))
                .clamp(0.0, 1.0);
            push = if rising > CLIMB_SPEED { 0.0 } else { push * fade };
        }
        Some(Tread {
            along,
            across,
            push,
            rolls,
            grip: config.grip,
        })
    }

    /// Drive a kept manifold between `first` and `second` by the tread of whichever of them
    /// is a wheel collider, or both (see the module's notes).
    fn drive_contact(&self, first: Entity, second: Entity, manifold: &mut ContactManifold) {
        let Some(point) = mean_point(manifold) else {
            return;
        };
        let normal = manifold.normal;
        let depth = manifold.points.iter().map(|p| p.penetration).sum::<f32>()
            / manifold.points.len() as f32;
        // The first's speed less the second's, at the touch.
        let relative = self.moving_at(first, point).unwrap_or(Vec3::ZERO)
            - self.moving_at(second, point).unwrap_or(Vec3::ZERO);
        let mut treads = [(first, true), (second, false)]
            .into_iter()
            .filter_map(|(collider, is_first)| {
                Some((self.tread(collider, point, normal, depth, relative, is_first)?, is_first))
            })
            .collect::<Vec<_>>();
        if treads.is_empty() {
            return;
        }
        // Tread on tread, only the tire going into the other the faster pushes: the
        // normal points out of the first, so the first goes into the second along it.
        if let [(rider, _), (other, _)] = treads.as_mut_slice() {
            let first_into = self.moving_at(first, point).map_or(0.0, |v| v.dot(normal));
            let second_into = self.moving_at(second, point).map_or(0.0, |v| -v.dot(normal));
            if rides_up(first_into, second_into) {
                other.push = 0.0;
            } else {
                rider.push = 0.0;
            }
        }
        manifold.tangent_velocity = target_sliding(relative, normal, treads.iter().copied());
        manifold.friction = climb_friction(treads.iter().map(|(tread, _)| tread.grip));
    }

    /// Turn the push of an edge of scenery or of a ground box on a wheel collider level
    /// (see the module's notes), and say whether it was. `pushed` is the way it pushes the
    /// wheel, in the world, and `wheel_first` whether the wheel is the manifold's first
    /// collider. Not the terrain, which has no edges, nor another truck or light loose
    /// scenery, which a tire rides up as it finds them.
    fn meet_edge(
        &self,
        wheel: Entity,
        other: Entity,
        pushed: Vec3,
        wheel_first: bool,
        manifold: &mut ContactManifold,
    ) -> bool {
        if !self.wheels.contains(wheel)
            || self.is_terrain(other)
            || self.is_truck_part(other)
            || self.is_loose(other)
            || !(UPWARD..EDGE_UPRIGHT).contains(&pushed.y)
        {
            return false;
        }
        let Some(level) = pushed.reject_from_normalized(Vec3::Y).try_normalize() else {
            return false;
        };
        manifold.normal = if wheel_first { -level } else { level };
        true
    }

    /// Turn a contact between the ground and a tire whose springs are shut to push
    /// straight up.
    ///
    /// Such a tire is solid (see `suspension_takes`), and where the ground rises ahead of
    /// it, a solid tire is stopped as a wheel is by a kerb: the ground pushes back along
    /// its slope, and takes the truck's speed away. Measured at speed on Alpine, 211 of 212
    /// sudden losses of forward speed with no other truck near had a tire with its springs
    /// shut, on slopes of 2 to 24 degrees, and the worst took 9.7 m/s off a truck doing 34.
    /// Pushed straight up, the tire still stands on the ground; with the bump stop as well,
    /// the speed these stops took away went from 563 m/s in all to 179. Pushed straight up
    /// without the bump stop, it went up: the stop is what keeps the springs from shutting
    /// in the first place.
    ///
    /// The bounce goes by how fast the two meet along the normal, which the physics worked
    /// out along the old one, so that is worked out again.
    fn stand_it_up(&self, wheel: Entity, wheel_first: bool, manifold: &mut ContactManifold) {
        manifold.normal = Vec3::Y * manifold.normal.y.signum();
        let normal = manifold.normal;
        for point in &mut manifold.points {
            let Some(moving) = self.moving_at(wheel, point.point) else {
                continue;
            };
            // The second collider's speed less the first's; the ground stands still.
            let meeting = if wheel_first { -moving } else { moving };
            point.normal_speed = meeting.dot(normal);
        }
    }
}

/// Where a wheel's tread goes at a touch `from_hub` out from its hub, in the world, on a
/// wheel whose `axle` points to its right: `along`, which way the tread goes round there
/// when the wheel rolls forwards, flattened onto the plane of a contact whose normal is
/// `normal`, and `across` the tread in that plane. Nothing for a touch on the axle itself,
/// or one whose tread goes straight along the normal.
fn tread_frame(axle: Vec3, from_hub: Vec3, normal: Vec3) -> Option<(Vec3, Vec3)> {
    // Rolling forwards, the wheel turns so that the top of the tire goes forwards and the
    // bottom back: the tread at any point goes along the axle crossed into the point,
    // the other way.
    let round = -axle.cross(from_hub);
    let along = round.reject_from_normalized(normal).try_normalize()?;
    Some((along, normal.cross(along)))
}

/// What the solver is to leave of the velocity of the first collider less the second's at
/// a contact whose normal is `normal`, from what it is now, `relative`: nothing across any
/// tire's tread, nothing along it either as far as the touch is on the sidewall, and each
/// tread's `push` more along it, the way its friction pushes the truck. Each tread comes
/// with whether it is the first collider's. The physics lets the first collider's surface
/// slide along the second's at this velocity without friction (see `tests/physics.rs`), so
/// this is what friction brings the two to, if the push on the contact allows.
fn target_sliding(relative: Vec3, normal: Vec3, treads: impl Iterator<Item = (Tread, bool)>) -> Vec3 {
    let mut target = relative.reject_from_normalized(normal);
    for (tread, is_first) in treads {
        target -= tread.across * target.dot(tread.across);
        target -= tread.along * target.dot(tread.along) * (1.0 - tread.rolls);
        // A tread told to go faster than the surface slides along it, and friction pushes
        // its truck the other way: the first collider against `along`, the second with it.
        let sign = if is_first { 1.0 } else { -1.0 };
        target -= sign * tread.push * tread.along;
    }
    target
}

/// The middle of a manifold's points, in the world.
fn mean_point(manifold: &ContactManifold) -> Option<Vec3> {
    let count = manifold.points.len();
    (count > 0).then(|| {
        manifold
            .points
            .iter()
            .map(|point| point.point)
            .sum::<Vec3>()
            / count as f32
    })
}

impl TireContacts<'_, '_> {
    /// Decides every manifold of a pair of colliders that one of them asked about. A
    /// manifold the physics is not to solve is emptied of its points: by now the physics
    /// has counted each pair's manifolds for its solver, and a manifold with no points
    /// gives it nothing to do.
    fn modify(&self, contacts: &mut ContactPair) {
        let (first, second) = (contacts.collider1, contacts.collider2);
        for manifold in &mut contacts.manifolds {
            if !self.keep(first, second, manifold) {
                manifold.points.clear();
            }
        }
    }

    /// Whether the physics is to solve this manifold between `first` and `second`, which it
    /// may first have changed.
    fn keep(&self, first: Entity, second: Entity, manifold: &mut ContactManifold) -> bool {
        // The normal points out of the first collider, so the first is pushed against
        // it and the second along it. Each point is given from each body's centre of
        // mass, in the world, halfway between the two surfaces; each surface is half the
        // depth from there along the normal. Taken halfway, a tire pressed 0.2 m into
        // another was touched inside its tread, and so on its sidewall.
        let normal = manifold.normal;
        let on_first = self.on_the_wheel(
            first,
            manifold
                .points
                .iter()
                .map(|p| p.anchor1 + normal * p.penetration / 2.0),
        );
        let on_second = self.on_the_wheel(
            second,
            manifold
                .points
                .iter()
                .map(|p| p.anchor2 - normal * p.penetration / 2.0),
        );
        if self.suspension_takes(first, second, -normal, &on_first)
            || self.suspension_takes(second, first, normal, &on_second)
            || self.ground_holds_down(first, second, normal)
            || self.ground_holds_down(second, first, -normal)
        {
            return false;
        }
        // A tire with its springs shut pushes straight up off the ground, not back along
        // it. A wheel's core does not: it meets the ground where the tire has gone into
        // it, which is often a bank beside the wheel, and it must push the wheel back out
        // of the bank. Pushed straight up it slid into the bank and rode up inside it: an
        // axle was under the ground for 1 164 wheel-steps on Alpine in two minutes of 7
        // trucks, and for none pushed along the ground's own face. Nor where the core meets
        // a steep slope ahead: turned up there too, a truck driven head on into a bank of
        // 50 degrees or more at 10 to 30 m/s went through it, nearly every time.
        // Nor against the side of a ground box, which is a wall: turned up, it would let
        // the tire through.
        if self.bottomed(first) && self.is_ground(second) && !self.is_box_wall(second, -normal) {
            self.stand_it_up(first, true, manifold);
        } else if self.bottomed(second) && self.is_ground(first) && !self.is_box_wall(first, normal)
        {
            self.stand_it_up(second, false, manifold);
        } else if !self.meet_edge(first, second, -normal, true, manifold) {
            self.meet_edge(second, first, normal, false, manifold);
        }
        self.drive_contact(first, second, manifold);
        true
    }
}

/// How many contact pairs each task is given: a task costs more to hand out than a pair
/// does to decide.
const PAIRS_PER_TASK: usize = 16;

/// Decides the contacts of every wheel collider and core at the end of each narrow phase,
/// before the solver sees them, on as many threads as the machine has. The physics marks
/// the pairs to decide from `ActiveCollisionHooks::MODIFY_CONTACTS`.
///
/// A system rather than one of the physics' own collision hooks. A hook makes the physics
/// build its broad and narrow phase round the hook's type, in this crate, so that they ran
/// unoptimised in a development build; and a hook is asked about each pair as the narrow
/// phase finds it, one after another.
pub fn decide_tire_contacts(contacts: TireContacts, mut graph: ResMut<ContactGraph>) {
    let contacts = &contacts;
    ComputeTaskPool::get_or_init(TaskPool::default).scope(|scope| {
        for chunk in graph.active_pairs_mut().chunks_mut(PAIRS_PER_TASK) {
            scope.spawn(async move {
                for pair in chunk {
                    if pair.flags.contains(ContactPairFlags::MODIFY_CONTACTS) && pair.is_touching()
                    {
                        contacts.modify(pair);
                    }
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    /// A wheel facing -Z, with its axle along +X.
    const AXLE: Vec3 = Vec3::X;

    #[test]
    fn under_the_tire_the_tread_goes_backwards_and_at_its_front_down() {
        // The ground under the tire pushes it up.
        let (along, across) = tread_frame(AXLE, Vec3::NEG_Y, Vec3::Y).unwrap();
        assert!(close(along, Vec3::Z), "{along}");
        assert!(across.y.abs() < 1e-4 && across.x.abs() > 0.99, "{across}");
        // A wall ahead pushes the front of the tire back.
        let (along, across) = tread_frame(AXLE, Vec3::NEG_Z, Vec3::Z).unwrap();
        assert!(close(along, Vec3::NEG_Y), "{along}");
        assert!(across.x.abs() > 0.99, "{across}");
        // A touch on the axle itself has no tread.
        assert!(tread_frame(AXLE, Vec3::X, Vec3::X).is_none());
    }

    #[test]
    fn a_tire_rolls_freely_along_its_tread_and_not_across_it() {
        let (along, across) = tread_frame(AXLE, Vec3::NEG_Y, Vec3::Y).unwrap();
        let tread = Tread {
            along,
            across,
            push: 0.0,
            rolls: 1.0,
            grip: 1.0,
        };
        // Going forwards and a little sideways over still ground.
        let going = Vec3::new(0.5, 0.0, -10.0);
        let target = target_sliding(going, Vec3::Y, [(tread, true)].into_iter());
        // Forwards it is left to roll; the sideways part is to be stopped.
        assert!(close(target, Vec3::new(0.0, 0.0, -10.0)), "{target}");
        // On the sidewall nothing rolls: the tire holds like a block.
        let sidewall = Tread { rolls: 0.0, ..tread };
        let target = target_sliding(going, Vec3::Y, [(sidewall, true)].into_iter());
        assert!(close(target, Vec3::ZERO), "{target}");
        // The same wheel as the second collider: the relative velocity is the other way.
        let target = target_sliding(-going, Vec3::Y, [(tread, false)].into_iter());
        assert!(close(target, Vec3::new(0.0, 0.0, 10.0)), "{target}");
    }

    #[test]
    fn the_push_drives_the_truck_forwards_on_the_ground_and_up_a_wall() {
        let on_ground = tread_frame(AXLE, Vec3::NEG_Y, Vec3::Y).unwrap();
        let tread = Tread {
            along: on_ground.0,
            across: on_ground.1,
            push: 0.1,
            rolls: 1.0,
            grip: 1.0,
        };
        // Standing still: the tread is to go 0.1 m/s faster backwards, so the first
        // collider -- the tire -- is to go forwards (-Z) by that.
        let target = target_sliding(Vec3::ZERO, Vec3::Y, [(tread, true)].into_iter());
        assert!(close(target, Vec3::new(0.0, 0.0, -0.1)), "{target}");
        // Pressed against a wall ahead: the push carries the tire up it.
        let on_wall = tread_frame(AXLE, Vec3::NEG_Z, Vec3::Z).unwrap();
        let tread = Tread {
            along: on_wall.0,
            across: on_wall.1,
            ..tread
        };
        let target = target_sliding(Vec3::ZERO, Vec3::Z, [(tread, true)].into_iter());
        assert!(close(target, Vec3::new(0.0, 0.1, 0.0)), "{target}");
        // As the second collider, the relative velocity is the wall's less the tire's.
        let target = target_sliding(Vec3::ZERO, Vec3::Z, [(tread, false)].into_iter());
        assert!(close(target, Vec3::new(0.0, -0.1, 0.0)), "{target}");
    }

    #[test]
    fn the_tire_going_into_the_other_faster_is_the_one_that_rides_up() {
        assert!(rides_up(5.0, 1.0));
        assert!(!rides_up(1.0, 5.0));
        // Evenly matched, or nearly, the first keeps riding up.
        assert!(rides_up(3.0, 3.0));
        assert!(rides_up(3.0, 3.4));
        assert!(!rides_up(3.0, 3.6));
    }

    #[test]
    fn a_tread_grips_what_it_climbs_twice_over_and_another_tread_twice_that() {
        assert_eq!(climb_friction([1.1].into_iter()), 2.2);
        assert_eq!(climb_friction([1.1, 0.9].into_iter()), 4.4);
    }

    #[test]
    fn the_throttle_drives_the_tread_and_fades_at_the_top_speed_and_the_brakes_stop_it() {
        let config = TruckConfig::default();
        let dt = 1.0 / 120.0;
        let per_force = dt / (config.mass / 4.0);
        // Standing still under throttle: the engine's whole push.
        let push = asked_of_the_tread(0.0, 1.0, 20_000.0, false, &config, dt);
        assert!((push - 20_000.0 * per_force).abs() < 1e-6, "{push}");
        // Rolling at the top speed: none of it.
        assert_eq!(asked_of_the_tread(config.top_speed, 1.0, 20_000.0, false, &config, dt), 0.0);
        // Rolling backwards under forward throttle: the brakes, towards rolling with the
        // surface, and never past it.
        let braking = asked_of_the_tread(-10.0, 1.0, 20_000.0, false, &config, dt);
        assert!(braking > 0.0 && braking <= config.brake_force * per_force, "{braking}");
        // Barely rolling backwards under forward throttle is driving forwards, not braking.
        assert!(asked_of_the_tread(-0.1, 1.0, 20_000.0, false, &config, dt) > 0.0);
        // Held: the brakes whatever the throttle, nothing at a standstill, and no more than
        // stops the tread when it is nearly stopped already.
        assert!(asked_of_the_tread(5.0, 1.0, 20_000.0, true, &config, dt) < 0.0);
        assert_eq!(asked_of_the_tread(0.0, 1.0, 20_000.0, true, &config, dt), 0.0);
        assert_eq!(asked_of_the_tread(0.01, 1.0, 20_000.0, true, &config, dt), -0.01);
        // Off the throttle the engine only slows the tread, whichever way it rolls, and by
        // no more than its braking.
        let coasting = asked_of_the_tread(5.0, 0.0, -3_000.0, false, &config, dt);
        assert!(coasting < 0.0 && coasting >= -3_000.0 * per_force, "{coasting}");
        assert!(asked_of_the_tread(-5.0, 0.0, -3_000.0, false, &config, dt) > 0.0);
        assert_eq!(asked_of_the_tread(0.0, 0.0, -3_000.0, false, &config, dt), 0.0);
    }
}
