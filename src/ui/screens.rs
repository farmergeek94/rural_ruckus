//! What is drawn: the frame (title, tabs, EXIT, summary line and GO) and, inside it, the screen
//! that is up. Reads the model and never changes it: a widget only says what activating it
//! does (`Does`), and `input` passes that on.
//!
//! The whole tree is built again whenever the model changes. It is a few dozen nodes, a
//! change is a key press, and one function that draws the model as it stands cannot get
//! out of step with it. Lists show a window of rows around the highlight instead of
//! scrolling, so a list can be walked with a stick and there is no scroll position to lose.
//! The options screen, which has more lines than fit, scrolls: with the mouse wheel, and by
//! itself to bring the line in hand into view. Where it was scrolled to is kept
//! (`OptionsScrolled`), so that building the tree again doesn't lose it.
//!
//! Text keeps to ASCII while Bevy's built-in font, which has little else, stands in.

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use bevy::ui_widgets::Button;

use super::input::Does;
use super::model::{Action, FrontEnd, List, Screen, split_in_two, visible_rows};
use super::theme::Look;
use super::{
    Catalogue, Dial, Dials, Entry, Folder, FolderBrowser, FrontEndOpen, Model, Preset, Presets,
    Setting, Settings, Theme, TrackPreview,
};

/// The root of everything drawn here.
#[derive(Component)]
pub(super) struct ScreenRoot;

/// What the options screen's lines scroll in.
#[derive(Component)]
pub(super) struct OptionsScroll;

/// The line of the options screen that is in hand.
#[derive(Component)]
pub(super) struct InHand;

/// How far down the options screen is scrolled, in logical pixels.
#[derive(Resource, Default)]
pub(super) struct OptionsScrolled(f32);

/// How far one notch of a mouse wheel scrolls the options, in logical pixels.
const SCROLL_LINE: f32 = 40.0;
/// How much room is left above and below the line in hand when it is scrolled into view,
/// so that the heading over the first line of a section shows with it.
const SCROLL_MARGIN: f32 = 40.0;

const TITLE: &str = "RURAL RUCKUS";
/// How much wider than a list the folder browser is, for the long paths in it.
const BROWSER_WIDTH: f32 = 1.6;
const HINTS: &str = "Q / E screens    arrows move and change    - / + opponents    Enter GO    drag or right stick turns the truck";
const BROWSING_HINTS: &str = "arrows move    Enter or right goes in    Backspace or left goes up    Space chooses    Esc leaves";
const SEARCH_HINT: &str = "    / searches";
const TYPING_HINTS: &str = "type to search    up / down move    Backspace erases    Enter keeps the search    Esc clears it";

#[expect(
    clippy::too_many_arguments,
    reason = "it draws everything the module is handed"
)]
pub(super) fn redraw(
    mut commands: Commands,
    model: Option<Res<Model>>,
    catalogue: Res<Catalogue>,
    dials: Res<Dials>,
    settings: Res<Settings>,
    presets: Res<Presets>,
    browser: Res<FolderBrowser>,
    preview: Res<TrackPreview>,
    theme: Res<Theme>,
    scrolled: Res<OptionsScrolled>,
    roots: Query<Entity, With<ScreenRoot>>,
) {
    let Some(model) = model else {
        return;
    };
    if !roots.is_empty() && !model.is_changed() && !preview.is_changed() && !theme.is_changed() {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }

    let drawing = Drawing {
        model: &model.0,
        catalogue: &catalogue,
        dials: &dials.0,
        settings: &settings.0,
        presets: &presets.0,
        // Only while the model has it too: the two change a frame apart.
        folder: browser.0.as_ref().filter(|_| model.0.browsing.is_some()),
        preview: preview.0.as_ref(),
        theme: &theme,
        scrolled: scrolled.0,
    };
    commands
        .spawn((
            Name::new("Front end"),
            ScreenRoot,
            DespawnOnExit(FrontEndOpen::Open),
            // Anchored to the window's edges, so that a wider screen gives the truck more
            // room and doesn't stretch the lists.
            layout(Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(px(theme.margin)),
                row_gap: px(theme.margin),
                ..default()
            }),
        ))
        .with_children(|root| {
            drawing.top(root);
            drawing.middle(root);
            drawing.bottom(root);
        });
}

fn laps_text(laps: u32) -> String {
    match laps {
        1 => "1 lap".into(),
        laps => format!("{laps} laps"),
    }
}

fn opponents_text(opponents: u32) -> String {
    match opponents {
        0 => "No opponents".into(),
        1 => "1 opponent".into(),
        opponents => format!("{opponents} opponents"),
    }
}

/// A node that only arranges others. It lets the pointer through to the showroom behind.
fn layout(node: Node) -> (Node, Pickable) {
    (node, Pickable::IGNORE)
}

struct Drawing<'a> {
    model: &'a FrontEnd,
    catalogue: &'a Catalogue,
    dials: &'a [Dial],
    settings: &'a [Setting],
    presets: &'a [Preset],
    folder: Option<&'a Folder>,
    preview: Option<&'a Handle<Image>>,
    theme: &'a Theme,
    scrolled: f32,
}

type Parent<'a, 'b> = &'a mut ChildSpawnerCommands<'b>;

impl Drawing<'_> {
    fn top(&self, parent: Parent) {
        let theme = self.theme;
        parent
            .spawn(layout(Node {
                align_items: AlignItems::Center,
                column_gap: px(theme.margin),
                ..default()
            }))
            .with_children(|top| {
                top.spawn((
                    Text::new(TITLE),
                    theme.heading(theme.title_size),
                    TextColor(theme.accent),
                    Pickable::IGNORE,
                ));
                // No other screen can be gone to while a folder is browsed.
                if self.folder.is_some() {
                    return;
                }
                top.spawn(layout(Node {
                    column_gap: px(8),
                    ..default()
                }))
                .with_children(|tabs| {
                    for screen in Screen::ALL {
                        self.button(
                            tabs,
                            screen.title(),
                            theme.heading_size,
                            Action::Show(screen),
                            screen == self.model.screen,
                            Node {
                                padding: UiRect::axes(px(28), px(12)),
                                ..default()
                            },
                        );
                    }
                });
                // At the far right, away from the tabs.
                top.spawn(layout(Node {
                    margin: UiRect::left(Val::Auto),
                    ..default()
                }))
                .with_children(|right| {
                    self.button(
                        right,
                        "EXIT",
                        theme.heading_size,
                        Action::Exit,
                        false,
                        Node {
                            padding: UiRect::axes(px(28), px(12)),
                            ..default()
                        },
                    );
                });
            });
    }

    fn middle(&self, parent: Parent) {
        if let Some(folder) = self.folder {
            parent
                .spawn(layout(Node {
                    flex_grow: 1.0,
                    align_items: AlignItems::FlexStart,
                    ..default()
                }))
                .with_children(|middle| self.browser(middle, folder));
            return;
        }
        parent
            .spawn(layout(Node {
                flex_grow: 1.0,
                // No taller than the room it is given, so that the options can scroll in it.
                min_height: px(0),
                align_items: AlignItems::FlexStart,
                column_gap: px(self.theme.margin),
                ..default()
            }))
            .with_children(|middle| match self.model.screen {
                Screen::Truck => {
                    self.list(
                        middle,
                        "SELECT TRUCK",
                        &self.catalogue.trucks,
                        &self.model.trucks,
                        Action::PickTruck,
                    );
                }
                Screen::Race => {
                    self.list(
                        middle,
                        "SELECT TRACK",
                        &self.catalogue.tracks,
                        &self.model.tracks,
                        Action::PickTrack,
                    );
                    self.race(middle);
                }
                Screen::Garage => self.garage(middle),
                Screen::Options | Screen::Controls => self.options(middle),
            });
    }

    fn panel(&self, width: f32) -> impl Bundle {
        (
            self.theme.card(Node {
                width: px(width),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(self.theme.padding)),
                row_gap: px(6),
                ..default()
            }),
            BackgroundColor(self.theme.panel),
        )
    }

    fn heading(&self, parent: Parent, text: &str) {
        parent.spawn((
            Text::new(text),
            self.theme.heading(self.theme.heading_size),
            TextColor(self.theme.text_dim),
            Node {
                margin: UiRect::bottom(px(8)),
                ..default()
            },
        ));
    }

    /// A list under its heading, with a search field over it when it is too long to see
    /// all at once. Only what the search finds is shown.
    fn list(
        &self,
        parent: Parent,
        heading: &str,
        entries: &[Entry],
        list: &List,
        pick: fn(usize) -> Action,
    ) {
        let theme = self.theme;
        parent
            .spawn(self.panel(theme.panel_width))
            .with_children(|panel| {
                self.heading(panel, heading);
                if list.searchable {
                    self.search_field(panel, list);
                }
                let shown = list.shown();
                // Where the highlight is among what is shown: at the top when it is hidden.
                let at = shown
                    .iter()
                    .position(|&index| index == list.highlighted)
                    .unwrap_or(0);
                let rows = visible_rows(at, shown.len(), theme.list_rows);
                for &index in &shown[rows.clone()] {
                    self.row(
                        panel,
                        &entries[index],
                        index == list.highlighted,
                        pick(index),
                    );
                }
                let found = if list.search.is_empty() { "" } else { " found" };
                let count = if rows.len() < shown.len() {
                    format!(
                        "{} to {} of {}{found}",
                        rows.start + 1,
                        rows.end,
                        shown.len()
                    )
                } else if !list.search.is_empty() && !shown.is_empty() {
                    format!("{} of {}", shown.len(), entries.len())
                } else {
                    String::new()
                };
                if !count.is_empty() {
                    panel.spawn((
                        Text::new(count),
                        theme.body(theme.small_size),
                        TextColor(theme.text_dim),
                    ));
                }
                let empty = if entries.is_empty() {
                    "Nothing to choose from"
                } else if shown.is_empty() {
                    "Nothing found"
                } else {
                    ""
                };
                if !empty.is_empty() {
                    panel.spawn((
                        Text::new(empty),
                        theme.body(theme.text_size),
                        TextColor(theme.text_dim),
                    ));
                }
            });
    }

    /// What the list is searched for, which takes the keys when it is clicked, and a button
    /// that empties it.
    fn search_field(&self, panel: Parent, list: &List) {
        let theme = self.theme;
        let typing = self.model.typing;
        let (text, color) = match (list.search.is_empty(), typing) {
            // A bar where the next character goes.
            (_, true) => (format!("{}|", list.search), theme.text),
            (true, false) => ("Search  ( / )".to_string(), theme.text_dim),
            (false, false) => (list.search.clone(), theme.text),
        };
        panel
            .spawn(Node {
                column_gap: px(8),
                margin: UiRect::bottom(px(6)),
                ..default()
            })
            .with_children(|line| {
                line.spawn((
                    Button,
                    Does(Action::Search),
                    Look {
                        normal: theme.row,
                        hovered: theme.row_hovered,
                    },
                    BackgroundColor(theme.row),
                    theme.pill(
                        Node {
                            flex_grow: 1.0,
                            min_width: px(0),
                            height: px(theme.row_height),
                            padding: UiRect::horizontal(px(24)),
                            align_items: AlignItems::Center,
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        typing,
                    ),
                ))
                .with_children(|field| {
                    field.spawn((
                        Text::new(text),
                        theme.body(theme.text_size),
                        TextColor(color),
                        TextLayout::no_wrap(),
                        Pickable::IGNORE,
                    ));
                });
                if !list.search.is_empty() {
                    self.button(
                        line,
                        "X",
                        theme.text_size,
                        Action::ClearSearch,
                        false,
                        Node {
                            width: px(theme.row_height),
                            height: px(theme.row_height),
                            flex_shrink: 0.0,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                    );
                }
            });
    }

    fn row(&self, parent: Parent, entry: &Entry, highlighted: bool, action: Action) {
        let theme = self.theme;
        let (normal, hovered, text) = match (highlighted, entry.available) {
            (true, true) => (theme.accent, theme.accent_hovered, theme.text_on_accent),
            // Highlighted all the same, so that the player can see why it can't be had.
            (true, false) => (theme.row_hovered, theme.row_hovered, theme.text_dim),
            (false, true) => (theme.row, theme.row_hovered, theme.text),
            (false, false) => (theme.row, theme.row_hovered, theme.text_dim),
        };
        parent
            .spawn((
                Button,
                Does(action),
                Look { normal, hovered },
                BackgroundColor(normal),
                theme.pill(
                    Node {
                        height: px(theme.row_height),
                        padding: UiRect::horizontal(px(24)),
                        align_items: AlignItems::Center,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    highlighted && entry.available,
                ),
            ))
            .with_children(|row| {
                row.spawn((
                    Text::new(entry.name.clone()),
                    theme.body(theme.text_size),
                    TextColor(text),
                    TextLayout::no_wrap(),
                    Pickable::IGNORE,
                ));
            });
    }

    /// A folder being browsed: where it is, what is in it, its subfolders, and what can be
    /// done there.
    fn browser(&self, parent: Parent, folder: &Folder) {
        let theme = self.theme;
        parent
            .spawn(self.panel(theme.panel_width * BROWSER_WIDTH))
            .with_children(|panel| {
                self.heading(panel, &folder.heading);
                panel.spawn((
                    Text::new(folder.path.clone()),
                    theme.body(theme.text_size),
                    TextColor(theme.text),
                ));
                panel.spawn((
                    Text::new(folder.found.clone()),
                    theme.body(theme.small_size),
                    TextColor(if folder.can_choose {
                        theme.accent
                    } else {
                        theme.text_dim
                    }),
                    Node {
                        margin: UiRect::bottom(px(8)),
                        ..default()
                    },
                ));

                let rows = visible_rows(folder.highlighted, folder.folders.len(), theme.list_rows);
                for index in rows.clone() {
                    self.row(
                        panel,
                        &folder.folders[index],
                        index == folder.highlighted,
                        Action::PickFolder(index),
                    );
                }
                if rows.len() < folder.folders.len() {
                    panel.spawn((
                        Text::new(format!(
                            "{} to {} of {}",
                            rows.start + 1,
                            rows.end,
                            folder.folders.len()
                        )),
                        theme.body(theme.small_size),
                        TextColor(theme.text_dim),
                    ));
                }
                if folder.folders.is_empty() {
                    panel.spawn((
                        Text::new("No folders in here"),
                        theme.body(theme.text_size),
                        TextColor(theme.text_dim),
                    ));
                }

                panel.spawn(Node {
                    height: px(16),
                    ..default()
                });
                panel
                    .spawn(Node {
                        column_gap: px(8),
                        ..default()
                    })
                    .with_children(|buttons| {
                        let node = || Node {
                            flex_grow: 1.0,
                            flex_basis: px(0),
                            height: px(theme.row_height),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        };
                        self.button_if(
                            buttons,
                            "UP",
                            folder.can_go_up,
                            Action::FolderUp,
                            false,
                            node(),
                        );
                        self.button_if(
                            buttons,
                            "USE THIS FOLDER",
                            folder.can_choose,
                            Action::ChooseFolder,
                            true,
                            node(),
                        );
                        self.button(
                            buttons,
                            &folder.leave,
                            theme.text_size,
                            Action::LeaveFolder,
                            false,
                            node(),
                        );
                    });
            });
    }

    /// A button that can't be pressed when `enabled` is false, and is drawn so.
    fn button_if(
        &self,
        parent: Parent,
        text: &str,
        enabled: bool,
        action: Action,
        lit: bool,
        node: Node,
    ) {
        let theme = self.theme;
        if enabled {
            self.button(parent, text, theme.text_size, action, lit, node);
            return;
        }
        // Rounded as `Theme::pill` rounds a button.
        let node = Node {
            border_radius: BorderRadius::all(px(999)),
            ..node
        };
        parent
            .spawn((node, BackgroundColor(theme.disabled)))
            .with_children(|button| {
                button.spawn((
                    Text::new(text),
                    theme.heading(theme.text_size),
                    TextColor(theme.text_dim),
                ));
            });
    }

    /// Beside the list of tracks: the highlighted one's map, the laps and the opponents.
    fn race(&self, parent: Parent) {
        let theme = self.theme;
        parent
            .spawn(self.panel(theme.map_size + theme.padding * 2.0))
            .with_children(|panel| {
                self.heading(panel, "COURSE");
                let mut map = panel.spawn((
                    Node {
                        width: px(theme.map_size),
                        height: px(theme.map_size),
                        border_radius: BorderRadius::all(px(theme.corner_radius)),
                        ..default()
                    },
                    BackgroundColor(theme.row),
                ));
                if let Some(image) = self.preview {
                    map.insert(ImageNode::new(image.clone()));
                }

                self.stepper(
                    panel,
                    "LAPS",
                    laps_text(self.model.laps),
                    Action::FewerLaps,
                    Action::MoreLaps,
                );
                self.stepper(
                    panel,
                    "OPPONENTS",
                    opponents_text(self.model.opponents),
                    Action::FewerOpponents,
                    Action::MoreOpponents,
                );
            });
    }

    /// A number with a button on each side of it, under a heading.
    fn stepper(&self, panel: Parent, heading: &str, text: String, fewer: Action, more: Action) {
        let theme = self.theme;
        // A little room between this and what is over it.
        panel.spawn(Node {
            height: px(12),
            ..default()
        });
        self.heading(panel, heading);
        panel
            .spawn(Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|stepper| {
                let square = || Node {
                    width: px(theme.row_height),
                    height: px(theme.row_height),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                };
                self.button(stepper, "-", theme.heading_size, fewer, false, square());
                stepper.spawn((
                    Text::new(text),
                    theme.heading(theme.heading_size),
                    TextColor(theme.text),
                ));
                self.button(stepper, "+", theme.heading_size, more, false, square());
            });
    }

    fn garage(&self, parent: Parent) {
        let theme = self.theme;
        parent
            .spawn(self.panel(theme.panel_width))
            .with_children(|panel| {
                self.heading(panel, "GARAGE");
                for (index, dial) in self.dials.iter().enumerate() {
                    let in_hand = index == self.model.dial_in_hand;
                    panel.spawn((
                        Text::new(dial.label.to_uppercase()),
                        theme.heading(theme.text_size),
                        TextColor(if in_hand { theme.accent } else { theme.text }),
                        Node {
                            margin: UiRect::top(px(14)),
                            ..default()
                        },
                    ));
                    // One pip for each place the dial can stand, and the one it stands at lit.
                    panel
                        .spawn(Node {
                            column_gap: px(6),
                            ..default()
                        })
                        .with_children(|pips| {
                            for step in 0..dial.steps {
                                self.button(
                                    pips,
                                    "",
                                    theme.small_size,
                                    Action::SetDial(index, step),
                                    step == dial.step,
                                    Node {
                                        flex_grow: 1.0,
                                        height: px(26),
                                        ..default()
                                    },
                                );
                            }
                        });
                    panel
                        .spawn(Node {
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        })
                        .with_children(|ends| {
                            for label in [&dial.low_label, &dial.high_label] {
                                ends.spawn((
                                    Text::new(label.clone()),
                                    theme.body(theme.small_size),
                                    TextColor(theme.text_dim),
                                ));
                            }
                        });
                }
            });
    }

    /// The settings of the screen that is up, section by section, in two columns so that
    /// they fit on the screen with room for the truck beside them, and a button that puts
    /// them all back.
    fn options(&self, parent: Parent) {
        let theme = self.theme;
        // Each section with the settings in it, in the order they first come in.
        let mut sections: Vec<(&str, Vec<usize>)> = Vec::new();
        for (index, setting) in self.settings.iter().enumerate() {
            if !self.model.on_screen(index) {
                continue;
            }
            match sections
                .iter_mut()
                .find(|(section, _)| *section == setting.section)
            {
                Some((_, members)) => members.push(index),
                None => sections.push((&setting.section, vec![index])),
            }
        }
        // A heading is about as tall as a setting.
        let heights: Vec<usize> = sections
            .iter()
            .map(|(_, members)| members.len() + 1)
            .collect();
        let split = split_in_two(&heights);
        let columns = [&sections[..split], &sections[split..]];
        let last = columns
            .iter()
            .rposition(|column| !column.is_empty())
            .unwrap_or(0);
        parent
            .spawn((
                OptionsScroll,
                ScrollPosition(Vec2::new(0.0, self.scrolled)),
                layout(Node {
                    height: percent(100),
                    align_items: AlignItems::FlexStart,
                    column_gap: px(theme.margin),
                    overflow: Overflow::scroll_y(),
                    ..default()
                }),
            ))
            .with_children(|parent| self.option_columns(parent, &sections, columns, last));
    }

    fn option_columns(
        &self,
        parent: Parent,
        sections: &[(&str, Vec<usize>)],
        columns: [&[(&str, Vec<usize>)]; 2],
        last: usize,
    ) {
        let theme = self.theme;
        for (number, column) in columns.into_iter().enumerate() {
            if column.is_empty() && number != last {
                continue;
            }
            parent
                .spawn(self.panel(theme.panel_width))
                .with_children(|panel| {
                    for (place, (section, members)) in column.iter().enumerate() {
                        if place > 0 {
                            panel.spawn(Node {
                                height: px(16),
                                ..default()
                            });
                        }
                        self.heading(panel, section);
                        for &index in members {
                            self.setting(panel, index);
                        }
                    }
                    if number != last {
                        return;
                    }
                    if sections.is_empty() {
                        panel.spawn((
                            Text::new("Nothing to set"),
                            theme.body(theme.text_size),
                            TextColor(theme.text_dim),
                        ));
                        return;
                    }
                    panel.spawn(Node {
                        height: px(16),
                        ..default()
                    });
                    self.button(
                        panel,
                        "RESTORE DEFAULTS",
                        theme.text_size,
                        Action::RestoreDefaults,
                        false,
                        Node {
                            height: px(theme.row_height),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                    );
                    // The presets, under it: each a set of values chosen at once.
                    for (index, preset) in self.presets.iter().enumerate() {
                        panel.spawn(Node {
                            height: px(8),
                            ..default()
                        });
                        self.button(
                            panel,
                            &preset.label,
                            theme.text_size,
                            Action::ApplyPreset(index),
                            false,
                            Node {
                                height: px(theme.row_height),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                        );
                    }
                });
        }
    }

    /// One line: the setting's name, and a button for each of its values with the chosen
    /// one lit. The line is marked while Less and More would change it.
    fn setting(&self, parent: Parent, index: usize) {
        let theme = self.theme;
        let setting = &self.settings[index];
        let in_hand = index == self.model.setting_in_hand;
        parent
            .spawn((
                Node {
                    // Taller where a label or a key's name takes two lines.
                    min_height: px(theme.row_height),
                    padding: UiRect::axes(px(10), px(4)),
                    align_items: AlignItems::Center,
                    column_gap: px(6),
                    border_radius: BorderRadius::all(px(theme.corner_radius)),
                    ..default()
                },
                BackgroundColor(if in_hand {
                    theme.row_hovered
                } else {
                    Color::NONE
                }),
            ))
            .insert_if(InHand, || in_hand)
            .with_children(|line| {
                line.spawn((
                    Text::new(setting.label.to_uppercase()),
                    theme.heading(theme.small_size),
                    TextColor(if in_hand { theme.accent } else { theme.text }),
                    Node {
                        width: px(theme.setting_label_width),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
                if !setting.keys.is_empty() {
                    self.key_button(line, index);
                    return;
                }
                if setting.opens {
                    self.open_button(line, index);
                    return;
                }
                if setting.values.len() > theme.most_setting_buttons {
                    self.value_stepper(line, index);
                    return;
                }
                for (value, name) in setting.values.iter().enumerate() {
                    self.button(
                        line,
                        name,
                        theme.small_size,
                        Action::SetSetting(index, value),
                        value == setting.chosen,
                        // As wide as its name, and a share of the room left over, so that
                        // a long name such as FULLSCREEN is not cut off to make them equal.
                        Node {
                            flex_grow: 1.0,
                            min_width: px(0),
                            height: px(theme.row_height - 10.0),
                            padding: UiRect::horizontal(px(8)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            overflow: Overflow::clip(),
                            ..default()
                        },
                    );
                }
            });
    }

    /// A line that opens something: one button with its value, on one line. What does not
    /// fit is cut off, and the game keeps it short.
    fn open_button(&self, line: Parent, index: usize) {
        let theme = self.theme;
        line.spawn((
            Button,
            Does(Action::OpenSetting(index)),
            Look {
                normal: theme.row,
                hovered: theme.row_hovered,
            },
            BackgroundColor(theme.row),
            theme.pill(
                Node {
                    flex_grow: 1.0,
                    // Its share of the line, however long the value.
                    flex_basis: px(0),
                    min_width: px(0),
                    height: px(theme.row_height - 10.0),
                    padding: UiRect::horizontal(px(16)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    overflow: Overflow::clip(),
                    ..default()
                },
                false,
            ),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(self.settings[index].chosen_value()),
                theme.heading(theme.small_size),
                TextColor(theme.text),
                TextLayout::no_wrap(),
                Pickable::IGNORE,
            ));
        });
    }

    /// A key binding: one button with the key it is bound to, which waits for a key when
    /// it is clicked, and says so.
    fn key_button(&self, line: Parent, index: usize) {
        let theme = self.theme;
        let setting = &self.settings[index];
        let listening = self.model.listening && index == self.model.setting_in_hand;
        let text = if listening {
            "PRESS A KEY  (ESC STOPS)"
        } else {
            setting.chosen_value()
        };
        let (normal, hovered, color) = if listening {
            (theme.accent, theme.accent_hovered, theme.text_on_accent)
        } else {
            (theme.row, theme.row_hovered, theme.text)
        };
        line.spawn((
            Button,
            Does(Action::Listen(index)),
            Look { normal, hovered },
            BackgroundColor(normal),
            theme.pill(
                Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    min_width: px(0),
                    // Taller, and no less round, where the text takes two lines.
                    min_height: px(theme.row_height - 10.0),
                    padding: UiRect::axes(px(16), px(4)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                listening,
            ),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(text),
                theme.heading(theme.small_size),
                TextColor(color),
                TextLayout::justify(bevy::text::Justify::Center),
                Pickable::IGNORE,
            ));
        });
    }

    /// A setting with too many values for a button each: the chosen one in the middle, which
    /// goes on to the next, round to the first after the last, as Enter does; an arrow either
    /// side, which stops at the ends, as the arrow keys do; and under the value, a dot for each
    /// value, the chosen one lit.
    fn value_stepper(&self, line: Parent, index: usize) {
        let theme = self.theme;
        let setting = &self.settings[index];
        let (chosen, count) = (setting.chosen, setting.values.len());
        let arrow_node = || Node {
            width: px(theme.row_height - 10.0),
            height: px(theme.row_height - 10.0),
            flex_shrink: 0.0,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        };
        let arrow = |line: Parent, text: &str, to: Option<usize>| match to {
            Some(to) => self.button(
                line,
                text,
                theme.small_size,
                Action::SetSetting(index, to),
                false,
                arrow_node(),
            ),
            // At an end: there, but with nowhere to go.
            None => {
                line.spawn(arrow_node()).with_children(|place| {
                    place.spawn((
                        Text::new(text),
                        theme.heading(theme.small_size),
                        TextColor(theme.disabled),
                    ));
                });
            }
        };
        arrow(line, "<", chosen.checked_sub(1));
        line.spawn((
            Button,
            Does(Action::SetSetting(index, (chosen + 1) % count)),
            Look {
                normal: theme.accent,
                hovered: theme.accent_hovered,
            },
            BackgroundColor(theme.accent),
            theme.pill(
                Node {
                    flex_grow: 1.0,
                    height: px(theme.row_height - 10.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: px(3),
                    overflow: Overflow::clip(),
                    ..default()
                },
                true,
            ),
        ))
        .with_children(|middle| {
            middle.spawn((
                Text::new(setting.values.get(chosen).map_or("", String::as_str)),
                theme.heading(theme.small_size),
                TextColor(theme.text_on_accent),
                TextLayout::no_wrap(),
                Pickable::IGNORE,
            ));
            middle
                .spawn((
                    Node {
                        column_gap: px(4),
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|dots| {
                    for value in 0..count {
                        let lit = value == chosen;
                        dots.spawn((
                            Node {
                                width: px(5),
                                height: px(5),
                                border_radius: BorderRadius::all(px(2.5)),
                                ..default()
                            },
                            BackgroundColor(theme.text_on_accent.with_alpha(if lit {
                                1.0
                            } else {
                                0.35
                            })),
                            Pickable::IGNORE,
                        ));
                    }
                });
        });
        arrow(line, ">", (chosen + 1 < count).then_some(chosen + 1));
    }

    /// The line that sums up the three screens that lead to GO, so that GO is never a leap
    /// in the dark, with GO itself at the right.
    fn bottom(&self, parent: Parent) {
        let theme = self.theme;
        parent
            .spawn(layout(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexEnd,
                ..default()
            }))
            .with_children(|bottom| {
                bottom
                    .spawn(layout(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(6),
                        ..default()
                    }))
                    .with_children(|lines| {
                        let line = |text: String, size: f32, color: Color| {
                            (
                                Text::new(text),
                                theme.body(size),
                                TextColor(color),
                                Pickable::IGNORE,
                            )
                        };
                        if let Some(folder) = self.folder {
                            lines.spawn(line(folder.about.clone(), theme.text_size, theme.text));
                            lines.spawn(line(
                                BROWSING_HINTS.into(),
                                theme.small_size,
                                theme.text_dim,
                            ));
                            return;
                        }
                        lines.spawn(line(self.detail(), theme.small_size, theme.text_dim));
                        lines.spawn(line(self.summary(), theme.text_size, theme.text));
                        lines.spawn(line(self.hints(), theme.small_size, theme.text_dim));
                    });
                // Nothing is raced from a folder.
                if self.folder.is_some() {
                    return;
                }

                let can_go = self.model.can_go();
                let (normal, hovered, text) = if can_go {
                    (theme.accent, theme.accent_hovered, theme.text_on_accent)
                } else {
                    (theme.disabled, theme.disabled, theme.text_dim)
                };
                bottom
                    .spawn((
                        Button,
                        Does(Action::Go),
                        Look { normal, hovered },
                        BackgroundColor(normal),
                        theme.pill(
                            Node {
                                width: px(300),
                                height: px(120),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            can_go,
                        ),
                    ))
                    .with_children(|go| {
                        go.spawn((
                            Text::new("GO"),
                            theme.heading(theme.go_size),
                            TextColor(text),
                            Pickable::IGNORE,
                        ));
                    });
            });
    }

    /// Which keys do what, with the search's own while it takes the keys.
    fn hints(&self) -> String {
        if self.model.typing {
            return TYPING_HINTS.into();
        }
        let searchable = match self.model.screen {
            Screen::Truck => self.model.trucks.searchable,
            Screen::Race => self.model.tracks.searchable,
            _ => false,
        };
        if searchable {
            format!("{HINTS}{SEARCH_HINT}")
        } else {
            HINTS.into()
        }
    }

    /// What is highlighted on the screen that is up, and where it came from.
    fn detail(&self) -> String {
        let entry = match self.model.screen {
            Screen::Truck => self.catalogue.trucks.get(self.model.trucks.highlighted),
            Screen::Race => self.catalogue.tracks.get(self.model.tracks.highlighted),
            Screen::Garage => None,
            Screen::Options | Screen::Controls => {
                return self
                    .settings
                    .get(self.model.setting_in_hand)
                    .map_or(String::new(), |setting| {
                        format!("{}  |  {}", setting.label, setting.detail)
                    });
            }
        };
        entry.map_or(String::new(), |entry| {
            format!("{}  |  {}", entry.name, entry.detail)
        })
    }

    fn summary(&self) -> String {
        let name = |entries: &[Entry], index: usize| {
            entries
                .get(index)
                .map_or("nothing", |entry| entry.name.as_str())
                .to_string()
        };
        // A dial away from its middle is worth a mention, by the name of the end it leans to.
        let leaning: Vec<String> = self
            .dials
            .iter()
            .filter(|dial| dial.value() != 0.0)
            .map(|dial| {
                let end = if dial.value() < 0.0 {
                    &dial.low_label
                } else {
                    &dial.high_label
                };
                format!("{} {}", dial.label, end).to_lowercase()
            })
            .collect();
        format!(
            "{}  |  {}  |  {}  |  {}  |  {}",
            name(&self.catalogue.trucks, self.model.trucks.highlighted),
            name(&self.catalogue.tracks, self.model.tracks.highlighted),
            laps_text(self.model.laps),
            opponents_text(self.model.opponents).to_lowercase(),
            if leaning.is_empty() {
                "standard setup".into()
            } else {
                leaning.join(", ")
            },
        )
    }

    /// A tab, a stepper's end or a dial's pip: lit when it is the one chosen.
    fn button(&self, parent: Parent, text: &str, size: f32, action: Action, lit: bool, node: Node) {
        let theme = self.theme;
        let (normal, hovered, color) = if lit {
            (theme.accent, theme.accent_hovered, theme.text_on_accent)
        } else {
            (theme.row, theme.row_hovered, theme.text)
        };
        parent
            .spawn((
                Button,
                Does(action),
                Look { normal, hovered },
                BackgroundColor(normal),
                theme.pill(node, lit),
            ))
            .with_children(|button| {
                if !text.is_empty() {
                    button.spawn((
                        Text::new(text),
                        theme.heading(size),
                        TextColor(color),
                        Pickable::IGNORE,
                    ));
                }
            });
    }
}

/// Scrolls the options with the mouse wheel, as far as there is to scroll.
pub(super) fn scroll_options(
    // Absent in an app without input.
    wheel: Option<Res<AccumulatedMouseScroll>>,
    mut scrolled: ResMut<OptionsScrolled>,
    mut scrolls: Query<(&mut ScrollPosition, &ComputedNode), With<OptionsScroll>>,
) {
    let Some(wheel) = wheel else {
        return;
    };
    if wheel.delta.y == 0.0 {
        return;
    }
    let by = match wheel.unit {
        MouseScrollUnit::Line => wheel.delta.y * SCROLL_LINE,
        MouseScrollUnit::Pixel => wheel.delta.y,
    };
    for (mut position, node) in &mut scrolls {
        // The wheel turned towards the player scrolls down, which shows what is lower.
        let most = (node.content_size.y - node.size.y).max(0.0) * node.inverse_scale_factor;
        scrolled.0 = (position.y - by).clamp(0.0, most);
        position.y = scrolled.0;
    }
}

/// The options' scroll, and where and how big what shows of it is.
type ScrollView<'a> = (
    &'a mut ScrollPosition,
    &'a ComputedNode,
    &'a UiGlobalTransform,
);

/// Once the tree has been built again and laid out, scrolls the options just far enough
/// that the line in hand shows, as when the arrows have moved to one out of sight. Only
/// then, so that the wheel can scroll away from it.
pub(super) fn keep_in_hand_in_view(
    lines: Query<(&ComputedNode, &UiGlobalTransform), Added<InHand>>,
    mut scrolls: Query<ScrollView, (With<OptionsScroll>, Without<InHand>)>,
    mut scrolled: ResMut<OptionsScrolled>,
) {
    let Ok((line, line_at)) = lines.single() else {
        return;
    };
    for (mut position, node, at) in &mut scrolls {
        let scale = node.inverse_scale_factor;
        // From the top of what shows, in logical pixels. A transform is a node's middle.
        let top_of_view = at.translation.y - node.size.y / 2.0;
        let top = (line_at.translation.y - line.size.y / 2.0 - top_of_view) * scale;
        let bottom = top + line.size.y * scale;
        let height = node.size.y * scale;
        let by = if top < SCROLL_MARGIN {
            top - SCROLL_MARGIN
        } else if bottom > height - SCROLL_MARGIN {
            bottom - (height - SCROLL_MARGIN)
        } else {
            continue;
        };
        let most = (node.content_size.y - node.size.y).max(0.0) * scale;
        scrolled.0 = (position.y + by).clamp(0.0, most);
        position.y = scrolled.0;
    }
}
