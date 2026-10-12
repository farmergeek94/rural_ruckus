//! The computer's drivers on a circuit: the `circuit` crate decides, and this module hands
//! it what each truck sees and writes what it decides to the truck's `TruckInput`.
//!
//! The `Difficulty` chooses the course that every driver of a race follows: on easy
//! `TrackData::course`, on normal the track's first other course, and on hard its second,
//! which is the one Monster Truck Madness 2's own computer trucks follow, and often takes
//! shortcuts (Crazy '98, Sidewinder Canyon). A track that does not have the one chosen has
//! its main course followed. The other courses have their corners put back
//! (`track::TrackData::other_courses`), so that a driver on a shortcut keeps to the middle
//! of its road round the bends. The difficulty scales the drivers' engines and catch-up
//! power too: when it is behind, a driver's engine is given more, so that it catches up on
//! the straights (`circuit::Skill`). The player's truck is given the same catch-up when it
//! is behind, by the same rule (`circuit::Race::catch_up`), as the user asked: the
//! difficulty's share of the catch-up, and none of its cut of the drivers' engines. Where
//! each checkpoint is along the course, and what fixed scenery stands on it, is found once
//! a race (`circuit::Route`). A driver that gets
//! nowhere, or that has gone past its next checkpoint without driving through it, asks the
//! race to put it back at its last checkpoint. A track with no course has nothing to
//! follow, and its computer trucks stand still.

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;

use super::{
    AIRBORNE, Boosted, ComputerDriver, Controls, Difficulty, Driving, OpponentsSettings,
    OwnEngine, Recovery, SETTLE,
};
use crate::collision_groups::GROUND;
use crate::game_state::GameState;
use crate::race::{BackToCheckpoint, RaceSystems, Racer, is_circuit};
use crate::track::Track;
use crate::truck::{PlayerTruck, TruckInput};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        // So that the race sees the request in the same frame.
        ask_to_be_put_back
            .before(RaceSystems::BackToCheckpoint)
            .run_if(in_state(GameState::Racing))
            .run_if(is_circuit),
    )
    // With the forces, so that a driver does the same at any frame rate.
    .add_systems(
        FixedUpdate,
        drive.run_if(in_state(GameState::Racing)).run_if(is_circuit),
    );
}

/// What the difficulty makes of a driver on a circuit.
fn skill(difficulty: Difficulty) -> ::circuit::Skill {
    ::circuit::Skill {
        power: difficulty.power(),
        catching_up: difficulty.catching_up(),
    }
}

/// What `drive` keeps from one step to the next.
#[derive(Default)]
struct Remembered {
    /// Where the player's truck was last found round the course, as a driver remembers its
    /// own: it is one of the trucks to catch and to pass, but nobody drives it.
    player_along: Option<f32>,
    /// The course and the checkpoints of this race, as the drivers know them.
    route: Option<::circuit::Route>,
    /// Every truck in the race and where it is this step, kept so that no step allocates.
    ids: Vec<Entity>,
    trucks: Vec<::circuit::OnCourse>,
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    time: Res<Time>,
    track: Res<Track>,
    settings: Res<OpponentsSettings>,
    spatial: SpatialQuery,
    mut drivers: Query<Driving>,
    mut player: Query<Boosted, (With<PlayerTruck>, Without<ComputerDriver>)>,
    mut remembered: Local<Remembered>,
) {
    let difficulty = settings.difficulty;
    let Some(course) = ::circuit::course_for(&track, difficulty.other_course()) else {
        return;
    };
    let Remembered {
        player_along,
        route,
        ids,
        trucks,
    } = &mut *remembered;
    if track.is_changed()
        || settings.is_changed()
        || route
            .as_ref()
            .is_none_or(|route| route.gates() != track.gates.len())
    {
        *route = Some(::circuit::Route::new(&track, course));
    }
    let route = route.as_ref().expect("found just above");
    let places = ::circuit::Places::new(course, route);
    // A truck is enlisted in the race a frame after it is spawned.
    let lap_number = |racer: Option<&Racer>| racer.map_or(0, |racer| racer.progress.lap);

    // Where every truck is, before any of them is driven: what a driver has to catch, and
    // what is in its way. Taken here so that no driver answers to half-moved trucks.
    ids.clear();
    trucks.clear();
    for (entity, transform, velocity, config, _, driver, racer, _) in &drivers {
        ids.push(entity);
        trucks.push(places.of(
            transform.translation.xz(),
            velocity.0.xz(),
            config,
            driver.circuit.along(),
            lap_number(racer),
        ));
    }
    if let Ok((entity, transform, velocity, config, racer, _)) = player.single() {
        let player = places.of(
            transform.translation.xz(),
            velocity.0.xz(),
            config,
            *player_along,
            lap_number(racer),
        );
        // Off the course altogether, its place is not to be trusted: look again everywhere.
        *player_along = player.found();
        ids.push(entity);
        trucks.push(player);
    }
    let race = ::circuit::Race::new(course, &track, route, trucks);
    let skill = skill(difficulty);
    let lap = course.length();
    let dt = time.delta_secs();

    // The terrain and the ground boxes, and not the trucks or the scenery.
    let the_ground = SpatialQueryFilter::from_mask(GROUND);
    for (entity, transform, velocity, mut config, mut input, mut driver, racer, held) in
        &mut drivers
    {
        let Some(me) = ids.iter().position(|id| *id == entity) else {
            continue;
        };
        let mine = &race.trucks[me];
        // Held on the grid, it waits for the start: going nowhere is not being stuck.
        if held {
            *input = TruckInput::default();
            driver.circuit.held_at(mine);
            driver.getting_nowhere = 0.0;
            driver.recovery = Recovery::default();
            continue;
        }
        let position = transform.translation.xz();
        let forward = transform.forward().xz().normalize_or_zero();
        let speed = velocity.0.dot(transform.forward().as_vec3());
        let progress = racer.map(|racer| &racer.progress);
        let decision = driver
            .circuit
            .decide(&race, me, position, forward, speed, progress, skill, dt);
        // Its share of its own engine for the difficulty, and more the further behind.
        let (pull, top_speed) = *driver
            .engine
            .get_or_insert((config.engine_force, config.top_speed));
        config.engine_force = pull * decision.boost;
        config.top_speed = top_speed * decision.boost;

        // Hands still in the air, and for a moment after landing. The terrain is cheap to
        // read, and nothing under the truck is lower: only above it, a ray looks for a box.
        let flying = config.wheel_radius - config.wheel_rest[0].y + AIRBORNE;
        let Vec3 { x, y, z } = transform.translation;
        let on_the_ground = y - track.heights.height_at(x, z) <= flying
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
        let Controls { throttle, steer } = driver.recovery.step(decision.controls, speed, dt);
        input.throttle = throttle;
        input.steer = if driver.settling > 0.0 { 0.0 } else { steer };
        input.handbrake = false;

        if driver.circuit.got_somewhere(mine, lap) {
            driver.getting_nowhere = 0.0;
            driver.recovery.got_somewhere();
        } else {
            driver.getting_nowhere += dt;
        }
    }

    // The player's truck, which was put last in `trucks`, gets the same catch-up as the
    // drivers when it is behind, on its own engine, which is kept the first time.
    if let Ok((entity, _, _, mut config, _, own)) = player.single_mut() {
        let me = race.trucks.len() - 1;
        let (pull, top_speed) = match own {
            Some(own) => (own.pull, own.top_speed),
            None => {
                let own = OwnEngine {
                    pull: config.engine_force,
                    top_speed: config.top_speed,
                };
                commands.entity(entity).insert(own);
                (own.pull, own.top_speed)
            }
        };
        let boost = race.catch_up(me, skill);
        config.engine_force = pull * boost;
        config.top_speed = top_speed * boost;
    }
}

/// In `Update`, where the race puts trucks back: a driver that is stuck, or that has gone
/// past the checkpoint it has to drive through next without doing so.
fn ask_to_be_put_back(
    track: Res<Track>,
    settings: Res<OpponentsSettings>,
    mut drivers: Query<(Entity, &mut ComputerDriver, &Racer, Option<&Name>)>,
    mut back: MessageWriter<BackToCheckpoint>,
    mut gates_along: Local<Vec<f32>>,
) {
    let course = ::circuit::course_for(&track, settings.difficulty.other_course());
    if let Some(course) = course
        && (track.is_changed() || settings.is_changed() || gates_along.len() != track.gates.len())
    {
        *gates_along = ::circuit::along_the_course(&track.gates, course);
    }
    for (truck, mut driver, racer, name) in &mut drivers {
        let missed = course.is_some_and(|course| {
            driver
                .circuit
                .missed_a_gate(course, &gates_along, &racer.progress)
        });
        let stuck = driver.is_stuck();
        if missed || stuck {
            info!(
                "{}: back to its last checkpoint from {:.0} m round the course, having {}",
                name.map_or("A computer truck", |name| name.as_str()),
                driver.circuit.along().unwrap_or(0.0),
                if stuck {
                    "got stuck"
                } else {
                    "missed the next one"
                },
            );
            super::put_back(&mut back, truck, &mut driver);
            driver.circuit.put_back(&racer.progress);
        }
    }
}
