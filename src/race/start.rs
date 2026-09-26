//! The start: every truck is held on the grid (`truck::Held`) while the count goes three,
//! two, one, and is let go on GO. A restart counts down again.
//!
//! The count is kept in physics ticks of the race clock, as the race times are, so that
//! every truck is let go in the same step whatever the frame rate. How long MTM2's start
//! takes is not measured: `COUNTDOWN` is the game's own.
//!
//! Each number jumps out large and shrinks back as its second goes by, and the screen
//! flashes in the application's blue (`BLUE`) as it comes; GO is larger, white, and
//! flashes harder.

use bevy::prelude::*;

use super::{RaceClock, Racer};
use crate::game_state::GameState;
use crate::truck::Held;

/// How long the trucks are held before GO, in seconds: one for each number counted.
const COUNTDOWN: f32 = 3.0;
/// How long GO stays on the screen after the start, in seconds.
const GO_SHOWN: f32 = 1.0;

/// The application's standard colour: the front end's accent (`ui::Theme::accent`), which
/// a test in `tests/front_end.rs` checks it is. The race may not use `ui`, so it is here too.
pub const BLUE: Color = Color::srgb(0.2, 0.5, 0.96);
/// How big a number is when it has settled, in pixels, and how much bigger it is as it
/// comes. GO is `GO_SCALE` times as big.
const NUMBER_SIZE: f32 = 150.0;
const POP: f32 = 0.9;
const GO_SCALE: f32 = 1.4;
/// How opaque the blue flash over the screen is as a number comes, and as GO comes, from
/// 0 to 1.
const FLASH: f32 = 0.35;
const GO_FLASH: f32 = 0.6;

/// When the trucks are let go: the race clock's tick at GO.
#[derive(Resource, Default, Debug)]
pub struct RaceStart {
    pub go: u64,
}

impl RaceStart {
    /// Whether the trucks are still held at `tick`.
    pub fn holding(&self, tick: u64) -> bool {
        tick < self.go
    }
}

/// How the count looks at one moment.
#[derive(Clone, Debug, PartialEq)]
struct CountLook {
    text: String,
    /// In pixels.
    size: f32,
    color: Color,
    /// How opaque the text is, and the blue flash over the screen, from 0 to 1.
    opacity: f32,
    flash: f32,
}

/// The count as the player sees it at `tick`: 3, 2 and 1 before GO, then GO for a while,
/// then nothing. `tick_seconds` is the length of one tick.
fn look(start: &RaceStart, tick: u64, tick_seconds: f32) -> Option<CountLook> {
    if start.holding(tick) {
        // In whole ticks: a second of them in floating point can come to a hair over one.
        let per_second = ((1.0 / tick_seconds).round() as u64).max(1);
        let left = start.go - tick;
        let number = left.div_ceil(per_second);
        // How far through its second the number is, from 0 as it comes towards 1.
        let through = 1.0 - (left - (number - 1) * per_second) as f32 / per_second as f32;
        let fresh = (1.0 - through).powi(3);
        return Some(CountLook {
            text: format!("{number}"),
            size: NUMBER_SIZE * (1.0 + POP * fresh),
            color: BLUE,
            opacity: (1.0 - through * through).max(0.25),
            flash: FLASH * fresh,
        });
    }
    let since = (tick - start.go) as f32 * tick_seconds;
    (since < GO_SHOWN).then(|| {
        let through = since / GO_SHOWN;
        let fresh = (1.0 - through).powi(3);
        CountLook {
            text: "GO!".into(),
            size: NUMBER_SIZE * GO_SCALE * (1.0 + POP * fresh),
            color: Color::WHITE,
            opacity: 1.0 - through * through,
            flash: GO_FLASH * fresh,
        }
    })
}

/// Counts down from now, at the race clock's tick.
pub(super) fn count_down(start: &mut RaceStart, tick: u64, tick_seconds: f32) {
    start.go = tick + (COUNTDOWN / tick_seconds).round() as u64;
}

pub(super) fn begin_countdown(
    clock: Res<RaceClock>,
    time: Res<Time<Fixed>>,
    mut start: ResMut<RaceStart>,
) {
    count_down(&mut start, clock.tick, time.timestep().as_secs_f32());
}

/// Holds every racer until GO, and lets it go then. Only on a change, so that nothing
/// that watches for `Held` sees one every step.
pub(super) fn hold_at_start(
    mut commands: Commands,
    clock: Res<RaceClock>,
    start: Res<RaceStart>,
    racers: Query<(Entity, Has<Held>), With<Racer>>,
) {
    let holding = start.holding(clock.tick);
    for (truck, held) in &racers {
        if holding && !held {
            commands.entity(truck).insert(Held);
        } else if !holding && held {
            commands.entity(truck).remove::<Held>();
        }
    }
}

#[derive(Component)]
pub(super) struct CountText;

/// The blue flash over the whole screen.
#[derive(Component)]
pub(super) struct CountFlash;

pub(super) fn spawn_count(mut commands: Commands) {
    commands.spawn((
        CountFlash,
        DespawnOnExit(GameState::Racing),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::NONE),
        // Under the rest of what is on the screen.
        GlobalZIndex(-1),
    ));
    commands.spawn((
        CountText,
        DespawnOnExit(GameState::Racing),
        Text::default(),
        TextFont::from_font_size(NUMBER_SIZE),
        TextColor(BLUE),
        TextShadow {
            offset: Vec2::splat(6.0),
            color: Color::srgba(0.0, 0.0, 0.0, 0.6),
        },
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            top: Val::Percent(22.0),
            ..default()
        },
    ));
}

pub(super) fn show_count(
    clock: Res<RaceClock>,
    start: Res<RaceStart>,
    time: Res<Time<Fixed>>,
    text: Single<(&mut Text, &mut TextFont, &mut TextColor), With<CountText>>,
    mut flash: Single<&mut BackgroundColor, With<CountFlash>>,
) {
    let (mut text, mut font, mut color) = text.into_inner();
    let look = look(&start, clock.tick, time.timestep().as_secs_f32());
    let shown = look
        .as_ref()
        .map_or(String::new(), |look| look.text.clone());
    if text.0 != shown {
        text.0 = shown;
    }
    let Some(look) = look else {
        flash.set_if_neq(BackgroundColor(Color::NONE));
        return;
    };
    let size = FontSize::Px(look.size);
    if font.font_size != size {
        font.font_size = size;
    }
    color.set_if_neq(TextColor(look.color.with_alpha(look.opacity)));
    flash.set_if_neq(BackgroundColor(BLUE.with_alpha(look.flash)));
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICK: f32 = 1.0 / 120.0;

    #[test]
    fn it_counts_three_two_one_go_then_nothing() {
        let mut start = RaceStart::default();
        count_down(&mut start, 10, TICK);
        assert_eq!(start.go, 10 + 360);
        let at = |seconds: f32| {
            look(&start, 10 + (seconds / TICK).round() as u64, TICK).map(|look| look.text)
        };
        assert_eq!(at(0.0).as_deref(), Some("3"));
        assert_eq!(at(0.5).as_deref(), Some("3"));
        assert_eq!(at(1.5).as_deref(), Some("2"));
        assert_eq!(at(2.5).as_deref(), Some("1"));
        assert_eq!(at(3.0).as_deref(), Some("GO!"));
        assert_eq!(at(3.0 + GO_SHOWN), None);
    }

    #[test]
    fn each_number_jumps_out_and_settles_with_a_flash() {
        let mut start = RaceStart::default();
        count_down(&mut start, 0, TICK);
        let at = |seconds: f32| look(&start, (seconds / TICK).round() as u64, TICK).unwrap();
        let coming = at(1.0);
        let settled = at(1.9);
        assert_eq!(coming.text, "2");
        assert!(coming.size > settled.size && coming.flash > settled.flash);
        assert_eq!(coming.color, BLUE);
        let go = at(3.0);
        assert!(go.size > coming.size && go.flash > coming.flash);
    }

    #[test]
    fn the_trucks_are_held_until_go() {
        let mut start = RaceStart::default();
        count_down(&mut start, 0, TICK);
        assert!(start.holding(0));
        assert!(start.holding(start.go - 1));
        assert!(!start.holding(start.go));
    }
}
