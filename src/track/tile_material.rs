//! The material for things covered in small square textures, which is both the terrain
//! and the scenery, and the texture array it reads them from.
//!
//! Every tile is a layer of one array texture, with its own chain of mipmaps. Layers
//! can't bleed into each other the way tiles packed side by side into one image do once
//! they are shrunk, so distant ground is smooth instead of shimmering or seamed.
//!
//! A mesh for this material gives the position within the tile in its first UV channel,
//! and the number of the tile in the first number of its second. The second number of
//! the second channel names an animated texture (`TileCycles`), or is 0 for a still one.
//!
//! The ground's material may also carry a map of which way a smooth surface through its
//! heights faces (`track/shading.rs`), and then lights the ground by that instead of by
//! its flat triangles. Scenery and backdrops leave it out.
//!
//! In the shadow maps, and in any prepass, a material with an alpha mask runs
//! `tiles_prepass.wgsl`, which cuts the tiles' holes out of its shadows. An opaque one is
//! drawn there by its depth alone, which is cheaper: see `docs/smoothness.md`.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat, TextureViewDescriptor,
    TextureViewDimension,
};
use bevy::shader::ShaderRef;

use super::TrackSettings;
use crate::mipmaps::mip_chain;

/// A standard material, so that what wears it is lit and shadowed like everything else,
/// with its colour (and, for an alpha mode that uses it, its alpha) taken from the tiles.
pub type TileMaterial = ExtendedMaterial<StandardMaterial, TileTextures>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct TileTextures {
    /// One layer per tile. See `tile_array`.
    #[texture(100, dimension = "2d_array")]
    #[sampler(101)]
    pub tiles: Handle<Image>,
    /// For the ground only: which way the smooth surface through its heights faces, from
    /// `track/shading.rs`. Used when `ground.lit_smoothly` says so.
    #[texture(102)]
    #[sampler(103)]
    pub ground_normals: Option<Handle<Image>>,
    #[uniform(104)]
    pub ground: GroundShading,
    /// Which frame each animated texture is on. All 0 for a material with none.
    #[uniform(105)]
    pub cycles: TileCycles,
    /// How a lit material is lit. Set by the `lighting` slice.
    #[uniform(106)]
    pub lighting: TileLighting,
    /// For the ground only: its cells' tiles, from `track/blend.rs`. Used when
    /// `ground.blend_width` says so. Read texel by texel, so it is never filtered.
    #[texture(107, filterable = false)]
    pub ground_cells: Option<Handle<Image>>,
}

/// How `tiles.wgsl` lights a lit material, as it declares it.
#[derive(ShaderType, Reflect, Debug, Clone, Copy, Default, PartialEq)]
pub struct TileLighting {
    /// 1 for the cheap way: the sun, its shadows and the ambient light on a matte surface,
    /// and nothing else. 0 for Bevy's own.
    pub simple: u32,
}

/// How many animated textures one material can show. Past that many, a texture shows its
/// first frame only. The most any track seen has is 3.
pub const MAX_TEXTURE_CYCLES: usize = 31;

/// Which frame each animated texture of a material is on, as `tiles.wgsl` declares it: how
/// many tiles on from its first frame's. The shader adds that to the tile a vertex names,
/// for the animated texture its second UV channel names. Kept on the CPU and changed only
/// when a frame changes, rather than worked out from the time on the graphics card: the
/// forward and the deferred passes see the time in different places.
#[derive(ShaderType, Reflect, Debug, Clone, Copy, Default, PartialEq)]
pub struct TileCycles {
    /// Four to a vector. The first, for textures that stay still, is always 0.
    offsets: [UVec4; (MAX_TEXTURE_CYCLES + 1) / 4],
}

impl TileCycles {
    /// Shows frame `frame` of animated texture `cycle`, counting from 1.
    pub fn set(&mut self, cycle: usize, frame: u32) {
        if (1..=MAX_TEXTURE_CYCLES).contains(&cycle) {
            self.offsets[cycle / 4][cycle % 4] = frame;
        }
    }

    /// The frame animated texture `cycle` shows, counting from 1. 0 for any other.
    pub fn frame(&self, cycle: usize) -> u32 {
        if (1..=MAX_TEXTURE_CYCLES).contains(&cycle) {
            self.offsets[cycle / 4][cycle % 4]
        } else {
            0
        }
    }
}

/// How the shader finds the ground's normal map and its cells, as `tiles.wgsl` declares
/// it.
#[derive(ShaderType, Reflect, Debug, Clone, Copy, Default)]
pub struct GroundShading {
    /// Side of the square the normal map and the cells cover, the whole track, in metres.
    pub size: f32,
    /// 1 to light the ground by `ground_normals`, 0 to light it by its triangles.
    pub lit_smoothly: u32,
    /// Ground cells along each side of the track, in `ground_cells`.
    pub cells: u32,
    /// How far into a cell, from each edge, the tile across it is faded in, as a share of
    /// the cell's side. 0 to leave every tile's edge hard, as MTM2 does.
    pub blend_width: f32,
}

impl MaterialExtension for TileTextures {
    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    /// Bevy only runs it for a material that may leave pixels out (see the shader).
    fn prepass_fragment_shader() -> ShaderRef {
        PREPASS_SHADER_PATH.into()
    }
}

/// Where `embedded_asset!` puts `tiles.wgsl` and `tiles_prepass.wgsl`: the crate's name,
/// then the path below `src`.
const SHADER_PATH: &str = "embedded://monster_truck_rural_ruckus/track/tiles.wgsl";
const PREPASS_SHADER_PATH: &str = "embedded://monster_truck_rural_ruckus/track/tiles_prepass.wgsl";

/// The most anisotropic filtering a sampler can ask for.
const MOST_ANISOTROPY: u16 = 16;

/// What the sampler is told, given the settings. Anisotropic filtering works by reading
/// between mipmap levels, so without them there is none.
fn anisotropy_clamp(settings: &TrackSettings) -> u16 {
    if settings.mipmaps {
        settings.anisotropy.clamp(1, MOST_ANISOTROPY)
    } else {
        1
    }
}

/// Square tiles `size` pixels a side, each given as red, green, blue and alpha bytes row
/// by row, as an array texture: one layer each.
///
/// With `settings.mipmaps` off, nothing to do with them happens at all: none are worked
/// out, none are stored or sent to the graphics card, and the texture is sampled as the
/// single image it is, without the filtering that goes with them.
pub fn tile_array(size: usize, tiles: &[Vec<u8>], settings: &TrackSettings) -> Image {
    let mipmaps = settings.mipmaps;
    let mut data = Vec::new();
    let mut levels = 0;
    for tile in tiles {
        let chain = if mipmaps {
            mip_chain(tile, size)
        } else {
            vec![tile.clone()]
        };
        levels = chain.len();
        data.extend(chain.into_iter().flatten());
    }

    let mut image = Image {
        data: Some(data),
        asset_usage: RenderAssetUsages::default(),
        // Said outright, because a single tile would otherwise pass for a plain 2D texture.
        texture_view_descriptor: Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2Array),
            ..default()
        }),
        sampler: ImageSampler::Descriptor(ImageSamplerDescriptor {
            // A cell shows exactly one tile, edge to edge, so nothing should wrap round.
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: if mipmaps {
                ImageFilterMode::Linear
            } else {
                ImageFilterMode::Nearest
            },
            anisotropy_clamp: anisotropy_clamp(settings),
            ..default()
        }),
        ..default()
    };
    image.texture_descriptor.size = Extent3d {
        width: size as u32,
        height: size as u32,
        depth_or_array_layers: tiles.len() as u32,
    };
    image.texture_descriptor.dimension = TextureDimension::D2;
    image.texture_descriptor.mip_level_count = levels as u32;
    // The palette's colours are as they appear on screen, not linear light.
    image.texture_descriptor.format = TextureFormat::Rgba8UnormSrgb;
    image
}

/// Carries a change of `TrackSettings::anisotropy` to the tiles already made, for the
/// ground and the scenery alike, so that its cost can be seen while driving. Tiles made
/// without mipmaps (the backdrop's) take none: a sampler refuses anisotropy without them.
pub(super) fn apply_anisotropy(
    settings: Res<TrackSettings>,
    // Both absent in a headless app, which has no textures.
    materials: Option<ResMut<Assets<TileMaterial>>>,
    images: Option<ResMut<Assets<Image>>>,
) {
    let (Some(mut materials), Some(mut images)) = (materials, images) else {
        return;
    };
    let wanted = |image: &Image| {
        if image.texture_descriptor.mip_level_count > 1 {
            anisotropy_clamp(&settings)
        } else {
            1
        }
    };
    let out_of_date: Vec<_> = materials
        .iter()
        .filter(|(_, material)| {
            images
                .get(&material.extension.tiles)
                .is_some_and(|image| match &image.sampler {
                    ImageSampler::Descriptor(sampler) => sampler.anisotropy_clamp != wanted(image),
                    _ => false,
                })
        })
        .map(|(id, _)| id)
        .collect();

    for id in out_of_date {
        // Borrowed mutably so that it counts as changed: a material holds on to the
        // sampler it was prepared with, and nothing tells it that its image has a new one.
        let Some(material) = materials.get_mut(id).map(|material| material.into_inner()) else {
            continue;
        };
        let Some(mut image) = images.get_mut(&material.extension.tiles) else {
            continue;
        };
        let wanted = wanted(&image);
        if let ImageSampler::Descriptor(sampler) = &mut image.sampler {
            sampler.anisotropy_clamp = wanted;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(mipmaps: bool) -> TrackSettings {
        TrackSettings {
            mipmaps,
            ..default()
        }
    }

    #[test]
    fn tile_array_has_a_layer_and_a_mip_chain_per_tile() {
        let tiles = [
            vec![10; 4 * 4 * 4],
            vec![20; 4 * 4 * 4],
            vec![30; 4 * 4 * 4],
        ];
        let image = tile_array(4, &tiles, &settings(true));
        assert_eq!(image.texture_descriptor.size.depth_or_array_layers, 3);
        assert_eq!(image.texture_descriptor.mip_level_count, 3);

        // Layer by layer, each with its levels in turn: 16 + 4 + 1 pixels.
        let data = image.data.as_ref().unwrap();
        assert_eq!(data.len(), 3 * (16 + 4 + 1) * 4);
        assert_eq!(data[0], 10);
        assert_eq!(data[(16 + 4 + 1) * 4], 20);
        assert_eq!(data[data.len() - 1], 30);
    }

    #[test]
    fn mipmaps_can_be_left_out() {
        let tiles = [vec![10; 4 * 4 * 4], vec![20; 4 * 4 * 4]];
        let image = tile_array(4, &tiles, &settings(false));
        assert_eq!(image.texture_descriptor.mip_level_count, 1);
        // Only the tiles themselves, byte for byte.
        assert_eq!(image.data.as_ref().unwrap(), &tiles.concat());
        let ImageSampler::Descriptor(sampler) = &image.sampler else {
            panic!("no sampler");
        };
        assert_eq!(sampler.anisotropy_clamp, 1);
        assert!(matches!(sampler.mipmap_filter, ImageFilterMode::Nearest));
    }

    #[test]
    fn a_still_texture_is_always_on_its_first_frame() {
        let mut cycles = TileCycles::default();
        cycles.set(0, 5);
        cycles.set(MAX_TEXTURE_CYCLES + 1, 5);
        assert_eq!(cycles, TileCycles::default());
        cycles.set(1, 3);
        cycles.set(MAX_TEXTURE_CYCLES, 7);
        assert_eq!((cycles.frame(1), cycles.frame(MAX_TEXTURE_CYCLES)), (3, 7));
        assert_eq!((cycles.frame(0), cycles.frame(2)), (0, 0));
    }

    /// Or a shadow would be cut out by the wrong tile.
    #[test]
    fn the_prepass_shader_finds_the_tile_as_the_main_shader_does() {
        let main = include_str!("tiles.wgsl");
        let prepass = include_str!("tiles_prepass.wgsl");
        for line in [
            "    let cycle = min(u32(round(in.uv_b.y)), 31u);",
            "    let tile = i32(round(in.uv_b.x)) + i32(cycles.offsets[cycle / 4u][cycle % 4u]);",
            "@group(#{MATERIAL_BIND_GROUP}) @binding(100) var tiles: texture_2d_array<f32>;",
            "@group(#{MATERIAL_BIND_GROUP}) @binding(101) var tiles_sampler: sampler;",
            "@group(#{MATERIAL_BIND_GROUP}) @binding(105) var<uniform> cycles: TileCycles;",
        ] {
            assert!(main.contains(line), "tiles.wgsl lacks {line}");
            assert!(prepass.contains(line), "tiles_prepass.wgsl lacks {line}");
        }
    }

    #[test]
    fn the_anisotropy_asked_for_is_what_the_sampler_gets() {
        let clamp = |anisotropy, mipmaps| {
            anisotropy_clamp(&TrackSettings {
                anisotropy,
                mipmaps,
                ..default()
            })
        };
        assert_eq!(clamp(4, true), 4);
        // Kept to what a sampler accepts, and to nothing at all without mipmaps.
        assert_eq!(clamp(0, true), 1);
        assert_eq!(clamp(64, true), 16);
        assert_eq!(clamp(16, false), 1);
    }
}
