//! In snow the water is frozen: a flat sheet of ice at the water's level, which trucks
//! drive on as on the ground.
//!
//! The ice is one thin, solid slab over the whole track, its top at the water's level.
//! Where there is land, the land is above it, so the wheels and the bodies meet the land
//! first and never the ice. It is in the ground's collision group, so that the wheels
//! treat it as the ground (see `truck/contacts.rs`): a tipped truck rests on it, and it
//! never pushes a truck down through itself.
//!
//! The slab is spawned with every race on a track with water, and is switched on and off
//! with the weather, so that F7 freezes and thaws it in a race. While it is frozen, the
//! liquid surface is hidden and the water neither holds trucks up nor splashes. How much a
//! tire grips on it is the `footing` slice's.
//!
//! A truck that is under the water when it freezes stays under, and drives along the
//! bottom until it comes out at the shore: the ice has no underside to hold it down.
//!
//! Monster Truck Madness 2 has no frozen water that is known. The look and the values are
//! the game's own.

use avian3d::prelude::*;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::WaterSettings;
use super::surface::{FlatWater, WaterMaterial};
use crate::game_state::GameState;
use crate::track::Track;
use crate::weather::WeatherSettings;

/// How thick the slab of ice is, in metres, down from the water's level. Thick enough
/// that a truck dropped onto it from a jump can not go through in one physics step;
/// thicker would catch more of a truck that is under the water when it freezes.
const THICKNESS: f32 = 1.0;
/// How the ice rubs on what slides over it that is not a tire: a truck on its roof or
/// its side. The tires' grip is the `footing` slice's. The ground's is 1. Lower slides
/// further.
const FRICTION: f32 = 0.05;
/// What the ice looks like: white with a little blue, and smooth, so that the sun glints
/// off it.
const ICE_COLOR: Color = Color::srgb(0.78, 0.87, 0.93);
const ICE_ROUGHNESS: f32 = 0.2;
/// How big the squares of the drawn sheet are, in metres. Across one enormous triangle
/// the depth test at the shore is not exact, and the shore line would wander.
const SHEET_SPACING: f32 = 20.0;

/// On the slab of ice.
#[derive(Component)]
pub(super) struct Ice;

/// Whether the water is frozen now. Without the weather, it is not.
pub(super) fn frozen(weather: Option<Res<WeatherSettings>>) -> bool {
    weather.is_some_and(|weather| weather.weather.freezes_water())
}

/// Spawns the slab, switched off. `freeze_or_thaw` switches it on.
pub(super) fn spawn_ice(
    mut commands: Commands,
    track: Res<Track>,
    // Both absent in a headless app, which has no use for looks.
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let Some(level) = track.water_level else {
        return;
    };
    let size = track.heights.size();
    let ice = commands
        .spawn((
            Name::new("Ice"),
            Ice,
            Transform::from_xyz(0.0, level, 0.0),
            Visibility::Hidden,
            DespawnOnExit(GameState::Racing),
            children![(
                Name::new("Ice slab"),
                // The collider is a child, so that its top, not its middle, is at the level.
                Transform::from_xyz(0.0, -THICKNESS / 2.0, 0.0),
                RigidBody::Static,
                Collider::cuboid(size, THICKNESS, size),
                ColliderDisabled,
                crate::collision_groups::ground(),
                Friction::new(FRICTION).with_combine_rule(CoefficientCombine::Min),
                Restitution::new(0.0).with_combine_rule(CoefficientCombine::Min),
            )],
        ))
        .id();
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    let squares = (size / SHEET_SPACING).ceil() as u32;
    commands.entity(ice).insert((
        Mesh3d(
            meshes.add(
                Plane3d::new(Vec3::Y, Vec2::splat(size / 2.0))
                    .mesh()
                    .subdivisions(squares - 1),
            ),
        ),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: ICE_COLOR,
            perceptual_roughness: ICE_ROUGHNESS,
            ..default()
        })),
        // Nothing is under it to shade, and on land it is under the ground.
        NotShadowCaster,
    ));
}

/// What draws the water: the moving water's two meshes, and the flat plane.
type DrawnWater = (
    Or<(With<MeshMaterial3d<WaterMaterial>>, With<FlatWater>)>,
    Without<Ice>,
);

/// Makes the ice solid and shows it while the water is frozen, and hides it and shows the
/// water while it is not: the moving water, or the flat plane where `WaterSettings::flat`
/// says. Only on a change, so that the physics is not told of one every step.
pub(super) fn freeze_or_thaw(
    mut commands: Commands,
    weather: Option<Res<WeatherSettings>>,
    settings: Res<WaterSettings>,
    mut ice: Query<(&mut Visibility, &Children), With<Ice>>,
    slabs: Query<Has<ColliderDisabled>, With<Collider>>,
    // Absent in a headless app, which draws no water.
    mut water: Query<(&mut Visibility, Has<FlatWater>), DrawnWater>,
) {
    let frozen = frozen(weather);
    for (mut visibility, children) in &mut ice {
        for &slab in children {
            let Ok(disabled) = slabs.get(slab) else {
                continue;
            };
            if disabled == frozen {
                if frozen {
                    commands.entity(slab).remove::<ColliderDisabled>();
                } else {
                    commands.entity(slab).insert(ColliderDisabled);
                }
            }
        }
        visibility.set_if_neq(shown(frozen));
    }
    for (mut visibility, flat) in &mut water {
        visibility.set_if_neq(shown(!frozen && flat == settings.flat));
    }
}

fn shown(show: bool) -> Visibility {
    if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}
