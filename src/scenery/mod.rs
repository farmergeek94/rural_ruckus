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
//! A model whose tiles have no holes is drawn opaque, and only one with holes with an
//! alpha mask, which costs more to draw and to shadow (see `docs/smoothness.md`).
//!
//! Uses the `track` slice for what there is and where (`TrackData::scenery`), for the
//! ground to stand it on, and for the tile material it is drawn with, and the `camera`
//! slice for the camera that pictures turn to face.

mod animation;
mod facing;
mod interpolate;
mod motion;

use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::{NoAutoAabb, VisibilityRange};
use bevy::mesh::morph::{MeshMorphWeights, MorphWeights};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::camera::CameraSystems;
use crate::game_state::GameState;
use crate::track::{
    SceneryModel, SceneryMotion, TextureCycle, TileMaterial, TileTextures, Track, TrackSettings,
    TrackSystems, tile_array,
};

pub struct SceneryPlugin;

impl Plugin for SceneryPlugin {
    fn build(&self, app: &mut App) {
        // `TrackPlugin`, which this slice needs, has made sure of the state.
        app.add_systems(
            OnEnter(GameState::Racing),
            spawn_scenery.after(TrackSystems::Prepare),
        )
        // Before the physics step, which is in `FixedPostUpdate`.
        .add_systems(
            FixedUpdate,
            (motion::settle, motion::keep_moving).run_if(in_state(GameState::Racing)),
        )
        .add_systems(
            FixedPostUpdate,
            (motion::remove_lost, interpolate::record_poses)
                .after(PhysicsSystems::Writeback)
                .run_if(in_state(GameState::Racing)),
        )
        .add_systems(
            Update,
            (
                interpolate::place_visuals,
                animation::cycle_textures,
                animation::move_keyframes,
                animation::refresh_morphs_in_view,
                facing::face_the_camera.after(CameraSystems::Place),
                keep_draw_distance,
                show_decorations,
            )
                .run_if(in_state(GameState::Racing)),
        );
    }
}

/// Every entity of the scenery: the objects, and the drawn copies of those that move.
#[derive(Component)]
pub struct SceneryObject;

/// On a drawn object that trucks go through: a bush, a tree or a sign that isn't solid.
/// Hiding one changes nothing but the look (`TrackSettings::decorations`).
#[derive(Component)]
struct Decoration;

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

    // One mesh per model and one texture array for the lot, however many objects there
    // are, with a material on it for the models whose tiles have holes and another for
    // the rest: an alpha mask costs (see `docs/smoothness.md`), so only what needs one
    // wears one.
    let looks = match (meshes, images, materials) {
        (Some(mut meshes), Some(mut images), Some(mut materials)) => {
            let tiles = images.add(tile_array(scenery.tile_size, &scenery.tiles, &settings));
            let holes = tiles_with_holes(&scenery.tiles);
            let (mut solid, mut cut_out) = (None, None);
            let material_of_model: Vec<Handle<TileMaterial>> = scenery
                .models
                .iter()
                .map(|model| {
                    let (material, alpha_mode) =
                        if shows_a_hole(model, &scenery.texture_cycles, &holes) {
                            (&mut cut_out, AlphaMode::Mask(MASK_CUTOFF))
                        } else {
                            (&mut solid, AlphaMode::Opaque)
                        };
                    material
                        .get_or_insert_with(|| {
                            materials.add(TileMaterial {
                                base: StandardMaterial {
                                    perceptual_roughness: 0.9,
                                    alpha_mode,
                                    ..default()
                                },
                                extension: TileTextures {
                                    tiles: tiles.clone(),
                                    ..default()
                                },
                            })
                        })
                        .clone()
                })
                .collect();
            let meshes: Vec<Handle<Mesh>> = scenery
                .models
                .iter()
                .map(|model| meshes.add(build_mesh(model)))
                .collect();
            if !scenery.texture_cycles.is_empty() {
                // Each material shows the animated textures' frames for itself.
                for material in [solid, cut_out].into_iter().flatten() {
                    commands.spawn((
                        animation::CyclingTiles { material },
                        DespawnOnExit(GameState::Racing),
                    ));
                }
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
            Some((meshes, material_of_model, morphs))
        }
        _ => None,
    };

    // A model's colliders are built once and shared by every object that uses it: the
    // triangles themselves for what doesn't move or is moved, and the hull round them for
    // a body that tumbles, since no contacts are found between two triangle meshes.
    // A model that moves by keyframes is solid as it stands in its first frame.
    let colliders: Vec<Option<Collider>> = scenery
        .models
        .iter()
        .map(|model| {
            let vertices = model.positions.iter().copied().map(Vec3::from).collect();
            let triangles = model.indices.as_chunks::<3>().0.to_vec();
            Collider::try_trimesh(vertices, triangles).ok()
        })
        .collect();
    let hulls: Vec<Option<Collider>> = scenery
        .models
        .iter()
        .map(|model| {
            let vertices: Vec<Vec3> = model.positions.iter().copied().map(Vec3::from).collect();
            Collider::convex_hull(vertices)
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
        // An invisible object, such as a ramp given by its size alone, is solid but unseen.
        let looks_of_object = looks.as_ref().filter(|_| object.visible);
        let drawn = looks_of_object.map(|(meshes, materials, _)| {
            (
                Mesh3d(meshes[object.model].clone()),
                MeshMaterial3d(materials[object.model].clone()),
            )
        });
        let morph = looks_of_object
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
                if collider.is_none() && looks_of_object.is_some() {
                    entity.insert(Decoration);
                }
                if let Some(collider) = collider {
                    entity.insert((RigidBody::Static, collider.clone(), bouncy()));
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

/// Gives what is drawn of the scenery the draw distance of `TrackSettings::scenery_distance`:
/// all of it when that changes, and what is new each frame. Bevy then skips an object
/// further than that from the camera as it skips one outside the view.
/// Shows or hides the decorations as `TrackSettings::decorations` says: all of them when
/// that changes, and each new one.
fn show_decorations(
    settings: Res<TrackSettings>,
    mut decorations: Query<(Ref<Decoration>, &mut Visibility)>,
) {
    let shown = if settings.decorations {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for (decoration, mut visibility) in &mut decorations {
        if settings.is_changed() || decoration.is_added() {
            visibility.set_if_neq(shown);
        }
    }
}

fn keep_draw_distance(
    mut commands: Commands,
    settings: Res<TrackSettings>,
    all: Query<Entity, (With<SceneryObject>, With<Mesh3d>)>,
    new: Query<Entity, (With<SceneryObject>, Added<Mesh3d>)>,
) {
    let objects = if settings.is_changed() {
        all.iter().collect::<Vec<_>>()
    } else {
        new.iter().collect()
    };
    let distance = settings.scenery_distance;
    for object in objects {
        if distance.is_finite() {
            // Gone at once at the distance, rather than dithered out: a fade draws the
            // object in both of its states while it lasts.
            commands
                .entity(object)
                .insert(VisibilityRange::abrupt(0.0, distance));
        } else {
            commands.entity(object).remove::<VisibilityRange>();
        }
    }
}

/// The alpha under which a texel of a tile is a hole, from 0 to 1: the cutoff of the alpha
/// mask that the models with holes are drawn with.
const MASK_CUTOFF: f32 = 0.5;

/// Whether a texel of alpha `alpha` (0 to 255) is a hole: one the alpha mask leaves out.
fn is_hole(alpha: u8) -> bool {
    (alpha as f32) / 255.0 < MASK_CUTOFF
}

/// Which of the scenery's tiles have a hole. A tile with none draws exactly as it did
/// under the mask whatever the sampling does: filtering and mipmaps only mix texels, and
/// a mix of alphas at or over the cutoff stays at or over it.
fn tiles_with_holes(tiles: &[Vec<u8>]) -> Vec<bool> {
    tiles
        .iter()
        .map(|tile| {
            tile.as_chunks::<4>()
                .0
                .iter()
                .any(|texel| is_hole(texel[3]))
        })
        .collect()
}

/// Whether `model` shows a tile with a hole: any tile a vertex names, and for an animated
/// texture any of its frames. A tile or a texture the scenery hasn't got shows nothing.
fn shows_a_hole(model: &SceneryModel, cycles: &[TextureCycle], holes: &[bool]) -> bool {
    let has_hole = |tile: u32| holes.get(tile as usize).copied().unwrap_or(false);
    model.tiles.iter().enumerate().any(|(vertex, &tile)| {
        let frames = model
            .texture_cycles
            .get(vertex)
            .and_then(|&cycle| cycles.get(cycle.checked_sub(1)? as usize))
            .map_or(0..0, |cycle| {
                cycle.first_tile..cycle.first_tile + cycle.frames
            });
        has_hole(tile) || frames.into_iter().any(has_hole)
    })
}

/// A tire takes the softer of itself and what it hits (see `truck/spawn.rs`), so scenery
/// says it is at least as lively as the tire to be left alone: a truck springs off a post
/// or a rail as it always did, and only the ground, which names a duller one, damps it.
fn bouncy() -> Restitution {
    Restitution::new(1.0).with_combine_rule(CoefficientCombine::Min)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A tile of white texels with these alphas.
    fn tile(alphas: &[u8]) -> Vec<u8> {
        alphas
            .iter()
            .flat_map(|&alpha| [255, 255, 255, alpha])
            .collect()
    }

    #[test]
    fn a_texel_under_the_masks_cutoff_is_a_hole() {
        assert!(is_hole(0));
        assert!(is_hole(127));
        assert!(!is_hole(128));
        assert!(!is_hole(255));
        let holes = tiles_with_holes(&[tile(&[255, 255]), tile(&[255, 127]), tile(&[128, 200])]);
        assert_eq!(holes, [false, true, false]);
    }

    #[test]
    fn a_model_shows_a_hole_through_any_frame_of_an_animated_texture() {
        // Tile 2 has a hole, and is the second frame of the one animated texture.
        let holes = [false, false, true];
        let cycles = [TextureCycle {
            first_tile: 1,
            frames: 2,
            seconds_per_frame: 1.0,
        }];
        let model = |tiles: Vec<u32>, texture_cycles: Vec<u32>| SceneryModel {
            tiles,
            texture_cycles,
            ..default()
        };
        assert!(!shows_a_hole(
            &model(vec![0, 0, 0], vec![]),
            &cycles,
            &holes
        ));
        assert!(shows_a_hole(&model(vec![2, 0, 0], vec![]), &cycles, &holes));
        // The animated texture's first frame is solid, but its second is not.
        assert!(!shows_a_hole(
            &model(vec![1, 1, 1], vec![]),
            &cycles,
            &holes
        ));
        assert!(shows_a_hole(
            &model(vec![1, 1, 1], vec![1, 1, 1]),
            &cycles,
            &holes
        ));
        // A tile or an animated texture the scenery hasn't got shows nothing.
        assert!(!shows_a_hole(&model(vec![9], vec![7]), &cycles, &holes));
    }
}
