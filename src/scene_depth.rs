//! The depth of the picture as drawn so far, for what is drawn over it: a copy of the
//! camera's depth buffer taken after the opaque pass, one sample a pixel, in an image
//! (`SceneDepth::image`) that the particles and the water bind, and read where they would
//! have read Bevy's depth prepass.
//!
//! Why not the depth prepass: it draws all the opaque geometry a second time, and when the
//! camera has MSAA, Bevy then copies the whole multisampled depth texture for shaders to
//! read (`bevy_core_pipeline::prepass::node`). On an integrated Intel GPU at 1920 x 1200
//! with 4x MSAA, the prepass cost 5 to 7 ms of a 20 ms frame, of which its own draws were
//! 0.2 ms, and the particles' reads of the copy nothing measurable (measured 2026-10-10,
//! see docs/smoothness.md). Letting a shader read the depth buffer costs nothing
//! measurable either. So here a pass over the whole picture reads one sample of each pixel
//! into an `R32Float` image: 9 MB written, no second pass over the geometry, no copy.
//!
//! Main world: `SceneDepth`, the image, kept the size of the race camera's target, and
//! every 3D camera's depth texture made readable. Render world: the pass, between the
//! opaque pass and the transmissive pass (the water's). In the frame after the window
//! changes size the image is the old size, and is left as it is for that frame.
//!
//! Knows nothing of the game. Does nothing in an app that cannot draw.

use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::asset::io::embedded::EmbeddedAssetRegistry;
use bevy::core_pipeline::core_3d::main_opaque_pass_3d;
use bevy::core_pipeline::{Core3d, Core3dSystems, FullscreenShader};
use bevy::pbr::{PbrPlugin, main_transmissive_pass_3d};
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BindingType,
    CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState, Operations,
    PipelineCache, RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
    ShaderStages, SpecializedRenderPipeline, SpecializedRenderPipelines, TextureFormat,
    TextureSampleType, TextureUsages, TextureViewDimension,
};
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::texture::GpuImage;
use bevy::render::view::{Msaa, ViewDepthTexture};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;

pub struct SceneDepthPlugin;

impl Plugin for SceneDepthPlugin {
    fn build(&self, app: &mut App) {
        // Only a look. An app that cannot draw has no use for it.
        if !app.is_plugin_added::<PbrPlugin>() {
            return;
        }
        // In `src/shaders`, with the game's other shaders: `embedded_asset!` takes only a
        // path below this file's folder.
        app.world_mut()
            .resource_mut::<EmbeddedAssetRegistry>()
            .insert_asset(
                PathBuf::from("src/shaders/scene_depth.wgsl"),
                Path::new(SHADER_PATH.trim_start_matches("embedded://")),
                include_bytes!("shaders/scene_depth.wgsl").as_slice(),
            );
        // Sized to the camera as soon as there is one.
        let size = UVec2::ONE;
        let image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(depth_image(size));
        app.insert_resource(SceneDepth { image, size })
            .add_plugins(ExtractResourcePlugin::<SceneDepth>::default())
            .add_systems(PostUpdate, (make_depth_readable, fit_the_camera));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<SpecializedRenderPipelines<SceneDepthPipeline>>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare_pipelines.in_set(RenderSystems::Prepare))
            .add_systems(
                Core3d,
                copy_depth
                    .in_set(Core3dSystems::MainPass)
                    .after(main_opaque_pass_3d)
                    .before(main_transmissive_pass_3d),
            );
    }
}

/// The depth of what is drawn before the water and the particles, as an `R32Float` image
/// the size of the race camera's target: each texel the depth buffer's value at that pixel,
/// 0 the furthest (the sky) and 1 the nearest. Read it with `textureLoad` at the pixel: it
/// is not filterable. A material that binds it must be touched when this resource changes,
/// which is when the window changes size: the image is then a new one, and a material keeps
/// the one it was prepared with.
#[derive(Resource, Clone, ExtractResource)]
pub struct SceneDepth {
    pub image: Handle<Image>,
    /// The image's size, in pixels. Kept here: the image is for the render world only, so
    /// it is dropped from `Assets<Image>` once it is extracted.
    size: UVec2,
}

/// Where `scene_depth.wgsl` is among the embedded assets.
const SHADER_PATH: &str = "embedded://monster_truck_rural_ruckus/shaders/scene_depth.wgsl";

/// 32 bits, as the depth buffer has: the particles fade over centimetres.
const FORMAT: TextureFormat = TextureFormat::R32Float;

fn depth_image(size: UVec2) -> Image {
    let mut image = Image::new_target_texture(size.x, size.y, FORMAT, None);
    // Drawn into and read on the graphics card only: nothing to upload, nothing to keep.
    image.data = None;
    image.asset_usage = RenderAssetUsages::RENDER_WORLD;
    // Bevy carries an image without data over to its new size by a copy on the card.
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    image
}

/// Lets a shader read a camera's depth buffer: a usage flag on the texture, which was
/// measured to cost nothing.
fn make_depth_readable(mut cameras: Query<&mut Camera3d, Added<Camera3d>>) {
    for mut camera in &mut cameras {
        let usages =
            TextureUsages::from(camera.depth_texture_usages) | TextureUsages::TEXTURE_BINDING;
        camera.depth_texture_usages = usages.into();
    }
}

/// Keeps the image the size of the 3D camera's target: a new image when that changes.
fn fit_the_camera(
    cameras: Query<&Camera, With<Camera3d>>,
    mut images: ResMut<Assets<Image>>,
    mut depth: ResMut<SceneDepth>,
) {
    let Some(size) = cameras
        .iter()
        .find_map(|camera| camera.physical_target_size())
    else {
        return;
    };
    if size == depth.size || size.min_element() == 0 {
        return;
    }
    // The handle is held here, so it is never stale.
    if images.insert(&depth.image, depth_image(size)).is_ok() {
        // A change to the resource tells the materials that bind it (see `SceneDepth`).
        depth.size = size;
    }
}

#[derive(Resource)]
struct SceneDepthPipeline {
    /// For a depth texture with one sample a pixel, and for a multisampled one.
    layouts: [BindGroupLayoutDescriptor; 2],
    fullscreen_shader: FullscreenShader,
    shader: Handle<Shader>,
}

fn init_pipeline(
    mut commands: Commands,
    fullscreen_shader: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
) {
    let layout = |multisampled: bool| {
        BindGroupLayoutDescriptor::new(
            if multisampled {
                "scene_depth_multisampled_layout"
            } else {
                "scene_depth_layout"
            },
            &BindGroupLayoutEntries::single(
                ShaderStages::FRAGMENT,
                BindingType::Texture {
                    sample_type: TextureSampleType::Depth,
                    view_dimension: TextureViewDimension::D2,
                    multisampled,
                },
            ),
        )
    };
    commands.insert_resource(SceneDepthPipeline {
        layouts: [layout(false), layout(true)],
        fullscreen_shader: fullscreen_shader.clone(),
        shader: asset_server.load(SHADER_PATH),
    });
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
struct SceneDepthKey {
    multisampled: bool,
}

impl SpecializedRenderPipeline for SceneDepthPipeline {
    type Key = SceneDepthKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("scene_depth_pipeline".into()),
            layout: vec![self.layouts[key.multisampled as usize].clone()],
            vertex: self.fullscreen_shader.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs: if key.multisampled {
                    vec!["MULTISAMPLED".into()]
                } else {
                    Vec::new()
                },
                targets: vec![Some(ColorTargetState {
                    format: FORMAT,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        }
    }
}

/// On a 3D view in the render world: the pipeline for its depth texture's sample count.
#[derive(Component)]
struct SceneDepthPipelineId(CachedRenderPipelineId);

fn prepare_pipelines(
    mut commands: Commands,
    views: Query<(Entity, &Msaa), With<Camera3d>>,
    pipeline: Res<SceneDepthPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<SceneDepthPipeline>>,
    cache: Res<PipelineCache>,
) {
    for (entity, msaa) in &views {
        let key = SceneDepthKey {
            multisampled: *msaa != Msaa::Off,
        };
        let id = pipelines.specialize(&cache, &pipeline, key);
        commands.entity(entity).insert(SceneDepthPipelineId(id));
    }
}

/// The pass: the view's depth buffer, one sample a pixel, into the image.
fn copy_depth(
    view: ViewQuery<(&ViewDepthTexture, &SceneDepthPipelineId)>,
    depth: Res<SceneDepth>,
    images: Res<RenderAssets<GpuImage>>,
    pipeline: Res<SceneDepthPipeline>,
    cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    let (depth_texture, pipeline_id) = view.into_inner();
    let (Some(render_pipeline), Some(image)) = (
        cache.get_render_pipeline(pipeline_id.0),
        images.get(&depth.image),
    ) else {
        return;
    };
    // The frame after the window changes size: the image is still the old size.
    if image.texture.size() != depth_texture.texture.size() {
        return;
    }
    let multisampled = depth_texture.texture.sample_count() > 1;
    let bind_group = device.create_bind_group(
        "scene_depth_bind_group",
        &cache.get_bind_group_layout(&pipeline.layouts[multisampled as usize]),
        &BindGroupEntries::single(depth_texture.view()),
    );
    let mut pass = ctx
        .command_encoder()
        .begin_render_pass(&RenderPassDescriptor {
            label: Some("scene_depth_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &image.texture_view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    pass.set_pipeline(render_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
}
