//! What a track shows round its horizon: distant hills, a skyline. The track names models
//! for it (`TrackData::backdrop`), and they are drawn round the camera wherever it goes,
//! so that they never come nearer, and behind everything else, unlit.
//!
//! **Reference** (JSTrackViewer, from the Traxx editor's source): the original game draws
//! the backdrop centred on the camera every frame, first, with no depth test, no lighting
//! and no fog. Here it is kept centred on the camera, and made as large as will fit inside
//! the camera's far plane, which looks the same, since from its middle a model looks the
//! same at any size: everything nearer is drawn over it. Only ground further off than
//! `FAR_SHARE` of the far plane would come in front of it. It takes the weather's fog, so
//! that rain and fog hide the hills as they hide everything far off, and it darkens with
//! the sun, so that a storm's hills are not in sunshine.
//!
//! Its textures are small for how much of the view they fill: each texel is ten or more
//! pixels across on screen, more on a large one, and blurred and stepped to match. Its
//! holes are given the colour of their edge (`fill_holes`), so that filtering draws no dark
//! fringe along a ridge, and the edge is antialiased (alpha to coverage). Smoothing the
//! outline of the holes on a texture made four times larger was tried: it cost up to 1.7 s
//! at the start of a race in a debug build, and barely showed, since the steps are in the
//! pictures themselves. `BackdropSettings::on` turns it off.
//!
//! Uses the `track` slice for the models and their material, the `camera` slice for where
//! it is, and the `environment` slice for the sun. Does nothing in an app that cannot draw.

use bevy::asset::RenderAssetUsages;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::PbrPlugin;
use bevy::prelude::*;

use crate::camera::{CameraSystems, ChaseCamera};
use crate::environment::{SUNLIGHT, Sun};
use crate::game_state::GameState;
use crate::track::{
    SceneryModel, TileMaterial, TileTextures, Track, TrackSettings, TrackSystems, tile_array,
};

/// How far out the furthest point of the backdrop is, as a share of the distance to the
/// camera's far plane. Near 1, so that as little as can be comes in front of it, and short
/// of it, so that none of it is cut off.
const FAR_SHARE: f32 = 0.9;
/// How many rings of pixels round the holes are given the colour of their edge, so that
/// filtering doesn't draw a dark fringe along a ridge. One texel's reach of the filter
/// needs one.
const EDGE_FILL: usize = 2;

/// How bright the backdrop is with the sun at its dimmest, as a share of how bright it is
/// in full sun. Unlit, it would otherwise shine in a storm.
const DARKEST: f32 = 0.35;

pub struct BackdropPlugin;

impl Plugin for BackdropPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BackdropSettings>();
        // Only a look. An app that cannot draw has no use for it.
        if !app.is_plugin_added::<PbrPlugin>() {
            return;
        }
        app.add_systems(
            OnEnter(GameState::Racing),
            spawn_backdrop.after(TrackSystems::Prepare),
        )
        .add_systems(
            Update,
            (
                follow_the_camera.after(CameraSystems::Place),
                follow_the_sun,
                show_or_hide,
            )
                .run_if(in_state(GameState::Racing)),
        );
    }
}

/// Choices about the backdrop. Change it at any time.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct BackdropSettings {
    /// Whether the backdrop is drawn. Off, the sky reaches down to the ground.
    pub on: bool,
}

impl Default for BackdropSettings {
    /// On, as in Monster Truck Madness 2.
    fn default() -> Self {
        Self { on: true }
    }
}

/// On the backdrop, which its models hang from.
#[derive(Component)]
pub struct Backdrop {
    /// How far out from its origin its furthest point is, in metres, as it was made.
    reach: f32,
    material: Handle<TileMaterial>,
}

fn spawn_backdrop(
    mut commands: Commands,
    track: Res<Track>,
    settings: Res<TrackSettings>,
    backdrop_settings: Res<BackdropSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<TileMaterial>>,
) {
    let Some(backdrop) = &track.backdrop else {
        return;
    };
    let settings_on = backdrop_settings.on;
    let reach = backdrop
        .models
        .iter()
        .flat_map(|model| &model.positions)
        .map(|&position| Vec3::from(position).length())
        .fold(0.0, f32::max);
    if reach <= 0.0 {
        return;
    }
    let material = materials.add(TileMaterial {
        base: StandardMaterial {
            unlit: true,
            // Seen from inside, and whichever way round a model's faces are wound.
            cull_mode: None,
            // A tile with holes in it says so with an alpha of 0, as for the scenery. The
            // edge is antialiased where the camera has MSAA, and cut at a half where not.
            alpha_mode: AlphaMode::AlphaToCoverage,
            ..default()
        },
        extension: TileTextures {
            tiles: images.add(tile_array(
                backdrop.tile_size,
                &backdrop
                    .tiles
                    .iter()
                    .map(|tile| fill_holes(tile, backdrop.tile_size))
                    .collect::<Vec<_>>(),
                // Never drawn smaller than it is, so its smaller copies would go unused.
                &TrackSettings {
                    mipmaps: false,
                    ..settings.clone()
                },
            )),
            ..default()
        },
    });
    commands
        .spawn((
            Backdrop {
                reach,
                material: material.clone(),
            },
            Name::new("Backdrop"),
            Transform::default(),
            visibility(settings_on),
            DespawnOnExit(GameState::Racing),
        ))
        .with_children(|parent| {
            for model in &backdrop.models {
                parent.spawn((
                    Name::new(model.name.clone()),
                    Mesh3d(meshes.add(build_mesh(model))),
                    MeshMaterial3d(material.clone()),
                    NotShadowCaster,
                    NotShadowReceiver,
                ));
            }
        });
}

type RaceCamera<'w, 's> = Query<
    'w,
    's,
    (&'static Transform, &'static Projection),
    (With<ChaseCamera>, Without<Backdrop>),
>;

/// Centred on the camera, and as large as fits inside its far plane.
fn follow_the_camera(cameras: RaceCamera, mut backdrops: Query<(&Backdrop, &mut Transform)>) {
    let Ok((eye, projection)) = cameras.single() else {
        return;
    };
    let far = match projection {
        Projection::Perspective(perspective) => perspective.far,
        _ => return,
    };
    for (backdrop, mut transform) in &mut backdrops {
        transform.translation = eye.translation;
        transform.scale = Vec3::splat(FAR_SHARE * far / backdrop.reach);
    }
}

/// As bright as the sun lets it be.
fn follow_the_sun(
    suns: Query<&DirectionalLight, With<Sun>>,
    backdrops: Query<&Backdrop>,
    mut materials: ResMut<Assets<TileMaterial>>,
) {
    let Some(sun) = suns.iter().next() else {
        return;
    };
    let shade = brightness(sun.illuminance);
    let color = Color::srgb(shade, shade, shade);
    for backdrop in &backdrops {
        // Only a real change, so that the material isn't sent again every frame.
        if let Some(material) = materials.get(&backdrop.material)
            && material.base.base_color != color
            && let Some(mut material) = materials.get_mut(&backdrop.material)
        {
            material.base.base_color = color;
        }
    }
}

fn visibility(on: bool) -> Visibility {
    if on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

/// Carries a change of `BackdropSettings::on` to the backdrop of the race.
fn show_or_hide(
    settings: Res<BackdropSettings>,
    mut backdrops: Query<&mut Visibility, With<Backdrop>>,
) {
    for mut shown in &mut backdrops {
        shown.set_if_neq(visibility(settings.on));
    }
}

/// How bright the backdrop is under a sun of `illuminance` lux, as a share of full sun.
fn brightness(illuminance: f32) -> f32 {
    let sun = (illuminance / SUNLIGHT).clamp(0.0, 1.0);
    DARKEST + (1.0 - DARKEST) * sun
}

/// The tile with each hole next to something solid given the mean colour of the solid
/// pixels round it, `EDGE_FILL` rings deep. The holes stay holes.
fn fill_holes(tile: &[u8], size: usize) -> Vec<u8> {
    let mut filled = tile.to_vec();
    let mut known: Vec<bool> = tile.as_chunks::<4>().0.iter().map(|p| p[3] > 0).collect();
    for _ in 0..EDGE_FILL {
        let mut next = filled.clone();
        let mut next_known = known.clone();
        for y in 0..size {
            for x in 0..size {
                if known[y * size + x] {
                    continue;
                }
                let (mut sum, mut count) = ([0u32; 3], 0);
                for (dx, dy) in [
                    (-1, 0),
                    (1, 0),
                    (0, -1),
                    (0, 1),
                    (-1, -1),
                    (1, 1),
                    (-1, 1),
                    (1, -1),
                ] {
                    let (nx, ny) = (x as isize + dx, y as isize + dy);
                    if nx < 0 || ny < 0 || nx >= size as isize || ny >= size as isize {
                        continue;
                    }
                    let near = ny as usize * size + nx as usize;
                    if known[near] {
                        for (i, total) in sum.iter_mut().enumerate() {
                            *total += filled[near * 4 + i] as u32;
                        }
                        count += 1;
                    }
                }
                if let Some(mean) = sum
                    .map(|total| total.checked_div(count))
                    .into_iter()
                    .collect::<Option<Vec<u32>>>()
                {
                    let at = (y * size + x) * 4;
                    for (i, value) in mean.into_iter().enumerate() {
                        next[at + i] = value as u8;
                    }
                    next_known[y * size + x] = true;
                }
            }
        }
        filled = next;
        known = next_known;
    }
    filled
}

fn build_mesh(model: &SceneryModel) -> Mesh {
    let tiles: Vec<[f32; 2]> = model.tiles.iter().map(|&tile| [tile as f32, 0.0]).collect();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, model.positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, model.normals.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, model.uvs.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, tiles)
    .with_inserted_indices(Indices::U32(model.indices.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tile of `size` pixels, solid where `solid` says, and red there.
    fn tile(size: usize, solid: impl Fn(usize, usize) -> bool) -> Vec<u8> {
        (0..size * size)
            .flat_map(|at| {
                if solid(at % size, at / size) {
                    [200, 0, 0, 255]
                } else {
                    [0, 0, 0, 0]
                }
            })
            .collect()
    }

    #[test]
    fn the_edge_of_a_hole_takes_the_colour_of_what_is_solid_beside_it() {
        let size = 8;
        let half = tile(size, |x, _| x >= 4);
        let filled = fill_holes(&half, size);
        // Next to the solid half: red, still a hole.
        let at = (2 * size + 3) * 4;
        assert_eq!(filled[at..at + 4], [200, 0, 0, 0]);
        // Further than `EDGE_FILL` from it: left black.
        let far = 2 * size * 4;
        assert_eq!(filled[far..far + 3], [0, 0, 0]);
        // What was solid is as it was.
        let solid = (2 * size + 5) * 4;
        assert_eq!(filled[solid..solid + 4], half[solid..solid + 4]);
    }

    #[test]
    fn the_backdrop_darkens_with_the_sun_but_never_goes_black() {
        assert_eq!(brightness(SUNLIGHT), 1.0);
        assert_eq!(brightness(SUNLIGHT * 3.0), 1.0);
        assert_eq!(brightness(0.0), DARKEST);
        assert!(brightness(SUNLIGHT * 0.5) < 1.0);
    }
}
