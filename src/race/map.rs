//! A map of the track in the bottom right corner of the screen, with a dot on it for every
//! truck: a red one for the player's, and a blue one for each truck the computer drives,
//! with its place in the race in it, counted as the race readout counts the player's
//! (`position`), so that the two agree. The key for `keys::Control::Map` shows and hides
//! the map, and it stays as it is for the next race.
//!
//! The picture is `track::map_image`, painted once as the race begins, and the dots are put
//! on it by `track::map_frame`, the rule the picture was painted by. A dot follows the drawn
//! truck, which moves smoothly from frame to frame, and is moved by its `UiTransform`, which
//! does not make the UI lay itself out again.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::{BLUE, Racer, position};
use crate::game_state::GameState;
use crate::track::{MapFrame, Track, map_frame, map_image};
use crate::truck::{PlayerTruck, TruckVisual};

/// Width and height of the map on screen, in pixels, with its border.
const MAP_SIZE: f32 = 320.0;
const BORDER: f32 = 2.0;
/// Width and height of the painted picture, in pixels: about the map's size on screen, so
/// that it is sharp. Painting it takes 36 to 46 ms in the dev build (measured on the built-in
/// track and three POD tracks), once a race. Twice the size takes four times as long.
const PICTURE_SIZE: usize = 320;
/// How much of the track shows through the map, from 0 to 1.
const MAP_OPACITY: f32 = 0.85;

/// Width and height of the dot for a truck the computer drives, in pixels, and of the
/// number in it.
const DRIVER_DOT: f32 = 21.0;
const NUMBER_SIZE: f32 = 14.0;
/// Width and height of the player's dot, in pixels.
const PLAYER_DOT: f32 = 16.0;
const PLAYER_COLOR: Color = Color::srgb(0.95, 0.1, 0.1);
const DOT_RIM: Color = Color::WHITE;

/// Whether the map is on screen. Kept from race to race.
#[derive(Resource, Default)]
pub(super) struct MapShown(bool);

/// The map, and the square of the world it shows.
#[derive(Component)]
pub(super) struct RaceMap(MapFrame);

/// A truck's dot on the map.
#[derive(Component)]
pub(super) struct MapDot {
    truck: Entity,
}

/// The number in the dot of a truck the computer drives: its place in the race.
#[derive(Component)]
pub(super) struct DotNumber {
    truck: Entity,
    /// The place shown, so that the text is only written when it changes. 0 is none yet.
    shown: usize,
}

pub(super) fn spawn_map(
    mut commands: Commands,
    track: Res<Track>,
    shown: Res<MapShown>,
    // Absent in a headless app, which has nowhere to show a picture.
    images: Option<ResMut<Assets<Image>>>,
) {
    let mut map = commands.spawn((
        RaceMap(map_frame(&track)),
        DespawnOnExit(GameState::Racing),
        Node {
            position_type: PositionType::Absolute,
            right: px(16),
            bottom: px(16),
            width: px(MAP_SIZE),
            height: px(MAP_SIZE),
            border: UiRect::all(px(BORDER)),
            ..default()
        },
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.8)),
        visibility(shown.0),
    ));
    if let Some(mut images) = images {
        let picture = images.add(Image::new(
            Extent3d {
                width: PICTURE_SIZE as u32,
                height: PICTURE_SIZE as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            map_image(&track, PICTURE_SIZE),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        ));
        map.insert(ImageNode::new(picture).with_color(Color::WHITE.with_alpha(MAP_OPACITY)));
    }
}

/// Shows the map, or hides it.
pub(super) fn toggle_map(
    mut shown: ResMut<MapShown>,
    mut map: Single<&mut Visibility, With<RaceMap>>,
) {
    shown.0 = !shown.0;
    **map = visibility(shown.0);
}

fn visibility(shown: bool) -> Visibility {
    if shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

/// Gives each truck that has just joined the race its dot.
pub(super) fn add_dots(
    mut commands: Commands,
    map: Single<Entity, With<RaceMap>>,
    racers: Query<(Entity, Has<PlayerTruck>), Added<Racer>>,
) {
    for (truck, is_player) in &racers {
        let size = if is_player { PLAYER_DOT } else { DRIVER_DOT };
        let mut dot = commands.spawn((
            MapDot { truck },
            ChildOf(*map),
            Node {
                position_type: PositionType::Absolute,
                // Its middle on the map's top left corner, until it is moved to its truck.
                left: px(-size / 2.0),
                top: px(-size / 2.0),
                width: px(size),
                height: px(size),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::MAX,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(if is_player { PLAYER_COLOR } else { BLUE }),
            BorderColor::all(DOT_RIM),
            // Hidden until it is moved to its truck.
            Visibility::Hidden,
            UiTransform::default(),
        ));
        if is_player {
            // Over the others, so that the player can always find it.
            dot.insert(ZIndex(1));
        } else {
            dot.with_child((
                DotNumber { truck, shown: 0 },
                Text::default(),
                TextFont::from_font_size(NUMBER_SIZE),
                TextColor(Color::WHITE),
            ));
        }
    }
}

/// Puts each dot where its drawn truck is. A truck off the map is shown on its edge.
pub(super) fn place_dots(
    map: Single<(&RaceMap, &Visibility)>,
    visuals: Query<(&TruckVisual, &Transform)>,
    mut dots: Query<(&MapDot, &mut UiTransform, &mut Visibility), Without<RaceMap>>,
) {
    let (RaceMap(frame), map_visibility) = *map;
    if *map_visibility == Visibility::Hidden {
        return;
    }
    // The picture inside the border, which is what the dots are placed against.
    let inside = MAP_SIZE - 2.0 * BORDER;
    for (dot, mut transform, mut visibility) in &mut dots {
        // A handful of trucks at most.
        let Some((_, truck)) = visuals.iter().find(|(visual, _)| visual.truck == dot.truck) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        visibility.set_if_neq(Visibility::Inherited);
        let place = frame
            .place(truck.translation.xz())
            .clamp(Vec2::ZERO, Vec2::ONE)
            * inside;
        transform.translation = Val2::px(place.x, place.y);
    }
}

/// Puts each computer's truck's place in the race in its dot.
pub(super) fn number_dots(
    track: Res<Track>,
    map: Single<&Visibility, With<RaceMap>>,
    racers: Query<(Entity, &Racer)>,
    mut numbers: Query<(&mut DotNumber, &mut Text)>,
) {
    if **map == Visibility::Hidden {
        return;
    }
    for (mut number, mut text) in &mut numbers {
        let Ok((_, racer)) = racers.get(number.truck) else {
            continue;
        };
        // A handful of trucks at most, so each is compared with every other.
        let place = position(
            (&racer.progress, racer.to_next_gate),
            racers
                .iter()
                .filter(|(other, _)| *other != number.truck)
                .map(|(_, other)| (&other.progress, other.to_next_gate)),
            track.gates.len(),
        );
        if number.shown != place {
            number.shown = place;
            text.0 = place.to_string();
        }
    }
}
