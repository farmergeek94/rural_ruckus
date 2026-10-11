//! The way up the summit, as a field over the arena worked out once a race: for every
//! place a truck can stand, which way to go next to reach the scoring zone by the
//! quickest drivable way, round every wall and rim, up any slope a truck climbs, and over
//! any gap it can jump.
//!
//! The arena is sampled on a square grid of nodes `NavParams::spacing` apart. A node is
//! the ground there, or the top of a ground box there, or both where a box is a roof with
//! room to drive under (`headroom`). Two neighbouring nodes join when a truck drives from
//! one to the other: over the ground, where it is no steeper than a truck climbs
//! (`climb`); onto or off a box, where its edge is no taller than a truck drives up
//! (`step`). A cliff, the side of a box and the rim of a pit join nothing, which is what
//! keeps a path off them. Where the ground rises to a crest and a higher surface stands
//! beyond a gap, a truck going fast enough flies from the crest onto it: such a leap joins
//! the two one way, with the speed it needs worked out from the crest's slope and the gap
//! (`leap_speed`). Arena Rumble's platform is reached only so, off the hump on each side
//! of it. A leap is taken straight: it begins `RUN_UP_NODES` back from the crest, in
//! line with it, and the crest is cut off from the hollow beyond it, so that no path
//! comes at a crest from the side, or drives over it into the hollow, or doubles back
//! over it. A node beside a drop costs more to pass through, out to `clearance`, so that a
//! path keeps a truck's width from the edge where it can, and still reaches a truck that
//! has been knocked right up to it: the node right beside a drop or a wall costs much
//! more anywhere (`EXPOSED_COST`), as does the whole of `clearance` on top of the
//! summit, where going over is a fall; off it, the rest of `clearance` costs little
//! (`EXPOSED_OFF_TOP`), since any way up is as good as another and the nearest is
//! wanted. Driving across a slope, rather than up or down it, costs more the steeper it
//! is (`TRAVERSE`), which keeps a path off the flank of a hump and out of the hollow
//! between two. Then one search from the scoring zone
//! outwards gives every node its cost and its next step (Dijkstra, a few milliseconds for
//! an arena).
//!
//! The summit is the summit zone's box: a node under it at the zone's height is on top,
//! and a node under it lower down, in a pit, is not. `run_off` looks along a heading for
//! where the top ends, which is what a driver's speed is held to up there. How far every
//! node on top is from a drop is counted too (`clear_of_drops`): the goal of the search
//! is the part of the scoring zone that is `SAFE_STEPS` in from every drop, so that a
//! path ends well inside the top and not on its rim; and the safest place on top, the
//! furthest from any drop, is where a driver waits (`hold_point`): on Hypercube Rumble
//! the middle of the zone is the corner where its two pits meet, and a driver that waited
//! there fell in.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use bevy::math::Vec2;
use content::track::{GroundBox, HeightGrid, Zone};

/// How the field is sampled and joined.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavParams {
    /// How far apart the nodes are, in metres. A truck is about 2.5 m wide; the ramps of
    /// the base game's arenas are 19.5 m wide and their pits 19.5 m square.
    pub spacing: f32,
    /// The steepest ground a truck is sent up, in metres up for each metre along. Measured
    /// headless with the built-in truck at full throttle: it climbs a 1.5 grade from a
    /// standing start 6 m from the foot, and not a 2.0. The base game's ramps rise 0.31
    /// to 0.9, and the bank onto Pyramid Rumble's summit 1.3.
    pub climb: f32,
    /// The tallest edge of a ground box a truck is sent up, in metres. Measured as
    /// `climb` was: head on from a crawl, 1.5 m yes and 2 m no; but at speed a 1 m edge
    /// stops it dead and a 0.5 m kerb is rolled over, and at an angle it climbs 1 m at
    /// most (`docs/formats/situation.md`, "Summit Rumbles"), and it arrives at an angle
    /// and at speed.
    pub step: f32,
    /// The least room under a box for a truck to drive under it, in metres.
    pub headroom: f32,
    /// How far from a drop a path is kept where it can be, in metres.
    pub clearance: f32,
    /// How far under a zone's bottom a surface may be and still be inside it, in metres: a
    /// truck's middle rides above the surface it stands on. The scoring's `UNDER_SUMMIT`.
    pub under_zone: f32,
    /// The most a truck is asked to do for a leap, in m/s. A leap that needs more is no
    /// way.
    pub fastest: f32,
}

impl Default for NavParams {
    fn default() -> Self {
        Self {
            spacing: 3.25,
            climb: 1.2,
            step: 0.5,
            headroom: 3.0,
            clearance: 4.0,
            under_zone: crate::UNDER_SUMMIT,
            fastest: 24.0,
        }
    }
}

/// What passing through a node near a drop or a wall costs, as a share of its distance,
/// on top of the distance: the node right beside it anywhere, and every node within
/// `clearance` of it on top of the summit, where a path goes round by up to this much
/// rather than along a rim; and the rest within `clearance` off the summit, where a path
/// keeps in from a ramp's shoulder or a wall when the way round is not much longer. A
/// truck sent along the node beside a wall noses into the wall and stalls on it, and one
/// steering at a point of its path up ahead cuts a corner of it by a few metres, into a
/// pillar.
const EXPOSED_COST: f32 = 4.0;
const EXPOSED_OFF_TOP: f32 = 2.0;
/// Driving across a slope, in metres up for each metre along across the way: what it
/// costs to cross one this steep, as a share of the distance on top of the distance,
/// growing with the square of the steepness, up to a limit. A truck sent along the
/// flank of one of Arena Rumble's humps, or through the hollow between two of them,
/// tipped over or stalled with its wheels turned; straight up or down the same slope it
/// goes fine, so a ramp is not penalised.
const TRAVERSE: f32 = 0.5;
const TRAVERSE_COST: f32 = 4.0;
const TRAVERSE_MOST: f32 = 20.0;
/// How far below the top of the summit a drop begins, in metres: the pits of Hypercube
/// Rumble are 4 to 8 m deep, and a truck's own bounce is under a metre.
const DROP: f32 = 1.5;
/// How many nodes in from every drop the goal of the search is, where the scoring zone
/// has that much room: a path ends there, not on the rim it climbed over.
const SAFE_STEPS: u32 = 2;
/// A leap: the least slope of the ground up to a crest for a truck to leave it flying, in
/// metres up for each metre along, taken as the steepest of the last few steps up to
/// the crest, beyond which the ground falls away; how far it may
/// land, in metres, two nodes at the least; how much higher and how much lower than the
/// crest the landing may be, in metres; how far under its flight everything between must
/// be, in metres; the share more than the sum says that it is asked to go, since the
/// crest is rounded and launches it lower than the slope up to it; and what a metre
/// flown costs against a metre driven, so that the shortest leap is taken and none where
/// driving would do. Measured on Arena Rumble (hump 0.65, gap 13 m, rise 3.4 m): the sum
/// says 15.3 m/s, and a built-in truck lands from 12.9 and comfortably from 16; the
/// platform is 39 m across, so a truck that comes in faster than it needs cannot stop
/// before the far rim.
const LAUNCH_GRADE: f32 = 0.3;
const LAUNCH_STEPS: i64 = 3;
const LEAP_REACH: f32 = 15.0;
const LEAP_RISE: f32 = 6.0;
const LEAP_DROP: f32 = 1.0;
const LEAP_GAP: f32 = 1.0;
const LEAP_MARGIN: f32 = 1.1;
const LEAP_COST: f32 = 2.0;
/// How many nodes back from a crest, in line with the leap, a leap begins: a straight
/// run that lines the truck up, and most of the room it needs to get up to speed (about
/// 45 m from rest for Arena Rumble's humps). Fewer where the ground runs out, down to
/// `RUN_UP_LEAST`.
const RUN_UP_NODES: i64 = 8;
const RUN_UP_LEAST: i64 = 2;
const GRAVITY: f32 = 9.81;

const NONE: u32 = u32::MAX;

/// A place of a path: where it is, and what leaving it needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Waypoint {
    pub at: Vec2,
    /// The height of the surface there, in metres.
    pub height: f32,
    /// The speed a truck must have as it leaves here to fly to the next place, in m/s:
    /// a leap. 0 where it drives there.
    pub leap: f32,
}

/// The field: see the module doc.
#[derive(Clone, Debug)]
pub struct NavField {
    origin: Vec2,
    spacing: f32,
    climb: f32,
    step: f32,
    /// Nodes along each side.
    side: usize,
    /// Two slots a cell: the ground, then the box top. `NaN` where there is no surface.
    height: Vec<f32>,
    /// Eight a cell, one for each way round it (`AROUND`): the height of the ground where
    /// the way to a boxed neighbour crosses onto the box, for the step up onto it. `NaN`
    /// where the neighbour is not boxed. A node samples the ground up to half a box cell
    /// short of the box, where a ramp up to it is still a metre low.
    edge: Vec<f32>,
    /// What it costs to reach the goal from each slot: infinite where it cannot be.
    cost: Vec<f32>,
    /// The slot to go to next from each slot: `NONE` at the goal, and where it cannot be
    /// reached.
    next: Vec<u32>,
    /// The speed leaving each slot for its next needs, in m/s: a leap. 0 where it drives.
    /// Where a slot begins a run-up, it is the leap's speed, which is needed at `via`.
    leap: Vec<f32>,
    /// The crest a slot's way to its next goes over, where the slot begins the run-up to a
    /// leap, else `NONE`.
    via: Vec<u32>,
    /// Pairs of slots, lower first, that are not joined although the ground would join
    /// them: a crest and the hollow beyond it.
    cut: HashSet<(usize, usize)>,
    /// Each crest a leap is from: where its run-up begins, where it lands, and the speed
    /// it needs. Where a crest has several, the first found, which is the shortest.
    crests: HashMap<usize, (usize, usize, f32)>,
    /// Which way the ground rises at each slot, and how steeply, in metres up for each
    /// metre along, on the ground plane: for the cost of driving across a slope. Zero on
    /// a box top.
    slope: Vec<Vec2>,
    /// Whether each slot is on top of the summit.
    on_top: Vec<bool>,
    /// How many nodes each slot on top is in from the nearest drop, 0 at the edge. 0 off
    /// the top.
    edge_steps: Vec<u32>,
    /// The place on top, in the scoring zone, furthest from any drop; `None` where there
    /// is no top to stand on.
    hold: Option<Vec2>,
}

/// The eight ways from a cell to the cells round it.
const AROUND: [(i64, i64); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

impl NavField {
    /// The field over a square of `radius` metres each way round `centre`, for the
    /// `scoring` zone as the goal on the `summit`, over the ground `heights` and its
    /// `boxes`.
    pub fn build(
        heights: &HeightGrid,
        boxes: &[GroundBox],
        scoring: &Zone,
        summit: &Zone,
        centre: Vec2,
        radius: f32,
        params: NavParams,
    ) -> Self {
        let spacing = params.spacing.max(0.5);
        let side = ((2.0 * radius / spacing).ceil() as usize).max(1) + 1;
        let origin = centre - Vec2::splat(radius);
        let slots = side * side * 2;
        let mut field = Self {
            origin,
            spacing,
            climb: params.climb,
            step: params.step,
            side,
            height: vec![f32::NAN; slots],
            edge: vec![f32::NAN; side * side * 8],
            cost: vec![f32::INFINITY; slots],
            next: vec![NONE; slots],
            leap: vec![0.0; slots],
            via: vec![NONE; slots],
            cut: HashSet::new(),
            crests: HashMap::new(),
            slope: vec![Vec2::ZERO; slots],
            on_top: vec![false; slots],
            edge_steps: vec![0; slots],
            hold: None,
        };
        // Only the boxes that reach into the square.
        let far = origin + Vec2::splat(2.0 * radius);
        let boxes: Vec<&GroundBox> = boxes
            .iter()
            .filter(|b| {
                b.max.x >= origin.x && b.min.x <= far.x && b.max.y >= origin.y && b.min.y <= far.y
            })
            .collect();
        for cell in 0..side * side {
            let at = field.position(cell);
            let ground = heights.height_at(at.x, at.y);
            let boxed = boxes
                .iter()
                .find(|b| at.x >= b.min.x && at.x < b.max.x && at.y >= b.min.y && at.y < b.max.y);
            match boxed {
                Some(b) => {
                    field.height[cell * 2 + 1] = b.top;
                    if b.bottom - ground >= params.headroom {
                        field.height[cell * 2] = ground;
                    }
                }
                None => field.height[cell * 2] = ground,
            }
            for layer in 0..2 {
                let slot = cell * 2 + layer;
                let height = field.height[slot];
                field.on_top[slot] = !height.is_nan()
                    && summit.covers(at)
                    && height >= summit.bottom - params.under_zone
                    && height <= summit.top;
            }
        }
        for cell in 0..side * side {
            if field.height[cell * 2].is_nan() {
                continue;
            }
            let at = field.position(cell);
            let (column, row) = field.column_row(cell);
            field.slope[cell * 2] = field.ground_slope(column, row);
            for (way, (dx, dz)) in AROUND.into_iter().enumerate() {
                let Some(next) = field.cell_from(column + dx, row + dz) else {
                    continue;
                };
                if field.height[next * 2 + 1].is_nan() {
                    continue;
                }
                let to = field.position(next);
                let Some(b) = boxes.iter().find(|b| {
                    to.x >= b.min.x && to.x < b.max.x && to.y >= b.min.y && to.y < b.max.y
                }) else {
                    continue;
                };
                let crossing = at.lerp(to, entry(at, to, b.min, b.max));
                field.edge[cell * 8 + way] = heights.height_at(crossing.x, crossing.y);
            }
        }
        let (leaps, cuts) = field.leaps(params.fastest);
        field.cut = cuts;
        for (&landing, from) in &leaps {
            for &(start, crest, needs) in from {
                field.crests.entry(crest).or_insert((start, landing, needs));
            }
        }

        // What it costs to step onto each slot: more beside a drop, out to `clearance`.
        let rings = (params.clearance / spacing).ceil() as usize;
        let exposed = field.exposed();
        let mut near_drop = exposed.clone();
        for _ in 0..rings {
            let before = near_drop.clone();
            for (slot, _) in before.iter().enumerate().filter(|(_, near)| **near) {
                for (other, _) in field.neighbours(slot) {
                    near_drop[other] = true;
                }
            }
        }

        // Dijkstra from every goal slot at once: the scoring zone's top, in from its
        // drops where it has that much room, and where it has not, all of it.
        field.edge_steps = field.steps_from_drops(&exposed);
        let goal = |field: &Self, slot: usize, in_from: u32| {
            field.on_top[slot]
                && field.edge_steps[slot] >= in_from
                && scoring.covers(field.position(slot / 2))
        };
        let in_from = if (0..slots).any(|slot| goal(&field, slot, SAFE_STEPS)) {
            SAFE_STEPS
        } else {
            0
        };
        let mut heap = BinaryHeap::new();
        for slot in 0..slots {
            if goal(&field, slot, in_from) {
                field.cost[slot] = 0.0;
                heap.push(Reached { cost: 0.0, slot });
            }
        }
        field.hold = field.safest(scoring);
        while let Some(Reached { cost, slot }) = heap.pop() {
            if cost > field.cost[slot] {
                continue;
            }
            // Driven to from the slots round it, and flown to from a crest.
            let driven = field.neighbours(slot).map(|(other, distance)| {
                let share = if exposed[other] || (near_drop[other] && field.on_top[other]) {
                    EXPOSED_COST
                } else if near_drop[other] {
                    EXPOSED_OFF_TOP
                } else {
                    0.0
                };
                // Driving across a slope rather than up or down it: how much of the
                // slope at either end is square to the step.
                let way = (field.position(slot / 2) - field.position(other / 2)) / distance;
                let across = field.slope[other]
                    .perp_dot(way)
                    .abs()
                    .max(field.slope[slot].perp_dot(way).abs());
                let traverse = (TRAVERSE_COST * (across / TRAVERSE).powi(2)).min(TRAVERSE_MOST);
                (other, distance * (1.0 + share + traverse), 0.0)
            });
            let flown = leaps
                .get(&slot)
                .into_iter()
                .flatten()
                .map(|&(from, crest, needs)| {
                    let run = field.position(from / 2).distance(field.position(crest / 2))
                        + field.position(crest / 2).distance(field.position(slot / 2));
                    (from, run * LEAP_COST, needs, crest)
                });
            let joined: Vec<(usize, f32, f32, usize)> = driven
                .map(|(other, step, needs)| (other, step, needs, NONE as usize))
                .chain(flown)
                .collect();
            for (other, step, needs, via) in joined {
                let through = cost + step;
                if through < field.cost[other] {
                    field.cost[other] = through;
                    field.next[other] = slot as u32;
                    field.leap[other] = needs;
                    field.via[other] = via as u32;
                    heap.push(Reached {
                        cost: through,
                        slot: other,
                    });
                }
            }
        }
        field
    }

    /// Where cell `cell` is on the ground plane.
    fn position(&self, cell: usize) -> Vec2 {
        let (column, row) = (cell % self.side, cell / self.side);
        self.origin + Vec2::new(column as f32, row as f32) * self.spacing
    }

    /// The cell that `position` is in, if it is on the field.
    fn cell_at(&self, position: Vec2) -> Option<usize> {
        let at = ((position - self.origin) / self.spacing).round();
        self.cell_from(at.x as i64, at.y as i64)
    }

    /// The cell at `column`, `row`, if that is on the field.
    fn cell_from(&self, column: i64, row: i64) -> Option<usize> {
        let side = self.side as i64;
        ((0..side).contains(&column) && (0..side).contains(&row))
            .then(|| (row * side + column) as usize)
    }

    /// The column and row of `cell`.
    fn column_row(&self, cell: usize) -> (i64, i64) {
        ((cell % self.side) as i64, (cell / self.side) as i64)
    }

    /// The slot a truck at `position`, with its middle `height` up, stands on: the surface
    /// of its cell nearest that height.
    pub fn locate(&self, position: Vec2, height: f32) -> Option<usize> {
        let cell = self.cell_at(position)?;
        (0..2)
            .map(|layer| cell * 2 + layer)
            .filter(|&slot| !self.height[slot].is_nan())
            .min_by(|&a, &b| {
                (self.height[a] - height)
                    .abs()
                    .total_cmp(&(self.height[b] - height).abs())
            })
    }

    /// Which way the ground rises at the cell at `column`, `row`, and how steeply, in
    /// metres up for each metre along, from the ground of the cells either side of it:
    /// one side where the other has no ground.
    fn ground_slope(&self, column: i64, row: i64) -> Vec2 {
        let here = self.height[(row * self.side as i64 + column) as usize * 2];
        let ground = |dx: i64, dz: i64| {
            self.cell_from(column + dx, row + dz)
                .map(|cell| self.height[cell * 2])
                .filter(|height| !height.is_nan())
        };
        let slope = |low: Option<f32>, high: Option<f32>| match (low, high) {
            (Some(low), Some(high)) => (high - low) / (2.0 * self.spacing),
            (Some(low), None) => (here - low) / self.spacing,
            (None, Some(high)) => (high - here) / self.spacing,
            (None, None) => 0.0,
        };
        Vec2::new(
            slope(ground(-1, 0), ground(1, 0)),
            slope(ground(0, -1), ground(0, 1)),
        )
    }

    /// Whether a truck drives between slots `a` and `b`, `distance` apart: over the
    /// ground, where it is no steeper than it climbs; on or off a box, where the edge is
    /// no taller than it steps up, measured where the ground meets the box when `b` is
    /// the `way` round `a` (an index into `AROUND`). `NaN` heights join nothing.
    fn joined(&self, a: usize, b: usize, way: Option<usize>, distance: f32) -> bool {
        if self.cut.contains(&(a.min(b), a.max(b))) {
            return false;
        }
        match (a % 2, b % 2) {
            (0, 0) => (self.height[a] - self.height[b]).abs() <= self.climb * distance,
            (0, 1) => self.stepped(a, b, way),
            (1, 0) => self.stepped(b, a, way.map(|way| 7 - way)),
            _ => (self.height[a] - self.height[b]).abs() <= self.step,
        }
    }

    /// Whether the box top `top` is no more than a step up or down from the `ground`,
    /// where the ground meets it if `way` says which way that is.
    fn stepped(&self, ground: usize, top: usize, way: Option<usize>) -> bool {
        let meets = way
            .map(|way| self.edge[(ground / 2) * 8 + way])
            .filter(|height| !height.is_nan())
            .unwrap_or(self.height[ground]);
        (self.height[top] - meets).abs() <= self.step
    }

    /// The slots a truck can drive to from `slot`, with how far each is: the eight round
    /// it, on either surface, that it is `joined` to.
    fn neighbours(&self, slot: usize) -> impl Iterator<Item = (usize, f32)> + '_ {
        let (column, row) = self.column_row(slot / 2);
        AROUND
            .into_iter()
            .enumerate()
            .filter_map(move |(way, (dx, dz))| {
                let other_cell = self.cell_from(column + dx, row + dz)?;
                let distance = self.spacing
                    * if dx != 0 && dz != 0 {
                        std::f32::consts::SQRT_2
                    } else {
                        1.0
                    };
                Some((way, other_cell, distance))
            })
            .flat_map(move |(way, other_cell, distance)| {
                (0..2).filter_map(move |layer| {
                    let other = other_cell * 2 + layer;
                    self.joined(slot, other, Some(way), distance)
                        .then_some((other, distance))
                })
            })
    }

    /// Every leap a truck can make, by the slot it lands on, as the slot its run-up begins
    /// at, the crest, and the speed it needs; and the pairs of slots to cut apart. A
    /// leap is from a crest of the ground, where the ground up to it rises at
    /// `LAUNCH_GRADE` or more and the ground beyond it falls away, straight on to a
    /// surface up to `LEAP_REACH` on that is not driven to, over nothing that reaches up
    /// to the flight, at a speed within `fastest`. Its run-up is the longest straight
    /// run of ground up to `RUN_UP_NODES` back from the crest, in line with it, and at
    /// least `RUN_UP_LEAST`. The crest is cut from the three cells beyond it.
    #[expect(
        clippy::type_complexity,
        reason = "the leaps, and the cuts, of one search"
    )]
    fn leaps(
        &self,
        fastest: f32,
    ) -> (
        HashMap<usize, Vec<(usize, usize, f32)>>,
        HashSet<(usize, usize)>,
    ) {
        let mut leaps: HashMap<usize, Vec<(usize, usize, f32)>> = HashMap::new();
        let mut cuts = HashSet::new();
        for cell in 0..self.side * self.side {
            let crest = cell * 2;
            let here = self.height[crest];
            if here.is_nan() {
                continue;
            }
            let (column, row) = self.column_row(cell);
            // Straight ways only: a box's sides run along the grid, and a slope's flank
            // taken cornerwise can look like a crest and lead into the box's corner.
            for (way, (dx, dz)) in AROUND
                .into_iter()
                .enumerate()
                .filter(|(_, (dx, dz))| dx * dz == 0)
            {
                let along = |k: i64| self.cell_from(column + dx * k, row + dz * k);
                let distance = self.spacing;
                let Some(ahead) = along(1) else {
                    continue;
                };
                // A crest, with the ground falling away beyond it: a wall there is not
                // one, but something to drive into.
                let beyond = self.height[ahead * 2];
                if beyond.is_nan() || beyond >= here {
                    continue;
                }
                // Not yet going down: the node before it is no higher.
                let before = along(-1).map_or(f32::NAN, |behind| self.height[behind * 2]);
                if before.is_nan() || before > here {
                    continue;
                }
                // The slope it is launched off: the steepest step of the way up to it.
                // The crest itself may fall between two nodes, the last of which is a
                // little past it and nearly level with the one before.
                let grade = (1..=LAUNCH_STEPS)
                    .filter_map(|k| {
                        let (near, far) = (along(-k + 1)?, along(-k)?);
                        Some((self.height[near * 2] - self.height[far * 2]) / distance)
                    })
                    .filter(|grade| !grade.is_nan())
                    .fold(f32::MIN, f32::max);
                if grade < LAUNCH_GRADE {
                    continue;
                }
                // The run-up: back from the crest while the ground joins up.
                let mut start = crest;
                let mut nodes = 0;
                for k in 1..=RUN_UP_NODES {
                    let Some(back) = along(-k) else {
                        break;
                    };
                    if !self.joined(start, back * 2, Some(7 - way), distance) {
                        break;
                    }
                    start = back * 2;
                    nodes = k;
                }
                if nodes < RUN_UP_LEAST {
                    continue;
                }
                let mut found = false;
                for k in 2.. {
                    let gap = distance * k as f32;
                    let Some(landing) = along(k).filter(|_| gap <= LEAP_REACH) else {
                        break;
                    };
                    for layer in 0..2 {
                        let to = landing * 2 + layer;
                        let rise = self.height[to] - here;
                        if !(-LEAP_DROP..=LEAP_RISE).contains(&rise)
                            || self.joined(crest, to, None, gap)
                        {
                            continue;
                        }
                        let Some(needs) = leap_speed(grade, gap, rise) else {
                            continue;
                        };
                        if needs > fastest {
                            continue;
                        }
                        // Nothing between may reach up to the flight, which starts off
                        // along the slope and so is above the straight line to the landing.
                        let clear = (1..k).all(|j| {
                            let between = along(j).expect("between two cells on the field");
                            let flight =
                                here + flight_height(grade, needs, distance * j as f32) - LEAP_GAP;
                            (0..2).all(|l| {
                                let surface = self.height[between * 2 + l];
                                surface.is_nan() || surface < flight
                            })
                        });
                        if clear {
                            leaps.entry(to).or_default().push((start, crest, needs));
                            found = true;
                        }
                    }
                }
                if found {
                    // No path comes over the crest from the hollow, or goes down into
                    // it: that is the leap's way, flown.
                    let (px, pz) = (dz, dx);
                    for side in -1..=1 {
                        if let Some(hollow) =
                            self.cell_from(column + dx + px * side, row + dz + pz * side)
                        {
                            cuts.insert((crest.min(hollow * 2), crest.max(hollow * 2)));
                        }
                    }
                }
            }
        }
        (leaps, cuts)
    }

    /// How many nodes each slot on top is in from the nearest drop, counted across the top
    /// from the slots at its edge: those that stand beside a drop (`exposed`) or beside a
    /// place that is not on top. 0 there, and off the top.
    fn steps_from_drops(&self, exposed: &[bool]) -> Vec<u32> {
        let slots = self.side * self.side * 2;
        let mut steps = vec![u32::MAX; slots];
        let mut queue = std::collections::VecDeque::new();
        for slot in 0..slots {
            if !self.on_top[slot] {
                steps[slot] = 0;
                continue;
            }
            let at_the_edge =
                exposed[slot] || self.neighbours(slot).any(|(other, _)| !self.on_top[other]);
            if at_the_edge {
                steps[slot] = 0;
                queue.push_back(slot);
            }
        }
        while let Some(slot) = queue.pop_front() {
            let joined: Vec<usize> = self.neighbours(slot).map(|(other, _)| other).collect();
            for other in joined {
                if self.on_top[other] && steps[other] == u32::MAX {
                    steps[other] = steps[slot] + 1;
                    queue.push_back(other);
                }
            }
        }
        // A top with no edge at all, which no arena has: as far in as can be.
        for step in &mut steps {
            if *step == u32::MAX {
                *step = 0;
            }
        }
        steps
    }

    /// The place on top, in the `scoring` zone, that is furthest from any drop, by
    /// `edge_steps`. Nearer the zone's middle between equals.
    fn safest(&self, scoring: &Zone) -> Option<Vec2> {
        let slots = self.side * self.side * 2;
        (0..slots)
            .filter(|&slot| self.on_top[slot] && scoring.covers(self.position(slot / 2)))
            .max_by(|&a, &b| {
                let nearer =
                    |slot: usize| -self.position(slot / 2).distance_squared(scoring.center);
                (self.edge_steps[a], nearer(a))
                    .partial_cmp(&(self.edge_steps[b], nearer(b)))
                    .unwrap_or(Ordering::Equal)
            })
            .map(|slot| self.position(slot / 2))
    }

    /// How far in from the nearest drop a truck on top of the summit at `position`,
    /// `height` up, stands, in metres: 0 at the rim of the top or a pit, and off the top.
    pub fn clear_of_drops(&self, position: Vec2, height: f32) -> f32 {
        self.locate(position, height)
            .map_or(0.0, |slot| self.edge_steps[slot] as f32 * self.spacing)
    }

    /// The place on top of the summit, in the scoring zone, furthest from any drop: where a
    /// driver with no one to push waits. `None` where nothing is on top.
    pub fn hold_point(&self) -> Option<Vec2> {
        self.hold
    }

    /// Which slots stand beside a drop: a surface with a neighbouring cell that it cannot
    /// drive onto, or none, in some direction.
    fn exposed(&self) -> Vec<bool> {
        let slots = self.side * self.side * 2;
        let mut exposed = vec![false; slots];
        for (slot, exposed) in exposed.iter_mut().enumerate() {
            if self.height[slot].is_nan() {
                continue;
            }
            let (column, row) = self.column_row(slot / 2);
            let joined: Vec<usize> = self.neighbours(slot).map(|(other, _)| other / 2).collect();
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let Some(cell) = self.cell_from(column + dx, row + dz) else {
                    continue;
                };
                if !joined.contains(&cell) {
                    *exposed = true;
                    break;
                }
            }
        }
        exposed
    }

    /// The way from where a truck stands to the scoring zone: the places to drive through,
    /// in order, up to `most` of them, the first at the truck's own node. Empty where there
    /// is no way, from off the field or from somewhere the goal cannot be reached from. A
    /// truck right beside a drop, where no node of its own has a way, starts from the
    /// nearest node that has one. A leap on the way is taken from the crest `lane` nodes
    /// to the side of the one the search found, where that crest has a leap too, so that
    /// trucks with different lanes do not queue for the same run-up.
    pub fn path_from(&self, position: Vec2, height: f32, most: usize, lane: i32) -> Vec<Waypoint> {
        let mut path = Vec::new();
        let Some(mut slot) = self.locate(position, height) else {
            return path;
        };
        if self.cost[slot].is_infinite() {
            let Some(found) = self.nearest_reachable(slot) else {
                return path;
            };
            slot = found;
        }
        while path.len() < most {
            let via = self.via[slot];
            if via != NONE {
                // A run-up, its crest, and the landing the leap is to: the lane's own
                // where it has one.
                let (start, crest, landing, needs) = self
                    .lane_crest(via as usize, self.next[slot] as usize, lane)
                    .unwrap_or((
                        slot,
                        via as usize,
                        self.next[slot] as usize,
                        self.leap[slot],
                    ));
                path.push(Waypoint {
                    at: self.position(start / 2),
                    height: self.height[start],
                    leap: 0.0,
                });
                path.push(Waypoint {
                    at: self.position(crest / 2),
                    height: self.height[crest],
                    leap: needs,
                });
                slot = landing;
                continue;
            }
            path.push(Waypoint {
                at: self.position(slot / 2),
                height: self.height[slot],
                leap: self.leap[slot],
            });
            let next = self.next[slot];
            if next == NONE {
                break;
            }
            slot = next as usize;
        }
        path
    }

    /// The leap from the crest `lane` nodes across from `crest`, whose leap is to
    /// `landing`, as its run-up's start, the crest, its landing and the speed it needs:
    /// `None` where there is none, or the goal cannot be reached from its landing.
    fn lane_crest(
        &self,
        crest: usize,
        landing: usize,
        lane: i32,
    ) -> Option<(usize, usize, usize, f32)> {
        if lane == 0 {
            return None;
        }
        let (cc, cr) = self.column_row(crest / 2);
        let (lc, lr) = self.column_row(landing / 2);
        let (dx, dz) = ((lc - cc).signum(), (lr - cr).signum());
        let cell = self.cell_from(cc + dz * i64::from(lane), cr + dx * i64::from(lane))?;
        let &(start, landing, needs) = self.crests.get(&(cell * 2))?;
        self.cost[landing]
            .is_finite()
            .then_some((start, cell * 2, landing, needs))
    }

    /// The nearest slot to `slot`, up to two cells away, that the goal can be reached from.
    fn nearest_reachable(&self, slot: usize) -> Option<usize> {
        let (column, row) = self.column_row(slot / 2);
        let height = self.height[slot];
        let mut best: Option<(f32, usize)> = None;
        for dz in -2..=2i64 {
            for dx in -2..=2i64 {
                let Some(cell) = self.cell_from(column + dx, row + dz) else {
                    continue;
                };
                for layer in 0..2 {
                    let other = cell * 2 + layer;
                    if self.cost[other].is_infinite() || (self.height[other] - height).abs() > DROP
                    {
                        continue;
                    }
                    let away = ((dx * dx + dz * dz) as f32).sqrt();
                    if best.is_none_or(|(nearest, _)| away < nearest) {
                        best = Some((away, other));
                    }
                }
            }
        }
        best.map(|(_, slot)| slot)
    }

    /// How far a truck standing at `position`, `height` up, has to go to reach the scoring
    /// zone, in metres along its way. `None` where it cannot.
    pub fn to_goal(&self, position: Vec2, height: f32) -> Option<f32> {
        let slot = self.locate(position, height)?;
        self.cost[slot].is_finite().then_some(self.cost[slot])
    }

    /// Whether a truck at `position`, `height` up, stands on top of the summit.
    pub fn on_top(&self, position: Vec2, height: f32) -> bool {
        self.locate(position, height)
            .is_some_and(|slot| self.on_top[slot])
    }

    /// How far a truck on top of the summit at `position`, `height` up, can go along
    /// `direction` (a unit vector) before it leaves the top: to a place that is not on top,
    /// the edge of the summit zone or a pit, or that drops more than `DROP` below the one
    /// before. `most` when it is further than that, and 0 where it is not on top to begin
    /// with. Leaving the top down a ramp costs what a fall does, so a ramp ends it too.
    pub fn run_off(&self, position: Vec2, height: f32, direction: Vec2, most: f32) -> f32 {
        let Some(mut slot) = self.locate(position, height) else {
            return 0.0;
        };
        if !self.on_top[slot] {
            return 0.0;
        }
        let step = self.spacing / 2.0;
        let mut gone = 0.0;
        while gone < most {
            gone += step;
            let at = position + direction * gone;
            let standing = self.height[slot];
            match self.locate(at, standing) {
                Some(there) if self.on_top[there] && self.height[there] >= standing - DROP => {
                    slot = there;
                }
                _ => return gone.min(most),
            }
        }
        most
    }
}

/// How far along the way from `from` to `to`, from 0 to 1, it enters the box with the
/// corners `min` and `max` on the ground plane: 0 when it starts inside.
fn entry(from: Vec2, to: Vec2, min: Vec2, max: Vec2) -> f32 {
    let along = |from: f32, to: f32, min: f32, max: f32| {
        if to > from && from < min {
            (min - from) / (to - from)
        } else if to < from && from > max {
            (max - from) / (to - from)
        } else {
            0.0
        }
    };
    along(from.x, to.x, min.x, max.x)
        .max(along(from.y, to.y, min.y, max.y))
        .clamp(0.0, 1.0)
}

/// The speed a truck needs leaving a crest whose approach rises `grade` (metres up for
/// each metre along) to land `gap` metres on and `rise` metres up, in m/s, with
/// `LEAP_MARGIN` on it: it flies off at the slope's angle. `None` where no speed will
/// do, the landing being above the line of launch.
fn leap_speed(grade: f32, gap: f32, rise: f32) -> Option<f32> {
    let cos = grade.atan().cos();
    let under = gap * grade - rise;
    if under <= 0.0 {
        return None;
    }
    Some((GRAVITY * gap * gap / (2.0 * cos * cos * under)).sqrt() * LEAP_MARGIN)
}

/// How high above the crest a truck that left it at `speed` m/s along a slope of `grade`
/// is, `along` metres on, in metres.
fn flight_height(grade: f32, speed: f32, along: f32) -> f32 {
    let cos = grade.atan().cos();
    along * grade - GRAVITY * along * along / (2.0 * speed * speed * cos * cos)
}

/// A slot reached by the search, ordered so that the heap gives the cheapest first.
#[derive(PartialEq)]
struct Reached {
    cost: f32,
    slot: usize,
}

impl Eq for Reached {}

impl PartialOrd for Reached {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Reached {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.total_cmp(&self.cost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plateau 10 m high and 20 m square at the origin, with one ramp down from its east
    /// side, 20 m wide, that reaches the plain at x = 40: a rise of 1 in 3.
    fn plateau_height(x: f32, z: f32) -> f32 {
        if x.abs() <= 10.0 && z.abs() <= 10.0 {
            10.0
        } else if (10.0..=40.0).contains(&x) && z.abs() <= 10.0 {
            10.0 * (40.0 - x) / 30.0
        } else {
            0.0
        }
    }

    fn zone(half: f32) -> Zone {
        Zone {
            center: Vec2::ZERO,
            yaw: 0.0,
            half_extents: Vec2::splat(half),
            bottom: 10.0,
            top: 20.0,
        }
    }

    /// The plateau as ground alone.
    fn terrain_field() -> NavField {
        let heights = HeightGrid::from_fn(161, 160.0, plateau_height);
        NavField::build(
            &heights,
            &[],
            &zone(5.0),
            &zone(10.0),
            Vec2::ZERO,
            70.0,
            NavParams::default(),
        )
    }

    /// The plateau as a ground box over flat ground, with the same ramp of ground, which
    /// meets the box flush at its east edge.
    fn boxed_field() -> NavField {
        let heights = HeightGrid::from_fn(161, 160.0, |x, z| {
            if x >= 10.0 { plateau_height(x, z) } else { 0.0 }
        });
        let boxes = [GroundBox {
            min: Vec2::splat(-10.0),
            max: Vec2::splat(10.0),
            bottom: 0.0,
            top: 10.0,
            faces: None,
        }];
        NavField::build(
            &heights,
            &boxes,
            &zone(5.0),
            &zone(10.0),
            Vec2::ZERO,
            70.0,
            NavParams::default(),
        )
    }

    /// No two places of a path are further apart than a step across a cell, and the path
    /// ends in the scoring zone.
    fn check_path(field: &NavField, from: Vec2, height: f32) -> Vec<Vec2> {
        let path: Vec<Vec2> = field
            .path_from(from, height, 200, 0)
            .iter()
            .map(|place| place.at)
            .collect();
        assert!(path.len() > 1, "no way from {from}");
        for pair in path.windows(2) {
            assert!(
                pair[0].distance(pair[1]) < 3.25 * 1.5,
                "a jump in the path at {}",
                pair[0]
            );
        }
        let end = *path.last().unwrap();
        assert!(
            end.length() <= 5.0 * 1.5,
            "the path ends at {end}, not in the zone"
        );
        path
    }

    #[test]
    fn the_way_up_is_the_ramp_whichever_side_the_truck_starts() {
        for field in [terrain_field(), boxed_field()] {
            // From the plain north of the plateau, the path goes round to the ramp: every
            // place of it that is within the plateau's square is reached from the east.
            let path = check_path(&field, Vec2::new(0.0, -50.0), 0.0);
            let first_on_top = path
                .iter()
                .position(|p| p.x.abs() < 10.0 && p.y.abs() < 10.0)
                .expect("gets on top");
            let before = path[first_on_top - 1];
            assert!(before.x >= 9.0, "climbed on at {before}, not up the ramp");
            // From the foot of the ramp, straight up it.
            let path = check_path(&field, Vec2::new(50.0, 0.0), 0.0);
            assert!(
                path.iter().all(|p| p.y.abs() < 8.0),
                "wandered off the ramp"
            );
        }
    }

    #[test]
    fn the_top_ends_at_the_rim_and_not_at_the_ramp() {
        for field in [terrain_field(), boxed_field()] {
            let top = Vec2::ZERO;
            assert!(field.on_top(top, 11.0));
            assert!(
                !field.on_top(Vec2::new(20.0, 0.0), 7.0),
                "the ramp is not the top"
            );
            let west = field.run_off(top, 11.0, Vec2::NEG_X, 40.0);
            assert!((8.0..=12.0).contains(&west), "{west} m to the western rim");
            // The ramp is no drop, but it is off the summit, which costs as much.
            let east = field.run_off(top, 11.0, Vec2::X, 40.0);
            assert!((8.0..=12.0).contains(&east), "{east} m to the ramp");
            assert_eq!(field.run_off(Vec2::new(50.0, 0.0), 0.0, Vec2::X, 40.0), 0.0);
        }
    }

    #[test]
    fn a_path_keeps_off_the_rim_where_it_can() {
        let field = terrain_field();
        // Up the ramp from its south edge: the path moves in from the edge.
        let path = field.path_from(Vec2::new(35.0, 9.0), 1.5, 50, 0);
        // The first place is the truck's own.
        let mut on_the_ramp = path
            .iter()
            .map(|place| place.at)
            .skip(1)
            .filter(|p| (12.0..=38.0).contains(&p.x));
        assert!(on_the_ramp.clone().count() > 2);
        assert!(on_the_ramp.all(|p| p.y.abs() < 7.0), "{path:?}");
    }

    #[test]
    fn the_safest_place_is_the_middle_of_a_plain_top_and_away_from_a_pit() {
        let field = terrain_field();
        let hold = field.hold_point().expect("a top");
        assert!(hold.length() < 3.0, "{hold}");
        assert!(field.clear_of_drops(hold, 11.0) >= 6.0);
        assert_eq!(
            field.clear_of_drops(Vec2::new(9.0, 0.0), 11.0),
            0.0,
            "at the rim"
        );
        assert_eq!(
            field.clear_of_drops(Vec2::new(30.0, 0.0), 4.0),
            0.0,
            "off the top"
        );
        // A wider plateau, 32 m square, with a pit 6 m square at its middle, 10 m deep
        // (a 5 m one is climbed out of across a corner): the safest place is between the
        // pit and the rim, towards a corner, where both are furthest.
        let heights = HeightGrid::from_fn(161, 160.0, |x, z| {
            if x.abs() <= 3.0 && z.abs() <= 3.0 {
                0.0
            } else if x.abs() <= 16.0 && z.abs() <= 16.0 {
                10.0
            } else {
                0.0
            }
        });
        let pitted = NavField::build(
            &heights,
            &[],
            &zone(16.0),
            &zone(16.0),
            Vec2::ZERO,
            70.0,
            NavParams::default(),
        );
        let hold = pitted.hold_point().unwrap();
        assert!(hold.x.abs() > 6.0 && hold.y.abs() > 6.0, "{hold}");
        assert!(hold.x.abs() < 13.0 && hold.y.abs() < 13.0, "{hold}");
        assert!(pitted.on_top(hold, 11.0));
        assert!(!pitted.on_top(Vec2::ZERO, 6.0), "the pit is not the top");
    }

    #[test]
    fn what_cannot_be_reached_has_no_way() {
        let field = terrain_field();
        assert!(
            field
                .path_from(Vec2::new(500.0, 0.0), 0.0, 10, 0)
                .is_empty()
        );
        assert!(
            field
                .to_goal(Vec2::ZERO, 11.0)
                .is_some_and(|cost| cost < 3.25 * 2.0)
        );
        assert!(
            field
                .to_goal(Vec2::new(60.0, 0.0), 0.0)
                .is_some_and(|cost| cost > 55.0)
        );
    }

    #[test]
    fn steep_ground_is_climbed_and_a_box_edge_is_not() {
        // The goal is on the high ground west of x = 0, 3 m up: a step of 3 m over one
        // node is a 0.92 grade, which is driven as ground and not as a box's edge.
        let goal = |top: f32| Zone {
            center: Vec2::new(-20.0, 0.0),
            yaw: 0.0,
            half_extents: Vec2::splat(8.0),
            bottom: top - 0.5,
            top: 20.0,
        };
        let heights = HeightGrid::from_fn(161, 160.0, |x, _| if x < 0.0 { 3.0 } else { 0.0 });
        let field = NavField::build(
            &heights,
            &[],
            &goal(3.0),
            &goal(3.0),
            Vec2::ZERO,
            70.0,
            NavParams::default(),
        );
        assert!(
            field
                .to_goal(Vec2::new(30.0, 0.0), 0.0)
                .is_some_and(|cost| cost < 60.0),
            "{:?}",
            field.to_goal(Vec2::new(30.0, 0.0), 0.0)
        );
        let flat = HeightGrid::from_fn(161, 160.0, |_, _| 0.0);
        let boxes = [GroundBox {
            min: Vec2::new(-80.0, -80.0),
            max: Vec2::new(0.0, 80.0),
            bottom: 0.0,
            top: 3.0,
            faces: None,
        }];
        let field = NavField::build(
            &flat,
            &boxes,
            &goal(3.0),
            &goal(3.0),
            Vec2::ZERO,
            70.0,
            NavParams::default(),
        );
        assert!(field.to_goal(Vec2::new(30.0, 0.0), 0.0).is_none());
        // A box edge a truck steps up is driven.
        let boxes = [GroundBox {
            top: 0.4,
            ..boxes[0].clone()
        }];
        let field = NavField::build(
            &flat,
            &boxes,
            &goal(0.4),
            &goal(0.4),
            Vec2::ZERO,
            70.0,
            NavParams::default(),
        );
        assert!(field.to_goal(Vec2::new(30.0, 0.0), 0.0).is_some());
    }

    #[test]
    fn a_ramp_that_meets_a_box_flush_between_two_nodes_joins_it() {
        // Hypercube Rumble's ramps: a box 12 m high, and ground rising to its top at a
        // 0.31 grade, which the node nearest the box, up to 3.25 m short of it, samples a
        // metre low. The box's edge is at x = 10, and the nodes are at ... 8.5, 11.75.
        let heights = HeightGrid::from_fn(161, 160.0, |x, _| {
            if x >= 10.0 {
                (12.0 - (x - 10.0) * 0.31).max(0.0)
            } else {
                0.0
            }
        });
        let boxes = [GroundBox {
            min: Vec2::new(-30.0, -30.0),
            max: Vec2::new(10.0, 30.0),
            bottom: 0.0,
            top: 12.0,
            faces: None,
        }];
        let top = Zone {
            center: Vec2::new(-10.0, 0.0),
            yaw: 0.0,
            half_extents: Vec2::splat(10.0),
            bottom: 11.5,
            top: 20.0,
        };
        let field = NavField::build(
            &heights,
            &boxes,
            &top,
            &top,
            Vec2::ZERO,
            70.0,
            NavParams::default(),
        );
        let path = field.path_from(Vec2::new(50.0, 0.0), 0.0, 100, 0);
        assert!(path.iter().all(|place| place.leap == 0.0), "{path:?}");
        assert!(field.to_goal(Vec2::new(50.0, 0.0), 0.0).is_some());
        // And the same ramp stopping a metre short of the top does not.
        let short = HeightGrid::from_fn(161, 160.0, |x, _| {
            if x >= 10.0 {
                (11.0 - (x - 10.0) * 0.31).max(0.0)
            } else {
                0.0
            }
        });
        let field = NavField::build(
            &short,
            &boxes,
            &top,
            &top,
            Vec2::ZERO,
            70.0,
            NavParams::default(),
        );
        assert!(field.to_goal(Vec2::new(50.0, 0.0), 0.0).is_none());
    }

    /// Arena Rumble, roughly: a platform 10 m up with a hump on its east side whose crest
    /// is 7 m up and 11 m from the platform's edge, with a dip between.
    fn arena_field() -> NavField {
        let heights = HeightGrid::from_fn(161, 160.0, |x, _| {
            if (10.0..=21.0).contains(&x) {
                // Down from the edge to the crest: 0 at the edge, 7 at x = 21.
                7.0 * (x - 10.0) / 11.0
            } else if (21.0..=31.0).contains(&x) {
                7.0 * (31.0 - x) / 10.0
            } else {
                0.0
            }
        });
        let boxes = [GroundBox {
            min: Vec2::splat(-10.0),
            max: Vec2::splat(10.0),
            bottom: 0.0,
            top: 10.0,
            faces: None,
        }];
        NavField::build(
            &heights,
            &boxes,
            &zone(5.0),
            &zone(10.0),
            Vec2::ZERO,
            70.0,
            NavParams::default(),
        )
    }

    #[test]
    fn a_platform_across_a_gap_is_leapt_onto_from_the_crest() {
        let field = arena_field();
        let path = field.path_from(Vec2::new(50.0, 0.0), 0.0, 100, 0);
        let at_crest = path
            .iter()
            .position(|place| place.leap > 0.0)
            .expect("a leap on the way");
        let crest = path[at_crest];
        assert!((19.0..=23.0).contains(&crest.at.x), "crest at {}", crest.at);
        // Run up to straight from well back.
        let start = path[at_crest - 1];
        assert!(
            start.at.x > crest.at.x + 20.0 && start.at.y == crest.at.y,
            "{start:?}"
        );
        // Off a 0.7 slope, 13 m to a landing 3 m up: about 14 m/s, and a tenth more.
        assert!((14.0..=20.0).contains(&crest.leap), "{} m/s", crest.leap);
        let after = path
            .iter()
            .skip_while(|place| place.leap == 0.0)
            .nth(1)
            .expect("a landing");
        assert!(after.at.x < 10.0, "lands at {}", after.at);
        assert!(field.on_top(after.at, 11.0));
        // Another lane runs up a crest beside it, to land beside its landing.
        let beside = field.path_from(Vec2::new(50.0, 0.0), 0.0, 100, 1);
        let other = beside.iter().find(|place| place.leap > 0.0).unwrap();
        assert!((other.at.y - crest.at.y).abs() > 3.0, "{other:?}");
        assert!((other.at.x - crest.at.x).abs() < 0.1);
        // From the other side, with no hump there, the way goes round to the hump.
        let round = field.path_from(Vec2::new(-50.0, 0.0), 0.0, 200, 0);
        assert!(
            round
                .iter()
                .any(|place| place.leap > 0.0 && place.at.x > 20.0)
        );
        // And the way down from the top is not a leap back.
        assert!(
            field
                .path_from(Vec2::ZERO, 11.0, 10, 0)
                .iter()
                .all(|place| place.leap == 0.0)
        );
    }

    #[test]
    fn the_leap_speed_is_the_sum_of_a_throw() {
        // Flat ground launches nothing; a landing above the line of launch cannot be
        // reached; the Arena's hump needs about 15 m/s before the margin.
        assert!(leap_speed(0.0, 10.0, 0.0).is_none());
        assert!(leap_speed(0.5, 10.0, 6.0).is_none());
        let arena = leap_speed(0.65, 13.0, 3.4).unwrap() / LEAP_MARGIN;
        assert!((arena - 15.3).abs() < 0.5, "{arena}");
    }
}
