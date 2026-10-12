//! Wall probe: drives the built-in truck, headless, at ground-box walls of several heights,
//! met at several angles and speeds, and into a parked truck, and prints what happened.
//! Run with `cargo run --example wall_probe`; `-- trace <wall m> <degrees> <m/s> <throttle>`
//! prints one run step by step with the contacts the solver saw, `-- parked <degrees>
//! <m/s> <throttle> [distance m]` the same into a parked truck (a short distance and no
//! speed is a shove: 5.5 m from behind, 5.2 m from the side), and `-- box` a bare cube
//! into a wall.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use monster_truck_rural_ruckus::physics::GamePhysicsPlugin;
use monster_truck_rural_ruckus::track::{GroundBox, HeightGrid, Track, TrackPlugin, builtin_track};
use monster_truck_rural_ruckus::truck::{
    Autopilot, ChosenTruck, ComputerTrucks, PlaceTruck, PlayerTruck, Truck, TruckConfig, TruckData,
    TruckInput, TruckPlugin,
};

const STEP: f64 = 1.0 / 120.0;
const G: f32 = 9.81;
/// The wall runs along X at z in [-THICK, 0]; the truck comes from +Z.
const THICK: f32 = 2.0;
const SECONDS: f32 = 8.0;

#[derive(Clone, Copy)]
struct Scenario {
    name: &'static str,
    wall: Option<f32>,
    parked: Option<f32>, // yaw of a parked truck at the origin
    angle_deg: f32,
    speed: f32,
    distance: f32,
    throttle: f32,
}

#[derive(Default)]
struct Outcome {
    over_at: Option<f32>,
    tipped: bool,
    peak_rise: f32,
    jolt: f32,
    rebound: f32,
    end_speed: f32,
    end_z: f32,
    other_moved: f32,
    other_rise: f32,
    other_tipped: bool,
}

fn app(wall: Option<f32>, parked: bool, truck: Option<TruckData>) -> App {
    let mut track = builtin_track();
    let (res, size) = (track.heights.resolution(), track.heights.size());
    track.heights = HeightGrid::from_fn(res, size, |_, _| 0.0);
    track.ground_boxes.clear();
    if let Some(top) = wall {
        track.ground_boxes.push(GroundBox {
            min: Vec2::new(-80.0, -THICK),
            max: Vec2::new(80.0, 0.0),
            bottom: -1.0,
            top,
            faces: None,
        });
    }
    let mut app = App::new();
    app.insert_resource(Track(track));
    if let Some(truck) = truck {
        app.insert_resource(ChosenTruck(truck));
    }
    if parked {
        app.insert_resource(ComputerTrucks(vec![TruckData::builtin()]));
    }
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        GamePhysicsPlugin,
        TrackPlugin,
        TruckPlugin,
    ))
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .init_resource::<ButtonInput<KeyCode>>()
    .insert_resource(Time::<Fixed>::from_seconds(STEP))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        STEP,
    )));
    // Solver settings under test, from the environment.
    {
        let mut solver = app
            .world_mut()
            .resource_mut::<avian3d::dynamics::solver::SolverConfig>();
        if let Ok(v) = std::env::var("WALL_DAMPING") {
            solver.contact_damping_ratio = v.parse().unwrap();
        }
        if let Ok(v) = std::env::var("WALL_FREQ") {
            solver.contact_frequency_factor = v.parse().unwrap();
        }
    }
    app.finish();
    app.cleanup();
    app.update();
    app
}

fn run(s: Scenario) -> Outcome {
    run_traced(s, false)
}

fn run_traced(s: Scenario, trace: bool) -> Outcome {
    let mut app = app(s.wall, s.parked.is_some(), None);
    let (player, standing) = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(Entity, &TruckConfig), With<PlayerTruck>>();
        let (e, c) = q.single(world).unwrap();
        (e, c.standing_height())
    };
    let other = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, (With<Truck>, Without<PlayerTruck>)>();
        q.iter(world).next()
    };
    let yaw = s.angle_deg.to_radians();
    let heading = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let start = -heading * s.distance;
    app.world_mut().write_message(PlaceTruck {
        truck: player,
        ground: start,
        yaw,
    });
    if let (Some(other), Some(parked_yaw)) = (other, s.parked) {
        app.world_mut().write_message(PlaceTruck {
            truck: other,
            ground: Vec3::ZERO,
            yaw: parked_yaw,
        });
    }
    app.update();
    app.world_mut().entity_mut(player).insert(Autopilot);
    app.world_mut().get_mut::<LinearVelocity>(player).unwrap().0 = heading * s.speed;
    let other_start = other.map(|o| app.world().get::<Transform>(o).unwrap().translation);

    let mut out = Outcome {
        rebound: f32::MAX,
        ..Default::default()
    };
    let mut last_v: Option<Vec3> = None;
    let steps = (SECONDS / STEP as f32) as usize;
    for step in 0..steps {
        {
            let mut input = app.world_mut().get_mut::<TruckInput>(player).unwrap();
            input.throttle = s.throttle;
            input.steer = 0.0;
        }
        app.update();
        let t = *app.world().get::<Transform>(player).unwrap();
        let v = app.world().get::<LinearVelocity>(player).unwrap().0;
        if let Some(lv) = last_v {
            // Skip the first two steps: the set velocity settles.
            if step > 2 {
                out.jolt = out.jolt.max((v - lv).length() / STEP as f32 / G);
            }
        }
        if trace && (step % 6 == 0 || last_v.is_some_and(|lv| (v - lv).length() > 1.0)) {
            let (_, pitch, _) = t.rotation.to_euler(EulerRot::YXZ);
            // Positions along the approach: how far the trucks are past the origin the
            // way the player came.
            let other_along = other.map_or(f32::NAN, |o| {
                app.world().get::<Transform>(o).unwrap().translation.dot(heading)
            });
            println!(
                "t={:5.2} along={:7.3} y={:6.3} pitch={:6.1} v={:7.2} vy={:6.2} dv={:6.2} other_along={:7.3}",
                step as f32 * STEP as f32,
                t.translation.dot(heading),
                t.translation.y - standing,
                pitch.to_degrees(),
                v.dot(heading),
                v.y,
                last_v.map_or(0.0, |lv| (v - lv).length()),
                other_along
            );
            if step % 24 == 0 || last_v.is_some_and(|lv| (v - lv).length() > 1.0) {
                print_contacts(&mut app, player);
            }
        }
        last_v = Some(v);
        out.peak_rise = out.peak_rise.max(t.translation.y - standing);
        out.rebound = out.rebound.min(v.dot(heading));
        if t.up().y < 0.3 {
            out.tipped = true;
        }
        // Over the wall: the truck's centre past its far face, along the approach.
        let along = -t.translation.z; // how far past z = 0 towards -Z
        if s.wall.is_some() && out.over_at.is_none() && along > THICK + 0.5 {
            out.over_at = Some(step as f32 * STEP as f32);
        }
        out.end_speed = v.dot(heading);
        out.end_z = t.translation.z;
        if let (Some(o), Some(os)) = (other, other_start) {
            let ot = *app.world().get::<Transform>(o).unwrap();
            out.other_moved = out.other_moved.max((ot.translation - os).xz().length());
            out.other_rise = out.other_rise.max(ot.translation.y - os.y);
            if ot.up().y < 0.3 {
                out.other_tipped = true;
            }
        }
    }
    out
}

/// Every touching contact pair on this truck, with what the solver was told of it.
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
    let mut lines = Vec::new();
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
            let depth = m
                .points
                .iter()
                .map(|p| p.penetration)
                .fold(f32::MIN, f32::max);
            let mean = m.points.iter().map(|p| p.point).sum::<Vec3>() / m.points.len() as f32;
            lines.push((
                pair.collider1,
                pair.collider2,
                m.normal,
                m.points.len(),
                depth,
                mean,
                m.total_normal_impulse(),
                m.tangent_velocity,
                m.friction,
            ));
        }
    }
    for (c1, c2, n, count, depth, mean, impulse, tv, fr) in lines {
        println!(
            "    {:<16} x {:<16} n=({:5.2},{:5.2},{:5.2}) pts={} depth={:6.3} at y={:5.2} J={:8.1} tv=({:5.2},{:5.2},{:5.2}) mu={:.2}",
            name_of(world, c1),
            name_of(world, c2),
            n.x,
            n.y,
            n.z,
            count,
            depth,
            mean.y,
            impulse,
            tv.x,
            tv.y,
            tv.z,
            fr
        );
    }
}

/// A truck of the base game in ~/Documents/mtm2, by part of its name, case aside.
fn base_truck(name: &str) -> TruckData {
    let home = std::env::var("HOME").unwrap();
    let folders: Vec<std::path::PathBuf> = ["Shared", "English"]
        .iter()
        .map(|f| format!("{home}/Documents/mtm2/{f}").into())
        .collect();
    let base = pod::BaseGame::open(&folders);
    let wanted = name.to_lowercase();
    let found = pod::truck::peek_base(&base)
        .into_iter()
        .find(|t| {
            t.name
                .as_ref()
                .is_ok_and(|n| n.to_lowercase().contains(&wanted))
        })
        .unwrap_or_else(|| panic!("no base truck named like {name}"));
    println!("truck: {}", found.name.as_ref().unwrap());
    pod::truck::load_base(&base, &found.archive, &found.file).unwrap()
}

/// The truck dropped on its side, rolled `roll` degrees, on flat ground: how its suspension
/// settles. `throttle` is held throughout.
fn side_test(roll: f32, throttle: f32, truck: Option<TruckData>) {
    let mut app = app(None, false, truck);
    let player = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<PlayerTruck>>();
        q.single(world).unwrap()
    };
    app.world_mut().write_message(PlaceTruck {
        truck: player,
        ground: Vec3::ZERO,
        yaw: 0.0,
    });
    app.update();
    app.world_mut().entity_mut(player).insert(Autopilot);
    {
        let world = app.world_mut();
        let to = Transform::from_xyz(0.0, 2.5, 0.0)
            .with_rotation(Quat::from_rotation_z(roll.to_radians()));
        *world.get_mut::<Transform>(player).unwrap() = to;
        world.get_mut::<Position>(player).unwrap().0 = to.translation;
        world.get_mut::<Rotation>(player).unwrap().0 = to.rotation;
        world.get_mut::<LinearVelocity>(player).unwrap().0 = Vec3::ZERO;
        world.get_mut::<AngularVelocity>(player).unwrap().0 = Vec3::ZERO;
    }
    for step in 0..(6.0 / STEP) as usize {
        app.world_mut()
            .get_mut::<TruckInput>(player)
            .unwrap()
            .throttle = throttle;
        app.update();
        if step % 12 == 0 {
            let world = app.world_mut();
            let t = *world.get::<Transform>(player).unwrap();
            let v = world.get::<LinearVelocity>(player).unwrap().0;
            let (_, _, roll) = t.rotation.to_euler(EulerRot::YXZ);
            let mut parts = world.query::<(&ColliderOf, &Name, &Transform)>();
            let hubs: Vec<String> = parts
                .iter(world)
                .filter(|(of, name, _)| of.body == player && name.as_str() == "Wheel collider")
                .map(|(_, _, c)| {
                    let world_y = (t.rotation * c.translation + t.translation).y;
                    format!("{:6.3}@{:4.2}", c.translation.y, world_y)
                })
                .collect();
            println!(
                "t={:5.2} y={:6.3} roll={:6.1} v=({:5.2},{:5.2},{:5.2}) hubs {}",
                step as f32 * STEP as f32,
                t.translation.y,
                roll.to_degrees(),
                v.x,
                v.y,
                v.z,
                hubs.join(" ")
            );
            if step + 12 >= (6.0 / STEP) as usize {
                print_contacts(&mut app, player);
            }
        }
    }
}

/// The built-in truck at full throttle and full lock on flat ground: whether a lifted
/// inside wheel bobs. Prints the hub lengths every step over the last two seconds.
fn turn_test() {
    let mut app = app(None, false, None);
    let player = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<PlayerTruck>>();
        q.single(world).unwrap()
    };
    app.world_mut().write_message(PlaceTruck {
        truck: player,
        ground: Vec3::ZERO,
        yaw: 0.0,
    });
    app.update();
    app.world_mut().entity_mut(player).insert(Autopilot);
    let steps = (10.0 / STEP) as usize;
    let mut last: Vec<f32> = Vec::new();
    let mut reversals = 0;
    let mut changes: Vec<f32> = Vec::new();
    for step in 0..steps {
        {
            let mut input = app.world_mut().get_mut::<TruckInput>(player).unwrap();
            input.throttle = 1.0;
            input.steer = if step > 2 * 120 { 1.0 } else { 0.0 };
        }
        app.update();
        let world = app.world_mut();
        let t = *world.get::<Transform>(player).unwrap();
        let v = world.get::<LinearVelocity>(player).unwrap().0;
        let mut parts = world.query::<(&ColliderOf, &Name, &Transform)>();
        let hubs: Vec<f32> = parts
            .iter(world)
            .filter(|(of, name, _)| of.body == player && name.as_str() == "Wheel collider")
            .map(|(_, _, c)| c.translation.y)
            .collect();
        if step >= 6 * 120 {
            if !last.is_empty() {
                let d = hubs[0] - last[0];
                if changes
                    .last()
                    .is_some_and(|&prev| d * prev < 0.0 && d.abs() > 0.005)
                {
                    reversals += 1;
                }
                changes.push(d);
            }
            if step % 12 == 0 {
                let (_, _, roll) = t.rotation.to_euler(EulerRot::YXZ);
                println!(
                    "t={:5.2} |v|={:5.2} roll={:5.1} hubs {:6.3} {:6.3} {:6.3} {:6.3}",
                    step as f32 * STEP as f32,
                    v.length(),
                    roll.to_degrees(),
                    hubs[0],
                    hubs[1],
                    hubs[2],
                    hubs[3]
                );
            }
        }
        last = hubs;
    }
    println!("inside front hub reversals of more than 5 mm over the last 4 s: {reversals}");
}

/// A bare cube driven into a static wall at 20 m/s, with no bounce and no friction: what
/// the physics engine alone does.
fn box_test() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, GamePhysicsPlugin))
        .insert_resource(Time::<Fixed>::from_seconds(STEP))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            STEP,
        )));
    app.finish();
    app.cleanup();
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 10.0, 2.0),
        Transform::from_xyz(0.0, 5.0, -1.0),
        Friction::new(1.0),
        Restitution::new(0.0).with_combine_rule(CoefficientCombine::Min),
    ));
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 1.0, 40.0),
        Transform::from_xyz(0.0, -0.5, 10.0),
        Friction::new(0.0),
        Restitution::new(0.0).with_combine_rule(CoefficientCombine::Min),
    ));
    let cube = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::cuboid(2.0, 1.0, 2.0),
            Mass(5000.0),
            Transform::from_xyz(0.0, 0.5, 3.0),
            LinearVelocity(Vec3::new(0.0, 0.0, -20.0)),
            Friction::new(0.0).with_combine_rule(CoefficientCombine::Min),
            Restitution::new(0.0).with_combine_rule(CoefficientCombine::Min),
            SpeculativeMargin(0.0),
        ))
        .id();
    for step in 0..30 {
        app.update();
        let t = app.world().get::<Transform>(cube).unwrap().translation;
        let v = app.world().get::<LinearVelocity>(cube).unwrap().0;
        println!("step {step:2} z={:7.3} vz={:7.3} vy={:6.3}", t.z, v.z, v.y);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "side") {
        let truck = args.get(4).map(|name| base_truck(name));
        side_test(
            args.get(2).map_or(90.0, |a| a.parse().unwrap()),
            args.get(3).map_or(0.0, |a| a.parse().unwrap()),
            truck,
        );
        return;
    }
    if args.get(1).is_some_and(|a| a == "turn") {
        turn_test();
        return;
    }
    if args.get(1).is_some_and(|a| a == "box") {
        box_test();
        return;
    }
    if args.get(1).is_some_and(|a| a == "parked") {
        let f = |i: usize| args[i].parse::<f32>().unwrap();
        run_traced(
            Scenario {
                name: "parked",
                wall: None,
                parked: Some(0.0),
                angle_deg: f(2),
                speed: f(3),
                distance: args.get(5).map_or(20.0, |_| f(5)),
                throttle: f(4),
            },
            true,
        );
        return;
    }
    if args.get(1).is_some_and(|a| a == "trace") {
        let f = |i: usize| args[i].parse::<f32>().unwrap();
        run_traced(
            Scenario {
                name: "trace",
                wall: Some(f(2)),
                parked: None,
                angle_deg: f(3),
                speed: f(4),
                distance: if f(4) <= 1.0 { 6.0 } else { 20.0 },
                throttle: f(5),
            },
            true,
        );
        return;
    }
    let mut scenarios = Vec::new();
    for &wall in &[0.5f32, 1.0, 1.25, 1.5, 2.0, 3.0] {
        for &angle in &[0.0f32, 30.0, 50.0] {
            for &(speed, distance) in &[(1.0f32, 6.0f32), (4.0, 12.0), (8.0, 20.0), (15.0, 25.0)] {
                scenarios.push(Scenario {
                    name: "wall",
                    wall: Some(wall),
                    parked: None,
                    angle_deg: angle,
                    speed,
                    distance,
                    throttle: 1.0,
                });
            }
        }
        // Coasting into it: a crash, not a climb.
        for &speed in &[8.0f32, 15.0] {
            scenarios.push(Scenario {
                name: "coast",
                wall: Some(wall),
                parked: None,
                angle_deg: 0.0,
                speed,
                distance: 20.0,
                throttle: 0.0,
            });
        }
    }
    // Into a parked truck: from behind, and into its side.
    for &(angle, parked_yaw, label) in &[(0.0f32, 0.0f32, "rear"), (90.0, 0.0, "side")] {
        for &speed in &[4.0f32, 8.0, 15.0] {
            scenarios.push(Scenario {
                name: label,
                wall: None,
                parked: Some(parked_yaw),
                angle_deg: angle,
                speed,
                distance: 20.0,
                throttle: 1.0,
            });
        }
    }
    // Flat ground, a sanity run.
    scenarios.push(Scenario {
        name: "flat",
        wall: None,
        parked: None,
        angle_deg: 0.0,
        speed: 0.0,
        distance: 0.0,
        throttle: 1.0,
    });

    println!(
        "{:<6} {:>5} {:>4} {:>5} {:>4} | {:>6} {:>5} {:>5} {:>6} {:>7} {:>6} | {:>6} {:>5} {:>4}",
        "run",
        "wall",
        "deg",
        "m/s",
        "thr",
        "over_s",
        "tip",
        "rise",
        "jolt_g",
        "rebound",
        "end_v",
        "o_move",
        "o_rise",
        "o_tip"
    );
    let mut summary: Vec<(f32, usize, usize, usize, f32)> = Vec::new();
    for s in &scenarios {
        let o = run(*s);
        println!(
            "{:<6} {:>5.2} {:>4.0} {:>5.1} {:>4.1} | {:>6} {:>5} {:>5.2} {:>6.1} {:>7.2} {:>6.2} | {:>6.2} {:>5.2} {:>4}",
            s.name,
            s.wall.unwrap_or(0.0),
            s.angle_deg,
            s.speed,
            s.throttle,
            o.over_at.map_or("-".to_string(), |t| format!("{t:.2}")),
            if o.tipped { "TIP" } else { "" },
            o.peak_rise,
            o.jolt,
            o.rebound,
            o.end_speed,
            o.other_moved,
            o.other_rise,
            if o.other_tipped { "TIP" } else { "" },
        );
        if let (Some(wall), true) = (s.wall, s.throttle > 0.0) {
            match summary.iter_mut().find(|row| row.0 == wall) {
                Some(row) => {
                    row.1 += 1;
                    row.2 += o.over_at.is_some() as usize;
                    row.3 += o.tipped as usize;
                    row.4 = row.4.max(o.jolt);
                }
                None => summary.push((
                    wall,
                    1,
                    o.over_at.is_some() as usize,
                    o.tipped as usize,
                    o.jolt,
                )),
            }
        }
    }
    println!("\nwall   runs  over  tipped  worst_jolt_g");
    for (wall, runs, over, tipped, jolt) in summary {
        println!("{wall:<6.2} {runs:>4} {over:>5} {tipped:>7} {jolt:>12.1}");
    }
}
