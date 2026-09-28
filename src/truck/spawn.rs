//! Builds the truck: a chassis rigid body, which is what the physics knows about, with a
//! wheel-shaped collider at each hub, and the body, wheels and axles that are drawn, which follow it (see `interpolate`) and are built
//! as any truck on show is (see `display`).
//!
//! The player's truck comes from the `ChosenTruck` resource, and how it is set up from
//! `TruckSetup`. The others come from `ComputerTrucks`, each set up at random for
//! the race (`TruckSetup::random`), except for the grip dial.

use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use super::display::{self, CHASSIS_HALF_EXTENTS, Paint};
use super::drive::TipGuard;
use super::interpolate::PhysicsPose;
use super::lamps;
use super::{
    ChosenTruck, ComputerTrucks, GroundGrip, PlayerTruck, Truck, TruckConfig, TruckData,
    TruckInput, TruckName, TruckSetup, TruckVisual, TruckWheelColliders, TruckWheels,
    WheelCollider, WheelCore,
};
use crate::game_state::GameState;

#[derive(Component)]
pub struct Wheel {
    /// Top of the suspension travel, in chassis space.
    pub(super) mount: Vec3,
    pub(super) front: bool,
    /// Accumulated roll angle, for the visual only.
    pub(super) spin: f32,
    /// How fast the tread goes round, in m/s, forwards positive, for the visual only. The
    /// ground's speed under it while it touches; in the air, what the throttle and the
    /// brakes make of it (see `drive::spin_in_the_air`).
    pub(super) tread_speed: f32,
    /// How far the axle's beam, and so the tire, leans from level in the truck's axes, in
    /// radians about its Z axis: positive with the right-hand hub higher. Set from where
    /// both of the axle's hubs are, and used for the tire's sweep and how it is drawn.
    pub(super) tilt: f32,
}

/// How much of the speed a tire meets something at it comes back off with, from 0 (a dead
/// stop) to 1 (all of it). Higher and trucks ricochet off each other.
const TIRE_BOUNCE: f32 = 0.8;
/// How big a wheel's core is, as a share of the tire's radius: a tire sinks into the
/// ground by the rest of its radius at most before the core stands on the surface. Larger
/// lets it sink less, and meets the ground sooner, where the suspension would have coped.
const CORE_SHARE: f32 = 0.75;
/// The same for the body: less than a tire, being steel over a frame.
const BODY_BOUNCE: f32 = 0.2;

const SPAWN_POSITION: Vec3 = Vec3::new(0.0, 3.0, 0.0);
/// How far apart trucks are spawned, in metres: two bodies in one place would be thrown
/// apart by the physics. A race puts each on its place of the grid straight away.
const SPAWN_SPACING: f32 = 8.0;

/// The player's truck, set up as the garage says, and the computer's trucks as they come.
pub(super) fn spawn_trucks(
    mut commands: Commands,
    chosen: Res<ChosenTruck>,
    setup: Res<TruckSetup>,
    computer_trucks: Res<ComputerTrucks>,
    mut paint: Paint,
) {
    let config = chosen.config.with_setup(&setup);
    let player = spawn_truck(&mut commands, &chosen, config, 0, &mut paint);
    commands
        .entity(player)
        .insert((Name::new("Truck"), PlayerTruck));

    // Only has to differ from one race to the next.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos() as u64);
    for (index, data) in computer_trucks.0.iter().enumerate() {
        let number = index + 1;
        let setup = TruckSetup::random(seed.wrapping_add(number as u64 * 0x1_0000));
        debug!("Computer truck {number}: {setup:?}");
        let config = data.config.with_setup(&setup);
        let truck = spawn_truck(&mut commands, data, config, number, &mut paint);
        // Only the computer's trucks are kept from tipping in a sharp turn (see `TipGuard`).
        commands.entity(truck).insert((
            Name::new(format!("Computer truck {number}")),
            TipGuard::default(),
        ));
    }
}

/// Builds truck `number` of the race, and returns its body.
fn spawn_truck(
    commands: &mut Commands,
    chosen: &TruckData,
    config: TruckConfig,
    number: usize,
    paint: &mut Paint,
) -> Entity {
    // Moment of inertia of a solid box, sized a little larger than the chassis collider
    // to account for the wheels and axles hanging off it.
    let inertia_half_extents = Vec3::new(1.7, 0.9, 2.5);
    let squared = inertia_half_extents * inertia_half_extents;
    let principal_inertia = config.mass / 3.0
        * Vec3::new(
            squared.y + squared.z,
            squared.x + squared.z,
            squared.x + squared.y,
        );

    // The hull of the body's corners, where the truck says what they are.
    let collider = Some(&chosen.collider_points)
        .filter(|points| points.len() >= 4)
        .and_then(|points| Collider::convex_hull(points))
        .unwrap_or_else(|| {
            Collider::cuboid(
                CHASSIS_HALF_EXTENTS.x,
                CHASSIS_HALF_EXTENTS.y,
                CHASSIS_HALF_EXTENTS.z,
            )
        });

    let start =
        Transform::from_translation(SPAWN_POSITION + Vec3::X * SPAWN_SPACING * number as f32);
    let truck = commands
        .spawn((
            Truck,
            TruckName(chosen.name.clone()),
            TruckInput::default(),
            start,
            PhysicsPose::at(start),
            config.clone(),
            DespawnOnExit(GameState::Racing),
        ))
        // A second insert because bundles are tuples, and tuples top out at 15 elements.
        .insert((
            RigidBody::Dynamic,
            collider,
            ColliderMassProperties::MassProperties(MassProperties {
                local_center_of_mass: config.center_of_mass,
                mass: config.mass,
                principal_inertia_local_frame: Quat::IDENTITY,
                principal_inertia,
            }),
            Friction::coefficient(0.4),
            // Bodywork that springs back off whatever it hits, the ground included: a
            // truck that lands on its roof bounces rather than sticking.
            Restitution {
                coefficient: BODY_BOUNCE,
                combine_rule: CoefficientCombineRule::Max,
            },
            Velocity::default(),
            ExternalForce::default(),
            ReadMassProperties::default(),
            GroundGrip::default(),
            Damping {
                linear_damping: 0.02,
                angular_damping: 0.4,
            },
            // The suspension forces come from us, not from contacts, so Rapier can't tell
            // on its own that a resting truck still needs simulating.
            Sleeping::disabled(),
        ))
        .id();

    let visual = commands
        .spawn((
            Name::new("Truck body"),
            TruckVisual { truck },
            start,
            Visibility::default(),
            DespawnOnExit(GameState::Racing),
        ))
        .id();
    // A tire-shaped collider at each wheel, as a child of the body so that Rapier makes it
    // part of it, which `drive` keeps at the hub as the suspension moves. It is what
    // walls, rails, scenery and other trucks meet at wheel height, and what another
    // truck's tire can climb onto. It touches the ground too, but only where the
    // suspension doesn't: a contact under the tread would fight the cast, so `contacts`
    // drops it, and what is left holds up a tire the ground meets anywhere else.
    let wheel_colliders = config
        .wheel_rest
        .iter()
        .map(|&rest| {
            let core = commands
                .spawn((
                    Name::new("Wheel core"),
                    WheelCore,
                    ChildOf(truck),
                    Transform::from_translation(rest),
                    // A ball, which rolls over the edges between the ground's triangles
                    // where the rim of a cylinder could catch on them.
                    Collider::ball(config.wheel_radius * CORE_SHARE),
                    crate::collision_groups::wheel_core(),
                    // So that `contacts` lets it roll rather than scrub.
                    ActiveHooks::MODIFY_SOLVER_CONTACTS,
                    ColliderMassProperties::Mass(0.0),
                    // The ground's own bounce, which is the softer (see `track`).
                    Restitution {
                        coefficient: TIRE_BOUNCE,
                        combine_rule: CoefficientCombineRule::Min,
                    },
                ))
                .id();
            commands
                .spawn((
                    Name::new("Wheel collider"),
                    WheelCollider {
                        bottomed: false,
                        core,
                    },
                    ChildOf(truck),
                    Transform::from_translation(rest)
                        .with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
                    Collider::cylinder(config.wheel_width / 2.0, config.wheel_radius),
                    crate::collision_groups::wheel(),
                    // So that `contacts` is asked which of its contacts to keep.
                    ActiveHooks::MODIFY_SOLVER_CONTACTS,
                    // The body's mass is given whole on the chassis.
                    ColliderMassProperties::Mass(0.0),
                    Friction::coefficient(0.4),
                    // Rubber: a tire that meets a tire, a rail or a body springs back
                    // off it. `Min` so that a surface may be softer than the tire and say
                    // so -- the ground does, because dirt is not rubber (see `track`) --
                    // while anything that does not name a bounce of its own, or names a
                    // livelier one, leaves the tire's. A collider carries one restitution
                    // for everything it touches, and this is the only way to tell the
                    // ground apart from a rail.
                    Restitution {
                        coefficient: TIRE_BOUNCE,
                        combine_rule: CoefficientCombineRule::Min,
                    },
                ))
                .id()
        })
        .collect();
    commands
        .entity(truck)
        .insert(TruckWheelColliders(wheel_colliders));

    let wheels = display::build(commands, visual, chosen, &config, paint);
    commands.entity(truck).insert(TruckWheels(wheels));
    let textures = chosen
        .looks
        .as_ref()
        .map_or(&[][..], |looks| &looks.textures[..]);
    lamps::spawn(commands, visual, &chosen.lamps, textures);
    truck
}
