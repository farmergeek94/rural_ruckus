//! Under the water, the camera sees through it: a blue-green tint over the whole picture
//! while the camera is below the water's level, as the water's colour would tint the view
//! from inside it. Drawn as a layer of the interface under everything else on it, so that
//! the race's own displays stay clear, and so that the fog, which the weather sets, is left
//! alone. Not while the water is frozen: the camera is then under ice, not in water.

use bevy::prelude::*;

use super::ice;
use crate::game_state::GameState;
use crate::track::Track;
use crate::weather::WeatherSettings;

/// The tint, over everything the camera sees under the water. A higher alpha hides the
/// scene more; a darker colour makes the picture darker.
const UNDERWATER_TINT: Color = Color::srgba(0.1, 0.45, 0.6, 0.55);

/// On the layer that tints the picture under water.
#[derive(Component)]
pub(super) struct UnderwaterTint;

pub(super) fn spawn_tint(mut commands: Commands, track: Res<Track>) {
    if track.water_level.is_none() {
        return;
    }
    commands.spawn((
        Name::new("Underwater tint"),
        UnderwaterTint,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(UNDERWATER_TINT),
        // Under the rest of the interface.
        GlobalZIndex(-1),
        Pickable::IGNORE,
        Visibility::Hidden,
        DespawnOnExit(GameState::Racing),
    ));
}

/// Shows the tint while a camera is under the water's surface, and hides it otherwise.
pub(super) fn tint_under_water(
    track: Res<Track>,
    weather: Option<Res<WeatherSettings>>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut tints: Query<&mut Visibility, With<UnderwaterTint>>,
) {
    let Some(level) = track.water_level else {
        return;
    };
    let under =
        !ice::frozen(weather) && cameras.iter().any(|camera| camera.translation().y < level);
    let wanted = if under {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut tints {
        visibility.set_if_neq(wanted);
    }
}
