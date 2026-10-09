//! A cast vehicle: the chassis is a single rigid body and each wheel is a tire-shaped
//! cylinder cast downwards from its mount point. Where it touches the ground we apply a
//! spring/damper force (suspension) plus grip and drive forces (tire) to the chassis.
//!
//! Wheels are not bodies of their own, which is what keeps this model stable and cheap.
//! Each is also a collider on the chassis, moved to its hub every step, that touches
//! everything but the ground: walls, rails, scenery and other trucks meet a tire, and a
//! tire can be stood on, so trucks climb over each other.
//!
//! A race has one truck that the player drives (`PlayerTruck`, from `ChosenTruck`) and any
//! number that someone else drives (from `ComputerTrucks`). They are built alike, and
//! differ only in who writes their `TruckInput`.
//!
//! On a world that repeats (`RepeatingWorld`), a truck that goes over an edge of the map is
//! moved across to the other (`wrap`), and `TruckWrapped` says so.
//!
//! A truck is plain data (`TruckData`): the built-in one, or one converted from a Monster
//! Truck Madness 2 archive by `pod_import`. Such an archive gives a truck's shape and its
//! looks, and nothing of how it drives, which is `TruckConfig`'s defaults for every truck.

mod axle;
mod config;
mod contacts;
mod data;
mod display;
mod drive;
mod gearbox;
mod input;
mod interpolate;
mod lamps;
mod looks;
mod pod_import;
mod reset;
mod spawn;
mod speedometer;
mod wrap;

use avian3d::prelude::{Collider, PhysicsSystems};
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

use crate::game_state::GameState;

pub use config::{TruckConfig, TruckSetup};
pub use contacts::{TireContacts, decide_tire_contacts};
pub use data::{
    AxleLink, AxleLinks, Beam, Dashboard, DashboardPicture, Dial, NormalMap, SteeringWheel,
    TruckData, TruckLamp, TruckLooks, TruckMesh, TruckModel, TruckTexture, TruckTextureCycle,
};
pub use display::TruckDisplay;
pub use gearbox::{FORWARD_GEARS, Gear, Gearbox};
pub use input::TruckInput;
pub use lamps::TruckLamps;
pub use pod_import::{
    BODY_DROP, BaseTruck, load_base, load_pod, peek_base, peek_pod, pod_holds_truck, truck_from_pod,
};
pub use reset::PlaceTruck;
pub use spawn::Wheel;
pub use wrap::{RepeatingWorld, TruckWrapped};

pub struct TruckPlugin;

impl Plugin for TruckPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<ChosenTruck>() {
            app.insert_resource(ChosenTruck(TruckData::builtin()));
        }
        // A slice works alone, so it makes sure of the state it is built in. A second
        // `init_state` would log a warning.
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        if !app.world().contains_resource::<State<GameState>>() {
            app.init_state::<GameState>();
        }
        app.init_resource::<TruckSetup>()
            .init_resource::<crate::keys::KeyBindings>()
            .init_resource::<ComputerTrucks>()
            .init_resource::<SpeedUnits>()
            .init_resource::<Transmission>()
            .init_resource::<TruckLooksSettings>()
            .init_resource::<TruckLamps>()
            .init_resource::<RepeatingWorld>()
            .add_message::<PlaceTruck>()
            .add_message::<TruckWrapped>()
            .add_systems(
                OnEnter(GameState::Racing),
                (spawn::spawn_trucks, speedometer::spawn_speedometer),
            )
            .add_systems(
                Update,
                (
                    input::read_input,
                    reset::reset_truck,
                    reset::place_trucks.in_set(TruckSystems::Place),
                    speedometer::update_speedometer,
                    (interpolate::place_visuals, interpolate::place_wheels)
                        .in_set(TruckSystems::PlaceVisuals),
                    // After the wheels they hang from have been placed for this frame.
                    (axle::place_axles, axle::place_axle_links).after(TruckSystems::PlaceVisuals),
                    display::build_displays,
                    lamps::dress_lamps,
                    lamps::light_lamps,
                    lamps::turn_glows.after(TruckSystems::PlaceVisuals),
                    lamps::show_from_inside,
                    looks::apply_settings.run_if(resource_changed::<TruckLooksSettings>),
                    looks::cycle_textures,
                ),
            )
            // Forces must be computed at the physics rate, before each step, which the
            // physics takes in `FixedPostUpdate`.
            .add_systems(
                FixedUpdate,
                (
                    gearbox::change_gears.before(TruckSystems::Drive),
                    drive::drive_truck.in_set(TruckSystems::Drive),
                ),
            )
            .add_systems(
                FixedPostUpdate,
                (wrap::bring_back_on, interpolate::record_poses)
                    .chain()
                    .after(PhysicsSystems::Writeback),
            );
    }
}

/// The truck to drive. Insert one before adding `TruckPlugin` to choose the truck;
/// otherwise the built-in one is used.
#[derive(Resource, Deref)]
pub struct ChosenTruck(pub TruckData);

/// The trucks that race beside the player's, one of each. The slice builds them and
/// leaves their `TruckInput` alone: whoever drives them writes it. Set it before a race is
/// entered. Empty unless someone asks for them.
#[derive(Resource, Default)]
pub struct ComputerTrucks(pub Vec<TruckData>);

/// What the speedometer reads in. Change it at any time.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpeedUnits {
    #[default]
    Kmh,
    Mph,
}

/// Who changes the player's gears (see `Gearbox`). Change it at any time. The computer's
/// trucks are always automatic.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Transmission {
    #[default]
    Automatic,
    /// The player changes the forward gears with `Control::ShiftUp` and
    /// `Control::ShiftDown`. Reverse still goes in by itself.
    Manual,
}

/// How trucks are drawn. Change it at any time: trucks already built are brought up to date.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct TruckLooksSettings {
    /// How much of the sky and the sun a truck's surfaces mirror, from 0 to 1 (Bevy's
    /// `StandardMaterial::reflectance`, where 0.5 is 4% and Bevy's default). Monster Truck
    /// Madness 2 had none. Even a little lays a grey sheen over dark rubber that hides the
    /// tread, whose texels are only a few shades apart.
    pub reflectance: f32,
}

impl Default for TruckLooksSettings {
    fn default() -> Self {
        Self { reflectance: 0.0 }
    }
}

/// On the one `Truck` that the keyboard and the gamepad drive. The camera, the
/// speedometer and the race readout follow it.
#[derive(Component)]
pub struct PlayerTruck;

/// Query filter for the player's truck.
pub type Player = (With<Truck>, With<PlayerTruck>);

/// On the player's truck when the keyboard and the gamepad no longer drive it, as after the
/// finish: whoever put it there writes its `TruckInput`. It is still the player's truck, and
/// the camera and the readouts still follow it.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Autopilot;

/// On a truck that is held where it stands, as on the starting grid before the start: its
/// throttle does nothing and its brakes are on. It can still be steered. Whoever holds it
/// takes it off to let it go.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Held;

/// On a truck: its name, as its `TruckData` gives it, for the player to know it by.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct TruckName(pub String);

/// On a drawn truck (`TruckVisual`) that the camera is inside, as in a cockpit view: its
/// body, wheels and the cones of its beams are hidden, and its lamps still light the way.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SeenFromInside;

/// The truck as the physics knows it: the rigid body. It moves in physics steps, so draw
/// and follow the `TruckVisual` instead.
#[derive(Component)]
pub struct Truck;

/// On a truck: what the ground under each of its tires multiplies the tire's grip by, in
/// `TruckConfig::wheel_rest`'s order. 1 leaves it as it is, less slides, and a little more
/// is firm ground. The slice builds a truck with all of it as it is and leaves it so:
/// whoever knows the ground writes it, before `TruckSystems::Drive`.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct GroundGrip(pub [f32; 4]);

impl Default for GroundGrip {
    fn default() -> Self {
        Self([1.0; 4])
    }
}

/// The truck as it is drawn: the body, with the wheels as its children. It follows its
/// `truck` smoothly from frame to frame.
#[derive(Component)]
pub struct TruckVisual {
    pub truck: Entity,
}

/// The wheels of a truck, which are children of its `TruckVisual`.
#[derive(Component)]
pub struct TruckWheels(pub(crate) Vec<Entity>);

/// The wheels' colliders, children of the body, in the same order as `TruckWheels`. Each
/// is moved to its hub every physics step (see `drive`).
#[derive(Component)]
pub(crate) struct TruckWheelColliders {
    pub(crate) colliders: Vec<Entity>,
    /// Their shape, which all four share: what `drive` sweeps each tire down with, kept
    /// here so that it need not look a collider up for it.
    pub(crate) tire: Collider,
}

/// On a wheel's collider.
#[derive(Component)]
pub(crate) struct WheelCollider {
    /// Whether the tire is pressed past the top of its travel this step, and squashed as
    /// far as it goes (`drive::GROUND_SQUASH` on the terrain, `drive::EDGE_SQUASH` into an
    /// edge). There the springs have nothing left to give and the tire is simply solid:
    /// `drive` stops pushing and `contacts` keeps the ground contact, which is what holds
    /// the truck up and bounces it. Written by `drive` every step, read by `contacts`.
    pub(super) bottomed: bool,
    /// What the suspension rolls the tire over the top of this step, if it is not the
    /// terrain: scenery, or the ground boxes. A tire met fast with an edge as high as its
    /// hub goes into the edge's face for a few steps as the springs lift it, and `contacts`
    /// lets it, and its core, further into that than into another face. Written by `drive`
    /// every step, read by `contacts` in the same step.
    pub(super) rolling_over: Option<Entity>,
}

/// A solid ball inside a wheel, at its hub, that touches the ground and the bodies of
/// other trucks, and nothing else. The ground under a tire is the suspension's, and a tire driven into it harder than the
/// suspension can answer in a step went on through; the core cannot, so a tire sinks into
/// the ground no further than the core is inside it, and then stands on the core, which
/// rolls over the surface (see `contacts`) and bounces off it as a tire does. Measured on
/// Alpine, two minutes of 7 trucks: without cores an axle went under the ground 9 times,
/// with them never. Another truck's body it meets rigidly, so that a tire which rides up
/// that truck does not go into its body further than the core is inside the tire.
#[derive(Component)]
pub(crate) struct WheelCore;

/// For other slices to order their systems against this one.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum TruckSystems {
    /// Handles `PlaceTruck` messages, in `Update`. Write them from a system that runs
    /// before this set and the truck moves in the same frame.
    Place,
    /// Moves every `TruckVisual` to where its truck is this frame, in `Update`. Run
    /// after this set to follow a truck on screen without judder.
    PlaceVisuals,
    /// Works out each truck's suspension and tire forces, in `FixedUpdate`, and hands them
    /// to the physics for the coming step. Add forces of your own through `Forces` in
    /// `FixedUpdate` too: they add to these, in any order.
    Drive,
}
