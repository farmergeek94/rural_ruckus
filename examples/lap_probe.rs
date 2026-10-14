//! Lap probe: drives a truck round a base-game or POD track, headless, on the diagnostics
//! autopilot, and prints every step that jolts the truck harder than a threshold, with
//! where it was and what its wheels touched. `cargo run --example lap_probe -- <truck.pod>
//! [seconds] [jolt g]`.

use std::path::PathBuf;
use std::time::Duration;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use monster_truck_rural_ruckus::diagnostics::DiagnosticsPlugin;
use monster_truck_rural_ruckus::footing::FootingPlugin;
use monster_truck_rural_ruckus::opponents::OpponentsPlugin;
use monster_truck_rural_ruckus::physics::GamePhysicsPlugin;
use monster_truck_rural_ruckus::race::RacePlugin;
use monster_truck_rural_ruckus::scenery::SceneryPlugin;
use monster_truck_rural_ruckus::track::{Track, TrackPlugin};
use monster_truck_rural_ruckus::truck::{ChosenTruck, PlayerTruck, TruckConfig, TruckPlugin};

const STEP: f64 = 1.0 / 120.0;
const G: f32 = 9.81;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let home = PathBuf::from(std::env::var("HOME").unwrap());
    let base = pod::BaseGame::open(&[home.join("Documents/mtm2")]);
    let track = match std::env::var("LAP_TRACK") {
        Ok(path) => pod::track::load_pod(std::path::Path::new(&path), &base).unwrap(),
        Err(_) => {
            let archive = home.join("Documents/mtm2/Shared/GAME.POD");
            pod::track::load_base(&base, &archive, "WORLD\\CIRC5.SIT").unwrap()
        }
    };
    let truck = args
        .get(1)
        .filter(|a| a.as_str() != "builtin")
        .map(|p| pod::truck::load_pod(std::path::Path::new(p), &base).unwrap());
    let seconds: f32 = args.get(2).map_or(150.0, |s| s.parse().unwrap());
    let threshold: f32 = args.get(3).map_or(15.0, |s| s.parse().unwrap());
    // A window of steps to trace in full, in seconds.
    let window: Option<(f32, f32)> = args
        .get(4)
        .zip(args.get(5))
        .map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap()));

    let mut app = App::new();
    app.insert_resource(Track(track));
    // `LAP_OPPONENTS` computer trucks, copies of the player's, for a measurement of the step.
    let opponents: usize = std::env::var("LAP_OPPONENTS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    let player = truck
        .clone()
        .unwrap_or_else(monster_truck_rural_ruckus::truck::TruckData::builtin);
    app.insert_resource(monster_truck_rural_ruckus::truck::ComputerTrucks(
        (1..=opponents)
            .map(|number| player.clone().in_color(number))
            .collect(),
    ));
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
        FootingPlugin,
        RacePlugin,
        OpponentsPlugin,
        DiagnosticsPlugin {
            log_fps: false,
            autopilot: true,
        },
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

    let (player, standing) = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(Entity, &TruckConfig), With<PlayerTruck>>();
        let (e, c) = q.single(world).unwrap();
        (e, c.standing_height())
    };
    let course = app.world().resource::<Track>().0.course.clone().unwrap();
    let mut last_v: Option<Vec3> = None;
    let mut worst = 0.0f32;
    let mut events = 0;
    let steps = (seconds / STEP as f32) as usize;
    let mut spent = std::time::Duration::ZERO;
    for step in 0..steps {
        let began = std::time::Instant::now();
        app.update();
        spent += began.elapsed();
        let t = *app.world().get::<Transform>(player).unwrap();
        let v = app.world().get::<LinearVelocity>(player).unwrap().0;
        let dv = last_v.map_or(0.0, |lv| (v - lv).length());
        let jolt = dv / STEP as f32 / G;
        last_v = Some(v);
        if step < 3 {
            continue;
        }
        worst = worst.max(jolt);
        let now = step as f32 * STEP as f32;
        if window.is_some_and(|(a, b)| now >= a && now <= b) {
            let (_, pitch, _) = t.rotation.to_euler(EulerRot::YXZ);
            let ground = app
                .world()
                .resource::<Track>()
                .0
                .heights
                .height_at(t.translation.x, t.translation.z);
            println!(
                "T={:6.3} at=({:8.3},{:7.3},{:8.3}) y-std={:5.2} pitch={:5.1} v=({:5.2},{:5.2},{:5.2}) jolt={:4.0} g",
                now,
                t.translation.x,
                t.translation.y,
                t.translation.z,
                t.translation.y - standing - ground,
                pitch.to_degrees(),
                v.x,
                v.y,
                v.z,
                jolt
            );
            print_wheels(&mut app, player);
            print_all_contacts(&mut app, player);
            continue;
        }
        if jolt > threshold {
            events += 1;
            let near = course.nearest(t.translation.xz());
            let along = course.distance_along(&near);
            let lv = v - last_v.unwrap_or(v);
            let _ = lv;
            println!(
                "t={:6.2} along={:6.0} off={:4.1} at=({:7.1},{:5.1},{:7.1}) y-std={:5.2} speed={:5.1} dv={:5.2} jolt={:5.0} g",
                step as f32 * STEP as f32,
                along,
                near.distance,
                t.translation.x,
                t.translation.y,
                t.translation.z,
                t.translation.y
                    - standing
                    - app
                        .world()
                        .resource::<Track>()
                        .0
                        .heights
                        .height_at(t.translation.x, t.translation.z),
                v.length(),
                dv,
                jolt
            );
            print_contacts(&mut app, player);
        }
    }
    let t = *app.world().get::<Transform>(player).unwrap();
    let near = course.nearest(t.translation.xz());
    println!(
        "done: {events} jolts over {threshold} g, worst {worst:.0} g, ended at along={:.0} m, mean step {:.0} us",
        course.distance_along(&near),
        spent.as_secs_f64() * 1e6 / steps as f64
    );
}

fn print_contacts(app: &mut App, truck: Entity) {
    let world = app.world_mut();
    let mut parts = world.query::<(Entity, &ColliderOf, Option<&Name>)>();
    let mine: Vec<Entity> = parts
        .iter(world)
        .filter(|(_, of, _)| of.body == truck)
        .map(|(e, ..)| e)
        .collect();
    let name_of = |world: &World, e: Entity| -> String {
        let body = world.get::<ColliderOf>(e).map_or(e, |of| of.body);
        world
            .get::<Name>(e)
            .or_else(|| world.get::<Name>(body))
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("{e:?}"))
    };
    let graph = world.resource::<ContactGraph>();
    let mut lines: Vec<String> = Vec::new();
    for pair in graph.iter_active_touching() {
        if !(mine.contains(&pair.collider1)
            || mine.contains(&pair.collider2)
            || pair.collider1 == truck
            || pair.collider2 == truck)
        {
            continue;
        }
        for m in &pair.manifolds {
            let impulse: f32 = m.points.iter().map(|p| p.normal_impulse).sum();
            if m.points.is_empty() || impulse < 500.0 {
                continue;
            }
            lines.push(format!(
                "    {:<16} x {:<16} n=({:5.2},{:5.2},{:5.2}) impulse={:.0} tangent_v=({:.2},{:.2},{:.2})",
                name_of(world, pair.collider1), name_of(world, pair.collider2), m.normal.x, m.normal.y, m.normal.z,
                impulse, m.tangent_velocity.x, m.tangent_velocity.y, m.tangent_velocity.z
            ));
        }
    }
    lines.sort();
    lines.dedup();
    for l in lines.iter().take(8) {
        println!("{l}");
    }
}

fn print_wheels(app: &mut App, truck: Entity) {
    let world = app.world_mut();
    let mut parts = world.query::<(
        Entity,
        &ColliderOf,
        Option<&Name>,
        &Transform,
        &GlobalTransform,
    )>();
    let heights = world.resource::<Track>().0.heights.clone();
    let mut line = String::new();
    for (_, of, name, t, g) in parts.iter(world) {
        if of.body != truck {
            continue;
        }
        if name.is_some_and(|n| n.starts_with("Wheel collider")) {
            let w = g.translation();
            line += &format!(
                "  hub y={:6.3} world=({:7.2},{:6.2},{:7.2}) ground={:5.2} hub-ground={:5.2}",
                t.translation.y,
                w.x,
                w.y,
                w.z,
                heights.height_at(w.x, w.z),
                w.y - heights.height_at(w.x, w.z)
            );
        }
    }
    println!("   {line}");
}

fn print_all_contacts(app: &mut App, truck: Entity) {
    let world = app.world_mut();
    let mut parts = world.query::<(Entity, &ColliderOf, &GlobalTransform)>();
    let mine: Vec<Entity> = parts
        .iter(world)
        .filter(|(_, of, _)| of.body == truck)
        .map(|(e, ..)| e)
        .collect();
    let hubs: std::collections::HashMap<Entity, Vec3> = parts
        .iter(world)
        .filter(|(_, of, _)| of.body == truck)
        .map(|(e, _, g)| (e, g.translation()))
        .collect();
    let radius = world.get::<TruckConfig>(truck).unwrap().wheel_radius;
    let name_of = |world: &World, e: Entity| -> String {
        let body = world.get::<ColliderOf>(e).map_or(e, |of| of.body);
        world
            .get::<Name>(e)
            .or_else(|| world.get::<Name>(body))
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("{e:?}"))
    };
    let graph = world.resource::<ContactGraph>();
    for pair in graph.iter_active_touching() {
        if !(mine.contains(&pair.collider1)
            || mine.contains(&pair.collider2)
            || pair.collider1 == truck
            || pair.collider2 == truck)
        {
            continue;
        }
        let n1 = name_of(world, pair.collider1);
        let n2 = name_of(world, pair.collider2);
        if n1 == "Terrain" || n2 == "Terrain" {
            continue;
        }
        for m in &pair.manifolds {
            let impulse: f32 = m.points.iter().map(|p| p.normal_impulse).sum();
            let depth = m
                .points
                .iter()
                .map(|p| p.penetration)
                .fold(f32::MIN, f32::max);
            let p = m.points.first().map(|p| p.anchor1).unwrap_or_default();
            let wheel = if mine.contains(&pair.collider1) {
                pair.collider1
            } else {
                pair.collider2
            };
            let unders: Vec<String> = hubs
                .get(&wheel)
                .map(|hub| {
                    m.points
                        .iter()
                        .map(|pt| format!("{:.2}", -(pt.point.y - hub.y) / radius))
                        .collect()
                })
                .unwrap_or_default();
            if !m.points.is_empty() {
                println!("      under per point: {}", unders.join(" "));
            }
            println!(
                "    {} {:<16} {:?} x {:<12} n=({:5.2},{:5.2},{:5.2}) pts={} depth={:.3} anchor1=({:5.2},{:5.2},{:5.2}) impulse={:.0} tangent_v=({:.2},{:.2},{:.2})",
                if m.points.is_empty() {
                    "DROPPED"
                } else {
                    "KEPT   "
                },
                n1,
                pair.collider1,
                n2,
                m.normal.x,
                m.normal.y,
                m.normal.z,
                m.points.len(),
                depth,
                p.x,
                p.y,
                p.z,
                impulse,
                m.tangent_velocity.x,
                m.tangent_velocity.y,
                m.tangent_velocity.z
            );
        }
    }
}
