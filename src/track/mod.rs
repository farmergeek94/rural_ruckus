//! The track being raced on: the ground, the course around it, and its checkpoints.
//!
//! A track is plain data (`TrackData`), either generated in code or converted from a
//! Monster Truck Madness 2 POD archive. This slice holds the current one in the `Track`
//! resource and builds the terrain from it: one height grid feeds both the render mesh
//! and the physics heightfield collider, so the two can never disagree.

mod blend;
mod collider;
mod course;
mod data;
mod generator;
mod ground_boxes;
mod height_grid;
mod map;
mod mesh;
mod pod_import;
mod pod_scenery;
mod shading;
mod tile_material;

use avian3d::prelude::*;
use bevy::asset::embedded_asset;
use bevy::ecs::change_detection::Tick;
use bevy::pbr::PbrPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

use crate::game_state::GameState;

pub use collider::build_collider;
pub use course::{Course, Nearest};
pub use data::{
    Backdrop, BoxFaces, Footing, Gate, GroundBox, GroundCell, GroundTextures, KeyframedFace,
    Keyframes, Scenery, SceneryModel, SceneryMotion, SceneryObject, Skies, SkyPicture,
    StartPosition, TextureCycle, TrackData, direction_yaw, yaw_direction,
};
pub use generator::builtin_track;
pub use height_grid::{Diagonals, HeightGrid};
pub use map::{MAP_COURSE, MAP_GATE, MAP_START, MapFrame, map_frame, map_image};
pub use pod_import::{BaseTrack, load_base, load_pod, peek_base, peek_pod, track_from_pod};
pub use tile_material::{
    GroundShading, MAX_TEXTURE_CYCLES, TileCycles, TileLighting, TileMaterial, TileTextures,
    tile_array,
};

/// The track in use, as it is raced on. Insert one before adding `TrackPlugin` to choose
/// the track; otherwise the built-in one is used. Once the app runs, choose with
/// `ChosenTrack`.
#[derive(Resource, Deref)]
pub struct Track(pub TrackData);

/// The track as it was chosen. Insert another before a race is entered and that race is
/// on it. `Track` is made from this as the race is entered.
#[derive(Resource, Deref)]
pub struct ChosenTrack(pub TrackData);

/// Which `ChosenTrack` `Track` was last made from, by when it changed.
#[derive(Resource)]
struct PreparedFrom {
    chosen: Tick,
}

/// For other slices to order their systems against this one.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum TrackSystems {
    /// Makes `Track` out of `ChosenTrack`, on entering a race. Run after this set to
    /// build on the ground the race will be run on.
    Prepare,
}

/// Choices about how tracks are presented. Insert it before adding `TrackPlugin`, or change
/// it between races: each race reads it as it begins.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct TrackSettings {
    /// Lights the ground as if its creases were rounded off (`shading`). Monster Truck
    /// Madness 2 ground is made of flat triangles 10 m across, and with this off it is lit
    /// as they are. Either way its shape, and what trucks drive on, is those triangles:
    /// this changes only the light and shade, and costs no time on the CPU while racing.
    /// Creases too sharp to round off, such as the lip of a ramp over a cliff, stay sharp.
    /// Read as each race begins.
    pub smooth_terrain: bool,
    /// Fades the textures of neighbouring ground cells into each other where they meet,
    /// blurred a little (`blend`), where Monster Truck Madness 2 draws each cell's texture
    /// edge to edge with a hard line between. Costs more texture reads for the pixels of
    /// ground near the edge of a cell, and no time on the CPU while racing. On by default,
    /// as part of the front end's Best quality; every lower level turns it off. Read as each
    /// race begins.
    pub blend_ground: bool,
    /// Gives the textures of the ground and the scenery smaller copies of themselves to
    /// be drawn from at a distance, which stops them shimmering. On unless there is a
    /// reason to see the game without them.
    pub mipmaps: bool,
    /// How much sharper the ground may be drawn along the direction it recedes in than a
    /// plain mipmap allows, from 1 (not at all) to 16. Ground is nearly always seen at a
    /// shallow angle, where a plain mipmap turns it to mush a few truck lengths ahead.
    /// Costs texture reads for every pixel of ground. May be changed while racing.
    pub anisotropy: u16,
    /// How far from the camera scenery is drawn, in metres, or `f32::INFINITY` for all of
    /// it. A track can have thousands of objects, and each one drawn costs the CPU and the
    /// graphics processor every frame, however small it is on screen. Solid ones stay solid
    /// beyond it. May be changed while racing.
    pub scenery_distance: f32,
    /// Whether the scenery that trucks go through, such as most bushes and trees, is drawn.
    /// What is solid is drawn either way, so that nothing unseen stands in the way. May be
    /// changed while racing.
    pub decorations: bool,
}

impl Default for TrackSettings {
    fn default() -> Self {
        Self {
            smooth_terrain: false,
            blend_ground: true,
            mipmaps: true,
            anisotropy: 16,
            scenery_distance: f32::INFINITY,
            decorations: true,
        }
    }
}

pub struct TrackPlugin;

impl Plugin for TrackPlugin {
    fn build(&self, app: &mut App) {
        // Systems, here and in other slices, can count on the settings being there.
        app.init_resource::<TrackSettings>();
        let chosen = match app.world_mut().remove_resource::<Track>() {
            Some(Track(chosen)) => chosen,
            None => builtin_track(),
        };
        // Done here as well as on entering a race, so that whatever looks at the ground
        // before the app has run sees what the race will.
        app.insert_resource(Track(chosen.clone()))
            .insert_resource(ChosenTrack(chosen));
        let chosen = app.world().resource_ref::<ChosenTrack>().last_changed();
        app.insert_resource(PreparedFrom { chosen });

        // A slice works alone, so it makes sure of the state it is built in. A second
        // `init_state` would log a warning.
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        if !app.world().contains_resource::<State<GameState>>() {
            app.init_state::<GameState>();
        }

        // Only an app that draws things can use a material. A headless one has neither
        // the renderer nor anywhere to put a shader.
        if app.is_plugin_added::<PbrPlugin>() {
            embedded_asset!(app, "tiles.wgsl");
            embedded_asset!(app, "tiles_prepass.wgsl");
            app.add_plugins(MaterialPlugin::<TileMaterial>::default());
        }
        app.add_systems(
            OnEnter(GameState::Racing),
            (
                prepare_track.in_set(TrackSystems::Prepare),
                spawn_terrain.after(TrackSystems::Prepare),
            ),
        )
        .add_systems(
            Update,
            tile_material::apply_anisotropy.run_if(resource_changed::<TrackSettings>),
        );
    }
}

/// How much of the speed a tire meets the ground at it comes back off with. A monster
/// truck lands heavily, squats on its tires and settles; it does not skip back off the
/// dirt like a ball.
const GROUND_BOUNCE: f32 = 0.2;

/// Before anything has looked at the ground, so that the mesh, the collider and everything
/// standing on the terrain agree about where it is. Does nothing when the race is on the
/// track the last one was on.
fn prepare_track(
    chosen: Res<ChosenTrack>,
    mut track: ResMut<Track>,
    mut prepared_from: ResMut<PreparedFrom>,
) {
    if prepared_from.chosen == chosen.last_changed() {
        return;
    }
    track.0 = chosen.0.clone();
    *prepared_from = PreparedFrom {
        chosen: chosen.last_changed(),
    };
}

fn spawn_terrain(
    mut commands: Commands,
    track: Res<Track>,
    settings: Res<TrackSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    // Both absent in a headless app, which has no use for textures.
    images: Option<ResMut<Assets<Image>>>,
    tile_materials: Option<ResMut<Assets<TileMaterial>>>,
) {
    let surface = StandardMaterial {
        perceptual_roughness: 0.95,
        ..default()
    };
    // The ground boxes are part of the ground: trucks drive on them as on it, and the
    // wheels treat them the same (see `truck/contacts.rs`).
    let ground = |layers: CollisionLayers| {
        (
            RigidBody::Static,
            // So that `truck/contacts.rs` can tell a wheel's contacts with it apart.
            layers,
            Friction::new(1.0),
            // Dirt, not rubber. Once a truck's springs are shut its tires are what meets
            // the ground (see `truck/contacts.rs`), and a tire's own bounce is a lively
            // 0.8: at that a truck dropped flat from 30 m came off the ground at 70% of
            // the speed it hit at and flew 13 m back up. The tire takes the softer of the
            // two (see `truck/spawn.rs`), so this is what it gets here, and its own is
            // what it keeps against a rail or another truck.
            Restitution::new(GROUND_BOUNCE).with_combine_rule(CoefficientCombine::Min),
        )
    };
    let terrain = commands
        .spawn((
            Name::new("Terrain"),
            DespawnOnExit(GameState::Racing),
            Transform::default(),
            Visibility::default(),
            build_collider(&track.heights),
            ground(crate::collision_groups::ground()),
        ))
        .id();
    if let Some(collider) = ground_boxes::build_collider(&track.ground_boxes) {
        commands.spawn((
            Name::new("Ground boxes"),
            ChildOf(terrain),
            Transform::default(),
            collider,
            // And their upright sides apart from the terrain's slopes.
            ground(crate::collision_groups::ground_boxes()),
        ));
    }

    info!(
        "track \"{}\": smooth shading {}, blended ground {}, texture mipmaps {}",
        track.name,
        if settings.smooth_terrain { "on" } else { "off" },
        if settings.blend_ground { "on" } else { "off" },
        if settings.mipmaps {
            "on"
        } else {
            "off (none generated or loaded)"
        },
    );

    // Worked out only for a track that is drawn with it.
    let ground_shading = |images: &mut Assets<Image>| {
        let normals = settings
            .smooth_terrain
            .then(|| images.add(shading::normal_map(&track.heights)));
        (
            GroundShading {
                size: track.heights.size(),
                lit_smoothly: normals.is_some() as u32,
                ..default()
            },
            normals,
        )
    };
    match (&track.ground, images, tile_materials) {
        (Some(ground), Some(mut images), Some(mut tile_materials)) => {
            let tiles = images.add(tile_array(ground.tile_size, &ground.tiles, &settings));
            let (mut shading, normals) = ground_shading(&mut images);
            let cells = settings
                .blend_ground
                .then(|| images.add(blend::cell_map(ground)));
            if cells.is_some() {
                shading.cells = ground.cells_per_side() as u32;
                shading.blend_width = blend::BLEND_WIDTH;
            }
            let material = tile_materials.add(TileMaterial {
                base: surface.clone(),
                extension: TileTextures {
                    tiles: tiles.clone(),
                    ground_normals: normals,
                    ground: shading,
                    ground_cells: cells,
                    ..default()
                },
            });
            for chunk in mesh::build_textured_meshes(&track.heights, ground) {
                commands.spawn((
                    Mesh3d(meshes.add(chunk)),
                    MeshMaterial3d(material.clone()),
                    ChildOf(terrain),
                ));
            }
            // Lit by their own faces: the smooth surface through the ground's heights is
            // not theirs.
            let boxes = tile_materials.add(TileMaterial {
                base: surface,
                extension: TileTextures { tiles, ..default() },
            });
            let (textured, plain) = ground_boxes::build_meshes(&track.ground_boxes, true);
            for chunk in textured {
                commands.spawn((
                    Mesh3d(meshes.add(chunk)),
                    MeshMaterial3d(boxes.clone()),
                    ChildOf(terrain),
                ));
            }
            spawn_plain_boxes(&mut commands, terrain, plain, &mut meshes, &mut materials);
        }
        // Shaded ground lit smoothly: the tile material for its normal map, with a tile
        // that its meshes, having no tile numbers, never read.
        (None, Some(mut images), Some(mut tile_materials)) if settings.smooth_terrain => {
            let tiles = images.add(tile_array(1, &[vec![255; 4]], &settings));
            let (shading, normals) = ground_shading(&mut images);
            let material = tile_materials.add(TileMaterial {
                base: surface,
                extension: TileTextures {
                    tiles,
                    ground_normals: normals,
                    ground: shading,
                    ..default()
                },
            });
            for chunk in mesh::build_shaded_meshes(&track) {
                commands.spawn((
                    Mesh3d(meshes.add(chunk)),
                    MeshMaterial3d(material.clone()),
                    ChildOf(terrain),
                ));
            }
            let (_, plain) = ground_boxes::build_meshes(&track.ground_boxes, false);
            spawn_plain_boxes(&mut commands, terrain, plain, &mut meshes, &mut materials);
        }
        _ => {
            let material = materials.add(surface);
            for chunk in mesh::build_shaded_meshes(&track) {
                commands.spawn((
                    Mesh3d(meshes.add(chunk)),
                    MeshMaterial3d(material.clone()),
                    ChildOf(terrain),
                ));
            }
            let (_, plain) = ground_boxes::build_meshes(&track.ground_boxes, false);
            spawn_plain_boxes(&mut commands, terrain, plain, &mut meshes, &mut materials);
        }
    }
}

/// Colour of ground boxes drawn without textures: weathered concrete.
const PLAIN_BOX_COLOR: Color = Color::srgb(0.55, 0.53, 0.5);

fn spawn_plain_boxes(
    commands: &mut Commands,
    terrain: Entity,
    chunks: Vec<Mesh>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    if chunks.is_empty() {
        return;
    }
    let material = materials.add(StandardMaterial {
        base_color: PLAIN_BOX_COLOR,
        perceptual_roughness: 0.95,
        ..default()
    });
    for chunk in chunks {
        commands.spawn((
            Mesh3d(meshes.add(chunk)),
            MeshMaterial3d(material.clone()),
            ChildOf(terrain),
        ));
    }
}
