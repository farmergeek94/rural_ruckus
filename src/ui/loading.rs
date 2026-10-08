//! The loading screen: covers the whole window while the game gets something ready, and
//! says what it is and what is being done. It is drawn while `LoadingScreen` holds a view,
//! whether the front end is open or not, so that it can stay up while the front end closes
//! and a race is built behind it. The game takes it down.
//!
//! The screen is built once, as it goes up. After that only its text changes, as the game
//! says what it is doing. Nothing on it moves: the game loads and builds a race in long
//! frames, in which nothing could.
//!
//! It has a camera of its own, drawn after any other, so that it shows whatever else is
//! there or not: the front end closes, and takes its camera with it, as the screen comes
//! up, and the race's camera comes only once the race is built.
//!
//! Sizes are parts of the window's height, not pixels: `UiScale` fits the front end to the
//! window and is put back as the front end closes, and the screen must not change size as
//! it does.

use bevy::prelude::*;

use super::theme::DESIGNED_FOR_HEIGHT;
use super::{Loading, LoadingScreen, Theme};

/// Over everything else on screen, the race's pause dialog among it.
const LOADING_LAYER: i32 = 100;
/// After every other camera: the race's draws first, under the screen.
const LOADING_CAMERA_ORDER: isize = 100;
/// The window's background colour under the loading screen: the front end's panels with
/// nothing showing through.
const BACKDROP: Color = Color::srgb(0.035, 0.045, 0.07);

/// The root of the loading screen, and its camera.
#[derive(Component)]
pub(super) struct LoadingRoot;

/// One line of the loading screen's text, which follows the view.
#[derive(Component, Clone, Copy)]
pub(super) enum Line {
    Title,
    Detail,
    Step,
}

impl Line {
    fn of(self, loading: &Loading) -> &str {
        match self {
            Line::Title => &loading.title,
            Line::Detail => &loading.detail,
            Line::Step => &loading.step,
        }
    }
}

/// A size on a screen 1080 high, as a part of this window's height.
fn tall(pixels: f32) -> Val {
    Val::Vh(pixels / DESIGNED_FOR_HEIGHT * 100.0)
}

fn text(font: &Option<Handle<Font>>, pixels: f32, colour: Color) -> (TextFont, TextColor) {
    let size = TextFont::from_font_size(bevy::text::FontSize::Vh(
        pixels / DESIGNED_FOR_HEIGHT * 100.0,
    ));
    let font = match font {
        Some(handle) => size.with_font(handle.clone()),
        None => size,
    };
    (font, TextColor(colour))
}

/// Puts the screen up, takes it down, and changes its text to follow the view. After the
/// game's systems in `Update`, so that a change shows in the same frame.
pub(super) fn redraw(
    mut commands: Commands,
    screen: Res<LoadingScreen>,
    theme: Res<Theme>,
    roots: Query<Entity, With<LoadingRoot>>,
    mut lines: Query<(&Line, &mut Text)>,
) {
    if !screen.is_changed() && !theme.is_changed() {
        return;
    }
    let Some(loading) = &screen.0 else {
        for root in &roots {
            commands.entity(root).despawn();
        }
        return;
    };
    if roots.is_empty() || theme.is_changed() {
        for root in &roots {
            commands.entity(root).despawn();
        }
        spawn(&mut commands, loading, &theme);
        return;
    }
    for (line, mut text) in &mut lines {
        let wanted = line.of(loading);
        if text.0 != wanted {
            text.0 = wanted.to_string();
        }
    }
}

fn spawn(commands: &mut Commands, loading: &Loading, theme: &Theme) {
    let line = |line: Line, font: &Option<Handle<Font>>, size: f32, colour: Color| {
        (line, Text::new(line.of(loading)), text(font, size, colour))
    };
    let camera = commands
        .spawn((
            Name::new("Loading screen camera"),
            LoadingRoot,
            Camera2d,
            Camera {
                order: LOADING_CAMERA_ORDER,
                clear_color: ClearColorConfig::Custom(BACKDROP),
                ..default()
            },
        ))
        .id();
    commands
        .spawn((
            Name::new("Loading screen"),
            LoadingRoot,
            UiTargetCamera(camera),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: tall(theme.padding),
                ..default()
            },
            BackgroundColor(BACKDROP),
            GlobalZIndex(LOADING_LAYER),
            // Takes every click, so that nothing under it is chosen while it is up.
            Pickable::default(),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("LOADING"),
                text(&theme.heading_font, theme.heading_size, theme.accent),
            ));
            root.spawn(line(
                Line::Title,
                &theme.heading_font,
                theme.title_size * 1.5,
                theme.text,
            ));
            root.spawn(line(
                Line::Detail,
                &theme.text_font,
                theme.text_size,
                theme.text_dim,
            ));
            root.spawn((
                line(
                    Line::Step,
                    &theme.text_font,
                    theme.small_size,
                    theme.text_dim,
                ),
                // Set apart from what is being loaded, as the bar did.
                Node {
                    margin: UiRect::top(tall(theme.margin)),
                    ..default()
                },
            ));
        });
}
