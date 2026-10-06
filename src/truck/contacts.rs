//! Which of a wheel collider's contacts the physics solves, and which the suspension
//! takes instead.
//!
//! A wheel collider is fastened to the chassis, so what it touches, it touches rigidly:
//! good for a wall or a rail, or another truck's wheel beside it, which the tire should
//! bump off. But a rigid contact under the tread would hold the hub where it is and let
//! nothing compress, and another truck's tire would be a wall on wheels instead of a hill.
//! The cast in `drive` already finds whatever is under the tread and lets the suspension
//! climb it, so a contact that pushes the wheel up on its tread is dropped here and left
//! to the cast. `decide_tire_contacts` is shown every pair of colliders one of which carries
//! `ActiveCollisionHooks::MODIFY_CONTACTS`, which the wheel colliders, their cores and the
//! body do (see `spawn`), with all their contact manifolds, and each manifold is decided on
//! its own: against the ground a tire has one for each triangle it touches.
//!
//! The ground is among what a wheel touches (see `collision_groups`), and nearly all of
//! it is dropped here, because nearly all of it is the cast's. What is not is the tire
//! above its axle: a truck tipped over lies on the tops of its wheels, and there the
//! suspension has nothing to push against -- it would be trying to lift the truck through
//! its own wheel, which once threw it off the ground (see `drive::UPWARD`). That contact
//! is kept, and it is what a tipped-over truck rests on.
//!
//! The body's contacts with the ground go through the same hook, for one rule: the ground
//! has no underside, so it never pushes a truck down. The physics sees a heightfield as
//! triangles, and a body whose centre has got under one, however it got there, is nearest
//! the underside and is held there by the contact: measured, a truck 1.65 m into level
//! ground sank until its roof met the ground from below and stayed. With that contact
//! dropped, the wheels' push (see `drive`) brings it up through. Under Rapier the body did
//! not ask for the hook, so this rule was never applied, and Bigfoot put 1.65 m into the
//! ground stayed there. The terrain never pushes a wheel collider or a core down either
//! (see `ground_holds_down`).
//!
//! Another truck is never the suspension's: `drive` does not sweep onto it, and every
//! contact a tire has with it is kept, so that one of them holds the tire at a time, and
//! never both. A tire driven into another truck low down, at or below its own axle --
//! another truck's tire, head on, or its bumper -- meets a face that pushes it straight
//! back, and held as it is, that face is a wall: measured, a truck doing 8.7 m/s into
//! another's rear tire lost all but 0.25 m/s of it in one step, and did not rise. A real
//! tire climbs it, by its grip and by giving where it is pressed. So does this one, where
//! it meets the other truck tread first: it walks up it by its grip (`climb`, below),
//! slowly and without bounce, and pushes the other truck down as hard as
//! it is carried up. Its throttle only sets how fast; a tire that coasts in is stopped and
//! stays. Not measured yet: it wants driving.
//!
//! Met any other way -- sideways, or by the side of the tire -- the contact is made a ramp
//! instead: pushed up as well as back, the tire rides up the other truck and on over it,
//! and the other truck is pushed down and on. Such a contact has no bounce and no
//! friction, so that the tire slides up rather than ricocheting or scrubbing. Every tire
//! on another truck was met so once, and a ramp turns the speed two trucks meet at into
//! lift within one step: measured, the truck above rose 1.15 m onto the other, which is a
//! launch and not a climb. Seven computer trucks racing on Alpine for two minutes were
//! then tipped past 60 degrees in 3 of 1912 samples, where they had been in 61.
//!
//! Light loose scenery -- a cone, a sign, a small rock (`drive::LIGHT_LOOSE`) -- is met as
//! another truck is, and for the same reason: `drive` does not sweep onto it (see its
//! notes), so every contact a tire has with it is kept, and one low on the tire is made a
//! ramp, which the tire rides up while pushing the object down and on. Left to the sweep,
//! the contact under the tread was dropped here, and a cone ended up inside the tire.
//! Heavy loose scenery, such as a parked car, is met as fixed scenery is.
//!
//! A face of scenery or of a ground box that a tire hits -- a kerb, a log, a ledge, a
//! wall, the side of a parked car -- is `drive`'s first. The upright sides of the ground
//! boxes are faces too, although the boxes are ground: most of a track's walls are made of
//! them. A face is met in the middle of a step, after `drive` has looked under the tire.
//! So a wheel's contact with a face met squarely (`drive::meets_squarely`) is dropped here
//! until the tire is `FACE_SQUASH` of its radius into it (`squashes`), and `drive` looks
//! again before the next step. If the springs reach the top of the face, the suspension
//! rolls the tire up onto it; the tire, and its core, may then go as far as `EDGE_DIG` into
//! that face while the springs lift it (`WheelCollider::rolling_over`). If they do not,
//! the face is a wall: `drive` squashes the tire against it and stops it over a few steps,
//! and the tread rides up the wall from the hit and drops back. Deeper than that, or met
//! at a glance, the wheel collider meets the face rigidly (`meet_face`): with the tire's
//! grip while it rides up from a hit, with no friction after, and with no bounce.
//!
//! Nothing but the hit lifts a tire up a wall, so a truck that crawls at a wall stays at
//! its foot. The throttle walked a tire up a wall by its grip before, and Bigfoot and MAXD
//! crawled up 2 to 3 m walls; measured with Avian, in 27 crawls at walls of 2 to 3 m, 8
//! got over before and none do now. Walls of 1.25 m and less: 144 runs of 144 get over,
//! where 130 did. The terrain has no upright faces, and a tire whose tread meets it
//! nearly upright still walks up it under throttle (`climb`).
//!
//! `physics::GamePhysicsPlugin` runs `decide_tire_contacts`. An app that sets up the
//! physics without it keeps every contact, and its trucks bump into each other rather
//! than climb.
//!
//! The figures above were measured with Rapier (see `docs/avian.md`), except those about
//! walls, which were measured with Avian.

use avian3d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::tasks::{ComputeTaskPool, TaskPool};

use super::drive::{
    AXLE_ALONG_X, FACE_SQUASH, LIGHT_LOOSE, SIDEWALL, UPWARD, WALL_CARRY, WALL_FACING,
    below_the_axle, carries, meets_squarely,
};
use super::spawn::CORE_SHARE;
use super::{Truck, TruckConfig, TruckInput, WheelCollider, WheelCore};
use crate::collision_groups::{GROUND, GROUND_BOXES};

#[derive(SystemParam)]
pub struct TireContacts<'w, 's> {
    wheels: Query<
        'w,
        's,
        (
            &'static Collider,
            &'static Transform,
            &'static ChildOf,
            &'static WheelCollider,
        ),
    >,
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
    cores: Query<'w, 's, (&'static ColliderOf, &'static ChildOf), With<WheelCore>>,
    /// The body each collider is part of, and what kind of body that is.
    bodies: Query<'w, 's, &'static ColliderOf>,
    kinds: Query<'w, 's, (&'static RigidBody, &'static ComputedMass)>,
    /// What each truck's driver asks of it, and its tires' grip, for `climb`.
    drivers: Query<'w, 's, (&'static TruckInput, &'static TruckConfig), With<Truck>>,
    /// The physics' clock, for how long a step is.
    time: Res<'w, Time>,
}

/// How steeply a tire rides up another truck or light loose scenery that it meets other
/// than tread first, in radians above level: the push that holds it back is turned up at
/// least this far. Steeper lifts it sooner and takes less of its speed, and pushes the
/// other down harder. It is also where a tire that walks up another truck is on top of it.
const RAMP: f32 = 35.0 * std::f32::consts::PI / 180.0;

/// How high above its own axle a tire may meet another truck and still ride up it, as a
/// fraction of the tire's radius. Higher, the other truck is a wall taller than the tire,
/// which no tire climbs.
const RAMP_REACH: f32 = 0.3;

/// How much full throttle at another truck counts for in which of two tires rides up the
/// other, as m/s of the speed they meet at. Once two trucks have met they go at one speed,
/// and by speed alone the tire that rode up was a toss of a coin in every step. Higher
/// lets the driver's foot decide over more of a difference in speed.
const RIDER_DRIVE: f32 = 5.0;

/// How much harder the second of two tires must be driven into the first to be the one
/// that rides up, in m/s (see `RIDER_DRIVE`). Under it the first rides up, so that two
/// tires evenly matched do not take turns. Higher holds on to the first for longer.
const RIDER_MARGIN: f32 = 0.5;

/// How fast a tire walks up another truck or the terrain under full throttle, in m/s (see
/// `climb`). Higher climbs quicker. A first guess, which wants driving.
const CLIMB_SPEED: f32 = 1.5;

/// How quickly a climbing tire gathers that speed, in m/s²: the most its speed up the face
/// is changed by in a second. Friction carries a tire to whatever speed its tread is said
/// to turn at within one step, if the push on the face allows, and a hit at speed allows
/// anything: told `CLIMB_SPEED` outright, a tire was thrown up at all of it in a step.
/// Higher starts the climb more sharply. A first guess, which wants driving.
const CLIMB_PICKUP: f32 = 4.0;

/// How far up the way the truck is driven may point for a tire to climb at full strength,
/// and where the climb has faded to nothing, in radians. Between them it fades evenly.
/// Higher lets a truck go up taller walls, and go over on its back from one sooner.
const CLIMB_PITCH_FULL: f32 = 35.0 * std::f32::consts::PI / 180.0;
const CLIMB_PITCH_NONE: f32 = 55.0 * std::f32::consts::PI / 180.0;

/// How long a tire that rides up a face keeps riding up it, in seconds: the time its speed
/// up the face takes to fall to a third. Longer rides further up.
const FACE_RIDE_TIME: f32 = 0.25;

/// The speed up a face, in m/s, under which a tire that rode up it stops riding and drops
/// back down.
const FACE_RIDE_END: f32 = 0.5;

/// How far a tire may go into the face of what the suspension rolls it over the top of, as
/// a share of its radius, before the face stops it outright (see `squashes`): as far as an
/// edge as high as the hub, met at 8 m/s, digs in while the springs lift the tire onto it.
/// Lower stops such a tire outright sooner, which jolted the truck by up to 50 g; higher
/// lets a tire further into a face.
const EDGE_DIG: f32 = 0.5;

impl TireContacts<'_, '_> {
    /// Whether a wheel collider that is `pushed` this way, in the world, at these points
    /// of its own cylinder (whose axis is Y), would rather be carried by its suspension.
    ///
    /// Against another truck or light loose scenery, never: the cast does not see either
    /// (see the module's notes).
    /// Against anything else but the ground -- a wall, a rail, a rock -- it is the tread
    /// pushed up, as the truck stands and as the world does (`drive::carries`), and nothing
    /// else: the cast climbs that, and the rest should bump (`meet_face`).
    ///
    /// Against the ground it is everything the springs can still answer for, which is
    /// nearly all of it, because the cast already covers the whole tire, sidewalls
    /// included. Two cases are not theirs. A tire pressed past the top of its travel has
    /// shut its springs, and from there, once it has squashed as far as it goes
    /// (`drive::GROUND_SQUASH` on the terrain, `drive::EDGE_SQUASH` on a ground box), the
    /// tire is simply solid: the contact holds it, so a truck that lands heavily stops on
    /// its tires and bounces off them, instead of sinking into the ground and being thrown
    /// back out by a spring. And a tire pressed on the ground above its own axle is a truck
    /// tipped over lying on the tops of its wheels, where the suspension has nothing to
    /// push against at all (see `drive::UPWARD`).
    fn suspension_takes(&self, wheel: Entity, other: Entity, pushed: Vec3, at: &[Vec3]) -> bool {
        let Ok((collider, wheel_at, child_of, wheel_collider)) = self.wheels.get(wheel) else {
            return false;
        };
        let (Ok((_, rotation, ..)), Some(cylinder)) =
            (self.trucks.get(child_of.0), collider.shape().as_cylinder())
        else {
            return false;
        };
        let up = rotation.0 * Vec3::Y;
        // `drive` keeps the collider turned by the steering and onto its axle; the cast is
        // given the steering alone, so take the axle back off.
        let steer = wheel_at.rotation * AXLE_ALONG_X.inverse();
        let on_the_tread = |point: Vec3| point.xz().length() / cylinder.radius > SIDEWALL;
        if self.is_ground(other) {
            // An upright side of a ground box is a wall and not ground: the tire climbs it
            // or bumps off it, as it does a wall of scenery. Dropped as ground is, it let
            // the tire into the box as far as the wheel's core, with nothing to climb by.
            if self.is_box_wall(other, pushed) {
                return false;
            }
            let upward = pushed.dot(up) > UPWARD;
            // Solid, but only underneath. A tire whose springs are shut still meets the
            // rising face of a bump with its front, and that push is nearly level: held
            // rigidly it is a kerb to stub against, and the ground's friction is 1.0, so
            // the truck stops dead instead of riding over. Measured, one step took 9.6 m/s
            // off a truck doing 22, and one doing 8 was stopped outright. Those faces stay
            // the cast's, which climbs them; what is kept is the push that holds the truck
            // up, which is what the springs can no longer do.
            if wheel_collider.bottomed && upward {
                return false;
            }
            let lying_on_it = !upward
                && at
                    .iter()
                    .any(|&point| on_the_tread(point) && !below_the_axle(steer, point));
            return !lying_on_it;
        }
        !self.is_truck_part(other)
            && !self.is_loose(other)
            && carries(pushed, up, false)
            && at.iter().copied().any(on_the_tread)
    }

    /// Whether a contact of a wheel collider, or of its core, with `other`, which pushes it
    /// this way, in the world, is with a face of scenery or of a ground box that the tire
    /// has gone into no further than `FACE_SQUASH`: `drive`'s, which rolls the tire onto the
    /// top of the face if the springs reach it, and squashes the tire against it if not. A
    /// face is met in the middle of a step, after `drive` has looked under the tire, and it
    /// looks again before the next. The face of what `drive` rolls the tire over the top
    /// of (`WheelCollider::rolling_over`) it may go further into, as far as `EDGE_DIG`.
    fn squashes(
        &self,
        collider: Entity,
        other: Entity,
        pushed: Vec3,
        manifold: &ContactManifold,
    ) -> bool {
        // A core is a ball inside its tire, `CORE_SHARE` of its radius.
        let (wheel, inside) = match self.cores.get(collider) {
            Ok((_, child_of)) => (child_of.0, 1.0 - CORE_SHARE),
            Err(_) => (collider, 0.0),
        };
        let Ok((shape, wheel_at, child_of, wheel_collider)) = self.wheels.get(wheel) else {
            return false;
        };
        let (Some(cylinder), Ok((_, rotation, ..))) =
            (shape.shape().as_cylinder(), self.trucks.get(child_of.0))
        else {
            return false;
        };
        // Over an edge the tire, and more so its core, also meets the top of the face at its
        // corner, which pushes it up as well as back.
        let rolling_over = wheel_collider.rolling_over == Some(other);
        let face = if rolling_over {
            pushed.y >= -UPWARD
        } else if self.is_ground(other) {
            self.is_box_wall(other, pushed)
        } else {
            pushed.y.abs() <= UPWARD && !self.is_truck_part(other) && !self.is_loose(other)
        };
        let dig = if rolling_over { EDGE_DIG } else { FACE_SQUASH };
        let deepest = (dig - inside) * cylinder.radius;
        if !face
            || manifold
                .points
                .iter()
                .any(|point| point.penetration > deepest)
        {
            return false;
        }
        // A face met at a glance, by the side of the tire, is left as it was: the tire
        // bounces off it.
        let steer = wheel_at.rotation * AXLE_ALONG_X.inverse();
        let heading = rotation.0 * steer * Vec3::NEG_Z;
        let going = mean_point(manifold).and_then(|point| self.truck_moving_at(wheel, point));
        rolling_over || going.is_some_and(|going| meets_squarely(going, heading, pushed))
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

    /// The truck a collider is part of.
    fn truck_owning(&self, collider: Entity) -> Option<Entity> {
        if self.trucks.contains(collider) {
            Some(collider)
        } else {
            self.truck_of(collider)
        }
    }

    /// If a wheel collider is driven into another truck, or into light loose scenery, low
    /// enough on the tire to ride up it, front on or sideways, that wheel and the way it is
    /// pushed, in the world. Where two tires meet, both may be low enough, and the one
    /// driven into the other the harder is the one that rides up: by how fast its truck
    /// goes at the other, and by its throttle (`RIDER_DRIVE`). The second must beat the
    /// first by `RIDER_MARGIN`.
    fn riding_up(
        &self,
        candidates: [(Entity, Entity, Vec3, &[Vec3]); 2],
    ) -> Option<(Entity, Vec3)> {
        candidates
            .into_iter()
            .filter_map(|(wheel, other, pushed, at)| {
                let (collider, wheel_at, child_of, _) = self.wheels.get(wheel).ok()?;
                let cylinder = collider.shape().as_cylinder()?;
                let truck = child_of.0;
                let climbable = self.is_truck_part(other) || self.is_loose(other);
                if !climbable || self.truck_owning(other) == Some(truck) {
                    return None;
                }
                // On the tread or on the side of the tire alike, so that a truck sliding
                // sideways into another rides up it as one driven into it nose first does.
                // Met only on the tread, the side of a tire was a wall: measured, a truck
                // sliding sideways into a parked one at 10 m/s never rose onto it, and end
                // on it bounced off and was thrown 18 m away.
                let low_enough = at
                    .iter()
                    .any(|&point| (wheel_at.rotation * point).y <= RAMP_REACH * cylinder.radius);
                let (_, rotation, velocity, ..) = self.trucks.get(truck).ok()?;
                let (input, _) = self.drivers.get(truck).ok()?;
                let steer = wheel_at.rotation * AXLE_ALONG_X.inverse();
                let heading = rotation.0 * steer * Vec3::NEG_Z;
                let driven_in =
                    velocity.0.dot(-pushed) + RIDER_DRIVE * input.throttle * heading.dot(-pushed);
                low_enough.then_some((wheel, pushed, driven_in))
            })
            .reduce(|first, second| {
                if second.2 > first.2 + RIDER_MARGIN {
                    second
                } else {
                    first
                }
            })
            .map(|(wheel, pushed, _)| (wheel, pushed))
    }

    /// Make a wheel collider walk up a face by its grip (see the module's notes), and say
    /// whether it was. `pushed` is the way the face pushes the wheel, in the world, `at`
    /// where it touches the wheel, in the wheel's own frame, and `wheel_first` whether the
    /// wheel is the manifold's first collider.
    ///
    /// `ridden` is for a tire that rides up another truck (`riding_up`), tread first. It
    /// walks up a face as steep as a wall, or as gentle as `RAMP`, where it is on top; and
    /// with no throttle it is only held, so that a tire that coasts into another truck
    /// neither bounces off it nor is thrown up it. Otherwise it is the terrain met nearly
    /// upright, which a tire walks up only under throttle.
    ///
    /// Not with the side of the tire: a sidewall against a bank has nothing to climb with.
    #[allow(clippy::too_many_arguments)]
    fn climb(
        &self,
        wheel: Entity,
        other: Entity,
        pushed: Vec3,
        at: &[Vec3],
        wheel_first: bool,
        ridden: bool,
        manifold: &mut ContactManifold,
    ) -> bool {
        let steepest = if ridden { RAMP.sin() } else { UPWARD };
        let Some(face) = self.face(wheel, other, pushed, at, manifold, steepest) else {
            return false;
        };
        let Some((input, config)) = self.drivers.get(face.truck).ok() else {
            return false;
        };
        // How hard the tire is driven at the face. Driven away from it, not at all.
        let throttle = (input.throttle * face.ahead.signum()).max(0.0);
        // Only a leading wheel fades as the nose comes up: it is what lifts the nose further.
        // A trailing one lowers it again by climbing, and it must, for the truck to get its
        // back wheels up after its front ones.
        let leading = face.wheel_z * face.ahead.signum() < 0.0;
        let pitch = (face.heading.y * face.ahead.signum())
            .clamp(-1.0, 1.0)
            .asin();
        let fade = if leading {
            ((CLIMB_PITCH_NONE - pitch) / (CLIMB_PITCH_NONE - CLIMB_PITCH_FULL)).clamp(0.0, 1.0)
        } else {
            1.0
        };
        if !ridden && (throttle == 0.0 || fade == 0.0) {
            return false;
        }
        // Up towards the climbing speed, a little faster each step than the tire already
        // goes (see `CLIMB_PICKUP`), and along the face as it already goes, so that it does
        // not scrub.
        let rising = face.against.dot(face.up_the_face);
        let wanted = CLIMB_SPEED * throttle * fade;
        let pickup = CLIMB_PICKUP * self.time.delta_secs();
        let up_speed = rising + (wanted - rising).clamp(-pickup, pickup);
        let sliding = face.up_the_face * up_speed + face.along;
        manifold.tangent_velocity = if wheel_first { sliding } else { -sliding };
        manifold.friction = config.grip;
        // A tire that climbs is not thrown back off the face.
        manifold.restitution = 0.0;
        true
    }

    /// Let a wheel collider that hits a face it cannot roll over -- a wall too tall for its
    /// springs to reach the top of, the side of a parked car, or the side of another truck
    /// above where a tire rides up it -- squash into it and ride up it from the hit (see
    /// the module's notes), and say whether it did. `pushed` is the way the face pushes
    /// the wheel, in the world, `at` where it touches the wheel, in the wheel's own frame,
    /// and `wheel_first` whether the wheel is the manifold's first collider.
    ///
    /// Until the tire is `FACE_SQUASH` into the face, the face is `drive`'s (see
    /// `squashes`); this is the face beyond that, which holds the tire rigidly, and a face
    /// met at a glance. The tread grips the face and carries the tire up it at `WALL_CARRY`
    /// of the speed it goes into it at, which only a hit has: a tire pressed against a face
    /// by the throttle goes into it at nothing, and stays at its foot. Riding up, it slows
    /// over `FACE_RIDE_TIME`, and then drops back as it would with nothing to grip. Along
    /// the face it goes as it already goes, so that it does not scrub, and it does not
    /// bounce off the face.
    fn meet_face(
        &self,
        wheel: Entity,
        other: Entity,
        pushed: Vec3,
        at: &[Vec3],
        wheel_first: bool,
        manifold: &mut ContactManifold,
    ) -> bool {
        // The terrain has slopes and no upright faces (see `drive::carries`).
        if self.is_terrain(other) {
            return false;
        }
        let Some(face) = self.face(wheel, other, pushed, at, manifold, UPWARD) else {
            return false;
        };
        let Some((_, config)) = self.drivers.get(face.truck).ok() else {
            return false;
        };
        let dt = self.time.delta_secs();
        let going_in = (-face.against.dot(pushed)).max(0.0);
        let rising = face.against.dot(face.up_the_face);
        let carry = WALL_CARRY * going_in;
        let up_speed = carry.max(rising * (-dt / FACE_RIDE_TIME).exp());
        // Done riding up, it has nothing to grip by: a tire pressed against a face by the
        // throttle neither climbs it nor is held down by it.
        let riding = carry.max(rising) > FACE_RIDE_END;
        let sliding = face.up_the_face * up_speed + face.along;
        manifold.tangent_velocity = if wheel_first { sliding } else { -sliding };
        manifold.friction = if riding { config.grip } else { 0.0 };
        // A tire that rides up is not thrown back off the face.
        manifold.restitution = 0.0;
        true
    }

    /// What `climb` and `meet_face` need to know of a wheel collider that meets a face, if
    /// it is one they take: on the tread, squarely enough (`WALL_FACING`), a face no
    /// gentler than `steepest` (the sine of its lean from upright, as the world stands) and
    /// not the underside of something, and not light loose scenery or its own truck.
    fn face(
        &self,
        wheel: Entity,
        other: Entity,
        pushed: Vec3,
        at: &[Vec3],
        manifold: &ContactManifold,
        steepest: f32,
    ) -> Option<Face> {
        let (collider, wheel_at, child_of, _) = self.wheels.get(wheel).ok()?;
        let cylinder = collider.shape().as_cylinder()?;
        let truck = child_of.0;
        let (_, rotation, ..) = self.trucks.get(truck).ok()?;
        let on_the_tread = |point: &Vec3| point.xz().length() / cylinder.radius > SIDEWALL;
        let other_truck = self.truck_owning(other);
        if self.is_loose(other) || other_truck == Some(truck) || !at.iter().any(on_the_tread) {
            return None;
        }
        // Where the tire points, and how squarely the face stands in its way: ahead of it,
        // or behind it if this is negative.
        let steer = wheel_at.rotation * AXLE_ALONG_X.inverse();
        let heading = rotation.0 * steer * Vec3::NEG_Z;
        let ahead = heading.dot(-pushed);
        // Up the face, as the world has it.
        let up_the_face = Vec3::Y.reject_from_normalized(pushed).normalize_or_zero();
        let steep = (-UPWARD..=steepest).contains(&pushed.y);
        if ahead.abs() < WALL_FACING || !steep || up_the_face == Vec3::ZERO {
            return None;
        }
        let point = mean_point(manifold)?;
        let moving = self.truck_moving_at(wheel, point)?;
        // Against the face, which is another truck's going its own way.
        let face_moving = other_truck
            .and_then(|other| self.moving_at(other, point))
            .unwrap_or(Vec3::ZERO);
        let against = moving - face_moving;
        let along = against
            .reject_from_normalized(pushed)
            .reject_from_normalized(up_the_face);
        Some(Face {
            truck,
            heading,
            ahead,
            wheel_z: wheel_at.translation.z,
            up_the_face,
            against,
            along,
        })
    }

    /// Let a wheel collider standing on top of another truck drive on it under throttle, as
    /// it would on the ground. `pushed` is the way the other truck holds it up, in the
    /// world, and `wheel_first` whether the wheel is the manifold's first collider.
    ///
    /// The suspension does not see another truck (see `drive`), so nothing else drives a
    /// tire on one, and a contact that a tire rides up is frictionless. Without this a
    /// truck driven broadside into another got its front wheels up onto it and stuck there
    /// at 38 degrees, its back wheels unable to push it up alone. The tire is carried
    /// forward towards `CLIMB_SPEED`, a little faster each step (`CLIMB_PICKUP`), and as
    /// fast as it already goes if that is faster, so that the grip never brakes it;
    /// sideways it slides as it already does.
    fn drive_on_top(
        &self,
        wheel: Entity,
        other: Entity,
        pushed: Vec3,
        wheel_first: bool,
        manifold: &mut ContactManifold,
    ) {
        let Ok((_, wheel_at, child_of, _)) = self.wheels.get(wheel) else {
            return;
        };
        let truck = child_of.0;
        let (Ok((_, rotation, ..)), Ok((input, config)), Some(other_truck)) = (
            self.trucks.get(truck),
            self.drivers.get(truck),
            self.truck_owning(other),
        ) else {
            return;
        };
        let Some(point) = mean_point(manifold) else {
            return;
        };
        let (Some(moving), Some(under)) = (
            self.moving_at(truck, point),
            self.moving_at(other_truck, point),
        ) else {
            return;
        };
        let steer = wheel_at.rotation * AXLE_ALONG_X.inverse();
        let driven = (rotation.0 * steer * Vec3::NEG_Z * input.throttle.signum())
            .reject_from_normalized(pushed)
            .normalize_or_zero();
        if input.throttle == 0.0 || driven == Vec3::ZERO {
            return;
        }
        let sliding = (moving - under).reject_from_normalized(pushed);
        let going = sliding.dot(driven);
        let wanted = going.max(CLIMB_SPEED * input.throttle.abs());
        let pickup = CLIMB_PICKUP * self.time.delta_secs();
        let sliding = sliding + driven * (wanted - going).min(pickup);
        manifold.tangent_velocity = if wheel_first { sliding } else { -sliding };
        manifold.friction = config.grip;
    }

    /// The truck a wheel's collider or core is part of.
    fn truck_of(&self, wheel: Entity) -> Option<Entity> {
        self.wheels
            .get(wheel)
            .map(|(_, _, child_of, _)| child_of.0)
            .or_else(|_| self.cores.get(wheel).map(|(of, _)| of.body))
            .ok()
    }

    /// Whether this collider is a wheel whose springs are shut, and so solid.
    fn bottomed(&self, collider: Entity) -> bool {
        self.wheels
            .get(collider)
            .is_ok_and(|(_, _, _, wheel)| wheel.bottomed)
    }

    /// Whether this collider is the terrain.
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

    /// Whether `ground` is the ground and `part` a truck's body that it is pushing this
    /// way, in the world, downward: the underside of the ground, which isn't there. Or a
    /// wheel collider or core that the terrain pushes down: the terrain is a field of
    /// heights, whose every face holds a tire up, so a push down is from under one of its
    /// triangles, met by a tire sunk into a slope. Kept, and turned straight down for a
    /// tire whose springs are shut (`stand_it_up`), it stopped a tire that rose out of the
    /// ground at 7 m/s in one step, a jolt of 38 g. Measured over rough ground at 15 and
    /// 25 m/s, the hardest jolt of a run was 57 g on average, and is 32 g. Not the ground
    /// boxes, whose upright sides are walls.
    fn ground_holds_down(&self, ground: Entity, part: Entity, pushed: Vec3) -> bool {
        let wheel = self.wheels.contains(part) || self.cores.contains(part);
        ((self.trucks.contains(part) && self.is_ground(ground))
            || (wheel && self.is_terrain(ground)))
            && pushed.y < 0.0
    }

    /// Where a contact touches a wheel collider, in the collider's own frame (the
    /// cylinder's axis is Y), from `touches`: each point on the wheel's surface, from its
    /// truck's centre of mass, in the world. Nothing for any other collider.
    fn on_the_wheel(&self, collider: Entity, touches: impl Iterator<Item = Vec3>) -> Vec<Vec3> {
        let Ok((_, wheel_at, child_of, _)) = self.wheels.get(collider) else {
            return Vec::new();
        };
        let Ok((_, rotation, _, _, centre)) = self.trucks.get(child_of.0) else {
            return Vec::new();
        };
        touches
            .map(|touch| {
                let in_the_truck = centre.0 + rotation.0.inverse() * touch;
                wheel_at.rotation.inverse() * (in_the_truck - wheel_at.translation)
            })
            .collect()
    }

    /// How fast a wheel's truck goes at `point`, in the world.
    fn truck_moving_at(&self, part: Entity, point: Vec3) -> Option<Vec3> {
        self.moving_at(self.truck_owning(part)?, point)
    }

    /// How fast `truck` goes at `point`, in the world.
    fn moving_at(&self, truck: Entity, point: Vec3) -> Option<Vec3> {
        let (position, rotation, linear, angular, centre) = self.trucks.get(truck).ok()?;
        let centre = position.0 + rotation.0 * centre.0;
        Some(linear.0 + angular.0.cross(point - centre))
    }

    /// Let a kept ground contact on a wheel roll instead of scrub.
    ///
    /// A wheel collider is bolted to the chassis and never turns -- only what is drawn
    /// does -- so to the solver its tread is a skid, and a contact the suspension has
    /// handed back (a tire whose springs are shut, riding a bump) brakes the truck as if
    /// the wheel were locked. The ground's friction is 1.0. Measured on the built-in
    /// track, a truck doing 22 m/s over a rise lost half its energy in a single step and
    /// was spat upwards: it caught on the bump instead of riding over it.
    ///
    /// Telling the solver the tread slides over the ground at exactly the speed the truck
    /// is passing over it leaves nothing to rub: the contact still holds the tire out of
    /// the ground, and the grip belongs to `drive`, which has already worked it out from
    /// the load on the tire. Without this the two are counted twice over, and the second
    /// one is a locked wheel. The physics lets the first collider slide along the second
    /// at `tangent_velocity` (see `tests/physics.rs`), and the ground stands still.
    fn let_it_roll(&self, wheel: Entity, wheel_first: bool, manifold: &mut ContactManifold) {
        let Some(point) = mean_point(manifold) else {
            return;
        };
        let Some(moving) = self.truck_moving_at(wheel, point) else {
            return;
        };
        let sliding = moving.reject_from_normalized(manifold.normal);
        manifold.tangent_velocity = if wheel_first { sliding } else { -sliding };
    }

    /// Turn a contact between the ground and a tire whose springs are shut to push
    /// straight up.
    ///
    /// Such a tire is solid (see `suspension_takes`), and where the ground rises ahead of
    /// it, a solid tire is stopped as a wheel is by a kerb: the ground pushes back along
    /// its slope, and takes the truck's speed away. Measured at speed on Alpine, 211 of 212
    /// sudden losses of forward speed with no other truck near had a tire with its springs
    /// shut, on slopes of 2 to 24 degrees, and the worst took 9.7 m/s off a truck doing 34.
    /// Pushed straight up, the tire still stands on the ground and comes off it with the
    /// ground's small bounce, upwards; with the bump stop as well, the speed these stops
    /// took away went from 563 m/s in all to 179. Pushed straight up without the bump
    /// stop, it went up: the stop is what keeps the springs from shutting in the first
    /// place.
    ///
    /// The bounce goes by how fast the two meet along the normal, which the physics worked
    /// out along the old one, so that is worked out again.
    fn stand_it_up(&self, wheel: Entity, wheel_first: bool, manifold: &mut ContactManifold) {
        manifold.normal = Vec3::Y * manifold.normal.y.signum();
        let normal = manifold.normal;
        for point in &mut manifold.points {
            let Some(moving) = self.truck_moving_at(wheel, point.point) else {
                continue;
            };
            // The second collider's speed less the first's; the ground stands still.
            let meeting = if wheel_first { -moving } else { moving };
            point.normal_speed = meeting.dot(normal);
        }
    }
}

/// A wheel collider meeting a face, as `TireContacts::face` finds it.
struct Face {
    /// The wheel's truck.
    truck: Entity,
    /// Where the tire points, in the world.
    heading: Vec3,
    /// How squarely the face stands in the tire's way, as a cosine: ahead of it, or behind
    /// it if this is negative.
    ahead: f32,
    /// Where the wheel is along the truck, in the truck's axes.
    wheel_z: f32,
    /// Up the face, as the world has it.
    up_the_face: Vec3,
    /// How fast the tire goes against the face, in the world, in m/s.
    against: Vec3,
    /// That, along the face and level.
    along: Vec3,
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
        let wheel = |collider| self.wheels.contains(collider) || self.cores.contains(collider);
        // The normal points out of the first collider, so the first is pushed against
        // it and the second along it. Each point is given from each body's centre of
        // mass, in the world, halfway between the two surfaces; each surface is half the
        // depth from there along the normal. Taken halfway, a tire pressed 0.2 m into
        // another was touched inside its tread, and so on its sidewall, and stopped as by
        // a wall instead of riding up.
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
            || self.squashes(first, second, -normal, manifold)
            || self.squashes(second, first, normal, manifold)
            || self.ground_holds_down(first, second, normal)
            || self.ground_holds_down(second, first, -normal)
        {
            return false;
        }
        // A tire driven into another truck or light loose scenery low down rides up it (see
        // the module's notes).
        if let Some((rider, pushed)) = self.riding_up([
            (first, second, -normal, &on_first),
            (second, first, normal, &on_second),
        ]) {
            let wheel_first = rider == first;
            let (other, at) = if wheel_first {
                (second, on_first.as_slice())
            } else {
                (first, on_second.as_slice())
            };
            // Up as the world stands, not as the truck does: a truck with its nose up on
            // another, its back tires against the other's, took their level push for one
            // already turned up, and could not get them up after its front ones.
            let up = Vec3::Y;
            let on_top = pushed.dot(up) >= RAMP.sin();
            // Tread first into another truck, it walks up it by its grip, as up a wall.
            if !on_top && self.climb(rider, other, pushed, at, wheel_first, true, manifold) {
                return true;
            }
            // Any other way, and up light loose scenery, it slides up a ramp.
            let level = pushed.reject_from_normalized(up).normalize_or_zero();
            if !on_top && level != Vec3::ZERO {
                let ramp = level * RAMP.cos() + up * RAMP.sin();
                manifold.normal = if wheel_first { -ramp } else { ramp };
            }
            manifold.friction = 0.0;
            manifold.restitution = 0.0;
            if on_top {
                self.drive_on_top(rider, other, pushed, wheel_first, manifold);
            }
            return true;
        }
        // A tire that hits a face squashes into it and rides up it from the hit (see the
        // module's notes): fixed or heavy scenery, or what of another truck is too high on
        // the tire to ride up.
        if self.meet_face(first, second, -normal, &on_first, true, manifold)
            || self.meet_face(second, first, normal, &on_second, false, manifold)
        {
            return true;
        }
        // The terrain met nearly upright by the tread, as the front of a steep rise, a tire
        // still walks up under throttle, as it did before walls stopped being climbed:
        // without it the rough ground of the probe jolted the trucks over 5 g 308 times
        // where it had 246, and its worst jolts rose from 756 g in all to 1015.
        if (self.is_terrain(second)
            && self.climb(first, second, -normal, &on_first, true, false, manifold))
            || (self.is_terrain(first)
                && self.climb(second, first, normal, &on_second, false, false, manifold))
        {
            return true;
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
        }
        // Whatever is left on a wheel or its core against the ground rolls; it does not
        // skid.
        if wheel(first) && self.is_ground(second) {
            self.let_it_roll(first, true, manifold);
        } else if wheel(second) && self.is_ground(first) {
            self.let_it_roll(second, false, manifold);
        }
        true
    }
}

/// How many contact pairs each task is given: a task costs more to hand out than a pair
/// does to decide.
const PAIRS_PER_TASK: usize = 16;

/// Decides the contacts of every wheel collider, core and truck body at the end of each
/// narrow phase, before the solver sees them, on as many threads as the machine has. The
/// physics marks the pairs to decide from `ActiveCollisionHooks::MODIFY_CONTACTS`.
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
