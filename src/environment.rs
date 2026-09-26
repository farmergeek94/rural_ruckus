//! Sky colour and sunlight. The sun belongs to a race: a showroom has lights of its own.
//!
//! This is the sky of a clear day. The `weather` slice changes the sky colour and how
//! brightly the sun (`Sun`) shines, from `SKY` and `SUNLIGHT`.

use bevy::light::{CascadeShadowConfig, CascadeShadowConfigBuilder};
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

use crate::game_state::GameState;

pub struct EnvironmentPlugin;

impl Plugin for EnvironmentPlugin {
    fn build(&self, app: &mut App) {
        // A slice works alone, so it makes sure of the state it is built in. A second
        // `init_state` would log a warning.
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        if !app.world().contains_resource::<State<GameState>>() {
            app.init_state::<GameState>();
        }
        app.insert_resource(ClearColor(SKY))
            .init_resource::<EnvironmentSettings>()
            .add_systems(OnEnter(GameState::Racing), spawn_sun)
            .add_systems(
                Update,
                apply_settings.run_if(resource_changed::<EnvironmentSettings>),
            );
    }
}

/// The colour of a clear sky.
pub const SKY: Color = Color::srgb(0.53, 0.74, 0.93);
/// Where the sun of a clear day is: the way to it from the middle of the track. `weather`
/// moves it for other times of day.
pub const SUN_FROM: Vec3 = Vec3::new(60.0, 100.0, 40.0);
/// How brightly the sun shines on a clear day, in lux.
pub const SUNLIGHT: f32 = light_consts::lux::OVERCAST_DAY * 4.0;

/// Choices about the sun's shadows. Insert it before adding `EnvironmentPlugin`, or change
/// it while racing.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct EnvironmentSettings {
    /// How many shadow maps the sun's shadows are drawn into, the one nearest the camera
    /// the finest. Everything within reach of the shadows is drawn once more for each.
    /// 0 is no shadows at all.
    pub shadow_cascades: usize,
    /// How far from the camera shadows reach, in metres. Further takes in more to draw,
    /// and spreads the same shadow maps thinner.
    pub shadow_distance: f32,
}

impl Default for EnvironmentSettings {
    /// Bevy's own defaults.
    fn default() -> Self {
        Self {
            shadow_cascades: 4,
            shadow_distance: 150.0,
        }
    }
}

/// Where the nearest, finest shadow map ends, in metres from the camera: Bevy's default.
const FIRST_CASCADE_FAR_BOUND: f32 = 10.0;

impl EnvironmentSettings {
    fn shadows(&self) -> bool {
        self.shadow_cascades > 0
    }

    fn cascades(&self) -> CascadeShadowConfig {
        CascadeShadowConfigBuilder {
            // The builder refuses none at all, and a reach inside the first cascade.
            num_cascades: self.shadow_cascades.max(1),
            maximum_distance: self.shadow_distance.max(2.0 * FIRST_CASCADE_FAR_BOUND),
            first_cascade_far_bound: FIRST_CASCADE_FAR_BOUND,
            ..default()
        }
        .build()
    }
}

/// On the sun: the one directional light of a race.
#[derive(Component)]
pub struct Sun;

fn spawn_sun(mut commands: Commands, settings: Res<EnvironmentSettings>) {
    commands.spawn((
        Sun,
        DespawnOnExit(GameState::Racing),
        DirectionalLight {
            illuminance: SUNLIGHT,
            shadow_maps_enabled: settings.shadows(),
            ..default()
        },
        settings.cascades(),
        Transform::from_translation(SUN_FROM).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn apply_settings(
    settings: Res<EnvironmentSettings>,
    mut suns: Query<(&mut DirectionalLight, &mut CascadeShadowConfig), With<Sun>>,
) {
    for (mut light, mut cascades) in &mut suns {
        light.shadow_maps_enabled = settings.shadows();
        *cascades = settings.cascades();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_become_cascades() {
        let settings = EnvironmentSettings {
            shadow_cascades: 2,
            shadow_distance: 80.0,
        };
        assert!(settings.shadows());
        let bounds = settings.cascades().bounds;
        assert_eq!(bounds.len(), 2);
        // The bounds are worked out as powers, so only nearly.
        assert!((bounds[1] - 80.0).abs() < 0.01);
    }

    #[test]
    fn no_cascades_is_no_shadows_and_nothing_the_builder_refuses() {
        let settings = EnvironmentSettings {
            shadow_cascades: 0,
            shadow_distance: 0.0,
        };
        assert!(!settings.shadows());
        assert_eq!(settings.cascades().bounds.len(), 1);
    }
}
