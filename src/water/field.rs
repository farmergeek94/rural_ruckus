//! The water round the camera as a field of heights that the trucks disturb, stepped on
//! the graphics card by `field.wgsl`: a moving truck piles the water up in front of its
//! tires and leaves a wake that spreads behind it, a wheel that comes down into the water
//! starts a ring, the waves cross, slow in the shallows and reflect off the shore, and the
//! foam the trucks churn up lies on the water, spreads and fades after they have gone.
//! `water.wgsl` raises the surface by the field's height, tilts it by its slope, and
//! whitens it by its foam.
//!
//! Each frame the trucks' doings are a short list of `Source`s (written by `wake` into
//! `Disturbances`), and the field is stepped as many times as the frame was long
//! (`STEP`), each step a pass over every texel on the graphics card. Nothing comes back
//! to the CPU. The field is a square window of `TEXELS` texels `SPACING` metres apart
//! round the camera, which the shader addresses round and round (see `field.wgsl`), so
//! that moving the camera costs nothing; when the camera jumps further than the window's
//! outer band is wide (`JUMP`), the field is cleared.
//!
//! The field is three textures: the state before and after a step, which swap places each
//! step, and what `water.wgsl` reads, written at every step. A step reads the depth map
//! that `shore` makes, for how deep the water is and where the land is.
//!
//! Only in an app that can draw. In one that can't, nothing here runs.

use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::asset::io::embedded::EmbeddedAssetRegistry;
use bevy::core_pipeline::schedule::camera_driver;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::binding_types::{
    storage_buffer_read_only, texture_2d, texture_storage_2d, uniform_buffer,
};
use bevy::render::render_resource::{
    BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
    CachedComputePipelineId, ComputePassDescriptor, ComputePipelineDescriptor, Extent3d,
    PipelineCache, ShaderStages, ShaderType, StorageBuffer, StorageTextureAccess, TextureDimension,
    TextureFormat, TextureSampleType, TextureUsages, UniformBuffer,
};
use bevy::render::renderer::{
    RenderContext, RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue,
};
use bevy::render::texture::GpuImage;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::transform::TransformSystems;

use super::WaterSettings;
use super::ice;
use super::shore::{ShoreUniform, depth_map};
use crate::game_state::GameState;
use crate::track::Track;
use crate::weather::WeatherSettings;

/// How many texels the window is along a side, and how far apart they are, in metres. The
/// window is their product across: 256 m, a little more than the patch of moving water
/// round the camera (`surface::PATCH_SIZE`). Each step is a pass over every texel, so
/// more texels cost more; closer texels carry shorter waves. Both must match
/// `field.wgsl` and `water.wgsl`, which a test checks.
pub(super) const TEXELS: u32 = 512;
pub(super) const SPACING: f32 = 0.5;
/// How long one step of the field is, in seconds, and the most steps taken in one frame:
/// a frame longer than that loses the rest, rather than taking longer still to catch up.
const STEP: f32 = 1.0 / 60.0;
const MOST_STEPS: u32 = 4;
/// How many sources the field is given at once: a tire each for the wheels of eight
/// trucks, and the rest for wheels coming down into the water. It must be the length of
/// the array in `field.wgsl`, which a test checks.
pub(super) const MAX_SOURCES: usize = 64;
/// How far across the window is, in metres.
const WINDOW: f32 = TEXELS as f32 * SPACING;
/// How far the camera may move in one frame before the field is cleared, in metres: the
/// window's outer band, which the shader eases flat, is about 20 m wide (a test checks).
/// Further, and what a texel held on the other side of the window would show.
const JUMP: f32 = WINDOW / 16.0;
/// Four 16-bit floats a texel: the height, its rate of rise and the foam, and for what
/// `water.wgsl` reads, the height, two slopes and the foam. 16 bits hold a height to
/// a tenth of a millimetre, and can be read between texels, which 32-bit ones can't.
const FORMAT: TextureFormat = TextureFormat::Rgba16Float;
/// The shader's workgroup, along each side. As `@workgroup_size` in `field.wgsl`.
const WORKGROUP: u32 = 8;
/// Where `field.wgsl` is among the embedded assets (see `add`).
const SHADER_PATH: &str = "embedded://monster_truck_rural_ruckus/shaders/field.wgsl";

/// One thing a truck does to the water this frame, as `field.wgsl` reads it.
#[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Source {
    /// Where, on the ground plane (world X and Z), in metres.
    pub at: Vec2,
    /// Which way it goes over the water, times how fast, in m/s.
    pub along: Vec2,
    /// How far out from `at` it reaches, in metres: a bell, a third as strong there.
    pub radius: f32,
    /// How fast it piles the water up ahead of it and draws it down behind, in metres of
    /// height each second at the bell's peak. A tire going through the water.
    pub push: f32,
    /// How much it lifts the water at once, in metres, at the bell's peak. Below 0, it
    /// pushes it down. A wheel coming down into the water.
    pub lift: f32,
    /// How much foam it churns up each second, and at once.
    pub foam: f32,
    pub burst: f32,
}

/// The sources as the shader reads them.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
struct Sources {
    sources: [Source; MAX_SOURCES],
    /// How many of `sources` are in use, from the first.
    count: u32,
}

impl Default for Sources {
    fn default() -> Self {
        Self {
            sources: [Source::default(); MAX_SOURCES],
            count: 0,
        }
    }
}

/// What the shader is told about the frame.
#[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
struct FieldUniform {
    /// Where the window's middle is: the camera, on the ground plane.
    middle: Vec2,
    /// How long one step is, in seconds.
    step: f32,
    /// 1 to forget everything.
    clear: u32,
    /// Where the depth map lies.
    shore: ShoreUniform,
}

/// The field's textures, made with each race on a track with water, and the depth map
/// the shore gives it. Only in an app that can draw.
#[derive(Resource, Clone, ExtractResource)]
pub(super) struct Field {
    before: Handle<Image>,
    after: Handle<Image>,
    /// What `water.wgsl` reads: per texel the height, in metres, how much it rises for each
    /// metre along X and along Z, and the foam.
    pub(super) look: Handle<Image>,
    /// How deep the still water is over the ground (`shore::depth_map`), and where it lies.
    pub(super) depths: Handle<Image>,
    pub(super) shore: ShoreUniform,
}

/// What the trucks do to the water this frame. `wake` writes it in `Update`; it is taken
/// in `PostUpdate`.
#[derive(Resource, Default, Debug)]
pub(super) struct Disturbances(pub(super) Vec<Source>);

/// What the field is told for this frame, which goes to the render world.
#[derive(Resource, Clone, ExtractResource, Default, Debug, PartialEq)]
struct FieldFrame {
    /// How many steps to take. None: nothing to do.
    steps: u32,
    uniform: FieldUniform,
    sources: Sources,
}

/// Makes the field work in `app`: its shader, and what steps it in the render world.
pub(super) fn add(app: &mut App) {
    // In `src/shaders`, with the game's other shaders: `embedded_asset!` takes only a path
    // below this file's folder.
    app.world_mut()
        .resource_mut::<EmbeddedAssetRegistry>()
        .insert_asset(
            PathBuf::from("src/shaders/field.wgsl"),
            Path::new(SHADER_PATH.trim_start_matches("embedded://")),
            include_bytes!("../shaders/field.wgsl").as_slice(),
        );
    app.add_plugins((
        ExtractResourcePlugin::<Field>::default(),
        ExtractResourcePlugin::<FieldFrame>::default(),
    ))
    .init_resource::<Disturbances>()
    .init_resource::<FieldFrame>()
    .add_systems(
        PostUpdate,
        // After the camera's place for the frame is settled.
        frame_field
            .after(TransformSystems::Propagate)
            .run_if(in_state(GameState::Racing)),
    );
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app
            .init_resource::<FieldBuffers>()
            .init_resource::<Parity>()
            .add_systems(RenderStartup, make_pipeline)
            .add_systems(
                Render,
                prepare_field.in_set(RenderSystems::PrepareBindGroups),
            )
            // Before the cameras draw, so that the water is drawn from this frame's field.
            .add_systems(
                RenderGraph,
                step_field
                    .in_set(RenderGraphSystems::Render)
                    .before(camera_driver),
            );
    }
}

/// Makes the field for a track with water, blank, with the depth map of its shore.
pub(super) fn make_field(
    mut commands: Commands,
    track: Res<Track>,
    // Absent in a headless app, which has no use for looks.
    images: Option<ResMut<Assets<Image>>>,
) {
    commands.remove_resource::<Field>();
    let (Some(level), Some(mut images)) = (track.water_level, images) else {
        return;
    };
    let (map, shore) = depth_map(&track.heights, level);
    let depths = images.add(map);
    commands.insert_resource(Field {
        before: images.add(blank()),
        after: images.add(blank()),
        look: images.add(blank()),
        depths,
        shore,
    });
}

/// A texture of the field, flat and still, which the shader may both write and read, and
/// which `water.wgsl` may read between texels, round and round.
fn blank() -> Image {
    let mut image = Image::new_fill(
        Extent3d {
            width: TEXELS,
            height: TEXELS,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0; 8],
        FORMAT,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage =
        TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

/// Tells the field about the frame: where the camera is, how many steps to take, and what
/// the trucks did to the water. Takes no steps while the water is drawn flat or is
/// frozen, when nothing is drawn from it.
#[allow(clippy::too_many_arguments)]
fn frame_field(
    time: Res<Time>,
    field: Option<Res<Field>>,
    settings: Res<WaterSettings>,
    weather: Option<Res<WeatherSettings>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut disturbances: ResMut<Disturbances>,
    mut frame: ResMut<FieldFrame>,
    // How much time has passed that has not been stepped yet, in seconds.
    mut owed: Local<f32>,
    // Where the window's middle was the frame before.
    mut last_middle: Local<Option<Vec2>>,
) {
    let eye = cameras
        .iter()
        .find(|(camera, _)| camera.is_active)
        .map(|(_, transform)| transform.translation().xz());
    let (Some(field), Some(eye)) = (field, eye) else {
        disturbances.0.clear();
        frame.set_if_neq(FieldFrame::default());
        return;
    };
    if settings.flat || ice::frozen(weather) {
        disturbances.0.clear();
        *last_middle = None;
        frame.set_if_neq(FieldFrame::default());
        return;
    }
    *owed = (*owed + time.delta_secs()).min(STEP * MOST_STEPS as f32);
    let steps = (*owed / STEP) as u32;
    *owed -= steps as f32 * STEP;
    if steps == 0 {
        // What is done at once is kept for the next frame; what is done each second is
        // covered by the steps the next frame takes.
        disturbances
            .0
            .retain(|source| source.lift != 0.0 || source.burst != 0.0);
        for source in &mut disturbances.0 {
            source.push = 0.0;
            source.foam = 0.0;
        }
        frame.set_if_neq(FieldFrame::default());
        return;
    }
    let clear = last_middle.is_none_or(|last| last.distance(eye) > JUMP);
    *last_middle = Some(eye);
    let mut sources = Sources::default();
    for (slot, source) in sources.sources.iter_mut().zip(disturbances.0.drain(..)) {
        // What is done at once is shared over the frame's steps.
        *slot = Source {
            lift: source.lift / steps as f32,
            burst: source.burst / steps as f32,
            ..source
        };
        sources.count += 1;
    }
    disturbances.0.clear();
    *frame = FieldFrame {
        steps,
        uniform: FieldUniform {
            middle: eye,
            step: STEP,
            clear: clear as u32,
            shore: field.shore,
        },
        sources,
    };
}

/// The shader's pipeline, made once.
#[derive(Resource)]
struct FieldPipeline {
    layout: BindGroupLayoutDescriptor,
    pipeline: CachedComputePipelineId,
}

fn make_pipeline(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "water field",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_storage_2d(FORMAT, StorageTextureAccess::ReadOnly),
                texture_storage_2d(FORMAT, StorageTextureAccess::WriteOnly),
                texture_storage_2d(FORMAT, StorageTextureAccess::WriteOnly),
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer::<FieldUniform>(false),
                storage_buffer_read_only::<Sources>(false),
            ),
        ),
    );
    let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("water field".into()),
        layout: vec![layout.clone()],
        shader: asset_server.load(SHADER_PATH),
        entry_point: Some("step".into()),
        ..default()
    });
    commands.insert_resource(FieldPipeline { layout, pipeline });
}

/// The frame's uniform and sources on the graphics card.
#[derive(Resource)]
struct FieldBuffers {
    uniform: UniformBuffer<FieldUniform>,
    sources: StorageBuffer<Sources>,
}

impl Default for FieldBuffers {
    fn default() -> Self {
        Self {
            uniform: UniformBuffer::from(FieldUniform::default()),
            sources: StorageBuffer::from(Sources::default()),
        }
    }
}

/// The two bind groups of a frame that takes steps: before to after, and after to before.
#[derive(Resource)]
struct FieldBindGroups([BindGroup; 2]);

/// Which of the two textures holds the latest state: the one the next step reads.
#[derive(Resource, Default)]
struct Parity(usize);

#[allow(clippy::too_many_arguments)]
fn prepare_field(
    mut commands: Commands,
    pipeline: Res<FieldPipeline>,
    pipeline_cache: Res<PipelineCache>,
    images: Res<RenderAssets<GpuImage>>,
    field: Option<Res<Field>>,
    frame: Res<FieldFrame>,
    mut buffers: ResMut<FieldBuffers>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    commands.remove_resource::<FieldBindGroups>();
    if frame.steps == 0 {
        return;
    }
    let Some(field) = field else {
        return;
    };
    // The textures are made in the frame the field is, before this; a race that is over
    // has dropped them.
    let (Some(before), Some(after), Some(look), Some(depths)) = (
        images.get(&field.before),
        images.get(&field.after),
        images.get(&field.look),
        images.get(&field.depths),
    ) else {
        return;
    };
    buffers.uniform.set(frame.uniform);
    buffers.uniform.write_buffer(&device, &queue);
    buffers.sources.set(frame.sources);
    buffers.sources.write_buffer(&device, &queue);
    let (Some(uniform), Some(sources)) = (buffers.uniform.binding(), buffers.sources.binding())
    else {
        return;
    };
    let layout = pipeline_cache.get_bind_group_layout(&pipeline.layout);
    let group = |from: &GpuImage, to: &GpuImage| {
        device.create_bind_group(
            "water field",
            &layout,
            &BindGroupEntries::sequential((
                &from.texture_view,
                &to.texture_view,
                &look.texture_view,
                &depths.texture_view,
                uniform.clone(),
                sources.clone(),
            )),
        )
    };
    commands.insert_resource(FieldBindGroups([
        group(before, after),
        group(after, before),
    ]));
}

/// Takes the frame's steps.
fn step_field(
    mut render_context: RenderContext,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<FieldPipeline>,
    groups: Option<Res<FieldBindGroups>>,
    frame: Res<FieldFrame>,
    mut parity: ResMut<Parity>,
) {
    let Some(groups) = groups else {
        return;
    };
    // Not yet compiled: the first frames.
    let Some(compute) = pipeline_cache.get_compute_pipeline(pipeline.pipeline) else {
        return;
    };
    let diagnostics = render_context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let encoder = render_context.command_encoder();
    let span = diagnostics.time_span(encoder, "water_field");
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("water field"),
            ..default()
        });
        pass.set_pipeline(compute);
        let workgroups = TEXELS.div_ceil(WORKGROUP);
        for _ in 0..frame.steps {
            pass.set_bind_group(0, &groups.0[parity.0], &[]);
            pass.dispatch_workgroups(workgroups, workgroups, 1);
            parity.0 ^= 1;
        }
    }
    span.end(encoder);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shaders_know_the_window() {
        let field = include_str!("../shaders/field.wgsl");
        assert!(field.contains(&format!("const TEXELS: i32 = {TEXELS};")));
        assert!(field.contains(&format!("const SPACING: f32 = {SPACING:?};")));
        assert!(field.contains(&format!("array<Source, {MAX_SOURCES}>")));
        assert!(field.contains(&format!("@workgroup_size({WORKGROUP}, {WORKGROUP}, 1)")));
        let water = include_str!("../shaders/water.wgsl");
        assert!(water.contains(&format!("const FIELD_TEXELS: f32 = {TEXELS}.0;")));
        assert!(water.contains(&format!("const FIELD_SPACING: f32 = {SPACING:?};")));
    }

    #[test]
    fn the_window_is_whole_workgroups() {
        assert_eq!(TEXELS % WORKGROUP, 0);
    }

    /// The window's outer band, which the shader eases flat, must be wider than the
    /// camera may move in a frame without the field being cleared.
    #[test]
    fn a_jump_is_within_the_band() {
        let band = include_str!("../shaders/field.wgsl")
            .lines()
            .find_map(|line| line.strip_prefix("const BAND: f32 = "))
            .and_then(|rest| rest.trim_end_matches(';').parse::<f32>().ok())
            .expect("field.wgsl sets BAND");
        let band = (1.0 - band) * WINDOW / 2.0;
        assert!(JUMP < band, "{JUMP} m jump, {band} m band");
    }
}
