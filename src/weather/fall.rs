//! Rain and snow round the camera.
//!
//! Every drop or flake has a place of its own in a box `FALL_BOX` across, which moves on as
//! it falls and is blown along. The box is laid over the world again and again, like tiles,
//! and the one round the camera is drawn: a drop that goes out of it at the bottom comes in
//! again at the top, and as the camera moves on, the drops ahead come into its box and
//! those behind go out. So the rain keeps to its place in the world, and the camera goes
//! through it.
//!
//! All of them are one mesh, a small square each, made once. `fall.wgsl` places every
//! corner of it each frame, on the graphics card, from the drop's place, the time and the
//! camera: done here, for thousands of drops, it cost 4 to 6 ms a frame in a debug build.
//! A raindrop is a thin streak along the way it falls, as long as the way it goes in
//! `EXPOSURE`, turned about its length to face the camera. A snowflake is a soft round
//! square turned to the camera, which sways as it falls. What is under the ground is hidden
//! by the ground. Near the camera, both look smaller the nearer they come, so that none go
//! past as big blobs. The mesh's entity is kept at the camera, so that it is sorted with
//! what is nearest.
//!
//! They are not lit, as squares turned to the camera would light oddly. So that snow does
//! not shine white in the dark, they are as bright as `ParticleLight` says, as the dust
//! and the spray are.
//!
//! The shader's clock (`globals.time`) goes back to 0 every hour, when the drops jump once.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat,
};
use bevy::shader::ShaderRef;

use super::WeatherSettings;
use super::conditions::{Fall, FallKind};
use crate::camera::ChaseCamera;
use crate::game_state::GameState;
use crate::particles::ParticleLight;

/// The size of the box of rain round the camera, in metres: across, up and along. Fog
/// hides what is further off, and a bigger box spreads the same drops thinner.
const FALL_BOX: Vec3 = Vec3::new(40.0, 24.0, 40.0);
/// How far above the camera the middle of the box is, in metres: more falls from above
/// than can be seen below the camera.
const BOX_RAISED: f32 = 4.0;
/// Which way the wind carries the rain and snow, on the ground plane (world X and Z): the
/// way the water's wind blows.
const WIND_TOWARDS: Vec2 = Vec2::new(0.8, -0.6);
/// How much the speed of one drop differs from another's, as a share either side.
const SPEED_SPREAD: f32 = 0.15;

/// How long the eye sees a raindrop for, in seconds: a drop is drawn as a streak as long
/// as the way it goes in that time.
const EXPOSURE: f32 = 0.05;
/// How wide a raindrop's streak is, in metres.
const RAIN_WIDTH: f32 = 0.02;
/// The colour of rain, in sRGB from 0 to 1, and how solid its streaks are, from 0 (not
/// there) to 1.
const RAIN_COLOR: Vec3 = Vec3::new(0.75, 0.79, 0.85);
const RAIN_OPACITY: f32 = 0.55;

/// How big a snowflake is, in metres of radius: the smallest and the largest.
const FLAKE_SIZE: [f32; 2] = [0.03, 0.06];
/// How far a snowflake sways from side to side as it falls, in metres, and how fast, in
/// radians a second.
const SWAY: f32 = 0.35;
const SWAY_RATE: f32 = 1.6;
const SNOW_OPACITY: f32 = 0.9;

/// How near the camera a drop starts to look smaller the nearer it comes, in metres.
const NEAR_CAMERA: f32 = 3.0;

/// Where `embedded_asset!` puts `fall.wgsl`: the crate's name, then the path below `src`.
const SHADER_PATH: &str = "embedded://monster_truck_rural_ruckus/weather/fall.wgsl";

pub(super) type FallMaterial = ExtendedMaterial<StandardMaterial, FallMotion>;

/// How the drops move, for `fall.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(super) struct FallMotion {
    #[uniform(100)]
    fall: FallUniform,
}

impl MaterialExtension for FallMotion {
    fn vertex_shader() -> ShaderRef {
        SHADER_PATH.into()
    }
}

/// As `Fall` in `fall.wgsl`, where each is explained.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
struct FallUniform {
    velocity: Vec3,
    snow: u32,
    box_size: Vec3,
    raised: f32,
    exposure: f32,
    width: f32,
    speed_spread: f32,
    near_camera: f32,
    flake_size: Vec2,
    sway: f32,
    sway_rate: f32,
}

impl FallUniform {
    fn new(fall: Fall) -> Self {
        let wind = WIND_TOWARDS.normalize() * fall.wind;
        Self {
            velocity: Vec3::new(wind.x, -fall.speed, wind.y),
            snow: (fall.kind == FallKind::Snow) as u32,
            box_size: FALL_BOX,
            raised: BOX_RAISED,
            exposure: EXPOSURE,
            width: RAIN_WIDTH,
            speed_spread: SPEED_SPREAD,
            near_camera: NEAR_CAMERA,
            flake_size: Vec2::from(FLAKE_SIZE),
            sway: SWAY,
            sway_rate: SWAY_RATE,
        }
    }
}

/// The pictures of a raindrop and a snowflake.
#[derive(Resource)]
pub(super) struct FallLooks {
    streak: Handle<Image>,
    flake: Handle<Image>,
}

/// On the mesh of rain or snow: what is falling, and the mesh and material made for it.
#[derive(Component)]
pub(super) struct Falling {
    fall: Fall,
    /// Its colour and opacity in full daylight.
    color: Color,
    mesh: Handle<Mesh>,
    material: Handle<FallMaterial>,
}

pub(super) fn make_looks(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(FallLooks {
        streak: images.add(picture(streak_opacity)),
        flake: images.add(picture(flake_opacity)),
    });
}

/// Starts the rain or snow that the weather asks for, and stops what it no longer does.
pub(super) fn start_falling(
    mut commands: Commands,
    settings: Res<WeatherSettings>,
    looks: Res<FallLooks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FallMaterial>>,
    falling: Query<(Entity, &Falling)>,
) {
    let wanted = settings.weather.conditions().fall;
    let mut already = false;
    for (entity, falling) in &falling {
        if Some(falling.fall) == wanted {
            already = true;
        } else {
            meshes.remove(&falling.mesh);
            materials.remove(&falling.material);
            commands.entity(entity).despawn();
        }
    }
    let Some(fall) = wanted.filter(|_| !already) else {
        return;
    };
    let (picture, color, opacity) = match fall.kind {
        FallKind::Rain => (looks.streak.clone(), RAIN_COLOR, RAIN_OPACITY),
        FallKind::Snow => (looks.flake.clone(), Vec3::ONE, SNOW_OPACITY),
    };
    let color = Color::srgba(color.x, color.y, color.z, opacity);
    let material = materials.add(FallMaterial {
        base: StandardMaterial {
            base_color: color,
            base_color_texture: Some(picture),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            // Seen from either side: a streak turns its face to the camera only about its
            // length.
            cull_mode: None,
            ..default()
        },
        extension: FallMotion {
            fall: FallUniform::new(fall),
        },
    });
    let mesh = meshes.add(squares(fall.count, &mut Random::default()));
    commands.spawn((
        Falling {
            fall,
            color,
            mesh: mesh.clone(),
            material: material.clone(),
        },
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::default(),
        // Placed by the shader round the camera, so never outside the view as a whole.
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
        DespawnOnExit(GameState::Racing),
    ));
}

/// Keeps the mesh at the camera, so that it is sorted with what is nearest. The shader
/// places the drops themselves.
pub(super) fn follow_the_camera(
    cameras: Query<&Transform, (With<ChaseCamera>, Without<Falling>)>,
    mut falling: Query<&mut Transform, With<Falling>>,
) {
    let Ok(eye) = cameras.single() else {
        return;
    };
    for mut transform in &mut falling {
        transform.translation = eye.translation;
    }
}

/// Makes the rain or snow as bright as `ParticleLight` says. Only a real change is sent.
pub(super) fn light_the_fall(
    light: Option<Res<ParticleLight>>,
    falling: Query<&Falling>,
    mut materials: ResMut<Assets<FallMaterial>>,
) {
    let level = light.map_or(1.0, |light| light.0.clamp(0.0, 1.0));
    for falling in &falling {
        let color = dimmed(falling.color, level);
        if materials
            .get(&falling.material)
            .is_some_and(|material| material.base.base_color != color)
            && let Some(mut material) = materials.get_mut(&falling.material)
        {
            material.base.base_color = color;
        }
    }
}

/// `color` as bright as `level` (0 to 1) says, as light is: its opacity kept.
fn dimmed(color: Color, level: f32) -> Color {
    let linear = color.to_linear();
    Color::linear_rgba(
        linear.red * level,
        linear.green * level,
        linear.blue * level,
        linear.alpha,
    )
}

/// A mesh of `count` squares, one for each drop. Every corner of a square carries the
/// drop's place in the box, as a share of it across, up and along (its position), which
/// corner it is (its UV, the top of the picture at the first two), and a number of the
/// drop's own (the first of its second UV).
fn squares(count: usize, random: &mut Random) -> Mesh {
    let mut places = Vec::with_capacity(count * 4);
    let mut owns = Vec::with_capacity(count * 4);
    for _ in 0..count {
        let place = [random.next(), random.next(), random.next()];
        let own = random.next();
        places.extend([place; 4]);
        owns.extend([[own, 0.0]; 4]);
    }
    let corners: Vec<[f32; 2]> = (0..count)
        .flat_map(|_| [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])
        .collect();
    let indices: Vec<u32> = (0..count as u32)
        .flat_map(|square| {
            let first = square * 4;
            [first, first + 2, first + 1, first, first + 3, first + 2]
        })
        .collect();
    // The graphics card alone needs it after this.
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, places)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, corners)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, owns)
    .with_inserted_indices(Indices::U32(indices))
}

/// How many texels the pictures have along a side.
const PICTURE_SIZE: u32 = 32;

/// A white picture, as solid as `opacity` says at each point of it, from -1 to 1 across
/// and up.
fn picture(opacity: fn(Vec2) -> f32) -> Image {
    let middle = PICTURE_SIZE as f32 / 2.0;
    let mut texels = Vec::with_capacity((PICTURE_SIZE * PICTURE_SIZE * 4) as usize);
    for row in 0..PICTURE_SIZE {
        for col in 0..PICTURE_SIZE {
            let point = Vec2::new(col as f32 + 0.5 - middle, middle - row as f32 - 0.5) / middle;
            let alpha = (opacity(point).clamp(0.0, 1.0) * 255.0).round() as u8;
            texels.extend_from_slice(&[255, 255, 255, alpha]);
        }
    }
    Image::new(
        Extent3d {
            width: PICTURE_SIZE,
            height: PICTURE_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        texels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// A raindrop's streak: solid down its middle, fading to the sides, and to its tail at the
/// top.
fn streak_opacity(point: Vec2) -> f32 {
    let across = (1.0 - point.x.abs()).max(0.0);
    let along = (1.0 - point.y) * 0.5;
    across * across * along.sqrt()
}

/// A snowflake: round, soft at the edge.
fn flake_opacity(point: Vec2) -> f32 {
    let out = point.length();
    (1.0 - out * out).max(0.0)
}

/// A small, quick source of numbers that only have to look random (xorshift).
struct Random(u64);

impl Default for Random {
    fn default() -> Self {
        Self(0x2545_F491_4F6C_DD1D)
    }
}

impl Random {
    /// A number from 0 up to 1.
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weather::Weather;

    #[test]
    fn every_square_has_four_corners_and_two_triangles() {
        let mesh = squares(3, &mut Random::default());
        assert_eq!(mesh.count_vertices(), 12);
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("no indices");
        };
        assert_eq!(indices.len(), 18);
        assert!(indices.iter().all(|&index| index < 12));
    }

    #[test]
    fn the_four_corners_of_a_drop_are_at_one_place_in_the_box() {
        use bevy::mesh::VertexAttributeValues::Float32x3;
        let mesh = squares(50, &mut Random::default());
        let Some(Float32x3(places)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
            panic!("no places");
        };
        for drop in places.as_chunks::<4>().0 {
            assert!(drop.iter().all(|corner| corner == &drop[0]));
            assert!(drop[0].iter().all(|share| (0.0..1.0).contains(share)));
        }
        // Not all at one place.
        assert_ne!(places[0], places[4]);
    }

    #[test]
    fn the_shader_reads_the_fall_in_the_order_it_is_written() {
        let shader = include_str!("fall.wgsl");
        let fields = [
            "velocity",
            "snow",
            "box_size",
            "raised",
            "exposure",
            "width",
            "speed_spread",
            "near_camera",
            "flake_size",
            "sway",
            "sway_rate",
        ];
        let struct_start = shader.find("struct Fall {").expect("struct Fall");
        let mut from = struct_start;
        for field in fields {
            let at = shader[from..]
                .find(&format!("    {field}: "))
                .unwrap_or_else(|| panic!("{field} out of order in fall.wgsl"));
            from += at + 1;
        }
        // And the uniform has those fields, as `FallUniform::new` fills them.
        let snow = FallUniform::new(Weather::Snow.conditions().fall.unwrap());
        assert_eq!(snow.snow, 1);
        assert!(snow.velocity.y < 0.0);
        let rain = FallUniform::new(Weather::Rain.conditions().fall.unwrap());
        assert_eq!(rain.snow, 0);
    }

    #[test]
    fn a_flake_in_the_dark_is_dark_but_as_solid() {
        let white = Color::srgba(1.0, 1.0, 1.0, SNOW_OPACITY);
        assert_eq!(dimmed(white, 1.0), white.to_linear().into());
        let night = dimmed(white, 0.1).to_linear();
        assert!(night.red < 0.11 && night.green < 0.11 && night.blue < 0.11);
        assert_eq!(night.alpha, SNOW_OPACITY);
    }

    #[test]
    fn a_streak_is_solid_in_the_middle_and_clear_at_its_sides_and_tail() {
        assert!(streak_opacity(Vec2::new(0.0, -0.5)) > 0.5);
        assert_eq!(streak_opacity(Vec2::new(1.0, 0.0)), 0.0);
        assert_eq!(streak_opacity(Vec2::new(0.0, 1.0)), 0.0);
        assert!(flake_opacity(Vec2::ZERO) > 0.9);
        assert_eq!(flake_opacity(Vec2::new(1.0, 1.0)), 0.0);
    }
}
