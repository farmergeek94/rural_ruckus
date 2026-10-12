//! What trucks and the water throw up into the air, as particles that the graphics card
//! moves.
//!
//! - `pool`: a pool of particles, moved on the graphics card. Knows nothing of the game.
//! - `ribbon`: ribbons laid on the ground, drawn on the graphics card. Knows nothing of
//!   the game.
//! - `dirt`: the clods, splatter and dust that tires throw up off loose ground.
//! - `tire_marks`: the tracks that tires press into loose ground, as ribbons.
//! - `sparks`: the sparks that fly where a truck's body scrapes or hits something hard.
//! - `spray`: the fan of water that tires throw as they go through the water, the mist
//!   with it, the froth they churn up, and the splash where a wheel comes down into it.
//! - `surf`: the spray where the water breaks on the shore.
//!
//! Each throws into its own pools, which are spawned with each race. Where the water is
//! broken, the `water` slice says (`water::TireInWater`, `water::Surf`); it also gives the
//! wind that carries the spray, and the swell that the foam rides. Where a body touches
//! something hard, the physics' contacts say (`sparks::Scrape`). Everything but the sparks,
//! which glow, is as bright as the `weather` slice says (`weather::ParticleLight`).
//!
//! Uses the `track` slice for the ground, the `truck` slice for the trucks, `scene_depth`
//! for the depth that particles fade against, and the `water` and `weather` slices, if they
//! are there. Only a look: does nothing in an app that cannot draw.

mod dirt;
mod pool;
mod ribbon;
mod sparks;
mod spray;
mod surf;
mod tire_marks;

use avian3d::prelude::PhysicsSystems;
use bevy::pbr::PbrPlugin;
use bevy::prelude::*;

use crate::game_state::GameState;
use crate::scene_depth::SceneDepth;
use crate::track::TrackSystems;
use crate::truck::TruckSystems;
use crate::water::{Surf, TireInWater, WaterSettings, WaterSystems};
use crate::weather::ParticleLight;

pub use dirt::{Dirt, DirtSettings};
pub use sparks::{SparkSettings, Sparks};
pub use tire_marks::{TireMarkSettings, TireMarks};

pub struct ParticlesPlugin;

impl Plugin for ParticlesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DirtSettings>()
            .init_resource::<SparkSettings>()
            .init_resource::<TireMarkSettings>();
        // Only a look. An app that cannot draw has no use for it.
        if !app.is_plugin_added::<PbrPlugin>() {
            return;
        }
        pool::add(app);
        ribbon::add(app);
        // Also made by `water`, if it is there; without it, none are written.
        app.add_message::<TireInWater>()
            .add_message::<Surf>()
            .add_message::<sparks::Scrape>()
            .add_systems(Update, (light_particles, rebind_scene_depth))
            // Made on entering the first race, not at `Startup`: a race begun from the
            // command line enters `Racing` before `Startup` runs.
            .add_systems(
                OnEnter(GameState::Racing),
                (
                    (
                        dirt::make_looks.run_if(not(resource_exists::<dirt::DirtLooks>)),
                        dirt::spawn_dirt,
                    )
                        .chain(),
                    (
                        spray::make_droplet_looks
                            .run_if(not(resource_exists::<spray::DropletLooks>)),
                        spray::spawn_spray,
                    )
                        .chain(),
                    (
                        sparks::make_looks.run_if(not(resource_exists::<sparks::SparkLooks>)),
                        sparks::spawn_sparks,
                    )
                        .chain(),
                    (
                        tire_marks::make_looks
                            .run_if(not(resource_exists::<tire_marks::TreadLooks>)),
                        tire_marks::spawn_tire_marks,
                    )
                        .chain(),
                )
                    .after(TrackSystems::Prepare),
            )
            // After the step, from the contacts it kept.
            .add_systems(
                FixedPostUpdate,
                sparks::find_scrapes
                    .after(PhysicsSystems::Writeback)
                    .run_if(in_state(GameState::Racing)),
            )
            .add_systems(
                Update,
                (
                    dirt::throw_dirt,
                    tire_marks::lay_tire_marks,
                    sparks::throw_sparks,
                    // The trucks' spray first, so that the shore leaves room for it.
                    (spray::throw_spray, spray::spread_rings, surf::throw_surf)
                        .chain()
                        .after(WaterSystems::Waves),
                    spray::keep_foam_on_the_water
                        .run_if(resource_exists_and_changed::<WaterSettings>),
                )
                    .after(TruckSystems::PlaceVisuals)
                    .run_if(in_state(GameState::Racing)),
            );
    }
}

/// Makes every pool but the ones that glow as bright as the weather says, or full bright
/// without it.
fn light_particles(
    light: Option<Res<ParticleLight>>,
    pools: Query<&pool::Particles, Without<sparks::Glows>>,
    mut materials: ResMut<Assets<pool::ParticleMaterial>>,
) {
    let level = light.map_or(1.0, |light| light.0);
    pool::light(level, pools, &mut materials);
}

/// A new scene depth image (the window changed size) is bound only when a material is
/// touched (see `scene_depth::SceneDepth`).
fn rebind_scene_depth(
    scene_depth: Res<SceneDepth>,
    mut materials: ResMut<Assets<pool::ParticleMaterial>>,
) {
    if scene_depth.is_changed() {
        for _ in materials.iter_mut() {}
    }
}
