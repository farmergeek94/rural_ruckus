//! The race slice in a headless app, as a Summit Rumble: on a track with rumble zones a
//! race has a session and no gates, every racer scores by where it stands, and the time
//! runs out for all of them at once.
//!
//! How the trucks drive, and how they push each other, is checked by driving (AGENTS.md).

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use monster_truck_rural_ruckus::game_state::GameState;
use monster_truck_rural_ruckus::physics::GamePhysicsPlugin;
use monster_truck_rural_ruckus::race::{
    RaceClock, RacePlugin, RaceSettings, RaceStart, Racer, RumbleScore, RumbleSession, ScoreLabel,
};
use monster_truck_rural_ruckus::track::{
    RaceType, RumbleZones, Track, TrackPlugin, Zone, builtin_track,
};
use monster_truck_rural_ruckus::truck::{PlaceTruck, PlayerTruck, TruckPlugin};

const STEP: f64 = 1.0 / 120.0;

/// The built-in track with a rumble's zones laid on its ground: a scoring zone 20 m
/// square at the origin, inside a summit 40 m square, both from the ground up to 10 m,
/// which the flat ground there is inside.
fn rumble_track() -> monster_truck_rural_ruckus::track::TrackData {
    let mut track = builtin_track();
    let ground = track.heights.height_at(0.0, 0.0);
    let zone = |half: f32| Zone {
        center: Vec2::ZERO,
        yaw: 0.0,
        half_extents: Vec2::splat(half),
        bottom: ground - 1.0,
        top: ground + 10.0,
    };
    track.race_type = RaceType::Rumble;
    track.rumble = Some(RumbleZones {
        scoring: zone(10.0),
        summit: zone(20.0),
    });
    track.gates.clear();
    track
}

fn headless_app() -> App {
    let mut app = App::new();
    app.insert_resource(Track(rumble_track()))
        .insert_resource(RaceSettings {
            rumble_minutes: 1,
            ..default()
        })
        .add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            GamePhysicsPlugin,
            TrackPlugin,
            TruckPlugin,
            RacePlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(Time::<Fixed>::from_seconds(STEP))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            STEP,
        )));
    // Avian makes some of its resources in `Plugin::finish`, which `App::update` never
    // calls (see `physics`).
    app.finish();
    app.cleanup();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Racing);
    app.update();
    app
}

fn run(app: &mut App, seconds: f64) {
    for _ in 0..(seconds / STEP) as usize {
        app.update();
    }
}

fn player(app: &mut App) -> (Entity, RumbleScore, Option<u64>) {
    let world = app.world_mut();
    let mut players = world.query_filtered::<(Entity, &RumbleScore, &Racer), With<PlayerTruck>>();
    let (entity, score, racer) = players.single(world).expect("the player's truck, scored");
    (entity, score.clone(), racer.progress.finished)
}

/// Waits for GO, then puts the player's truck down at a point on the ground plane.
fn put_down_after_go(app: &mut App, at: Vec2) -> Entity {
    let (truck, _, _) = player(app);
    let go = app.world().resource::<RaceStart>().go;
    while app.world().resource::<RaceClock>().tick <= go {
        app.update();
    }
    let ground = app
        .world()
        .resource::<Track>()
        .heights
        .height_at(at.x, at.y);
    app.world_mut().write_message(PlaceTruck {
        truck,
        ground: Vec3::new(at.x, ground, at.y),
        yaw: 0.0,
    });
    app.update();
    truck
}

#[test]
fn a_rumble_has_a_session_of_the_chosen_minutes_and_no_gates() {
    let mut app = headless_app();
    let session = app.world().resource::<RumbleSession>();
    assert_eq!(session.duration, 60 * 120);
    assert!(!session.over);
    assert!(app.world().resource::<Track>().gates.is_empty());
    let (_, score, finished) = player(&mut app);
    assert_eq!(score, RumbleScore::default());
    assert!(finished.is_none());
}

#[test]
fn every_racer_has_its_score_over_it() {
    let mut app = headless_app();
    put_down_after_go(&mut app, Vec2::ZERO);
    run(&mut app, 2.0);
    let world = app.world_mut();
    let racers: Vec<Entity> = world
        .query_filtered::<Entity, With<Racer>>()
        .iter(world)
        .collect();
    let mut labels: Vec<(Entity, String)> = world
        .query::<(&ScoreLabel, &Text)>()
        .iter(world)
        .map(|(label, text)| (label.truck, text.0.clone()))
        .collect();
    labels.sort();
    let mut over: Vec<Entity> = labels.iter().map(|(truck, _)| *truck).collect();
    over.dedup();
    let mut expected = racers.clone();
    expected.sort();
    assert_eq!(over, expected, "one over each truck");
    let (player, score, _) = player(&mut app);
    let shown = &labels.iter().find(|(truck, _)| *truck == player).unwrap().1;
    assert_eq!(*shown, (score.0.points.floor() as i64).to_string());
    assert_ne!(shown, "0", "{score:?}");
}

#[test]
fn a_truck_in_the_scoring_zone_earns_and_one_off_the_summit_loses() {
    let mut app = headless_app();
    put_down_after_go(&mut app, Vec2::ZERO);
    run(&mut app, 2.0);
    let (_, score, _) = player(&mut app);
    assert!(score.0.on_summit);
    assert!((15.0..=22.0).contains(&score.0.points), "{score:?}");
    assert_eq!(score.0.falls, 0);

    // Carried off the summit: it pays for the fall, and then a point a second.
    put_down_after_go(&mut app, Vec2::new(60.0, 0.0));
    run(&mut app, 2.0);
    let (_, after, _) = player(&mut app);
    assert!(!after.0.on_summit);
    assert_eq!(after.0.falls, 1);
    assert!(
        after.0.points < score.0.points - 50.0,
        "{after:?} from {score:?}"
    );
    assert!(
        after.0.points > score.0.points - 54.0,
        "{after:?} from {score:?}"
    );
}

#[test]
fn the_time_runs_out_for_every_racer_at_once() {
    let mut app = headless_app();
    put_down_after_go(&mut app, Vec2::ZERO);
    run(&mut app, 61.0);
    assert!(app.world().resource::<RumbleSession>().over);
    let (_, score, finished) = player(&mut app);
    assert_eq!(finished, Some(60 * 120));
    let points = score.0.points;
    // Frozen from then on.
    run(&mut app, 1.0);
    assert_eq!(player(&mut app).1.0.points, points);
}
