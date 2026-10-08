//! The water that fills a track's hollows: one level surface over the whole track, at
//! `TrackData::water_level`.
//!
//! - `forces`: how it holds trucks up and slows them down. Monster trucks drive through
//!   water, so it never stops one or sends it back.
//! - `surface`: the surface, drawn with `water.wgsl`, which moves it with waves: a swell
//!   that rolls downwind, and a chop that the wind's gusts ruffle as they sweep over it.
//! - `wind`: the wind that raises the waves and carries the spray, which gusts.
//! - `ripples`: the rings that spread from where trucks disturb it, which the shader draws.
//! - `wake`: where trucks' tires break the surface, and the ripples they leave.
//! - `shore`: where the water meets the land: the foam and surf that `water.wgsl` draws
//!   there, and where the swell and the trucks' ripples break on it.
//!
//! The spray is the `particles` slice's. This slice tells it where the water is broken
//! (`TireInWater`, `Surf`), in `WaterSystems::Waves`, and gives it the wind (`Wind`).
//! - With `WaterSettings::flat`, `surface` draws a plain flat plane instead of both.
//! - `underwater`: the tint over the picture while the camera is under the surface.
//! - `ice`: in snow the water is frozen, a solid sheet of ice that trucks drive on. Then
//!   the water neither holds trucks up nor splashes, and its surface is hidden.
//!
//! Where the ground repeats beyond the edges of the map, so does the water (`water_width`).
//!
//! Only the look moves. Trucks meet the water at its still level: the swell is a few tenths
//! of a metre, which the forces and the splashes pass over.
//!
//! The forces work in any app. What is drawn (the surface's look and ripples), and what is
//! told to the spray, only in an app that can draw.
//!
//! Uses the `track` slice for where the water is, the `truck` slice for the trucks, and the
//! `weather` slice, if it is there, for whether the water is frozen.
//! Its forces are added to the truck's own, after `TruckSystems::Drive` has written them.

mod forces;
mod ice;
mod ripples;
mod shore;
mod surface;
mod underwater;
mod wake;
mod wind;

use std::path::{Path, PathBuf};

use bevy::asset::io::embedded::EmbeddedAssetRegistry;
use bevy::pbr::PbrPlugin;
use bevy::prelude::*;

use crate::game_state::GameState;
use crate::track::{DRAWN_PAST_EDGE, TrackData, TrackSystems};
use crate::truck::TruckSystems;

pub use shore::SwellWave;
pub use surface::WATER_COLOR;
pub use wake::SPLASH_SPEED;
pub use wind::Wind;

pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        // `TrackPlugin`, which this slice needs, has made sure of the state.
        app.init_resource::<WaterSettings>()
            .add_systems(
                OnEnter(GameState::Racing),
                (surface::spawn_water, ice::spawn_ice).after(TrackSystems::Prepare),
            )
            .add_systems(
                FixedUpdate,
                // Before the physics step, which is in `FixedPostUpdate`.
                (
                    ice::freeze_or_thaw
                        .before(TruckSystems::Drive)
                        .run_if(in_state(GameState::Racing)),
                    forces::push_trucks
                        .after(TruckSystems::Drive)
                        .run_if(not(ice::frozen)),
                ),
            );

        // Only an app that draws things can use a material. A headless one has neither
        // the renderer nor anywhere to put a shader.
        if app.is_plugin_added::<PbrPlugin>() {
            // In `src/shaders`, with the game's other shaders: `embedded_asset!` takes only
            // a path below this file's folder.
            app.world_mut()
                .resource_mut::<EmbeddedAssetRegistry>()
                .insert_asset(
                    PathBuf::from("src/shaders/water.wgsl"),
                    Path::new(surface::SHADER_PATH.trim_start_matches("embedded://")),
                    include_bytes!("../shaders/water.wgsl").as_slice(),
                );
            app.add_plugins(MaterialPlugin::<surface::WaterMaterial>::default())
                .init_resource::<ripples::Ripples>()
                .init_resource::<Wind>()
                .add_message::<TireInWater>()
                .add_message::<Surf>()
                // On entering the first race, for one begun before `Startup` (see
                // `particles`).
                .add_systems(
                    OnEnter(GameState::Racing),
                    (shore::find_shore, underwater::spawn_tint).after(TrackSystems::Prepare),
                )
                .add_systems(
                    Update,
                    (
                        (wake::wade, shore::find_surf)
                            .run_if(not(ice::frozen))
                            .in_set(WaterSystems::Waves),
                        surface::show_waves,
                        surface::mirror,
                        underwater::tint_under_water,
                    )
                        .chain()
                        .after(TruckSystems::PlaceVisuals)
                        .run_if(in_state(GameState::Racing)),
                );
        }
    }
}

/// What the water does each frame, for others to order against.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum WaterSystems {
    /// Where the trucks and the swell break the water: `TireInWater` and `Surf` are
    /// written, and the trucks' ripples left. In `Update`.
    Waves,
}

/// A tire breaking the water's surface this frame. Only in an app that can draw, and only
/// while `WaterSettings::splashes` is on.
#[derive(Message, Clone, Copy, Debug)]
pub struct TireInWater {
    /// The truck, as it is drawn (`truck::TruckVisual`).
    pub truck: Entity,
    /// Which of its wheels, as `truck::TruckConfig::wheel_rest` counts them.
    pub wheel: usize,
    /// Where the tire meets the water, on the outside of the tire, at the still level.
    pub at: Vec3,
    /// The middle of the wheel.
    pub hub: Vec3,
    /// Out from the truck's side, of length 1.
    pub outwards: Vec3,
    /// The way the truck faces, of length 1.
    pub forward: Vec3,
    /// How fast the tire goes, in m/s.
    pub moving: Vec3,
    /// How fast its tread goes round, in m/s, forwards positive (`truck::Wheel::tread_speed`):
    /// faster than `moving` while the tire spins.
    pub tread: f32,
    /// How far up the tire the water comes, from 0 (at its bottom) to 1 (over its top).
    pub wet: f32,
    /// The tire's radius and width, in metres.
    pub wheel_radius: f32,
    pub wheel_width: f32,
    /// Whether it was out of the water the frame before.
    pub entered: bool,
    /// When it came down into the water fast enough to splash: how.
    pub plunge: Option<Plunge>,
}

/// How a tire came down into the water.
#[derive(Clone, Copy, Debug)]
pub struct Plunge {
    /// How fast it came down, in m/s.
    pub speed: f32,
    /// The surface under its hub, on the swell as it is drawn.
    pub under_hub: Vec3,
}

/// The water breaking on the shore near the camera this frame. Only in an app that can
/// draw, and only while `WaterSettings::splashes` is on.
#[derive(Message, Clone, Copy, Debug)]
pub struct Surf {
    /// Where, on the water's surface.
    pub at: Vec3,
    /// How far along the shore it breaks, in metres, centred on `at`.
    pub width: f32,
    /// Which way the water comes in, on the ground plane (world X and Z), of length 1.
    pub inwards: Vec2,
    /// How steep the bank is, in metres of rise for each metre across.
    pub steepness: f32,
    pub kind: SurfKind,
}

/// What breaks on the shore.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SurfKind {
    /// A crest of the swell.
    Swell,
    /// A truck's ripple, `height` metres high where it comes in.
    Ripple { height: f32 },
}

/// Choices about the water. Change it at any time.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct WaterSettings {
    /// Whether trucks and the shore throw water up: spray off the tires, splashes where a
    /// wheel comes down, and spray where the swell comes in. The ripples, the wake and the
    /// foam on the water are part of its surface, and stay. Spray in the air when it is
    /// turned off falls as it would.
    pub splashes: bool,
    /// Draws the water as one flat see-through plane in its colour, with no waves, ripples
    /// or foam: no patch of fine mesh round the camera and none of `water.wgsl`, the least
    /// it can cost. How it holds trucks up and the splashes are as ever.
    pub flat: bool,
    /// Whether the moving water mirrors the scenery round it, as well as the sky's colour.
    /// It is found in the picture already drawn, so what is off the picture is not
    /// mirrored. It costs time for every pixel of water seen at a glancing angle.
    pub reflections: bool,
}

impl Default for WaterSettings {
    fn default() -> Self {
        Self {
            splashes: true,
            flat: false,
            reflections: true,
        }
    }
}

/// On the water's surface, which is drawn in an app that can draw, and is there in one
/// that can't, as a marker that the track has water.
#[derive(Component)]
pub struct WaterSurface;

/// How far across the water is, in metres, centred on the origin: the whole track, and
/// where the ground repeats beyond its edges, as far past them as the ground is drawn again
/// (`track::DRAWN_PAST_EDGE`), so that the copies of the ground have water round them too.
fn water_width(track: &TrackData) -> f32 {
    let past_edges = if track.heights.repeats() {
        2.0 * DRAWN_PAST_EDGE
    } else {
        0.0
    };
    track.heights.size() + past_edges
}
