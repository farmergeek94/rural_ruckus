//! The trucks that the computer drives: every `truck::Truck` that is not the
//! `truck::PlayerTruck`. The `truck` slice builds them, from `truck::ComputerTrucks`, and
//! the `race` slice puts them on the grid and counts their laps as it does the player's.
//! This slice is the driver: it writes each one's `TruckInput`, which is all the player's
//! keys do, so a computer truck has no more grip than the player's. It has no more power
//! either, except when it is behind: then its engine is given more, up to `CHASING_POWER`
//! more top speed, so that it catches up on the straights.
//!
//! The driver follows `TrackData::course`, which for a Monster Truck Madness 2 track is the
//! route its own computer trucks follow. It keeps to the road rather than cutting corners:
//! it steers at a point of its own line a little way up the course, and so follows the road
//! round every bend on that line. Near a bend that point is nearer (`BEND_CUT`), so that a
//! truck at speed does not take a chord across the inside of the bend to a point far round it. Each driver's line is the middle of the road or a little to
//! one side of it, so that trucks don't all want the same one. It keeps to a speed that the
//! bends ahead allow, which is the sharper the slower (`bend_speed`): all of it in `plan`, a
//! pure function. Every truck drives the straights flat out, so a driver catches up in the
//! bends instead: the further down the order it is, and the further behind the leader, the
//! more it asks of its tires, the later it brakes, and the more of the road it cuts corners
//! across, never leaving it (`chasing`, `Style::cutting`), until it is back in range.
//! No corner is cut past the checkpoint it has to drive through next, and near one it
//! leaves the course and its line and steers straight through the gate, inside its edges
//! (`NextGate`): the course is only near the gates, and a line, a pass or a cut corner can
//! take a truck round one.
//! One that comes up behind another truck swerves round it instead of slowing down: it pulls
//! out to one side (`passing_line`) and follows that line until it is by. Only boxed in, with
//! no side clear, does it hold station a truck's length behind (`keeping_off`) and press. It
//! remembers how far along the course it is and looks for itself only about there: where a
//! mountain road doubles back, the nearest piece of the course is often the wrong one.
//!
//! A driver that stops against something backs up first, turning the opposite way to the
//! turn it wants, and then goes on (`Recovery`). If it stops again before it has got
//! anywhere, backing up has failed, and it asks the race to put it back at its last
//! checkpoint, as the player does with a key (`race::BackToCheckpoint`). So does one that
//! gets nowhere for a few seconds without ever stopping (on its roof, off down a
//! mountainside). So does one that has gone past its next
//! checkpoint without driving through it. A track with no course has nothing to follow,
//! and its computer trucks stand still.
//!
//! Once the player's truck has finished, a driver takes its wheel as well
//! (`take_the_players_wheel`), and it drives on round the course with the others. It gets
//! `truck::Autopilot`, so that the keys leave it alone.
//!
//! Uses the `track` slice for the course, the `truck` slice for the trucks and the `race`
//! slice for the checkpoints. Needs `RacePlugin`.

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;

use crate::game_state::GameState;
use crate::race::{BackToCheckpoint, RaceSystems, Racer};
use crate::track::{Course, Track};
use crate::truck::{Autopilot, Held, PlayerTruck, Truck, TruckConfig, TruckInput};

/// The speed the quickest driver keeps to where the course is straight, in m/s. The truck
/// can do 38. Bumps throw a truck off a mountain road now and then at this, and the race
/// puts it back.
const CRUISE_SPEED: f32 = 55.0;
/// Each driver keeps to this much less of `CRUISE_SPEED` than the one before, so that the
/// trucks spread out round the course instead of driving in a knot. A driver that falls
/// behind gets the difference back through `catching_up`, so this only spreads a field
/// that is racing together.
const PACE_STEP: f32 = 0.025;
/// Sideways acceleration a driver allows itself in a bend, in m/s². The tires give about 10
/// on level ground, so at this even a driver racing its own race is asking for a little more
/// than there is and runs wide. Higher is quicker through bends, and slides off more of them.
/// Much lower is too slow for the jumps: at 20, trucks on Alpine fell into the ditch after
/// checkpoint 2 and got stuck there, where at 30 they got by.
const CORNERING: f32 = 22.0;
/// Braking a driver counts on when it works out how early to slow for a bend, in m/s²: a
/// little under what the tires give, so it has done its braking by the bend and drives
/// through it. Measured on Alpine, one truck from sixteen starts to the foot of the ravine
/// after checkpoint 6: at 7 it got there in 302 s on average and rolled 13 times in all; at
/// 10, arriving hot, in 346 s with 30 rolls; at 14, in 399 s with 92.
const BRAKING: f32 = 7.0;
/// No bend is taken slower than this, in m/s.
const SLOWEST_BEND: f32 = 16.0;
/// A bend is measured as the change in the course's direction over this many metres.
const BEND_SPAN: f32 = 30.0;
/// How far apart the places are at which the course ahead is looked at for bends, in metres.
const BEND_STEP: f32 = 10.0;
/// How far ahead a driver looks for bends to slow for, in metres.
const HORIZON: f32 = 250.0;
/// How far up its line a driver steers at, in seconds of driving, and its limits in metres.
/// Aimed at from inside a bend, a point further round it is nearer across the inside, so the
/// further this is the more of every bend the truck cuts. Shorter keeps it nearer its line
/// and weaves more, and swerves more sharply.
const AIM_AHEAD_SECONDS: f32 = 1.7;
const AIM_AHEAD: std::ops::RangeInclusive<f32> = 20.0..=50.0;
/// How far up its line a driver steers at near a bend, as a length in metres: a bend of
/// radius `R` within `AIM_AHEAD_SECONDS` of driving brings the aim point in to
/// `sqrt(2 R BEND_CUT)`, and never nearer than the start of `AIM_AHEAD`. Without it a driver
/// at speed steered at a point up to 50 m round a bend, and in bends it was 8 to 9 m inside
/// its line, at the edge of the road, three times in four. Measured with a model truck on
/// five Monster Truck Madness 2 tracks: at 16, 4 m; at 10, 2.5 m, but then it turned in late
/// and ran wide off the outside of more bends than before. Smaller cuts less of every bend.
const BEND_CUT: f32 = 16.0;
/// Full lock is used when the aim point is this far off straight ahead, in radians. Smaller
/// is a driver that turns sooner and harder for the same aim point, and weaves more on a
/// straight; at 0.3 it is at full lock 17 degrees off.
const FULL_LOCK_ANGLE: f32 = 0.3;
/// Further off straight ahead than this, in radians, the truck is pointing the wrong way and
/// slows down to turn round. It does not follow `FULL_LOCK_ANGLE`, which is how hard it
/// steers rather than what counts as facing the right way.
const WRONG_WAY_ANGLE: f32 = 1.0;
/// A bend that turns this much over a `BEND_SPAN` is as sharp as a driver reckons with, in
/// radians: a right angle in 30 m.
const SHARP_BEND: f32 = std::f32::consts::FRAC_PI_2;
/// How much of its cornering a driver asks for in the sharpest bends, as a share. It spends
/// the lot in a long sweep, where running wide costs nothing but a wide exit, and this much
/// of it in a hairpin, where running wide costs the corner and the truck. In between it is in
/// proportion to how sharp the bend is, so the sharper the turn the slower it goes through it.
const SHARP_BEND_CARE: f32 = 0.5;
/// How many m/s over or under its speed a driver is at full brake or full throttle.
const THROTTLE_BAND: f32 = 3.0;
/// The lines that drivers take are this far apart across the course, in metres.
const LANE_SPACING: f32 = 2.5;
/// How far inside the edge of a checkpoint gate a driver keeps its outer wheels, in metres,
/// as it drives through. Larger keeps trucks nearer the middle of a gate, and bunches them
/// up there; at 0 a truck knocked a little sideways at the last moment misses the gate.
const GATE_MARGIN: f32 = 1.0;

/// A driver no further behind the truck leading the race than this, in metres, is in range of
/// it. The player counts as a truck to catch like any other.
const IN_RANGE: f32 = 50.0;
/// A driver this far behind the leader, in metres, is as desperate as its place in the race
/// alone would make it. In between it gets the share of that distance it is behind by.
const CHASING_BY: f32 = 500.0;
/// How much more sideways acceleration a driver asks of its tires in a bend when it is as
/// desperate as it gets, as a share of `CORNERING`: a little, so that it is only slightly
/// quicker through the bends. It catches up on the straights instead (`CHASING_POWER`).
const CHASING_CORNER: f32 = 0.2;
/// How much more braking it counts on then, as a share of `BRAKING`: it leaves its braking
/// that much later, and arrives in the bend that much hotter.
const CHASING_BRAKE: f32 = 0.15;
/// How much more top speed a truck has when its driver is as desperate as it gets, as a
/// share: its engine's pull and the speed at which the pull runs out both go up by this,
/// which raises the speed it reaches on the level by the same share. In between, in
/// proportion, and none for the leader. This is the catch-up: a truck that is behind has more
/// power than the player's, and loses it again as it gets back in range.
const CHASING_POWER: f32 = 0.15;
/// How far ahead a driver looks out for a truck to pass, in metres, along the course and
/// in a straight line: about six truck lengths, which is far enough to move aside before
/// it arrives behind one.
const LOOK_OUT: f32 = 45.0;
/// Daylight a driver wants beside another truck, in metres, on top of what the two of them
/// reach across the course. Every sideways figure below is worked out from the trucks
/// themselves, wheels included (`reach`), so that clear means clear and not touching, and so
/// that a Monster Truck Madness 2 truck of any size is measured as itself.
const ROOM_TO_SPARE: f32 = 0.7;
/// How much of the length at which two trucks are clear of each other another truck's middle
/// has to be up the road before it is in front of the driver rather than beside it: at eight
/// tenths they still overlap a little, and it is nearly past the driver's nose. Nearer than
/// this the two are alongside, and neither gives way to the other: neither waits behind the
/// other, neither shuts the other's door, and neither treats the other as passed. Two that
/// each took the other for the truck in front would both stop and stay stopped, and one that
/// let a truck level with it count as in front would be boxed in by a truck it is beside.
const IN_FRONT: f32 = 0.8;
/// How fast a driver moves its line across the course, in m/s. Faster pulls out in less
/// road; slower keeps the steering from snapping over when a truck appears in front.
const LANE_RATE: f32 = 3.0;
/// Daylight a driver leaves between itself and a truck it cannot get by, in metres, on top of
/// the length at which the two are clear of each other: close enough to press, and not into
/// it. It holds this and does not drop back from it.
const PRESSURE_ROOM: f32 = 2.5;
/// Daylight between a driver and a truck alongside it, in metres, on top of what the two of
/// them need to be clear of each other, within which the driver holds its line rather than
/// turning in for a corner: about as far as a corner's line moves it across. Further apart
/// than this, turning in cannot reach the other truck, and each takes its corner as it would
/// alone. Larger holds more trucks to their lines, and pairs of them run wide together.
const SIDE_BY_SIDE: f32 = 7.0;

/// How far above its ride height a truck's middle has to be, in metres, for its driver to
/// take it to be in the air, and how long after it comes down it leaves the steering alone,
/// in seconds. Steering hard while a truck flies or lands, when an impact has just spun it,
/// is what rolls it: from eight starts on Alpine, 68 of 77 rolls were at the ditch after
/// checkpoint 2, and with the wheel held straight through the ditch, 10. Bumps
/// and crests lift a truck on the road by up to a metre, so less than that takes the
/// steering away from a driver on the ground.
const AIRBORNE: f32 = 2.0;
const SETTLE: f32 = 0.5;

/// A driver that is trying to go but has been slower than `STOPPED_SPEED` m/s for
/// `STOPPED_FOR` seconds is stuck, against a wall, a tree or another truck, and backs up.
const STOPPED_SPEED: f32 = 1.0;
const STOPPED_FOR: f32 = 1.0;
/// How long a driver backs up for, in seconds: at least the first, so that it gets clear of
/// what stopped it, and at most the second, where it gives up turning and tries forwards.
/// In between it stops as soon as it faces the way it wants to go, within `FACING`.
const BACK_UP: std::ops::RangeInclusive<f32> = 0.3..=1.0;
/// How near straight ahead the way it wants to go must be for a driver backing up to have
/// turned far enough, as a share of full lock (`FULL_LOCK_ANGLE`): at 0.5, within 9 degrees.
const FACING: f32 = 0.5;
/// The throttle a driver backs up at: reverse, a little short of full.
const REVERSING: f32 = -0.8;

/// How far behind and ahead of where it last was a driver looks for itself, in metres.
const BEHIND: f32 = 20.0;
const AHEAD: f32 = 60.0;
/// A driver that has not got this far along the course, in metres, within `PATIENCE`
/// seconds is stuck, whatever it is doing. The slowest bend is driven at 7 m/s.
const PROGRESS: f32 = 10.0;
const PATIENCE: f32 = 10.0;
/// Further than this from the course, in metres, is no progress either, wherever along
/// the course the nearest piece of it is.
const LOST_DISTANCE: f32 = 45.0;
/// A driver this far along the course past a checkpoint that it still has to drive
/// through, in metres, has missed it.
const MISSED_BY: f32 = 40.0;

pub struct OpponentsPlugin;

impl Plugin for OpponentsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                take_the_wheel,
                take_the_players_wheel,
                // So that the race sees the request in the same frame.
                ask_to_be_put_back.before(RaceSystems::BackToCheckpoint),
            )
                .run_if(in_state(GameState::Racing)),
        )
        // With the forces, so that a driver does the same at any frame rate.
        .add_systems(FixedUpdate, drive.run_if(in_state(GameState::Racing)));
    }
}

/// On every truck that the computer drives.
#[derive(Component, Debug)]
pub struct ComputerDriver {
    /// How much of `CRUISE_SPEED` this driver keeps to when it is up with the race, from
    /// 0 to 1. Behind, it drives above this: see `catching_up`.
    pace: f32,
    /// The line this driver takes when the road in front of it is clear: how far to the
    /// left of the centreline, in metres.
    home: f32,
    /// The line it is on now, which moves aside to pass what is in front of it.
    lane: f32,
    /// How far along the course's centreline the truck was last found, in metres.
    /// `None` until it has been looked for, which is then done over the whole course.
    along: Option<f32>,
    /// Where along the course the truck was when it last got somewhere, in metres, and
    /// how long ago that was, in seconds.
    got_to: f32,
    getting_nowhere: f32,
    /// How much longer, in seconds, it leaves the steering alone after flying (`SETTLE`).
    settling: f32,
    /// Getting unstuck, by backing up and turning round (`Recovery`).
    recovery: Recovery,
    /// Its truck's own engine, before `CHASING_POWER` is added: its pull in newtons and the
    /// speed where that runs out in m/s. Taken the first time the driver drives.
    engine: Option<(f32, f32)>,
}

impl ComputerDriver {
    /// Driver `number` of a race, from 0: each a little slower than the driver numbered
    /// before it, on the middle line, the left and the right by turns.
    fn new(number: usize) -> Self {
        let home = LINES[number % LINES.len()];
        Self {
            pace: (1.0 - PACE_STEP * number as f32).max(0.5),
            home,
            lane: home,
            along: None,
            got_to: 0.0,
            getting_nowhere: 0.0,
            settling: 0.0,
            recovery: Recovery::default(),
            engine: None,
        }
    }
}

fn take_the_wheel(
    mut commands: Commands,
    trucks: Query<Entity, Undriven>,
    drivers: Query<(), With<ComputerDriver>>,
) {
    let mut trucks: Vec<Entity> = trucks.iter().collect();
    // The same driver for the same truck, race after race.
    trucks.sort();
    let taken = drivers.iter().count();
    for (index, truck) in trucks.into_iter().enumerate() {
        commands
            .entity(truck)
            .insert(ComputerDriver::new(taken + index));
    }
}

/// The player's truck while the player still drives it.
type PlayersHands = (With<PlayerTruck>, Without<ComputerDriver>);

/// Once the player's truck has finished, drives it on as another driver, the slowest.
fn take_the_players_wheel(
    mut commands: Commands,
    player: Query<(Entity, &Racer), PlayersHands>,
    drivers: Query<(), With<ComputerDriver>>,
) {
    for (truck, racer) in &player {
        if racer.progress.finished.is_some() {
            commands
                .entity(truck)
                .insert((Autopilot, ComputerDriver::new(drivers.iter().count())));
        }
    }
}

/// The lines drivers take, in turn: the middle, the left and the right.
const LINES: [f32; 3] = [0.0, LANE_SPACING, -LANE_SPACING];

/// The line after `home` in `LINES`, for a driver that is trying again.
fn next_line(home: f32) -> f32 {
    let at = LINES.iter().position(|line| *line == home).unwrap_or(0);
    LINES[(at + 1) % LINES.len()]
}

/// A truck that nobody drives: not the player, and not a computer driver yet.
type Undriven = (With<Truck>, Without<PlayerTruck>, Without<ComputerDriver>);

/// What a driver does with the controls, as `TruckInput` has them.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Controls {
    /// -1 (full brake) to 1 (full throttle).
    throttle: f32,
    /// -1 (right) to 1 (left).
    steer: f32,
}

/// A driver getting its truck unstuck: when it has been trying to go and getting nowhere, it
/// backs up with the wheel turned the opposite way to the turn it wants, which swings the nose
/// round towards where it wants to go, and then drives forwards again, turning the right way.
/// A truck against a wall, or nose into a tree, gets off it and round it like this instead of
/// pressing on until it is put back. It gets one go: stopped again before it has got anywhere
/// (`Recovery::got_somewhere`), it gives up, and is put back at its last checkpoint.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Recovery {
    /// How long it has been trying to go and slower than `STOPPED_SPEED`, in seconds.
    stopped_for: f32,
    /// How long it has been backing up, in seconds, while it is.
    backing_up: Option<f32>,
    /// Which way it steers while backing up: 1 left, -1 right.
    back_steer: f32,
    /// Whether it has backed up since it last got somewhere.
    backed_up: bool,
    /// Whether it has stopped again after backing up, which is to be put back.
    gave_up: bool,
}

impl Recovery {
    /// The truck has got somewhere since it last backed up, and gets another go next time.
    fn got_somewhere(&mut self) {
        self.backed_up = false;
    }

    /// The controls to use, from the ones the driver `wanted` for going forwards, with the
    /// truck going at `speed` m/s along its nose (negative backwards), `dt` seconds on.
    fn step(&mut self, wanted: Controls, speed: f32, dt: f32) -> Controls {
        if let Some(backed) = self.backing_up.as_mut() {
            *backed += dt;
            let turned = *backed >= *BACK_UP.start() && wanted.steer.abs() < FACING;
            if turned || *backed >= *BACK_UP.end() {
                self.backing_up = None;
                return wanted;
            }
            return Controls {
                throttle: REVERSING,
                steer: self.back_steer,
            };
        }
        let trying = wanted.throttle > 0.0;
        self.stopped_for = if trying && speed.abs() < STOPPED_SPEED {
            self.stopped_for + dt
        } else {
            0.0
        };
        if self.stopped_for < STOPPED_FOR {
            return wanted;
        }
        self.stopped_for = 0.0;
        if self.backed_up {
            // Backing up did not get it going: it waits where it is to be put back.
            self.gave_up = true;
            return wanted;
        }
        self.backed_up = true;
        self.backing_up = Some(0.0);
        // Opposite to the turn it wants; with none to speak of, it has driven straight into
        // something, and either way round will do.
        self.back_steer = if wanted.steer > 0.0 { -1.0 } else { 1.0 };
        Controls {
            throttle: REVERSING,
            steer: self.back_steer,
        }
    }
}

/// How a driver is driving: the numbers that tell `plan` what it is trying to do.
#[derive(Clone, Copy, Debug)]
struct Style {
    /// How much of `CRUISE_SPEED` it keeps to where the course is straight, from 0 to 1.
    /// Chasing does not change this: the straights are driven flat out either way.
    pace: f32,
    /// The line it is taking: how far to the left of the centreline, in metres.
    lane: f32,
    /// How desperate it is, from 0 to 1, as `chasing` works it out. It asks more of the tires
    /// in a bend and brakes later for one.
    chasing: f32,
    /// How far its own truck reaches sideways from its middle, in metres, wheels included:
    /// what says how near the edge of the course its line may go to pass.
    reach_across: f32,
    /// How much of the road it cuts corners across, from 0 (none: it follows its line) to 1
    /// (all of it, its outer wheels at the edge). A driver that is behind cuts them as it is
    /// desperate (`chasing`), and none while it is passing or has a truck beside it.
    cutting: f32,
    /// The checkpoint it has to drive through next, if it has one: it cuts no corner past
    /// it, and near it steers through it (`Ahead::aim`).
    gate: Option<NextGate>,
}

/// The checkpoint gate a driver has to drive through next, as `Ahead::aim` steers for it.
#[derive(Clone, Copy, Debug)]
struct NextGate {
    /// How far up the course from the truck it is, in metres, as near as the course
    /// passes to its middle. Negative once the truck is past that.
    ahead: f32,
    /// Its middle on the ground plane, in metres.
    center: Vec2,
    /// The way through it, a unit vector on the ground plane.
    direction: Vec2,
    /// Half its width, in metres.
    half_width: f32,
}

impl NextGate {
    /// Where a driver on a line `lane` metres to the left of the centreline, that reaches
    /// `reach_across` metres sideways, steers at when it is nearer the gate than it steers
    /// ahead (`reach`): a point on a line straight through the gate, the rest of `reach`
    /// beyond it. Its line is kept inside the gate, its outer wheels `GATE_MARGIN` from the
    /// edge. So it lines up with the gate and drives through it square, whatever the course
    /// does here and wherever the gate stands on it.
    fn aim(&self, lane: f32, reach: f32, reach_across: f32) -> Vec2 {
        let room = (self.half_width - reach_across - GATE_MARGIN).max(0.0);
        let beyond = (reach - self.ahead).max(0.0);
        self.center - self.direction.perp() * lane.clamp(-room, room) + self.direction * beyond
    }
}

/// What to do with the controls of a truck that is `along` metres round the `course`, at
/// `position` on the ground plane, facing `forward` (a unit vector) at `speed` m/s, for a
/// driver with this `style`.
fn plan(
    course: &Course,
    along: f32,
    position: Vec2,
    forward: Vec2,
    speed: f32,
    style: Style,
) -> Controls {
    let ahead = Ahead::of(course, along, style.reach_across);
    // Nothing nearer than this is steered at: further when going faster.
    let reach = (speed * AIM_AHEAD_SECONDS).clamp(*AIM_AHEAD.start(), *AIM_AHEAD.end());
    let aim = ahead.aim(course, position, ahead.steering_reach(reach), style);
    let to_aim = (aim - position).normalize_or_zero();
    // Positive when the aim point is to the left.
    let off_straight = (-forward.perp_dot(to_aim)).atan2(forward.dot(to_aim));
    let steer = (off_straight / FULL_LOCK_ANGLE).clamp(-1.0, 1.0);

    // The speed that every bend within braking distance allows here and now. A driver that
    // is chasing asks more of its tires in the bend and counts on more of its brakes to get
    // there, which is the caution it is throwing away to catch up.
    let cornering = CORNERING * (1.0 + CHASING_CORNER * style.chasing);
    let braking = BRAKING * (1.0 + CHASING_BRAKE * style.chasing);
    // The tightest bends are held to a floor, which a desperate driver raises with the rest
    // of its caution: it takes a hairpin at a speed it cannot hold, and often does not.
    let slowest_bend = SLOWEST_BEND * (1.0 + CHASING_CORNER * style.chasing);
    let cruise = CRUISE_SPEED * style.pace;
    let braking_distance = speed * speed / (2.0 * braking) + reach;
    let mut allowed = cruise;
    for i in 0..ahead.bends() {
        let distance = ahead.distance(i);
        if distance > braking_distance {
            break;
        }
        let bend = ahead.bend(i).abs();
        if bend > 1e-3 {
            let in_the_bend = bend_speed(bend, cornering, slowest_bend);
            // Going this fast here, the brakes bring it down to that by there.
            let here = (in_the_bend * in_the_bend + 2.0 * braking * distance).sqrt();
            allowed = allowed.min(here);
        }
    }
    // Pointing the wrong way is a bend too, whatever the course does.
    if off_straight.abs() > WRONG_WAY_ANGLE {
        allowed = allowed.min(SLOWEST_BEND);
    }

    Controls {
        throttle: throttle_for(allowed, speed),
        steer,
    }
}

/// The speed a bend of `bend` radians over a `BEND_SPAN` allows, in m/s, for a driver that
/// would spend `cornering` m/s² of its tires in a long sweep and takes no bend slower than
/// `slowest`. The sharper the bend the less of that it asks for (`SHARP_BEND_CARE`), so that
/// it slows for a hairpin by more than the radius alone would have it: a slide in a sweep
/// costs a wide exit, and a slide in a hairpin costs the corner.
fn bend_speed(bend: f32, cornering: f32, slowest: f32) -> f32 {
    let bend = bend.abs().max(1e-3);
    let sharpness = (bend / SHARP_BEND).clamp(0.0, 1.0);
    let asked_for = cornering * (1.0 - (1.0 - SHARP_BEND_CARE) * sharpness);
    (asked_for * BEND_SPAN / bend).sqrt().max(slowest)
}

/// How desperate a driver is, from 0 (the leader, driving its own race with its own care) to
/// 1 (throwing its caution away to get on terms). It buys nothing on the straights, which
/// every truck drives flat out: a desperate driver catches up by carrying more speed into the
/// bends and braking later for them, both of which risk the truck. See `Style`.
///
/// Its `place` in the race sets it, from 1 for the lead to `field` for the last of them, so
/// that each truck down the order is a little more desperate than the one in front of it. A
/// driver further behind the leader than `IN_RANGE` is as desperate again as the ground it
/// has to make up, whatever its place: second by half a lap is a lost race too.
fn chasing(place: usize, field: usize, behind: f32) -> f32 {
    let by_place = (place.max(1) - 1) as f32 / (field.max(2) - 1) as f32;
    let by_ground = (behind - IN_RANGE) / (CHASING_BY - IN_RANGE);
    by_place.max(by_ground).clamp(0.0, 1.0)
}

/// The line a driver wants: its `home` line when the road in front is clear, and otherwise
/// `PASSING_ROOM` to one side of the nearest truck in the way, so that it pulls out to pass
/// rather than sitting behind, and stays out until it is by. It goes to the side it is
/// already on, and to the other when the road runs out there or another truck holds it.
/// Which side of a truck it is on is where it actually is, `at` metres to the left of the
/// centreline, not where its `lane` is: the two part at the start, where a grid can stand
/// well off the course, and after a knock, and a driver that went by its lane would cross
/// in front of the truck it means to pass.
/// Boxed in, it holds the `lane` it is on and waits for a way through, which is what
/// `keeping_off` then keeps it a truck's length from. Whatever line it settles on, it is
/// held off any truck alongside it by `giving_room`, so that it never steers into one.
///
/// `others` holds every other truck near enough to matter. `room` is how far either side of
/// the centreline a line may be, as `passing_room` says.
fn passing_line(home: f32, lane: f32, at: f32, others: &[Neighbour], room: f32) -> f32 {
    giving_room(at, wanted_line(home, lane, at, others, room), others, room)
}

/// Another truck, as the driver's rules see it: where it is in relation to the driver, and how
/// much room the two of them need to be clear of each other. That room is worked out from both
/// trucks, wheels and all (`reach`), so a driver beside a big truck gives it more than a small
/// one, and nothing here assumes every truck is the size of the built-in one.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Neighbour {
    /// How far ahead along the course it is, in metres. Behind is negative.
    ahead: f32,
    /// How far to the left of the centreline it is, in metres.
    across: f32,
    /// How fast it is going along the course, in m/s.
    speed: f32,
    /// How far apart across the course the two of them have to be to pass each other, in
    /// metres: what each reaches sideways, wheels included, and `ROOM_TO_SPARE`.
    clear_by: f32,
    /// How far apart along the course their middles have to be for one to be clear of the
    /// other, in metres: what each reaches fore and aft, tires included.
    past_by: f32,
}

impl Neighbour {
    /// Whether it is alongside the driver: their middles are nearer than the length at which
    /// they are clear of each other, so the two overlap, and moving across the course towards
    /// it would run into it whatever their lines say.
    fn alongside(&self) -> bool {
        self.ahead.abs() < self.past_by
    }

    /// Whether it is alongside a driver that is `across` metres to the left of the
    /// centreline, and near enough across the course that turning in would run into it: no
    /// more than `SIDE_BY_SIDE` further off than the two need to be clear.
    fn side_by_side(&self, across: f32) -> bool {
        self.alongside() && (self.across - across).abs() < self.clear_by + SIDE_BY_SIDE
    }

    /// Whether it is in front of the driver rather than beside it: `IN_FRONT` of the way to
    /// being clear of it, and no further up the road than a driver looks out.
    fn in_front(&self) -> bool {
        (IN_FRONT * self.past_by..=LOOK_OUT).contains(&self.ahead)
    }
}

/// A `wanted` line held off every truck that is alongside: it may not be taken nearer than
/// `clear_by` across the course to one, on the side the driver, `at` metres to the left of the
/// centreline, is already passing it. So a
/// driver whose way back to its own line is beside another truck does not turn in on it: it
/// runs alongside until the truck is clear, and then goes home. Nothing is pushed further out
/// than `room`, because beyond that there is no road.
fn giving_room(at: f32, wanted: f32, others: &[Neighbour], room: f32) -> f32 {
    let held = others
        .iter()
        .filter(|other| other.alongside())
        .fold(wanted, |line, other| {
            // Away from it, on the side the driver is already on.
            if at >= other.across {
                line.max(other.across + other.clear_by)
            } else {
                line.min(other.across - other.clear_by)
            }
        });
    held.clamp(-room, room)
}

/// The line a driver `at` metres to the left of the centreline steers by, from the `lane` it
/// is on: no nearer any truck side by side with it (`Neighbour::side_by_side`) than it is
/// already. Its lane moves across at `LANE_RATE`, and is often still on the far side of such a
/// truck, at the start above all, where a grid can stand well off the course and every truck
/// makes for its line at once; steering for it would take the driver into that truck. So,
/// beside one, it runs parallel to the course until the truck is clear, and then carries on.
fn not_towards(lane: f32, at: f32, others: &[Neighbour]) -> f32 {
    others
        .iter()
        .filter(|other| other.side_by_side(at))
        .fold(lane, |line, other| {
            if other.across > at {
                line.min(at)
            } else {
                line.max(at)
            }
        })
}

/// The line `passing_line` wants before it is held off the trucks alongside.
fn wanted_line(home: f32, lane: f32, at: f32, others: &[Neighbour], room: f32) -> f32 {
    let in_the_way = others
        .iter()
        .filter(|other| {
            // In the way of the line it is on, or of the line it came from: a truck it has
            // pulled out for is exactly `clear_by` away and so clear of the line it is on
            // now, and letting it go at that would take the driver straight back in. One
            // that is not yet `IN_FRONT` behind is still alongside, and cutting back across
            // it is how a pass turns into a crash.
            (-IN_FRONT * other.past_by..=LOOK_OUT).contains(&other.ahead)
                && ((other.across - lane).abs() < other.clear_by
                    || (other.across - home).abs() < other.clear_by)
        })
        .min_by(|a, b| a.ahead.total_cmp(&b.ahead));
    let Some(blocking) = in_the_way else {
        return home;
    };
    let across = blocking.across;
    let beside = |side: f32| across + side * blocking.clear_by;
    // A side to move to must be road, and nothing may be overlapping the driver on it: a
    // truck it is level with is a door it cannot open. One that is `IN_FRONT` up the road,
    // or that far behind, is clear of it and shuts nothing; it is only the next truck to
    // pass, or one that has been passed. The truck being passed sits `PASSING_ROOM` away,
    // which is wider than a truck, so it never shuts its own door either.
    let clear = |side: f32| {
        let line = beside(side);
        line.abs() <= room
            && !others
                .iter()
                .any(|other| other.alongside() && (other.across - line).abs() < other.clear_by)
    };
    let side = if at >= across { 1.0 } else { -1.0 };
    match (clear(side), clear(-side)) {
        (true, _) => beside(side),
        (false, true) => beside(-side),
        (false, false) => lane,
    }
}

/// The truck a driver `across` metres to the left of the centreline has to slow for, if any:
/// the nearest one it is behind and still square with, by where the two of them actually are,
/// which it would run into. Once it has moved far enough across, or far enough up beside it,
/// there is nobody in front of it. Nor is there while the line it `wanted` is clear of that
/// truck: it is on its way round it, and swerves rather than brakes. Only a driver boxed in,
/// whose line is still behind the truck, slows to hold station there.
fn held_up_by(others: &[Neighbour], across: f32, wanted: f32) -> Option<Neighbour> {
    others
        .iter()
        .filter(|other| {
            other.in_front()
                && (other.across - across).abs() < other.clear_by
                && (other.across - wanted).abs() < other.clear_by
        })
        .min_by(|a, b| a.ahead.total_cmp(&b.ahead))
        .copied()
}

/// How fast a driver may go without driving into the back of the truck `in_front` of it, in
/// m/s: the speed it can brake off again by the time it is up behind that truck, which is the
/// same sum as slowing for a bend. Where that is comes from the two trucks themselves, tires
/// included, and `PRESSURE_ROOM`. Nothing in front is no limit at all, and nearer than that it
/// asks for the other truck's own speed, so it sits there and presses rather than dropping
/// back.
fn keeping_off(in_front: Option<Neighbour>) -> f32 {
    let Some(other) = in_front else {
        return f32::INFINITY;
    };
    let room_to_brake = (other.ahead - other.past_by - PRESSURE_ROOM).max(0.0);
    (other.speed * other.speed + 2.0 * BRAKING * room_to_brake).sqrt()
}

/// The throttle, from -1 (full brake) to 1 (full throttle), that a driver doing `speed`
/// uses to get to `wanted`, both in m/s.
fn throttle_for(wanted: f32, speed: f32) -> f32 {
    ((wanted - speed) / THROTTLE_BAND).clamp(-1.0, 1.0)
}

/// How far a truck has raced, in metres: the laps it has driven and how far it is round the
/// course from the start line, which is `start_line` metres along the course. `lap_number`
/// counts from 1 on the lap the start line begins, and is 0 on the grid, where the truck is
/// at the end of a lap it has not begun; the two meet at the line, so the figure grows
/// evenly through it.
fn raced(along: f32, start_line: f32, lap_number: u32, lap: f32) -> f32 {
    (lap_number as f32 - 1.0) * lap + (along - start_line).rem_euclid(lap)
}

/// How far ahead along a course of `lap` metres a `difference` between two trucks' distance
/// along it is: negative for a truck behind, since a course comes round again.
fn gap(difference: f32, lap: f32) -> f32 {
    let ahead = difference.rem_euclid(lap);
    if ahead > lap / 2.0 {
        ahead - lap
    } else {
        ahead
    }
}

/// The room a line has when it is passing, in metres: out to where a truck that reaches
/// `reach_across` metres sideways from its middle has its outer wheels at the edge of the
/// course.
fn passing_room(course: &Course, reach_across: f32) -> f32 {
    (course.width / 2.0 - reach_across).max(0.0)
}

/// Where a truck is round the course, worked out before any driver is given its controls:
/// one place for every truck in the race, the player's included.
struct OnCourse {
    entity: Entity,
    /// Where it is on the ground plane, in metres.
    position: Vec2,
    /// How far along the centreline it is, in metres.
    along: f32,
    /// How far from the centreline it is, in metres.
    off: f32,
    /// How far to the left of the centreline it is, in metres, as a driver's line is.
    across: f32,
    /// How fast it is going along the course, in m/s. Negative if it is going backwards.
    speed: f32,
    /// How far it reaches from its middle, in metres, as `reach` works it out.
    reach: Vec2,
    /// How far it has raced since the start line, laps included, in metres.
    raced: f32,
}

/// How far a truck reaches from its middle, in metres, with its wheels: `x` across it and
/// `y` along it. A wheel collider is a cylinder on the axle, so a tire reaches its own half
/// width outwards from the hub and its radius fore and aft of it. The body is inside that on
/// the built-in truck; a Monster Truck Madness 2 one is measured the same way from its own
/// wheels.
fn reach(config: &TruckConfig) -> Vec2 {
    config.wheel_rest.iter().fold(Vec2::ZERO, |reach, wheel| {
        reach.max(Vec2::new(
            wheel.x.abs() + config.wheel_width / 2.0,
            wheel.z.abs() + config.wheel_radius,
        ))
    })
}

/// What a truck's place round the course is worked out against: the course, the length of a
/// lap of it, and how far along it the start line is.
struct Places<'a> {
    course: &'a Course,
    lap: f32,
    start_line: f32,
}

impl Places<'_> {
    /// Where a truck at `position` going `velocity` is. It is looked for near where it was
    /// `last` seen, when that is known: where a mountain road doubles back, the nearest
    /// piece of the course is often the wrong one. `lap_number` is the lap it is on, which
    /// counts from the start line, so that the distance raced counts from there too.
    fn of(
        &self,
        entity: Entity,
        position: Vec2,
        velocity: Vec2,
        reach: Vec2,
        last: Option<f32>,
        lap_number: u32,
    ) -> OnCourse {
        let course = self.course;
        let nearest = match last {
            Some(along) => course.nearest_within(position, along - BEHIND, along + AHEAD),
            None => course.nearest(position),
        };
        let along = course.distance_along(&nearest);
        let (point, direction) = course.point_at(along);
        OnCourse {
            entity,
            position,
            along,
            off: nearest.distance,
            // A line `across` to the left is `point - direction.perp() * across`.
            across: -(position - point).dot(direction.perp()),
            speed: velocity.dot(direction),
            reach,
            raced: raced(along, self.start_line, lap_number, self.lap),
        }
    }
}

/// The course ahead of a truck: its position and direction of travel every `BEND_STEP`
/// metres from where the truck is, to `HORIZON` and a `BEND_SPAN` beyond, so that a bend
/// at the horizon can be measured.
struct Ahead {
    /// How far along the course the truck is, in metres.
    along: f32,
    samples: Vec<(Vec2, Vec2)>,
    /// How many samples a `BEND_SPAN` is.
    span: usize,
    /// How far out a line may go, in metres, as `passing_room` works it out.
    passing_room: f32,
}

impl Ahead {
    fn of(course: &Course, along: f32, reach_across: f32) -> Self {
        let span = (BEND_SPAN / BEND_STEP).round() as usize;
        let count = (HORIZON / BEND_STEP).round() as usize + span + 1;
        Self {
            along,
            samples: course.sample_ahead(along, BEND_STEP, count),
            span,
            passing_room: passing_room(course, reach_across),
        }
    }

    /// How far along the course sample `i` is, in metres.
    fn distance(&self, i: usize) -> f32 {
        i as f32 * BEND_STEP
    }

    /// How many samples have a bend measured from them.
    fn bends(&self) -> usize {
        self.samples.len() - self.span
    }

    /// The course's change of direction over the `BEND_SPAN` from sample `i`, in radians,
    /// positive to the right.
    fn bend(&self, i: usize) -> f32 {
        self.samples[i].1.angle_to(self.samples[i + self.span].1)
    }

    /// How far up its line a driver steers at, in metres: `reach`, or nearer where a bend
    /// within it would take the truck far across the inside of it (`BEND_CUT`).
    fn steering_reach(&self, reach: f32) -> f32 {
        let mut nearest = reach;
        for i in 0..self.bends() {
            if self.distance(i) > reach {
                break;
            }
            let bend = self.bend(i).abs();
            if bend > 1e-3 {
                let radius = BEND_SPAN / bend;
                nearest = nearest.min((2.0 * radius * BEND_CUT).sqrt());
            }
        }
        nearest.max(*AIM_AHEAD.start()).min(reach)
    }

    /// Where a driver of this `style` at `position` steers at: the point of its line `reach`
    /// metres up the course. So it follows the road round every bend on its line, and a truck
    /// knocked off its line, or off the road, heads back to it within about that distance. A
    /// line is held to the road, however far out passing would take it.
    ///
    /// A driver that cuts corners (`Style::cutting`) steers further up its line instead, as far
    /// as a straight line from the truck reaches without leaving its share of the road, which
    /// through a bend is a chord across the inside of it. That share is never more than the
    /// road: its outer wheels at the edge, and not over it.
    ///
    /// Its next checkpoint (`Style::gate`) comes first: nearer than `reach`, it steers
    /// through the gate (`NextGate::aim`), and further, it cuts no corner beyond the gate. A
    /// chord across the inside of a bend with a gate in it goes round the gate.
    fn aim(&self, course: &Course, position: Vec2, reach: f32, style: Style) -> Vec2 {
        let lane = style.lane.clamp(-self.passing_room, self.passing_room);
        if let Some(gate) = style.gate
            && gate.ahead <= reach
        {
            return gate.aim(lane, reach, style.reach_across);
        }
        let (point, direction) = course.point_at(self.along + reach);
        let mut aim = point - direction.perp() * lane;
        if style.cutting <= 0.0 {
            return aim;
        }
        // From its own line, where cutting nothing leaves it, out to the edge of the road.
        let room = lane.abs() + (self.passing_room - lane.abs()) * style.cutting.min(1.0);
        for i in 1..self.bends() {
            let distance = self.distance(i);
            if distance <= reach {
                continue;
            }
            if style.gate.is_some_and(|gate| distance > gate.ahead) {
                break;
            }
            let (point, direction) = self.samples[i];
            let target = point - direction.perp() * lane;
            if !self.is_clear(position, target, distance, room) {
                break;
            }
            aim = target;
        }
        aim
    }

    /// Whether a straight line from `position` to `target`, which is `distance` metres along
    /// the course, keeps within `room` of the centreline, judged at every sample on the way.
    /// The sample beside the truck is not on the way, so a truck that has been knocked out to
    /// the edge of the road still gets to look ahead.
    fn is_clear(&self, position: Vec2, target: Vec2, distance: f32, room: f32) -> bool {
        let before = (distance / BEND_STEP).ceil() as usize;
        self.samples[1..before.min(self.samples.len())]
            .iter()
            .all(|(point, _)| distance_to_segment(*point, position, target) <= room + 1e-3)
    }
}

/// How far `point` is from the line segment from `a` to `b`, in metres.
fn distance_to_segment(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let along = b - a;
    let length_squared = along.length_squared();
    if length_squared < 1e-6 {
        return point.distance(a);
    }
    let t = ((point - a).dot(along) / length_squared).clamp(0.0, 1.0);
    point.distance(a + along * t)
}

/// What a driver is given the controls by: its truck, and its place in the race.
type Driving<'a> = (
    Entity,
    &'a Transform,
    &'a LinearVelocity,
    &'a mut TruckConfig,
    &'a mut TruckInput,
    &'a mut ComputerDriver,
    Option<&'a Racer>,
    Has<Held>,
);

/// The player's truck, which nobody here drives: it is one of the trucks to catch and to
/// pass like any other.
type Watching<'a> = (
    Entity,
    &'a Transform,
    &'a LinearVelocity,
    &'a TruckConfig,
    Option<&'a Racer>,
);

fn drive(
    time: Res<Time>,
    track: Res<Track>,
    mut drivers: Query<Driving>,
    player: Query<Watching, (With<PlayerTruck>, Without<ComputerDriver>)>,
    // Where the player's truck was last found round the course, as a driver remembers its
    // own: it is one of the trucks to catch and to pass, but nobody drives it.
    mut player_along: Local<Option<f32>>,
) {
    let Some(course) = &track.course else {
        return;
    };
    let lap = course.length();
    // Laps are counted from the start line, wherever along the course that is.
    let places = Places {
        course,
        lap,
        start_line: track.gates.first().map_or(0.0, |gate| {
            course.distance_along(&course.nearest(gate.center))
        }),
    };
    // A truck is enlisted in the race a frame after it is spawned.
    let lap_number = |racer: Option<&Racer>| racer.map_or(0, |racer| racer.progress.lap);
    // How far along the course each checkpoint is, as `ask_to_be_put_back` measures it.
    let gates_along: Vec<f32> = track
        .gates
        .iter()
        .map(|gate| course.distance_along(&course.nearest(gate.center)))
        .collect();

    // Where every truck is, before any of them is driven: what a driver has to catch, and
    // what is in its way. Taken here so that no driver answers to half-moved trucks.
    let mut trucks: Vec<OnCourse> = drivers
        .iter()
        .map(
            |(entity, transform, velocity, config, _, driver, racer, _)| {
                places.of(
                    entity,
                    transform.translation.xz(),
                    velocity.0.xz(),
                    reach(config),
                    driver.along,
                    lap_number(racer),
                )
            },
        )
        .collect();
    if let Ok((entity, transform, velocity, config, racer)) = player.single() {
        let player = places.of(
            entity,
            transform.translation.xz(),
            velocity.0.xz(),
            reach(config),
            *player_along,
            lap_number(racer),
        );
        // Off the course altogether, its place is not to be trusted: look again everywhere.
        *player_along = (player.off < LOST_DISTANCE).then_some(player.along);
        trucks.push(player);
    }
    let leader = trucks
        .iter()
        .map(|truck| truck.raced)
        .fold(f32::MIN, f32::max);

    let mut others: Vec<Neighbour> = Vec::new();
    for (entity, transform, velocity, mut config, mut input, mut driver, racer, held) in
        &mut drivers
    {
        let Some(me) = trucks.iter().find(|truck| truck.entity == entity) else {
            continue;
        };
        // Held on the grid, it waits for the start: going nowhere is not being stuck.
        if held {
            *input = TruckInput::default();
            driver.got_to = me.along;
            driver.getting_nowhere = 0.0;
            driver.recovery = Recovery::default();
            driver.along = Some(me.along);
            continue;
        }
        let position = transform.translation.xz();
        let forward = transform.forward().xz().normalize_or_zero();
        let speed = velocity.0.dot(transform.forward().as_vec3());

        // The trucks near enough to be in the way. Near in a straight line as well as along
        // the course: where the course doubles back, what is on the next stretch of it is
        // not in front.
        //
        // Where one is, though, is measured from the driver's own truck, along its nose and
        // out to its left, and not along the course. Near a sharp bend in the course the two
        // part: Alpine's grid stands before one, facing across the straight that leads to it,
        // so by the course a truck behind another is beside it and a truck beside another is
        // behind it, and every driver steers into its neighbours while keeping clear of them.
        let left = -forward.perp();
        others.clear();
        others.extend(
            trucks
                .iter()
                .filter(|other| {
                    other.entity != entity
                        && other.position.distance(position) <= LOOK_OUT
                        && gap(other.along - me.along, lap).abs() <= 2.0 * LOOK_OUT
                })
                .map(|other| Neighbour {
                    ahead: (other.position - position).dot(forward),
                    across: me.across + (other.position - position).dot(left),
                    speed: other.speed,
                    // What the two of them reach towards each other, and a little more.
                    clear_by: me.reach.x + other.reach.x + ROOM_TO_SPARE,
                    past_by: me.reach.y + other.reach.y,
                }),
        );
        // The line moves across at its own speed, so that pulling out is a move, not a jerk.
        let room = passing_room(course, me.reach.x);
        let wanted = passing_line(driver.home, driver.lane, me.across, &others, room);
        let step = LANE_RATE * time.delta_secs();
        driver.lane += (wanted - driver.lane).clamp(-step, step);
        let steering_line = not_towards(driver.lane, me.across, &others);

        let in_front = held_up_by(&others, me.across, wanted);

        // Its place in the race: one more than the trucks that have raced further than it.
        let place = 1 + trucks.iter().filter(|other| other.raced > me.raced).count();
        let chasing = chasing(place, trucks.len(), leader - me.raced);
        // More power the further behind, on its own engine.
        let (pull, top_speed) = *driver
            .engine
            .get_or_insert((config.engine_force, config.top_speed));
        let boost = 1.0 + CHASING_POWER * chasing;
        config.engine_force = pull * boost;
        config.top_speed = top_speed * boost;
        // Cutting a corner across a truck it is passing, or one beside it, is running into it.
        let passing = (steering_line - driver.home.clamp(-room, room)).abs() > 1e-3
            || others.iter().any(|other| other.side_by_side(me.across));
        // Once it has finished, no checkpoint counts for it any more.
        let gate = racer
            .filter(|racer| racer.progress.finished.is_none())
            .map(|racer| racer.progress.next_gate)
            .and_then(|next| Some((track.gates.get(next)?, gates_along[next])))
            .map(|(gate, along)| NextGate {
                ahead: gap(along - me.along, lap),
                center: gate.center,
                direction: gate.direction(),
                half_width: gate.half_width,
            });
        let style = Style {
            pace: driver.pace,
            lane: steering_line,
            chasing,
            reach_across: me.reach.x,
            cutting: if passing { 0.0 } else { chasing },
            gate,
        };
        let controls = plan(course, me.along, position, forward, speed, style);
        // Whichever asks for less: the bends ahead, or the truck in front.
        input.throttle = controls
            .throttle
            .min(throttle_for(keeping_off(in_front), speed));
        // Hands still in the air, and for a moment after landing.
        let ride_height = config.wheel_radius - config.wheel_rest[0].y;
        let clearance = transform.translation.y
            - track
                .heights
                .height_at(transform.translation.x, transform.translation.z);
        driver.settling = if clearance > ride_height + AIRBORNE {
            SETTLE
        } else {
            (driver.settling - time.delta_secs()).max(0.0)
        };
        let wanted = Controls {
            throttle: input.throttle,
            steer: controls.steer,
        };
        let Controls { throttle, steer } = driver.recovery.step(wanted, speed, time.delta_secs());
        input.throttle = throttle;
        input.steer = if driver.settling > 0.0 { 0.0 } else { steer };
        input.handbrake = false;

        // Forwards only: a truck that rolls back down a hill is getting nowhere.
        let since = (me.along - driver.got_to).rem_euclid(lap);
        let got_somewhere = (PROGRESS..lap / 2.0).contains(&since);
        if driver.along.is_none() || (got_somewhere && me.off < LOST_DISTANCE) {
            driver.got_to = me.along;
            driver.getting_nowhere = 0.0;
            driver.recovery.got_somewhere();
        } else {
            driver.getting_nowhere += time.delta_secs();
        }
        driver.along = Some(me.along);
    }
}

/// Whether a truck `along` metres round a course of `lap` metres has gone past its `next`
/// checkpoint, having driven there from its `last` one, which are that far round too.
/// Measured from the last checkpoint, where a truck that is put back starts again: where
/// the next checkpoint is says nothing alone, when it can be half a lap away either way.
fn has_missed(along: f32, last: f32, next: f32, lap: f32) -> bool {
    let leg = match (next - last).rem_euclid(lap) {
        // The only checkpoint there is comes round again after a lap.
        0.0 => lap,
        leg => leg,
    };
    let driven = (along - last).rem_euclid(lap);
    // Up to half of what is left of the lap. Further than that is a truck that stands a
    // little before its last checkpoint, which the race does to keep it clear of another.
    driven > leg + MISSED_BY && driven < leg + (lap - leg) / 2.0
}

/// In `Update`, where the race puts trucks back.
fn ask_to_be_put_back(
    track: Res<Track>,
    mut drivers: Query<(Entity, &mut ComputerDriver, &Racer, Option<&Name>)>,
    mut back: MessageWriter<BackToCheckpoint>,
) {
    let Some(course) = &track.course else {
        return;
    };
    let lap = course.length();
    for (truck, mut driver, racer, name) in &mut drivers {
        let Some(along) = driver.along else {
            continue;
        };
        // Gone past the checkpoint that it has to drive through next, without doing so.
        let gate_along = |index: usize| {
            track
                .gates
                .get(index)
                .map(|gate| course.distance_along(&course.nearest(gate.center)))
        };
        let missed = racer.progress.finished.is_none()
            && racer
                .progress
                .last_gate(track.gates.len())
                .and_then(gate_along)
                .zip(gate_along(racer.progress.next_gate))
                .is_some_and(|(last, next)| has_missed(along, last, next, lap));
        // Stopped again after backing up, or getting nowhere however it tries.
        let stuck = driver.recovery.gave_up || driver.getting_nowhere > PATIENCE;
        if missed || stuck {
            info!(
                "{}: back to its last checkpoint from {along:.0} m round the course, having {}",
                name.map_or("A computer truck", |name| name.as_str()),
                if stuck {
                    "got stuck"
                } else {
                    "missed the next one"
                },
            );
            back.write(BackToCheckpoint { truck });
            // Wherever that is, it is somewhere else: look over the whole course again.
            driver.along = None;
            driver.getting_nowhere = 0.0;
            driver.recovery = Recovery::default();
            // On another line: put back, a truck drives off exactly as it did last time, and
            // a jump or a bend that caught it once catches it every time after.
            driver.home = next_line(driver.home);
            driver.lane = driver.home;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square of 300 m sides. The first side runs along +X from the origin, and the
    /// first corner turns towards +Z, which is to the right.
    fn course() -> Course {
        Course::new(
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(300.0, 0.0),
                Vec2::new(300.0, 300.0),
                Vec2::new(0.0, 300.0),
            ],
            14.0,
        )
    }

    /// What the built-in truck reaches from its middle, wheels included: 1.9 m across it
    /// and 2.75 m along it.
    fn built_in() -> Vec2 {
        reach(&TruckConfig::default())
    }

    /// How far apart across the course two built-in trucks have to be to pass each other,
    /// and how far apart along it to be clear of each other, in metres: 4.5 and 5.5.
    fn clear_by() -> f32 {
        2.0 * built_in().x + ROOM_TO_SPARE
    }
    fn past_by() -> f32 {
        2.0 * built_in().y
    }

    /// Another built-in truck, `ahead` metres along the course and `across` metres to the
    /// left of the centreline, standing still.
    fn near(ahead: f32, across: f32) -> Neighbour {
        Neighbour {
            ahead,
            across,
            speed: 0.0,
            clear_by: clear_by(),
            past_by: past_by(),
        }
    }

    /// One `ahead` metres up the road and going at `speed`, to hold station behind.
    fn in_front(ahead: f32, speed: f32) -> Neighbour {
        Neighbour {
            speed,
            ..near(ahead, 0.0)
        }
    }

    /// Where a driver settles behind a truck it cannot get by, in metres: 8.
    fn pressure_gap() -> f32 {
        past_by() + PRESSURE_ROOM
    }

    /// A driver on the middle line, at full pace, with nobody to chase.
    fn steady() -> Style {
        Style {
            pace: 1.0,
            lane: 0.0,
            chasing: 0.0,
            reach_across: built_in().x,
            cutting: 0.0,
            gate: None,
        }
    }

    fn on_the_first_side(along: f32, off_to_the_right: f32, forward: Vec2, speed: f32) -> Controls {
        // Heading +X, the right is +Z: `perp` of (1, 0) is (0, 1).
        let position = Vec2::new(along, off_to_the_right);
        plan(&course(), along, position, forward, speed, steady())
    }

    /// Where a driver at `along` on the first side steers at, 15 m up a line `lane` metres
    /// to the left.
    fn aim_from_the_first_side(along: f32, lane: f32) -> Vec2 {
        aim_cutting(along, Style { lane, ..steady() })
    }

    /// The same, from on its line, for a driver of any `style`.
    fn aim_cutting(along: f32, style: Style) -> Vec2 {
        let course = course();
        let position = Vec2::new(along, -style.lane);
        Ahead::of(&course, along, style.reach_across).aim(&course, position, 15.0, style)
    }

    /// The first side's corner is beyond the horizon from here: a plain straight ahead.
    const FAR_FROM_THE_CORNER: f32 = 20.0;

    #[test]
    fn a_truck_on_its_line_drives_straight_and_up_to_its_pace() {
        let slow = on_the_first_side(FAR_FROM_THE_CORNER, 0.0, Vec2::X, 5.0);
        assert_eq!(
            slow,
            Controls {
                throttle: 1.0,
                steer: 0.0
            }
        );
        let at_pace = on_the_first_side(FAR_FROM_THE_CORNER, 0.0, Vec2::X, CRUISE_SPEED);
        assert_eq!(at_pace.throttle, 0.0);
        let too_fast = on_the_first_side(FAR_FROM_THE_CORNER, 0.0, Vec2::X, CRUISE_SPEED + 10.0);
        assert_eq!(too_fast.throttle, -1.0);

        // A slower driver wants less: at a speed the quickest is happy with, it brakes.
        let slower = plan(
            &course(),
            FAR_FROM_THE_CORNER,
            Vec2::new(FAR_FROM_THE_CORNER, 0.0),
            Vec2::X,
            CRUISE_SPEED,
            Style {
                pace: 0.8,
                ..steady()
            },
        );
        assert!(slower.throttle < 0.0, "{slower:?}");
    }

    #[test]
    fn it_steers_back_towards_its_line() {
        // `TruckInput::steer` is positive to the left. Off to the right, steer left: gently,
        // since it is heading for a point well up the road, but it is heading there.
        let right_of_it = on_the_first_side(FAR_FROM_THE_CORNER, 4.4, Vec2::X, 15.0);
        assert!(right_of_it.steer > 0.05, "{right_of_it:?}");
        let left_of_it = on_the_first_side(FAR_FROM_THE_CORNER, -4.4, Vec2::X, 15.0);
        assert!(left_of_it.steer < -0.05, "{left_of_it:?}");
        // The two are mirror images.
        assert!((right_of_it.steer + left_of_it.steer).abs() < 1e-5);

        // A lane to the left is a line to the left: from the centreline, steer left.
        let wants_left = plan(
            &course(),
            FAR_FROM_THE_CORNER,
            Vec2::new(FAR_FROM_THE_CORNER, 0.0),
            Vec2::X,
            15.0,
            Style {
                lane: LANE_SPACING,
                ..steady()
            },
        );
        assert!(wants_left.steer > 0.0, "{wants_left:?}");
    }

    #[test]
    fn it_steers_at_its_own_line_a_little_way_up_the_road() {
        assert!(
            (aim_from_the_first_side(FAR_FROM_THE_CORNER, 0.0) - Vec2::new(35.0, 0.0)).length()
                < 1e-3
        );
        // A line to the left of a course heading +X is towards -Z.
        let left = aim_from_the_first_side(FAR_FROM_THE_CORNER, LANE_SPACING);
        assert!(
            (left - Vec2::new(35.0, -LANE_SPACING)).length() < 1e-3,
            "{left}"
        );
        // And no further out than the road: a truck's outer wheels at its edge.
        let wide = aim_from_the_first_side(FAR_FROM_THE_CORNER, 100.0);
        assert!((wide.y + room_to_pass()).abs() < 1e-3, "{wide}");
    }

    #[test]
    fn it_follows_the_road_round_a_corner_rather_than_cutting_it() {
        // The corner is at (300, 0), turning right onto the side along +Z. From 5 m before
        // it, the point 15 m up the road is 10 m along that side, on the centreline: not
        // across the inside of the corner, and not the corner itself.
        let aim = aim_from_the_first_side(295.0, 0.0);
        assert!((aim - Vec2::new(300.0, 10.0)).length() < 1e-3, "{aim}");
        // On a line to the left, the outside of this corner, it keeps to that line round it.
        let left = aim_from_the_first_side(295.0, LANE_SPACING);
        assert!(
            (left - Vec2::new(300.0 + LANE_SPACING, 10.0)).length() < 1e-3,
            "{left}"
        );
    }

    #[test]
    fn near_a_bend_it_steers_at_a_nearer_point_of_its_line() {
        let course = course();
        let reach = *AIM_AHEAD.end();
        // A plain straight ahead: as far up the road as its speed has it.
        let straight = Ahead::of(&course, FAR_FROM_THE_CORNER, built_in().x);
        assert_eq!(straight.steering_reach(reach), reach);
        // The corner at (300, 0) is a right angle: within reach of it, nearer, but never
        // nearer than the start of `AIM_AHEAD`.
        let near = Ahead::of(&course, 270.0, built_in().x).steering_reach(reach);
        assert!(near < reach && near >= *AIM_AHEAD.start(), "{near}");
        // At a speed that steers at no more than that already, nothing changes.
        let slow = Ahead::of(&course, 270.0, built_in().x);
        assert_eq!(slow.steering_reach(*AIM_AHEAD.start()), *AIM_AHEAD.start());
    }

    #[test]
    fn a_driver_that_is_behind_cuts_the_corner_and_stays_on_the_road() {
        let behind = |along: f32, cutting: f32| {
            aim_cutting(
                along,
                Style {
                    cutting,
                    ..steady()
                },
            )
        };
        // 50 m before the corner at (300, 0), which turns right onto the side along +Z. Its
        // own race is its line, 15 m up the road. Behind, it steers further up the straight,
        // but no further than the corner: any line round it from here would leave the road.
        assert!((behind(250.0, 0.0) - Vec2::new(265.0, 0.0)).length() < 1e-3);
        let far = behind(250.0, 1.0);
        assert!(
            far.x > 265.0 && far.x <= 300.0 && far.y.abs() < 1e-3,
            "{far}"
        );

        // 4 m before it, its own race goes round on its line; far behind, it cuts across
        // the inside, far up the next side, with its wheels on the road all the way.
        let own_race = behind(296.0, 0.0);
        assert!(
            (own_race - Vec2::new(300.0, 11.0)).length() < 1e-3,
            "{own_race}"
        );
        let cut = behind(296.0, 1.0);
        assert!(
            cut.x.abs() - 300.0 < 1e-3 && cut.y > own_race.y + 10.0,
            "{cut}"
        );
        assert!(
            distance_to_segment(Vec2::new(300.0, 0.0), Vec2::new(296.0, 0.0), cut)
                <= room_to_pass() + 1e-3
        );
        // A little behind, a little room: not enough to cut this corner at all.
        assert_eq!(behind(296.0, 0.3), own_race);
    }

    /// A gate on the square course `along` metres round it, `off_to_the_left` of the
    /// centreline, facing the way the course goes there.
    fn gate_at(along: f32, off_to_the_left: f32, half_width: f32) -> (f32, NextGate) {
        let (point, direction) = course().point_at(along);
        let gate = NextGate {
            ahead: 0.0,
            center: point - direction.perp() * off_to_the_left,
            direction,
            half_width,
        };
        (along, gate)
    }

    /// Where a driver of this `style` at `along` on the first side, on its line, steers at,
    /// with the `gate` next.
    fn aim_for_the_gate(along: f32, (gate_along, gate): (f32, NextGate), style: Style) -> Vec2 {
        let gate = NextGate {
            ahead: gate_along - along,
            ..gate
        };
        aim_cutting(
            along,
            Style {
                gate: Some(gate),
                ..style
            },
        )
    }

    #[test]
    fn it_cuts_no_corner_past_its_next_checkpoint() {
        let cutting = Style {
            cutting: 1.0,
            ..steady()
        };
        // 4 m before the corner at (300, 0), a truck far behind cuts across the inside of it
        // and far up the next side (see above). A gate 30 m up the course, 26 m up that side,
        // is as far as it cuts.
        let gate = gate_at(326.0, 0.0, 7.0);
        let cut = aim_for_the_gate(296.0, gate, cutting);
        assert!(cut.y <= 26.0 + 1e-3, "{cut}");
        assert!(cut.y > aim_cutting(296.0, steady()).y, "{cut}");
        // Nearer than it steers ahead, it steers straight through the gate: 14 m up the course
        // and 15 m ahead, the point 1 m beyond the gate on the line through it.
        let through = aim_for_the_gate(296.0, gate_at(310.0, 0.0, 7.0), cutting);
        assert!(
            (through - Vec2::new(300.0, 11.0)).length() < 1e-3,
            "{through}"
        );
    }

    #[test]
    fn it_steers_through_a_gate_that_stands_off_its_line() {
        // A gate 4 m to the left of the centreline, 8 m wide, 10 m ahead of a driver on the
        // line to the right: its outer wheels are to stay a `GATE_MARGIN` inside the gate.
        let half_width = 4.0;
        let (along, gate) = gate_at(280.0, 4.0, half_width);
        let style = Style {
            lane: -LANE_SPACING,
            ..steady()
        };
        let aim = aim_for_the_gate(270.0, (along, gate), style);
        let room = half_width - built_in().x - GATE_MARGIN;
        // 5 m beyond the gate, and as far to the right in it as it may go (left is -Z).
        assert!(
            (aim - Vec2::new(285.0, gate.center.y + room)).length() < 1e-3,
            "{aim}"
        );
        // A gate too narrow to keep to a side of: through the middle of it.
        let (along, narrow) = gate_at(280.0, 4.0, built_in().x);
        let aim = aim_for_the_gate(270.0, (along, narrow), style);
        assert!(
            (aim - Vec2::new(285.0, narrow.center.y)).length() < 1e-3,
            "{aim}"
        );
    }

    #[test]
    fn it_matches_the_steering_of_a_bevy_rotation() {
        // A truck that has yawed to the left of the course (positive yaw) steers right.
        let yawed = Quat::from_rotation_y(0.2) * Vec3::X;
        let controls = on_the_first_side(50.0, 0.0, yawed.xz().normalize(), 15.0);
        assert!(controls.steer < -0.1, "{controls:?}");
    }

    #[test]
    fn it_brakes_for_a_corner_in_good_time_and_not_before() {
        // The corner at 300 m is a right angle, and what it allows follows the tuning, so the
        // test asks for twice that speed: a driver that has to lose three quarters of its
        // speed for a corner brakes for it, whatever the tuning is.
        let allows = bend_speed(SHARP_BEND, CORNERING, SLOWEST_BEND);
        let speed = 2.0 * allows;
        let style = Style {
            pace: speed / CRUISE_SPEED,
            ..steady()
        };
        let at = |along: f32, speed: f32| {
            plan(
                &course(),
                along,
                Vec2::new(along, 0.0),
                Vec2::X,
                speed,
                style,
            )
        };
        // A long way off, at full pace: nothing to slow for. It stops braking in the room
        // three quarters of its speed needs, and looks that far plus a `reach` up the road.
        let braking_room = (speed * speed - allows * allows) / (2.0 * BRAKING);
        assert!(
            braking_room + *AIM_AHEAD.end() < 180.0,
            "{braking_room} m of braking"
        );
        assert!(
            at(100.0, speed).throttle.abs() < 1e-3,
            "{:?}",
            at(100.0, speed)
        );
        // On top of it at full pace: brake.
        let close = at(280.0, speed);
        assert!(close.throttle < -0.9, "{close:?}");
        // At the speed the corner allows, a little short of it: carry on.
        let slow = at(260.0, allows);
        assert!(slow.throttle > 0.0, "{slow:?}");
    }

    #[test]
    fn the_sharper_the_bend_the_slower_it_goes_through_it() {
        let speed = |bend: f32| bend_speed(bend, CORNERING, 0.0);
        // A hairpin is slower than a right angle, which is slower than a kink.
        assert!(speed(SHARP_BEND) < speed(SHARP_BEND / 2.0));
        assert!(speed(SHARP_BEND / 2.0) < speed(SHARP_BEND / 8.0));
        // By more than the radius alone says: at half the radius a corner would allow
        // `1 / sqrt(2)` of the speed, and the care it takes in the sharper one makes it less.
        let radius_alone = speed(SHARP_BEND / 2.0) / 2.0f32.sqrt();
        assert!(speed(SHARP_BEND) < radius_alone, "{}", speed(SHARP_BEND));
        // Nothing is taken slower than the floor, and the floor is what a driver is held to.
        assert_eq!(bend_speed(SHARP_BEND, CORNERING, 1000.0), 1000.0);
        // A bend that is no bend does not stop it: it is held to what the tires allow.
        assert!(speed(0.0).is_finite());
    }

    #[test]
    fn facing_the_wrong_way_it_turns_round_slowly() {
        // Faster than it turns round at, so it slows down to turn.
        let backwards = on_the_first_side(50.0, 0.0, Vec2::NEG_X, SLOWEST_BEND + 5.0);
        assert_eq!(backwards.steer.abs(), 1.0);
        assert!(backwards.throttle < 0.0, "{backwards:?}");
    }

    #[test]
    fn a_stuck_driver_backs_up_turning_the_other_way_and_then_goes_on() {
        const DT: f32 = 1.0 / 120.0;
        // It wants to go, turning left, and the truck does not move.
        let left = Controls {
            throttle: 1.0,
            steer: 1.0,
        };
        let mut recovery = Recovery::default();
        let mut controls = left;
        let mut seconds = 0.0;
        while controls == left {
            controls = recovery.step(left, 0.0, DT);
            seconds += DT;
            assert!(seconds < STOPPED_FOR + 0.1, "never backs up");
        }
        assert!(seconds >= STOPPED_FOR);
        // Backwards, with the wheel turned right, which swings the nose left.
        assert_eq!(
            controls,
            Controls {
                throttle: REVERSING,
                steer: -1.0
            }
        );
        // Still wanting to turn left, it backs up for as long as it may, and then goes on.
        let mut backing = DT;
        while recovery.step(left, -3.0, DT) != left {
            backing += DT;
        }
        assert!((backing - BACK_UP.end()).abs() < 2.0 * DT, "{backing}");
        // Facing the right way sooner, it stops backing up sooner, but not at once.
        let mut recovery = Recovery::default();
        while recovery.step(left, 0.0, DT) == left {}
        let ahead = Controls {
            throttle: 1.0,
            steer: 0.0,
        };
        let mut backing = DT;
        while recovery.step(ahead, -3.0, DT) != ahead {
            backing += DT;
        }
        assert!((backing - BACK_UP.start()).abs() < 2.0 * DT, "{backing}");
    }

    #[test]
    fn stopped_again_after_backing_up_it_gives_up_unless_it_got_somewhere() {
        const DT: f32 = 1.0 / 120.0;
        let go = Controls {
            throttle: 1.0,
            steer: 0.0,
        };
        let stall = |recovery: &mut Recovery| {
            for _ in 0..(2.0 * (STOPPED_FOR + BACK_UP.end()) / DT) as usize {
                recovery.step(go, 0.0, DT);
            }
        };
        // Stopped, it backs up; stopped again, it gives up.
        let mut recovery = Recovery::default();
        stall(&mut recovery);
        assert!(recovery.gave_up);
        // Having got somewhere in between, it backs up again instead.
        let mut recovery = Recovery::default();
        while recovery.step(go, 0.0, DT) == go {}
        while recovery.step(go, -3.0, DT) != go {}
        recovery.got_somewhere();
        while recovery.step(go, 0.0, DT) == go {}
        assert!(!recovery.gave_up);
        assert!(recovery.backing_up.is_some());
    }

    #[test]
    fn a_driver_that_is_moving_or_not_trying_is_not_stuck() {
        const DT: f32 = 1.0 / 120.0;
        let go = Controls {
            throttle: 1.0,
            steer: 0.0,
        };
        let wait = Controls {
            throttle: -1.0,
            steer: 0.0,
        };
        let mut recovery = Recovery::default();
        for _ in 0..(5.0 / DT) as usize {
            // Moving, however slowly it wants to go.
            assert_eq!(recovery.step(go, STOPPED_SPEED + 0.5, DT), go);
            // Standing still because it wants to, behind a truck it cannot pass.
            assert_eq!(recovery.step(wait, 0.0, DT), wait);
        }
    }

    #[test]
    fn a_checkpoint_is_missed_only_by_driving_past_it() {
        // My First Track: a lap of 550 m, and checkpoints at 7 m and 282 m, half a lap apart.
        let lap = 550.0;
        for (last, next) in [(7.0, 282.0), (282.0, 7.0)] {
            // Standing at the last checkpoint, where a truck is put back, and on the way.
            for driven in [0.0, 1.0, 100.0, 270.0, 300.0] {
                assert!(
                    !has_missed(last + driven, last, next, lap),
                    "{last} + {driven}"
                );
            }
            assert!(has_missed(last + 275.0 + MISSED_BY + 5.0, last, next, lap));
            // Set down some way before the last checkpoint, to be clear of another truck.
            assert!(!has_missed(last - 42.0, last, next, lap));
        }
        // A single checkpoint can't be gone past without being a lap further on.
        for along in [0.0, 50.0, 300.0, 549.0] {
            assert!(!has_missed(along, 7.0, 7.0, lap), "{along}");
        }
    }

    #[test]
    fn drivers_differ_in_pace_and_in_line() {
        let drivers: Vec<_> = (0..7).map(ComputerDriver::new).collect();
        for pair in drivers.windows(2) {
            assert!(pair[1].pace < pair[0].pace);
            assert_ne!(pair[1].home, pair[0].home);
            // Each starts out on its own line.
            assert_eq!(pair[1].lane, pair[1].home);
        }
        assert!(drivers[6].pace > 0.75);
    }

    #[test]
    fn each_place_down_the_order_is_more_desperate_than_the_one_in_front() {
        // A field of eight, all in a heap, so that only their places tell them apart.
        let field = 8;
        let together = |place| chasing(place, field, 0.0);
        assert_eq!(together(1), 0.0);
        assert_eq!(together(field), 1.0);
        for place in 1..field {
            assert!(together(place) < together(place + 1), "place {place}");
        }
        // Two of them are the two of them: one leads and one chases.
        assert_eq!(chasing(1, 2, 0.0), 0.0);
        assert_eq!(chasing(2, 2, 0.0), 1.0);
        // Alone, or before the race has sorted itself out, there is nobody to chase.
        assert_eq!(chasing(1, 1, 0.0), 0.0);
    }

    #[test]
    fn the_ground_it_has_to_make_up_makes_a_driver_desperate_too() {
        // Second of eight, which is barely desperate at all while it is in range.
        let field = 8;
        let by_place = chasing(2, field, 0.0);
        assert_eq!(chasing(2, field, IN_RANGE), by_place);
        // Half a lap behind, it is as desperate as the last of them, place or no place.
        assert_eq!(chasing(2, field, CHASING_BY), 1.0);
        assert_eq!(chasing(2, field, 10.0 * CHASING_BY), 1.0);
        // And in between, more the further back it is.
        let a_little = chasing(2, field, IN_RANGE + 100.0);
        assert!(by_place < a_little && a_little < 1.0, "{a_little}");
        assert!(a_little < chasing(2, field, IN_RANGE + 200.0));
        // The leader is the leader, however far ahead it is.
        assert_eq!(chasing(1, field, -500.0), 0.0);
    }

    #[test]
    fn a_chasing_driver_brakes_later_and_carries_more_speed_into_the_corner() {
        // Coming up to the corner at 300 m at 30 m/s, on the middle line.
        let at = |along: f32, chasing: f32| {
            let style = Style {
                pace: 30.0 / CRUISE_SPEED,
                chasing,
                ..steady()
            };
            plan(
                &course(),
                along,
                Vec2::new(along, 0.0),
                Vec2::X,
                30.0,
                style,
            )
        };
        // Where along the road each one first gets on the brakes.
        let brakes_at = |chasing: f32| {
            (100..300)
                .map(|metres| metres as f32)
                .find(|&along| at(along, chasing).throttle < 0.0)
                .expect("the corner is braked for somewhere")
        };
        let (own_race, chased) = (brakes_at(0.0), brakes_at(1.0));
        assert!(
            chased > own_race,
            "braking at {chased} m against {own_race} m"
        );
        // Where the careful one is braking, the chasing one is still going...
        assert!(at(own_race, 0.0).throttle < 0.0);
        assert!(at(own_race, 1.0).throttle >= 0.0);
        // ...and nowhere on the way to the corner does it brake the harder of the two.
        for metres in 100..300 {
            let along = metres as f32;
            let (own, chased) = (at(along, 0.0), at(along, 1.0));
            assert!(
                chased.throttle >= own.throttle,
                "{along} m: {chased:?} {own:?}"
            );
        }
        // It is not throttle it is finding on the straight: both are flat out there.
        assert_eq!(at(20.0, 0.0).throttle, at(20.0, 1.0).throttle);
    }

    /// What `passing_room` allows on the 14 m course above: a line out to 5.1 m either
    /// side of the middle, which is wide enough to pass a truck on the centreline.
    fn room_to_pass() -> f32 {
        passing_room(&course(), built_in().x)
    }

    #[test]
    fn a_driver_with_the_road_to_itself_keeps_to_its_own_line() {
        // Nothing in front; something behind; something too far ahead to bother with.
        assert_eq!(
            passing_line(LANE_SPACING, 0.0, 0.0, &[], room_to_pass()),
            LANE_SPACING
        );
        let behind = [near(-10.0, 0.0)];
        assert_eq!(
            passing_line(LANE_SPACING, 0.0, 0.0, &behind, room_to_pass()),
            LANE_SPACING
        );
        let up_the_road = [near(LOOK_OUT + 10.0, 0.0)];
        assert_eq!(
            passing_line(LANE_SPACING, 0.0, 0.0, &up_the_road, room_to_pass()),
            LANE_SPACING
        );
        // And something in front, but on a line of its own, which it can be left on.
        let beside_it = [near(12.0, -clear_by() - 0.1)];
        assert_eq!(
            passing_line(LANE_SPACING, 0.0, 0.0, &beside_it, room_to_pass()),
            LANE_SPACING
        );
    }

    #[test]
    fn it_pulls_out_to_pass_what_is_in_front_of_it() {
        // A truck just in front, on the same line: out to one side of it, clear of it.
        let passing = passing_line(0.0, 0.0, 0.0, &[near(12.0, 0.0)], room_to_pass());
        assert_eq!(passing.abs(), clear_by(), "{passing}");
        // Already to the right of it, it passes on the right, and on the left to the left.
        assert_eq!(
            passing_line(0.0, -1.0, -1.0, &[near(12.0, 0.5)], room_to_pass()),
            0.5 - clear_by()
        );
        assert_eq!(
            passing_line(0.0, 1.0, 1.0, &[near(12.0, -0.5)], room_to_pass()),
            -0.5 + clear_by()
        );
        // Out is where it stays: the truck it has pulled out for is `clear_by()` away
        // across the course, and it must not take that for a clear road and come back in.
        let pulled_out = [near(12.0, 0.0)];
        assert_eq!(
            passing_line(0.0, clear_by(), clear_by(), &pulled_out, room_to_pass()),
            clear_by()
        );
        // ...while it is alongside, and until it is a truck's length by, when its own line
        // has it again. On the way it keeps a truck's width from it, rather than turning in.
        let beside_it = [near(-1.0, 0.0)];
        assert_eq!(
            passing_line(0.0, clear_by(), clear_by(), &beside_it, room_to_pass()),
            clear_by()
        );
        let nearly_by = [near(-IN_FRONT * past_by() - 0.5, 0.0)];
        assert_eq!(
            passing_line(0.0, clear_by(), clear_by(), &nearly_by, room_to_pass()),
            clear_by()
        );
        let by = [near(-past_by() - 0.1, 0.0)];
        assert_eq!(
            passing_line(0.0, clear_by(), clear_by(), &by, room_to_pass()),
            0.0
        );
        // A truck a clear truck's length up the road, or that far back, is no longer
        // alongside and shuts no door: it is only the next one to pass, or one passed.
        let up_the_road = [near(12.0, 0.0), near(past_by() + 0.1, clear_by())];
        assert_eq!(
            passing_line(0.0, 0.0, 0.0, &up_the_road, room_to_pass()),
            clear_by()
        );
        let passed = [near(12.0, 0.0), near(-past_by() - 0.1, clear_by())];
        assert_eq!(
            passing_line(0.0, 0.0, 0.0, &passed, room_to_pass()),
            clear_by()
        );
        // One alongside on the side it would pull out to shuts that door: it goes the other
        // way, and holds its line when both are shut.
        let door_shut = [near(12.0, 0.0), near(past_by() - 0.1, clear_by())];
        assert_eq!(
            passing_line(0.0, 0.0, 0.0, &door_shut, room_to_pass()),
            -clear_by()
        );
        let boxed_in = [
            near(12.0, 0.0),
            near(2.0, clear_by()),
            near(-2.0, -clear_by()),
        ];
        assert_eq!(passing_line(0.0, 0.0, 0.0, &boxed_in, room_to_pass()), 0.0);
        // The nearest one in the way is the one to pass: out to the left of the near truck,
        // not of the far one, though both are on the driver's line.
        let two = [near(24.0, 0.0), near(12.0, -1.0)];
        assert_eq!(
            passing_line(0.0, 0.0, 0.0, &two, room_to_pass()),
            -1.0 + clear_by()
        );
    }

    #[test]
    fn it_never_turns_in_on_a_truck_that_is_alongside() {
        // Out on the passing line with the truck it has passed now beside it: it stays out,
        // whichever side of the road it went.
        let beside_it = [near(-2.0, 0.0)];
        assert_eq!(
            passing_line(0.0, clear_by(), clear_by(), &beside_it, room_to_pass()),
            clear_by()
        );
        assert_eq!(
            passing_line(0.0, -clear_by(), -clear_by(), &beside_it, room_to_pass()),
            -clear_by()
        );
        // Drawing clear of it, its own line pulls it back in, but no nearer than a truck's
        // width while the two still overlap: it runs alongside rather than turning in.
        let nearly_by = [near(-IN_FRONT * past_by() - 0.5, 0.0)];
        assert_eq!(
            passing_line(0.0, clear_by(), clear_by(), &nearly_by, room_to_pass()),
            clear_by()
        );
        // Room is given from where the driver is, not from the line it wants: on the middle
        // line with a truck out to its left, it does not drift across into it.
        let out_to_the_left = [near(1.0, clear_by())];
        assert!(passing_line(0.0, 0.0, 0.0, &out_to_the_left, room_to_pass()) <= 0.0);
        // Hemmed in either side, it holds the middle rather than leaning on either.
        let both_sides = [near(0.0, clear_by() + 0.2), near(0.0, -clear_by() - 0.2)];
        let held = passing_line(0.0, 0.0, 0.0, &both_sides, room_to_pass());
        assert!(held.abs() <= 0.2, "{held}");
    }

    #[test]
    fn it_passes_on_the_other_side_when_the_road_runs_out() {
        // Out to the right of a truck on its left would be off the road: pass on the left.
        assert_eq!(
            passing_line(0.0, -3.0, -3.0, &[near(12.0, -1.0)], room_to_pass()),
            -1.0 + clear_by()
        );
        // Boxed in on a road too narrow to pass on: hold the line and wait for one.
        assert_eq!(passing_line(0.0, 0.5, 0.5, &[near(12.0, 0.0)], 1.0), 0.5);
    }

    #[test]
    fn how_far_a_truck_has_raced_grows_evenly_through_the_start_line() {
        // My First Track: a lap of 550 m, with the start line 7 m round the course.
        let (lap, line) = (550.0, 7.0);
        // On the grid, a few metres short of the line, on no lap yet.
        assert_eq!(raced(line - 5.0, line, 0, lap), -5.0);
        // Over it, and round to just before it again.
        assert_eq!(raced(line, line, 1, lap), 0.0);
        assert_eq!(raced(line + 300.0, line, 1, lap), 300.0);
        assert_eq!(raced(line - 5.0, line, 1, lap), lap - 5.0);
        // And over it a second time: a lap more than the first time round.
        assert_eq!(raced(line, line, 2, lap), lap);
        assert_eq!(raced(line + 300.0, line, 2, lap), lap + 300.0);
    }

    #[test]
    fn it_holds_station_behind_a_truck_it_has_not_got_by() {
        // A clear road in front is no limit on its speed at all.
        assert!(keeping_off(None).is_infinite());
        // Up behind one doing 20 m/s: 20 m/s, so the gap stays as it is. Nearer than
        // that it still asks for 20, and presses rather than dropping back.
        assert_eq!(keeping_off(Some(in_front(pressure_gap(), 20.0))), 20.0);
        assert_eq!(
            keeping_off(Some(in_front(pressure_gap() - 3.0, 20.0))),
            20.0
        );
        // Further back it may go quicker, by exactly what it can brake off again in the
        // road that is left: from here, braking at `BRAKING`, it is down to 20 m/s with
        // `PRESSURE_ROOM` and its own length to spare.
        let coming_up = keeping_off(Some(in_front(pressure_gap() + 30.0, 20.0)));
        let braking_distance = (coming_up * coming_up - 20.0 * 20.0) / (2.0 * BRAKING);
        assert!((braking_distance - 30.0).abs() < 1e-3, "{coming_up}");
        // And behind one that has stopped, it stops.
        assert_eq!(keeping_off(Some(in_front(pressure_gap(), 0.0))), 0.0);
    }

    #[test]
    fn it_swerves_round_a_truck_in_front_rather_than_slowing_for_it() {
        let ahead = near(20.0, 0.0);
        // Heading round it: the line it wants is clear, so nothing holds it up, though
        // it is still square behind the truck.
        let wanted = passing_line(0.0, 0.0, 0.0, &[ahead], room_to_pass());
        assert!(wanted.abs() >= clear_by(), "{wanted}");
        assert_eq!(held_up_by(&[ahead], 0.0, wanted), None);
        // Boxed in, its line is still behind the truck: it slows for it.
        assert_eq!(held_up_by(&[ahead], 0.0, 0.0), Some(ahead));
        // Out of its way already, whatever line it wants: nothing to slow for.
        assert_eq!(held_up_by(&[ahead], clear_by(), 0.0), None);
    }

    #[test]
    fn the_throttle_takes_the_lower_of_the_two_speeds_asked_of_it() {
        // Full throttle for the road ahead, but the truck in front wants 20 m/s of the
        // 30 m/s it is doing: the brakes win.
        let for_the_road = throttle_for(60.0, 30.0);
        let for_the_truck = throttle_for(keeping_off(Some(in_front(pressure_gap(), 20.0))), 30.0);
        assert_eq!(for_the_road, 1.0);
        assert_eq!(for_the_road.min(for_the_truck), -1.0);
        // With nobody in front, the road has it.
        assert_eq!(for_the_road.min(throttle_for(keeping_off(None), 30.0)), 1.0);
    }

    #[test]
    fn a_gap_round_the_course_is_measured_the_short_way() {
        let lap = 550.0;
        assert_eq!(gap(30.0, lap), 30.0);
        assert_eq!(gap(-30.0, lap), -30.0);
        // Just round the start line, either way.
        assert_eq!(gap(-30.0 + lap, lap), -30.0);
        assert_eq!(gap(30.0 - lap, lap), 30.0);
    }

    #[test]
    fn scratch_what_a_driver_asks_of_its_tires() {
        // The tires' grip on level ground, sideways.
        let real = TruckConfig::default().grip * 9.81;
        println!("bend  radius   chosen   holdable   asked/real");
        for step in 1..=12 {
            let bend = 0.05 * step as f32 * SHARP_BEND;
            let radius = BEND_SPAN / bend;
            let chosen = bend_speed(bend, CORNERING, SLOWEST_BEND);
            let holdable = (real * radius).sqrt();
            let asked = chosen * chosen / radius;
            println!(
                "{bend:5.2} {radius:7.0} {chosen:8.1} {holdable:10.1} {:12.2}",
                asked / real,
            );
        }
    }

    #[test]
    fn scratch_where_it_aims_coming_up_to_the_corner() {
        // The corner is at (300, 0). Speeds are what a driver would carry there.
        println!("to apex   aim x    aim z   dist to aim   steer");
        for back in [200.0f32, 150.0, 100.0, 70.0, 50.0, 40.0, 30.0, 20.0, 10.0] {
            let along = 300.0 - back;
            let position = Vec2::new(along, 0.0);
            let aim = aim_from_the_first_side(along, 0.0);
            let controls = on_the_first_side(along, 0.0, Vec2::X, 30.0);
            println!(
                "{back:7.0} {:7.1} {:8.2} {:13.1} {:7.2}",
                aim.x,
                aim.y,
                aim.distance(position),
                controls.steer
            );
        }
    }
}
