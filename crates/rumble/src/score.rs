//! The scoring of a Summit Rumble, as MTM2's help file states it ("Scoring: Summit
//! Rumbles", quoted in `docs/formats/situation.md`): 10 points for each second on the
//! summit and in the scoring zone, 1 point lost for each second off the summit, and 50
//! lost each time a truck is booted, or falls, off it. Points are counted continuously,
//! so that a step of any length scores its share; whether MTM2 counted whole seconds is
//! open. A truck on the summit outside the scoring zone neither earns nor loses.

use bevy::math::{Vec3, Vec3Swizzles};
use content::track::Zone;

/// Points a second on the summit and in the scoring zone.
pub const ON_ZONE_PER_SECOND: f32 = 10.0;
/// Points a second off the summit: a loss.
pub const OFF_SUMMIT_PER_SECOND: f32 = -1.0;
/// Points lost each time a truck leaves the summit.
pub const BOOTED: f32 = -50.0;
/// How far under the summit zone's bottom the surface a truck stands on may be for the
/// truck to be on the summit, in metres: about how high its middle rides above it, so
/// that a truck standing on the slope out of one of Hypercube Rumble's pits is on the
/// summit just where the zone's box would have its middle. The zone is a box from the
/// track, whose bottom is a little under the summit's top on Hypercube Rumble (0.2 m)
/// and 6 m under it on Arena Rumble, where a truck in the dip beside the platform, or
/// under it, has its middle inside the box. Measured at 2.0, drivers held the slope's
/// lower reaches and were pushed off it a little, a fall each time (13 of 14 falls in
/// three minutes).
pub const UNDER_SUMMIT: f32 = 1.2;

/// Whether a truck whose middle is at `middle`, standing on a surface `surface` high
/// (the ground, or the top of a ground box, under its middle), is on the `summit`: under
/// the zone and no higher than its top, on a surface no more than `UNDER_SUMMIT` under
/// the zone's bottom. Where the truck stands, not where its middle is: a truck hanging
/// at a rim with its front wheels over it, or thrown up off the top, is still on it, and
/// one in a pit or beside the summit below its top is not, whatever the zone's box says.
pub fn on_summit(summit: &Zone, middle: Vec3, surface: f32) -> bool {
    summit.covers(middle.xz()) && middle.y <= summit.top && surface >= summit.bottom - UNDER_SUMMIT
}

/// One truck's score.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Score {
    pub points: f32,
    /// Whether it was on the summit at the last step.
    pub on_summit: bool,
    /// How many times it has been booted, or fallen, off the summit.
    pub falls: u32,
    /// How long it has spent earning points, in seconds.
    pub scoring_for: f32,
}

/// What a step of the score noticed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// It got onto the summit.
    Climbed,
    /// It left the summit, and paid for it.
    Fell,
}

impl Score {
    /// Scores `dt` seconds with the truck `on_summit` or not, and in the `scoring` zone
    /// or not. Only a truck on the summit can be in the scoring zone.
    pub fn step(&mut self, on_summit: bool, scoring: bool, dt: f32) -> Option<Event> {
        let event = match (self.on_summit, on_summit) {
            (true, false) => {
                self.points += BOOTED;
                self.falls += 1;
                Some(Event::Fell)
            }
            (false, true) => Some(Event::Climbed),
            _ => None,
        };
        self.on_summit = on_summit;
        if on_summit {
            if scoring {
                self.points += ON_ZONE_PER_SECOND * dt;
                self.scoring_for += dt;
            }
        } else {
            self.points += OFF_SUMMIT_PER_SECOND * dt;
        }
        event
    }

    /// Whether this score beats `other`: more points, and with the same points, fewer
    /// falls.
    fn beats(&self, other: &Score) -> bool {
        self.points > other.points || (self.points == other.points && self.falls < other.falls)
    }
}

/// The place of the score `of`, where 1 is the lead: one more than the `others` that beat
/// it.
pub fn position<'a>(of: &Score, others: impl IntoIterator<Item = &'a Score>) -> usize {
    1 + others.into_iter().filter(|other| other.beats(of)).count()
}

/// The order of the rumble: the indices of `scores`, the leader first. Equal scores keep
/// their order.
pub fn standings(scores: &[&Score]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by_key(|&index| {
        let others = scores
            .iter()
            .enumerate()
            .filter(|&(other, _)| other != index)
            .map(|(_, &score)| score);
        position(scores[index], others)
    });
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_the_summit_in_the_zone_earns_ten_a_second() {
        let mut score = Score::default();
        assert_eq!(score.step(true, true, 0.5), Some(Event::Climbed));
        assert_eq!(score.step(true, true, 0.5), None);
        assert_eq!(score.points, 10.0);
        assert_eq!(score.scoring_for, 1.0);
    }

    #[test]
    fn on_the_summit_is_where_the_truck_stands_and_not_where_its_middle_is() {
        // Arena Rumble: the platform's top is 42.1 m up and the zone's box starts 6 m
        // under it.
        let summit = Zone {
            center: bevy::math::Vec2::ZERO,
            yaw: 0.0,
            half_extents: bevy::math::Vec2::splat(10.0),
            bottom: 36.3,
            top: 49.0,
        };
        let on_top = Vec3::new(9.0, 43.3, 0.0);
        assert!(on_summit(&summit, on_top, 42.1));
        assert!(
            on_summit(&summit, Vec3::new(9.0, 46.0, 0.0), 42.1),
            "thrown up"
        );
        assert!(
            !on_summit(&summit, Vec3::new(9.0, 38.0, 0.0), 32.0),
            "in the dip"
        );
        assert!(
            !on_summit(&summit, Vec3::new(5.0, 31.5, 0.0), 30.5),
            "under it"
        );
        assert!(
            !on_summit(&summit, Vec3::new(11.0, 43.3, 0.0), 42.1),
            "off the side"
        );
        assert!(
            !on_summit(&summit, Vec3::new(9.0, 50.0, 0.0), 42.1),
            "over the top"
        );
        // Hypercube Rumble's trench: its upper slope is near enough the top to count.
        assert!(on_summit(&summit, Vec3::new(0.0, 36.5, 0.0), 35.2));
        assert!(!on_summit(&summit, Vec3::new(0.0, 35.5, 0.0), 34.5));
    }

    #[test]
    fn on_the_summit_outside_the_zone_earns_nothing() {
        let mut score = Score::default();
        score.step(true, false, 3.0);
        assert_eq!(score.points, 0.0);
    }

    #[test]
    fn off_the_summit_loses_one_a_second_from_the_start() {
        let mut score = Score::default();
        assert_eq!(
            score.step(false, false, 2.0),
            None,
            "it has not fallen: it never got up"
        );
        assert_eq!(score.points, -2.0);
        assert_eq!(score.falls, 0);
    }

    #[test]
    fn leaving_the_summit_costs_fifty() {
        let mut score = Score::default();
        score.step(true, true, 1.0);
        assert_eq!(score.step(false, false, 1.0), Some(Event::Fell));
        assert_eq!(score.points, 10.0 - 50.0 - 1.0);
        assert_eq!(score.falls, 1);
    }

    #[test]
    fn the_order_is_by_points_and_then_by_fewer_falls() {
        let score = |points, falls| Score {
            points,
            falls,
            ..Default::default()
        };
        let scores = [
            score(30.0, 1),
            score(80.0, 2),
            score(30.0, 0),
            score(-5.0, 0),
        ];
        let all: Vec<&Score> = scores.iter().collect();
        assert_eq!(standings(&all), [1, 2, 0, 3]);
        assert_eq!(
            position(&scores[0], [&scores[1], &scores[2], &scores[3]]),
            3
        );
        assert_eq!(position(&scores[1], [&scores[0]]), 1);
    }
}
