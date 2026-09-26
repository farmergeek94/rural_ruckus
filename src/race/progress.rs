//! How far through the race one truck is. Pure data and rules, with no ECS involved.
//!
//! Time is counted in physics ticks, so lap times don't depend on the frame rate.

use bevy::prelude::*;

use super::gate::crossed;
use crate::track::Gate;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RaceProgress {
    /// Index of the only gate that counts right now. Others are ignored until it has
    /// been crossed, which is what makes a skipped checkpoint block the lap.
    pub next_gate: usize,
    /// The lap being driven, starting at 1. 0 until the start line is first crossed.
    pub lap: u32,
    /// Tick at which the start line was first crossed.
    pub race_started: Option<u64>,
    /// Tick at which the current lap began.
    pub lap_started: Option<u64>,
    /// Duration of the most recently completed lap, in ticks.
    pub last_lap: Option<u64>,
    /// Duration of the fastest completed lap, in ticks.
    pub best_lap: Option<u64>,
    /// Duration of the whole race in ticks, once the final lap is complete.
    pub finished: Option<u64>,
}

/// What a step of movement achieved, if anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Milestone {
    RaceStarted,
    GatePassed,
    LapCompleted,
    RaceFinished,
}

impl RaceProgress {
    /// Accounts for the truck moving `from` one position `to` another during `tick`,
    /// in a race of `laps` laps around `gates`. Gate 0 is the start/finish line.
    pub fn advance(
        &mut self,
        gates: &[Gate],
        laps: u32,
        from: Vec3,
        to: Vec3,
        tick: u64,
    ) -> Option<Milestone> {
        if self.finished.is_some() || gates.is_empty() {
            return None;
        }
        if !crossed(&gates[self.next_gate], from, to) {
            return None;
        }

        let crossed_start_line = self.next_gate == 0;
        self.next_gate = (self.next_gate + 1) % gates.len();
        if !crossed_start_line {
            return Some(Milestone::GatePassed);
        }

        let (Some(race_started), Some(lap_started)) = (self.race_started, self.lap_started) else {
            self.lap = 1;
            self.race_started = Some(tick);
            self.lap_started = Some(tick);
            return Some(Milestone::RaceStarted);
        };

        let lap_time = tick - lap_started;
        self.last_lap = Some(lap_time);
        self.best_lap = Some(self.best_lap.map_or(lap_time, |best| best.min(lap_time)));
        if self.lap >= laps {
            self.finished = Some(tick - race_started);
            Some(Milestone::RaceFinished)
        } else {
            self.lap += 1;
            self.lap_started = Some(tick);
            Some(Milestone::LapCompleted)
        }
    }

    /// The gate most recently driven through, if any: where to put a truck that asks to
    /// go back to its last checkpoint.
    pub fn last_gate(&self, gate_count: usize) -> Option<usize> {
        (self.race_started.is_some() && gate_count > 0)
            .then(|| (self.next_gate + gate_count - 1) % gate_count)
    }

    /// Gates driven through since the race began, the start line included each time.
    pub fn gates_passed(&self, gate_count: usize) -> u64 {
        match self.last_gate(gate_count) {
            Some(last_gate) if self.finished.is_some() => {
                // The line that ended the last lap is one more than the laps' own gates.
                debug_assert_eq!(last_gate, 0);
                self.lap as u64 * gate_count as u64 + 1
            }
            Some(last_gate) => (self.lap as u64 - 1) * gate_count as u64 + last_gate as u64 + 1,
            None => 0,
        }
    }

    /// The tick at which the race was finished, which is what says who finished first:
    /// `finished` is counted from each truck's own crossing of the start line.
    fn finished_at(&self) -> Option<u64> {
        Some(self.race_started? + self.finished?)
    }

    /// Whether this truck is ahead of `other` in the race. Whoever finished first, then
    /// whoever has driven through more gates, then whoever is nearer its next gate, which
    /// `to_next_gate` and `other_to_next_gate` say in metres.
    fn is_ahead_of(
        &self,
        to_next_gate: f32,
        other: &RaceProgress,
        other_to_next_gate: f32,
        gate_count: usize,
    ) -> bool {
        match (self.finished_at(), other.finished_at()) {
            (Some(mine), Some(theirs)) => mine < theirs,
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => {
                let (mine, theirs) = (
                    self.gates_passed(gate_count),
                    other.gates_passed(gate_count),
                );
                mine > theirs || (mine == theirs && to_next_gate < other_to_next_gate)
            }
        }
    }

    /// Ticks spent on the current lap so far.
    pub fn current_lap_time(&self, tick: u64) -> Option<u64> {
        match self.finished {
            Some(_) => None,
            None => self.lap_started.map(|started| tick - started),
        }
    }
}

/// The place in the race of the truck `of`, where 1 is the lead: one more than the number
/// of `others` ahead of it. Each truck is its progress and the metres to its next gate.
pub fn position<'a>(
    of: (&RaceProgress, f32),
    others: impl IntoIterator<Item = (&'a RaceProgress, f32)>,
    gate_count: usize,
) -> usize {
    1 + others
        .into_iter()
        .filter(|(other, to_next_gate)| other.is_ahead_of(*to_next_gate, of.0, of.1, gate_count))
        .count()
}

/// The order of a race: the indices of `racers`, the leader first. Each racer is its
/// progress and the metres to its next gate, as for `position`.
pub fn standings(racers: &[(&RaceProgress, f32)], gate_count: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..racers.len()).collect();
    order.sort_by_key(|&index| {
        let others = racers
            .iter()
            .enumerate()
            .filter(|&(other, _)| other != index)
            .map(|(_, &racer)| racer);
        position(racers[index], others, gate_count)
    });
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three gates along a line heading -Z, at z = 0, -100 and -200. Not a real loop,
    /// but progress only cares about the order gates are crossed in.
    fn gates() -> Vec<Gate> {
        [0.0, -100.0, -200.0]
            .into_iter()
            .map(|z| Gate {
                center: Vec2::new(0.0, z),
                yaw: 0.0,
                half_width: 10.0,
            })
            .collect()
    }

    /// Drives through gate `index` during `tick`.
    fn pass(
        progress: &mut RaceProgress,
        gates: &[Gate],
        laps: u32,
        index: usize,
        tick: u64,
    ) -> Option<Milestone> {
        let z = gates[index].center.y;
        progress.advance(
            gates,
            laps,
            Vec3::new(0.0, 0.0, z + 1.0),
            Vec3::new(0.0, 0.0, z - 1.0),
            tick,
        )
    }

    #[test]
    fn nothing_happens_before_the_start_line() {
        let gates = gates();
        let mut progress = RaceProgress::default();
        assert_eq!(pass(&mut progress, &gates, 3, 1, 5), None);
        assert_eq!(progress, RaceProgress::default());
        assert_eq!(progress.last_gate(gates.len()), None);
        assert_eq!(progress.current_lap_time(50), None);
    }

    #[test]
    fn a_full_race() {
        let gates = gates();
        let mut progress = RaceProgress::default();

        assert_eq!(
            pass(&mut progress, &gates, 2, 0, 100),
            Some(Milestone::RaceStarted)
        );
        assert_eq!((progress.lap, progress.next_gate), (1, 1));
        assert_eq!(progress.current_lap_time(130), Some(30));
        assert_eq!(progress.last_gate(gates.len()), Some(0));

        assert_eq!(
            pass(&mut progress, &gates, 2, 1, 200),
            Some(Milestone::GatePassed)
        );
        assert_eq!(
            pass(&mut progress, &gates, 2, 2, 300),
            Some(Milestone::GatePassed)
        );
        assert_eq!(progress.last_gate(gates.len()), Some(2));
        assert_eq!(
            pass(&mut progress, &gates, 2, 0, 1100),
            Some(Milestone::LapCompleted)
        );
        assert_eq!((progress.lap, progress.next_gate), (2, 1));
        assert_eq!(
            (progress.last_lap, progress.best_lap),
            (Some(1000), Some(1000))
        );
        assert_eq!(progress.current_lap_time(1150), Some(50));

        // A faster second lap.
        pass(&mut progress, &gates, 2, 1, 1200);
        pass(&mut progress, &gates, 2, 2, 1300);
        assert_eq!(
            pass(&mut progress, &gates, 2, 0, 1700),
            Some(Milestone::RaceFinished)
        );
        assert_eq!(
            (progress.last_lap, progress.best_lap),
            (Some(600), Some(600))
        );
        assert_eq!(progress.finished, Some(1600));
        assert_eq!(progress.lap, 2);
        assert_eq!(progress.current_lap_time(1800), None);

        // Nothing changes after the finish.
        let finished = progress.clone();
        for index in [1, 2, 0] {
            assert_eq!(pass(&mut progress, &gates, 2, index, 2000), None);
        }
        assert_eq!(progress, finished);
    }

    #[test]
    fn position_goes_by_the_finish_then_gates_then_the_distance_to_the_next() {
        let gates = gates();
        let count = gates.len();
        // Nobody has crossed the line: the nearest to it leads.
        let (mut leader, mut chaser) = (RaceProgress::default(), RaceProgress::default());
        assert_eq!(position((&leader, 5.0), [(&chaser, 9.0)], count), 1);
        assert_eq!(position((&chaser, 9.0), [(&leader, 5.0)], count), 2);
        // Level is nobody ahead.
        assert_eq!(position((&chaser, 5.0), [(&leader, 5.0)], count), 1);

        // A gate in hand beats being near the next one, and a lap beats both.
        pass(&mut leader, &gates, 1, 0, 10);
        assert_eq!(leader.gates_passed(count), 1);
        assert_eq!(position((&leader, 90.0), [(&chaser, 1.0)], count), 1);
        // The chaser crossed the line before the leader did, so its own race is longer.
        pass(&mut chaser, &gates, 1, 0, 2);
        pass(&mut chaser, &gates, 1, 1, 30);
        assert_eq!(chaser.gates_passed(count), 2);
        assert_eq!(position((&leader, 1.0), [(&chaser, 90.0)], count), 2);

        // Finishing first wins, however quick the other's own race was.
        pass(&mut chaser, &gates, 1, 2, 40);
        assert_eq!(chaser.gates_passed(count), 3);
        pass(&mut chaser, &gates, 1, 0, 50);
        assert_eq!(chaser.gates_passed(count), 4);
        assert_eq!(position((&leader, 1.0), [(&chaser, 0.0)], count), 2);
        pass(&mut leader, &gates, 1, 1, 51);
        pass(&mut leader, &gates, 1, 2, 52);
        pass(&mut leader, &gates, 1, 0, 53);
        assert!(leader.finished < chaser.finished);
        assert_eq!(position((&leader, 0.0), [(&chaser, 0.0)], count), 2);
        assert_eq!(position((&chaser, 0.0), [(&leader, 0.0)], count), 1);
    }

    #[test]
    fn best_lap_keeps_the_fastest() {
        let gates = gates();
        let mut progress = RaceProgress::default();
        let mut tick = 0;
        for lap_time in [0, 500, 400, 450] {
            tick += lap_time;
            pass(&mut progress, &gates, 9, 0, tick);
            pass(&mut progress, &gates, 9, 1, tick);
            pass(&mut progress, &gates, 9, 2, tick);
        }
        assert_eq!(
            (progress.last_lap, progress.best_lap),
            (Some(450), Some(400))
        );
    }

    #[test]
    fn a_skipped_gate_blocks_the_lap_until_it_is_driven() {
        let gates = gates();
        let mut progress = RaceProgress::default();
        pass(&mut progress, &gates, 3, 0, 0);

        // Misses gate 1, then carries on round.
        assert_eq!(pass(&mut progress, &gates, 3, 2, 10), None);
        assert_eq!(pass(&mut progress, &gates, 3, 0, 20), None);
        assert_eq!((progress.lap, progress.next_gate), (1, 1));
        assert_eq!(progress.last_lap, None);

        // Goes back for it.
        assert_eq!(
            pass(&mut progress, &gates, 3, 1, 30),
            Some(Milestone::GatePassed)
        );
        assert_eq!(
            pass(&mut progress, &gates, 3, 2, 40),
            Some(Milestone::GatePassed)
        );
        assert_eq!(
            pass(&mut progress, &gates, 3, 0, 50),
            Some(Milestone::LapCompleted)
        );
    }

    #[test]
    fn going_the_wrong_way_through_a_gate_is_ignored() {
        let gates = gates();
        let mut progress = RaceProgress::default();
        let backwards = progress.advance(
            &gates,
            3,
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, 1.0),
            0,
        );
        assert_eq!(backwards, None);
        assert_eq!(progress, RaceProgress::default());
    }

    #[test]
    fn standings_put_the_finished_first_then_the_furthest_on() {
        let gates = gates();
        let mut finished = RaceProgress::default();
        for (tick, gate) in [0, 1, 2, 0].into_iter().enumerate() {
            pass(&mut finished, &gates, 1, gate, tick as u64);
        }
        assert!(finished.finished.is_some());
        let mut on_lap = RaceProgress::default();
        pass(&mut on_lap, &gates, 1, 0, 0);
        let not_started = RaceProgress::default();
        let racers = [
            (&not_started, 50.0),
            (&on_lap, 90.0),
            (&finished, 0.0),
            (&on_lap, 10.0),
        ];
        assert_eq!(standings(&racers, gates.len()), [2, 3, 1, 0]);
    }
}
