//! Bump probe: drives a truck, headless, over one of Mud Pies' small road obstacles (a log,
//! a rock, a crushed car) set on flat ground, at several speeds, and prints what happened.
//! `cargo run --example bump_probe -- [truck.pod] [model name] [speed]`; with a speed it
//! traces one run.

use std::path::PathBuf;
use std::time::Duration;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use content::track::{SceneryObject, TrackData};
use monster_truck_rural_ruckus::physics::GamePhysicsPlugin;
use monster_truck_rural_ruckus::scenery::SceneryPlugin;
use monster_truck_rural_ruckus::track::{HeightGrid, Track, TrackPlugin, builtin_track};
use monster_truck_rural_ruckus::truck::{
    Autopilot, ChosenTruck, PlaceTruck, PlayerTruck, TruckConfig, TruckData, TruckInput,
    TruckPlugin,
};

const STEP: f64 = 1.0 / 120.0;
const G: f32 = 9.81;
const SECONDS: f32 = 5.0;

fn mud_pies() -> TrackData {
    let home = PathBuf::from(std::env::var("HOME").unwrap());
    let base = pod::BaseGame::open(&[home.join("Documents/mtm2")]);
    let archive = home.join("Documents/mtm2/Shared/GAME.POD");
    pod::track::load_base(&base, &archive, "WORLD\\CIRC5.SIT").unwrap()
}

fn app(mud: &TrackData, model: &str, yaw: f32, truck: Option<TruckData>) -> App {
    let mut track = builtin_track();
    let (res, size) = (track.heights.resolution(), track.heights.size());
    track.heights = HeightGrid::from_fn(res, size, |_, _| 0.0);
    track.ground_boxes.clear();
    track.scenery = mud.scenery.clone();
    let index = mud
        .scenery
        .models
        .iter()
        .position(|m| m.name.eq_ignore_ascii_case(model))
        .expect("model");
    // The object as the track places one of them: same height above ground and motion.
    // The highest-set one of them: some of Mud Pies' logs are buried.
    let sample = mud
        .scenery
        .objects
        .iter()
        .filter(|o| o.model == index)
        .max_by(|a, b| a.height_above_ground.total_cmp(&b.height_above_ground))
        .expect("object")
        .clone();
    track.scenery.objects = vec![SceneryObject {
        position: Vec2::ZERO,
        yaw,
        ..sample
    }];
    let mut app = App::new();
    app.insert_resource(Track(track));
    if let Some(truck) = truck {
        app.insert_resource(ChosenTruck(truck));
    }
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        GamePhysicsPlugin,
        TrackPlugin,
        SceneryPlugin,
        TruckPlugin,
    ))
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .init_resource::<ButtonInput<KeyCode>>()
    .insert_resource(Time::<Fixed>::from_seconds(STEP))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        STEP,
    )));
    app.finish();
    app.cleanup();
    app.update();
    app
}

struct Outcome {
    jolt: f32,
    min_speed: f32,
    end_speed: f32,
    peak_rise: f32,
    tipped: bool,
    past: bool,
}

fn run(
    mud: &TrackData,
    model: &str,
    yaw: f32,
    truck: Option<TruckData>,
    speed: f32,
    trace: bool,
) -> Outcome {
    let mut app = app(mud, model, yaw, truck);
    let (player, standing, radius) = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(Entity, &TruckConfig), With<PlayerTruck>>();
        let (e, c) = q.single(world).unwrap();
        (e, c.standing_height(), c.wheel_radius)
    };
    let heading = Vec3::NEG_Z;
    // The front-right tire runs over the object's centre.
    let tire_x = app.world().get::<TruckConfig>(player).unwrap().wheel_rest[1].x;
    // BUMP_CENTRE: the truck's middle runs over the object's centre instead.
    let start = if std::env::var_os("BUMP_CENTRE").is_some() {
        Vec3::new(0.0, 0.0, 12.0)
    } else {
        Vec3::new(-tire_x, 0.0, 12.0)
    };
    app.world_mut().write_message(PlaceTruck {
        truck: player,
        ground: start,
        yaw: 0.0,
    });
    app.update();
    app.world_mut().entity_mut(player).insert(Autopilot);
    app.world_mut().get_mut::<LinearVelocity>(player).unwrap().0 = heading * speed;
    let mut out = Outcome {
        jolt: 0.0,
        min_speed: f32::MAX,
        end_speed: 0.0,
        peak_rise: 0.0,
        tipped: false,
        past: false,
    };
    let mut last_v: Option<Vec3> = None;
    let steps = (SECONDS / STEP as f32) as usize;
    let mut stepping = Duration::ZERO;
    if trace {
        println!(
            "wheel radius {radius:.2} standing {standing:.2} object above ground {:.2}",
            app.world().resource::<Track>().0.scenery.objects[0].height_above_ground
        );
    }
    for step in 0..steps {
        {
            let mut input = app.world_mut().get_mut::<TruckInput>(player).unwrap();
            input.throttle = 1.0;
            input.steer = 0.0;
        }
        let began = std::time::Instant::now();
        app.update();
        stepping += began.elapsed();
        let t = *app.world().get::<Transform>(player).unwrap();
        let v = app.world().get::<LinearVelocity>(player).unwrap().0;
        let dv = last_v.map_or(0.0, |lv| (v - lv).length());
        if step > 2 {
            out.jolt = out.jolt.max(dv / STEP as f32 / G);
        }
        if trace
            && (t.translation.z < 8.0 && t.translation.z > -60.0)
            && (step % 3 == 0 || dv > 0.5)
        {
            let (_, pitch, roll) = t.rotation.to_euler(EulerRot::YXZ);
            let gear = app
                .world()
                .get::<monster_truck_rural_ruckus::truck::Gearbox>(player)
                .unwrap()
                .gear();
            let engine = app
                .world()
                .get::<monster_truck_rural_ruckus::truck::Engine>(player)
                .unwrap();
            println!(
                "t={:5.2} z={:7.3} y={:6.3} pitch={:6.1} roll={:6.1} v={:6.2} vy={:6.2} dv={:5.2} ({:5.1} g) gear {} rpm {:.0} tq {:.2} cl {:.2} fuel {:.2} load {:.2} run {}",
                step as f32 * STEP as f32,
                t.translation.z,
                t.translation.y - standing,
                pitch.to_degrees(),
                roll.to_degrees(),
                -v.z,
                v.y,
                dv,
                dv / STEP as f32 / G,
                gear,
                engine.rpm(),
                engine.torque(),
                engine.clutch(),
                engine.fueling(),
                engine.load(),
                engine.running()
            );
            if dv > 0.5 || std::env::var_os("BUMP_CONTACTS").is_some() {
                print_contacts(&mut app, player);
            }
        }
        last_v = Some(v);
        if step > 2 && t.translation.z > -8.0 {
            out.min_speed = out.min_speed.min(-v.z);
        }
        out.peak_rise = out.peak_rise.max(t.translation.y - standing);
        if t.up().y < 0.3 {
            out.tipped = true;
        }
        if t.translation.z < -8.0 {
            out.past = true;
        }
        out.end_speed = -v.z;
    }
    if trace {
        println!("mean step {} us", stepping.as_micros() / steps as u128);
    }
    out
}

fn print_contacts(app: &mut App, truck: Entity) {
    let world = app.world_mut();
    let mut parts = world.query::<(Entity, &ColliderOf, Option<&Name>, &Transform)>();
    let mine: Vec<(Entity, String, f32)> = parts
        .iter(world)
        .filter(|(_, of, _, _)| of.body == truck)
        .map(|(e, _, name, t)| {
            (
                e,
                name.map_or("?".into(), |n| n.to_string()),
                t.translation.y,
            )
        })
        .collect();
    let name_of = |world: &World, e: Entity| -> String {
        world
            .get::<Name>(e)
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("{e:?}"))
    };
    for (e, name, y) in &mine {
        if name.starts_with("Wheel collider") {
            println!("    {name} {e:?} hub y in truck {y:.3}");
        }
    }
    let graph = world.resource::<ContactGraph>();
    for pair in graph.iter_active_touching() {
        let ours = mine
            .iter()
            .any(|(e, ..)| *e == pair.collider1 || *e == pair.collider2)
            || pair.collider1 == truck
            || pair.collider2 == truck;
        if !ours {
            continue;
        }
        for m in &pair.manifolds {
            if m.points.is_empty() {
                println!(
                    "    DROPPED {:<16} x {:<16} n=({:5.2},{:5.2},{:5.2})",
                    name_of(world, pair.collider1),
                    name_of(world, pair.collider2),
                    m.normal.x,
                    m.normal.y,
                    m.normal.z
                );
                continue;
            }
            let impulse: f32 = m.points.iter().map(|p| p.normal_impulse).sum();
            let depth = m
                .points
                .iter()
                .map(|p| p.penetration)
                .fold(f32::MIN, f32::max);
            println!(
                "    KEPT    {:<16} x {:<16} n=({:5.2},{:5.2},{:5.2}) pts={} depth={:.3} impulse={:.0} tangent_v=({:.2},{:.2},{:.2}) fric={:.2}",
                name_of(world, pair.collider1),
                name_of(world, pair.collider2),
                m.normal.x,
                m.normal.y,
                m.normal.z,
                m.points.len(),
                depth,
                impulse,
                m.tangent_velocity.x,
                m.tangent_velocity.y,
                m.tangent_velocity.z,
                m.friction
            );
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mud = mud_pies();
    let home = PathBuf::from(std::env::var("HOME").unwrap());
    let base = pod::BaseGame::open(&[home.join("Documents/mtm2")]);
    let truck = args
        .get(1)
        .filter(|a| a.as_str() != "builtin")
        .map(|p| pod::truck::load_pod(std::path::Path::new(p), &base).unwrap());
    let models: Vec<&str> = match args.get(2) {
        Some(m) => vec![m.as_str()],
        None => vec!["LOG.BIN", "ROCK.BIN", "CRUCARR.BIN", "CRURAMP.BIN"],
    };
    if let Some(speed) = args.get(3) {
        let speed: f32 = speed.parse().unwrap();
        let yaw: f32 = args.get(4).map_or(90.0, |y| y.parse().unwrap());
        let out = run(&mud, models[0], yaw.to_radians(), truck, speed, true);
        println!(
            "jolt {:.1} g  min speed {:.2}  end {:.2}  rise {:.2}  tipped {}  past {}",
            out.jolt, out.min_speed, out.end_speed, out.peak_rise, out.tipped, out.past
        );
        return;
    }
    println!(
        "{:<12} {:>5} {:>7} {:>9} {:>7} {:>6} {:>6} past",
        "model", "m/s", "jolt g", "min m/s", "end", "rise", "tipped"
    );
    for model in models {
        for yaw in [90.0f32, 0.0] {
            for speed in [5.0f32, 10.0, 15.0, 20.0, 25.0] {
                let out = run(&mud, model, yaw.to_radians(), truck.clone(), speed, false);
                println!(
                    "{:<12} {:>5.0} {:>7.1} {:>9.2} {:>7.2} {:>6.2} {:>6} {}  (yaw {})",
                    model,
                    speed,
                    out.jolt,
                    out.min_speed,
                    out.end_speed,
                    out.peak_rise,
                    out.tipped,
                    out.past,
                    yaw
                );
            }
        }
    }
}
