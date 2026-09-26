//! Steps the game clock evenly, because the screen shows frames evenly.
//!
//! Under vsync every frame is on screen for one refresh of the display, but the moment a
//! frame *starts*, which is all the clock can measure, comes early or late by whatever else
//! the computer is doing: on the development machine a quarter of all frames start 4 to 13 ms
//! late, each followed by one as much shorter. Everything drawn is placed by the clock, so a
//! clock that believes those measurements places everything unevenly: a truck circling at
//! 62 deg/s turns the picture 1.4 degrees in one frame and 0.6 in the next, where 1.04 every
//! frame is what happened (measured headless).
//!
//! So the clock is advanced by what a frame really lasts: the refresh interval, which is the
//! mean length of recent frames. Lateness cancels out of a mean exactly, since a frame that
//! starts late makes the next one short, so sixty frames give the interval to a fraction of a
//! percent however wobbly each one is. What is left over, real time the game hasn't been
//! given yet, is fed back a hundredth at a time, which keeps the game in step with the real
//! clock without any frame's step differing visibly from the last.
//!
//! - A frame that really missed a refresh (the scene got heavy) is made up over the next
//!   second, a percent or two fast. It is not made up at once: nothing in one measurement
//!   tells a missed refresh from a late start, and guessing wrong is a jump.
//! - A real stall (a track loading) is passed on whole, and changes nothing else.
//! - A change of frame rate is followed over a second or two.
//!
//! An earlier version averaged twenty frames and took anything over 1.75 times the average
//! for a stall. Twenty is too few (the step wandered by 4% from frame to frame) and 1.75 is
//! inside what a late frame reaches, so ordinary frames were passed on whole and the
//! averaging begun again: a jerk every few seconds.
//!
//! Only for vsync: without it the screen shows each frame as soon as it is done, and the
//! measured times are the true ones. `FramePacing::on` says which, and `display` keeps it
//! in step with the window. Added by `main.rs` only. Tests drive time themselves.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::time::{TimeSystems, TimeUpdateStrategy};

pub struct FramePacingPlugin;

impl Plugin for FramePacingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FrameClock>()
            .init_resource::<FramePacing>()
            .add_systems(First, pace_the_clock.before(TimeSystems));
    }
}

/// Whether the clock is paced. On while frames wait for the screen, and off otherwise.
#[derive(Resource)]
pub struct FramePacing {
    pub on: bool,
}

impl Default for FramePacing {
    fn default() -> Self {
        Self { on: true }
    }
}

/// How many frames the refresh interval is taken over: a second at 60 Hz. The error of the
/// mean is the lateness of its first and last frames divided by this.
const WINDOW: usize = 60;
/// How much of the way the interval in use moves to the newest mean each frame. The mean
/// still changes a little with every frame, and this keeps that out of the step.
const REFRESH_SMOOTHING: f64 = 1.0 / 30.0;
/// The same for the real time not yet given to the game, which swings by a whole late frame
/// from one frame to the next and means something only on average.
const BEHIND_SMOOTHING: f64 = 1.0 / 16.0;
/// How much of that is added to each step. Small, so that making up a missed refresh never
/// speeds the game up by more than a percent or two.
const CATCH_UP: f64 = 0.01;
/// How much of whatever is more than a whole refresh behind is added as well: a real backlog,
/// as when the frame rate has just halved, and worth running visibly fast for a moment.
const BACKLOG_CATCH_UP: f64 = 0.15;
/// A frame this many refreshes long is a stall. Well clear of a late frame, which is under
/// two, and of one that missed a refresh and started late besides, which is under three.
const STALL_FACTOR: f64 = 4.0;
/// Frames measured before the interval is believed at all. Until then the clock is the raw one.
const WARM_UP: usize = 8;

#[derive(Resource, Default)]
struct FrameClock {
    last_frame: Option<Instant>,
    pacer: Pacer,
}

fn pace_the_clock(
    pacing: Res<FramePacing>,
    mut clock: ResMut<FrameClock>,
    mut strategy: ResMut<TimeUpdateStrategy>,
) {
    if !pacing.on {
        // Back to Bevy's own clock, and the next pacing starts afresh: frames measured
        // without vsync say nothing of the refresh.
        if !matches!(*strategy, TimeUpdateStrategy::Automatic) {
            *strategy = TimeUpdateStrategy::Automatic;
            *clock = FrameClock::default();
        }
        return;
    }
    let now = Instant::now();
    let measured = clock
        .last_frame
        .map_or(Duration::ZERO, |last| now.duration_since(last));
    clock.last_frame = Some(now);
    *strategy = TimeUpdateStrategy::ManualDuration(clock.pacer.step(measured));
}

/// Turns measured frame lengths into the steps the game clock takes.
#[derive(Default)]
pub struct Pacer {
    /// In seconds.
    recent: VecDeque<f64>,
    /// The refresh interval in use, in seconds. `None` until a frame has been measured.
    refresh: Option<f64>,
    /// Real time that has passed and not been given to the game, in seconds, and the same
    /// smoothed. Negative when the game is ahead.
    behind: f64,
    lingering: f64,
}

impl Pacer {
    pub fn step(&mut self, measured: Duration) -> Duration {
        if measured.is_zero() {
            return measured;
        }
        let seconds = measured.as_secs_f64();
        if self.recent.len() >= WARM_UP && seconds > STALL_FACTOR * self.typical() {
            // Passed on as it is, and kept out of the interval and out of the debt: the
            // frames after it are paced as the frames before it were.
            return measured;
        }

        self.recent.push_back(seconds);
        if self.recent.len() > WINDOW {
            self.recent.pop_front();
        }
        // A game's first frames take seconds, and would sit in the mean for sixty more.
        let typical = self.typical();
        self.recent.retain(|&frame| frame <= STALL_FACTOR * typical);

        let mean = self.recent.iter().sum::<f64>() / self.recent.len() as f64;
        let refresh = match self.refresh {
            Some(refresh) if self.recent.len() > WARM_UP => {
                refresh + (mean - refresh) * REFRESH_SMOOTHING
            }
            _ => {
                self.refresh = Some(mean);
                return measured;
            }
        };
        self.refresh = Some(refresh);

        self.behind += seconds;
        let backlog = (self.lingering.abs() - refresh).max(0.0) * self.lingering.signum();
        let step = (refresh + CATCH_UP * self.lingering + BACKLOG_CATCH_UP * backlog).max(0.0);
        self.behind -= step;
        self.lingering += (self.behind - self.lingering) * BEHIND_SMOOTHING;
        Duration::from_secs_f64(step)
    }

    /// The median of the recent frames: what an ordinary frame is, whatever a few
    /// extraordinary ones were. Too coarse to pace by, and just right to tell a stall by.
    fn typical(&self) -> f64 {
        let mut sorted: Vec<f64> = self.recent.iter().copied().collect();
        sorted.sort_by(f64::total_cmp);
        sorted[sorted.len() / 2]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REFRESH: f64 = 1.0 / 60.0;

    fn millis(value: f64) -> Duration {
        Duration::from_secs_f64(value / 1000.0)
    }

    /// The moments frames start at: `refresh` apart, each up to `lateness` seconds early
    /// or late, with a whole refresh missed before each of `missed`. As frame lengths.
    fn frames(count: usize, refresh: f64, lateness: f64, missed: &[usize]) -> Vec<Duration> {
        // A small generator of its own, so that the test is the same every time.
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut random = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
        };
        let (mut on_time, mut last) = (0.0, 0.0_f64);
        (0..count)
            .map(|frame| {
                on_time += refresh * if missed.contains(&frame) { 2.0 } else { 1.0 };
                let start = (on_time + lateness * random()).max(last + 0.001);
                let length = start - last;
                last = start;
                Duration::from_secs_f64(length)
            })
            .collect()
    }

    struct Paced {
        /// In milliseconds, after the first two hundred frames.
        steps: Vec<f64>,
        /// The furthest the game clock got from the real one, in milliseconds.
        apart: f64,
    }

    fn pace(measured: &[Duration]) -> Paced {
        let mut pacer = Pacer::default();
        let (mut real, mut game, mut apart) = (0.0, 0.0, 0.0_f64);
        let mut steps = Vec::new();
        for (frame, &measured) in measured.iter().enumerate() {
            let step = pacer.step(measured);
            real += measured.as_secs_f64() * 1000.0;
            game += step.as_secs_f64() * 1000.0;
            if frame >= 200 {
                steps.push(step.as_secs_f64() * 1000.0);
                apart = apart.max((real - game).abs());
            }
        }
        Paced { steps, apart }
    }

    fn least_and_most(steps: &[f64]) -> (f64, f64) {
        (
            steps.iter().copied().fold(f64::MAX, f64::min),
            steps.iter().copied().fold(0.0, f64::max),
        )
    }

    fn worst_change(steps: &[f64]) -> f64 {
        steps
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0, f64::max)
    }

    #[test]
    fn the_logged_frames_become_even_steps() {
        // As logged under vsync on the development machine, with the two frames that the
        // old clock took for stalls.
        let logged = [16.7, 23.0, 10.0, 16.7, 30.0, 9.4, 16.7, 11.3];
        let measured: Vec<Duration> = (0..1200).map(|frame| millis(logged[frame % 8])).collect();
        let paced = pace(&measured);
        let (least, most) = least_and_most(&paced.steps);
        assert!(most - least < 0.05, "steps from {least} to {most} ms");
        assert!(paced.apart < 15.0, "{} ms from the real clock", paced.apart);
    }

    #[test]
    fn frames_that_start_early_and_late_become_even_steps() {
        let paced = pace(&frames(3000, REFRESH, 0.007, &[]));
        let (least, most) = least_and_most(&paced.steps);
        assert!(
            least > 16.5 && most < 16.85,
            "steps from {least} to {most} ms"
        );
        // A hundredth of a millisecond is a thousandth of a degree in the tightest turn.
        let change = worst_change(&paced.steps);
        assert!(change < 0.03, "a step changed by {change} ms");
        assert!(paced.apart < 25.0, "{} ms from the real clock", paced.apart);
    }

    #[test]
    fn a_missed_refresh_is_made_up_gently() {
        let missed: Vec<usize> = (300..3000).step_by(230).collect();
        let paced = pace(&frames(3000, REFRESH, 0.007, &missed));
        let (least, most) = least_and_most(&paced.steps);
        // Never more than a few percent fast, and never a jump.
        assert!(
            least > 16.5 && most < 17.2,
            "steps from {least} to {most} ms"
        );
        assert!(worst_change(&paced.steps) < 0.05);
        assert!(paced.apart < 35.0, "{} ms from the real clock", paced.apart);
    }

    #[test]
    fn a_heavy_scene_missing_refreshes_all_the_time_is_still_even() {
        let missed: Vec<usize> = (300..3000).step_by(15).collect();
        let paced = pace(&frames(3000, REFRESH, 0.007, &missed));
        let change = worst_change(&paced.steps);
        assert!(change < 0.5, "a step changed by {change} ms");
        assert!(paced.apart < 70.0, "{} ms from the real clock", paced.apart);
    }

    #[test]
    fn a_stall_is_passed_on_whole_and_changes_nothing_else() {
        let mut measured = frames(1000, REFRESH, 0.007, &[]);
        measured[600] = millis(250.0);
        let mut pacer = Pacer::default();
        let steps: Vec<Duration> = measured.iter().map(|&frame| pacer.step(frame)).collect();
        assert_eq!(steps[600], millis(250.0));
        let change = steps[601].abs_diff(steps[599]);
        assert!(change < millis(0.05), "{change:?}");
    }

    #[test]
    fn the_first_frames_of_a_game_are_forgotten() {
        // As logged: a window opening, a track loading, shaders compiling.
        let mut measured: Vec<Duration> = [3220.0, 197.9, 271.2, 51.5, 30.0, 33.9]
            .into_iter()
            .map(millis)
            .collect();
        measured.extend(frames(400, REFRESH, 0.007, &[]));
        let mut pacer = Pacer::default();
        let steps: Vec<f64> = measured
            .iter()
            .map(|&frame| pacer.step(frame).as_secs_f64() * 1000.0)
            .collect();
        // Raw while it finds its feet, then a refresh at a time, never several at once.
        let (_, most) = least_and_most(&steps[12..]);
        assert!(most < 30.0, "a step of {most} ms");
        let (least, most) = least_and_most(&steps[150..]);
        assert!(least > 16.4 && most < 16.95, "{least} to {most} ms");
    }

    #[test]
    fn follows_a_change_of_frame_rate() {
        // A steady 60, then a steady 30 on the same screen.
        let mut measured = frames(600, REFRESH, 0.004, &[]);
        measured.extend(frames(900, 2.0 * REFRESH, 0.004, &[]));
        let mut pacer = Pacer::default();
        let steps: Vec<f64> = measured
            .iter()
            .map(|&frame| pacer.step(frame).as_secs_f64() * 1000.0)
            .collect();
        // Smoothly there, and settled.
        assert!(worst_change(&steps[200..]) < 2.0);
        let (least, most) = least_and_most(&steps[1200..]);
        assert!(least > 32.5 && most < 34.2, "{least} to {most} ms");
    }

    #[test]
    fn a_fast_screen_is_paced_as_well() {
        let paced = pace(&frames(3000, 1.0 / 144.0, 0.002, &[]));
        let (least, most) = least_and_most(&paced.steps);
        assert!(least > 6.9 && most < 7.0, "steps from {least} to {most} ms");
    }

    #[test]
    fn a_frame_of_no_length_is_a_step_of_none() {
        assert_eq!(Pacer::default().step(Duration::ZERO), Duration::ZERO);
    }
}
