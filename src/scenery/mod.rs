//! The things standing around a track: trees, signs, guard rails, banners, parked cars and
//! trains. Draws them and makes the solid ones solid.
//!
//! Most stay where they are put, fixed to the world. A solid one with a mass is loose: it
//! stands asleep until something knocks it over or away, and then it is a body like any
//! other. A moving one goes on at its velocity whatever it meets, and pushes aside what is
//! in its way (see `motion`). What is drawn of either of those is not the body itself but a
//! copy placed between its last two physics poses (see `interpolate`).
//!
//! Some animate in place, as drawn only: textures that step through frames, and models
//! that move by keyframes (see `animation`). Flat pictures of trees turn to face the
//! camera (see `facing`).
//!
//! Uses the `track` slice for what there is and where (`TrackData::scenery`), for the
//! ground to stand it on, and for the tile material it is drawn with, and the `camera`
//! slice for the camera that pictures turn to face.

mod animation;
mod facing;
mod interpolate;
mod motion;

use bevy::asset::RenderAssetUsages;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::NoAutoAabb;
use bevy::mesh::morph::{MeshMorphWeights, MorphWeights};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::camera::CameraSystems;
use crate::game_state::GameState;
use crate::track::{
    SceneryModel, SceneryMotion, TileMaterial, TileTextures, Track, TrackSettings, TrackSystems,
    tile_array,
};

pub struct SceneryPlugin;

impl Plugin for SceneryPlugin {
    fn build(&self, app: &mut App) {
        // `TrackPlugin`, which this slice needs, has made sure of the state.
        app.add_systems(
            OnEnter(GameState::Racing),
            spawn_scenery.after(TrackSystems::Prepare),
        )
        .add_systems(
            FixedUpdate,
            (
                (motion::settle, motion::keep_moving).before(PhysicsSet::SyncBackend),
                (motion::remove_lost, interpolate::record_poses).after(PhysicsSet::Writeback),
            )
                .run_if(in_state(GameState::Racing)),
        )
        .add_systems(
            Update,
            (
                interpolate::place_visuals,
                animation::cycle_textures,
                animation::move_keyframes,
                facing::face_the_camera.after(CameraSystems::Place),
            )
                .run_if(in_state(GameState::Racing)),
        );
    }
}

/// Every entity of the scenery: the objects, and the drawn copies of those that move.
#[derive(Component)]
pub struct SceneryObject;

fn spawn_scenery(
    mut commands: Commands,
    track: Res<Track>,
    settings: Res<TrackSettings>,
    // All absent in a headless app, where scenery is only something to run into.
    meshes: Option<ResMut<Assets<Mesh>>>,
    images: Option<ResMut<Assets<Image>>>,
    materials: Option<ResMut<Assets<TileMaterial>>>,
) {
    let scenery = &track.scenery;
    if scenery.objects.is_empty() {
        return;
    }

    // One mesh per model and one material for the lot, however many objects there are.
    let looks = match (meshes, images, materials) {
        (Some(mut meshes), Some(mut images), Some(mut materials)) => {
            let material = materials.add(TileMaterial {
                base: StandardMaterial {
                    perceptual_roughness: 0.9,
                    // Tiles with holes in them say so with an alpha of 0, and solid tiles
                    // pass the test everywhere, so one material serves both.
                    alpha_mode: AlphaMode::Mask(0.5),
                    ..default()
                },
                extension: TileTextures {
                    tiles: images.add(tile_array(scenery.tile_size, &scenery.tiles, &settings)),
                    ..default()
                },
            });
            let meshes: Vec<Handle<Mesh>> = scenery
                .models
                .iter()
                .map(|model| meshes.add(build_mesh(model)))
                .collect();
            if !scenery.texture_cycles.is_empty() {
                commands.spawn((
                    animation::CyclingTiles {
                        material: material.clone(),
                    },
                    DespawnOnExit(GameState::Racing),
                ));
            }
            // What an object of a model that moves by keyframes is drawn with as well: the
            // model's morph weights, and a box round everywhere it goes.
            let morphs: Vec<Option<(MeshMorphWeights, Aabb)>> = scenery
                .models
                .iter()
                .zip(&meshes)
                .enumerate()
                .map(|(index, (model, mesh))| {
                    let keyframes = animation::drawn_keyframes(model);
                    if keyframes.is_none() && model.keyframes.is_some() {
                        warn!(
                            "{}: its keyframes can't be drawn, so it stands still",
                            model.name
                        );
                    }
                    let keyframes = keyframes?;
                    let weights = vec![0.0; keyframes.frames.len() - 1];
                    let weights = MorphWeights::new(weights, Some(mesh.clone())).ok()?;
                    let entity = commands
                        .spawn((
                            animation::KeyframedMesh { model: index },
                            weights,
                            DespawnOnExit(GameState::Racing),
                        ))
                        .id();
                    Some((
                        MeshMorphWeights::Reference(entity),
                        animation::bounds(keyframes)?,
                    ))
                })
                .collect();
            Some((meshes, material, morphs))
        }
        _ => None,
    };

    // A model's colliders are built once and shared by every object that uses it: the
    // triangles themselves for what doesn't move or is moved, and the hull round them for
    // a body that tumbles, since Rapier finds no contacts between two triangle meshes.
    // A model that moves by keyframes is solid as it stands in its first frame.
    let colliders: Vec<Option<Collider>> = scenery
        .models
        .iter()
        .map(|model| {
            let vertices = model.positions.iter().copied().map(Vec3::from).collect();
            let triangles = model.indices.as_chunks::<3>().0.to_vec();
            Collider::trimesh(vertices, triangles).ok()
        })
        .collect();
    let hulls: Vec<Option<Collider>> = scenery
        .models
        .iter()
        .map(|model| {
            let vertices: Vec<Vec3> = model.positions.iter().copied().map(Vec3::from).collect();
            Collider::convex_hull(&vertices)
        })
        .collect();

    for object in &scenery.objects {
        let ground = track
            .heights
            .height_at(object.position.x, object.position.y);
        let transform = Transform::from_xyz(
            object.position.x,
            ground + object.height_above_ground,
            object.position.y,
        )
        .with_rotation(Quat::from_rotation_y(object.yaw));
        let name = Name::new(scenery.models[object.model].name.clone());
        let drawn = looks.as_ref().map(|(meshes, material, _)| {
            (
                Mesh3d(meshes[object.model].clone()),
                MeshMaterial3d(material.clone()),
            )
        });
        let morph = looks
            .as_ref()
            .and_then(|(_, _, morphs)| morphs[object.model].clone())
            .map(|(weights, bounds)| (weights, bounds, NoAutoAabb));
        let collider = colliders[object.model].as_ref().filter(|_| object.solid);
        let hull = hulls[object.model].as_ref().filter(|_| object.solid);

        let body = match (object.motion, collider, hull) {
            (SceneryMotion::Loose { mass }, _, Some(hull)) => {
                motion::loose(&mut commands, transform, hull.clone(), mass)
            }
            (SceneryMotion::Moving { velocity }, Some(collider), _) => {
                motion::moving(&mut commands, transform, collider.clone(), velocity)
            }
            // A model with no volume to it, which can't tumble, stays put.
            (_, collider, _) => {
                let mut entity = commands.spawn((
                    SceneryObject,
                    DespawnOnExit(GameState::Racing),
                    name,
                    transform,
                ));
                if let Some(drawn) = drawn {
                    entity.insert(drawn);
                }
                if let Some(morph) = morph {
                    entity.insert(morph);
                }
                // A solid one keeps its yaw, so that its collider stays where it was put.
                if object.faces_camera && collider.is_none() {
                    entity.insert(facing::FacesCamera);
                }
                if let Some(collider) = collider {
                    entity.insert((RigidBody::Fixed, collider.clone(), bouncy()));
                }
                continue;
            }
        };
        commands.entity(body).insert(name.clone());
        if let Some(drawn) = drawn {
            let visual = interpolate::draw(&mut commands, body, transform, name, drawn);
            if let Some(morph) = morph {
                commands.entity(visual).insert(morph);
            }
        }
    }
}

/// A tire takes the softer of itself and what it hits (see `truck/spawn.rs`), so scenery
/// says it is at least as lively as the tire to be left alone: a truck springs off a post
/// or a rail as it always did, and only the ground, which names a duller one, damps it.
fn bouncy() -> Restitution {
    Restitution {
        coefficient: 1.0,
        combine_rule: CoefficientCombineRule::Min,
    }
}

/// The model's mesh, drawn as its first keyframe has it where it moves by keyframes, with
/// the others as its morph targets.
fn build_mesh(model: &SceneryModel) -> Mesh {
    // The tile, and the animated texture, counting from 1, or 0 for a still one.
    let tiles: Vec<[f32; 2]> = model
        .tiles
        .iter()
        .enumerate()
        .map(|(vertex, &tile)| {
            let cycle = model.texture_cycles.get(vertex).copied().unwrap_or(0);
            [tile as f32, cycle as f32]
        })
        .collect();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, model.positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, model.normals.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, model.uvs.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, tiles)
    .with_inserted_indices(Indices::U32(model.indices.clone()));
    if let Some(keyframes) = animation::drawn_keyframes(model) {
        mesh.set_morph_targets(animation::morph_targets(keyframes, &model.normals));
    }
    mesh
}
