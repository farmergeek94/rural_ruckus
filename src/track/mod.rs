//! The track being raced on: the ground, the course around it, and its checkpoints.
//!
//! A track is plain data (`TrackData`, from the `content` crate), either generated in code
//! (`generator`) or filled in by a loader such as the `pod` crate. This slice holds the
//! current one in the `Track` resource and builds the terrain from it: one height grid feeds both the render mesh
//! and the physics heightfield collider, so the two can never disagree.
//!
//! Where the ground repeats (`HeightGrid::repeats`), as a Monster Truck Madness 2 world
//! does, the ground is copied round the map: the collider whole, and what is drawn as far
//! from the map as the camera sees (`DRAWN_PAST_EDGE`). Copies share their originals'
//! meshes and shapes. Whatever crosses an edge is moved across to the other by its own
//! slice: a truck by `truck::RepeatingWorld`.

mod blend;
mod collider;
mod generator;
mod ground_boxes;
mod map;
mod mesh;
mod shading;
mod tile_material;

use std::path::{Path, PathBuf};

use avian3d::prelude::*;
use bevy::asset::io::embedded::EmbeddedAssetRegistry;
use bevy::camera::primitives::MeshAabb;
use bevy::camera::visibility::VisibilityRange;
use bevy::ecs::change_detection::Tick;
use bevy::pbr::PbrPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

use crate::game_state::GameState;

pub use collider::build_collider;
pub use content::track::{
    Backdrop, BoxFaces, Course, Diagonals, Footing, Gate, GroundBox, GroundCell, GroundTextures,
    HeightGrid, KeyframedFace, Keyframes, Nearest, Scenery, SceneryModel, SceneryMotion,
    SceneryObject, Skies, SkyPicture, StartPosition, TextureCycle, TrackData, direction_yaw,
    yaw_direction,
};
pub use generator::builtin_track;
pub use map::{MAP_COURSE, MAP_GATE, MAP_START, MapFrame, map_frame, map_image};
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
    /// Fades the textures of two ground cells into each other across the line between them
    /// (`blend`), where Monster Truck Madness 2 draws each cell's texture edge to edge with
    /// a hard line between. Costs a second texture read for the pixels of ground on the
    /// line, and no time on the CPU while racing. On by default,
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
    /// Lowers a fixed object on sloping ground until the lowest corner of its foot touches
    /// the ground. Monster Truck Madness 2's editor stands a model on its lowest point at
    /// the ground under its origin only, so one with a broad, flat foot on a slope floats at
    /// its downhill corners: Sidewinder Canyon's checkpoint pillars by up to 0.66 m. Loose
    /// and moving objects, and those put above the ground on purpose, stay where they are
    /// put. How far to lower each is worked out as the track is loaded (`settle`), so this
    /// costs nothing as a race begins. Read as each race begins.
    pub settle_scenery: bool,
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
            settle_scenery: true,
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
            // In `src/shaders`, with the game's other shaders: `embedded_asset!` takes only
            // a path below this file's folder.
            let embedded = app.world_mut().resource_mut::<EmbeddedAssetRegistry>();
            for (path, file, bytes) in [
                (
                    tile_material::SHADER_PATH,
                    "src/shaders/tiles.wgsl",
                    include_bytes!("../shaders/tiles.wgsl").as_slice(),
                ),
                (
                    tile_material::PREPASS_SHADER_PATH,
                    "src/shaders/tiles_prepass.wgsl",
                    include_bytes!("../shaders/tiles_prepass.wgsl").as_slice(),
                ),
            ] {
                embedded.insert_asset(
                    PathBuf::from(file),
                    Path::new(path.trim_start_matches("embedded://")),
                    bytes,
                );
            }
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
    let collider = build_collider(&track.heights);
    let terrain = commands
        .spawn((
            Name::new("Terrain"),
            DespawnOnExit(GameState::Racing),
            Transform::default(),
            Visibility::default(),
            collider.clone(),
            ground(crate::collision_groups::ground()),
        ))
        .id();
    let copies = track.heights.copies();
    // Whole, round a map that repeats: a truck is moved across only once its middle is
    // over the edge, and its wheels, or another truck, may be over it before then. The
    // copies share the original's heights.
    for &copy in &copies {
        commands.spawn((
            Name::new("Terrain copy"),
            ChildOf(terrain),
            Transform::from_xyz(copy.x, 0.0, copy.y),
            collider.clone(),
            ground(crate::collision_groups::ground()),
        ));
    }
    if let Some(collider) = ground_boxes::build_collider(&track.ground_boxes) {
        for offset in std::iter::once(Vec2::ZERO).chain(copies.iter().copied()) {
            commands.spawn((
                Name::new("Ground boxes"),
                ChildOf(terrain),
                Transform::from_xyz(offset.x, 0.0, offset.y),
                collider.clone(),
                // And their upright sides apart from the terrain's slopes.
                ground(crate::collision_groups::ground_boxes()),
            ));
        }
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
                repeats: track.heights.repeats() as u32,
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
                shading.fade = blend::FADE * shading.cells as f32 / track.heights.size();
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
                spawn_chunk(
                    &mut commands,
                    terrain,
                    &track,
                    chunk,
                    &mut meshes,
                    &material,
                );
            }
            // Lit by their own faces: the smooth surface through the ground's heights is
            // not theirs.
            let boxes = tile_materials.add(TileMaterial {
                base: surface,
                extension: TileTextures { tiles, ..default() },
            });
            let (textured, plain) = ground_boxes::build_meshes(&track.ground_boxes, true);
            for chunk in textured {
                spawn_chunk(&mut commands, terrain, &track, chunk, &mut meshes, &boxes);
            }
            spawn_plain_boxes(
                &mut commands,
                terrain,
                &track,
                plain,
                &mut meshes,
                &mut materials,
            );
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
                spawn_chunk(
                    &mut commands,
                    terrain,
                    &track,
                    chunk,
                    &mut meshes,
                    &material,
                );
            }
            let (_, plain) = ground_boxes::build_meshes(&track.ground_boxes, false);
            spawn_plain_boxes(
                &mut commands,
                terrain,
                &track,
                plain,
                &mut meshes,
                &mut materials,
            );
        }
        _ => {
            let material = materials.add(surface);
            for chunk in mesh::build_shaded_meshes(&track) {
                spawn_chunk(
                    &mut commands,
                    terrain,
                    &track,
                    chunk,
                    &mut meshes,
                    &material,
                );
            }
            let (_, plain) = ground_boxes::build_meshes(&track.ground_boxes, false);
            spawn_plain_boxes(
                &mut commands,
                terrain,
                &track,
                plain,
                &mut meshes,
                &mut materials,
            );
        }
    }
}

/// Colour of ground boxes drawn without textures: weathered concrete.
const PLAIN_BOX_COLOR: Color = Color::srgb(0.55, 0.53, 0.5);

fn spawn_plain_boxes(
    commands: &mut Commands,
    terrain: Entity,
    track: &TrackData,
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
        spawn_chunk(commands, terrain, track, chunk, meshes, &material);
    }
}

/// How far past the edges of a map that repeats its ground is drawn again, in metres: as
/// far as the camera sees, which is Bevy's default far plane, that the backdrop and the sky
/// are fitted inside. Less, and the ground beyond an edge ends in mid-air in plain view of
/// a truck near it; more draws nothing that can be seen.
pub const DRAWN_PAST_EDGE: f32 = 1000.0;

/// Spawns one chunk of what is drawn of the ground, with `material`. Where the ground
/// repeats, so do its copies round the map that come within `DRAWN_PAST_EDGE` of it. Bevy
/// culls only to the sides of the view, not by distance, so each copy is drawn only within
/// `DRAWN_PAST_EDGE` of the camera: from the middle of the map, none of them is.
fn spawn_chunk<M: Material>(
    commands: &mut Commands,
    terrain: Entity,
    track: &TrackData,
    chunk: Mesh,
    meshes: &mut Assets<Mesh>,
    material: &Handle<M>,
) {
    let bounds = chunk.compute_aabb();
    let mesh = meshes.add(chunk);
    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        ChildOf(terrain),
    ));
    let Some(bounds) = bounds else {
        return;
    };
    let middle = Vec3::from(bounds.center).xz();
    let half_across = Vec3::from(bounds.half_extents).xz().length();
    for copy in track.heights.copies() {
        if track.heights.beyond_edge(middle + copy) > DRAWN_PAST_EDGE + half_across {
            continue;
        }
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            ChildOf(terrain),
            Transform::from_xyz(copy.x, 0.0, copy.y),
            // Measured to the middle of the chunk, so that one partly within reach is drawn.
            VisibilityRange {
                start_margin: 0.0..0.0,
                end_margin: DRAWN_PAST_EDGE + half_across..DRAWN_PAST_EDGE + half_across,
                use_aabb: true,
            },
        ));
    }
}
