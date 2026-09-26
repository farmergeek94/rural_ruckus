//! Meshes, textures and materials for a truck's models. Rendering only: nothing here
//! affects how the truck drives, and a headless app never gets this far.
//!
//! A truck has a couple of dozen textures of different sizes, so each is an ordinary
//! texture with a material of its own and a model is one mesh per texture. (The ground
//! and the scenery, with hundreds of tiles all one size, use an array texture instead.)
//!
//! An animated texture is a mesh with a material of its own, whose texture is switched
//! from frame to frame on the game clock (`CyclingTexture`).

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::{
    AxleLink, AxleLinks, NormalMap, TruckLooks, TruckLooksSettings, TruckMesh, TruckModel,
    TruckTexture,
};
use crate::mipmaps::{data_mip_chain, mip_chain};

/// How rough the truck's surfaces are, from 0 (a mirror) to 1. Paint and rubber alike, as
/// the models don't say which is which. Lower is shinier.
const ROUGHNESS: f32 = 0.6;

/// How much sharper a texture may be drawn along the direction it recedes in than a plain
/// mipmap allows. A truck's sides are mostly seen at a slant from the chase camera.
const ANISOTROPY: u16 = 16;

/// One mesh of a model and the material it is drawn with.
pub(super) type Part = (Handle<Mesh>, Handle<StandardMaterial>);

/// A truck's models, ready to draw. Each is `None` if the truck hasn't got it.
#[derive(Default)]
pub(super) struct Models {
    pub(super) body: Option<Vec<Part>>,
    pub(super) left_tire: Option<Vec<Part>>,
    pub(super) right_tire: Option<Vec<Part>>,
    /// Front left, front right, rear left, rear right: see `TruckLooks::wheel_tires`.
    pub(super) wheel_tires: [Option<Vec<Part>>; 4],
    pub(super) axle: Option<Vec<Part>>,
    /// The axle bars, shocks and driveshaft: one mesh and material for each kind, and
    /// where each of that kind goes.
    pub(super) axle_links: Vec<(Part, Vec<AxleLink>)>,
    /// The materials with animated textures, to be spawned with the truck.
    pub(super) cycles: Vec<CyclingTexture>,
}

/// A material whose texture steps through frames, each shown for the same time, round
/// and round. Every truck's are on the one game clock.
#[derive(Component, Clone)]
pub(super) struct CyclingTexture {
    material: Handle<StandardMaterial>,
    frames: Vec<Handle<Image>>,
    /// In seconds.
    seconds_per_frame: f32,
    /// The frame the material shows now.
    showing: usize,
}

impl Models {
    pub(super) fn build(
        looks: &TruckLooks,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
        settings: &TruckLooksSettings,
    ) -> Self {
        let textures: Vec<Handle<Image>> = looks
            .textures
            .iter()
            .map(|texture| images.add(texture_image(texture)))
            .collect();
        let normal_maps: Vec<Option<Handle<Image>>> = looks
            .textures
            .iter()
            .map(|texture| {
                let normal_map = texture.normal_map.as_ref()?;
                Some(images.add(normal_map_image(normal_map)))
            })
            .collect();
        let mut cycles = Vec::new();
        let mut build = |model: &Option<TruckModel>| {
            let model = model.as_ref()?;
            Some(
                model
                    .parts
                    .iter()
                    .map(|part| {
                        let [red, green, blue] = part.color;
                        let opacity = part.opacity.unwrap_or(1.0);
                        let normal_map = part.texture.and_then(|index| normal_maps[index].clone());
                        let material = StandardMaterial {
                            base_color: Color::srgba_u8(red, green, blue, 255).with_alpha(opacity),
                            base_color_texture: part.texture.map(|index| textures[index].clone()),
                            perceptual_roughness: ROUGHNESS,
                            reflectance: settings.reflectance,
                            // A texture with holes in it says so with an alpha of 0. Glass is
                            // as see-through as its material and its texture's alpha say.
                            alpha_mode: if part.cutout {
                                AlphaMode::Mask(0.5)
                            } else if part.opacity.is_some() {
                                AlphaMode::Blend
                            } else {
                                AlphaMode::Opaque
                            },
                            // Community Patch 3's normal maps are made for DirectX, whose
                            // green runs down the texture; Bevy's runs up it.
                            flip_normal_map_y: normal_map.is_some(),
                            normal_map_texture: normal_map.clone(),
                            ..default()
                        };
                        let mut mesh = build_mesh(part);
                        // A normal map is laid on the surface by the mesh's tangents.
                        if normal_map.is_some() && mesh.generate_tangents().is_err() {
                            warn!("a truck mesh has no tangents for its normal map");
                        }
                        let material = materials.add(material);
                        if let Some(cycle) = &part.texture_cycle {
                            cycles.push(CyclingTexture {
                                material: material.clone(),
                                frames: cycle
                                    .frames
                                    .iter()
                                    .map(|&index| textures[index].clone())
                                    .collect(),
                                seconds_per_frame: cycle.seconds_per_frame,
                                showing: 0,
                            });
                        }
                        (meshes.add(mesh), material)
                    })
                    .collect(),
            )
        };
        let (body, left_tire, right_tire) = (
            build(&looks.body),
            build(&looks.left_tire),
            build(&looks.right_tire),
        );
        let wheel_tires = looks.wheel_tires.each_ref().map(&mut build);
        let axle = build(&looks.axle);
        Self {
            body,
            left_tire,
            right_tire,
            wheel_tires,
            axle,
            cycles,
            axle_links: [&looks.axle_bars, &looks.shocks, &looks.driveshaft]
                .into_iter()
                .filter(|kind| !kind.links.is_empty())
                .map(|kind: &AxleLinks| {
                    let [red, green, blue] = kind.color;
                    let material = StandardMaterial {
                        base_color: match kind.texture {
                            Some(_) => Color::WHITE,
                            None => Color::srgb_u8(red, green, blue),
                        },
                        base_color_texture: kind.texture.map(|index| textures[index].clone()),
                        perceptual_roughness: ROUGHNESS,
                        reflectance: settings.reflectance,
                        ..default()
                    };
                    let mesh = meshes.add(Cylinder::new(kind.radius, 1.0));
                    ((mesh, materials.add(material)), kind.links.clone())
                })
                .collect(),
        }
    }
}

/// On every mesh of a truck, so that a change of `TruckLooksSettings` can find its material.
#[derive(Component)]
pub(super) struct Paintwork;

pub(super) fn apply_settings(
    settings: Res<TruckLooksSettings>,
    parts: Query<&MeshMaterial3d<StandardMaterial>, With<Paintwork>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let Some(mut materials) = materials else {
        return;
    };
    for material in &parts {
        if let Some(mut material) = materials.get_mut(&material.0) {
            material.reflectance = settings.reflectance;
        }
    }
}

/// Which frame an animation of `frames` frames, each `seconds_per_frame` long, is on at
/// `seconds` on the clock. The same rule as the scenery's (`scenery/animation.rs`).
fn frame_at(seconds: f64, seconds_per_frame: f32, frames: usize) -> usize {
    if frames == 0 || seconds_per_frame.is_nan() || seconds_per_frame <= 0.0 {
        return 0;
    }
    (seconds / seconds_per_frame as f64)
        .floor()
        .rem_euclid(frames as f64) as usize
}

/// Shows each animated texture's frame for the time on the game clock.
pub(super) fn cycle_textures(
    time: Res<Time>,
    mut cycles: Query<&mut CyclingTexture>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let Some(mut materials) = materials else {
        return;
    };
    let seconds = time.elapsed_secs_f64();
    for mut cycle in &mut cycles {
        let frame = frame_at(seconds, cycle.seconds_per_frame, cycle.frames.len());
        if frame == cycle.showing {
            continue;
        }
        if let Some(mut material) = materials.get_mut(&cycle.material) {
            material.base_color_texture = Some(cycle.frames[frame].clone());
        }
        cycle.showing = frame;
    }
}

fn build_mesh(part: &TruckMesh) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, part.positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, part.normals.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, part.uvs.clone())
    .with_inserted_indices(Indices::U32(part.indices.clone()))
}

/// A normal map with its chain of mipmaps. Its numbers are directions, so they are
/// neither sRGB nor averaged as light.
fn normal_map_image(normal_map: &NormalMap) -> Image {
    image_with_mipmaps(
        data_mip_chain(&normal_map.rgba, normal_map.size),
        normal_map.size,
        TextureFormat::Rgba8Unorm,
    )
}

/// A texture with its chain of mipmaps, made here because Bevy can't make them for an
/// sRGB texture. See `mipmaps.rs`.
pub(super) fn texture_image(texture: &TruckTexture) -> Image {
    // The palette's colours are as they appear on screen, not linear light.
    image_with_mipmaps(
        mip_chain(&texture.rgba, texture.size),
        texture.size,
        TextureFormat::Rgba8UnormSrgb,
    )
}

fn image_with_mipmaps(chain: Vec<Vec<u8>>, size: usize, format: TextureFormat) -> Image {
    let levels = chain.len() as u32;
    let mut image = Image {
        data: Some(chain.into_iter().flatten().collect()),
        asset_usage: RenderAssetUsages::default(),
        sampler: ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            // Anisotropic filtering works by reading between mipmap levels.
            anisotropy_clamp: if levels > 1 { ANISOTROPY } else { 1 },
            ..default()
        }),
        ..default()
    };
    image.texture_descriptor.size = Extent3d {
        width: size as u32,
        height: size as u32,
        depth_or_array_layers: 1,
    };
    image.texture_descriptor.dimension = TextureDimension::D2;
    image.texture_descriptor.mip_level_count = levels;
    image.texture_descriptor.format = format;
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_animated_texture_shows_each_frame_for_its_time_and_goes_round() {
        let frame = |seconds| frame_at(seconds, 0.25, 3);
        assert_eq!(
            [0.0, 0.2, 0.25, 0.6, 0.75, 1.0].map(frame),
            [0, 0, 1, 2, 0, 1]
        );
        assert_eq!(frame_at(1.0, 0.0, 3), 0);
        assert_eq!(frame_at(1.0, 0.5, 0), 0);
    }

    #[test]
    fn a_texture_carries_its_whole_chain_of_mipmaps() {
        let image = texture_image(&TruckTexture {
            size: 4,
            rgba: vec![90; 4 * 4 * 4],
            normal_map: None,
        });
        assert_eq!(image.texture_descriptor.mip_level_count, 3);
        assert_eq!(image.data.unwrap().len(), (16 + 4 + 1) * 4);
    }
}
