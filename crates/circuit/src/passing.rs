//! Passing: the line a driver takes past what is in front of it, and how it keeps clear of
//! what is beside it. A driver never slows for a truck or fixed scenery in its way; it moves
//! its line aside and goes round.

use content::track::Course;

/// The least a driver looks out ahead for a truck to pass, or for fixed scenery, in metres,
/// along the course and in a straight line: about six truck lengths. Closing faster, it looks
/// further (`LOOK_AHEAD`).
pub(crate) const LOOK_OUT: f32 = 45.0;
/// Daylight a driver wants beside another truck, in metres, on top of what the two of them
/// reach across the course. Every sideways figure below is worked out from the trucks
/// themselves, wheels included (`reach`), so that clear means clear and not touching, and so
/// that a Monster Truck Madness 2 truck of any size is measured as itself.
pub(crate) const ROOM_TO_SPARE: f32 = 0.7;
/// How much of the length at which two trucks are clear of each other another truck's middle
/// has to be up the road before it is in front of the driver rather than beside it: at eight
/// tenths they still overlap a little, and it is nearly past the driver's nose. Nearer than
/// this the two are alongside, and neither gives way to the other: neither waits behind the
/// other, neither shuts the other's door, and neither treats the other as passed. Two that
/// each took the other for the truck in front would both stop and stay stopped, and one that
/// let a truck level with it count as in front would be boxed in by a truck it is beside.
const IN_FRONT: f32 = 0.8;
/// Daylight between a driver and a truck alongside it, in metres, on top of what the two of
/// them need to be clear of each other, within which the driver holds its line rather than
/// turning in for a corner: about as far as a corner's line moves it across. Further apart
/// than this, turning in cannot reach the other truck, and each takes its corner as it would
/// alone. Larger holds more trucks to their lines, and pairs of them run wide together.
const SIDE_BY_SIDE: f32 = 7.0;
/// How far ahead a driver looks out for what is in its way, a truck or fixed scenery, in
/// seconds: until it would be up against it at the speed it is closing on it, and never less
/// than `LOOK_OUT` (`look_out`). Its line moves across at `LANE_RATE`, so it needs about 2 s
/// to move a truck's width and a half. At `LOOK_OUT` alone, drivers on Crazy '98 saw the
/// loader too late to move off it.
pub(crate) const LOOK_AHEAD: f32 = 3.0;

/// How far ahead a driver looks out for something it is closing on at `closing` m/s, in
/// metres: until it would be up against it in `LOOK_AHEAD`, and never less than `LOOK_OUT`.
pub(crate) fn look_out(closing: f32) -> f32 {
    (closing * LOOK_AHEAD).max(LOOK_OUT)
}

/// The line a driver wants: its `home` line when the road in front is clear, and otherwise
/// `PASSING_ROOM` to one side of the nearest truck in the way, so that it pulls out to pass
/// rather than sitting behind, and stays out until it is by. It goes to the side it is
/// already on, and to the other when the road runs out there or another truck holds it.
/// Which side of a truck it is on is where it actually is, `at` metres to the left of the
/// centreline, not where its `lane` is: the two part at the start, where a grid can stand
/// well off the course, and after a knock, and a driver that went by its lane would cross
/// in front of the truck it means to pass.
/// Boxed in, it holds the `lane` it is on, and drives on along it until a side is clear: it
/// does not slow for the truck in front. Whatever line it settles on, it is held off any
/// truck alongside it by `giving_room`, so that it never steers into one.
///
/// `others` holds every other truck near enough to matter. `room` is how far either side of
/// the centreline a line may be, as `passing_room` says.
pub(crate) fn passing_line(home: f32, lane: f32, at: f32, others: &[Neighbour], room: f32) -> f32 {
    giving_room(at, wanted_line(home, lane, at, others, room), others, room)
}

/// Another truck, as the driver's rules see it: where it is in relation to the driver, and how
/// much room the two of them need to be clear of each other. That room is worked out from both
/// trucks, wheels and all (`reach`), so a driver beside a big truck gives it more than a small
/// one, and nothing here assumes every truck is the size of the built-in one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Neighbour {
    /// How far ahead along the course it is, in metres. Behind is negative.
    pub(crate) ahead: f32,
    /// How far to the left of the centreline it is, in metres.
    pub(crate) across: f32,
    /// How far apart across the course the two of them have to be to pass each other, in
    /// metres: what each reaches sideways, wheels included, and `ROOM_TO_SPARE`.
    pub(crate) clear_by: f32,
    /// How far apart along the course their middles have to be for one to be clear of the
    /// other, in metres: what each reaches fore and aft, tires included.
    pub(crate) past_by: f32,
    /// How far up the road it matters, in metres: `LOOK_AHEAD` of closing on it (`look_out`).
    pub(crate) look_out: f32,
}

impl Neighbour {
    /// Whether it is alongside the driver: their middles are nearer than the length at which
    /// they are clear of each other, so the two overlap, and moving across the course towards
    /// it would run into it whatever their lines say.
    fn alongside(&self) -> bool {
        self.ahead.abs() < self.past_by
    }

    /// Whether it is alongside a driver that is `across` metres to the left of the
    /// centreline, and near enough across the course that turning in would run into it: no
    /// more than `SIDE_BY_SIDE` further off than the two need to be clear.
    fn side_by_side(&self, across: f32) -> bool {
        self.alongside() && (self.across - across).abs() < self.clear_by + SIDE_BY_SIDE
    }
}

/// A `wanted` line held off every truck that is alongside: it may not be taken nearer than
/// `clear_by` across the course to one, on the side the driver, `at` metres to the left of the
/// centreline, is already passing it. So a
/// driver whose way back to its own line is beside another truck does not turn in on it: it
/// runs alongside until the truck is clear, and then goes home. Nothing is pushed further out
/// than `room`, because beyond that there is no road.
fn giving_room(at: f32, wanted: f32, others: &[Neighbour], room: f32) -> f32 {
    let held = others
        .iter()
        .filter(|other| other.alongside())
        .fold(wanted, |line, other| {
            // Away from it, on the side the driver is already on.
            if at >= other.across {
                line.max(other.across + other.clear_by)
            } else {
                line.min(other.across - other.clear_by)
            }
        });
    held.clamp(-room, room)
}

/// The line a driver `at` metres to the left of the centreline steers by, from the `lane` it
/// is on: no nearer any truck side by side with it (`Neighbour::side_by_side`) than it is
/// already. Its lane moves across at `LANE_RATE`, and is often still on the far side of such a
/// truck, at the start above all, where a grid can stand well off the course and every truck
/// makes for its line at once; steering for it would take the driver into that truck. So,
/// beside one, it runs parallel to the course until the truck is clear, and then carries on.
pub(crate) fn not_towards(lane: f32, at: f32, others: &[Neighbour]) -> f32 {
    others
        .iter()
        .filter(|other| other.side_by_side(at))
        .fold(lane, |line, other| {
            if other.across > at {
                line.min(at)
            } else {
                line.max(at)
            }
        })
}

/// The line `passing_line` wants before it is held off the trucks alongside.
fn wanted_line(home: f32, lane: f32, at: f32, others: &[Neighbour], room: f32) -> f32 {
    let in_the_way = others
        .iter()
        .filter(|other| {
            // In the way of the line it is on, or of the line it came from: a truck it has
            // pulled out for is exactly `clear_by` away and so clear of the line it is on
            // now, and letting it go at that would take the driver straight back in. One
            // that is not yet `IN_FRONT` behind is still alongside, and cutting back across
            // it is how a pass turns into a crash.
            (-IN_FRONT * other.past_by..=other.look_out).contains(&other.ahead)
                && ((other.across - lane).abs() < other.clear_by
                    || (other.across - home).abs() < other.clear_by)
        })
        .min_by(|a, b| a.ahead.total_cmp(&b.ahead));
    let Some(blocking) = in_the_way else {
        return home;
    };
    let across = blocking.across;
    let beside = |side: f32| across + side * blocking.clear_by;
    // A side to move to must be road, and nothing may be overlapping the driver on it: a
    // truck it is level with is a door it cannot open. One that is `IN_FRONT` up the road,
    // or that far behind, is clear of it and shuts nothing; it is only the next truck to
    // pass, or one that has been passed. The truck being passed sits `PASSING_ROOM` away,
    // which is wider than a truck, so it never shuts its own door either.
    let clear = |side: f32| {
        let line = beside(side);
        line.abs() <= room
            && !others
                .iter()
                .any(|other| other.alongside() && (other.across - line).abs() < other.clear_by)
    };
    let side = if at >= across { 1.0 } else { -1.0 };
    match (clear(side), clear(-side)) {
        (true, _) => beside(side),
        (false, true) => beside(-side),
        (false, false) => lane,
    }
}

/// The room a line has when it is passing, in metres: out to where a truck that reaches
/// `reach_across` metres sideways from its middle has its outer wheels at the edge of the
/// course.
pub(crate) fn passing_room(course: &Course, reach_across: f32) -> f32 {
    (course.width / 2.0 - reach_across).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::LANE_SPACING;
    use crate::fixtures::{built_in, course};

    /// How far apart across the course two built-in trucks have to be to pass each other,
    /// and how far apart along it to be clear of each other, in metres: 4.5 and 5.5.
    fn clear_by() -> f32 {
        2.0 * built_in().x + ROOM_TO_SPARE
    }
    fn past_by() -> f32 {
        2.0 * built_in().y
    }

    /// Another built-in truck, `ahead` metres along the course and `across` metres to the
    /// left of the centreline.
    fn near(ahead: f32, across: f32) -> Neighbour {
        Neighbour {
            ahead,
            across,
            clear_by: clear_by(),
            past_by: past_by(),
            look_out: LOOK_OUT,
        }
    }

    /// What `passing_room` allows on the 14 m course above: a line out to 5.1 m either
    /// side of the middle, which is wide enough to pass a truck on the centreline.
    fn room_to_pass() -> f32 {
        passing_room(&course(), built_in().x)
    }

    #[test]
    fn a_driver_with_the_road_to_itself_keeps_to_its_own_line() {
        // Nothing in front; something behind; something too far ahead to bother with.
        assert_eq!(
            passing_line(LANE_SPACING, 0.0, 0.0, &[], room_to_pass()),
            LANE_SPACING
        );
        let behind = [near(-10.0, 0.0)];
        assert_eq!(
            passing_line(LANE_SPACING, 0.0, 0.0, &behind, room_to_pass()),
            LANE_SPACING
        );
        let up_the_road = [near(LOOK_OUT + 10.0, 0.0)];
        assert_eq!(
            passing_line(LANE_SPACING, 0.0, 0.0, &up_the_road, room_to_pass()),
            LANE_SPACING
        );
        // And something in front, but on a line of its own, which it can be left on.
        let beside_it = [near(12.0, -clear_by() - 0.1)];
        assert_eq!(
            passing_line(LANE_SPACING, 0.0, 0.0, &beside_it, room_to_pass()),
            LANE_SPACING
        );
    }

    #[test]
    fn it_pulls_out_to_pass_what_is_in_front_of_it() {
        // A truck just in front, on the same line: out to one side of it, clear of it.
        let passing = passing_line(0.0, 0.0, 0.0, &[near(12.0, 0.0)], room_to_pass());
        assert_eq!(passing.abs(), clear_by(), "{passing}");
        // Already to the right of it, it passes on the right, and on the left to the left.
        assert_eq!(
            passing_line(0.0, -1.0, -1.0, &[near(12.0, 0.5)], room_to_pass()),
            0.5 - clear_by()
        );
        assert_eq!(
            passing_line(0.0, 1.0, 1.0, &[near(12.0, -0.5)], room_to_pass()),
            -0.5 + clear_by()
        );
        // Out is where it stays: the truck it has pulled out for is `clear_by()` away
        // across the course, and it must not take that for a clear road and come back in.
        let pulled_out = [near(12.0, 0.0)];
        assert_eq!(
            passing_line(0.0, clear_by(), clear_by(), &pulled_out, room_to_pass()),
            clear_by()
        );
        // ...while it is alongside, and until it is a truck's length by, when its own line
        // has it again. On the way it keeps a truck's width from it, rather than turning in.
        let beside_it = [near(-1.0, 0.0)];
        assert_eq!(
            passing_line(0.0, clear_by(), clear_by(), &beside_it, room_to_pass()),
            clear_by()
        );
        let nearly_by = [near(-IN_FRONT * past_by() - 0.5, 0.0)];
        assert_eq!(
            passing_line(0.0, clear_by(), clear_by(), &nearly_by, room_to_pass()),
            clear_by()
        );
        let by = [near(-past_by() - 0.1, 0.0)];
        assert_eq!(
            passing_line(0.0, clear_by(), clear_by(), &by, room_to_pass()),
            0.0
        );
        // A truck a clear truck's length up the road, or that far back, is no longer
        // alongside and shuts no door: it is only the next one to pass, or one passed.
        let up_the_road = [near(12.0, 0.0), near(past_by() + 0.1, clear_by())];
        assert_eq!(
            passing_line(0.0, 0.0, 0.0, &up_the_road, room_to_pass()),
            clear_by()
        );
        let passed = [near(12.0, 0.0), near(-past_by() - 0.1, clear_by())];
        assert_eq!(
            passing_line(0.0, 0.0, 0.0, &passed, room_to_pass()),
            clear_by()
        );
        // One alongside on the side it would pull out to shuts that door: it goes the other
        // way, and holds its line when both are shut.
        let door_shut = [near(12.0, 0.0), near(past_by() - 0.1, clear_by())];
        assert_eq!(
            passing_line(0.0, 0.0, 0.0, &door_shut, room_to_pass()),
            -clear_by()
        );
        let boxed_in = [
            near(12.0, 0.0),
            near(2.0, clear_by()),
            near(-2.0, -clear_by()),
        ];
        assert_eq!(passing_line(0.0, 0.0, 0.0, &boxed_in, room_to_pass()), 0.0);
        // The nearest one in the way is the one to pass: out to the left of the near truck,
        // not of the far one, though both are on the driver's line.
        let two = [near(24.0, 0.0), near(12.0, -1.0)];
        assert_eq!(
            passing_line(0.0, 0.0, 0.0, &two, room_to_pass()),
            -1.0 + clear_by()
        );
    }

    #[test]
    fn it_never_turns_in_on_a_truck_that_is_alongside() {
        // Out on the passing line with the truck it has passed now beside it: it stays out,
        // whichever side of the road it went.
        let beside_it = [near(-2.0, 0.0)];
        assert_eq!(
            passing_line(0.0, clear_by(), clear_by(), &beside_it, room_to_pass()),
            clear_by()
        );
        assert_eq!(
            passing_line(0.0, -clear_by(), -clear_by(), &beside_it, room_to_pass()),
            -clear_by()
        );
        // Drawing clear of it, its own line pulls it back in, but no nearer than a truck's
        // width while the two still overlap: it runs alongside rather than turning in.
        let nearly_by = [near(-IN_FRONT * past_by() - 0.5, 0.0)];
        assert_eq!(
            passing_line(0.0, clear_by(), clear_by(), &nearly_by, room_to_pass()),
            clear_by()
        );
        // Room is given from where the driver is, not from the line it wants: on the middle
        // line with a truck out to its left, it does not drift across into it.
        let out_to_the_left = [near(1.0, clear_by())];
        assert!(passing_line(0.0, 0.0, 0.0, &out_to_the_left, room_to_pass()) <= 0.0);
        // Hemmed in either side, it holds the middle rather than leaning on either.
        let both_sides = [near(0.0, clear_by() + 0.2), near(0.0, -clear_by() - 0.2)];
        let held = passing_line(0.0, 0.0, 0.0, &both_sides, room_to_pass());
        assert!(held.abs() <= 0.2, "{held}");
    }

    #[test]
    fn it_passes_on_the_other_side_when_the_road_runs_out() {
        // Out to the right of a truck on its left would be off the road: pass on the left.
        assert_eq!(
            passing_line(0.0, -3.0, -3.0, &[near(12.0, -1.0)], room_to_pass()),
            -1.0 + clear_by()
        );
        // Boxed in on a road too narrow to pass on: hold the line and wait for one.
        assert_eq!(passing_line(0.0, 0.5, 0.5, &[near(12.0, 0.0)], 1.0), 0.5);
    }

    #[test]
    fn a_driver_looks_out_as_far_as_it_closes_in_three_seconds() {
        // Catching something up at 30 m/s: 90 m ahead, 3 s from it.
        assert_eq!(look_out(30.0), 30.0 * LOOK_AHEAD);
        // Running with a truck, or dropping back from it: still `LOOK_OUT`, so a truck just
        // in front is seen until the two touch.
        assert_eq!(look_out(0.0), LOOK_OUT);
        assert_eq!(look_out(-10.0), LOOK_OUT);
    }

    #[test]
    fn it_swerves_round_a_truck_in_front_rather_than_slowing_for_it() {
        let ahead = near(20.0, 0.0);
        // Square behind it, the line it wants is out to one side, clear of it: it goes round,
        // and nothing here slows it.
        let wanted = passing_line(0.0, 0.0, 0.0, &[ahead], room_to_pass());
        assert!(wanted.abs() >= clear_by(), "{wanted}");
    }
}
