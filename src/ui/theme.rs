//! Every colour, size and font of the front end, in one place.
//!
//! The look follows Material Design 3: flat tonal surfaces over the dark showroom, one blue
//! accent for whatever is highlighted, and depth shown only by shadows (elevation), never by
//! gradients or bevels. Panels are cards with large rounded corners; buttons and list rows
//! are pills. Hovering lightens a widget, as Material's state layer does. Sizes are written for a screen 1080 high,
//! and `UiScale` makes them fit any other (see `fit_the_window`).

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// The height of the screen the theme's sizes were written for, in pixels.
pub(super) const DESIGNED_FOR_HEIGHT: f32 = 1080.0;

/// Replace it, before the front end opens, to change how everything looks.
#[derive(Resource, Clone, Debug)]
pub struct Theme {
    /// Panels, over the showroom. See-through enough to show there is a room behind.
    pub panel: Color,
    /// A row or a button that isn't highlighted, and the same with the pointer over it.
    pub row: Color,
    pub row_hovered: Color,
    /// Whatever is highlighted, and the GO button.
    pub accent: Color,
    pub accent_hovered: Color,
    /// Text on a panel, text on the accent, and text that matters less.
    pub text: Color,
    pub text_on_accent: Color,
    pub text_dim: Color,
    /// GO while there is nothing to race.
    pub disabled: Color,
    /// The shadows that lift a panel, and a button that is lit, off what is under them.
    pub shadow: Color,
    /// The thin line round a button or row, which gives it a shape against its panel, and
    /// the same round one that is lit.
    pub outline: Color,
    pub outline_lit: Color,

    /// Headings want a heavy slanted face and text a plain one. `None` is Bevy's built-in
    /// font, until a face we may ship has been chosen.
    pub heading_font: Option<Handle<Font>>,
    pub text_font: Option<Handle<Font>>,
    pub title_size: f32,
    pub heading_size: f32,
    pub text_size: f32,
    pub small_size: f32,
    pub go_size: f32,

    /// Distance from the window's edges to the panels, and between them.
    pub margin: f32,
    /// Space inside a panel, around what it holds.
    pub padding: f32,
    /// How wide the panels on the left are. The truck has the rest.
    pub panel_width: f32,
    pub row_height: f32,
    /// How wide the name of a setting is, on the options screen. Its values have the rest
    /// of the line.
    pub setting_label_width: f32,
    /// The most values a setting shows as a row of buttons, one for each. A setting with
    /// more is a stepper instead: the value in the middle, arrows either side, and a dot
    /// for each value under it.
    pub most_setting_buttons: usize,
    /// How many rows of a list are shown at once. More than this and the list moves
    /// under the highlight.
    pub list_rows: usize,
    /// Corners of small surfaces, such as the map and a line of the options screen.
    pub corner_radius: f32,
    /// Corners of a panel, and of GO.
    pub panel_radius: f32,
    /// The side of the track's map.
    pub map_size: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            panel: Color::srgba(0.07, 0.085, 0.12, 0.9),
            row: Color::srgba(0.6, 0.75, 1.0, 0.1),
            row_hovered: Color::srgba(0.6, 0.75, 1.0, 0.2),
            accent: Color::srgb(0.2, 0.5, 0.96),
            accent_hovered: Color::srgb(0.32, 0.59, 0.98),
            text: Color::srgb(0.9, 0.92, 0.96),
            text_on_accent: Color::WHITE,
            text_dim: Color::srgb(0.58, 0.63, 0.72),
            disabled: Color::srgba(1.0, 1.0, 1.0, 0.08),
            shadow: Color::srgba(0.0, 0.0, 0.0, 0.5),
            outline: Color::srgba(0.62, 0.74, 1.0, 0.3),
            outline_lit: Color::srgba(0.62, 0.8, 1.0, 0.75),
            heading_font: None,
            text_font: None,
            title_size: 44.0,
            heading_size: 26.0,
            text_size: 24.0,
            small_size: 18.0,
            go_size: 56.0,
            margin: 32.0,
            padding: 20.0,
            panel_width: 600.0,
            row_height: 46.0,
            setting_label_width: 170.0,
            most_setting_buttons: 5,
            list_rows: 12,
            corner_radius: 12.0,
            panel_radius: 24.0,
            map_size: 320.0,
        }
    }
}

impl Theme {
    pub(super) fn heading(&self, size: f32) -> TextFont {
        font(&self.heading_font, size)
    }

    pub(super) fn body(&self, size: f32) -> TextFont {
        font(&self.text_font, size)
    }

    /// A button or list row: an outlined pill, raised when it is `lit`, flat on its panel
    /// when not.
    pub(super) fn pill(&self, node: Node, lit: bool) -> (Node, BorderColor, BoxShadow) {
        (
            Node {
                border: UiRect::all(px(1)),
                // Bevy clamps a radius to half the shorter side, which makes a pill.
                border_radius: BorderRadius::all(px(999)),
                ..node
            },
            BorderColor::all(if lit { self.outline_lit } else { self.outline }),
            if lit {
                self.elevation(1)
            } else {
                BoxShadow::default()
            },
        )
    }

    /// A panel: a card with large corners, raised well off the showroom.
    pub(super) fn card(&self, node: Node) -> (Node, BoxShadow) {
        (
            Node {
                border_radius: BorderRadius::all(px(self.panel_radius)),
                ..node
            },
            self.elevation(3),
        )
    }

    /// Material's elevation: a wide, soft shadow for the height and a tight one for the edge,
    /// both growing with `level`. Bevy draws them in order, back to front.
    pub(super) fn elevation(&self, level: u8) -> BoxShadow {
        let level = f32::from(level);
        let alpha = self.shadow.alpha();
        BoxShadow(vec![
            ShadowStyle {
                color: self.shadow.with_alpha(alpha * 0.5),
                x_offset: px(0),
                y_offset: px(2.0 * level),
                spread_radius: px(level),
                blur_radius: px(6.0 * level),
            },
            ShadowStyle {
                color: self.shadow.with_alpha(alpha * 0.6),
                x_offset: px(0),
                y_offset: px(1),
                spread_radius: px(0),
                blur_radius: px(2.0 + level),
            },
        ])
    }
}

fn font(handle: &Option<Handle<Font>>, size: f32) -> TextFont {
    let font = TextFont::from_font_size(size);
    match handle {
        Some(handle) => font.with_font(handle.clone()),
        None => font,
    }
}

/// How a widget is coloured, with the pointer over it and without.
#[derive(Component, Clone, Copy)]
#[require(Hovered)]
pub(super) struct Look {
    pub(super) normal: Color,
    pub(super) hovered: Color,
}

pub(super) fn show_hovering(
    mut widgets: Query<(&Hovered, &Look, &mut BackgroundColor), Changed<Hovered>>,
) {
    for (hovered, look, mut background) in &mut widgets {
        background.0 = if hovered.0 { look.hovered } else { look.normal };
    }
}

/// Looks the same at 1440p and at 4K, and nothing is measured twice. A headless app has
/// neither a window nor a scale, and nothing to fit.
pub(super) fn fit_the_window(
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    scale: Option<ResMut<UiScale>>,
) {
    let (Some(window), Some(mut scale)) = (window, scale) else {
        return;
    };
    let wanted = window.height() / DESIGNED_FOR_HEIGHT;
    if wanted > 0.0 && scale.0 != wanted {
        scale.0 = wanted;
    }
}

/// What is drawn outside the front end was sized without it.
pub(super) fn forget_the_window(scale: Option<ResMut<UiScale>>) {
    if let Some(mut scale) = scale {
        scale.0 = 1.0;
    }
}
