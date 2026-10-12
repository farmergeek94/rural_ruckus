//! The mind of a computer truck on a circuit: what it does with the controls each step,
//! from where its truck is round the course, where the others are, and what it has to
//! drive through next. A `Driver` is what one keeps from step to step, and `Driver::decide`
//! is the rule; `plan` is its heart, a pure function of the course and the truck's state.
//!
//! A driver keeps to the road rather than cutting corners: it steers at a point of its own
//! line a little way up the course, and so follows the road round every bend on that line.
//! Near a bend that point is nearer (`BEND_CUT`), so that a truck at speed does not take a
//! chord across the inside of the bend to a point far round it. Each driver's line is the
//! middle of the road or a little to one side of it, so that trucks don't all want the same
//! one. It keeps to a speed that the bends ahead allow, which is the sharper the slower
//! (`bend_speed`), and round a bend that goes on, a loop or a right angle, the speed that
//! its whole radius allows (`LONG_BEND_SPAN`). Downhill it counts on less braking, and so
//! brakes sooner (`DOWNHILL_BRAKING`). A driver that is behind catches up on its line, never
//! across the inside of a bend: the further down the order it is, and the further behind
//! the leader, the more power its engine has, the more it asks of its tires and the later
//! it brakes (`chasing`), until it is back in range. The edge of the course is not the edge
//! of the road: a Monster Truck Madness 2 course leaves out the corners between its
//! straight pieces, and the road goes round them outside the course's centreline, on Round
//! and Round up to 11 m outside it. A chord across the inside of a bend to the edge of the
//! course took the trucks that were behind across the grass.
//!
//! Near the checkpoint it has to drive through next, a driver leaves the course and its
//! line and steers straight through the gate, inside its edges (`NextGate`): the course is
//! only near the gates, and a line or a pass can take a truck round one. One that comes up
//! behind another truck swerves round it instead of slowing down (`passing`), and steers
//! round fixed scenery on the road in the same way, as if it were a truck stopped there
//! (`obstacles`). It remembers how far along the course it is and looks for itself only
//! about there: where a mountain road doubles back, the nearest piece of the course is
//! often the wrong one. Put back at a checkpoint, it takes another line, and until it has
//! driven through its next checkpoint it cuts no corners (`Style::careful`): a corner it
//! cut is often what caught it.
//!
//! `lead_driver` lends the same rules to whatever wants to drive a truck round the course as
//! the quickest driver would on a clear road.
//!
//! On a world that repeats, a driver measures the way to anything the short way, which may
//! be across an edge of the map (`Course::offset`).

use bevy::math::Vec2;
use content::track::{Course, Gate, TrackData};
use content::truck::TruckConfig;

use crate::obstacles::{Obstacle, obstacles_between, obstacles_on};
use crate::passing::{Neighbour, ROOM_TO_SPARE, look_out, not_towards, passing_line, passing_room};
use crate::progress::RaceProgress;

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
/// How much less braking a driver counts on for each metre the course falls per metre along
/// it, between its truck and where its braking would end, in m/s². Measured headless,
/// seven computer trucks braking fully for 4 minutes on Sidewinder Canyon and on Alpine, at
/// 20 to 30 m/s: about 6 m/s² on the level, 4.1 to 6.3 at 12 % downhill, 2.7 to 5.3 at
/// 18 % and 0.8 to 3.7 at 24 %. That is more than gravity alone takes away (9.8): a truck
/// bounces on the way down. Counting on 7 there, drivers arrived at the canyons' U-turns too
/// fast and went off.
const DOWNHILL_BRAKING: f32 = 16.0;
/// The least braking a driver counts on however steep the way down, in m/s², so that it
/// still drives on down a cliff of a slope.
const LEAST_BRAKING: f32 = 1.5;
/// No bend is taken slower than this, in m/s.
const SLOWEST_BEND: f32 = 16.0;
/// A bend is measured as the change in the course's direction over this many metres.
const BEND_SPAN: f32 = 30.0;
/// It is measured over this longer stretch as well, in metres. A turn that goes on, a loop or
/// a right angle, is driven on its whole radius, which no 30 m of it shows when the course
/// makes it of straight pieces: Round and Round's loops are turns of 45 degrees 19 to 55 m
/// apart, and a driver took each at the speed of a single kink, ran wide out of the loops
/// and missed the checkpoints in them. Measured in eight races of 240 s on each of four
/// tracks, against the same drivers without it: on Round and Round, missed checkpoints fell
/// from 18 to 4.5 a race and trucks put back from 28 to 12; elsewhere, the distance driven
/// changed by less than 2 %.
const LONG_BEND_SPAN: f32 = 60.0;
/// Sideways acceleration a driver allows itself all the way round a long bend, in m/s²,
/// chasing or not: what the trucks hold. Measured on flat ground round circles of 20, 40 and
/// 80 m, speeding up slowly: the built-in truck, Bigfoot and Max-D, each as tuned and with
/// three random setups, held 11.5 to 13.3 and then slid wide; none tipped. Higher runs wide
/// out of loops; lower is slower round them.
const LONG_BEND_CORNERING: f32 = 12.0;
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
/// `BEND_CUT` for a driver that has been put back, until it drives through its next
/// checkpoint (`Style::careful`), and the nearest it then steers at, in metres. A bend of
/// radius `R` cuts about `BEND_CUT / 4` metres inside the line, so at 2 it keeps within half a
/// metre of it, where at 16 it was 4 m inside. The inside of a bend is where a truck that was
/// put back was caught: on a tree, in a ditch, or past a checkpoint that stands on the road
/// round the bend. Smaller cuts less, and weaves more, since it steers at a nearer point.
/// Not measured in a race yet.
const CAREFUL_BEND_CUT: f32 = 2.0;
const CAREFUL_NEAREST: f32 = 8.0;
/// Full lock is used when the aim point is this far off straight ahead, in radians. Smaller
/// is a driver that turns sooner and harder for the same aim point, and weaves more on a
/// straight; at 0.3 it is at full lock 17 degrees off.
pub const FULL_LOCK_ANGLE: f32 = 0.3;
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
pub(crate) const LANE_SPACING: f32 = 2.5;
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
/// How much more braking it counts on then, as a share of `braking_on`: it leaves its
/// braking that much later, and arrives in the bend that much hotter.
const CHASING_BRAKE: f32 = 0.15;
/// How much more top speed a truck has when its driver is as desperate as it gets, as a
/// share: its engine's pull and the speed at which the pull runs out both go up by this,
/// which raises the speed it reaches on the level by the same share. In between, in
/// proportion, and none for the leader. This is the catch-up: a truck that is behind has more
/// power than the player's, and loses it again as it gets back in range.
const CHASING_POWER: f32 = 0.15;
/// How fast a driver moves its line across the course, in m/s. Faster pulls out in less
/// road; slower keeps the steering from snapping over when a truck appears in front.
const LANE_RATE: f32 = 3.0;
/// Daylight a driver leaves beside fixed scenery, in metres, on top of `ROOM_TO_SPARE`: its
/// line can still be half a metre inside in a bend (`CAREFUL_BEND_CUT`), and it may still be
/// moving across to it.
const OBSTACLE_ROOM: f32 = 1.0;
/// How far behind and ahead of where it last was a driver looks for itself, in metres.
const BEHIND: f32 = 20.0;
const AHEAD: f32 = 60.0;
/// A driver that has not got this far along the course, in metres, within `PATIENCE`
/// seconds is stuck, whatever it is doing. The slowest bend is driven at 7 m/s.
pub const PROGRESS: f32 = 10.0;
/// Further than this from the course, in metres, is no progress either, wherever along
/// the course the nearest piece of it is.
const LOST_DISTANCE: f32 = 45.0;
/// A driver this far along the course past a checkpoint that it still has to drive
/// through, in metres, has missed it.
const MISSED_BY: f32 = 40.0;

/// What a level of difficulty makes of a driver on a circuit: the share of its truck's own
/// engine it has, and of the catch-up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Skill {
    /// The share of its truck's own engine (pull and top speed) a driver has. Lower is
    /// slower on the straights and up the hills; much lower and the trucks are left behind
    /// on the first straight.
    pub power: f32,
    /// The share of `CHASING_POWER` a driver has, so that it gives up a lead it has lost.
    pub catching_up: f32,
}

/// The course every driver follows: `TrackData::other_courses[other]`, which is the one
/// Monster Truck Madness 2's own computer trucks follow for its second, where the track has
/// it, and the main course where it does not, or where `other` is `None`.
pub fn course_for(track: &TrackData, other: Option<usize>) -> Option<&Course> {
    other
        .and_then(|index| track.other_courses.get(index)?.as_ref())
        .or(track.course.as_ref())
}

/// What a driver keeps from one step to the next on a circuit.
#[derive(Clone, Debug)]
pub struct Driver {
    /// How much of `CRUISE_SPEED` this driver keeps to when it is up with the race, from
    /// 0 to 1. Behind, it drives above this: see `Skill::catching_up`.
    pace: f32,
    /// The line this driver takes when the road in front of it is clear: how far to the
    /// left of the centreline, in metres.
    home: f32,
    /// The line it is on now, which moves aside to pass what is in front of it.
    lane: f32,
    /// How far along the course's centreline the truck was last found, in metres.
    /// `None` until it has been looked for, which is then done over the whole course.
    along: Option<f32>,
    /// Where along the course the truck was when it last got somewhere, in metres.
    got_to: f32,
    /// Where it was in the race when it was last put back, as the lap and the checkpoint it
    /// had to drive through next: until it has driven through that one, it is careful
    /// (`Style::careful`). `None` when it has not been put back since its last checkpoint.
    careful_until: Option<(u32, usize)>,
    /// The trucks and the fixed scenery near enough to be in its way this step, kept so
    /// that no step allocates.
    others: Vec<Neighbour>,
}

/// What a driver decides for a step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decision {
    pub controls: Controls,
    /// The share of its truck's own engine (pull and top speed) it has this step: the
    /// difficulty's, and the more the further behind it is (`CHASING_POWER`).
    pub boost: f32,
}

impl Driver {
    /// Driver `number` of a race, from 0: each a little slower than the driver numbered
    /// before it, on the middle line, the left and the right by turns.
    pub fn new(number: usize) -> Self {
        let home = LINES[number % LINES.len()];
        Self {
            pace: (1.0 - PACE_STEP * number as f32).max(0.5),
            home,
            lane: home,
            along: None,
            got_to: 0.0,
            careful_until: None,
            others: Vec::new(),
        }
    }

    /// How far along the course's centreline the truck was last found, in metres, to look
    /// for it near there (`Places::of`); `None` to look over the whole course.
    pub fn along(&self) -> Option<f32> {
        self.along
    }

    /// Held on the grid, at `mine`, it waits for the start: going nowhere is not being
    /// stuck.
    pub fn held_at(&mut self, mine: &OnCourse) {
        self.got_to = mine.along;
        self.along = Some(mine.along);
    }

    /// What to do with the controls of its truck, which is `race.trucks[me]`, at `position`
    /// on the ground plane, facing `forward` (a unit vector) at `speed` m/s along its nose,
    /// with its `progress` in the race (none until the race has enlisted it), for a driver
    /// of this `skill`, `dt` seconds on from the last step.
    #[expect(
        clippy::too_many_arguments,
        reason = "the whole race, and the truck in it"
    )]
    pub fn decide(
        &mut self,
        race: &Race,
        me: usize,
        position: Vec2,
        forward: Vec2,
        speed: f32,
        progress: Option<&RaceProgress>,
        skill: Skill,
        dt: f32,
    ) -> Decision {
        let Race {
            course,
            track,
            route,
            trucks,
            leader,
        } = *race;
        let lap = course.length();
        let mine = &trucks[me];

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
        self.others.clear();
        // Seen from `LOOK_AHEAD` of catching it up, until they touch.
        self.others
            .extend(trucks.iter().enumerate().filter_map(|(index, other)| {
                let look_out = look_out(mine.speed - other.speed);
                let offset = course.offset(position, other.position);
                (index != me
                    && offset.length() <= look_out
                    && gap(other.along - mine.along, lap).abs() <= 2.0 * look_out)
                    .then(|| Neighbour {
                        ahead: offset.dot(forward),
                        across: mine.across + offset.dot(left),
                        // What the two of them reach towards each other, and a little more.
                        clear_by: mine.reach.x + other.reach.x + ROOM_TO_SPARE,
                        past_by: mine.reach.y + other.reach.y,
                        look_out,
                    })
            }));
        // And the fixed scenery up the road, each point of it a truck that is stopped there.
        // Where it is is measured along and across the course, as the driver's line is: in
        // a bend, measured from the truck's nose, an object at the inside of the bend looks
        // clear of the line until the truck is on it.
        // It closes on what stands still at its own speed.
        let obstacle_look_out = look_out(speed);
        let obstacle_clear_by = mine.reach.x + ROOM_TO_SPARE + OBSTACLE_ROOM;
        let obstacles_from = self.others.len();
        self.others.extend(
            obstacles_between(
                &route.obstacles,
                mine.along - 2.0 * mine.reach.y,
                mine.along + obstacle_look_out,
                lap,
            )
            .map(|obstacle| Neighbour {
                ahead: gap(obstacle.along - mine.along, lap),
                across: obstacle.across,
                clear_by: obstacle_clear_by,
                past_by: mine.reach.y,
                look_out: obstacle_look_out,
            }),
        );
        // Near fixed scenery it keeps to its line round a bend, as one that was put back
        // does: cutting the bend takes it about `BEND_CUT / 4` metres inside its line, which
        // is where the loader on Crazy '98 stands.
        let by_scenery = self.others[obstacles_from..].iter().any(|obstacle| {
            obstacle.ahead >= -obstacle.past_by
                && (obstacle.across - self.lane).abs() < obstacle.clear_by + BEND_CUT / 4.0
        });
        // The line moves across at its own speed, so that pulling out is a move, not a jerk.
        let room = passing_room(course, mine.reach.x);
        let wanted = passing_line(self.home, self.lane, mine.across, &self.others, room);
        let step = LANE_RATE * dt;
        self.lane += (wanted - self.lane).clamp(-step, step);
        let steering_line = not_towards(self.lane, mine.across, &self.others);

        // Its place in the race: one more than the trucks that have raced further than it.
        let place = 1 + trucks
            .iter()
            .filter(|other| other.raced > mine.raced)
            .count();
        let chasing = chasing(place, trucks.len(), leader - mine.raced);
        // Its share of its own engine for the difficulty, and more the further behind.
        let boost = skill.power * (1.0 + CHASING_POWER * skill.catching_up * chasing);
        // Careful from being put back until it drives through its next checkpoint, which
        // moves the race on from where it was then.
        let put_back = progress.is_some_and(|progress| {
            progress.finished.is_none()
                && self.careful_until == Some((progress.lap, progress.next_gate))
        });
        if !put_back {
            self.careful_until = None;
        }
        // Once it has finished, no checkpoint counts for it any more.
        let gate = progress
            .filter(|progress| progress.finished.is_none())
            .map(|progress| progress.next_gate)
            .and_then(|next| Some((track.gates.get(next)?, route.gates_along[next])))
            .map(|(gate, along)| NextGate {
                ahead: gap(along - mine.along, lap),
                center: gate.center,
                direction: gate.direction(),
                half_width: gate.half_width,
            });
        // How far the ground falls from here to where its braking would end, if it braked now
        // for a bend there.
        let stopping = (speed * speed / (2.0 * BRAKING) + *AIM_AHEAD.start()).min(HORIZON);
        let (there, _) = course.point_at(mine.along + stopping);
        let fall = track.heights.height_at(position.x, position.y)
            - track.heights.height_at(there.x, there.y);
        let style = Style {
            pace: self.pace,
            lane: steering_line,
            chasing,
            downhill: (fall / stopping).max(0.0),
            reach_across: mine.reach.x,
            gate,
            careful: put_back || by_scenery,
        };
        // Only the road ahead sets its speed: a truck or fixed scenery in front is steered
        // round, not braked for.
        Decision {
            controls: plan(course, mine.along, position, forward, speed, style),
            boost,
        }
    }

    /// Notes where its truck is after a step (`mine`, its place round a course of `lap`
    /// metres), and says whether it has got somewhere since it last did: `PROGRESS` metres
    /// on, forwards only, since a truck that rolls back down a hill is getting nowhere, and
    /// on the course. The first time it is looked at counts.
    pub fn got_somewhere(&mut self, mine: &OnCourse, lap: f32) -> bool {
        let since = (mine.along - self.got_to).rem_euclid(lap);
        let got_somewhere = (PROGRESS..lap / 2.0).contains(&since);
        let noted = self.along.is_none() || (got_somewhere && mine.off < LOST_DISTANCE);
        if noted {
            self.got_to = mine.along;
        }
        self.along = Some(mine.along);
        noted
    }

    /// Whether its truck has gone past the checkpoint it has to drive through next without
    /// driving through it, where `gates_along` is how far along `course` each of the track's
    /// gates is (`along_the_course`).
    pub fn missed_a_gate(
        &self,
        course: &Course,
        gates_along: &[f32],
        progress: &RaceProgress,
    ) -> bool {
        let Some(along) = self.along else {
            return false;
        };
        let gate_along = |index: usize| gates_along.get(index).copied();
        progress.finished.is_none()
            && progress
                .last_gate(gates_along.len())
                .and_then(gate_along)
                .zip(gate_along(progress.next_gate))
                .is_some_and(|(last, next)| has_missed(along, last, next, course.length()))
    }

    /// Put back at its last checkpoint, with `progress` where the race has it. Wherever that
    /// is, it is somewhere else: it looks over the whole course again. It takes another line:
    /// put back, a truck drives off exactly as it did last time, and a jump or a bend that
    /// caught it once catches it every time after. And it keeps to that line round every
    /// bend, to the checkpoint it has to drive through.
    pub fn put_back(&mut self, progress: &RaceProgress) {
        self.along = None;
        self.home = next_line(self.home);
        self.lane = self.home;
        self.careful_until = Some((progress.lap, progress.next_gate));
    }
}

/// The lines drivers take, in turn: the middle, the left and the right.
const LINES: [f32; 3] = [0.0, LANE_SPACING, -LANE_SPACING];

/// The line after `home` in `LINES`, for a driver that is trying again.
fn next_line(home: f32) -> f32 {
    let at = LINES.iter().position(|line| *line == home).unwrap_or(0);
    LINES[(at + 1) % LINES.len()]
}

/// What a driver does with the controls, as `TruckInput` has them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controls {
    /// -1 (full brake) to 1 (full throttle).
    pub throttle: f32,
    /// -1 (right) to 1 (left).
    pub steer: f32,
}

/// What the quickest driver would do with the controls of a truck at `position` on the
/// ground plane, facing `forward` (a unit vector) at `speed` m/s along its nose, on a clear
/// road: on the middle line, not chasing and with no checkpoint to aim through. `last` is
/// how far round the `course` the truck was last found, in metres, if it is known; the
/// second value is how far round it is now, for next time, or `None` when it is so far off
/// the course that it is to be looked for everywhere.
pub fn lead_driver(
    course: &Course,
    last: Option<f32>,
    position: Vec2,
    forward: Vec2,
    speed: f32,
    config: &TruckConfig,
) -> (Controls, Option<f32>) {
    let nearest = match last {
        Some(along) => course.nearest_within(position, along - BEHIND, along + AHEAD),
        None => course.nearest(position),
    };
    let along = course.distance_along(&nearest);
    let style = Style {
        pace: 1.0,
        lane: 0.0,
        chasing: 0.0,
        // It has no ground to look at: as on the level.
        downhill: 0.0,
        reach_across: reach(config).x,
        gate: None,
        careful: false,
    };
    let controls = plan(course, along, position, forward, speed, style);
    // Off the course altogether, where it is is not to be trusted: look everywhere next time.
    (
        controls,
        (nearest.distance < LOST_DISTANCE).then_some(along),
    )
}

/// The course as the drivers know it, found once a race: how far along it each checkpoint
/// is, and the fixed scenery on it.
#[derive(Clone, Debug)]
pub struct Route {
    /// `along_the_course`, for the course and the checkpoints of this race.
    gates_along: Vec<f32>,
    /// `obstacles_on` the course of this race.
    obstacles: Vec<Obstacle>,
}

impl Route {
    pub fn new(track: &TrackData, course: &Course) -> Self {
        Self {
            gates_along: along_the_course(&track.gates, course),
            obstacles: obstacles_on(&track.scenery, course),
        }
    }

    /// How many checkpoints it knows of: as many as the track had gates when it was made.
    pub fn gates(&self) -> usize {
        self.gates_along.len()
    }
}

/// The race a driver is driven in this step: the course, the track, what the drivers know of
/// the course, and where every truck is round it, worked out before any driver is given its
/// controls, so that no driver answers to half-moved trucks.
#[derive(Clone, Copy)]
pub struct Race<'a> {
    pub course: &'a Course,
    pub track: &'a TrackData,
    pub route: &'a Route,
    /// Every truck in the race, the player's included, each as `Places::of` places it.
    pub trucks: &'a [OnCourse],
    /// How far the truck leading the race has raced, in metres.
    leader: f32,
}

impl<'a> Race<'a> {
    pub fn new(
        course: &'a Course,
        track: &'a TrackData,
        route: &'a Route,
        trucks: &'a [OnCourse],
    ) -> Self {
        let leader = trucks
            .iter()
            .map(|truck| truck.raced)
            .fold(f32::MIN, f32::max);
        Self {
            course,
            track,
            route,
            trucks,
            leader,
        }
    }
}

/// How a driver is driving: the numbers that tell `plan` what it is trying to do.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Style {
    /// How much of `CRUISE_SPEED` it keeps to where the course is straight, from 0 to 1.
    /// Chasing does not change this: the straights are driven flat out either way.
    pub(crate) pace: f32,
    /// The line it is taking: how far to the left of the centreline, in metres.
    lane: f32,
    /// How desperate it is, from 0 to 1, as `chasing` works it out. It asks more of the tires
    /// in a bend and brakes later for one, and keeps to its line.
    chasing: f32,
    /// How steeply the course falls away ahead, between the truck and where its braking
    /// would end: metres down for each metre along, and 0 on the level or uphill. The steeper,
    /// the less braking it counts on (`braking_on`).
    downhill: f32,
    /// How far its own truck reaches sideways from its middle, in metres, wheels included:
    /// what says how near the edge of the course its line may go to pass.
    reach_across: f32,
    /// The checkpoint it has to drive through next, if it has one: near it, it steers
    /// through it (`Ahead::aim`).
    gate: Option<NextGate>,
    /// Whether it has been put back and not yet driven through its next checkpoint: then it
    /// cuts no corners, and keeps to its line round every bend (`CAREFUL_BEND_CUT`).
    careful: bool,
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
    let aim = ahead.aim(course, ahead.steering_reach(reach, style.careful), style);
    // The short way, which on a world that repeats may be across an edge of the map.
    let to_aim = course.offset(position, aim).normalize_or_zero();
    // Positive when the aim point is to the left.
    let off_straight = (-forward.perp_dot(to_aim)).atan2(forward.dot(to_aim));
    let steer = (off_straight / FULL_LOCK_ANGLE).clamp(-1.0, 1.0);

    // The speed that every bend within braking distance allows here and now. A driver that
    // is chasing asks more of its tires in the bend and counts on more of its brakes to get
    // there, which is the caution it is throwing away to catch up.
    let cornering = CORNERING * (1.0 + CHASING_CORNER * style.chasing);
    let braking = braking_on(style.downhill) * (1.0 + CHASING_BRAKE * style.chasing);
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
        let (bend, long_bend) = (ahead.bend(i).abs(), ahead.long_bend(i).abs());
        let mut in_the_bend = f32::INFINITY;
        if bend > 1e-3 {
            in_the_bend = bend_speed(bend, cornering, slowest_bend);
        }
        if long_bend > 1e-3 {
            let round_it = (LONG_BEND_CORNERING * LONG_BEND_SPAN / long_bend).sqrt();
            in_the_bend = in_the_bend.min(round_it.max(slowest_bend));
        }
        if in_the_bend.is_finite() {
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

/// The braking a driver counts on, in m/s², where the course falls `downhill` metres for each
/// metre along it (see `DOWNHILL_BRAKING`).
fn braking_on(downhill: f32) -> f32 {
    (BRAKING - DOWNHILL_BRAKING * downhill.max(0.0)).max(LEAST_BRAKING)
}

/// How desperate a driver is, from 0 (the leader, driving its own race with its own care) to
/// 1 (throwing its caution away to get on terms). It buys more power on the straights
/// (`CHASING_POWER`), and more speed into the bends and later braking for them, both of
/// which risk the truck. See `Style`.
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

/// The throttle, from -1 (full brake) to 1 (full throttle), that a driver doing `speed`
/// uses to get to `wanted`, both in m/s.
pub fn throttle_for(wanted: f32, speed: f32) -> f32 {
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

/// Where a truck is round the course, worked out before any driver is given its controls:
/// one place for every truck in the race, the player's included (`Places::of`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OnCourse {
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

impl OnCourse {
    /// How far along the course it is, in metres, to look for it near next time; `None`
    /// when it is so far off the course that its place is not to be trusted, and it is to be
    /// looked for everywhere.
    pub fn found(&self) -> Option<f32> {
        (self.off < LOST_DISTANCE).then_some(self.along)
    }
}

/// How far a truck reaches from its middle, in metres, with its wheels: `x` across it and
/// `y` along it. A wheel collider is a cylinder on the axle, so a tire reaches its own half
/// width outwards from the hub and its radius fore and aft of it. The body is inside that on
/// the built-in truck; a Monster Truck Madness 2 one is measured the same way from its own
/// wheels.
pub(crate) fn reach(config: &TruckConfig) -> Vec2 {
    config.wheel_rest.iter().fold(Vec2::ZERO, |reach, wheel| {
        reach.max(Vec2::new(
            wheel.x.abs() + config.wheel_width / 2.0,
            wheel.z.abs() + config.wheel_radius,
        ))
    })
}

/// What a truck's place round the course is worked out against: the course, the length of a
/// lap of it, and how far along it the start line is.
pub struct Places<'a> {
    course: &'a Course,
    lap: f32,
    start_line: f32,
}

impl<'a> Places<'a> {
    /// The places of a race round `course`, with the checkpoints of `route` on it. Laps are
    /// counted from the start line, gate 0, wherever along the course that is.
    pub fn new(course: &'a Course, route: &Route) -> Self {
        Self {
            course,
            lap: course.length(),
            start_line: route.gates_along.first().copied().unwrap_or(0.0),
        }
    }

    /// Where a truck at `position` going `velocity`, with `config`'s wheels, is. It is looked for near where it was
    /// `last` seen, when that is known: where a mountain road doubles back, the nearest
    /// piece of the course is often the wrong one. `lap_number` is the lap it is on, which
    /// counts from the start line, so that the distance raced counts from there too.
    pub fn of(
        &self,
        position: Vec2,
        velocity: Vec2,
        config: &TruckConfig,
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
            position,
            along,
            off: nearest.distance,
            // A line `across` to the left is `point - direction.perp() * across`.
            across: -course.offset(point, position).dot(direction.perp()),
            speed: velocity.dot(direction),
            reach: reach(config),
            raced: raced(along, self.start_line, lap_number, self.lap),
        }
    }
}

/// The course ahead of a truck: its position and direction of travel every `BEND_STEP`
/// metres from where the truck is, to `HORIZON` and a `LONG_BEND_SPAN` beyond, so that a bend
/// at the horizon can be measured.
struct Ahead {
    /// How far along the course the truck is, in metres.
    along: f32,
    samples: Vec<(Vec2, Vec2)>,
    /// How many samples a `BEND_SPAN` is, and a `LONG_BEND_SPAN`.
    span: usize,
    long_span: usize,
    /// How far out a line may go, in metres, as `passing_room` works it out.
    passing_room: f32,
}

impl Ahead {
    fn of(course: &Course, along: f32, reach_across: f32) -> Self {
        let span = (BEND_SPAN / BEND_STEP).round() as usize;
        let long_span = (LONG_BEND_SPAN / BEND_STEP).round() as usize;
        let count = (HORIZON / BEND_STEP).round() as usize + long_span + 1;
        Self {
            along,
            samples: course.sample_ahead(along, BEND_STEP, count),
            span,
            long_span,
            passing_room: passing_room(course, reach_across),
        }
    }

    /// How far along the course sample `i` is, in metres.
    fn distance(&self, i: usize) -> f32 {
        i as f32 * BEND_STEP
    }

    /// How many samples have a bend measured from them, a long one too.
    fn bends(&self) -> usize {
        self.samples.len() - self.long_span
    }

    /// The course's change of direction over the `BEND_SPAN` from sample `i`, in radians,
    /// positive to the right.
    fn bend(&self, i: usize) -> f32 {
        self.samples[i].1.angle_to(self.samples[i + self.span].1)
    }

    /// The same over the `LONG_BEND_SPAN` from sample `i`.
    fn long_bend(&self, i: usize) -> f32 {
        self.samples[i]
            .1
            .angle_to(self.samples[i + self.long_span].1)
    }

    /// How far up its line a driver steers at, in metres: `reach`, or nearer where a bend
    /// within it would take the truck far across the inside of it (`BEND_CUT`). A `careful`
    /// driver cuts almost none of it (`CAREFUL_BEND_CUT`).
    fn steering_reach(&self, reach: f32, careful: bool) -> f32 {
        let (cut, nearest_allowed) = if careful {
            (CAREFUL_BEND_CUT, CAREFUL_NEAREST)
        } else {
            (BEND_CUT, *AIM_AHEAD.start())
        };
        let mut nearest = reach;
        for i in 0..self.bends() {
            if self.distance(i) > reach {
                break;
            }
            let bend = self.bend(i).abs();
            if bend > 1e-3 {
                let radius = BEND_SPAN / bend;
                nearest = nearest.min((2.0 * radius * cut).sqrt());
            }
        }
        nearest.max(nearest_allowed).min(reach)
    }

    /// Where a driver of this `style` steers at: the point of its line `reach` metres up the
    /// course. So it follows the road round every bend on its line, and a truck knocked off
    /// its line, or off the road, heads back to it within about that distance. A line is held
    /// to the course, however far out passing would take it. A driver that is behind steers
    /// at the same point: it catches up on its line.
    ///
    /// Its next checkpoint (`Style::gate`) comes first: nearer than `reach`, it steers
    /// through the gate (`NextGate::aim`).
    fn aim(&self, course: &Course, reach: f32, style: Style) -> Vec2 {
        let lane = style.lane.clamp(-self.passing_room, self.passing_room);
        if let Some(gate) = style.gate
            && gate.ahead <= reach
        {
            return gate.aim(lane, reach, style.reach_across);
        }
        let (point, direction) = course.point_at(self.along + reach);
        point - direction.perp() * lane
    }
}

/// How far along `course` each of the track's checkpoints is, in metres. It does not change
/// during a race, and a course whose corners are curves has hundreds of pieces to search,
/// so the systems find it once and keep it.
pub fn along_the_course(gates: &[Gate], course: &Course) -> Vec<f32> {
    gates
        .iter()
        .map(|gate| course.distance_along(&course.nearest(gate.center)))
        .collect()
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

#[cfg(test)]
mod tests {
    use bevy::math::{Quat, Vec3, Vec3Swizzles};
    use content::truck::TruckConfig;

    use super::*;
    use crate::fixtures::{built_in, course};

    /// A driver on the middle line, at full pace, with nobody to chase.
    fn steady() -> Style {
        Style {
            pace: 1.0,
            lane: 0.0,
            chasing: 0.0,
            downhill: 0.0,
            reach_across: built_in().x,
            gate: None,
            careful: false,
        }
    }

    /// What `passing_room` allows on the 14 m course: a line out to 5.1 m either side of
    /// the middle, which is wide enough to pass a truck on the centreline.
    fn room_to_pass() -> f32 {
        passing_room(&course(), built_in().x)
    }

    fn on_the_first_side(along: f32, off_to_the_right: f32, forward: Vec2, speed: f32) -> Controls {
        // Heading +X, the right is +Z: `perp` of (1, 0) is (0, 1).
        let position = Vec2::new(along, off_to_the_right);
        plan(&course(), along, position, forward, speed, steady())
    }

    /// Where a driver at `along` on the first side steers at, 15 m up a line `lane` metres
    /// to the left.
    fn aim_from_the_first_side(along: f32, lane: f32) -> Vec2 {
        aim_of(along, Style { lane, ..steady() })
    }

    /// The same, for a driver of any `style`.
    fn aim_of(along: f32, style: Style) -> Vec2 {
        let course = course();
        Ahead::of(&course, along, style.reach_across).aim(&course, 15.0, style)
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
        assert_eq!(straight.steering_reach(reach, false), reach);
        // The corner at (300, 0) is a right angle: within reach of it, nearer, but never
        // nearer than the start of `AIM_AHEAD`.
        let near = Ahead::of(&course, 270.0, built_in().x).steering_reach(reach, false);
        assert!(near < reach && near >= *AIM_AHEAD.start(), "{near}");
        // At a speed that steers at no more than that already, nothing changes.
        let slow = Ahead::of(&course, 270.0, built_in().x);
        assert_eq!(
            slow.steering_reach(*AIM_AHEAD.start(), false),
            *AIM_AHEAD.start()
        );
    }

    #[test]
    fn a_driver_that_was_put_back_cuts_no_corner() {
        let course = course();
        let reach = *AIM_AHEAD.end();
        // On a straight it steers as far up the road as any driver.
        let straight = Ahead::of(&course, FAR_FROM_THE_CORNER, built_in().x);
        assert_eq!(straight.steering_reach(reach, true), reach);
        // Near the right angle at (300, 0) it steers at a nearer point of its line than any
        // other driver may, and so keeps to that line round the corner.
        let near = Ahead::of(&course, 270.0, built_in().x);
        let careful = near.steering_reach(reach, true);
        assert!(careful < *AIM_AHEAD.start(), "{careful}");
        assert!(careful >= CAREFUL_NEAREST, "{careful}");
        // 5 m before the corner, on its line, it steers at a point of the next side: the
        // straight way there passes inside the corner, and less far inside when careful.
        let at = Vec2::new(295.0, 0.0);
        let corner = Vec2::new(300.0, 0.0);
        let inside_by = |careful: bool| {
            let ahead = Ahead::of(&course, at.x, built_in().x);
            let aim = ahead.aim(
                &course,
                ahead.steering_reach(reach, careful),
                Style {
                    careful,
                    ..steady()
                },
            );
            (corner - at).perp_dot((aim - at).normalize()).abs()
        };
        assert!(
            inside_by(true) < inside_by(false) - 1.0,
            "{} {}",
            inside_by(true),
            inside_by(false)
        );
    }

    #[test]
    fn a_driver_that_is_behind_takes_the_same_line_round_a_corner() {
        // Coming up to the corner at (300, 0), which turns right onto the side along +Z, and
        // in it. However desperate, a driver steers as one that races its own race: it catches
        // up on its line, and does not cut across the inside of the bend.
        for along in [250.0, 280.0, 290.0, 296.0] {
            for chasing in [0.3, 1.0] {
                let style = |chasing: f32| Style {
                    chasing,
                    ..steady()
                };
                let at = |chasing: f32| {
                    plan(
                        &course(),
                        along,
                        Vec2::new(along, 0.0),
                        Vec2::X,
                        20.0,
                        style(chasing),
                    )
                };
                assert_eq!(aim_of(along, style(chasing)), aim_of(along, steady()));
                assert_eq!(at(chasing).steer, at(0.0).steer, "{along} m, {chasing}");
            }
        }
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
        aim_of(
            along,
            Style {
                gate: Some(gate),
                ..style
            },
        )
    }

    #[test]
    fn near_its_next_checkpoint_it_steers_straight_through_it() {
        // 4 m before the corner at (300, 0), with a gate 14 m up the course: nearer than it
        // steers ahead, so it steers straight through the gate, 15 m ahead, at the point 1 m
        // beyond the gate on the line through it.
        let through = aim_for_the_gate(296.0, gate_at(310.0, 0.0, 7.0), steady());
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

    /// The speed `plan` wants of a driver on the centreline of `course`, `along` metres round
    /// it and facing the way it goes: where its throttle goes from on to off.
    fn wanted_speed(course: &Course, along: f32) -> f32 {
        let (position, forward) = course.point_at(along);
        let throttle =
            |speed: f32| plan(course, along, position, forward, speed, steady()).throttle;
        let (mut slow, mut fast) = (1.0, CRUISE_SPEED + 10.0);
        for _ in 0..40 {
            let speed = (slow + fast) / 2.0;
            if throttle(speed) > 0.0 {
                slow = speed;
            } else {
                fast = speed;
            }
        }
        slow
    }

    /// A course of 300 m straights that turns 45 degrees at a time, `apart` metres from one
    /// turn to the next round each corner, and so right angles of two turns.
    fn corners_of_two_turns(apart: f32) -> Course {
        let directions = (0..8).map(|k| Vec2::from_angle(k as f32 * std::f32::consts::FRAC_PI_4));
        let mut point = Vec2::ZERO;
        let mut points = Vec::new();
        for (k, direction) in directions.enumerate() {
            points.push(point);
            point += direction * if k % 2 == 0 { 300.0 } else { apart };
        }
        Course::new(points, 14.0)
    }

    #[test]
    fn a_bend_that_goes_on_is_taken_at_the_speed_its_whole_radius_allows() {
        // A right angle of two half turns 50 m apart, as a course that leaves the corner out
        // makes it, 8 m before the first: the whole of it is a quarter circle of about 38 m,
        // and no 30 m of it turns more than 45 degrees.
        let right_angle = wanted_speed(&corners_of_two_turns(50.0), 292.0);
        let whole_radius = LONG_BEND_SPAN / std::f32::consts::FRAC_PI_2;
        let round_it = (LONG_BEND_CORNERING * whole_radius).sqrt();
        assert!(
            (right_angle - round_it).abs() < 0.1,
            "{right_angle} {round_it}"
        );
        // Half turns 300 m apart are kinks, and each is taken at the speed of a kink.
        let kink = wanted_speed(&corners_of_two_turns(300.0), 292.0);
        let as_a_kink = bend_speed(std::f32::consts::FRAC_PI_4, CORNERING, SLOWEST_BEND);
        assert!((kink - as_a_kink).abs() < 0.1, "{kink} {as_a_kink}");
        assert!(right_angle < kink);
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
        let drivers: Vec<_> = (0..7).map(Driver::new).collect();
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

    #[test]
    fn downhill_a_driver_brakes_sooner_for_the_same_corner() {
        // Coming up to the corner at 300 m at 30 m/s, on the middle line.
        let at = |along: f32, downhill: f32| {
            let style = Style {
                pace: 30.0 / CRUISE_SPEED,
                downhill,
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
        // Where along the road it first gets on the brakes.
        let brakes_at = |downhill: f32| {
            (0..300)
                .map(|metres| metres as f32)
                .find(|&along| at(along, downhill).throttle < 0.0)
                .expect("the corner is braked for somewhere")
        };
        let (level, steep, steeper) = (brakes_at(0.0), brakes_at(0.15), brakes_at(0.3));
        assert!(
            steep < level && steeper < steep,
            "{level}, {steep}, {steeper} m"
        );
        // Uphill counts as level: it is the brakes that bring it down, not the hill.
        assert_eq!(brakes_at(-0.2), level);
    }

    #[test]
    fn the_braking_a_driver_counts_on_falls_downhill_to_a_floor() {
        assert_eq!(braking_on(0.0), BRAKING);
        assert_eq!(braking_on(-0.5), BRAKING);
        assert!(braking_on(0.1) < BRAKING);
        assert_eq!(braking_on(1.0), LEAST_BRAKING);
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
