//! The water that fills a track's hollows: one level surface over the whole track, at
//! `TrackData::water_level`.
//!
//! - `forces`: how it holds trucks up and slows them down. Monster trucks drive through
//!   water, so it never stops one or sends it back.
//! - `surface`: the surface, drawn with `water.wgsl`, which moves it with waves: a swell
//!   that rolls downwind, and a chop that the wind's gusts ruffle as they sweep over it.
//! - `wind`: the wind that raises the waves and carries the spray, which gusts.
//! - `ripples`: the rings that spread from where trucks disturb it, which the shader draws.
//! - `splash`: the spray and splashes that trucks throw up, and the ripples they leave.
//! - `shore`: where the water meets the land: the foam and surf that `water.wgsl` draws
//!   there, and the spray thrown up where the swell and the trucks' ripples come in.
//! - `ice`: in snow the water is frozen, a solid sheet of ice that trucks drive on. Then
//!   the water neither holds trucks up nor splashes, and its surface is hidden.
//!
//! Only the look moves. Trucks meet the water at its still level: the swell is a few tenths
//! of a metre, which the forces and the splashes pass over.
//!
//! The forces work in any app. What is drawn (the surface's look, ripples and spray) only
//! in an app that can draw.
//!
//! Uses the `track` slice for where the water is, the `truck` slice for the trucks, and the
//! `weather` slice, if it is there, for whether the water is frozen.
//! Its forces are added to the truck's own, after `TruckSystems::Drive` has written them.

mod forces;
mod ice;
mod ripples;
mod shore;
mod splash;
mod surface;
mod wind;

use bevy::asset::embedded_asset;
use bevy::pbr::PbrPlugin;
use bevy::prelude::*;

use crate::game_state::GameState;
use crate::track::TrackSystems;
use crate::truck::TruckSystems;

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
            embedded_asset!(app, "water.wgsl");
            crate::particles::add(app);
            app.add_plugins(MaterialPlugin::<surface::WaterMaterial>::default())
                .init_resource::<ripples::Ripples>()
                .init_resource::<wind::Wind>()
                .add_systems(Startup, splash::make_droplet_looks)
                .add_systems(
                    OnEnter(GameState::Racing),
                    (shore::find_shore, splash::spawn_spray).after(TrackSystems::Prepare),
                )
                .add_systems(
                    Update,
                    (
                        (splash::make_waves, shore::splash_shore).run_if(not(ice::frozen)),
                        splash::spread_rings,
                        surface::show_waves,
                    )
                        .chain()
                        .after(TruckSystems::PlaceVisuals)
                        .run_if(in_state(GameState::Racing)),
                );
        }
    }
}

/// Choices about the water. Change it at any time.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct WaterSettings {
    /// Whether trucks and the shore throw water up: spray off the tires, splashes where a
    /// wheel comes down, and spray where the swell comes in. The ripples, the wake and the
    /// foam on the water are part of its surface, and stay. Spray in the air when it is
    /// turned off falls as it would.
    pub splashes: bool,
}

impl Default for WaterSettings {
    fn default() -> Self {
        Self { splashes: true }
    }
}

/// On the water's surface, which is drawn in an app that can draw, and is there in one
/// that can't, as a marker that the track has water.
#[derive(Component)]
pub struct WaterSurface;
