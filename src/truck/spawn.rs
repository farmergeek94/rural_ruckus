//! Builds the truck: a chassis rigid body, which is what the physics knows about, with a
//! wheel-shaped collider at each hub, and the body, wheels and axles that are drawn, which follow it (see `interpolate`) and are built
//! as any truck on show is (see `display`).
//!
//! The player's truck comes from the `ChosenTruck` resource, and how it is set up from
//! `TruckSetup`. The others come from `ComputerTrucks`, each set up at random for
//! the race (`TruckSetup::random`), except for the grip dial.

use std::f32::consts::FRAC_PI_2;

use avian3d::parry::shape::SharedShape;
use avian3d::prelude::*;
use bevy::prelude::*;

use super::CHASSIS_HALF_EXTENTS;
use super::display::{self, Paint};
use super::drive::TipGuard;
use super::interpolate::PhysicsPose;
use super::lamps;
use super::{
    ChosenTruck, ComputerTrucks, Engine, Gearbox, GroundGrip, PlayerTruck, Truck, TruckConfig,
    TruckData, TruckInput, TruckName, TruckSetup, TruckVisual, TruckWheelColliders, TruckWheels,
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

impl Wheel {
    /// How fast the tread goes round, in m/s, forwards positive: as fast as the tire rolls
    /// over the ground, and faster while it spins.
    pub fn tread_speed(&self) -> f32 {
        self.tread_speed
    }
}

/// How much of the speed a tire meets something at it comes back off with, from 0 (a dead
/// stop) to 1 (all of it). None: a truck that hits a wall or another truck stops, and what
/// rocks it back is its own suspension and the give of the contacts, not a bounce. At 0.8
/// trucks ricocheted off each other and off rails.
const TIRE_BOUNCE: f32 = 0.0;
/// How far round the rim of a tire is rounded, as a share of half its width: a monster
/// truck's tire has soft, round shoulders, and a rounded rim rolls onto an edge that a
/// sharp one catches on. Higher is rounder and leaves less flat tread.
const RIM_ROUNDING: f32 = 0.3;
/// How big a wheel's core is, as a share of the tire's radius: a tire sinks into the
/// ground by the rest of its radius at most before the core stands on the surface. Larger
/// lets it sink less, and meets the ground sooner, where the suspension would have coped.
pub(super) const CORE_SHARE: f32 = 0.75;
/// The same for the body: none either. A truck that lands on its roof stays down.
const BODY_BOUNCE: f32 = 0.0;
/// How far ahead of touching something the physics makes a contact with it, in metres.
/// The physics makes one as far ahead as the truck will go in a step unless told
/// otherwise, which keeps a fast body from passing through a thin one. But a truck is
/// built round contacts that are made only where it overlaps something: `contacts` reads
/// where a tire is pressed in, and the sweep in `drive` climbs whatever a tire has
/// reached into. Made ahead of time, the contact with a kerb's face held the tire off it,
/// the sweep never found the top, and the truck stopped dead: measured, a 0.9 m step
/// stopped Bigfoot at 14 m/s, where it had gone over. The truck's tires and body are
/// large and its ground is swept, so it has little to pass through.
const SPECULATION: f32 = 0.0;

/// How far round a tire's rim is rounded, in metres (`RIM_ROUNDING`).
fn rim_rounding(config: &TruckConfig) -> f32 {
    RIM_ROUNDING * config.wheel_width / 2.0
}

/// The shape of a tire: a cylinder `wheel_width` long with its axis along Y, as Parry has
/// a cylinder, of radius `wheel_radius`, with its rim rounded by `rim_rounding`. `drive`
/// turns it onto the axle (`drive::AXLE_ALONG_X`).
pub(super) fn tire_shape(config: &TruckConfig) -> Collider {
    let half_width = config.wheel_width / 2.0;
    let border = rim_rounding(config);
    Collider::from(SharedShape::round_cylinder(
        half_width - border,
        config.wheel_radius - border,
        border,
    ))
}

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
        .and_then(|points| Collider::convex_hull(points.clone()))
        .unwrap_or_else(|| {
            let size = CHASSIS_HALF_EXTENTS * 2.0;
            Collider::cuboid(size.x, size.y, size.z)
        });

    let start =
        Transform::from_translation(SPAWN_POSITION + Vec3::X * SPAWN_SPACING * number as f32);
    let truck = commands
        .spawn((
            Truck,
            TruckName(chosen.name.clone()),
            TruckInput::default(),
            Gearbox::default(),
            Engine::at_idle(&config.engine),
            start,
            PhysicsPose::at(start),
            config.clone(),
            DespawnOnExit(GameState::Racing),
        ))
        // A second insert because bundles are tuples, and tuples top out at 15 elements.
        .insert((
            RigidBody::Dynamic,
            collider,
            // So that other trucks' wheel cores touch it (see `collision_groups`).
            crate::collision_groups::truck_body(),
            // The truck's own figures, whatever its colliders would make of them.
            (
                Mass(config.mass),
                AngularInertia::new(principal_inertia),
                CenterOfMass(config.center_of_mass),
                NoAutoMass,
                NoAutoAngularInertia,
                NoAutoCenterOfMass,
            ),
            Friction::new(0.4),
            // Bodywork that does not spring back off what it hits. `Min`, as the tires
            // have: scenery names a bounce of 1 (see `scenery::bouncy`), and `Max` wins
            // over every other rule, so under it the body bounced off scenery with all of
            // its speed.
            Restitution::new(BODY_BOUNCE).with_combine_rule(CoefficientCombine::Min),
            GroundGrip::default(),
            LinearDamping(0.02),
            AngularDamping(0.4),
            // The suspension forces come from us, not from contacts, so the physics can't
            // tell on its own that a resting truck still needs simulating.
            SleepingDisabled,
            // Contacts only where the truck touches, not where it is about to: see
            // `SPECULATION`.
            SpeculativeMargin(SPECULATION),
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
    // A tire-shaped collider at each wheel, as a child of the body so that the physics
    // makes it part of it, which `drive` keeps at the hub as the suspension moves. It is what
    // walls, rails, scenery and other trucks meet at wheel height, which the tire grips
    // and climbs (see `contacts`), and what another truck's tire can climb onto. It touches
    // the ground too, but only where the suspension doesn't: a contact under the tread
    // would fight the cast, so `contacts` drops it, and what is left holds up a tire the
    // ground meets anywhere else. `drive` sweeps the same shape, so that what the sweep
    // finds and what the collider meets agree.
    let tire = tire_shape(&config);
    let wheel_colliders = config
        .wheel_rest
        .iter()
        .map(|&rest| {
            let collider = commands
                .spawn((
                    Name::new("Wheel collider"),
                    WheelCollider {
                        bottomed: false,
                        stands_on: None,
                        throttle: 0.0,
                        engine_force: 0.0,
                        brakes_on: false,
                    },
                    ChildOf(truck),
                    Transform::from_translation(rest)
                        .with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
                    tire.clone(),
                    crate::collision_groups::wheel(),
                    // So that `contacts` is asked which of its contacts to keep.
                    ActiveCollisionHooks::MODIFY_CONTACTS,
                    // The body's mass is given whole on the chassis.
                    ColliderDensity(0.0),
                    // A tire grips whatever it touches with its own grip, whatever that is
                    // made of: `Max` wins over every other rule.
                    Friction::new(config.grip).with_combine_rule(CoefficientCombine::Max),
                    // No bounce: `Min` so that nothing it touches can give it one.
                    Restitution::new(TIRE_BOUNCE).with_combine_rule(CoefficientCombine::Min),
                ))
                .id();
            // The core is the collider's child, at its middle, so that it goes wherever
            // `drive` puts the collider, with nothing to move of its own.
            commands.spawn((
                Name::new("Wheel core"),
                WheelCore,
                ChildOf(collider),
                Transform::IDENTITY,
                // A ball, which rolls over the edges between the ground's triangles
                // where the rim of a cylinder could catch on them.
                Collider::sphere(config.wheel_radius * CORE_SHARE),
                crate::collision_groups::wheel_core(),
                // So that `contacts` decides its contacts with the ground as the tire's.
                ActiveCollisionHooks::MODIFY_CONTACTS,
                ColliderDensity(0.0),
                // It rolls, and never scrubs: the tire's grip is the tread's (see `contacts`).
                Friction::new(0.0).with_combine_rule(CoefficientCombine::Min),
                Restitution::new(TIRE_BOUNCE).with_combine_rule(CoefficientCombine::Min),
            ));
            collider
        })
        .collect();
    commands.entity(truck).insert(TruckWheelColliders {
        colliders: wheel_colliders,
        tire,
    });

    let wheels = display::build(commands, visual, chosen, &config, paint);
    commands.entity(truck).insert(TruckWheels(wheels));
    let textures = chosen
        .looks
        .as_ref()
        .map_or(&[][..], |looks| &looks.textures[..]);
    lamps::spawn(commands, visual, &chosen.lamps, textures);
    truck
}
