//! The pause dialog: Esc (or a gamepad's Start or Select) in a race stops the game and shows
//! four choices: continue, restart the race, save a screenshot, and cancel the race.
//!
//! Pausing is `RacePause::Paused`, a state that exists only in a race, and stops Bevy's
//! virtual clock. The fixed timestep runs on that clock, so physics, the race clock and
//! the computer's drivers stop with it, and so does everything that moves by `Time`. The
//! race's own keys (restart, race again, back to the checkpoint) do nothing while it is
//! paused: Enter, which races again after the finish, also chooses in the dialog.
//!
//! Restarting from the dialog builds the whole race again, as GO on the front end does:
//! `GameState::Racing` is left and entered again, so that everything of the race is
//! despawned and spawned afresh (trucks, knocked-over scenery, the countdown, the computer's
//! setups and a random weather).
//!
//! A screenshot is of the race as it stands, without the dialog: the dialog is hidden in
//! the frame that is captured, and shown again, saying where the picture went, once it is
//! saved. Pictures go in `SCREENSHOT_FOLDER`, numbered one after the other.
//!
//! Cancelling the race writes `RaceCancelled`, which the front end answers by coming back
//! (`RaceSettings::front_end`). A race started from the command line has no front end
//! to come back to, and the choice quits the game instead.
//!
//! What MTM2's pause screen offered is not confirmed. This one is the game's own.

use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::ui::widget::Button;

use super::RaceSettings;
use super::start::BLUE;
use crate::game_state::GameState;

/// Where screenshots are saved, beside the working directory as `tracks/` and `saves/` are.
const SCREENSHOT_FOLDER: &str = "screenshots";
/// How long to wait for a screenshot before giving up on it, in real seconds. A capture
/// takes a frame or two; an app that cannot draw never captures at all.
const CAPTURE_TIMEOUT: f32 = 3.0;
/// The dialog's buttons, in pixels.
const BUTTON_WIDTH: f32 = 380.0;
const BUTTON_HEIGHT: f32 = 58.0;
const BUTTON_TEXT: f32 = 26.0;
/// A button that is not highlighted.
const BUTTON_GREY: Color = Color::srgba(0.18, 0.18, 0.2, 0.9);

/// Whether a race is paused. Exists only while `GameState::Racing`.
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(GameState = GameState::Racing)]
pub enum RacePause {
    #[default]
    Running,
    Paused,
}

/// The player chose to cancel the race from the pause dialog. The front end goes back to
/// its screens.
#[derive(Message, Clone, Copy, Debug)]
pub struct RaceCancelled;

/// One line of the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
    Continue,
    Restart,
    Screenshot,
    Cancel,
}

impl Choice {
    /// Top to bottom.
    const ALL: [Choice; 4] = [
        Choice::Continue,
        Choice::Restart,
        Choice::Screenshot,
        Choice::Cancel,
    ];

    fn label(self, front_end: bool) -> &'static str {
        match self {
            Choice::Continue => "CONTINUE",
            Choice::Restart => "RESTART RACE",
            Choice::Screenshot => "SAVE SCREENSHOT",
            Choice::Cancel if front_end => "CANCEL RACE",
            Choice::Cancel => "QUIT GAME",
        }
    }
}

/// What the dialog shows.
#[derive(Resource, Default)]
pub(super) struct PauseMenu {
    /// Which of `Choice::ALL` is highlighted.
    highlighted: usize,
    /// A line under the choices: where the last screenshot went, or why it did not.
    note: String,
    /// When a screenshot was asked for, in real time since the app started. The dialog is
    /// hidden until it is saved.
    capturing: Option<Duration>,
}

impl PauseMenu {
    /// Moves the highlight one line, and stops at the ends.
    fn step(&mut self, down: bool) {
        self.highlighted = if down {
            (self.highlighted + 1).min(Choice::ALL.len() - 1)
        } else {
            self.highlighted.saturating_sub(1)
        };
    }
}

#[derive(Component)]
pub(super) struct PauseDialog;

/// On the button for `Choice::ALL[0]`.
#[derive(Component)]
pub(super) struct PauseButton(usize);

#[derive(Component)]
pub(super) struct PauseNote;

/// Whether the race runs: for the race's systems that read keys.
pub(super) fn running(pause: Option<Res<State<RacePause>>>) -> bool {
    pause.is_none_or(|pause| *pause.get() == RacePause::Running)
}

pub(super) fn open_pause_dialog(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut pause: ResMut<NextState<RacePause>>,
) {
    let asked = keys.just_pressed(KeyCode::Escape)
        || gamepads
            .iter()
            .any(|gamepad| gamepad.any_just_pressed([GamepadButton::Start, GamepadButton::Select]));
    if asked {
        pause.set(RacePause::Paused);
    }
}

pub(super) fn stop_the_clock(mut time: ResMut<Time<Virtual>>, mut menu: ResMut<PauseMenu>) {
    time.pause();
    *menu = PauseMenu::default();
}

pub(super) fn start_the_clock(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

pub(super) fn spawn_pause_dialog(mut commands: Commands, settings: Res<RaceSettings>) {
    commands
        .spawn((
            PauseDialog,
            DespawnOnExit(RacePause::Paused),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            // Over the race readout, the results and the dashboard.
            GlobalZIndex(10),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(px(32)),
                        row_gap: px(14),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("PAUSED"),
                        TextFont::from_font_size(40.0),
                        TextColor(BLUE),
                        Node {
                            margin: UiRect::bottom(px(10)),
                            ..default()
                        },
                    ));
                    for (index, choice) in Choice::ALL.into_iter().enumerate() {
                        panel
                            .spawn((
                                Button,
                                PauseButton(index),
                                BackgroundColor(BUTTON_GREY),
                                Node {
                                    width: px(BUTTON_WIDTH),
                                    height: px(BUTTON_HEIGHT),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                            ))
                            .with_children(|button| {
                                button.spawn((
                                    Text::new(choice.label(settings.front_end)),
                                    TextFont::from_font_size(BUTTON_TEXT),
                                ));
                            });
                    }
                    panel.spawn((PauseNote, Text::default(), TextFont::from_font_size(16.0)));
                    panel.spawn((
                        Text::new("arrows move    Enter chooses    Esc continues"),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                    ));
                });
        });
}

/// Keys, the gamepad and the mouse, while the dialog is up.
#[expect(clippy::too_many_arguments, reason = "it reads every kind of input")]
pub(super) fn use_pause_dialog(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    buttons: Query<(&PauseButton, &Interaction), Changed<Interaction>>,
    real: Res<Time<Real>>,
    settings: Res<RaceSettings>,
    mut menu: ResMut<PauseMenu>,
    mut pause: ResMut<NextState<RacePause>>,
    mut game: ResMut<NextState<GameState>>,
    mut cancelled: MessageWriter<RaceCancelled>,
    mut exit: MessageWriter<AppExit>,
) {
    // Nothing happens until the picture is taken.
    if menu.capturing.is_some() {
        return;
    }
    let gamepad = |button| gamepads.iter().any(|gamepad| gamepad.just_pressed(button));
    let mut chosen = None;
    if keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW]) || gamepad(GamepadButton::DPadUp) {
        menu.step(false);
    }
    if keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS])
        || gamepad(GamepadButton::DPadDown)
    {
        menu.step(true);
    }
    if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space])
        || gamepad(GamepadButton::South)
    {
        chosen = Some(Choice::ALL[menu.highlighted]);
    }
    if keys.just_pressed(KeyCode::Escape)
        || gamepad(GamepadButton::East)
        || gamepad(GamepadButton::Start)
    {
        chosen = Some(Choice::Continue);
    }
    for (button, interaction) in &buttons {
        match interaction {
            Interaction::Hovered => menu.highlighted = button.0,
            Interaction::Pressed => {
                menu.highlighted = button.0;
                chosen = Some(Choice::ALL[button.0]);
            }
            Interaction::None => {}
        }
    }

    match chosen {
        None => {}
        Some(Choice::Continue) => pause.set(RacePause::Running),
        Some(Choice::Restart) => {
            // `set`, not `set_if_neq`: leaving and entering the same state is the point.
            game.set(GameState::Racing);
            // Leaves the pause too, which despawns the dialog and starts the clock.
            pause.set(RacePause::Running);
        }
        Some(Choice::Screenshot) => {
            menu.capturing = Some(real.elapsed());
            menu.note.clear();
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_screenshot);
        }
        Some(Choice::Cancel) if settings.front_end => {
            cancelled.write(RaceCancelled);
        }
        Some(Choice::Cancel) => {
            exit.write(AppExit::Success);
        }
    }
}

/// Gives up on a screenshot that never comes, and shows the dialog again.
pub(super) fn give_up_on_capture(real: Res<Time<Real>>, mut menu: ResMut<PauseMenu>) {
    if let Some(asked) = menu.capturing
        && (real.elapsed() - asked).as_secs_f32() > CAPTURE_TIMEOUT
    {
        menu.capturing = None;
        menu.note = "The screenshot was not taken.".into();
    }
}

pub(super) fn show_pause_dialog(
    menu: Res<PauseMenu>,
    mut dialog: Single<&mut Visibility, With<PauseDialog>>,
    mut buttons: Query<(&PauseButton, &mut BackgroundColor)>,
    mut note: Single<&mut Text, With<PauseNote>>,
) {
    dialog.set_if_neq(if menu.capturing.is_some() {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    });
    for (button, mut background) in &mut buttons {
        let color = if button.0 == menu.highlighted {
            BLUE
        } else {
            BUTTON_GREY
        };
        background.set_if_neq(BackgroundColor(color));
    }
    if note.0 != menu.note {
        note.0 = menu.note.clone();
    }
}

fn save_screenshot(captured: On<ScreenshotCaptured>, menu: Option<ResMut<PauseMenu>>) {
    let saved = next_screenshot(Path::new(SCREENSHOT_FOLDER))
        .and_then(|path| write_png(&path, captured.image.clone()).map(|()| path));
    let note = match saved {
        Ok(path) => {
            info!("Screenshot saved to {}", path.display());
            format!("Saved {}", path.display())
        }
        Err(error) => {
            warn!("The screenshot was not saved: {error}");
            format!("Not saved: {error}")
        }
    };
    // The race may have been left while the picture was on its way.
    if let Some(mut menu) = menu {
        menu.capturing = None;
        menu.note = note;
    }
}

/// The path of the next screenshot in `folder`, which is made if it is not there.
fn next_screenshot(folder: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(folder)
        .map_err(|error| format!("cannot make {}: {error}", folder.display()))?;
    let names = std::fs::read_dir(folder)
        .map_err(|error| format!("cannot read {}: {error}", folder.display()))?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok());
    Ok(folder.join(screenshot_name(names)))
}

/// `screenshot-0001.png`, numbered one after the highest already among `names`, so that
/// none is written over and they sort in the order they were taken.
fn screenshot_name(names: impl Iterator<Item = String>) -> String {
    let highest = names
        .filter_map(|name| {
            name.strip_prefix("screenshot-")?
                .strip_suffix(".png")?
                .parse::<u32>()
                .ok()
        })
        .max()
        .unwrap_or(0);
    format!("screenshot-{:04}.png", highest + 1)
}

fn write_png(path: &Path, image: Image) -> Result<(), String> {
    // Without alpha: with high dynamic range on, the window's alpha holds brightness.
    let pixels = image
        .try_into_dynamic()
        .map_err(|error| format!("{error:?}"))?
        .to_rgb8();
    let file = std::fs::File::create(path)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), pixels.width(), pixels.height());
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(pixels.as_raw()))
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_highlight_stops_at_the_ends() {
        let mut menu = PauseMenu::default();
        menu.step(false);
        assert_eq!(menu.highlighted, 0);
        for _ in 0..Choice::ALL.len() {
            menu.step(true);
        }
        assert_eq!(Choice::ALL[menu.highlighted], Choice::Cancel);
        menu.step(false);
        assert_eq!(Choice::ALL[menu.highlighted], Choice::Screenshot);
    }

    #[test]
    fn screenshots_are_numbered_after_the_highest() {
        let names = |names: &[&str]| {
            names
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            screenshot_name(names(&[]).into_iter()),
            "screenshot-0001.png"
        );
        assert_eq!(
            screenshot_name(
                names(&[
                    "screenshot-0002.png",
                    "screenshot-0010.png",
                    "screenshot-9.jpg",
                    "notes.txt",
                ])
                .into_iter()
            ),
            "screenshot-0011.png"
        );
    }

    #[test]
    fn cancelling_says_where_it_goes() {
        assert_eq!(Choice::Cancel.label(true), "CANCEL RACE");
        assert_eq!(Choice::Cancel.label(false), "QUIT GAME");
    }
}
