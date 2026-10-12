//! The trucks that the computer drives: every `truck::Truck` that is not the
//! `truck::PlayerTruck`. The `truck` slice builds them, from `truck::ComputerTrucks`, and
//! the `race` slice puts them on the grid and counts them as it does the player's. This
//! slice is the driver: it writes each one's `TruckInput`, which is all the player's keys
//! do, so a computer truck has no more grip than the player's. Its power is that of its
//! truck's engine, scaled by the `Difficulty`: all of it on hard, less on normal and less
//! again on easy.
//!
//! Which race it is comes from the track, and only that race's drivers are run: on a
//! circuit (`circuit`) the `circuit` crate decides, following the course round and through
//! the checkpoints; in a Summit Rumble (`rumble`) the `rumble` crate decides, climbing the
//! summit and pushing the others off it. Each module hands its crate what the truck sees
//! and writes what the crate decides to the truck's `TruckInput`. What every driver shares
//! is here: the `Difficulty`, its truck's own engine, its hands off the wheel while it flies
//! and lands (`SETTLE`), and getting unstuck. A driver that stops against something backs
//! up first, turning the opposite way to the turn it wants, and then goes on (`Recovery`).
//! If it stops again before it has got anywhere, backing up has failed, and it asks the
//! race to put it back at its last checkpoint, as the player does with a key
//! (`race::BackToCheckpoint`). So does one that gets nowhere for `PATIENCE` seconds without
//! ever stopping (on its roof, off down a mountainside).
//!
//! Once the player's truck has finished, a driver takes its wheel as well
//! (`take_the_players_wheel`), and drives it on with the others. It gets `truck::Autopilot`,
//! so that the keys leave it alone. `lead_driver` lends the circuit's rules to
//! `--autopilot` (`diagnostics`), which presses the player's keys to drive as the quickest
//! driver would on a clear road.
//!
//! Uses the `track` slice for the course, the `truck` slice for the trucks and the `race`
//! slice for the checkpoints and the scores. Needs `RacePlugin`, and the physics, which
//! says where the ground is under a truck.

mod circuit;
mod rumble;

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;

use crate::game_state::GameState;
use crate::race::{BackToCheckpoint, Racer};
use crate::truck::{Autopilot, Held, PlayerTruck, Truck, TruckConfig, TruckInput};

pub use ::circuit::{Controls, lead_driver};

/// The share of its truck's own engine (pull and top speed) a driver has on easy and on
/// normal. Hard has all of it, as much as the player's truck. Lower is slower on the
/// straights and up the hills; much lower and the trucks are left behind on the first
/// straight.
const EASY_POWER: f32 = 0.8;
const NORMAL_POWER: f32 = 0.9;
/// The share of `CHASING_POWER` a driver has on easy and on normal, so that it gives up a
/// lead it has lost. Hard has all of it.
const EASY_CATCHING_UP: f32 = 0.25;
const NORMAL_CATCHING_UP: f32 = 0.5;
/// How far above its ride height a truck's middle has to be, in metres, for its driver to
/// take it to be in the air, and how long after it comes down it leaves the steering alone,
/// in seconds. Steering hard while a truck flies or lands, when an impact has just spun it,
/// is what rolls it: from eight starts on Alpine, 68 of 77 rolls were at the ditch after
/// checkpoint 2, and with the wheel held straight through the ditch, 10. Bumps
/// and crests lift a truck on the road by up to a metre, so less than that takes the
/// steering away from a driver on the ground. It is measured down to the ground under the
/// truck, ground boxes included: on a bridge or a deck the terrain is far below, and a
/// driver that took that for flying did not steer across it. Round and Round's bridge over
/// the lake is 18 m above the terrain, with a bend at either end of it. Measured in eight
/// races of 240 s: the trucks went 5 % further on Round and Round and 3 % on Winding Way.
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

/// A driver that has not got `circuit::PROGRESS` metres within this many seconds is stuck,
/// whatever it is doing. The slowest bend is driven at 7 m/s.
const PATIENCE: f32 = 10.0;

pub struct OpponentsPlugin;

impl Plugin for OpponentsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OpponentsSettings>()
            .add_systems(
                Update,
                (take_the_wheel, take_the_players_wheel).run_if(in_state(GameState::Racing)),
            )
            .add_plugins((circuit::plugin, rumble::plugin));
    }
}

/// How the computer drives. Set it before a race: the course it chooses is the one the
/// drivers measure where they are on.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct OpponentsSettings {
    pub difficulty: Difficulty,
}

/// How hard the computer's drivers are to beat: the course they follow, and the power they
/// have (see the module's doc).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Self; 3] = [Self::Easy, Self::Normal, Self::Hard];

    /// Which of `TrackData::other_courses` the drivers follow, or `None` for the main
    /// course.
    fn other_course(self) -> Option<usize> {
        match self {
            Self::Easy => None,
            Self::Normal => Some(0),
            Self::Hard => Some(1),
        }
    }

    /// The share of its truck's own engine that a driver has.
    fn power(self) -> f32 {
        match self {
            Self::Easy => EASY_POWER,
            Self::Normal => NORMAL_POWER,
            Self::Hard => 1.0,
        }
    }

    /// The share of `CHASING_POWER` that a driver has.
    fn catching_up(self) -> f32 {
        match self {
            Self::Easy => EASY_CATCHING_UP,
            Self::Normal => NORMAL_CATCHING_UP,
            Self::Hard => 1.0,
        }
    }
}

/// On every truck that the computer drives.
#[derive(Component, Debug)]
pub struct ComputerDriver {
    /// What it keeps on a circuit, and in a Summit Rumble.
    circuit: ::circuit::Driver,
    rumble: rumble::RumbleDriver,
    /// How long it has been getting nowhere, in seconds, as each race's driver counts it.
    getting_nowhere: f32,
    /// How much longer, in seconds, it leaves the steering alone after flying (`SETTLE`).
    settling: f32,
    /// Getting unstuck, by backing up and turning round (`Recovery`).
    recovery: Recovery,
    /// Its truck's own engine, before the difficulty and the catch-up scale it: its pull in
    /// newtons and the speed where that runs out in m/s. Taken the first time the driver
    /// drives.
    engine: Option<(f32, f32)>,
}

impl ComputerDriver {
    /// Driver `number` of a race, from 0.
    fn new(number: usize) -> Self {
        Self {
            circuit: ::circuit::Driver::new(number),
            rumble: rumble::RumbleDriver::default(),
            getting_nowhere: 0.0,
            settling: 0.0,
            recovery: Recovery::default(),
            engine: None,
        }
    }

    /// Whether it is to be put back: stopped again after backing up, or getting nowhere
    /// however it tries.
    fn is_stuck(&self) -> bool {
        self.recovery.gave_up || self.getting_nowhere > PATIENCE
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

/// A truck that nobody drives: not the player, and not a computer driver yet.
type Undriven = (With<Truck>, Without<PlayerTruck>, Without<ComputerDriver>);

/// Asks the race to put `truck` back, and has its `driver` start again from there.
fn put_back(
    back: &mut MessageWriter<BackToCheckpoint>,
    truck: Entity,
    driver: &mut ComputerDriver,
) {
    back.write(BackToCheckpoint { truck });
    driver.getting_nowhere = 0.0;
    driver.recovery = Recovery::default();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::{Course, TrackData};

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

    #[test]
    fn each_difficulty_follows_its_own_course_or_else_the_main_one() {
        // Told apart by how long a lap is.
        let other = |side: f32| {
            Some(Course::new(
                vec![Vec2::ZERO, Vec2::X * side, Vec2::Y * side],
                14.0,
            ))
        };
        let lap = |track: &TrackData, difficulty: Difficulty| {
            ::circuit::course_for(track, difficulty.other_course())
                .unwrap()
                .length()
        };
        let track = TrackData {
            course: Some(course()),
            other_courses: vec![other(100.0), other(10.0), other(1.0)],
            ..crate::track::builtin_track()
        };
        assert_eq!(lap(&track, Difficulty::Easy), course().length());
        assert_eq!(
            lap(&track, Difficulty::Normal),
            other(100.0).unwrap().length()
        );
        assert_eq!(lap(&track, Difficulty::Hard), other(10.0).unwrap().length());

        // A track that leaves its shortcut out, and one with no other courses at all.
        let without = |other_courses| TrackData {
            other_courses,
            ..track.clone()
        };
        let no_shortcut = without(vec![other(100.0), None]);
        assert_eq!(lap(&no_shortcut, Difficulty::Hard), course().length());
        assert_eq!(
            lap(&without(Vec::new()), Difficulty::Normal),
            course().length()
        );
    }

    #[test]
    fn the_harder_the_quicker() {
        let [easy, normal, hard] = Difficulty::ALL;
        assert!(easy.power() < normal.power() && normal.power() < hard.power());
        assert!(easy.catching_up() < normal.catching_up());
        assert!(normal.catching_up() < hard.catching_up());
        assert_eq!(Difficulty::default(), normal);
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
}
