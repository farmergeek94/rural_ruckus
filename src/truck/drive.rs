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
//! Nor does it see light loose scenery -- a cone, a sign, a small rock, lighter than
//! `LIGHT_LOOSE` -- which a truck knocks aside rather than climbs: the tire meets that
//! only as a solid too, which rides up it as up another truck (see `contacts`). Swept
//! onto, a light body was ground that the tire pushed away as it stood
//! on it, and the suspension took the speed it went at for the ground rising: measured, a
//! truck at 10 m/s whose tire met a loose 146 kg cone (tightcorners.pod) a quarter of a
//! metre inboard fired it off at 7.5 m/s sideways and 5.8 m/s up, and its dampers and bump
//! stop answered with a jolt of 27 g that threw the truck up at 3.6 m/s and spun it at 3.9
//! rad/s. The cone ended up inside the tire, because the wheel collider had left it to
//! the sweep.
//!
//! What the sweep does find need not stand still: moving scenery goes its own way, and
//! heavy loose scenery, such as a parked car, can be knocked about. A tire grips against
//! the way it is going, so a truck can ride on it. The tire does not push it back: a
//! heavy object holds a truck up on its own, and is moved by its contacts with the
//! truck's wheels and body.
//!
//! Nor is an upright face of scenery or of a ground box ground, however the truck leans
//! against it (see `carries`). A tire rolls over the top of such a face if the springs
//! reach it (`EDGE_INTO_STOP`), as over a kerb or a log. The sweep's touch on the top edge
//! is the ground, turned up (`edge_lean`), and the hub rises onto it only so fast
//! (`EDGE_RAMP`), so that it rolls up over a few steps and does not jump. A face whose top
//! is out of reach is a wall. Met squarely (`meets_squarely`), a wall squashes the tire,
//! which stops it evenly over a few steps (`wall_squash`), and the tread rides up the wall
//! from the hit, at a share of the speed it hit at (`WALL_CARRY`), and drops back. The
//! wheel stands on the ground at the foot of the wall, found with a ray down through the
//! hub, or holds its length. The wheel collider meets what is deeper (see `contacts`).
//!
//! Measured with the probe, against walls of 0.5 to 3 m met at 0, 30 and 50 degrees, at a
//! crawl and at 4, 8 and 15 m/s, for the built-in truck, Bigfoot and MAXD: every wall of
//! 1.25 m and less is got over, where 130 runs of 144 were; none of 108 runs at 2 to 3 m
//! gets over, where 17 did; the worst jolt into walls of 2 to 3 m at 4 m/s is 20 g on
//! average, where it was 52 g. One run of 288 tips, where 9 did: the built-in truck, met
//! with a 2 m wall at 50 degrees and 15 m/s, gets two wheels onto its top, goes along it
//! and falls off its end.
//!
//! A sweep that begins inside something says nothing of how deep the tire is in it, and a
//! sweep that meets a face first says nothing of the ground at the foot of the face. So
//! such a tire is swept again from higher up, where its bottom is above all that it can
//! stand on: on the terrain from `RESWEEP` of its radius higher, and elsewhere from just
//! above the highest edge in reach.
//!
//! A tire pressed further than its springs go squashes before it is solid. On the terrain
//! it squashes up to `GROUND_SQUASH` past the top of its travel. On an edge, the springs go
//! only `EDGE_SPRINGS` into the bump stop, and the tire squashes into the edge for the rest
//! (`edge_squash`), up to `EDGE_SQUASH`.
//!
//! The springs' dampers go by how fast each hub moves along the truck's up, which keeps
//! the body calm over bumps but does not feel the ground rise under a tire. On the
//! terrain the bump stop's damper, and a damper of its own at the wheel (`GROUND_DAMPING`),
//! go by how fast the ground presses the hub up instead (`stroke_rate`). On scenery and the
//! ground boxes they do not: there an edge met at speed reads as ground that rises very
//! fast (see `STEEP_FULL`).
//!
//! The forces are handed to the physics through `Forces`, which forgets them after each
//! step, so each step's are worked out afresh.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::tasks::{ComputeTaskPool, TaskPool};

use super::interpolate::WheelPose;
use super::{
    GroundGrip, Held, Truck, TruckConfig, TruckInput, TruckWheelColliders, TruckWheels, Wheel,
    WheelCollider, WheelCore,
};
use crate::collision_groups::{GROUND, GROUND_BOXES};

/// How far above its mount, and above how far the tire squashes (`reach_above_mount`),
/// the tire is swept from, in metres, so that a tire pressed up past its mount on a hard
/// landing is found that far in and not merely touching: a cast that begins inside
/// something stops at once and says nothing of how deep. Anything found past the mount
/// means the springs are shut: on the terrain the tire then squashes (`GROUND_SQUASH`),
/// on an edge it has squashed already (`EDGE_SPRINGS`), and elsewhere it is solid (see
/// `WheelCollider`). It also keeps the sweep out of the wheel arch, where a rail or
/// another truck's body would be taken for ground under the tire.
const BUMP_STOP_REACH: f32 = 0.1;

/// How much higher than its first sweep a tire that began that sweep inside the terrain is
/// swept again, as a share of its radius (see `drive_truck`). It must be more than such a
/// tire is pressed in; higher costs nothing more, but finds more of what is overhead.
/// Measured on rough ground at 25 m/s, 0.5% of the tires' sweeps were made again, and on
/// Critic.pod 0.1%. Against anything else the tire is swept again from just above the
/// highest edge in reach (see `EDGE_INTO_STOP`), so that it finds that edge.
const RESWEEP: f32 = 1.0;

/// How far a tire on the terrain squashes once its springs are shut, before it is solid,
/// as a share of its radius. A 66-inch tire really does squash this far. It must stay
/// under the part of the radius outside the wheel's core (`spawn::CORE_SHARE`), so that
/// the tire is solid by the time the core meets the ground. More soaks up harder hits,
/// and sinks the drawn tire further into the ground.
///
/// Measured over whoops 1 m high at 10 to 30 m/s, the hardest jolt went from 109 g to 73
/// g, and on landsbetween.pod from 54 to 114 g to 40 to 43 g. On the ground boxes and on
/// scenery the tire squashes into an edge by a rule of its own (`EDGE_SPRINGS`) and is
/// solid on anything else: with this squash there too, before that rule, a truck driven
/// at a step 1 to 1.25 m high at 8 or 15 m/s was thrown 1.6 to 4.3 m above its height on
/// top, where without it it was thrown 1.7 m at most.
const GROUND_SQUASH: f32 = 0.2;
/// How stiff a tire is as it squashes into the terrain, as a multiple of
/// `bump_stop_spring`, so that it goes with the truck's weight. Higher stops a hit in less
/// of the squash, and jolts more often; lower lets more hits through to the rigid tire.
/// 0.75 jolted less often but harder: on the same tracks, 67 g at worst against 41 g.
const GROUND_SQUASH_STIFFNESS: f32 = 1.75;

/// How far above its mount a tire is swept from, in metres: far enough to find a tire
/// squashed into the terrain as far as it goes, and `BUMP_STOP_REACH` more.
fn reach_above_mount(config: &TruckConfig) -> f32 {
    BUMP_STOP_REACH + GROUND_SQUASH * config.wheel_radius
}

/// How steep the terrain under a tire may be for the dampers to answer all of its rise
/// (see `stroke_rate`), as the cosine of the angle between its normal and straight up:
/// 45 degrees. From there they answer less of it, and none at `STEEP_NONE`, 60 degrees,
/// and go by the hub's own speed instead. A face that steep is a wall that a tire runs
/// into more than ground that rises under it. On the ground boxes and on scenery, whose
/// edges a tire meets at any angle, they never answer it: answered there, the top edge of
/// a step 1.25 m high met at 15 m/s read as the hub being pressed up at 62 m/s, and the
/// bump stop threw the truck 3 to 4 m into the air and over.
///
/// The truck's own lean leaves this alone: going by how far the face leans from the
/// truck's up, over whoops 1 m high at 10 to 30 m/s the hardest jolt was 185 g where it
/// was 73 g, and 2 runs of 18 went over where none did. Higher here and lower in
/// `STEEP_NONE` answer less of a steep bank.
///
/// It is also the least share of the truck's up that `stroke_rate` divides by: by all of
/// it, down to `UPWARD`, a truck on its side was read as pressed in at up to ten times its
/// speed, and 8 computer trucks in 8 races on AlpineMtns.pod had 7 399 truck-steps with a
/// jolt over 5 g and went over 66 times; with it, 6 661 and 52 times. Over those whoops it
/// cost: 127 g at worst, and 3 runs over.
const STEEP_FULL: f32 = 0.707;
const STEEP_NONE: f32 = 0.5;

/// A damper on how fast the terrain presses each hub up (see `stroke_rate`), at the wheel
/// as the bump stop is, as a share of `damper`, so that it follows the garage's
/// suspension dial and suits every truck. The dampers in `beam_loads` go by the hub's own
/// speed, which keeps the body calm but leaves a tire that runs onto a rise to the bump
/// stop alone. It works only as the tire is pressed in. Measured over whoops 0.6 m high at
/// 10 to 30 m/s, with the bump stop's damper on the same rate: without it, 369 steps over
/// 2 g and 61 over 5 g; with it, 125 and 22, and over whoops 0.3 m high no change. At 1,
/// over the whoops 0.6 m high 196 steps over 2 g and none over 5 g, and fewer jolts on
/// the tracks, but 8 computer trucks racing on AlpineMtns.pod were jolted over 5 g in
/// 15 137 truck-steps in 16 races, where at this they were in 13 924, and before it in 14 308.
const GROUND_DAMPING: f32 = 0.57;
/// The rate, in m/s, above which `GROUND_DAMPING`'s damper stiffens by only
/// 1 / `GROUND_DAMPING_BLOWOFF` as fast, as a damper's valve blows off, so that a tire
/// that meets an edge at speed is not kicked up off it. Higher kicks harder.
const GROUND_DAMPING_KNEE: f32 = 0.5;
const GROUND_DAMPING_BLOWOFF: f32 = 4.0;

/// How far below its mount the hub may still be and count as having shut its springs, in
/// metres, and on the terrain squashed its tire as far as it goes (`GROUND_SQUASH`). The
/// sweep reports how far below the mount the tire touches, and the travel is
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

/// The mass under which loose scenery is knocked aside rather than climbed, in kilograms:
/// the sweep does not stand on it, and the tire meets it as a solid (see the module's
/// notes and `contacts`). The cones on tightcorners.pod are 146 kg, its rocks 438 kg, and
/// the parked cars on Critic.pod 907 kg. Higher knocks more aside, and climbs less.
pub(super) const LIGHT_LOOSE: f32 = 250.0;

/// For `holdable_lock`: what the tires' grip is measured against, in m/s².
const GRAVITY: f32 = 9.81;

/// How much of the fore-and-aft lean of what a tire touches its tread takes up, from 0 to 1.
/// A monster truck's tire is big and soft and wraps around an edge it rolls into, so the
/// edge pushes it up more than back. The suspension's load goes along what the tire
/// touches (see `drive_truck`), and on the front of a step or a log that points mostly
/// backwards: at 60 degrees from up, 87% of the load pushes the truck back, and past about
/// 48 degrees full throttle at `grip` 1.1 cannot pull the tire up it. At 0 the tire is
/// rigid and does exactly that. Higher climbs edges quicker and loses less speed to them;
/// at 1 an edge pushes straight up and takes no speed at all. Sideways lean is left alone,
/// so that a bank met side-on still pushes the truck off it. A first guess, not measured:
/// it wants driving over steps and logs.
const TIRE_WRAP: f32 = 0.6;

/// What a tire's tread is pushed along when it touches something whose outward normal is
/// `normal`, heading along `heading`, on a truck whose up is `up`: the normal with
/// `TIRE_WRAP` of its lean along the heading taken out.
fn wrapped(normal: Vec3, heading: Vec3, up: Vec3) -> Vec3 {
    let along = heading.reject_from_normalized(up).normalize_or_zero();
    (normal - along * normal.dot(along) * TIRE_WRAP).normalize_or(normal)
}

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

/// Whether the suspension carries a tire on a face that pushes it along `normal`, in the
/// world, on a truck whose up is `up`. The face must hold the tire up by `UPWARD` as the
/// truck stands and, unless it is the `terrain`, as the world stands too. `contacts` asks
/// the same of each contact, so that a face is the suspension's or the wheel collider's,
/// and never both or neither.
///
/// As the world stands, because an upright wall is a wall however the truck leans on it.
/// By the truck's up alone, a wall turned into ground once a tire that climbed it had
/// lifted the nose 6 degrees: the climb (see `contacts`) stopped there, and the springs
/// pushed the truck back off the wall instead. The terrain is left to the truck's up
/// alone, as it was: it is a field of heights, which has slopes and no upright face.
/// Scenery and the ground boxes have them. Not measured: it wants driving at a wall.
pub(super) fn carries(normal: Vec3, up: Vec3, terrain: bool) -> bool {
    normal.dot(up) > UPWARD && (terrain || normal.y > UPWARD)
}

/// How far into the bump stop the springs may be pressed for a tire to roll over an edge,
/// as a share of `bump_stop_travel`: an edge whose top is further below the mount than
/// this is in reach. 0 reaches as high as the bump stop begins; at 1 the hub may reach the
/// mount itself. With the built-in truck at rest, 0.5 reaches 1.4 m up; with a POD truck
/// on 0.91 m tires, 1.31 m. Higher rolls over taller walls, with a harder push from the bump
/// stop.
const EDGE_INTO_STOP: f32 = 0.5;

/// How far into the bump stop the springs may be pressed by an edge, as a share of
/// `bump_stop_travel`. Where the sweep would put the hub higher still, the tire squashes
/// into the edge instead (see `edge_squash`): pressed fully into the bump stop by an edge
/// met at 8 m/s, the springs threw the truck up and back. Higher passes more of an edge to
/// the bump stop.
const EDGE_SPRINGS: f32 = 0.5;

/// How steeply a hub may rise towards an edge it rolls over, as rise per metre the truck
/// goes on. A real tire squashes against an edge and rises over it as it rolls on, where
/// the sweep finds the hub's place on top of the edge at once: measured with the built-in
/// truck at 8 m/s into a 1.25 m wall, it put the hub 0.44 m higher in one step, and the
/// springs answered with the bump stop. Higher rises quicker and digs the
/// tire less far into the face; lower spreads the rise over more steps.
const EDGE_RAMP: f32 = 3.0;

/// How fast a hub may rise towards an edge however slowly the truck goes, in m/s, so that a
/// truck creeping up to an edge still gets onto it. Higher is quicker at a crawl.
const EDGE_RISE_SPEED: f32 = 3.0;

/// The most that an edge's push may lean from the truck's up, in radians. The sweep meets
/// the top of a face with a normal that is nearly level, which would push the truck back
/// off the edge with nearly all of its load. Lower glides over with less speed lost;
/// higher behaves more like a rigid wheel, which an edge stops.
const EDGE_LEAN_MAX: f32 = 30.0 * std::f32::consts::PI / 180.0;

/// How long a tire squashed into an edge takes to stop going further in, in seconds, for
/// its quarter of the truck: the time to take all but a third of the speed it goes in at.
/// Longer stops it more gently, and lets it further into the edge before the edge's face
/// stops it outright (see `contacts`).
const EDGE_SQUASH_TIME: f32 = 0.025;

/// How far a tire may squash into a face of scenery or of a ground box, as a share of its
/// radius. Beyond it the face stops it outright (see `contacts`), and so does a ground
/// box's face against a wheel's core, at a quarter of the radius.
pub(super) const FACE_SQUASH: f32 = 0.35;

/// How far into a face `wall_squash` stops a tire, as a share of its radius: short of
/// `FACE_SQUASH`, so that what it misses by does not reach the rigid contact there.
/// Further spreads the stop over more of the tire, and lets it further into the face.
const FACE_STOP: f32 = 0.25;

/// How stiff a tire squashed into a face is, as the frequency in Hz at which its quarter of
/// the truck would bounce on it. It holds a truck that the throttle pushes into a face.
/// Higher holds it further out, and pushes a truck that slides along a wall off it harder.
const FACE_FREQUENCY: f32 = 2.0;

/// The least that `wall_squash` counts on being left of `FACE_STOP`, as a share of the
/// radius, so that its push stays finite.
const SQUASH_LEFT_LEAST: f32 = 0.02;

/// How far a tire may squash into an edge that it rolls onto, as a share of its radius,
/// before it is solid, as a tire whose springs are shut is (see `WheelCollider`). On the
/// terrain it is `GROUND_SQUASH`.
const EDGE_SQUASH: f32 = 0.5;

/// Over how much of its radius the damper of a tire squashed into an edge comes in, as a
/// share. Lower comes in more sharply.
const EDGE_SQUASH_ONSET: f32 = 0.05;

/// How stiff a tire squashed into an edge is, as the frequency in Hz at which its quarter
/// of the truck would bounce on it. Higher lifts a truck over an edge as high as its hub
/// sooner, with less of the tire in the edge, and throws it higher: at 2.5 Hz the built-in
/// truck met a 1.25 m wall at 15 m/s with 160 g, and at 4 Hz with 66 g, but rose 3.1 m.
const EDGE_SQUASH_FREQUENCY: f32 = 4.0;

/// How much of the speed a tire goes into a wall at carries it up the wall, as a share: its
/// tread grips the wall as it hits, and rides up it from the hit. Nothing else lifts it, so
/// that a truck crawling at a wall stays at its foot; it drops back down once it has
/// stopped going in. Higher rides further up, and throws the nose up harder.
pub(super) const WALL_CARRY: f32 = 0.3;

/// How hard a tire squashed `depth` metres into an edge pushes back, in newtons, as it
/// goes further in at `going_in` m/s: its spring and its damper, for its quarter of the
/// truck.
fn edge_squash(config: &TruckConfig, depth: f32, going_in: f32) -> f32 {
    if depth <= 0.0 {
        return 0.0;
    }
    let quarter = config.mass / 4.0;
    let stiffness = quarter * (std::f32::consts::TAU * EDGE_SQUASH_FREQUENCY).powi(2);
    // The damper comes in over the first `EDGE_SQUASH_ONSET` of the radius, as the bump
    // stop's does: whole at once, it met a tire at speed with all of its force in one step.
    let onset = (depth / (EDGE_SQUASH_ONSET * config.wheel_radius)).min(1.0);
    // Damped both ways, so that it does not throw back what it soaks up.
    (stiffness * depth + onset * quarter / EDGE_SQUASH_TIME * going_in).max(0.0)
}

/// How squarely a face must stand in a tire's way for the tire to squash into it and ride
/// up it, as the cosine of the angle between them: 0.5 is 60 degrees. Met more at a glance,
/// the side of the tire brushes the face and bounces off it, as before: squashed, a truck
/// that brushed a wall on Critic went on along another line and lost 160 m in a minute.
/// Lower squashes into faces met more at a glance.
pub(super) const WALL_FACING: f32 = 0.5;

/// The speed under which `meets_squarely` also goes by where a tire points, and not only by
/// where it goes, in m/s. Higher squashes tires steered at a wall that the truck slides
/// along faster; lower leaves a truck that slides slowly along a wall, driven into it, to
/// the rigid contact, which rolled the built-in truck over at 50 degrees to a 2 m wall.
const SQUARELY_SLOW: f32 = 8.0;

/// Whether a tire that goes at `velocity`, in m/s, pointing along `heading`, both in the
/// world, meets a face whose outward normal is `normal` squarely enough to squash into it
/// (`WALL_FACING`): by where it goes, or, slower than `SQUARELY_SLOW`, by where it points.
/// By where it points at any speed, a front tire steered towards a wall that the truck slid
/// along at 21 m/s squashed into it. `drive` and `contacts` both ask, so that a face is
/// squashed by the one and left unsolved by the other, or neither.
pub(super) fn meets_squarely(velocity: Vec3, heading: Vec3, normal: Vec3) -> bool {
    let level = normal.reject_from(Vec3::Y).normalize_or_zero();
    let going = velocity.reject_from(Vec3::Y);
    let speed = going.length();
    -going.dot(level) >= WALL_FACING * speed
        || (speed < SQUARELY_SLOW && heading.dot(level).abs() >= WALL_FACING)
}

/// The push of a face on a tire squashed `depth` metres into it: along `level`, the face's
/// level outward normal, and up the face by the tire's grip as it rides up it (see
/// `WALL_CARRY`). `hub_velocity` is how fast the hub goes, in m/s.
///
/// The tire is stopped evenly over what is left of `FACE_STOP`, so that it stops before
/// it is that far in, with no harder a push than that takes: a spring or a damper pushes
/// hardest at one end of the squash, and twice as hard as an even stop over the same
/// depth. Its spring holds a tire that the throttle pushes into the face.
fn wall_squash(config: &TruckConfig, depth: f32, level: Vec3, hub_velocity: Vec3, dt: f32) -> Vec3 {
    let quarter = config.mass / 4.0;
    let going_in = (-hub_velocity.dot(level)).max(0.0);
    let stiffness = quarter * (std::f32::consts::TAU * FACE_FREQUENCY).powi(2);
    let left =
        (FACE_STOP * config.wheel_radius - depth).max(SQUASH_LEFT_LEAST * config.wheel_radius);
    // As if this tire stopped half the truck, as each front tire does in a wall met head
    // on. One that stops more of it is stopped less evenly, and later.
    let half = config.mass / 2.0;
    let pressed = stiffness * depth.max(0.0) + half * going_in * going_in / (2.0 * left);
    let rising = hub_velocity.y;
    let lift = ((WALL_CARRY * going_in - rising) * quarter / dt).clamp(0.0, config.grip * pressed);
    level * pressed + Vec3::Y * lift
}

/// How far inside a face and above where a sweep met it a ray looks for the top of the
/// face, in metres (see `drive_truck`). Each must be more than the error of where a sweep
/// says it touched, and less than the thinnest wall and the smallest step that a tire
/// rolls onto.
const TOP_TEST_IN: f32 = 0.02;
const TOP_TEST_ABOVE: f32 = 0.05;

/// The push of what is not the terrain, from its outward normal, turned up towards `up`
/// until it leans no further than `EDGE_LEAN_MAX`.
fn edge_lean(normal: Vec3, up: Vec3) -> Vec3 {
    let (sin, cos) = EDGE_LEAN_MAX.sin_cos();
    if normal.dot(up) >= cos {
        return normal;
    }
    let level = normal.reject_from_normalized(up).normalize_or_zero();
    (up * cos + level * sin).normalize_or(up)
}

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

/// How much faster than the ground a tire that the engine asks too much of spins, at full
/// throttle, in m/s of tread speed: a tire that slips (see `tread_on_the_ground`). For the
/// visual only. Higher spins it faster as it slips; much past 20 and it blurs.
const SLIP_SPIN: f32 = 12.0;
/// How long a slipping tire takes to spin up to `SLIP_SPIN`, in seconds: the time to close
/// all but a third of the gap.
const SLIP_SPIN_UP_TIME: f32 = 0.3;
/// How long a tire that grips again takes to slow to the ground's speed, in seconds. Short,
/// so that it rolls with the ground, but not one step, so that a tire on the edge of its
/// grip does not flicker between spinning and rolling.
const GRIP_CATCH_TIME: f32 = 0.08;
/// How steep the ground must be along a tire, as the sine of its slope, before throttle
/// against the way the truck goes is taken for climbing and not for the brakes: a truck
/// that slides back down a hill with its tires spinning uphill. About 6 degrees.
const SLIP_UPHILL: f32 = 0.1;

/// How many times `brake_force` the handbrake brakes the rear wheels with, which it locks.
/// The front wheels get `brake_force` alone and roll on, so that they still steer. A
/// locked tire slides at `handbrake_grip` times its load, which is less than `brake_force`
/// on a truck at rest on its springs, so higher than 1 matters only on a tire pressed
/// down hard, as in a landing.
const HANDBRAKE_REAR_BRAKE: f32 = 2.0;

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
/// What `drive_truck` writes of each wheel collider: where it is, and whether its springs
/// are shut.
type WheelColliderParts = (&'static mut Transform, &'static mut WheelCollider);
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

/// One tire's sweep, which sees only what `stands_on` lets it.
fn sweep(
    spatial: &SpatialQuery,
    cast: &Cast,
    stands_on: &dyn Fn(Entity) -> bool,
) -> Option<ShapeHitData> {
    sweep_from(
        spatial,
        &cast.tire,
        cast.from,
        cast.turned,
        cast.down,
        cast.reach,
        stands_on,
    )
}

/// A tire's sweep, from `from` down `reach` metres.
fn sweep_from(
    spatial: &SpatialQuery,
    tire: &Collider,
    from: Vec3,
    turned: Quat,
    down: Dir3,
    reach: f32,
    stands_on: &dyn Fn(Entity) -> bool,
) -> Option<ShapeHitData> {
    spatial.cast_shape_predicate(
        tire,
        from,
        turned,
        down,
        &ShapeCastConfig {
            max_distance: reach,
            target_distance: 0.0,
            // A cast that starts inside something stops there, and says so.
            compute_contact_on_penetration: true,
            ignore_origin_penetration: false,
        },
        &SpatialQueryFilter::default(),
        stands_on,
    )
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
    // A few to each task: a task costs more to hand out than a sweep does to make.
    const PER_TASK: usize = 4;
    ComputeTaskPool::get_or_init(TaskPool::default)
        .scope(|scope| {
            for chunk in casts.chunks(PER_TASK) {
                scope.spawn(async move {
                    chunk
                        .iter()
                        .map(|cast| sweep(spatial, cast, stands_on))
                        .collect::<Vec<_>>()
                });
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
    mut wheels: Query<(&mut Wheel, &mut WheelPose), OnlyWheels>,
    mut wheel_colliders: Query<WheelColliderParts, OnlyWheelColliders>,
    // Whatever a tire stands on that is a body, the body a collider is part of, and what
    // kind of body it is.
    surfaces: Query<Forces, Without<Truck>>,
    bodies: Query<&ColliderOf>,
    kinds: Query<(&RigidBody, &ComputedMass, Has<Truck>)>,
    // Which collider is the terrain, for `carries`.
    layers: Query<&CollisionLayers>,
) {
    let dt = time.delta_secs();
    // Not this truck, another, or light loose scenery (see the module's notes).
    let stands_on = |collider: Entity| {
        let body = bodies.get(collider).map_or(collider, |of| of.body);
        !kinds.get(body).is_ok_and(|(kind, mass, truck)| {
            kind.is_dynamic() && (truck || mass.value() < LIGHT_LOOSE)
        })
    };

    // First where every tire of every truck is swept from, then all the sweeps at once. For
    // each truck, for each of its wheels, what is known of it before its sweep.
    let mut casts = Vec::new();
    let mut planned: Vec<Vec<Option<Planned>>> = Vec::new();
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
            .map(|&child| {
                let (wheel, _) = wheels.get(child).ok()?;
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
                    tire: colliders.tire.clone(),
                    from: mount + up * reach_above_mount(config),
                    turned: transform.rotation * tire_rotation * AXLE_ALONG_X,
                    down: Dir3::new(-up).unwrap_or(Dir3::NEG_Y),
                    reach: travel + reach_above_mount(config),
                });
                Some(Planned {
                    cast: casts.len() - 1,
                    steer_rotation,
                    mount: wheel.mount,
                    mount_in_world: mount,
                })
            })
            .collect();
        planned.push(wheels_of_truck);
    }
    let hits = sweep_all(&spatial, &casts, &stands_on);

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
        for (index, ((&child, &collider), plan)) in children
            .0
            .iter()
            .zip(&colliders.colliders)
            .zip(planned)
            .enumerate()
        {
            let Some(Planned {
                cast,
                steer_rotation,
                mount: mount_in_truck,
                mount_in_world: mount,
            }) = *plan
            else {
                continue;
            };
            let lift = reach_above_mount(config);
            let (from, tire_turned) = (casts[cast].from, casts[cast].turned);
            // The terrain alone has no upright faces (see `carries`).
            let is_terrain = |collider: Entity| {
                layers.get(collider).is_ok_and(|layers| {
                    layers.memberships.has_all(GROUND) && !layers.memberships.has_all(GROUND_BOXES)
                })
            };
            // A touch on what the suspension carries the tire on.
            let stood_on = |collider: Entity, normal: Vec3, below_mount: f32, on_the_tread: f32| {
                // The ground and scenery are each a body of their own; only a collider
                // within a body, such as a truck's, needs its body looked up.
                let (surface, kind) = match kinds.get(collider) {
                    Ok((kind, ..)) => (collider, Some(kind)),
                    Err(_) => {
                        let body = bodies.get(collider).map_or(collider, |of| of.body);
                        (body, kinds.get(body).ok().map(|(kind, ..)| kind))
                    }
                };
                let terrain = is_terrain(collider);
                Touch {
                    below_mount,
                    normal,
                    on_the_tread,
                    surface,
                    moves: kind.is_some_and(|kind| !kind.is_static()),
                    terrain,
                    // Pressed past the top of its travel, a tire on the terrain squashes
                    // (see `GROUND_SQUASH`).
                    squash: if terrain {
                        (-below_mount).clamp(0.0, GROUND_SQUASH * config.wheel_radius)
                    } else {
                        0.0
                    },
                    rolling_over: None,
                }
            };
            // Where the hub was after the last step, as far below its mount.
            let last_length = wheel_colliders
                .get(collider)
                .map_or(travel, |(at, _)| mount_in_truck.y - at.translation.y);
            // How far the hub may rise towards what is not the terrain in this step (see
            // `EDGE_RAMP`), and more as the body comes down onto it.
            let mount_velocity = forces.velocity_at_point(mount);
            let rolling = mount_velocity.reject_from_normalized(up).length();
            let sinking = (-mount_velocity.dot(up)).max(0.0);
            let rise_limit = (EDGE_RISE_SPEED.max(rolling * EDGE_RAMP) + sinking) * dt;
            // How far below the mount the top of an edge must be for the springs to reach
            // it (see `EDGE_INTO_STOP`).
            let reach_floor = config.bump_stop_travel * EDGE_INTO_STOP;
            let edge_springs = config.bump_stop_travel * (1.0 - EDGE_SPRINGS);
            // Whether a sweep that met a face met it at its top, and not partway up it: a
            // sweep goes down along the truck's up, and on a truck that leans, a tire
            // pressed against a wall meets the wall's face as it goes down. Taken for the
            // top, the face was climbed. A short ray down from just above the touch, just
            // inside the face, finds the top there, or begins inside the face.
            let at_the_top = |hit: &ShapeHitData| {
                let inward = -hit.normal1.reject_from(Vec3::Y).normalize_or_zero();
                spatial
                    .cast_ray_predicate(
                        hit.point1 + inward * TOP_TEST_IN + Vec3::Y * TOP_TEST_ABOVE,
                        Dir3::NEG_Y,
                        2.0 * TOP_TEST_ABOVE,
                        true,
                        &SpatialQueryFilter::default(),
                        &|collider| collider == hit.entity,
                    )
                    .is_some_and(|ray| ray.distance > 0.0)
            };
            // What a sweep from `lift` above the mount found, if the suspension stands the
            // tire on it.
            let stand_on = |hit: &ShapeHitData, from: Vec3, lift: f32| {
                // `normal1` is the outward normal of what was hit, in the world, and
                // `point2` the touch on the tire, in the world too: taken into the
                // cylinder's own frame, whose axis is Y, from where the tire touched.
                let normal = hit.normal1.try_normalize().unwrap_or(up);
                let touched_at = from - up * hit.distance;
                let on_the_tire = tire_turned.inverse() * (hit.point2 - touched_at);
                let out_from_axle = on_the_tire.xz().length() / config.wheel_radius;
                let on_the_tread = ((out_from_axle - SIDEWALL) / (1.0 - SIDEWALL)).clamp(0.0, 1.0);
                // Brushed by the sidewall, and not stood on: the wheel hangs.
                if on_the_tread <= 0.0 {
                    return None;
                }
                let found = hit.distance - lift;
                // The terrain against a tire that it does not hold up is the top of a tire
                // on a truck tipped over, which the wheel collider takes as a rigid
                // contact instead (see `contacts`).
                if is_terrain(hit.entity) {
                    return carries(normal, up, true)
                        .then(|| stood_on(hit.entity, normal, found, on_the_tread));
                }
                // Anything else the tire rolls onto, as a kerb, a deck or the top of a wall,
                // if the springs reach it. A sweep that began inside it says nothing of how
                // far in, and a face that does not hold the tire up is rolled onto only at
                // its top.
                if hit.distance <= 0.0 || (mount - hit.point1).dot(up) < reach_floor {
                    return None;
                }
                if !carries(normal, up, false) && (normal.dot(up) <= -UPWARD || !at_the_top(hit)) {
                    return None;
                }
                // Further up than the springs go, the tire squashes rather than shutting them.
                let length = found.max(last_length - rise_limit).max(edge_springs);
                let leaned = edge_lean(normal, up);
                let squash = (edge_springs - found).max(0.0);
                Some(Touch {
                    squash,
                    rolling_over: (leaned != normal || squash > 0.0).then_some(hit.entity),
                    ..stood_on(hit.entity, leaned, length, on_the_tread)
                })
            };
            // Against a face that it cannot roll over, with nothing found under it, the
            // wheel holds its length rather than dropping to the bottom of its travel: a
            // wheel that drops on a truck whose nose is up swings into the face.
            let mut walled = false;
            let touch = hits[cast].and_then(|first| {
                // What the sweep found, unless it began inside it: such a sweep says nothing
                // of how deep the tire is in it.
                if first.distance > 0.0 {
                    if let Some(stood) = stand_on(&first, from, lift) {
                        return Some(stood);
                    }
                    // The terrain has no upright faces (see `carries`), and what holds the
                    // tire up but is out of reach is no ground for it either.
                    let normal = first.normal1.try_normalize().unwrap_or(up);
                    if is_terrain(first.entity) || carries(normal, up, false) {
                        return None;
                    }
                }
                // Something the tire is already in, or a face. A sweep stops at the first
                // thing it meets, and a tire pressed into a face meets the face before the
                // top of it or the ground at its foot. So the tire is swept again from where
                // its bottom is above all that it can stand on. In the terrain that is
                // `RESWEEP` of its radius higher: taken at its word, a sweep that began
                // inside the terrain left the wheel at the bottom of its travel, deep in the
                // ground, and the rigid contacts of its collider and core stopped the truck;
                // measured, a truck at 28 m/s into a whoop came out of one step going
                // backwards at 4 m/s. Against anything else it is just above the highest
                // edge the springs reach, so that whatever is found, the tire rolls onto.
                // Few wheels are in either case in any step, so these are cast here, one
                // after another.
                let higher = if is_terrain(first.entity) {
                    RESWEEP * config.wheel_radius
                } else {
                    config.wheel_radius - reach_floor - lift
                };
                // What the last sweep met.
                let mut met = first.entity;
                if higher > 0.0 {
                    let again_from = from + up * higher;
                    let again = sweep_from(
                        &spatial,
                        &colliders.tire,
                        again_from,
                        tire_turned,
                        casts[cast].down,
                        casts[cast].reach + higher,
                        &stands_on,
                    );
                    if let Some(hit) = again {
                        if let Some(stood) = stand_on(&hit, again_from, lift + higher) {
                            return Some(stood);
                        }
                        met = hit.entity;
                    }
                }
                // The terrain has no upright faces: what of it the tire does not stand on is
                // no wall either.
                if is_terrain(met) {
                    return None;
                }
                walled = true;
                // Out of reach, a wall. What the tire stands on is found with a ray
                // instead, down through the hub, which the wall is not in the way of: the
                // tire reaches the ground a radius before the ray does. Left without it,
                // the wheel hung with nothing under it, and neither held its end of the
                // truck up nor drove it. From where the first sweep began, low in the
                // wheel arch: from higher up, the ray of a truck that leans on a wall
                // can begin inside the wall.
                let ray = spatial.cast_ray_predicate(
                    from,
                    casts[cast].down,
                    casts[cast].reach + config.wheel_radius,
                    true,
                    &SpatialQueryFilter::default(),
                    &stands_on,
                )?;
                let normal = ray.normal.try_normalize().unwrap_or(up);
                // No deeper than a sweep that begins inside something says.
                let below_mount = (ray.distance - config.wheel_radius - lift).max(-lift);
                carries(normal, up, is_terrain(ray.entity))
                    .then(|| stood_on(ray.entity, normal, below_mount, 1.0))
            });

            // Suspension length: from the mount down to the hub. With nothing under it the
            // wheel hangs at the bottom of its travel, unless it is against a face. Above
            // the mount it can't go: the hub stays at its upper limit and the rest is taken
            // by the bump stop.
            let hanging = if walled {
                last_length.clamp(0.0, travel)
            } else {
                travel
            };
            let length = touch.map_or(hanging, |touch| touch.below_mount.max(0.0));
            let hub = mount - up * length;
            // Against what it stands on, which is only the ground's own when that is still.
            let surface_velocity = touch
                .filter(|touch| touch.moves)
                .and_then(|touch| surfaces.get(touch.surface).ok())
                .map_or(Vec3::ZERO, |surface| surface.velocity_at_point(hub));
            let hub_velocity = forces.velocity_at_point(hub) - surface_velocity;
            // A face of scenery or of a ground box squashes the tire where it reaches as
            // high as the hub, and the tread rides up it from the hit (see `wall_squash`):
            // the suspension rolls a tire over what is lower without losing speed, and
            // lifts it only so fast over what is higher. Few wheels are against a face in
            // any step, so these rays are cast here, one after another.
            let face = hits[cast].filter(|hit| {
                !is_terrain(hit.entity)
                    && (hit.distance <= 0.0 || hit.normal1.dot(up) < EDGE_LEAN_MAX.cos())
            });
            if let Some((face, normal)) = face.map(|hit| (hit.entity, hit.normal1)) {
                let level = normal.reject_from(Vec3::Y).normalize_or_zero();
                let tire_forward = transform.rotation * steer_rotation * Vec3::NEG_Z;
                let tire_axle = transform.rotation * steer_rotation * Vec3::X;
                // How far the tire reaches from its hub towards the face.
                let reach = config.wheel_radius * level.dot(tire_forward).abs()
                    + config.wheel_width / 2.0 * level.dot(tire_axle).abs();
                // A face met at a glance, by the side of the tire, is the wheel collider's
                // alone (see `contacts`).
                let squarely = meets_squarely(hub_velocity, tire_forward, level);
                let into = Dir3::new(-level)
                    .ok()
                    .filter(|_| squarely)
                    .and_then(|into| {
                        spatial.cast_ray_predicate(
                            hub,
                            into,
                            reach,
                            true,
                            &SpatialQueryFilter::default(),
                            &|collider| collider == face,
                        )
                    });
                if let Some(ray) = into {
                    let push = wall_squash(config, reach - ray.distance, level, hub_velocity, dt);
                    forces.apply_force_at_point(push, hub - level * ray.distance);
                }
            }
            sweeps.push(Sweep {
                index,
                wheel: child,
                collider,
                mount: mount_in_truck,
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
            let Ok((mut wheel, mut pose)) = wheels.get_mut(sweep.wheel) else {
                continue;
            };
            wheel.tilt = tilt;
            let steer_rotation = sweep.steer_rotation;
            // The handbrake locks the rear wheels; the front ones it brakes, and they roll on.
            let locked = input.handbrake && !wheel.front;
            // The ground's speed under the tread, while it touches, and whether the tire
            // slips on it under the throttle.
            let mut rolling_speed = None;
            let mut slipping = false;
            // Pressed past the top of its travel: the springs are shut and the tire is
            // solid from here on (see `WheelCollider`).
            let mut bottomed = false;

            if let Some(touch) = sweep.touch {
                // Pressed past the top of its travel, and squashed as far as the tire goes:
                // on the terrain `GROUND_SQUASH` further, and into an edge `EDGE_SQUASH`.
                let radius = config.wheel_radius;
                bottomed = if touch.terrain {
                    touch.below_mount < SPRINGS_SHUT - GROUND_SQUASH * radius
                } else {
                    touch.below_mount < SPRINGS_SHUT || touch.squash > EDGE_SQUASH * radius
                };
                let hub = sweep.hub;
                let hub_velocity = sweep.hub_velocity;
                // How much the dampers at the wheel go by how fast the terrain presses the
                // hub up, and not by the hub's own speed (see `STEEP_FULL`).
                let share = if touch.terrain {
                    ground_share(touch.normal.y)
                } else {
                    0.0
                };
                let ground_rate = stroke_rate(hub_velocity, touch.normal, up) * share;
                let compression_rate = rise_rate(sweep) * (1.0 - share) + ground_rate;

                // The bump stop, over the top of the travel, at the wheel itself: a stiff
                // spring, and a damper that works only as the wheel presses in. So it soaks
                // up most of a hit, and gives a little back as the wheel comes out, a small
                // upward push. Both come in gradually, as rubber does, from nothing where the
                // stop begins to all of their figures at the top of the travel: switched on
                // whole, the damper alone met a wheel entering the stop at 1 m/s with 54 kN in
                // one step. Measured turning into a bank of 25 to 45 degrees at 10 and 15 m/s
                // and riding up it, the steps in which the lift or the turning changed sharply
                // went from 116 to 66, and the sharpest change of lift from 2.5 g to 0.5 to
                // 1.7 g; on Alpine the springs shut, the truck was jolted and lost speed no
                // more often than before.
                //
                // On the terrain its damper goes by how fast the ground presses the hub up,
                // as much as the face is not too steep (see `STEEP_FULL`). By the hub's own
                // speed, as the springs' dampers go, a tire that ran onto a rise was not
                // slowed by it at all, and the springs shut: measured over whoops 0.6 m
                // high at 30 m/s, the hardest jolt went from 391 g to 21 g.
                let into_stop = rise(sweep) - (config.suspension_bump - config.bump_stop_travel);
                let bump_stop = if into_stop > 0.0 {
                    let depth = (into_stop / config.bump_stop_travel).min(1.0);
                    depth
                        * (config.bump_stop_spring * into_stop
                            + config.bump_stop_damper * compression_rate.max(0.0))
                } else {
                    0.0
                };
                // See `GROUND_DAMPING`.
                let stroke = ground_rate.max(0.0);
                let ground_damper = config.damper
                    * GROUND_DAMPING
                    * (stroke.min(GROUND_DAMPING_KNEE)
                        + (stroke - GROUND_DAMPING_KNEE).max(0.0) / GROUND_DAMPING_BLOWOFF);
                // The tire squashed: into the terrain, past the top of its travel, or into an
                // edge that the springs cannot lift it onto.
                let squashed = if touch.terrain {
                    GROUND_SQUASH_STIFFNESS * config.bump_stop_spring * touch.squash
                } else {
                    edge_squash(config, touch.squash, compression_rate)
                };
                let load =
                    (spring + bump_stop + ground_damper + squashed).max(0.0) * touch.on_the_tread;
                let heading = transform.rotation * steer_rotation * Vec3::NEG_Z;
                let normal = wrapped(touch.normal, heading, up);

                // Tire axes, flattened onto the ground plane.
                let forward = heading.reject_from_normalized(normal).normalize_or_zero();
                let side = forward.cross(normal);
                let forward_speed = hub_velocity.dot(forward);
                let side_speed = hub_velocity.dot(side);
                rolling_speed = Some(if locked { 0.0 } else { forward_speed });

                let braking = input.throttle * forward_speed < 0.0 && forward_speed.abs() > 0.5;
                let asked = if held {
                    hold(forward_speed, config.brake_force)
                } else if locked {
                    -forward_speed.signum() * config.brake_force * HANDBRAKE_REAR_BRAKE
                } else if input.handbrake {
                    hold(forward_speed, config.brake_force)
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
                let asked_of_tire = forward * drive + side * cornering;
                let tire = asked_of_tire.clamp_length_max(grip * load);
                // The engine turns a tire faster than the ground goes by when it asks for
                // more than the tire grips. Throttle against the way the truck goes is the
                // brakes, unless the truck is sliding back down a hill it faces up.
                let climbing = forward.y * input.throttle > SLIP_UPHILL;
                slipping = input.throttle != 0.0
                    && !held
                    && !input.handbrake
                    && (!braking || climbing)
                    && asked_of_tire.length_squared() > (grip * load).powi(2);

                // The load along the surface's normal, as the ground pushes: a bank met
                // sideways pushes the truck off it, not upwards. At the hub rather than
                // where the springs sit on the beam: the beam's loads already carry what
                // that does to the body. Drive and brakes at the hub too, and the sideways
                // grip `cornering_lever` of the way down to the ground.
                let sideways = side * tire.dot(side);
                forces.apply_force_at_point(normal * load + tire - sideways, hub);
                forces.apply_force_at_point(
                    sideways,
                    hub - normal * config.wheel_radius * config.cornering_lever,
                );
            }

            let tire_rotation = Quat::from_rotation_z(tilt) * steer_rotation;
            wheel.tread_speed = match rolling_speed {
                Some(ground) => tread_on_the_ground(
                    wheel.tread_speed,
                    ground,
                    if slipping { input.throttle } else { 0.0 },
                    dt,
                ),
                None => spin_in_the_air(
                    wheel.tread_speed,
                    if held { 0.0 } else { input.throttle },
                    input.handbrake || held,
                    config.top_speed,
                    dt,
                ),
            };
            wheel.spin -= wheel.tread_speed / config.wheel_radius * dt;
            let hub_in_truck = wheel.mount - Vec3::Y * sweep.length;
            // The wheel is a pivot whose axle lies along X. What is drawn hangs from it, and
            // is drawn between this step's pose and the last (see `interpolate`).
            pose.step(
                Transform::from_translation(hub_in_truck)
                    .with_rotation(tire_rotation * Quat::from_rotation_x(wheel.spin)),
            );
            // And the collider goes with it, and its core, which is its child. A cylinder
            // needs no spin. It also carries whether the springs are shut, which is what
            // tells `contacts` to stop dropping this wheel's ground contact and let the
            // tire be solid.
            if let Ok((mut collider_transform, mut wheel_collider)) =
                wheel_colliders.get_mut(sweep.collider)
            {
                collider_transform.translation = hub_in_truck;
                collider_transform.rotation = tire_rotation * AXLE_ALONG_X;
                wheel_collider.bottomed = bottomed;
                wheel_collider.rolling_over = sweep.touch.and_then(|touch| touch.rolling_over);
            }
        }
    }
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
/// against its spin, the brakes, or the handbrake stop it quickly; and
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
    towards(speed, target, time, dt)
}

/// How fast the tread of a wheel on the ground goes round a step on, in m/s, from `speed`.
/// It rolls with the ground (`ground`), or, given the throttle it slips under, spins
/// `SLIP_SPIN` faster than that the way the throttle pushes: trying to go, and slipping.
fn tread_on_the_ground(speed: f32, ground: f32, slip_throttle: f32, dt: f32) -> f32 {
    if slip_throttle != 0.0 {
        towards(speed, ground + slip_throttle * SLIP_SPIN, SLIP_SPIN_UP_TIME, dt)
    } else {
        towards(speed, ground, GRIP_CATCH_TIME, dt)
    }
}

/// `speed` drawn towards `target` for `dt` seconds, closing all but a third of the gap in
/// `time` seconds, whatever the step.
fn towards(speed: f32, target: f32, time: f32, dt: f32) -> f32 {
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
    /// The body it touches.
    surface: Entity,
    /// Whether that body moves: moving or heavy loose scenery, and not the ground or fixed
    /// scenery, which are not looked into.
    moves: bool,
    /// Whether it is the terrain, and not a ground box or scenery.
    terrain: bool,
    /// How far the tire is squashed, in metres: on the terrain, as far as it is pressed
    /// past the top of its travel (`GROUND_SQUASH`); on an edge that it rolls over, as far
    /// as the edge presses it past what the springs take (`EDGE_SPRINGS`).
    squash: f32,
    /// What it rolls over the edge of, if that is not the terrain (see `WheelCollider`).
    rolling_over: Option<Entity>,
}

/// How fast a hub is pressed up towards its mount, in m/s, by what its tire stands on:
/// from how fast it goes, `hub_velocity`, against a surface whose outward normal is
/// `normal`, on a truck whose up is `up`. A tire that stays on the surface keeps its
/// distance from it along the normal, so the hub must give along `up` by as much as it
/// closes in along the normal, over how much of `up` the normal has (no less than
/// `STEEP_FULL`). Unlike the hub's own speed along `up`, it holds the rise of the ground
/// under a tire that runs onto a slope or a bump; and it is 0 for a truck that drives
/// over level ground nose up, where the hub's own speed is its speed times the sine of
/// the pitch.
fn stroke_rate(hub_velocity: Vec3, normal: Vec3, up: Vec3) -> f32 {
    -hub_velocity.dot(normal) / up.dot(normal).max(STEEP_FULL)
}

/// How much of the terrain's rise under a tire the dampers answer, from 0 to 1, on a face
/// whose normal has this much of straight up in it (see `STEEP_FULL`).
fn ground_share(upright: f32) -> f32 {
    ((upright - STEEP_NONE) / (STEEP_FULL - STEEP_NONE)).clamp(0.0, 1.0)
}

/// What is known of a wheel before its tire is swept.
#[derive(Clone, Copy)]
struct Planned {
    /// Which of the sweeps is its.
    cast: usize,
    steer_rotation: Quat,
    /// The top of its travel, in the truck's axes and in the world.
    mount: Vec3,
    mount_in_world: Vec3,
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
    fn an_edge_pushes_no_further_back_than_its_lean_allows() {
        let up = Vec3::Y;
        // The top of a face, met with the front of the tire: nearly level.
        let leaned = edge_lean(Vec3::new(0.0, 0.05, 1.0).normalize(), up);
        assert!(close(leaned.dot(up), EDGE_LEAN_MAX.cos()), "{leaned}");
        assert!(leaned.z > 0.0 && close(leaned.x, 0.0));
        // Ground no steeper than that is left as it is.
        let slope = Vec3::new(0.0, 1.0, 0.3).normalize();
        assert_eq!(edge_lean(slope, up), slope);
    }

    #[test]
    fn a_face_is_met_squarely_by_where_the_tire_goes_or_slowly_by_where_it_points() {
        let wall = Vec3::Z;
        let at_it = Vec3::NEG_Z;
        // Head on, fast or slow.
        assert!(meets_squarely(Vec3::new(0.0, 0.0, -15.0), at_it, wall));
        assert!(meets_squarely(Vec3::new(0.0, 0.0, -2.0), at_it, wall));
        // Pushed into it at a standstill.
        assert!(meets_squarely(Vec3::ZERO, at_it, wall));
        // Sliding fast along it with the tire steered at it: a glance.
        assert!(!meets_squarely(Vec3::new(20.0, 0.0, -1.0), at_it, wall));
        // Sliding slowly along it, driven into it.
        assert!(meets_squarely(Vec3::new(4.0, 0.0, -0.5), at_it, wall));
        // Going away from it.
        assert!(!meets_squarely(Vec3::new(0.0, 0.0, 15.0), Vec3::Z, wall));
    }

    #[test]
    fn a_squash_pushes_only_while_the_tire_is_in_and_never_pulls() {
        let config = TruckConfig::default();
        assert_eq!(edge_squash(&config, 0.0, 5.0), 0.0);
        assert!(edge_squash(&config, 0.1, 0.0) > 0.0);
        assert!(edge_squash(&config, 0.1, 3.0) > edge_squash(&config, 0.1, 0.0));
        // Coming back out fast, the damper takes the push away but never pulls.
        assert_eq!(edge_squash(&config, 0.01, -50.0), 0.0);
    }

    /// Drives half of a truck at a face at `speed`, and returns how far into the face its
    /// tire stops, in metres, and the hardest it is slowed, in m/s².
    fn into_a_face(config: &TruckConfig, speed: f32) -> (f32, f32) {
        let dt = 1.0 / 120.0;
        let (mut depth, mut velocity, mut hardest) = (0.0f32, -speed, 0.0f32);
        let level = Vec3::Z;
        for _ in 0..240 {
            let push = wall_squash(config, depth, level, Vec3::Z * velocity, dt);
            let slowing = push.dot(level) / (config.mass / 2.0);
            hardest = hardest.max(slowing);
            velocity += slowing * dt;
            if velocity >= 0.0 {
                break;
            }
            depth -= velocity * dt;
        }
        (depth, hardest)
    }

    #[test]
    fn a_tire_that_hits_a_wall_is_stopped_evenly_before_the_wall_holds_it() {
        for config in [
            TruckConfig::default(),
            TruckConfig {
                wheel_radius: 0.9144,
                ..TruckConfig::default()
            },
        ] {
            let stop = FACE_STOP * config.wheel_radius;
            for speed in [2.5, 4.0, 8.0, 15.0] {
                let (depth, hardest) = into_a_face(&config, speed);
                // Short of the rigid contact, give or take what one step overshoots by.
                assert!(depth <= 1.01 * stop, "{speed} m/s: {depth} m in");
                assert!(depth < FACE_SQUASH * config.wheel_radius);
                // No harder than twice an even stop over the whole depth.
                let even = speed * speed / (2.0 * stop);
                assert!(
                    hardest <= 2.0 * even,
                    "{speed} m/s: {hardest} against {even}"
                );
            }
        }
    }

    #[test]
    fn a_tire_rides_up_a_wall_from_the_hit_only() {
        let config = TruckConfig::default();
        let dt = 1.0 / 120.0;
        // Hit at 8 m/s: lifted up the wall.
        let hit = wall_squash(&config, 0.02, Vec3::Z, Vec3::new(0.0, 0.0, -8.0), dt);
        assert!(hit.y > 0.0);
        // Pressed against it by the throttle, at a standstill: not lifted at all.
        let pressed = wall_squash(&config, 0.05, Vec3::Z, Vec3::ZERO, dt);
        assert_eq!(pressed.y, 0.0);
        assert!(pressed.z > 0.0);
        // Never more than the tire's grip.
        assert!(hit.y <= config.grip * hit.z + 1e-3);
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
    fn a_slipping_tire_spins_the_way_the_throttle_pushes_and_catches_up_when_it_grips() {
        let dt = 1.0 / 120.0;
        let run = |seconds: f32, speed: f32, ground: f32, throttle: f32| {
            (0..(seconds / dt).round() as usize)
                .fold(speed, |speed, _| tread_on_the_ground(speed, ground, throttle, dt))
        };
        // Sliding back down a hill at 2 m/s under full throttle: the tread goes forwards.
        assert!(close(run(3.0, -2.0, -2.0, 1.0), -2.0 + SLIP_SPIN));
        // Gripping, it rolls with the ground, and soon after slipping.
        assert_eq!(run(1.0, 5.0, 5.0, 0.0), 5.0);
        assert!((run(0.5, 15.0, 5.0, 0.0) - 5.0).abs() < 0.05);
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
    fn the_stroke_rate_is_the_ground_rising_under_the_tire() {
        let up = Vec3::Y;
        // Level ground, driven over level: nothing, whichever way the truck goes.
        assert!(close(
            stroke_rate(Vec3::new(3.0, 0.0, -20.0), Vec3::Y, up),
            0.0
        ));
        // Nose up 10 degrees over level ground: the hub's own speed along the truck's up
        // is the speed times the sine of the pitch, but the tire is not pressed in at all.
        let pitched = Quat::from_rotation_x(10f32.to_radians()) * Vec3::Y;
        let along = Vec3::new(0.0, 0.0, -20.0);
        assert!(close(stroke_rate(along, Vec3::Y, pitched), 0.0));
        assert!(-along.dot(pitched) > 3.0);
        // A slope that rises 1 in 5 ahead, met at 20 m/s: pressed up at 4 m/s.
        let slope = Vec3::new(0.0, 1.0, 0.2).normalize();
        assert!(close(stroke_rate(along, slope, up), 4.0));
        // Falling onto level ground at 3 m/s: pressed up at 3 m/s.
        assert!(close(
            stroke_rate(Vec3::new(0.0, -3.0, 0.0), Vec3::Y, up),
            3.0
        ));
        // A face that holds the tire up little is read no faster than at 45 degrees.
        let face = Vec3::new(0.0, 0.1, 1.0).normalize();
        assert!(stroke_rate(along, face, up) <= along.length() / STEEP_FULL + 1e-3);
    }

    #[test]
    fn the_dampers_answer_the_terrain_less_as_it_steepens() {
        assert_eq!(ground_share(1.0), 1.0);
        assert_eq!(ground_share(STEEP_FULL), 1.0);
        assert_eq!(ground_share(STEEP_NONE), 0.0);
        assert_eq!(ground_share(0.0), 0.0);
        let between = ground_share((STEEP_FULL + STEEP_NONE) / 2.0);
        assert!(close(between, 0.5));
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
