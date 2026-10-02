//! The water's surface, drawn with a standard material and `water.wgsl`, which moves it
//! with the wind's waves and the trucks' ripples.
//!
//! Waves that move the surface up and down need vertices close together, and the water can
//! cover kilometres. So it is two meshes with one look. Round the camera is a fine patch
//! (`PATCH_SIZE`), which the shader moves along under the camera, whose waves really rise
//! and fall, and which eases flat towards its edge. Everywhere else is one flat sheet over
//! the whole track, which the shader leaves out under the patch. Far off, the waves only
//! bend the light, and at that distance there is no telling the difference.
//!
//! The patch keeps its vertices on a fixed grid of the world as it moves (`PATCH_SPACING`),
//! so that the waves don't swim across it.
//!
//! Which of the two draws a pixel is settled by one test on the pixel, the same in both:
//! whether it is inside the patch's square. The patch reaches a square further, flat, and
//! leaves out what is past its edge, as the sheet leaves out what is inside it. Settled by
//! the triangles' own edges instead, antialiasing left some of the edge's pixels to
//! neither, which showed as a dashed line across the water.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;

use super::WaterSurface;
use super::ripples::{Ripples, ShaderRipples};
use super::shore::{ShoreUniform, depth_map};
use super::wind::Wind;
use crate::game_state::GameState;
use crate::track::Track;

/// What the water's surface looks like. Monster Truck Madness 2's tracks do not say, so
/// it is a deep teal, see-through looking down into it, so that the ground under shallow
/// water shows and a road through a ford can still be followed. `water.wgsl` makes it
/// less see-through at a glancing angle.
const WATER_COLOR: Color = Color::srgba(0.04, 0.26, 0.34, 0.55);

/// How far the patch of moving water round the camera reaches, in metres along a side,
/// and how far apart its vertices are. Closer vertices shape shorter waves; the shortest
/// the shader makes move the surface are about four times this long. The patch has
/// (`PATCH_SIZE` / `PATCH_SPACING`)² squares, all drawn every frame. Both must match
/// `water.wgsl`, which a test checks.
const PATCH_SIZE: f32 = 240.0;
const PATCH_SPACING: f32 = 1.5;
/// How big the squares of the sheet round the patch are, in metres. Across one enormous
/// triangle, where a pixel is on the water can only be worked out to a few tenths of a
/// metre, and the edge the sheet leaves for the patch wandered by that much, a broken
/// line in the distance. Across squares this size it is exact.
const SHEET_SPACING: f32 = 20.0;

pub(super) type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterWaves>;

/// On the plain flat plane drawn instead of the moving water (`WaterSettings::flat`).
#[derive(Component)]
pub(super) struct FlatWater;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(super) struct WaterWaves {
    /// The ripples (`ShaderRipples`), in a buffer that both materials share.
    #[storage(100, read_only)]
    pub(super) ripples: Handle<ShaderBuffer>,
    /// The colour of the sky, in linear light, which the water mirrors at a glancing angle.
    /// There is no picture of the sky for it to mirror, only its colour.
    #[uniform(101)]
    pub(super) sky: Vec4,
    #[uniform(102)]
    pub(super) wind: WindUniform,
    /// Where the map of the water's depth lies, and the map, for the shore.
    #[uniform(103)]
    pub(super) shore: ShoreUniform,
    #[texture(104, filterable = false)]
    pub(super) depths: Handle<Image>,
}

/// The buffer the water's materials read the ripples from, and what was last written to it.
#[derive(Resource)]
pub(super) struct RippleBuffer {
    buffer: Handle<ShaderBuffer>,
    sent: ShaderRipples,
}

/// The wind as `water.wgsl` reads it, and which of the two meshes this is. How hard it
/// blows the shader works out from its clock, as `wind` does (see there).
#[derive(ShaderType, Clone, Copy, Debug)]
pub(super) struct WindUniform {
    /// Which way the wind blows, of length 1, on the ground plane (world X and Z).
    towards: Vec2,
    /// 1 on the patch round the camera, 0 on the sheet everywhere else.
    round_camera: u32,
}

impl WindUniform {
    fn new(wind: Wind, patch: bool) -> Self {
        Self {
            towards: wind.towards,
            round_camera: patch as u32,
        }
    }
}

impl MaterialExtension for WaterWaves {
    fn vertex_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }
}

/// Where `embedded_asset!` puts `water.wgsl`: the crate's name, then the path below `src`.
const SHADER_PATH: &str = "embedded://monster_truck_rural_ruckus/water/water.wgsl";

/// Spawns the surface. In an app that can't draw it is still there, bare, as a marker that
/// the track has water.
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_water(
    mut commands: Commands,
    track: Res<Track>,
    sky: Option<Res<ClearColor>>,
    wind: Option<Res<Wind>>,
    // All absent in a headless app, which has no use for looks.
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<WaterMaterial>>>,
    flat_materials: Option<ResMut<Assets<StandardMaterial>>>,
    images: Option<ResMut<Assets<Image>>>,
    buffers: Option<ResMut<Assets<ShaderBuffer>>>,
    ripples: Option<ResMut<Ripples>>,
) {
    // Nothing of the last race's water is left on this one.
    if let Some(mut ripples) = ripples {
        ripples.clear();
    }
    commands.remove_resource::<RippleBuffer>();
    let Some(level) = track.water_level else {
        return;
    };
    let sheet = commands
        .spawn((
            Name::new("Water"),
            WaterSurface,
            Transform::from_xyz(0.0, level, 0.0),
            DespawnOnExit(GameState::Racing),
        ))
        .id();
    let (
        Some(mut meshes),
        Some(mut materials),
        Some(mut flat_materials),
        Some(mut images),
        Some(mut buffers),
    ) = (meshes, materials, flat_materials, images, buffers)
    else {
        return;
    };
    let sent = ShaderRipples::default();
    let ripples = buffers.add(ShaderBuffer::from(sent.clone()));
    commands.insert_resource(RippleBuffer {
        buffer: ripples.clone(),
        sent,
    });
    let (depth_map, shore) = depth_map(&track.heights, level);
    let depths = images.add(depth_map);
    let wind = wind.map_or_else(Wind::default, |wind| *wind);
    let mut material = |patch: bool| {
        materials.add(WaterMaterial {
            base: StandardMaterial {
                base_color: WATER_COLOR,
                alpha_mode: AlphaMode::Blend,
                // Smooth, so that the sun glints off the waves that the shader makes. Little
                // reflectance, since the sky it should mirror is not there to be: it would
                // mirror a plain grey. `water.wgsl` mirrors the sky's colour instead.
                perceptual_roughness: 0.08,
                reflectance: 0.3,
                // So that a camera that dips under the surface still sees it.
                double_sided: true,
                cull_mode: None,
                ..default()
            },
            extension: WaterWaves {
                ripples: ripples.clone(),
                sky: sky
                    .as_ref()
                    .map_or(Color::WHITE, |sky| sky.0)
                    .to_linear()
                    .to_vec4(),
                wind: WindUniform::new(wind, patch),
                shore,
                depths: depths.clone(),
            },
        })
    };
    let (sheet_material, patch_material) = (material(false), material(true));

    let size = track.heights.size();
    let sheet_squares = (size / SHEET_SPACING).ceil() as u32;
    commands.entity(sheet).insert((
        Mesh3d(
            meshes.add(
                Plane3d::new(Vec3::Y, Vec2::splat(size / 2.0))
                    .mesh()
                    .subdivisions(sheet_squares - 1),
            ),
        ),
        MeshMaterial3d(sheet_material),
        // A see-through sheet over the whole track would darken everything under it.
        NotShadowCaster,
    ));

    // With a square to spare all round: see the module's notes.
    let reach = PATCH_SIZE + 2.0 * PATCH_SPACING;
    let squares = (reach / PATCH_SPACING).round() as u32;
    // Drawn instead of both when the water is flat (`WaterSettings::flat`), which
    // `ice::freeze_or_thaw` sees to. One square: with no patch, nothing needs the sheet's
    // small ones.
    commands.spawn((
        Name::new("Flat water"),
        FlatWater,
        Transform::from_xyz(0.0, level, 0.0),
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(size / 2.0)).mesh())),
        MeshMaterial3d(flat_materials.add(StandardMaterial {
            base_color: WATER_COLOR,
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.08,
            reflectance: 0.3,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
        NotShadowCaster,
        Visibility::Hidden,
        DespawnOnExit(GameState::Racing),
    ));

    commands.spawn((
        Name::new("Water round the camera"),
        Transform::from_xyz(0.0, level, 0.0),
        Mesh3d(
            meshes.add(
                Plane3d::new(Vec3::Y, Vec2::splat(reach / 2.0))
                    .mesh()
                    .subdivisions(squares - 1),
            ),
        ),
        MeshMaterial3d(patch_material),
        NotShadowCaster,
        // The shader moves it to wherever the camera is, which Bevy can't know.
        NoFrustumCulling,
        DespawnOnExit(GameState::Racing),
    ));
}

/// Gives the shader the ripples, when they have changed. Only then, and only the list: the
/// buffer is written in place, and the materials, which only point at it, stay as they are.
pub(super) fn show_waves(
    time: Res<Time>,
    mut ripples: ResMut<Ripples>,
    buffer: Option<ResMut<RippleBuffer>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
    ripples.expire(time.elapsed_secs());
    let Some(mut buffer) = buffer else {
        return;
    };
    let sent = ripples.for_shader(
        time.elapsed_secs_wrapped(),
        time.wrap_period().as_secs_f32(),
    );
    if sent != buffer.sent
        && let Some(mut data) = buffers.get_mut(&buffer.buffer)
    {
        data.set_data(sent.clone());
        buffer.sent = sent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shader_knows_the_patch() {
        let shader = include_str!("water.wgsl");
        assert!(shader.contains(&format!("const PATCH_SIZE: f32 = {PATCH_SIZE:?};")));
        assert!(shader.contains(&format!("const PATCH_SPACING: f32 = {PATCH_SPACING:?};")));
    }

    /// The patch's vertices must fall on the grid the shader moves it along, or its waves
    /// would swim as it moved.
    #[test]
    fn the_patch_is_a_whole_number_of_squares() {
        let squares = PATCH_SIZE / PATCH_SPACING;
        assert_eq!(squares, squares.round());
        assert_eq!((squares as u32) % 2, 0, "its middle must be a vertex");
    }
}
