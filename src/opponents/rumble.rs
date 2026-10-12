//! The computer's drivers in a Summit Rumble (`race::RumbleSession`), where there is no
//! course to follow and no checkpoint to drive through: the `rumble` crate decides, and
//! this module hands it what each truck sees and writes what it decides to the truck's
//! `TruckInput`. Monster Truck Madness 2 drove no computer trucks in a rumble: all of this
//! is the game's own.
//!
//! Once a race, after the track is made, the way up the summit is worked out for the whole
//! arena as a `rumble::NavField` (`prepare_rumble`): every wall, ramp, rim and pit of the
//! ground and its ground boxes, and every crest a truck can fly off onto the summit, in
//! one search. Any way up is as good as another: each driver takes the quickest from where
//! it is. Each step a driver off the summit follows the field's path from where it is, a
//! few seconds of it at a time (`REPATH_EVERY`), at the speed the path's bends allow, or a
//! leap needs (`rumble::follow`); with not the room before a crest to get up to that speed,
//! it backs off first (`RUN_UP_ACCEL`), and each driver runs up its own lane of a crest
//! (`LANES`). Off the summit it follows a truck in its way rather than ram it. On the
//! summit it picks a rival to push off, the one with the most points before all
//! (`rumble::choose_target`), among them one that is about to arrive over the rim
//! (`ARRIVING`), which it meets there; aims just inside it so that the hit carries it
//! outward; and never goes faster than it can stop in before its own rim
//! (`rumble::edge_speed`, `rumble::NavField::run_off`, and a look at the real ground ahead
//! for a drop the field misses, `DROP_LOOK`), except that on normal and hard it counts on
//! the hit itself to slow it when a rival that is all but standing is square in front
//! (`rumble::charge_speed`, `CHARGE_STANDING`). A rival it has in contact ahead of it (`rumble::pushing`,
//! `PUSH_GAP`) it pushes all the way to the rim, whoever else is worth more: the rival is
//! its buffer before the drop, so it fights on however near the drop it comes and means to
//! stop only once its own nose is nearly there (`PUSH_MARGIN`), the rival carried over. A
//! pushing match that has stopped (`STALLED`, `RAM_STALL`) it breaks off by backing up and
//! hitting again (`RAM_BACK`), and one that stalls again it gives up for a while
//! (`BREAK_OFF`). Earning nothing, it fights only a rival near it, and
//! otherwise goes back to the zone. With no one up there it waits at the
//! safest place of the scoring zone, furthest from any drop
//! (`rumble::NavField::hold_point`), nose to whoever comes (`rumble::hold_aim`); and on
//! hard it sidesteps a truck about to hit it (`rumble::threat`). Stopped at the rim with
//! its way behind it, it backs up rather than turn out over the edge. Off the summit,
//! getting stuck is handled as on a circuit (`Recovery`, and the race's put-back, which in
//! a rumble is the truck's grid place). On it, a pushing match that goes nowhere is the
//! rumble, not being stuck: a truck is never put back off the summit, which would cost it
//! a fall, unless it lies on its side or its roof (`TIPPED`), which nothing else rights.
//! Whether a truck is on the summit is the field's `on_top`, by the surface it stands on,
//! as the scoring judges it.

use std::f32::consts::FRAC_PI_2;

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;
use rumble::{Limits, Me, Mode, NavField, NavParams, Rival, Waypoint};

// How a driver works the controls, and when it has got somewhere, are the circuit's, and
// the same in a rumble.
use ::circuit::{FULL_LOCK_ANGLE, PROGRESS, throttle_for};

use super::{
    AIRBORNE, ComputerDriver, Controls, Difficulty, Driving, OpponentsSettings, REVERSING,
    Recovery, SETTLE, Watching,
};
use crate::collision_groups::GROUND;
use crate::game_state::GameState;
use crate::race::{BackToCheckpoint, RaceSystems, Racer, RumbleScore, RumbleSession, is_rumble};
use crate::track::{Track, TrackSystems};
use crate::truck::{PlayerTruck, TruckInput};

/// How often a climbing driver asks the field for its way again, in seconds, and how far
/// off its path it may be knocked before it asks at once, in metres.
const REPATH_EVERY: f32 = 0.5;
const OFF_PATH: f32 = 8.0;
/// How many places of the path a driver takes at a time: a few seconds of driving.
const PATH_PLACES: usize = 40;
/// How fast a driver creeps while holding the middle, in m/s.
const HOLD_SPEED: f32 = 6.0;
/// How far a driver looks for the rim along its nose, in metres.
const RIM_LOOK: f32 = 60.0;
/// Nearer the rim than this, in metres, with its way behind it, a driver backs up to turn.
const TURN_ROOM: f32 = 9.0;
/// How long a dodge is held, in seconds, and how far ahead it steers while dodging. A
/// dodge needs this much room before a drop, in metres, or it goes the other way, or not
/// at all.
const DODGE_FOR: f32 = 0.8;
const DODGE_AHEAD: f32 = 10.0;
const DODGE_ROOM: f32 = 10.0;
/// How much room behind it a driver on the summit needs to back up at all, in metres, and
/// how hard it brakes when it is backing with less: a trench the field misses was behind.
const BACK_ROOM: f32 = 8.0;
const BRAKE_BACK: f32 = 0.8;
/// A driver counts on the hit to slow it only at a rival all but standing along its nose:
/// going slower than this along it, in m/s. A rival that is itself going towards the rim,
/// fleeing or pushed, may never be hit before it: two chargers on Hypercube held full
/// throttle to the drop at 13 m/s with their rival 5 m ahead the whole way, and went over.
const CHARGE_STANDING: f32 = 3.0;
/// How far along its nose, and behind it when it backs, a driver on the summit looks at
/// the real ground for a drop, in metres, a metre at a time, and how far below what it
/// stands on the ground is a drop (the field's own rule). The field knows the rim to
/// within half a node and misses a trench narrower than a node: Hypercube's, which
/// drivers backed into.
const DROP_LOOK: f32 = 8.0;
const DROP: f32 = 1.5;
/// How far in from every drop a driver that has got onto the summit keeps following its
/// way, in metres, before it turns to the fight: a truck that turned round on the rim it
/// had just climbed over fell back off it. One node of the field.
const CLIMB_UNTIL: f32 = 3.5;
/// Slower than this, in m/s, with nowhere to go, a driver on the summit holds its truck
/// with the handbrake: a slope at the rim of a pit rolled it in.
const STANDING: f32 = 1.5;
/// How near a drop beside it a driver on the summit lets its middle come, in metres,
/// beyond what its truck reaches sideways: nearer, it steers away from it.
const SIDE_MARGIN: f32 = 1.5;
/// How hard it steers away from a drop beside it, from 0 to 1 of full lock.
const SIDE_STEER: f32 = 0.6;
/// On easy, a driver goes after rivals within this many metres only.
const EASY_REACH: f32 = 25.0;
/// A rival this far outside the summit, in metres, and no further below its top than
/// this, is about to arrive over the rim: a driver goes to meet it there.
const ARRIVING: f32 = 8.0;
const ARRIVING_BELOW: f32 = 6.0;
/// A fight that has stalled: slower than this, in m/s, with its rival within this many
/// metres, for this long, in seconds, a driver backs off for this long, in seconds, and
/// hits it again. All but stopped: a push that moves at all is carried on to the rim (the
/// user asked for drivers to back off only when they are stopped).
const STALLED: f32 = 0.5;
const RAM_REACH: f32 = 8.0;
const RAM_STALL: f32 = 1.0;
const RAM_BACK: f32 = 0.8;
/// A rival this much further than two trucks' reach ahead of a driver's nose, in metres,
/// is in contact with it, or about to be: the one it is pushing. While it pushes, the rival
/// is its buffer before the rim: it fights on however near the drop it comes, and means to
/// stop only this far short of the rim, in metres, beyond what its truck reaches, so that
/// the rival, a truck's length ahead, is carried over before it stops. Less than the
/// usual margin (`Skill::margin`): the rim is known to within 1.6 m, and a pusher may go
/// over after its rival. The user asked for drivers to push all the way to the edge.
const PUSH_GAP: f32 = 1.5;
const PUSH_MARGIN: f32 = 1.0;
/// After this many stalled pushes of one fight, a driver breaks it off for this long, in
/// seconds, and holds the scoring zone instead.
const STALLS: u32 = 2;
const BREAK_OFF: f32 = 4.0;
/// A run-up to a leap: about how hard a truck gets going from rest, in m/s², and how
/// much of the run its crest's own slope eats, in metres (measured on Arena Rumble: a
/// built-in truck is doing 16 m/s at the crest from 32 m back, 13 from 27 m); how far a
/// driver that has not the room backs off first, in metres, at most; and how near a
/// wall behind it it backs. Three of the Arena's grid places are 26 to 38 m from a
/// crest, and a truck that went off one at 7 m/s landed under the platform.
const RUN_UP_ACCEL: f32 = 4.0;
const RUN_UP_CREST: f32 = 8.0;
/// Faster than this, in m/s, a driver is on its run and goes for the leap whatever the
/// room: backing off now would only lose what it has.
const RETREAT_BELOW: f32 = 6.0;
/// On a run-up, how many m/s short of the leap's speed a driver comes off full throttle:
/// the usual band (`THROTTLE_BAND`) left it 2 m/s short at the crest, and short of the
/// platform.
const RUN_UP_BAND: f32 = 0.5;
/// A truck on its side or its roof (its up this far from upright, as the cosine) for
/// this long, in seconds, is put back, even from the summit, which costs it a fall: it
/// earns nothing lying there, and nothing else rights it.
const TIPPED: f32 = 0.5;
const TIPPED_FOR: f32 = 3.0;
/// How many lanes the drivers spread over, up a crest beside the one the field found,
/// so that they do not all queue for one run-up.
const LANES: usize = 5;
/// Off the summit, a rival straight ahead within this, in metres, is not rammed: a
/// moving one is followed at its speed, and a stopped one is gone round, this far to
/// its side, in metres.
const FOLLOW_GAP: f32 = 10.0;
const GO_ROUND: f32 = 4.0;
const GO_ROUND_SPEED: f32 = 8.0;
const RETREAT_MOST: f32 = 35.0;
const RETREAT_ROOM: f32 = 6.0;
/// Backing off slower than this, in m/s, for this long, in seconds, it has backed into
/// something, and stops backing.
const RETREAT_STALLED: f32 = 0.5;
const RETREAT_STALL: f32 = 0.6;
/// How far beyond the furthest grid place the field reaches, in metres, and the least it
/// reaches from the summit's middle.
const FIELD_MARGIN: f32 = 60.0;
const FIELD_LEAST: f32 = 150.0;

/// The way up the summit of the rumble being run, for every driver. Only while one is.
#[derive(Resource)]
pub(super) struct RumbleNav {
    field: NavField,
    /// Where the fight is about: the safest place of the scoring zone, or its middle
    /// where the field finds no top at all.
    centre: Vec2,
    /// About how far the summit's rim is from its middle, in metres.
    reach: f32,
}

/// What a driver keeps from one step to the next in a rumble.
#[derive(Debug, Default)]
pub(super) struct RumbleDriver {
    /// The places of its way up, in order, from `path_from`.
    path: Vec<Waypoint>,
    /// Seconds until it asks for its way again.
    repath_in: f32,
    /// Seconds left of a dodge, and which way it goes.
    dodging: f32,
    dodge: Vec2,
    /// How long its fight has been stalled, in seconds, how long it has left of backing
    /// off to hit again, how many times this fight has stalled, and how long it has left
    /// of having broken the fight off.
    stalled: f32,
    backing: f32,
    stalls: u32,
    broken_off: f32,
    /// Backing off for a run-up to a leap: how far it has still to go, in metres, how
    /// long it has not been moving back, in seconds, and whether it has done so for the
    /// crest ahead, so that it does not do it twice.
    retreat: Option<f32>,
    retreat_stalled: f32,
    retreated: bool,
    /// Whether its way has a leap coming up, which it is committed to: the way is not
    /// asked for again until it has landed, since from the crest the field's way back
    /// is round, not over.
    committed: bool,
    /// How long it has been on its side or its roof, in seconds.
    tipped: f32,
    /// Where it was when it last got somewhere.
    got_at: Vec2,
}

/// What each level of difficulty makes of a driver: the harder, the faster and the nearer
/// its own rim it fights, and the more it risks.
#[derive(Clone, Copy, Debug)]
struct Skill {
    /// How far it looks for a rival to push, in metres.
    reach: f32,
    /// Whether it gets out of the way of a truck about to hit it.
    dodges: bool,
    /// How fast it goes at a rival, in m/s, before the rim holds it back.
    fight_speed: f32,
    /// The braking it counts on up on the summit, in m/s², and how far short of the rim
    /// it means to stop, in metres, beyond what its own truck reaches. The field knows
    /// the rim to within half a node (1.6 m), and a truck whose front tires go over it
    /// tips: counting on more braking than the truck has, or on less than 2 m, put the
    /// drivers into Hypercube's pits and off Arena's platform at a walk.
    braking: f32,
    margin: f32,
    /// Whether it counts on the hit itself to slow it, and so goes at a rival square in
    /// front of it faster than it could stop before the rim on its own.
    charges: bool,
}

fn skill(difficulty: Difficulty) -> Skill {
    match difficulty {
        Difficulty::Easy => Skill {
            reach: EASY_REACH,
            dodges: false,
            fight_speed: 16.0,
            braking: 5.0,
            margin: 2.0,
            charges: false,
        },
        Difficulty::Normal => Skill {
            reach: f32::INFINITY,
            dodges: false,
            fight_speed: 24.0,
            braking: 5.0,
            margin: 2.0,
            charges: true,
        },
        Difficulty::Hard => Skill {
            reach: f32::INFINITY,
            dodges: true,
            fight_speed: 30.0,
            braking: 5.0,
            margin: 2.0,
            charges: true,
        },
    }
}

/// Works the field out for a rumble track, once a race.
pub(super) fn prepare_rumble(mut commands: Commands, track: Res<Track>) {
    let Some(zones) = &track.rumble else {
        return;
    };
    let centre = zones.scoring.center;
    let radius = std::iter::once(&track.start)
        .chain(&track.grid)
        .map(|place| place.position.distance(centre))
        .fold(FIELD_LEAST - FIELD_MARGIN, f32::max)
        + FIELD_MARGIN;
    let field = NavField::build(
        &track.heights,
        &track.ground_boxes,
        &zones.scoring,
        &zones.summit,
        centre,
        radius,
        NavParams::default(),
    );
    info!(
        "Rumble: the safest place on the summit is {:?}; the zone's middle is {centre}",
        field.hold_point()
    );
    commands.insert_resource(RumbleNav {
        centre: field.hold_point().unwrap_or(centre),
        field,
        reach: zones.summit.half_extents.max_element(),
    });
}

pub(super) fn forget_rumble(mut commands: Commands) {
    commands.remove_resource::<RumbleNav>();
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(GameState::Racing),
        prepare_rumble
            .after(TrackSystems::Prepare)
            .run_if(is_rumble),
    )
    .add_systems(OnExit(GameState::Racing), forget_rumble)
    .add_systems(
        Update,
        // So that the race sees the request in the same frame.
        ask_to_be_put_back
            .before(RaceSystems::BackToCheckpoint)
            .run_if(in_state(GameState::Racing))
            .run_if(is_rumble),
    )
    .add_systems(
        FixedUpdate,
        drive_rumble
            .run_if(in_state(GameState::Racing))
            .run_if(resource_exists::<RumbleNav>),
    );
}

/// In `Update`, where the race puts trucks back: a driver that is stuck, which with no
/// checkpoint to go back to is put on the grid. A driver on the summit is never stuck
/// (`drive_rumble`): put back from up there would cost it a fall.
fn ask_to_be_put_back(
    mut drivers: Query<(Entity, &mut ComputerDriver, Option<&Name>), With<Racer>>,
    mut back: MessageWriter<BackToCheckpoint>,
) {
    for (truck, mut driver, name) in &mut drivers {
        if driver.is_stuck() {
            info!(
                "{}: back to the grid, having got stuck",
                name.map_or("A computer truck", |name| name.as_str()),
            );
            super::put_back(&mut back, truck, &mut driver);
        }
    }
}

/// A truck as every driver sees it this step, taken before any of them is driven.
struct Seen {
    entity: Entity,
    rival: Rival,
}

/// In `FixedUpdate`, with the forces, as `drive` is on a circuit.
#[expect(
    clippy::too_many_arguments,
    reason = "the whole race, and the field over it"
)]
pub(super) fn drive_rumble(
    time: Res<Time>,
    track: Res<Track>,
    settings: Res<OpponentsSettings>,
    nav: Res<RumbleNav>,
    session: Option<Res<RumbleSession>>,
    spatial: SpatialQuery,
    mut drivers: Query<Driving>,
    player: Query<Watching, (With<PlayerTruck>, Without<ComputerDriver>)>,
    scores: Query<&RumbleScore>,
) {
    let Some(zones) = &track.rumble else {
        return;
    };
    let dt = time.delta_secs();
    let skill = skill(settings.difficulty);
    let limits = Limits::default();
    // Once the time is up, the drivers keep the trucks moving and fight on: the results
    // are up, and the scores are frozen.
    let _ = session;

    let seen = |entity: Entity, transform: &Transform, velocity: Vec3| {
        let middle = transform.translation;
        let on_summit = nav.field.on_top(middle.xz(), middle.y);
        // About to come over the rim: worth meeting there.
        let arriving = !on_summit
            && zones.summit.near(middle.xz(), ARRIVING)
            && middle.y >= zones.summit.bottom - ARRIVING_BELOW;
        Seen {
            entity,
            rival: Rival {
                position: middle.xz(),
                velocity: velocity.xz(),
                on_summit: on_summit || arriving,
                scoring: on_summit && zones.scoring.covers(middle.xz()),
                points: scores.get(entity).map_or(0.0, |score| score.0.points),
            },
        }
    };
    let mut trucks: Vec<Seen> = drivers
        .iter()
        .map(|(entity, transform, velocity, ..)| seen(entity, transform, velocity.0))
        .collect();
    if let Ok((entity, transform, velocity, _, _)) = player.single() {
        trucks.push(seen(entity, transform, velocity.0));
    }

    let the_ground = SpatialQueryFilter::from_mask(GROUND);
    let mut rivals: Vec<Rival> = Vec::new();
    for (lane, (entity, transform, velocity, mut config, mut input, mut driver, _, held)) in
        (&mut drivers).into_iter().enumerate()
    {
        let lane = (lane % LANES) as i32 - (LANES / 2) as i32;
        let position = transform.translation.xz();
        let height = transform.translation.y;
        if held {
            *input = TruckInput::default();
            driver.getting_nowhere = 0.0;
            driver.recovery = Recovery::default();
            driver.rumble.got_at = position;
            continue;
        }
        let forward = transform.forward().xz().normalize_or_zero();
        let speed = velocity.0.dot(transform.forward().as_vec3());
        let on_summit = nav.field.on_top(position, height);
        let me = Me {
            position,
            forward,
            speed,
            scoring: on_summit && zones.scoring.covers(position),
        };
        // Its share of its own engine for the difficulty.
        let (pull, top_speed) = *driver
            .engine
            .get_or_insert((config.engine_force, config.top_speed));
        config.engine_force = pull * settings.difficulty.power();
        config.top_speed = top_speed * settings.difficulty.power();

        // The others, as the tactics see them: on easy, only the near ones are worth going
        // after.
        rivals.clear();
        rivals.extend(
            trucks
                .iter()
                .filter(|other| other.entity != entity)
                .map(|other| {
                    let far = other.rival.position.distance(position) > skill.reach;
                    Rival {
                        on_summit: other.rival.on_summit && !far,
                        ..other.rival
                    }
                }),
        );

        let state = &mut driver.rumble;
        let room_along = |direction: Vec2| nav.field.run_off(position, height, direction, RIM_LOOK);
        // How far along `direction` the real ground first drops away under the truck, by a
        // ray down at each metre: between the last sample on the top and the first off it.
        // Infinite when it does not within `DROP_LOOK`.
        let stands_at = height + config.wheel_rest[0].y - config.wheel_radius;
        let look_from = height + 6.0;
        let drop_along = |direction: Vec2| -> f32 {
            let mut gone = 1.0;
            while gone <= DROP_LOOK {
                let at = position + direction * gone;
                let floor = spatial
                    .cast_ray(
                        Vec3::new(at.x, look_from, at.y),
                        Dir3::NEG_Y,
                        look_from - stands_at + DROP + 0.5,
                        true,
                        &the_ground,
                    )
                    .map(|hit| look_from - hit.distance);
                if floor.is_none_or(|floor| floor < stands_at - DROP) {
                    return gone - 0.5;
                }
                gone += 1.0;
            }
            f32::INFINITY
        };
        let room = if on_summit {
            room_along(forward).min(drop_along(forward))
        } else {
            room_along(forward)
        };
        // How far ahead of its middle the truck reaches: its front tires.
        let reach_ahead = config
            .wheel_rest
            .iter()
            .map(|w| w.z.abs())
            .fold(0.0, f32::max)
            + config.wheel_radius;
        // The rival it is pushing, if any: in contact ahead of it, and before its rim.
        let contact = 2.0 * reach_ahead + PUSH_GAP;
        let pushed = rivals
            .iter()
            .enumerate()
            .filter(|(_, rival)| rival.on_summit)
            .find_map(|(index, rival)| {
                rumble::pushing(&me, rival, contact)
                    .filter(|&ahead| ahead < room)
                    .map(|_| index)
            });
        // Just up, or knocked to the rim, it is still climbing: in from the drops first.
        // Unless it is pushing a rival there: that rival is its buffer.
        let clear = on_summit
            && (pushed.is_some() || nav.field.clear_of_drops(position, height) >= CLIMB_UNTIL);
        state.broken_off = (state.broken_off - dt).max(0.0);
        let mut mode = if state.dodging > 0.0 {
            Mode::Dodge(rumble::Dodge {
                direction: state.dodge,
            })
        } else {
            match rumble::decide(&me, clear, &rivals, nav.centre, nav.reach, skill.dodges) {
                // A fight it has broken off: back to the zone for a while.
                Mode::Fight(_) if state.broken_off > 0.0 => Mode::Hold,
                // A rival it is pushing it goes on pushing, whoever else is worth more.
                Mode::Fight(_) if let Some(target) = pushed => Mode::Fight(target),
                mode => mode,
            }
        };
        let pushing = matches!(mode, Mode::Fight(target) if pushed == Some(target));
        // A dodge goes to the side with room before a drop, or not at all: better hit than
        // over the rim or into a pit.
        if let Mode::Dodge(dodge) = mode
            && state.dodging <= 0.0
        {
            let (this_way, other_way) = (room_along(dodge.direction), room_along(-dodge.direction));
            mode = if this_way >= DODGE_ROOM {
                mode
            } else if other_way >= DODGE_ROOM {
                Mode::Dodge(rumble::Dodge {
                    direction: -dodge.direction,
                })
            } else {
                match rumble::choose_target(&me, &rivals, nav.centre, nav.reach) {
                    Some(target) => Mode::Fight(target),
                    None => Mode::Hold,
                }
            };
        }
        if !matches!(mode, Mode::Fight(_)) {
            state.stalled = 0.0;
            state.stalls = 0;
        }
        let (aim, mut wanted) = match mode {
            Mode::Climb => {
                state.repath_in -= dt;
                let off = state
                    .path
                    .iter()
                    .map(|place| place.at.distance(position))
                    .fold(f32::INFINITY, f32::min);
                if (state.repath_in <= 0.0 && !state.committed) || off > OFF_PATH {
                    state.path = nav.field.path_from(position, height, PATH_PLACES, lane);
                    state.repath_in = REPATH_EVERY;
                    state.committed = false;
                }
                match rumble::follow(&state.path, position, height, speed, &limits) {
                    Some(pursuit) => {
                        state.committed = pursuit.leap_in.is_some();
                        // A leap ahead that it has not the room to get up to speed for:
                        // it backs off first, once.
                        match pursuit.leap_in {
                            Some(to_crest) if speed < RETREAT_BELOW => {
                                let run = (pursuit.speed.powi(2) - speed.max(0.0).powi(2))
                                    / (2.0 * RUN_UP_ACCEL)
                                    + RUN_UP_CREST;
                                if run > to_crest && !state.retreated && state.retreat.is_none() {
                                    state.retreat = Some((run - to_crest).min(RETREAT_MOST));
                                    state.retreated = true;
                                }
                            }
                            Some(_) => {}
                            None => state.retreated = false,
                        }
                        // A rival straight ahead is not rammed down here: a moving one
                        // is followed, a stopped one gone round.
                        let mut aim = pursuit.aim;
                        let mut wanted = pursuit.speed;
                        let ahead = rivals
                            .iter()
                            .filter_map(|rival| {
                                rumble::ahead_in_lane(&me, rival).map(|gap| (gap, rival))
                            })
                            .filter(|(gap, _)| *gap < FOLLOW_GAP)
                            .min_by(|a, b| a.0.total_cmp(&b.0));
                        if let Some((_, rival)) = ahead {
                            let its_speed = rival.velocity.dot(forward);
                            if its_speed > STANDING {
                                wanted = wanted.min(its_speed);
                            } else {
                                aim += -forward.perp() * GO_ROUND;
                                wanted = wanted.min(GO_ROUND_SPEED);
                            }
                        }
                        (aim, wanted)
                    }
                    // No way from here: towards the middle, and the rules for being stuck.
                    None => (nav.centre, limits.arrival),
                }
            }
            Mode::Fight(target) => {
                let rival = &rivals[target];
                // A pushing match going nowhere: back off, and hit it again.
                if speed.abs() < STALLED && rival.position.distance(position) < RAM_REACH {
                    state.stalled += dt;
                } else {
                    state.stalled = 0.0;
                }
                if state.stalled >= RAM_STALL {
                    state.stalled = 0.0;
                    state.stalls += 1;
                    if state.stalls >= STALLS {
                        state.stalls = 0;
                        state.broken_off = BREAK_OFF;
                    } else {
                        state.backing = RAM_BACK;
                    }
                }
                (
                    rumble::attack_aim(&me, rival, nav.centre),
                    skill.fight_speed,
                )
            }
            Mode::Hold => match rumble::hold_aim(&me, &rivals, nav.centre) {
                Some(aim) => (aim, HOLD_SPEED),
                None => (position + forward, 0.0),
            },
            Mode::Dodge(dodge) => {
                if state.dodging <= 0.0 {
                    state.dodging = DODGE_FOR;
                    state.dodge = dodge.direction;
                }
                state.dodging -= dt;
                (position + dodge.direction * DODGE_AHEAD, skill.fight_speed)
            }
        };
        let ramming_back = state.backing > 0.0;
        if ramming_back {
            state.backing -= dt;
        }
        // Backing off for a run-up: straight back, as far as it meant to or a wall lets.
        let retreating = match state.retreat {
            Some(left) if matches!(mode, Mode::Climb) => {
                let back = transform.back();
                let wall = spatial
                    .cast_ray(
                        transform.translation,
                        Dir3::new(back.as_vec3()).unwrap_or(Dir3::NEG_Z),
                        RETREAT_ROOM,
                        true,
                        &the_ground,
                    )
                    .is_some();
                let gone = (-speed).max(0.0) * dt;
                state.retreat_stalled = if -speed < RETREAT_STALLED {
                    state.retreat_stalled + dt
                } else {
                    0.0
                };
                if wall || left <= gone || state.retreat_stalled > RETREAT_STALL {
                    state.retreat = None;
                    false
                } else {
                    state.retreat = Some(left - gone);
                    true
                }
            }
            _ => {
                state.retreat = None;
                state.retreat_stalled = 0.0;
                false
            }
        };
        // Up on the summit, never faster than it can stop before the rim: along its nose,
        // along the way it is actually going, which after a hit is not the same, and
        // along the way to its aim, which its nose is turning towards. Unless a rival is
        // square in front, and the hit will do the stopping.
        let to_aim = (aim - position).normalize_or_zero();
        if on_summit {
            let going = velocity.0.xz();
            let mut least = room.min(room_along(to_aim));
            if going.length() > 1.0 {
                least = least.min(room_along(going.normalize()));
            }
            // Pushing, the rival ahead is the buffer: nearly to the rim.
            let margin = reach_ahead + if pushing { PUSH_MARGIN } else { skill.margin };
            let mut cap = rumble::edge_speed(least, skill.braking, margin);
            if skill.charges
                && let Mode::Fight(target) = mode
                && let Some(to_rival) = rumble::ahead_in_lane(&me, &rivals[target])
                && to_rival < room
                && rivals[target].velocity.dot(forward) < CHARGE_STANDING
            {
                cap = cap.max(rumble::charge_speed(room, to_rival, skill.braking, margin));
            }
            wanted = wanted.min(cap);
        }

        // Positive when the aim is to the left.
        let off_straight = (-forward.perp_dot(to_aim)).atan2(forward.dot(to_aim));
        let mut steer = (off_straight / FULL_LOCK_ANGLE).clamp(-1.0, 1.0);
        // A drop right beside it, under its outer wheels: away from it, whatever the aim.
        if on_summit {
            let left = -forward.perp();
            let side_room = config
                .wheel_rest
                .iter()
                .map(|w| w.x.abs())
                .fold(0.0, f32::max)
                + config.wheel_width / 2.0
                + SIDE_MARGIN;
            let (room_left, room_right) = (room_along(left), room_along(-left));
            if room_left < side_room && room_right >= room_left {
                steer = steer.min(-SIDE_STEER);
            } else if room_right < side_room && room_left > room_right {
                steer = steer.max(SIDE_STEER);
            }
        }
        // Turning right round while climbing, as a truck under a leap's flight does to
        // get back to the crest: slowly, since it may be on a slope.
        if matches!(mode, Mode::Climb) && off_straight.abs() > FRAC_PI_2 {
            wanted = wanted.min(limits.slowest);
        }
        let planned = if ramming_back || retreating {
            // Straight back: to come at its rival again, or for a run-up.
            Controls {
                throttle: REVERSING,
                steer: 0.0,
            }
        } else if on_summit && room < TURN_ROOM && off_straight.abs() > FRAC_PI_2 {
            // At the rim with its way behind it: back up, nose swinging towards the way.
            Controls {
                throttle: REVERSING,
                steer: -steer.signum(),
            }
        } else if state.committed && !on_summit {
            // On a run-up: full throttle until it has the leap's speed.
            Controls {
                throttle: ((wanted - speed) / RUN_UP_BAND).clamp(-1.0, 1.0),
                steer,
            }
        } else {
            Controls {
                throttle: throttle_for(wanted, speed),
                steer,
            }
        };

        // Hands still in the air, and for a moment after landing, as on a circuit.
        let flying = config.wheel_radius - config.wheel_rest[0].y + AIRBORNE;
        let on_the_ground = height - track.heights.height_at(position.x, position.y) <= flying
            || spatial
                .cast_ray(
                    transform.translation,
                    Dir3::NEG_Y,
                    flying,
                    true,
                    &the_ground,
                )
                .is_some();
        driver.settling = if !on_the_ground {
            SETTLE
        } else {
            (driver.settling - dt).max(0.0)
        };
        let Controls {
            mut throttle,
            steer,
        } = driver.recovery.step(planned, speed, dt);
        // Stopped twice over on the summit is a pushing match, not being stuck: it is
        // never put back from up there, which would cost it a fall.
        if on_summit {
            driver.recovery.gave_up = false;
        }
        // Backing up on the summit needs room behind: a pit or the rim may be there. Already
        // going back, it brakes.
        if on_summit && throttle < 0.0 && room_along(-forward).min(drop_along(-forward)) < BACK_ROOM
        {
            throttle = if speed < -1.0 { BRAKE_BACK } else { 0.0 };
        }
        // Standing at a drop with nowhere to go, it holds its truck there.
        let standing = on_summit && wanted < STANDING && speed.abs() < STANDING && throttle >= 0.0;
        input.throttle = if standing { 0.0 } else { throttle };
        input.steer = if driver.settling > 0.0 { 0.0 } else { steer };
        input.handbrake = standing;
        input.shifts = 0;

        // Getting somewhere is moving, off the summit. On it, standing still is holding
        // the middle or a pushing match, and backing up to hit again is the fight. On its
        // side or its roof it is going nowhere anywhere.
        let state = &mut driver.rumble;
        state.tipped = if transform.up().y < TIPPED {
            state.tipped + dt
        } else {
            0.0
        };
        if state.tipped > TIPPED_FOR {
            driver.getting_nowhere = f32::INFINITY;
        } else if on_summit || position.distance(state.got_at) >= PROGRESS {
            state.got_at = position;
            driver.getting_nowhere = 0.0;
            driver.recovery.got_somewhere();
        } else {
            driver.getting_nowhere += dt;
        }
    }
}
