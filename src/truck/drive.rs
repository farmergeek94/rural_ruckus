//! Suspension and tire forces, computed once per physics step.
//!
//! Each wheel is a tire-shaped cylinder swept down from the top of its travel until it
//! touches something: the ground, or whatever is standing on it. A ray would do for level
//! ground, but only a shape rides onto a kerb or a log before it is directly over it, and
//! only a shape that starts inside something (a landing on scenery) knows that it has.
//! Between steps the wheel is also a collider on the body, kept at the hub, so that walls,
//! rails and other trucks meet a tire and not just a bumper, and so that a tire is
//! something another truck's tire can roll up onto (see `spawn`).
//!
//! The sweep does not see other trucks. A tire meets another truck only as that collider,
//! a solid, which rides up it (see `contacts`). Swept onto, another truck's round, moving
//! tire made a poor ground: straight down onto its steep front the sweep found the edge
//! tenths of a metre higher or lower for every centimetre the trucks moved, and the springs
//! followed. Measured, a truck driven into another's rear tire 0.8 m off its line: its
//! front hubs jumped by up to 0.8 m between steps and turned back 27 times in 5 s, with
//! jolts of 15 g. Met as a solid, they moved by 0.04 m at most, and turned back 6 times.
//!
//! What the sweep does find need not stand still. A tire grips against the way whatever it
//! stands on is going, if that is a body that moves, so a truck can ride on something that
//! moves. And whatever it stands on is pushed back as hard as it holds the tire up and
//! grips it, if it is a body that can be pushed. The ground and fixed scenery cannot.
//!
//! The forces are handed to the physics through `Forces`, which forgets them after each
//! step, so each step's are worked out afresh.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::tasks::{ComputeTaskPool, TaskPool};

use super::{
    GroundGrip, Held, Truck, TruckConfig, TruckInput, TruckWheelColliders, TruckWheels, Wheel,
    WheelCollider, WheelCore,
};

/// How far above its mount the tire is swept from, in metres, so that a tire pressed up
/// past its mount on a hard landing is found that far in and not merely touching: a cast
/// that begins inside something stops at once and says nothing of how deep. Anything found
/// past the mount means the springs are shut and the tire is solid (see `WheelCollider`).
/// It also keeps the sweep out of the wheel arch, where a rail or another truck's body
/// would be taken for ground under the tire.
const BUMP_STOP_REACH: f32 = 0.1;

/// How far below its mount the hub may still be and count as having shut its springs, in
/// metres. The sweep reports how far below the mount the tire touches, and the travel is
/// `suspension_bump` plus `suspension_droop`: 0 is the springs exactly shut, and this is the last
/// couple of centimetres before that, where they have almost nothing left to give and
/// handing the wheel back and forth between them and the collider only chatters.
///
/// It wants to stay small. Further out the collider takes over while the suspension is
/// still working, and a rigid contact is not a spring: measured on the built-in track
/// under power, at 0.2 -- half the travel -- the truck jolted half again as often, went a
/// third less far, and at 22 m/s was stopped outright by a rise. It buys nothing for it
/// either; a tire sinks about as far into the ground at 0.05 as at 0.2, and on the run it
/// was stopped on, further.
const SPRINGS_SHUT: f32 = 0.0;

/// Where on the tire something touches it: how far out from the axle, as a fraction of
/// the radius. On the tread it is 1, and so is the corner of the rim, however the ground
/// slopes under it, so a tire on a cambered road or rolling up onto a kerb or another
/// truck's tire carries fully. Only a touch on the flat sidewall is less: inside
/// `SIDEWALL` of the radius it is a bank the side of the tire has brushed, which carries
/// nothing, and from there to the rim the load fades in. The wheel collider still meets a
/// bank (see `spawn`), softly, as a contact.
pub(super) const SIDEWALL: f32 = 0.9;

/// How much of the tires' grip full lock may ask for at speed, as a fraction. The steering
/// lock shrinks with speed (`holdable_lock`) so that a held key never asks the tires for a
/// turn they can't make. At 1 the truck turns as hard as its tires allow and no harder, so
/// it never slides on the steering alone; above 1, full lock at speed slides a little and
/// scrubs off speed. Measured at 20 m/s on flat ground in the built-in truck: with the
/// lock unlimited the body slid at 7 to 10 degrees and lost 4 m/s in 2.5 s; at 1 here it
/// slid under a degree, cornered at the same 1 g, and kept its speed.
const LOCK_GRIP: f32 = 4.0;

/// For `holdable_lock`: what the tires' grip is measured against, in m/s².
const GRAVITY: f32 = 9.81;

/// How much of a surface's push must be along the truck's up for the suspension to carry
/// it, as the cosine of the angle between the two. 0.1 is 84° from up; below it the
/// surface is nearly a wall, and at or under zero it is on the wrong side of the wheel
/// altogether. Lower climbs taller edges: the front of a tire meets an edge the more
/// sideways the higher it is, and at 0.1 one of nine tenths of the tire's radius is
/// climbed by the suspension instead of stopping the truck. Measured with the built-in
/// truck driving at a step and at a log: at 0.2, 0.9 m and 1.1 m stopped it dead at 4
/// and 8 m/s; at 0.1 it rode over every height from 0.3 to 1.1 m at 4, 8 and 14 m/s. A truck tipped over meets the ground with the tops of its tires, where the
/// sweep -- which starts `BUMP_STOP_REACH` in and stops at penetration -- reads a tire
/// pressed fully past its bump stop and would answer with 100 kN a wheel square out of the
/// ground: measured, 416 kN on a two-tonne truck, twenty-one g, and it was thrown off.
/// Such a touch is the wheel collider's instead (see `contacts`).
pub(super) const UPWARD: f32 = 0.1;

/// Whether a tire is touched below its axle, at this point of its own cylinder (whose axis
/// is Y) with the wheel steered this way: the half of it that can stand on something. Only
/// `contacts` asks. The cast does not, because a tire rides onto a kerb or a log by its
/// front, which on a tall one is above the axle, and the suspension must still carry it.
pub(super) fn below_the_axle(steer: Quat, touched_at: Vec3) -> bool {
    (steer * AXLE_ALONG_X * touched_at).y < 0.0
}

/// How long a wheel in the air takes to spin up to the speed the throttle asks for, in
/// seconds: the time to close all but a third of the gap. Longer revs up more lazily.
const AIR_SPIN_UP_TIME: f32 = 0.5;
/// The same for a wheel in the air left to itself, running down to a stop. Longer keeps it
/// spinning for longer after the throttle is let go.
const AIR_COAST_TIME: f32 = 2.0;
/// The same for a wheel in the air held by the brakes or the handbrake.
const AIR_BRAKE_TIME: f32 = 0.15;

/// The speed at which the rear wheels' steering has faded to `REAR_STEER_FLOOR`, in m/s.
/// Their counter-steer is at its full `rear_steer_ratio` at a standstill and fades here,
/// much as a real four-wheel
/// steer does: at low speed it tightens the turn, and at speed it only swings the tail out.
/// Measured on flat ground with the built-in truck, throttle held, full lock swapped from
/// side to side every second from 15 m/s: with the rear steering at all speeds the rear
/// tires slid 20 degrees on average and up to 61, and the truck went sideways by up to 50;
/// fading out by 20 m/s, 2.7 and 7.3, and 6.6. Full lock with the throttle held from 10
/// m/s: up to 45 and 35, now 13 and 9. Turning from a standstill it yaws at 60 deg/s where
/// it did at 66. Fading by 15 m/s turned tighter at speed still; by 30, the tail still
/// came out at 10 m/s. Those figures were measured with the steering fading to nothing,
/// before `REAR_STEER_FLOOR`.
const REAR_STEER_FADE_SPEED: f32 = 20.0;
/// How much of its counter-steer the rear axle keeps at `REAR_STEER_FADE_SPEED` and above,
/// from 0 to 1. Higher turns tighter at speed, and swings the tail out more.
const REAR_STEER_FLOOR: f32 = 0.4;

/// How much of its counter-steer the rear axle has at this speed, from 1 to
/// `REAR_STEER_FLOOR`.
fn rear_steer_share(speed: f32) -> f32 {
    (1.0 - speed.abs() / REAR_STEER_FADE_SPEED).clamp(REAR_STEER_FLOOR, 1.0)
}

/// The speed along a tire under which a held truck's brakes ease off, in m/s, so that they
/// stop it without rocking it back and forth. Lower holds it more stiffly.
const HOLD_SPEED: f32 = 0.5;

/// What the brakes of a held truck (`Held`) push a tire with, in newtons, when it goes
/// along at `forward_speed`: all of `brake_force` against it, easing off near a standstill
/// so that it holds still on a slope instead of stopping and starting.
fn hold(forward_speed: f32, brake_force: f32) -> f32 {
    -(forward_speed / HOLD_SPEED).clamp(-1.0, 1.0) * brake_force
}

/// How much of full lock the steering must be at for the tip guard to act, from 0 to 1.
/// Lower lets it step in on gentler turns.
const SHARP_TURN: f32 = 0.6;
/// The slowest the truck may be going for the tip guard to act, in m/s.
const TIP_GUARD_SPEED: f32 = 5.0;
/// How long the tip guard takes to come on once an inside wheel lifts, and to go off once
/// the wheels are down again, in seconds: the time to close all but a third of the gap.
const TIP_GUARD_ON_TIME: f32 = 0.05;
const TIP_GUARD_OFF_TIME: f32 = 0.3;
/// At full tip guard, how much of the steering is taken off, from 0 to 1. Higher unwinds
/// the turn further, and runs wider.
const TIP_GUARD_STEER: f32 = 0.6;
/// At full tip guard, how much of `brake_force` each tire brakes with, from 0 to 1, in place
/// of the throttle.
const TIP_GUARD_BRAKE: f32 = 0.6;

/// A truck's tip guard: how hard it is at present easing the steering and braking because an
/// inside wheel has lifted in a sharp turn, from 0 (not at all) to 1. See
/// `lifting_in_a_sharp_turn`. Only the computer's trucks have one: the player keeps their
/// truck on its wheels themselves, and the steering and brakes are theirs alone.
#[derive(Component, Default)]
pub(super) struct TipGuard(f32);

/// A physics cylinder stands along Y; a tire's axle lies along X.
pub(super) const AXLE_ALONG_X: Quat = Quat::from_xyzw(
    0.0,
    0.0,
    std::f32::consts::FRAC_1_SQRT_2,
    std::f32::consts::FRAC_1_SQRT_2,
);

/// The three queries write `Transform`s of three kinds of entity, which must not overlap.
type OnlyWheels = (Without<Truck>, Without<WheelCollider>, Without<WheelCore>);
type OnlyWheelColliders = (With<WheelCollider>, Without<Truck>);
/// What `drive_truck` reads and writes of each wheel collider: where it is, whether its
/// springs are shut, and its shape, which is the tire's and what the sweep sweeps.
type WheelColliderParts = (
    &'static mut Transform,
    &'static mut WheelCollider,
    &'static Collider,
);
type OnlyWheelCores = (With<WheelCore>, Without<Truck>, Without<WheelCollider>);
/// Any part of a truck the sweep could find: its body, a wheel collider or a core.
type TruckParts = Or<(With<Truck>, With<WheelCollider>, With<WheelCore>)>;
/// What `drive_truck` reads and writes of each truck.
type DrivenTruck = (
    Entity,
    &'static Transform,
    Forces,
    &'static TruckConfig,
    &'static TruckInput,
    &'static TruckWheels,
    &'static TruckWheelColliders,
    Option<&'static mut TipGuard>,
    Option<&'static GroundGrip>,
    Has<Held>,
);

/// Where one tire is swept from and how, worked out for every tire before any is swept, so
/// that the sweeps can all be made at once (see `sweep_all`).
struct Cast {
    /// The tire's shape: its wheel collider's.
    tire: Collider,
    /// Where it starts, in the world.
    from: Vec3,
    /// How it is turned: steered, leaning with its axle, and onto its axle.
    turned: Quat,
    /// Down, in the truck's axes.
    down: Dir3,
    /// How far it goes, in metres.
    reach: f32,
}

/// Makes every tire's sweep, on as many threads as the machine has. The sweeps only read
/// the world, and none depends on another. One after another they were two thirds of this
/// system's time: measured on Scrapyard Run with eight trucks, unoptimised, 32 sweeps took
/// 0.65 to 0.77 ms a step. At once, on 16 threads, they take 0.28 to 0.31 ms, whether a task
/// is given 1, 2 or 4 of them; given 8, 0.35 ms. Each sweep sees only what `stands_on`
/// lets it.
fn sweep_all(
    spatial: &SpatialQuery,
    casts: &[Cast],
    stands_on: &(dyn Fn(Entity) -> bool + Sync),
) -> Vec<Option<ShapeHitData>> {
    let sweep = |cast: &Cast| {
        spatial.cast_shape_predicate(
            &cast.tire,
            cast.from,
            cast.turned,
            cast.down,
            &ShapeCastConfig {
                max_distance: cast.reach,
                target_distance: 0.0,
                // A cast that starts inside something stops there, and says so.
                compute_contact_on_penetration: true,
                ignore_origin_penetration: false,
            },
            &SpatialQueryFilter::default(),
            stands_on,
        )
    };
    // A few to each task: a task costs more to hand out than a sweep does to make.
    const PER_TASK: usize = 4;
    ComputeTaskPool::get_or_init(TaskPool::default)
        .scope(|scope| {
            for chunk in casts.chunks(PER_TASK) {
                scope.spawn(async move { chunk.iter().map(sweep).collect::<Vec<_>>() });
            }
        })
        .into_iter()
        .flatten()
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn drive_truck(
    time: Res<Time>,
    spatial: SpatialQuery,
    mut trucks: Query<DrivenTruck, With<Truck>>,
    mut wheels: Query<(&mut Wheel, &mut Transform), OnlyWheels>,
    mut wheel_colliders: Query<WheelColliderParts, OnlyWheelColliders>,
    mut wheel_cores: Query<&mut Transform, OnlyWheelCores>,
    truck_parts: Query<(), TruckParts>,
    // Whatever a tire stands on that is a body, and the body a collider is part of.
    mut surfaces: Query<(Forces, &ComputedMass), Without<Truck>>,
    bodies: Query<&ColliderOf>,
) {
    let dt = time.delta_secs();
    // Not this truck, nor any other (see the module's notes).
    let not_a_truck = |collider: Entity| !truck_parts.contains(collider);

    // First where every tire of every truck is swept from, then all the sweeps at once. For
    // each truck, for each of its wheels, which of `casts` is its, and how it is steered.
    let mut casts = Vec::new();
    let mut planned: Vec<Vec<Option<(usize, Quat)>>> = Vec::new();
    for (_, transform, forces, config, input, children, colliders, guard, ..) in &mut trucks {
        let up = transform.up().as_vec3();
        // Full lock is what the tires can hold at this speed.
        let body_speed = forces.linear_velocity().dot(transform.forward().as_vec3());
        // The tip guard takes some of the steering off, as it stood after the last step. A
        // truck without one is never guarded.
        let guarding = guard.as_ref().map_or(0.0, |guard| guard.0);
        let lock = holdable_lock(config, body_speed) * (1.0 - TIP_GUARD_STEER * guarding);
        let travel = config.suspension_bump + config.suspension_droop;
        let wheels_of_truck = children
            .0
            .iter()
            .zip(&colliders.0)
            .map(|(&child, &collider)| {
                let (wheel, _) = wheels.get(child).ok()?;
                let (_, _, tire) = wheel_colliders.get(collider).ok()?;
                let steer_angle = input.steer
                    * lock
                    * if wheel.front {
                        1.0
                    } else {
                        -config.rear_steer_ratio * rear_steer_share(body_speed)
                    };
                let steer_rotation = Quat::from_rotation_y(steer_angle);
                // Turned as the wheel is steered, and leaning as its axle leans.
                let tire_rotation = Quat::from_rotation_z(wheel.tilt) * steer_rotation;
                // The mount is the top of the travel, where the hub meets the bump stop.
                // The tire is swept down from a little higher, so that a tire pressed up
                // past its mount, on a hard landing, is found that far in and not merely
                // touching: a cast that begins inside something stops at once and says
                // nothing of how deep.
                let mount = transform.transform_point(wheel.mount);
                casts.push(Cast {
                    tire: tire.clone(),
                    from: mount + up * BUMP_STOP_REACH,
                    turned: transform.rotation * tire_rotation * AXLE_ALONG_X,
                    down: Dir3::new(-up).unwrap_or(Dir3::NEG_Y),
                    reach: travel + BUMP_STOP_REACH,
                });
                Some((casts.len() - 1, steer_rotation))
            })
            .collect();
        planned.push(wheels_of_truck);
    }
    let hits = sweep_all(&spatial, &casts, &not_a_truck);

    for (
        (
            _truck,
            transform,
            mut forces,
            config,
            input,
            children,
            colliders,
            mut guard,
            ground_grip,
            held,
        ),
        planned,
    ) in trucks.iter_mut().zip(&planned)
    {
        let up = transform.up().as_vec3();
        let body_speed = forces.linear_velocity().dot(transform.forward().as_vec3());

        // Every tire has been swept, to find where its hub is. Now each axle's springs are
        // worked out from where both of its hubs are, since they sit on one beam.
        let travel = config.suspension_bump + config.suspension_droop;
        let mut sweeps = Vec::with_capacity(children.0.len());
        // Both in `TruckConfig::wheel_rest`'s order, as `GroundGrip` is.
        for (index, ((&child, &collider), plan)) in
            children.0.iter().zip(&colliders.0).zip(planned).enumerate()
        {
            let (Some((cast, steer_rotation)), Ok((wheel, _))) = (*plan, wheels.get(child)) else {
                continue;
            };
            let mount = transform.transform_point(wheel.mount);
            let lift = BUMP_STOP_REACH;
            let (from, tire_turned) = (casts[cast].from, casts[cast].turned);
            let touch = hits[cast].and_then(|hit| {
                // `normal1` is the outward normal of what was hit, in the world, and
                // `point2` the touch on the tire, in the world too: taken into the
                // cylinder's own frame, whose axis is Y, from where the tire touched.
                let normal = hit.normal1.try_normalize().unwrap_or(up);
                let touched_at = from - up * hit.distance;
                let on_the_tire = tire_turned.inverse() * (hit.point2 - touched_at);
                let out_from_axle = on_the_tire.xz().length() / config.wheel_radius;
                let on_the_tread = ((out_from_axle - SIDEWALL) / (1.0 - SIDEWALL)).clamp(0.0, 1.0);
                // Brushed by the sidewall, and not stood on: the wheel hangs. So does
                // a wheel whose tread meets a face that is not holding it up at all --
                // the ground against the top of a tire on a truck tipped over, which
                // the wheel collider takes as a rigid contact instead (see `contacts`).
                let underneath = normal.dot(up) > UPWARD;
                (on_the_tread > 0.0 && underneath).then_some(Touch {
                    below_mount: hit.distance - lift,
                    normal,
                    on_the_tread,
                    surface: bodies.get(hit.entity).map_or(hit.entity, |of| of.body),
                })
            });

            // Suspension length: from the mount down to the hub. With nothing under it the
            // wheel hangs at the bottom of its travel. Above the mount it can't go: the hub
            // stays at its upper limit and the rest is taken by the bump stop.
            let length = touch.map_or(travel, |touch| touch.below_mount.max(0.0));
            let hub = mount - up * length;
            // Against what it stands on, which is only the ground's own when that is still.
            let surface_velocity = touch
                .and_then(|touch| surfaces.get(touch.surface).ok())
                .map_or(Vec3::ZERO, |(surface, _)| surface.velocity_at_point(hub));
            let hub_velocity = forces.velocity_at_point(hub) - surface_velocity;
            sweeps.push(Sweep {
                index,
                wheel: child,
                collider,
                mount: wheel.mount,
                steer_rotation,
                length,
                hub,
                hub_velocity,
                touch,
            });
        }

        // An inside wheel off the ground in a sharp turn: ease the steering and brake.
        let tipping = lifting_in_a_sharp_turn(
            input.steer,
            body_speed,
            forces.angular_velocity().dot(up),
            sweeps
                .iter()
                .map(|sweep| (sweep.mount.x, sweep.touch.is_some())),
        );
        let (target, time) = if tipping {
            (1.0, TIP_GUARD_ON_TIME)
        } else {
            (0.0, TIP_GUARD_OFF_TIME)
        };
        if let Some(guard) = guard.as_mut() {
            guard.0 = target + (guard.0 - target) * (-dt / time).exp();
        }
        // The brakes go by it as it stands after this step.
        let guarding = guard.as_ref().map_or(0.0, |guard| guard.0);

        // How far each hub has risen above its rest position, and how fast. A wheel that
        // hangs free has no speed of its own to speak of: its hub follows the body.
        let rise = |sweep: &Sweep| config.suspension_bump - sweep.length;
        let rise_rate = |sweep: &Sweep| match sweep.touch {
            Some(_) => -sweep.hub_velocity.dot(up),
            None => 0.0,
        };
        // The wheels come an axle at a time, either side first.
        let mut springs = Vec::with_capacity(sweeps.len());
        let mut tilts = Vec::with_capacity(sweeps.len());
        for axle in sweeps.chunks(2) {
            match axle {
                [a, b] => {
                    springs.extend(beam_loads(
                        config,
                        [rise(a), rise(b)],
                        [rise_rate(a), rise_rate(b)],
                    ));
                    let tilt =
                        beam_tilt(a.mount - Vec3::Y * a.length, b.mount - Vec3::Y * b.length);
                    tilts.extend([tilt, tilt]);
                }
                _ => {
                    for sweep in axle {
                        springs
                            .push(beam_loads(config, [rise(sweep); 2], [rise_rate(sweep); 2])[0]);
                        tilts.push(0.0);
                    }
                }
            }
        }

        for ((sweep, spring), tilt) in sweeps.iter().zip(springs).zip(tilts) {
            let Ok((mut wheel, mut wheel_transform)) = wheels.get_mut(sweep.wheel) else {
                continue;
            };
            wheel.tilt = tilt;
            let steer_rotation = sweep.steer_rotation;
            let locked = input.handbrake && !wheel.front;
            // The ground's speed under the tread, while it touches.
            let mut rolling_speed = None;
            // Pressed past the top of its travel: the springs are shut and the tire is
            // solid from here on (see `WheelCollider`).
            let mut bottomed = false;

            if let Some(touch) = sweep.touch {
                bottomed = touch.below_mount < SPRINGS_SHUT;
                let hub = sweep.hub;
                let hub_velocity = sweep.hub_velocity;
                let compression_rate = rise_rate(sweep);

                // The bump stop, over the top of the travel, at the wheel itself: a stiff
                // spring, and a damper that works only as the wheel presses in. So it soaks
                // up most of a hit, and gives a little back as the wheel comes out, a small
                // upward push.
                let into_stop = rise(sweep) - (config.suspension_bump - config.bump_stop_travel);
                let bump_stop = if into_stop > 0.0 {
                    config.bump_stop_spring * into_stop
                        + config.bump_stop_damper * compression_rate.max(0.0)
                } else {
                    0.0
                };
                let load = (spring + bump_stop).max(0.0) * touch.on_the_tread;
                let normal = touch.normal;

                // Tire axes, flattened onto the ground plane.
                let heading = transform.rotation * steer_rotation * Vec3::NEG_Z;
                let forward = heading.reject_from_normalized(normal).normalize_or_zero();
                let side = forward.cross(normal);
                let forward_speed = hub_velocity.dot(forward);
                let side_speed = hub_velocity.dot(side);
                rolling_speed = Some(if locked { 0.0 } else { forward_speed });

                let braking = input.throttle * forward_speed < 0.0 && forward_speed.abs() > 0.5;
                let asked = if held {
                    hold(forward_speed, config.brake_force)
                } else if locked {
                    -forward_speed.signum() * config.brake_force
                } else if braking {
                    input.throttle * config.brake_force
                } else {
                    let pull = (1.0 - forward_speed.abs() / config.top_speed).max(0.0);
                    input.throttle * config.engine_force * pull
                };
                // The tip guard brakes in place of what was asked for, as far as it is on.
                let guarded = if forward_speed.abs() > 0.5 {
                    -forward_speed.signum() * config.brake_force * TIP_GUARD_BRAKE
                } else {
                    0.0
                };
                let drive = asked + (guarded - asked) * guarding
                    - forward_speed * config.rolling_resistance;

                // Sideways grip: the force that would cancel this wheel's share of the
                // chassis' sideways momentum within a step, softened by `lateral_stiffness`.
                let cornering = -side_speed * config.lateral_stiffness * (config.mass / 4.0) / dt;

                // Friction circle: a tire only has so much grip to split between
                // accelerating and cornering, and it scales with the load on the wheel,
                // and with how well the ground under it grips.
                let ground = ground_grip.map_or(1.0, |ground| ground.0[sweep.index]);
                let grip = ground
                    * if locked {
                        config.handbrake_grip
                    } else {
                        config.grip
                    };
                let tire = (forward * drive + side * cornering).clamp_length_max(grip * load);

                // The load along the surface's normal, as the ground pushes: a bank met
                // sideways pushes the truck off it, not upwards. At the hub rather than
                // where the springs sit on the beam: the beam's loads already carry what
                // that does to the body. Drive and brakes at the hub too, and the sideways
                // grip `cornering_lever` of the way down to the ground.
                let sideways = side * tire.dot(side);
                forces.apply_force_at_point(normal * load + tire - sideways, hub);
                // And what it stands on takes the same back, where the tire meets it.
                if let Ok((mut surface, mass)) = surfaces.get_mut(touch.surface) {
                    let push =
                        pushed_back(-(normal * load + tire) * dt, mass.value(), hub_velocity, dt);
                    surface.apply_linear_impulse_at_point(push, hub - normal * config.wheel_radius);
                }
                forces.apply_force_at_point(
                    sideways,
                    hub - normal * config.wheel_radius * config.cornering_lever,
                );
            }

            let tire_rotation = Quat::from_rotation_z(tilt) * steer_rotation;
            wheel.tread_speed = rolling_speed.unwrap_or_else(|| {
                spin_in_the_air(
                    wheel.tread_speed,
                    if held { 0.0 } else { input.throttle },
                    locked || held,
                    config.top_speed,
                    dt,
                )
            });
            wheel.spin -= wheel.tread_speed / config.wheel_radius * dt;
            wheel_transform.translation = wheel.mount - Vec3::Y * sweep.length;
            // The wheel is a pivot whose axle lies along X. What is drawn hangs from it.
            wheel_transform.rotation = tire_rotation * Quat::from_rotation_x(wheel.spin);
            // And the collider goes with it. A cylinder needs no spin. It also carries
            // whether the springs are shut, which is what tells `contacts` to stop
            // dropping this wheel's ground contact and let the tire be solid.
            if let Ok((mut collider_transform, mut wheel_collider, _)) =
                wheel_colliders.get_mut(sweep.collider)
            {
                collider_transform.translation = wheel_transform.translation;
                collider_transform.rotation = tire_rotation * AXLE_ALONG_X;
                wheel_collider.bottomed = bottomed;
                // The core rides at the hub with it.
                if let Ok(mut core_transform) = wheel_cores.get_mut(wheel_collider.core) {
                    core_transform.translation = wheel_transform.translation;
                }
            }
        }
    }
}

/// The impulse with which a tire pushes back on a body it stands on, in N·s, from the
/// `impulse` that would take the whole of the tire's load and grip, for a body of `mass`
/// kilograms under a hub going at `hub_velocity` against it, over a step of `dt` seconds.
///
/// A tire cannot drive what it stands on away faster than it is itself going that way, and
/// what holds a light object up is the ground under it, not the tire. Given the whole of
/// the tire's load, a 1.4 kg cone under a wheel of a truck doing 20 m/s was pushed into the
/// ground at 77 m/s in a step, came back out of it as fast, and threw the truck into the air
/// at 51 m/s; a 45 kg crate met at 10 m/s was fired off at 1 194 m/s; and a 227 kg rock was
/// pressed down through the ground, stood there as a kerb, and flipped the truck. So the push
/// changes the body's speed by no more than the hub closes on it along the push, and what
/// gravity adds in the step. A tire rolling over the top of a body hardly closes on it at
/// all, and presses it down with about its own weight; a heavy body takes the whole push.
fn pushed_back(impulse: Vec3, mass: f32, hub_velocity: Vec3, dt: f32) -> Vec3 {
    let closing = hub_velocity.dot(impulse.normalize_or_zero()).max(0.0);
    impulse.clamp_length_max(mass * (closing + GRAVITY * dt))
}

/// Whether a truck is in a sharp turn with an inside wheel off the ground and an outside one
/// on it: about to tip over, and not simply in the air. `steer` is the steering asked for,
/// as a share of full lock either way, `speed` how fast the truck is going forwards in m/s,
/// and `yaw_rate` how fast it is turning about its own up, in rad/s, anticlockwise from
/// above positive, which is to the left. Each wheel is given as its mount's X in the
/// truck's axes (left negative) and whether it touches.
fn lifting_in_a_sharp_turn(
    steer: f32,
    speed: f32,
    yaw_rate: f32,
    wheels: impl Iterator<Item = (f32, bool)>,
) -> bool {
    if steer.abs() < SHARP_TURN || speed.abs() < TIP_GUARD_SPEED || yaw_rate == 0.0 {
        return false;
    }
    // The inside of the turn is the side the truck is turning towards.
    let inside = |x: f32| (x < 0.0) == (yaw_rate > 0.0);
    let (mut inside_lifted, mut outside_down) = (false, false);
    for (x, touching) in wheels {
        if inside(x) {
            inside_lifted |= !touching;
        } else {
            outside_down |= touching;
        }
    }
    inside_lifted && outside_down
}

/// How fast the tread of a wheel with nothing under it goes round a step on, in m/s, from
/// `speed`. The throttle spins it up towards the truck's top speed, either way; throttle
/// against its spin, the brakes, or the handbrake on a rear wheel stop it quickly; and
/// left alone it runs down slowly. Each draws the speed towards its target by the same
/// share of the gap in every second, whatever the step.
fn spin_in_the_air(speed: f32, throttle: f32, locked: bool, top_speed: f32, dt: f32) -> f32 {
    // As on the ground: throttle against the way it goes is the brakes, until it has all
    // but stopped.
    let braking = throttle * speed < 0.0 && speed.abs() > 0.5;
    let (target, time) = if locked || braking {
        (0.0, AIR_BRAKE_TIME)
    } else if throttle != 0.0 {
        (throttle * top_speed, AIR_SPIN_UP_TIME)
    } else {
        (0.0, AIR_COAST_TIME)
    };
    target + (speed - target) * (-dt / time).exp()
}

/// What a tire's sweep found under it.
#[derive(Clone, Copy)]
struct Touch {
    /// How far below the mount the hub is with the tire touching, in metres. Negative when
    /// the tire is pressed up past the top of its travel.
    below_mount: f32,
    /// The outward normal of what it touches, in the world.
    normal: Vec3,
    /// How much of the touch is on the tread rather than the sidewall, from 0 to 1.
    on_the_tread: f32,
    /// The collider it touches.
    surface: Entity,
}

/// One wheel's sweep, kept until its axle's springs are known.
struct Sweep {
    /// Which of `TruckConfig::wheel_rest` it is.
    index: usize,
    wheel: Entity,
    collider: Entity,
    /// The wheel's mount, in the truck's axes.
    mount: Vec3,
    steer_rotation: Quat,
    /// From the mount down to the hub, in metres.
    length: f32,
    /// In the world.
    hub: Vec3,
    hub_velocity: Vec3,
    touch: Option<Touch>,
}

/// The loads that an axle's springs and dampers put on its two tires, in newtons, from how
/// far each hub has risen above its rest position (negative below it) and how fast.
///
/// The axle is a solid beam, and its springs sit on it `spring_spread` of the way out from
/// its middle to each wheel. Where the beam is at each spring follows from where it is at
/// the wheels: the middle rise, plus that share of how much the two differ. So one wheel
/// rising squeezes both springs, its own the more. A beam of no mass is balanced between
/// what the springs push down on it and what the tires push up: together the tires carry
/// what the springs do, and the difference between them is the springs' difference, less
/// by the same share, since the springs work on a shorter lever. Each spring is squeezed
/// by `sag` at rest, where the four carry the truck, and pushes nothing once it has
/// stretched back to its full length.
fn beam_loads(config: &TruckConfig, rise: [f32; 2], rise_rate: [f32; 2]) -> [f32; 2] {
    let share = config.spring_spread;
    let middle = |pair: [f32; 2]| (pair[0] + pair[1]) / 2.0;
    let apart = |pair: [f32; 2]| (pair[0] - pair[1]) / 2.0;
    let at_spring = |side: f32| {
        let rise = middle(rise) + side * share * apart(rise);
        let rate = middle(rise_rate) + side * share * apart(rise_rate);
        let spring = config.spring * (config.sag() + rise).max(0.0);
        let damper = if rate > 0.0 {
            config.damper
        } else {
            config.rebound_damper
        };
        spring + damper * rate
    };
    let springs = [at_spring(1.0), at_spring(-1.0)];
    let (together, difference) = (middle(springs), apart(springs));
    [together + share * difference, together - share * difference]
}

/// How far a beam axle leans from level, in radians about the truck's Z axis, positive
/// with the right-hand hub higher: from its two hubs in the truck's axes, either first.
fn beam_tilt(a: Vec3, b: Vec3) -> f32 {
    let (left, right) = if a.x < b.x { (a, b) } else { (b, a) };
    let along = right - left;
    along.y.atan2(along.x)
}

/// The front steering lock the tires can hold at `speed` m/s, in radians: `max_steer` at
/// low speed, and less as speed rises. A truck turning on a radius R at speed v needs v²/R
/// sideways, the tires give about `grip` times its weight, and the radius the front and
/// rear steer angles make together is about the wheelbase over their sum. Full lock,
/// however fast, then asks the tires for `LOCK_GRIP` of what they have, and no more.
pub(super) fn holdable_lock(config: &TruckConfig, speed: f32) -> f32 {
    let along = config.wheel_rest.iter().map(|rest| rest.z);
    let wheelbase = along.clone().fold(f32::MIN, f32::max) - along.fold(f32::MAX, f32::min);
    // The radius that cornering at the tires' limit makes at this speed. Zero at rest,
    // where any lock is holdable.
    let radius = speed * speed / (LOCK_GRIP * config.grip * GRAVITY);
    let together = (wheelbase / radius).atan();
    (together / (1.0 + config.rear_steer_ratio)).min(config.max_steer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-2
    }

    #[test]
    fn the_tip_guard_acts_only_when_an_inside_wheel_lifts_in_a_sharp_turn() {
        const LEFT: f32 = -1.0;
        const RIGHT: f32 = 1.0;
        let lifting = |steer, speed, yaw_rate, wheels: &[(f32, bool)]| {
            lifting_in_a_sharp_turn(steer, speed, yaw_rate, wheels.iter().copied())
        };
        let left_lifted = [(LEFT, false), (RIGHT, true), (LEFT, true), (RIGHT, true)];
        // Turning left, the left wheels are on the inside.
        assert!(lifting(1.0, 15.0, 0.8, &left_lifted));
        // Turning right, a lifted left wheel is on the outside: nothing to correct.
        assert!(!lifting(-1.0, 15.0, -0.8, &left_lifted));
        // A gentle turn, a crawl, or all four down: left alone.
        assert!(!lifting(0.3, 15.0, 0.8, &left_lifted));
        assert!(!lifting(1.0, 2.0, 0.8, &left_lifted));
        let all_down = [(LEFT, true), (RIGHT, true), (LEFT, true), (RIGHT, true)];
        assert!(!lifting(1.0, 15.0, 0.8, &all_down));
        // All four in the air is a jump, not a tip.
        let airborne = [(LEFT, false), (RIGHT, false), (LEFT, false), (RIGHT, false)];
        assert!(!lifting(1.0, 15.0, 0.8, &airborne));
    }

    /// `spin_in_the_air`, held for `seconds` at 120 steps a second.
    fn spin_for(seconds: f32, speed: f32, throttle: f32, locked: bool) -> f32 {
        let dt = 1.0 / 120.0;
        (0..(seconds / dt).round() as usize).fold(speed, |speed, _| {
            spin_in_the_air(speed, throttle, locked, 20.0, dt)
        })
    }

    #[test]
    fn a_wheel_in_the_air_spins_up_under_throttle_either_way() {
        assert!(spin_for(2.0, 0.0, 1.0, false) > 19.0);
        assert!(spin_for(2.0, 0.0, -1.0, false) < -19.0);
        // Half throttle asks for half the speed.
        assert!(close(spin_for(5.0, 0.0, 0.5, false), 10.0));
    }

    #[test]
    fn a_wheel_in_the_air_left_alone_runs_down_slowly() {
        let after_a_second = spin_for(1.0, 20.0, 0.0, false);
        assert!(after_a_second > 10.0 && after_a_second < 20.0);
        assert!(spin_for(10.0, 20.0, 0.0, false).abs() < 0.2);
    }

    #[test]
    fn a_tire_pushes_a_light_body_no_faster_than_it_closes_on_it() {
        let dt = 1.0 / 120.0;
        let load = Vec3::NEG_Y * 13_000.0 * dt;
        // Rolling over a cone at 20 m/s, and coming down onto it at 2 m/s: the cone is
        // pressed down at 2 m/s and a step of gravity, not 77 m/s.
        let cone = pushed_back(load, 1.4, Vec3::new(0.0, -2.0, -20.0), dt);
        assert!((cone.length() / 1.4 - (2.0 + GRAVITY * dt)).abs() < 1e-3);
        assert_eq!(cone.normalize(), Vec3::NEG_Y);
        // Going away from it, with gravity's step alone.
        let cone = pushed_back(load, 1.4, Vec3::new(0.0, 1.0, -20.0), dt);
        assert!((cone.length() / 1.4 - GRAVITY * dt).abs() < 1e-3);
        // A heavy body takes the whole of it.
        let heavy = Vec3::new(0.0, -2.0, -20.0);
        assert_eq!(pushed_back(load, 20_000.0, heavy, dt), load);
    }

    #[test]
    fn a_held_truck_brakes_against_the_way_it_goes() {
        assert_eq!(hold(10.0, 100.0), -100.0);
        assert_eq!(hold(-10.0, 100.0), 100.0);
        assert_eq!(hold(0.0, 100.0), 0.0);
        assert_eq!(hold(HOLD_SPEED / 2.0, 100.0), -50.0);
    }

    #[test]
    fn the_rear_steering_fades_with_speed_down_to_its_floor() {
        assert_eq!(rear_steer_share(0.0), 1.0);
        assert_eq!(rear_steer_share(REAR_STEER_FADE_SPEED / 2.0), 0.5);
        assert_eq!(rear_steer_share(-REAR_STEER_FADE_SPEED / 2.0), 0.5);
        assert_eq!(rear_steer_share(REAR_STEER_FADE_SPEED), REAR_STEER_FLOOR);
        assert_eq!(
            rear_steer_share(REAR_STEER_FADE_SPEED * 3.0),
            REAR_STEER_FLOOR
        );
    }

    #[test]
    fn brakes_stop_a_wheel_in_the_air_quickly() {
        assert!(spin_for(0.5, 20.0, 0.0, true).abs() < 1.0);
        // Throttle against the spin brakes it first, then spins it the other way.
        assert!(spin_for(0.3, 20.0, -1.0, false).abs() < 3.0);
        assert!(spin_for(3.0, 20.0, -1.0, false) < -15.0);
    }

    #[test]
    fn at_rest_each_tire_carries_a_quarter_of_the_truck_whatever_the_springs() {
        for spring in [15_000.0, 23_000.0, 40_000.0] {
            let config = TruckConfig {
                spring,
                ..TruckConfig::default()
            };
            let quarter = config.mass * GRAVITY / 4.0;
            let [a, b] = beam_loads(&config, [0.0; 2], [0.0; 2]);
            assert!(close(a, quarter) && close(b, quarter), "{a} {b} {quarter}");
        }
    }

    #[test]
    fn one_wheel_rising_loads_the_other_through_the_beam() {
        let config = TruckConfig {
            spring_spread: 0.5,
            ..TruckConfig::default()
        };
        let (k, sag, rise) = (config.spring, config.sag(), 0.2);
        let [up, other] = beam_loads(&config, [rise, 0.0], [0.0; 2]);
        // Its own spring gets (1 + 0.25) / 2 of the rise and the other's (1 - 0.25) / 2.
        assert!(close(up, k * (sag + 0.625 * rise)), "{up}");
        assert!(close(other, k * (sag + 0.375 * rise)), "{other}");
        // The beam passes on all the springs push, and no more.
        let [first, second] = beam_loads(&config, [rise, -0.1], [0.3, -0.2]);
        let springs = |side: f32| {
            let middle_rise = (rise - 0.1) / 2.0 + side * 0.5 * (rise + 0.1) / 2.0;
            let middle_rate = (0.3 - 0.2) / 2.0 + side * 0.5 * (0.3 + 0.2) / 2.0;
            let damper = if middle_rate > 0.0 {
                config.damper
            } else {
                config.rebound_damper
            };
            k * (sag + middle_rise).max(0.0) + damper * middle_rate
        };
        assert!(close(first + second, springs(1.0) + springs(-1.0)));

        // Springs at the wheels leave the wheels independent.
        let independent = TruckConfig {
            spring_spread: 1.0,
            ..config
        };
        let [up, other] = beam_loads(&independent, [rise, 0.0], [0.0; 2]);
        assert!(close(up, k * (sag + rise)) && close(other, k * sag));
    }

    #[test]
    fn springs_stretched_to_their_length_push_nothing() {
        let config = TruckConfig::default();
        let hanging = -config.suspension_droop;
        assert!(config.sag() < config.suspension_droop);
        assert_eq!(beam_loads(&config, [hanging; 2], [0.0; 2]), [0.0; 2]);
    }

    #[test]
    fn a_beam_leans_towards_its_lower_hub() {
        let (left, right) = (Vec3::new(-1.5, -0.8, 1.0), Vec3::new(1.5, -0.5, 1.0));
        let tilt = beam_tilt(left, right);
        assert!(close(tilt, (0.3f32 / 3.0).atan()));
        // Either hub first.
        assert_eq!(beam_tilt(right, left), tilt);
        assert_eq!(beam_tilt(left, left + Vec3::X * 3.0), 0.0);
    }

    #[test]
    fn the_lock_is_full_at_low_speed_and_shrinks_with_speed() {
        let config = TruckConfig::default();
        assert_eq!(holdable_lock(&config, 0.0), config.max_steer);
        assert_eq!(holdable_lock(&config, 3.0), config.max_steer);
        assert_eq!(holdable_lock(&config, -3.0), config.max_steer);
        assert_eq!(holdable_lock(&config, 10.0), config.max_steer);
        let mut last = config.max_steer;
        for speed in [16.0, 20.0, 30.0, 45.0] {
            let lock = holdable_lock(&config, speed);
            assert!(lock < last, "{speed} m/s: {lock}");
            last = lock;
        }
        assert!(holdable_lock(&config, 35.0) < 0.1);
    }

    #[test]
    fn full_lock_at_speed_asks_the_tires_for_exactly_their_grip() {
        let config = TruckConfig::default();
        let wheelbase = 3.5;
        // Fast enough that the lock is below `max_steer`, or it could not ask for that much.
        for speed in [15.0_f32, 20.0, 30.0] {
            let lock = holdable_lock(&config, speed);
            let radius = wheelbase / (lock * (1.0 + config.rear_steer_ratio)).tan();
            let asked = speed * speed / radius;
            let given = LOCK_GRIP * config.grip * GRAVITY;
            assert!(
                (asked / given - 1.0).abs() < 1e-3,
                "{speed} m/s asks {asked}"
            );
        }
    }
}
