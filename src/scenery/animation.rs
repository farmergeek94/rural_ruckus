//! Scenery that animates in place: animated textures, and models that move by keyframes.
//! Both run on the game clock, so every copy of a model moves in step. Drawn only, and
//! only in an app that can draw: what is solid is the first keyframe. How MTM2 showed
//! either is not measured: these rules are the game's own (`docs/formats/model.md`).
//!
//! An animated texture shows frame `floor(t / seconds per frame) mod frames`, with no
//! blending between frames. The frame each one is on is written into the scenery
//! material's `TileCycles`, and only when it changes.
//!
//! A model that moves by keyframes has each vertex moved in a straight line from where one
//! frame puts it to where the next does, round and round from the last frame to the
//! first. Its mesh is sent to the graphics card once, with every keyframe after the first
//! as a morph target: how far each vertex, and each face's normal, is from where the first
//! frame has it (`morph_targets`). Bevy's vertex shader blends them, so that all the CPU
//! sends each frame is a weight for each keyframe (`move_keyframes`). A face's normal is
//! blended between the normals it has in the two frames, which a face that turns is lit
//! by as it faces.

use bevy::camera::primitives::Aabb;
use bevy::mesh::morph::{MAX_MORPH_WEIGHTS, MorphAttributes, MorphWeights};
use bevy::prelude::*;

use crate::track::{Keyframes, SceneryModel, TileMaterial, Track};

/// The scenery's material, and the animated textures in it.
#[derive(Component)]
pub(super) struct CyclingTiles {
    pub(super) material: Handle<TileMaterial>,
}

/// The morph weights of a model that moves by keyframes, which every object that uses the
/// model is drawn with, on the same entity as them.
#[derive(Component)]
pub(super) struct KeyframedMesh {
    /// Index into `Scenery::models`.
    pub(super) model: usize,
}

/// The keyframes of `model`, if it moves by them and they can be drawn as morph targets:
/// Bevy takes no more than `MAX_MORPH_WEIGHTS`, one for each keyframe after the first, and
/// the first must be the mesh.
pub(super) fn drawn_keyframes(model: &SceneryModel) -> Option<&Keyframes> {
    model.keyframes.as_ref().filter(|keyframes| {
        fits_morph_targets(keyframes) && keyframes.frames[0].len() == model.positions.len()
    })
}

fn fits_morph_targets(keyframes: &Keyframes) -> bool {
    (2..=MAX_MORPH_WEIGHTS + 1).contains(&keyframes.frames.len())
}

/// Every keyframe after the first as a morph target of a mesh drawn in the first, whose
/// normals are `normals`: how far each vertex, and its normal, is from there. Target `i` is
/// keyframe `i + 1`. A keyframe short of vertices leaves the rest where the first has them.
pub(super) fn morph_targets(keyframes: &Keyframes, normals: &[[f32; 3]]) -> Vec<MorphAttributes> {
    let first = &keyframes.frames[0];
    keyframes.frames[1..]
        .iter()
        .flat_map(|frame| {
            let frame_normals = normals_of(keyframes, frame);
            (0..first.len()).map(move |vertex| {
                let position = frame
                    .get(vertex)
                    .map_or(Vec3::ZERO, |&at| Vec3::from(at) - Vec3::from(first[vertex]));
                let normal = match (frame_normals.get(vertex), normals.get(vertex)) {
                    (Some(&to), Some(&from)) => Vec3::from(to) - Vec3::from(from),
                    _ => Vec3::ZERO,
                };
                MorphAttributes::new(position, normal, Vec3::ZERO)
            })
        })
        .collect()
}

/// A box round every place the keyframes put the model, since the mesh itself only says
/// where the first frame has it.
pub(super) fn bounds(keyframes: &Keyframes) -> Option<Aabb> {
    let mut points = keyframes.frames.iter().flatten().map(|&at| Vec3::from(at));
    let first = points.next()?;
    let (least, most) = points.fold((first, first), |(least, most), at| {
        (least.min(at), most.max(at))
    });
    Some(Aabb::from_min_max(least, most))
}

/// Which frame an animation of `frames` frames, each `seconds_per_frame` long, is on at
/// `seconds` on the clock.
fn frame_at(seconds: f64, seconds_per_frame: f32, frames: u32) -> u32 {
    if frames == 0 || seconds_per_frame.is_nan() || seconds_per_frame <= 0.0 {
        return 0;
    }
    ((seconds / seconds_per_frame as f64)
        .floor()
        .rem_euclid(frames as f64)) as u32
}

pub(super) fn cycle_textures(
    time: Res<Time>,
    track: Res<Track>,
    tiles: Query<&CyclingTiles>,
    materials: Option<ResMut<Assets<TileMaterial>>>,
) {
    let Some(mut materials) = materials else {
        return;
    };
    let seconds = time.elapsed_secs_f64();
    for tiles in &tiles {
        let Some(material) = materials.get(&tiles.material) else {
            continue;
        };
        let mut cycles = material.extension.cycles;
        for (index, cycle) in track.scenery.texture_cycles.iter().enumerate() {
            cycles.set(
                index + 1,
                frame_at(seconds, cycle.seconds_per_frame, cycle.frames),
            );
        }
        // Borrowed mutably only when a frame changes, since that sends the material to the
        // graphics card again.
        if cycles != material.extension.cycles
            && let Some(mut material) = materials.get_mut(&tiles.material)
        {
            material.extension.cycles = cycles;
        }
    }
}

pub(super) fn move_keyframes(
    time: Res<Time>,
    track: Res<Track>,
    mut keyframed: Query<(&KeyframedMesh, &mut MorphWeights)>,
) {
    let seconds = time.elapsed_secs_f64();
    for (keyframed, mut weights) in &mut keyframed {
        let Some(model) = track.scenery.models.get(keyframed.model) else {
            continue;
        };
        let Some(keyframes) = &model.keyframes else {
            continue;
        };
        weights_at(keyframes, seconds, weights.weights_mut());
    }
}

/// The two frames to blend at `seconds` on the clock, and how far it is from the first to
/// the second, from 0 to 1: frame `i` and the one after it, round and round.
fn blend_at(seconds: f64, seconds_per_frame: f32, frames: usize) -> (usize, usize, f32) {
    let along = (seconds / seconds_per_frame as f64).rem_euclid(frames as f64);
    let frame = (along.floor() as usize).min(frames - 1);
    let fraction = (along - along.floor()) as f32;
    (frame, (frame + 1) % frames, fraction)
}

/// The weight of each morph target (`morph_targets`) at `seconds` on the clock: the share of
/// the keyframe blended from and of the one blended to. The first keyframe is the mesh
/// itself, and has no target.
fn weights_at(keyframes: &Keyframes, seconds: f64, weights: &mut [f32]) {
    let (from, to, fraction) =
        blend_at(seconds, keyframes.seconds_per_frame, keyframes.frames.len());
    weights.fill(0.0);
    for (frame, share) in [(from, 1.0 - fraction), (to, fraction)] {
        if let Some(weight) = frame
            .checked_sub(1)
            .and_then(|target| weights.get_mut(target))
        {
            *weight += share;
        }
    }
}

/// The normals of the mesh with its vertices at `positions`: each face's from where its
/// corners are, as `track/pod_scenery.rs` works out the first frame's.
fn normals_of(keyframes: &Keyframes, positions: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0; 3]; positions.len()];
    for face in &keyframes.faces {
        let corners = face.first as usize..(face.first + face.corners) as usize;
        let Some(points) = positions.get(corners.clone()) else {
            continue;
        };
        // Newell's method, as for the first frame.
        let normal = (0..points.len())
            .map(|index| {
                Vec3::from(points[index]).cross(Vec3::from(points[(index + 1) % points.len()]))
            })
            .sum::<Vec3>()
            .normalize_or_zero()
            .to_array();
        normals[corners].fill(normal);
    }
    normals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::KeyframedFace;

    #[test]
    fn a_texture_shows_each_frame_for_its_time_and_goes_round() {
        let frame = |seconds| frame_at(seconds, 2.0 / 3.0, 2);
        assert_eq!(frame(0.0), 0);
        assert_eq!(frame(0.6), 0);
        assert_eq!(frame(0.7), 1);
        assert_eq!(frame(1.4), 0);
        assert_eq!(frame(1000.0 * 4.0 / 3.0 + 0.1), 0);
        assert_eq!(frame_at(0.5, 0.0, 4), 0);
        assert_eq!(frame_at(0.5, 1.0, 0), 0);
    }

    #[test]
    fn the_blend_is_between_a_frame_and_the_next_and_goes_round() {
        assert_eq!(blend_at(0.0, 1.25, 4), (0, 1, 0.0));
        assert_eq!(blend_at(2.5, 1.25, 4), (2, 3, 0.0));
        assert_eq!(blend_at(3.5, 1.0, 4), (3, 0, 0.5));
        // And the clock may be read before it starts.
        assert_eq!(blend_at(-0.25, 1.0, 4), (3, 0, 0.75));
    }

    /// A triangle that turns from lying flat to standing up and back.
    fn hinge() -> Keyframes {
        let flat = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]];
        let up = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        Keyframes {
            seconds_per_frame: 1.0,
            frames: vec![flat, up],
            faces: vec![KeyframedFace {
                first: 0,
                corners: 3,
            }],
        }
    }

    /// Three keyframes of a triangle, whose third corner goes up and then over.
    fn three_frames() -> Keyframes {
        let corner = |at: [f32; 3]| vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], at];
        Keyframes {
            seconds_per_frame: 1.0,
            frames: vec![
                corner([0.0, 0.0, -1.0]),
                corner([0.0, 1.0, 0.0]),
                corner([0.0, 0.0, 1.0]),
            ],
            faces: vec![KeyframedFace {
                first: 0,
                corners: 3,
            }],
        }
    }

    /// What the vertex shader makes of the mesh drawn in the first keyframe, with `normals`,
    /// and its morph targets at `seconds`: the positions and the normals.
    fn blended(keyframes: &Keyframes, seconds: f64) -> (Vec<Vec3>, Vec<Vec3>) {
        let normals = normals_of(keyframes, &keyframes.frames[0]);
        let targets = morph_targets(keyframes, &normals);
        let vertices = keyframes.frames[0].len();
        let mut weights = vec![0.0; keyframes.frames.len() - 1];
        weights_at(keyframes, seconds, &mut weights);
        (0..vertices)
            .map(|vertex| {
                weights.iter().enumerate().fold(
                    (
                        Vec3::from(keyframes.frames[0][vertex]),
                        Vec3::from(normals[vertex]),
                    ),
                    |(at, normal), (target, weight)| {
                        let morph = targets[target * vertices + vertex];
                        (at + morph.position * weight, normal + morph.normal * weight)
                    },
                )
            })
            .unzip()
    }

    #[test]
    fn a_vertex_goes_straight_from_where_one_frame_puts_it_to_the_next() {
        for keyframes in [hinge(), three_frames()] {
            let frames = keyframes.frames.len();
            for (seconds, from, to, fraction) in [
                (0.0, 0, 1, 0.0),
                (1.0, 1, 2 % frames, 0.0),
                (0.25, 0, 1, 0.25),
                (frames as f64 - 0.5, frames - 1, 0, 0.5),
                (frames as f64, 0, 1, 0.0),
            ] {
                let (positions, _) = blended(&keyframes, seconds);
                for (vertex, at) in positions.iter().enumerate() {
                    let wanted = Vec3::from(keyframes.frames[from][vertex])
                        .lerp(Vec3::from(keyframes.frames[to][vertex]), fraction);
                    assert!(at.distance(wanted) < 1e-6, "{seconds} s: {at} {wanted}");
                }
            }
        }
    }

    #[test]
    fn a_face_turns_its_normal_as_it_moves() {
        let keyframes = hinge();
        let normal = |seconds| blended(&keyframes, seconds).1[0].normalize();
        assert!(normal(0.0).distance(Vec3::Y) < 1e-6);
        assert!(normal(1.0).distance(Vec3::Z) < 1e-6);
        // Halfway, it faces halfway: as the face does, whose corner is halfway up.
        assert!(normal(0.5).distance(Vec3::new(0.0, 1.0, 1.0).normalize()) < 1e-6);
    }

    #[test]
    fn the_bounds_hold_every_keyframe() {
        let bounds = bounds(&three_frames()).unwrap();
        assert_eq!(Vec3::from(bounds.min()), Vec3::new(0.0, 0.0, -1.0));
        assert_eq!(Vec3::from(bounds.max()), Vec3::new(1.0, 1.0, 1.0));
    }

    #[test]
    fn a_model_has_a_morph_target_for_each_keyframe_after_the_first() {
        let keyframes = three_frames();
        assert!(fits_morph_targets(&keyframes));
        let normals = normals_of(&keyframes, &keyframes.frames[0]);
        assert_eq!(morph_targets(&keyframes, &normals).len(), 2 * 3);
        let too_many = Keyframes {
            frames: vec![keyframes.frames[0].clone(); MAX_MORPH_WEIGHTS + 2],
            ..keyframes
        };
        assert!(!fits_morph_targets(&too_many));
    }
}
