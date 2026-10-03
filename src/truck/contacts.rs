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
//! ground stayed there.
//!
//! Another truck is never the suspension's: `drive` does not sweep onto it, and every
//! contact a tire has with it is kept, so that one of them holds the tire at a time, and
//! never both. A tire driven into another truck low down, at or below its own axle --
//! another truck's tire, head on, or its bumper -- meets a face that pushes it straight
//! back, and held as it is, that face is a wall: measured, a truck doing 8.7 m/s into
//! another's rear tire lost all but 0.25 m/s of it in one step, and did not rise. A real
//! tire climbs it, by its grip and by giving where it is pressed. Here the contact is made
//! a ramp instead: pushed up as well as back, the tire rides up the other truck and on
//! over it, and the other truck is pushed down and on. Such a contact has no bounce and no
//! friction, so that the tire rolls up rather than ricocheting or scrubbing: measured, the
//! same truck rose 1.15 m onto the other. Seven computer trucks racing on Alpine for two
//! minutes were tipped past 60 degrees in 3 of 1912 samples, where they had been in 61.
//!
//! Light loose scenery -- a cone, a sign, a small rock (`drive::LIGHT_LOOSE`) -- is met as
//! another truck is, and for the same reason: `drive` does not sweep onto it (see its
//! notes), so every contact a tire has with it is kept, and one low on the tire is made a
//! ramp, which the tire rides up while pushing the object down and on. Left to the sweep,
//! the contact under the tread was dropped here, and a cone ended up inside the tire.
//! Heavy loose scenery, such as a parked car, is climbed as fixed scenery is.
//!
//! A wall a tire is driven into -- a face too steep for the suspension to roll up, such as
//! a ledge, the side of a parked car, or the side of another truck above where a tire
//! rides up it -- it climbs by its grip, as a monster truck's
//! front tires walk up a car (`climb`). The contact is given the tire's grip, and told
//! that the tire's tread is turning up the face, so that friction carries the tire up it
//! as hard as the truck pushes it in. Only under throttle, and less as the nose points up,
//! so that a truck tries a building and falls back rather than going over on its back.
//!
//! `physics::GamePhysicsPlugin` runs `decide_tire_contacts`. An app that sets up the
//! physics without it keeps every contact, and its trucks bump into each other rather
//! than climb.
//!
//! The figures above were measured with Rapier (see `docs/avian.md`).

use avian3d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::tasks::{ComputeTaskPool, TaskPool};

use super::drive::{AXLE_ALONG_X, LIGHT_LOOSE, SIDEWALL, UPWARD, below_the_axle};
use super::{Truck, TruckConfig, TruckInput, WheelCollider, WheelCore};
use crate::collision_groups::GROUND;

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
    cores: Query<'w, 's, &'static ColliderOf, With<WheelCore>>,
    /// The body each collider is part of, and what kind of body that is.
    bodies: Query<'w, 's, &'static ColliderOf>,
    kinds: Query<'w, 's, (&'static RigidBody, &'static ComputedMass)>,
    /// What each truck's driver asks of it, and its tires' grip, for `climb`.
    drivers: Query<'w, 's, (&'static TruckInput, &'static TruckConfig), With<Truck>>,
}

/// How steeply a tire rides up another truck that it is driven into, in radians above
/// level: the push that holds it back is turned up at least this far. Steeper lifts it
/// sooner and takes less of its speed, and pushes the other truck down harder.
const RAMP: f32 = 35.0 * std::f32::consts::PI / 180.0;

/// How high above its own axle a tire may meet another truck and still ride up it, as a
/// fraction of the tire's radius. Higher, the other truck is a wall taller than the tire,
/// which no tire climbs.
const RAMP_REACH: f32 = 0.3;



/// How fast a tire driven into a wall walks up it under full throttle, in m/s. It is also
/// the most a hit on a wall can throw the tire up by, since friction only carries the tire
/// up to this speed. Higher climbs quicker, and pops up more off a wall met at speed.
/// A first guess, which wants driving.
const CLIMB_SPEED: f32 = 3.0;

/// How squarely a face must stand against the way a tire is driven for the tire to climb
/// it, as the cosine of the angle between them: 0.5 is 60 degrees. Lower climbs walls met
/// more at a glance, and scrapes along fewer.
const WALL_FACING: f32 = 0.5;

/// How far up the way the truck is driven may point for a tire to climb at full strength,
/// and where the climb has faded to nothing, in radians. Between them it fades evenly.
/// Higher lets a truck go up taller walls, and go over on its back from one sooner.
const CLIMB_PITCH_FULL: f32 = 35.0 * std::f32::consts::PI / 180.0;
const CLIMB_PITCH_NONE: f32 = 55.0 * std::f32::consts::PI / 180.0;

impl TireContacts<'_, '_> {
    /// Whether a wheel collider that is `pushed` this way, in the world, at these points
    /// of its own cylinder (whose axis is Y), would rather be carried by its suspension.
    ///
    /// Against another truck or light loose scenery, never: the cast does not see either
    /// (see the module's notes).
    /// Against anything else but the ground -- a wall, a rail, a rock -- it is the tread
    /// pushed up the truck, and nothing else: the cast climbs that, and the rest should
    /// bump.
    ///
    /// Against the ground it is everything the springs can still answer for, which is
    /// nearly all of it, because the cast already covers the whole tire, sidewalls
    /// included. Two cases are not theirs. A tire pressed past the top of its travel has
    /// shut its springs, and from there the tire is simply solid: the contact holds it, so
    /// a truck that lands heavily stops on its tires and bounces off them, instead of
    /// sinking into the ground and being thrown back out by a spring. And a tire pressed
    /// on the ground above its own axle is a truck tipped over lying on the tops of its
    /// wheels, where the suspension has nothing to push against at all (see
    /// `drive::UPWARD`).
    fn suspension_takes(&self, wheel: Entity, other: Entity, pushed: Vec3, at: &[Vec3]) -> bool {
        let Ok((collider, wheel_at, child_of, wheel_collider)) = self.wheels.get(wheel) else {
            return false;
        };
        let (Ok((_, rotation, ..)), Some(cylinder)) =
            (self.trucks.get(child_of.0), collider.shape().as_cylinder())
        else {
            return false;
        };
        let upward = pushed.dot(rotation.0 * Vec3::Y) > UPWARD;
        // `drive` keeps the collider turned by the steering and onto its axle; the cast is
        // given the steering alone, so take the axle back off.
        let steer = wheel_at.rotation * AXLE_ALONG_X.inverse();
        let on_the_tread = |point: Vec3| point.xz().length() / cylinder.radius > SIDEWALL;
        if self.is_ground(other) {
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
            && upward
            && at.iter().copied().any(on_the_tread)
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
    /// driven into the other the faster is the one that rides up.
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
                let (_, _, velocity, ..) = self.trucks.get(truck).ok()?;
                let driven_in = velocity.0.dot(-pushed);
                low_enough.then_some((wheel, pushed, driven_in))
            })
            .max_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(wheel, pushed, _)| (wheel, pushed))
    }

    /// Make a wheel collider that is driven into a face it cannot roll onto walk up it by
    /// its grip (see the module's notes), and say whether it was. `pushed` is the way the
    /// face pushes the wheel, in the world, `at` where it touches the wheel, in the wheel's
    /// own frame, and `wheel_first` whether the wheel is the manifold's first collider.
    ///
    /// Not against light loose scenery, which a tire rides up instead (`riding_up`):
    /// gripped, a 146 kg cone was pulled down under the tire and the truck thrown up off it
    /// at 4 m/s. Against another truck only with `trucks_too`, which is asked once
    /// `riding_up` has not taken the contact: what is too high on the tire to ride up, such
    /// as the side of its body, a tire climbs. Nor with the side of the tire: a sidewall
    /// against a bank has nothing to climb with.
    #[allow(clippy::too_many_arguments)]
    fn climb(
        &self,
        wheel: Entity,
        other: Entity,
        pushed: Vec3,
        at: &[Vec3],
        wheel_first: bool,
        trucks_too: bool,
        manifold: &mut ContactManifold,
    ) -> bool {
        let Ok((collider, wheel_at, child_of, _)) = self.wheels.get(wheel) else {
            return false;
        };
        let Some(cylinder) = collider.shape().as_cylinder() else {
            return false;
        };
        let truck = child_of.0;
        let (Ok((_, rotation, ..)), Ok((input, config))) =
            (self.trucks.get(truck), self.drivers.get(truck))
        else {
            return false;
        };
        let on_the_tread = |point: &Vec3| point.xz().length() / cylinder.radius > SIDEWALL;
        let other_truck = self.truck_owning(other);
        let rides_up = self.is_loose(other)
            || (other_truck.is_some() && !trucks_too)
            || other_truck == Some(truck);
        if rides_up || input.throttle == 0.0 || !at.iter().any(on_the_tread) {
            return false;
        }
        // The way the tire is driven: where it points, or behind it in reverse.
        let steer = wheel_at.rotation * AXLE_ALONG_X.inverse();
        let driven = rotation.0 * steer * Vec3::NEG_Z * input.throttle.signum();
        // Only a leading wheel fades as the nose comes up: it is what lifts the nose further.
        // A trailing one lowers it again by climbing, and it must, for the truck to get its
        // back wheels up after its front ones.
        let leading = wheel_at.translation.z * input.throttle.signum() < 0.0;
        let pitch = driven.y.clamp(-1.0, 1.0).asin();
        let fade = if leading {
            ((CLIMB_PITCH_NONE - pitch) / (CLIMB_PITCH_NONE - CLIMB_PITCH_FULL)).clamp(0.0, 1.0)
        } else {
            1.0
        };
        // Up the face, as the world has it.
        let up_the_face = Vec3::Y.reject_from_normalized(pushed).normalize_or_zero();
        // Only a wall: a face within `UPWARD` of upright, as the world stands. One that
        // holds the tire up at all is a slope, which the suspension rolls up, and gripped
        // here it was a kerb that stopped the truck (see `stand_it_up`). Nor the underside
        // of something, which pushes the tire down.
        let wall = pushed.y.abs() <= UPWARD;
        if pushed.dot(-driven) < WALL_FACING || !wall || fade == 0.0 || up_the_face == Vec3::ZERO {
            return false;
        }
        let Some(point) = mean_point(manifold) else {
            return false;
        };
        let Some(moving) = self.truck_moving_at(wheel, point) else {
            return false;
        };
        // Against the face, which is another truck's going its own way.
        let face_moving = other_truck
            .and_then(|other| self.moving_at(other, point))
            .unwrap_or(Vec3::ZERO);
        // Up at the climbing speed, and along the face as the tire already goes, so that it
        // does not scrub.
        let along = (moving - face_moving)
            .reject_from_normalized(pushed)
            .reject_from_normalized(up_the_face);
        let sliding = up_the_face * CLIMB_SPEED * input.throttle.abs() * fade + along;
        manifold.tangent_velocity = if wheel_first { sliding } else { -sliding };
        manifold.friction = config.grip;
        // A tire that climbs is not thrown back off the face.
        manifold.restitution = 0.0;
        true
    }

    /// Let a wheel collider standing on top of another truck drive on it under throttle, as
    /// it would on the ground. `pushed` is the way the other truck holds it up, in the
    /// world, and `wheel_first` whether the wheel is the manifold's first collider.
    ///
    /// The suspension does not see another truck (see `drive`), so nothing else drives a
    /// tire on one, and a contact that a tire rides up is frictionless. Without this a
    /// truck driven broadside into another got its front wheels up onto it and stuck there
    /// at 38 degrees, its back wheels unable to push it up alone. The tire is carried
    /// forward at no less than `CLIMB_SPEED`, and as fast as it already goes if that is
    /// faster, so that the grip never brakes it; sideways it slides as it already does.
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
        let sliding = sliding + driven * (wanted - going);
        manifold.tangent_velocity = if wheel_first { sliding } else { -sliding };
        manifold.friction = config.grip;
    }

    /// The truck a wheel's collider or core is part of.
    fn truck_of(&self, wheel: Entity) -> Option<Entity> {
        self.wheels
            .get(wheel)
            .map(|(_, _, child_of, _)| child_of.0)
            .or_else(|_| self.cores.get(wheel).map(|of| of.body))
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

    /// Whether `ground` is the ground and `body` a truck's body that it is pushing this
    /// way, in the world, downward: the underside of the ground, which isn't there.
    fn ground_holds_down(&self, ground: Entity, body: Entity, pushed: Vec3) -> bool {
        self.trucks.contains(body) && self.is_ground(ground) && pushed.y < 0.0
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
            || self.ground_holds_down(first, second, normal)
            || self.ground_holds_down(second, first, -normal)
        {
            return false;
        }
        // A tire driven into a wall climbs it under throttle (see the module's notes).
        if self.climb(first, second, -normal, &on_first, true, false, manifold)
            || self.climb(second, first, normal, &on_second, false, false, manifold)
        {
            return true;
        }
        // A tire driven into another truck or light loose scenery low down rides up it (see
        // the module's notes).
        if let Some((rider, pushed)) = self.riding_up([
            (first, second, -normal, &on_first),
            (second, first, normal, &on_second),
        ]) {
            // Up as the world stands, not as the truck does: a truck with its nose up on
            // another, its back tires against the other's, took their level push for one
            // already turned up, and could not get them up after its front ones.
            let up = Vec3::Y;
            let level = pushed.reject_from_normalized(up).normalize_or_zero();
            let on_top = pushed.dot(up) >= RAMP.sin();
            if !on_top && level != Vec3::ZERO {
                let ramp = level * RAMP.cos() + up * RAMP.sin();
                manifold.normal = if rider == first { -ramp } else { ramp };
            }
            manifold.friction = 0.0;
            manifold.restitution = 0.0;
            let other = if rider == first { second } else { first };
            if on_top {
                self.drive_on_top(rider, other, pushed, rider == first, manifold);
            }
            return true;
        }
        // And one driven into another truck too high on the tire to ride up climbs it, as a
        // wall (see the module's notes).
        if self.climb(first, second, -normal, &on_first, true, true, manifold)
            || self.climb(second, first, normal, &on_second, false, true, manifold)
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
        if self.bottomed(first) && self.is_ground(second) {
            self.stand_it_up(first, true, manifold);
        } else if self.bottomed(second) && self.is_ground(first) {
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
