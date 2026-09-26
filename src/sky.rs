//! The sky: a picture of it on a dome round the camera, behind the backdrop and everything
//! else, which the weather and the time of day choose.
//!
//! A track names its own sky, which is the sky of clear weather by day; the base game has
//! a cloudy one, a dusk one and a night one (`TrackData::skies`). Overcast weather by day
//! shows the cloudy sky, and night the night one. The dusk sky is a sunset, so only a clear
//! dusk shows it. Otherwise, and in fog, rain, a storm and snow, there is none: the
//! weather's plain sky colour and fog stand for the sky, as before.
//!
//! **Measured** (see `docs/formats/level.md`): a sky picture's top row is the top of the
//! sky and its bottom row the horizon, and its left and right edges meet, so it is laid
//! round the horizon. How MTM2 laid it on the sky, how many times round and how high, is
//! **open**: here it goes round `REPEATS` times on a dome from overhead to the horizon, and
//! its bottom row goes on below the horizon, where only a gap between backdrop hills shows
//! it. Like the backdrop, it is kept centred on the camera and as large as fits inside the
//! camera's far plane, a little further out than the backdrop, and it is unlit and
//! unfogged.
//!
//! Uses the `track` slice for the pictures, the `camera` slice for where it is, and the
//! `weather` slice for which to show. Does nothing in an app that cannot draw.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::PbrPlugin;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::camera::{CameraSystems, ChaseCamera};
use crate::game_state::GameState;
use crate::track::{SkyPicture, Track, TrackSystems};
use crate::weather::{TimeOfDay, Weather, WeatherSettings};

/// How far out the dome is, as a share of the distance to the camera's far plane: further
/// than the backdrop's 0.9, so that it is behind it, and short of 1, so that none of it is
/// cut off.
const FAR_SHARE: f32 = 0.95;
/// How many times the picture goes round the horizon. More makes its clouds smaller.
const REPEATS: f32 = 4.0;
/// How far below the horizon the dome reaches, in radians, carrying on the horizon's
/// colour, so that the sky has no edge above a low backdrop.
const BELOW_HORIZON: f32 = 0.35;
/// How finely the dome is made: rings from overhead down, and segments round.
const RINGS: usize = 24;
const SEGMENTS: usize = 64;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        // Only a look. An app that cannot draw has no use for it.
        if !app.is_plugin_added::<PbrPlugin>() {
            return;
        }
        app.add_systems(
            OnEnter(GameState::Racing),
            spawn_sky.after(TrackSystems::Prepare),
        )
        .add_systems(
            Update,
            (follow_the_camera.after(CameraSystems::Place), show_the_sky)
                .run_if(in_state(GameState::Racing)),
        );
    }
}

/// Which of a track's skies to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Which {
    Own,
    Cloudy,
    Dusk,
    Night,
}

/// The sky for `weather` at `time`. `None` for weather that hides the sky.
fn which(weather: Weather, time: TimeOfDay) -> Option<Which> {
    match (weather, time) {
        (Weather::Fog | Weather::Rain | Weather::Storm | Weather::Snow, _) => None,
        (Weather::Clear, TimeOfDay::Dusk) => Some(Which::Dusk),
        (Weather::Overcast, TimeOfDay::Dusk) => None,
        (_, TimeOfDay::Night) => Some(Which::Night),
        (Weather::Overcast, TimeOfDay::Day) => Some(Which::Cloudy),
        (Weather::Clear, TimeOfDay::Day) => Some(Which::Own),
    }
}

/// On the dome, with the track's pictures made into images.
#[derive(Component)]
struct Dome {
    material: Handle<StandardMaterial>,
    own: Option<Handle<Image>>,
    cloudy: Option<Handle<Image>>,
    dusk: Option<Handle<Image>>,
    night: Option<Handle<Image>>,
}

impl Dome {
    fn picture(&self, which: Which) -> Option<&Handle<Image>> {
        match which {
            Which::Own => self.own.as_ref(),
            Which::Cloudy => self.cloudy.as_ref(),
            Which::Dusk => self.dusk.as_ref(),
            Which::Night => self.night.as_ref(),
        }
    }
}

fn spawn_sky(
    mut commands: Commands,
    track: Res<Track>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let skies = &track.skies;
    let mut image = |picture: &Option<SkyPicture>| {
        picture
            .as_ref()
            .map(|picture| images.add(sky_image(picture)))
    };
    let dome = Dome {
        material: materials.add(StandardMaterial {
            unlit: true,
            fog_enabled: false,
            // Seen from inside.
            cull_mode: None,
            ..default()
        }),
        own: image(&skies.own),
        cloudy: image(&skies.cloudy),
        dusk: image(&skies.dusk),
        night: image(&skies.night),
    };
    if [&dome.own, &dome.cloudy, &dome.dusk, &dome.night]
        .iter()
        .all(|picture| picture.is_none())
    {
        return;
    }
    commands.spawn((
        Name::new("Sky"),
        Mesh3d(meshes.add(dome_mesh())),
        MeshMaterial3d(dome.material.clone()),
        dome,
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
        NotShadowReceiver,
        DespawnOnExit(GameState::Racing),
    ));
}

/// Shows the picture the weather and the time of day call for, or no dome at all.
fn show_the_sky(
    weather: Res<WeatherSettings>,
    mut domes: Query<(&Dome, &mut Visibility)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (dome, mut visibility) in &mut domes {
        let picture = which(weather.weather, weather.time_of_day)
            .and_then(|which| dome.picture(which))
            .cloned();
        visibility.set_if_neq(if picture.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        // Only a real change, so that the material isn't sent again every frame.
        if picture.is_some()
            && materials
                .get(&dome.material)
                .is_some_and(|material| material.base_color_texture != picture)
            && let Some(mut material) = materials.get_mut(&dome.material)
        {
            material.base_color_texture = picture;
        }
    }
}

type RaceCamera<'w, 's> =
    Query<'w, 's, (&'static Transform, &'static Projection), (With<ChaseCamera>, Without<Dome>)>;

/// Centred on the camera, and as large as fits inside its far plane.
fn follow_the_camera(cameras: RaceCamera, mut domes: Query<&mut Transform, With<Dome>>) {
    let Ok((eye, projection)) = cameras.single() else {
        return;
    };
    let Projection::Perspective(perspective) = projection else {
        return;
    };
    for mut transform in &mut domes {
        transform.translation = eye.translation;
        transform.scale = Vec3::splat(FAR_SHARE * perspective.far);
    }
}

/// A dome of radius 1 round the origin, from overhead to `BELOW_HORIZON` below the
/// horizon, with the picture laid round it: across it `REPEATS` times round, and down it
/// from overhead to the horizon, below which it keeps the bottom row.
fn dome_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let lowest = -BELOW_HORIZON;
    let top = std::f32::consts::FRAC_PI_2;
    for ring in 0..=RINGS {
        let elevation = top - (top - lowest) * ring as f32 / RINGS as f32;
        for segment in 0..=SEGMENTS {
            let round = segment as f32 / SEGMENTS as f32;
            let azimuth = round * std::f32::consts::TAU;
            positions.push([
                elevation.cos() * azimuth.sin(),
                elevation.sin(),
                elevation.cos() * azimuth.cos(),
            ]);
            uvs.push([round * REPEATS, sky_row(elevation)]);
        }
    }
    let mut indices = Vec::new();
    let across = SEGMENTS as u32 + 1;
    for ring in 0..RINGS as u32 {
        for segment in 0..SEGMENTS as u32 {
            let a = ring * across + segment;
            let b = a + across;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// How far down the picture the sky at `elevation` radians above the horizon is, from 0
/// (overhead: the top row) to 1 (the horizon and below: the bottom row).
fn sky_row(elevation: f32) -> f32 {
    (1.0 - elevation / std::f32::consts::FRAC_PI_2).clamp(0.0, 1.0)
}

/// The picture as an image that repeats across and stops at its top and bottom rows.
fn sky_image(picture: &SkyPicture) -> Image {
    let size = picture.size as u32;
    let mut image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        picture.rgba.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_weather_and_the_time_choose_the_sky() {
        assert_eq!(which(Weather::Clear, TimeOfDay::Day), Some(Which::Own));
        assert_eq!(
            which(Weather::Overcast, TimeOfDay::Day),
            Some(Which::Cloudy)
        );
        assert_eq!(which(Weather::Clear, TimeOfDay::Dusk), Some(Which::Dusk));
        // The dusk sky is an orange sunset, which cloud hides.
        assert_eq!(which(Weather::Overcast, TimeOfDay::Dusk), None);
        assert_eq!(
            which(Weather::Overcast, TimeOfDay::Night),
            Some(Which::Night)
        );
        for hidden in [Weather::Fog, Weather::Rain, Weather::Storm, Weather::Snow] {
            assert_eq!(which(hidden, TimeOfDay::Day), None);
        }
    }

    #[test]
    fn the_top_row_is_overhead_and_the_bottom_row_the_horizon_and_below() {
        assert_eq!(sky_row(std::f32::consts::FRAC_PI_2), 0.0);
        assert_eq!(sky_row(0.0), 1.0);
        assert_eq!(sky_row(-0.2), 1.0);
        assert!((sky_row(std::f32::consts::FRAC_PI_4) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn the_dome_is_round_and_meets_itself() {
        let mesh = dome_mesh();
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };
        for position in positions {
            assert!((Vec3::from(*position).length() - 1.0).abs() < 1e-5);
        }
        // The first and last segment of a ring are in one place, with the picture's left
        // and right edges on them.
        let first = Vec3::from(positions[SEGMENTS + 1]);
        let last = Vec3::from(positions[(SEGMENTS + 1) + SEGMENTS]);
        assert!(first.abs_diff_eq(last, 1e-5));
    }
}
