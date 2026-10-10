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
//!
//! The water mirrors the scenery: `water.wgsl` follows the reflected ray across the
//! picture drawn so far, and where it meets the scenery, shows what is there (screen-space
//! reflections). To read that picture, the water is drawn after everything solid, in
//! Bevy's transmissive pass, which copies the picture for it. So it is drawn solid, and
//! mixes what is under it into its colour itself, as a see-through material would.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{
    ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::{ShaderDefVal, ShaderRef};

use super::field::Field;
use super::ripples::Ripples;
use super::shore::ShoreUniform;
use super::wind::Wind;
use super::{WaterSettings, WaterSurface};
use crate::game_state::GameState;
use crate::track::Track;

/// What the water's surface looks like. Monster Truck Madness 2's tracks do not say, so
/// it is a deep blue. Its alpha is how much of the ground deep water hides: `water.wgsl`
/// lets more show through where it is shallow (`WATER_CLARITY`), so that a road through a
/// ford can still be followed, and less at a glancing angle.
/// `water.wgsl` makes it less see-through at a glancing angle.
pub const WATER_COLOR: Color = Color::srgba(0.03, 0.2, 0.4, 0.98);
/// The flat water's (`WaterSettings::flat`) alpha, the same at every depth and angle:
/// it has none of `water.wgsl`.
const FLAT_WATER_ALPHA: f32 = 0.96;

/// How far the patch of moving water round the camera reaches, in metres along a side,
/// and how far apart its vertices are. Closer vertices shape shorter waves; the shortest
/// the shader makes move the surface are about four times this long. The patch has
/// (`PATCH_SIZE` / `PATCH_SPACING`)² squares, all drawn every frame. Both must match
/// `water.wgsl`, which a test checks.
const PATCH_SIZE: f32 = 240.0;
const PATCH_SPACING: f32 = 1.5;
/// The fine patch: how far it reaches, in metres along a side, and how far apart its
/// vertices are. Its middle is `FINE_AHEAD` metres ahead of the camera (`water.wgsl`),
/// where the truck the camera follows is. Vertices this close show a bow wave and a wake
/// as a shape, not only in the light. It has (`FINE_SIZE` / `FINE_SPACING`)² squares. Its
/// edge must lie on the patch's grid, so its size is a whole, even number of the patch's
/// squares (a test checks). Both must match `water.wgsl`.
const FINE_SIZE: f32 = 36.0;
const FINE_SPACING: f32 = 0.5;
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
    /// The field of heights round the camera that the trucks disturb (`Field::look`),
    /// read between its texels, round and round.
    #[texture(100)]
    #[sampler(105)]
    pub(super) field: Handle<Image>,
    #[uniform(101)]
    pub(super) mirroring: Mirroring,
    #[uniform(102)]
    pub(super) wind: WindUniform,
    /// Where the map of the water's depth lies, and the map, for the shore.
    #[uniform(103)]
    pub(super) shore: ShoreUniform,
    #[texture(104, filterable = false)]
    pub(super) depths: Handle<Image>,
}

/// What the water mirrors at a glancing angle, as `water.wgsl` reads it.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
pub(super) struct Mirroring {
    /// The colour of the sky, in linear light. There is no picture of the sky for it to
    /// mirror, only its colour.
    sky: Vec4,
    /// 1 to mirror the scenery as well (`WaterSettings::reflections`), 0 for only the sky.
    scenery: u32,
}

impl Mirroring {
    fn new(sky: Option<&ClearColor>, settings: &WaterSettings) -> Self {
        Self {
            sky: sky.map_or(Color::WHITE, |sky| sky.0).to_linear().to_vec4(),
            scenery: settings.reflections as u32,
        }
    }
}

/// The wind as `water.wgsl` reads it, and which of the three meshes this is. How hard it
/// blows the shader works out from its clock, as `wind` does (see there).
#[derive(ShaderType, Clone, Copy, Debug)]
pub(super) struct WindUniform {
    /// Which way the wind blows, of length 1, on the ground plane (world X and Z).
    towards: Vec2,
    /// 0 on the sheet everywhere, 1 on the patch round the camera, 2 on the fine patch
    /// ahead of it.
    round_camera: u32,
}

impl WindUniform {
    fn new(wind: Wind, mesh: u32) -> Self {
        Self {
            towards: wind.towards,
            round_camera: mesh,
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

    /// Leaves out the standard material's own light through glass, which only
    /// `TRANSMISSIVE_PASS` turns on: it blurs the picture behind with many samples, and
    /// `water.wgsl` reads the picture itself.
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.retain(|def| {
                let (ShaderDefVal::Bool(name, _)
                | ShaderDefVal::Int(name, _)
                | ShaderDefVal::UInt(name, _)) = def;
                !matches!(
                    name.as_str(),
                    "STANDARD_MATERIAL_SPECULAR_TRANSMISSION"
                        | "STANDARD_MATERIAL_DIFFUSE_OR_SPECULAR_TRANSMISSION"
                )
            });
        }
        Ok(())
    }
}

/// The standard material's light through glass, which puts it in Bevy's transmissive pass,
/// so that the picture drawn so far is copied for `water.wgsl` to read. As little as can
/// be: `WaterWaves::specialize` leaves the glass out, and the shader sets it to none.
const TRANSMISSIVE_PASS: f32 = 1e-4;

/// Where `water.wgsl` is among the embedded assets (see `WaterPlugin`).
pub(super) const SHADER_PATH: &str = "embedded://monster_truck_rural_ruckus/shaders/water.wgsl";

/// Spawns the surface. In an app that can't draw it is still there, bare, as a marker that
/// the track has water.
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_water(
    mut commands: Commands,
    track: Res<Track>,
    sky: Option<Res<ClearColor>>,
    settings: Res<WaterSettings>,
    wind: Option<Res<Wind>>,
    // All absent in a headless app, which has no use for looks. The field is made for
    // this race just before, on a track with water.
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<WaterMaterial>>>,
    flat_materials: Option<ResMut<Assets<StandardMaterial>>>,
    field: Option<Res<Field>>,
    ripples: Option<ResMut<Ripples>>,
) {
    // Nothing of the last race's water is left on this one.
    if let Some(mut ripples) = ripples {
        ripples.clear();
    }
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
    let (Some(mut meshes), Some(mut materials), Some(mut flat_materials), Some(field)) =
        (meshes, materials, flat_materials, field)
    else {
        return;
    };
    let wind = wind.map_or_else(Wind::default, |wind| *wind);
    let mut material = |mesh: u32| {
        materials.add(WaterMaterial {
            base: StandardMaterial {
                base_color: WATER_COLOR,
                alpha_mode: alpha_mode(&settings),
                specular_transmission: specular_transmission(&settings),
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
                field: field.look.clone(),
                mirroring: Mirroring::new(sky.as_deref(), &settings),
                wind: WindUniform::new(wind, mesh),
                shore: field.shore,
                depths: field.depths.clone(),
            },
        })
    };
    let (sheet_material, patch_material, fine_material) = (material(0), material(1), material(2));
    let shadows = |mut entity: bevy::ecs::system::EntityCommands| {
        if !settings.shadows {
            entity.insert(NotShadowReceiver);
        }
    };

    let size = super::water_width(&track);
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
    shadows(commands.entity(sheet));

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
            base_color: WATER_COLOR.with_alpha(FLAT_WATER_ALPHA),
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

    let patch = commands.spawn((
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
    shadows(patch);

    let reach = FINE_SIZE + 2.0 * FINE_SPACING;
    let squares = (reach / FINE_SPACING).round() as u32;
    let fine = commands.spawn((
        Name::new("Water round the truck"),
        Transform::from_xyz(0.0, level, 0.0),
        Mesh3d(
            meshes.add(
                Plane3d::new(Vec3::Y, Vec2::splat(reach / 2.0))
                    .mesh()
                    .subdivisions(squares - 1),
            ),
        ),
        MeshMaterial3d(fine_material),
        NotShadowCaster,
        NoFrustumCulling,
        DespawnOnExit(GameState::Racing),
    ));
    shadows(fine);
}

/// How the moving water is drawn, for whether it mirrors the scenery. Mirroring reads the
/// picture drawn so far, which only the transmissive pass copies for it: then the water
/// is drawn solid there, and mixes in what is under it itself. Without, it is a plain
/// see-through surface in the transparent pass, which is cheaper, the more so with MSAA,
/// for which the copy is a resolve of the whole picture.
fn alpha_mode(settings: &WaterSettings) -> AlphaMode {
    if settings.reflections {
        AlphaMode::Opaque
    } else {
        AlphaMode::Blend
    }
}

fn specular_transmission(settings: &WaterSettings) -> f32 {
    if settings.reflections {
        TRANSMISSIVE_PASS
    } else {
        0.0
    }
}

/// Gives the water what it mirrors as it is now, when it has changed: the sky's colour,
/// and whether it mirrors the scenery, with the pass that goes with that. The water is
/// spawned before whoever lights the sky has set it for the race, and the sky changes in
/// the race too (lightning): a sky kept from the spawn left the water mirroring the day
/// at night.
pub(super) fn mirror(
    sky: Option<Res<ClearColor>>,
    settings: Res<WaterSettings>,
    surfaces: Query<&MeshMaterial3d<WaterMaterial>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    let mirroring = Mirroring::new(sky.as_deref(), &settings);
    for surface in &surfaces {
        // Compared first: changing a material sends all of it to the GPU again.
        if materials
            .get(&surface.0)
            .is_some_and(|material| material.extension.mirroring != mirroring)
            && let Some(mut material) = materials.get_mut(&surface.0)
        {
            material.extension.mirroring = mirroring;
            material.base.alpha_mode = alpha_mode(&settings);
            material.base.specular_transmission = specular_transmission(&settings);
        }
    }
}

/// Has the trucks' shadows fall on the moving water, or not, as the settings say.
pub(super) fn receive_shadows(
    mut commands: Commands,
    settings: Res<WaterSettings>,
    surfaces: Query<Entity, With<MeshMaterial3d<WaterMaterial>>>,
) {
    for surface in &surfaces {
        if settings.shadows {
            commands.entity(surface).remove::<NotShadowReceiver>();
        } else {
            commands.entity(surface).insert(NotShadowReceiver);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shader_knows_the_patch() {
        let shader = include_str!("../shaders/water.wgsl");
        assert!(shader.contains(&format!("const PATCH_SIZE: f32 = {PATCH_SIZE:?};")));
        assert!(shader.contains(&format!("const PATCH_SPACING: f32 = {PATCH_SPACING:?};")));
        assert!(shader.contains(&format!("const FINE_SIZE: f32 = {FINE_SIZE:?};")));
        assert!(shader.contains(&format!("const FINE_SPACING: f32 = {FINE_SPACING:?};")));
    }

    /// The fine patch's edge must lie along the patch's rows of vertices, or the two would
    /// not meet, and its own vertices must fall on the patch's.
    #[test]
    fn the_fine_patch_fits_the_patch() {
        let squares = FINE_SIZE / PATCH_SPACING;
        assert_eq!(squares, squares.round());
        assert_eq!(
            (squares as u32) % 2,
            0,
            "its middle must be a vertex of the patch"
        );
        let fine = PATCH_SPACING / FINE_SPACING;
        assert_eq!(fine, fine.round());
        // Well inside where the patch eases its waves flat.
        let ahead = include_str!("../shaders/water.wgsl")
            .lines()
            .find_map(|line| line.strip_prefix("const FINE_AHEAD: f32 = "))
            .and_then(|rest| rest.trim_end_matches(';').parse::<f32>().ok())
            .expect("water.wgsl sets FINE_AHEAD");
        assert!(FINE_SIZE / 2.0 + ahead < PATCH_SIZE / 2.0 * 0.7);
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
