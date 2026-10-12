//! The sparks that fly where a truck's body scrapes or hits something hard.
//!
//! A truck's bodywork is metal. Dragged along a road, a wall, a rail, a parked car or
//! another truck, it throws sparks: more the faster it scrapes. Hit hard, it throws a
//! burst of them. A truck on its roof or its side sliding down the road sparks all the
//! way; on grass, dirt or sand it ploughs, and throws nothing here (the dirt is `dirt`'s).
//! Tires are rubber: a tire against a wall or another tire throws no sparks, but a tire
//! that hits a truck's body does, since the body is hit.
//!
//! Where a body touches comes from the physics' contacts after each step
//! (`find_scrapes`, in `FixedPostUpdate`): every contact pair with a truck's body on one
//! side, whose other side is the terrain only where the ground there is firm
//! (`track::TrackData::loose_at`) and above the water, or anything else that is not a
//! tire. Each becomes a `Scrape` for the frame, with how fast the body slides over what it
//! touches there and how hard it hit. `throw_sparks` throws them in `Update`, from where
//! the touch is on the truck as it is drawn (`TruckVisual`), so that the sparks leave the
//! body where it is seen.
//!
//! A spark is a hot speck drawn as a streak along its path (`Motion::exposure`), thrown
//! along the way the body drags over the surface, slower than the body, with a spread, and
//! a little off the surface, so that it skitters along the ground behind a sliding truck
//! and showers off a wall. It falls, and is gone when it comes down on the ground. Sparks
//! glow: they are not dimmed with the weather and the time of day as the other particles
//! are (`Glows`), and are bright enough to bloom.
//!
//! The values are the game's own: Monster Truck Madness 2's sparks are not measured.

use avian3d::prelude::{
    AngularVelocity, CollisionLayers, ComputedCenterOfMass, ContactGraph, ContactManifold,
    LinearVelocity, Position, Rotation,
};
use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;

use super::dirt::{Random, picture};
use super::pool::{self, Air, Motion, Particle, Particles};
use crate::collision_groups::{GROUND, GROUND_BOXES};
use crate::game_state::GameState;
use crate::scene_depth::SceneDepth;
use crate::track::Track;
use crate::truck::{Truck, TruckVisual};

/// How fast a body must slide over what it touches to throw sparks, in m/s, and how many
/// sparks it throws each second for each m/s faster than that. And the most a touch throws
/// a second, however fast it slides.
const SCRAPE_SPEED: f32 = 1.0;
const SPARKS_PER_SPEED: f32 = 50.0;
const MOST_SCRAPE_SPARKS: f32 = 500.0;
/// How fast two things must come together for the hit to throw a burst of sparks, in m/s,
/// and how many sparks the burst has for each m/s faster than that. And the most in one
/// burst.
const IMPACT_SPEED: f32 = 2.0;
const SPARKS_PER_IMPACT: f32 = 20.0;
const MOST_IMPACT_SPARKS: f32 = 120.0;
/// How much of the body's speed over the surface a spark keeps as it comes off, the least
/// and the most: it is dragged along by the body and held back by the surface.
const SPARK_KEEPS: [f32; 2] = [0.3, 0.9];
/// How fast a spark is thrown aside from the way the body goes, at the most, in m/s, and
/// how fast off the surface, the least and the most, in m/s.
const SPARK_SPREAD: f32 = 3.0;
const SPARK_OFF: [f32; 2] = [0.5, 3.0];
/// How fast a spark leaves a hit, the least and the most, in m/s, every way from the
/// touch.
const BURST_SPEED: [f32; 2] = [3.0, 10.0];
/// How long a spark may stay up, in seconds, if it doesn't come down on the ground first,
/// and how old it is when it starts to fade.
const SPARK_LIFE: f32 = 0.6;
const SPARK_FADE_FROM: f32 = 0.2;
/// How much of its speed through the air a spark loses each second.
const SPARK_DRAG: f32 = 1.2;
/// How big a spark is, in metres of radius, and how long the eye sees it for, in seconds,
/// which draws it as a streak as long as its path in that time.
const SPARK_SIZE: [f32; 2] = [0.012, 0.025];
const SPARK_EXPOSURE: f32 = 0.03;
/// A spark's colour: white-hot in the middle, orange at the edge, brighter than anything
/// the sun lights (see `camera::GLOW_THRESHOLD`), so that it blooms.
const SPARK_COLOR: [LinearRgba; 2] = [
    LinearRgba::new(6.0, 4.5, 2.5, 1.0),
    LinearRgba::new(5.0, 2.0, 0.5, 1.0),
];
/// The most sparks in the air at once, over all the trucks. Past it, no more are thrown
/// until some have gone.
const SPARK_SLOTS: usize = 3000;
/// How near the camera a spark starts to look smaller the nearer it comes, in metres.
const NEAR_CAMERA: f32 = 4.0;
/// How far above the water a touch must be to throw sparks, in metres: a body on frozen
/// water, or under it, throws none.
const ABOVE_WATER: f32 = 0.1;
const GRAVITY: f32 = 9.81;

/// Choices about the sparks. Change it at any time.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct SparkSettings {
    /// Whether bodies throw sparks. Sparks in the air when it is turned off fall as they
    /// would.
    pub on: bool,
}

impl Default for SparkSettings {
    fn default() -> Self {
        Self { on: true }
    }
}

/// On the race's pool of sparks.
#[derive(Component)]
pub struct Sparks;

/// On a pool whose particles give their own light: not dimmed with the weather and the
/// time of day, as every other pool is.
#[derive(Component)]
pub struct Glows;

/// A truck's body touching something hard in one physics step. Written by `find_scrapes`
/// after each step, read by `throw_sparks` each frame.
#[derive(Message, Clone, Copy, Debug)]
pub(super) struct Scrape {
    /// The truck whose body it is (the physics body).
    truck: Entity,
    /// Where the touch is, in the truck's own frame, so that it can be found again on the
    /// truck as it is drawn.
    local: Vec3,
    /// Which way the surface pushes the body, of length 1, in the world.
    off_surface: Vec3,
    /// How fast the body goes over the surface at the touch, in m/s, in the world: along
    /// the surface, and relative to it.
    sliding: Vec3,
    /// How fast the surface itself goes there, in m/s, in the world: a moving car or
    /// another truck. The sparks go along with it.
    surface: Vec3,
    /// How fast the two came together this step, in m/s, if they hit: 0 while they only
    /// touch.
    hit: f32,
    /// How long the step was, in seconds.
    step: f32,
}

/// How sparks move: thrown, slowed by the air, pulled down, and gone when they come down
/// on the ground.
const SPARK_MOTION: Motion = Motion {
    gravity: GRAVITY,
    drag: SPARK_DRAG,
    life: SPARK_LIFE,
    lands: true,
    spread: 0.0,
    fade_from: SPARK_FADE_FROM,
    shrink_from: SPARK_LIFE,
    shapes: 1,
    soft: 0.0,
    exposure: SPARK_EXPOSURE,
    near_camera: NEAR_CAMERA,
    lies_on: None,
};

/// The sparks' picture, made once.
#[derive(Resource)]
pub(super) struct SparkLooks(Handle<Image>);

pub(super) fn make_looks(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(SparkLooks(images.add(picture(1, spark_opacity))));
}

/// Makes the race's pool of sparks.
pub(super) fn spawn_sparks(
    mut commands: Commands,
    looks: Res<SparkLooks>,
    scene_depth: Res<SceneDepth>,
    mut assets: pool::PoolAssets,
) {
    let pool = pool::pool(
        &mut assets,
        SPARK_SLOTS,
        SPARK_MOTION,
        Air::STILL,
        looks.0.clone(),
        scene_depth.image.clone(),
    );
    commands.spawn((pool, Sparks, Glows, DespawnOnExit(GameState::Racing)));
}

/// What a body is and how it moves, as the physics has it after the step.
type Body<'a> = (
    &'a Position,
    &'a Rotation,
    &'a LinearVelocity,
    &'a AngularVelocity,
    &'a ComputedCenterOfMass,
);

/// Finds every touch of a truck's body on something hard in the step just done, and
/// writes it as a `Scrape`.
#[allow(clippy::too_many_arguments)]
pub(super) fn find_scrapes(
    time: Res<Time>,
    track: Res<Track>,
    settings: Res<SparkSettings>,
    graph: Res<ContactGraph>,
    trucks: Query<(), With<Truck>>,
    bodies: Query<Body>,
    layers: Query<&CollisionLayers>,
    mut scrapes: MessageWriter<Scrape>,
) {
    if !settings.on {
        return;
    }
    let step = time.delta_secs();
    for pair in graph.iter_active_touching() {
        // The body's collider is on the truck itself; its tires are its children.
        let truck_first = trucks.contains(pair.collider1);
        let truck_second = trucks.contains(pair.collider2);
        if !truck_first && !truck_second {
            continue;
        }
        let (truck, other) = if truck_first {
            (pair.collider1, pair.collider2)
        } else {
            (pair.collider2, pair.collider1)
        };
        let Ok((position, rotation, ..)) = bodies.get(truck) else {
            continue;
        };
        let terrain = layers.get(other).is_ok_and(|layers| {
            layers.memberships.has_all(GROUND) && !layers.memberships.has_all(GROUND_BOXES)
        });
        for manifold in pair.manifolds.iter().filter(|m| !m.points.is_empty()) {
            let Some(point) = mean_point(manifold) else {
                continue;
            };
            if terrain && !hard_ground(&track, point) {
                continue;
            }
            // The normal points out of the first collider, so it pushes the first collider
            // against it and the second along it.
            let off_surface = if truck_first {
                -manifold.normal
            } else {
                manifold.normal
            };
            let (truck_body, other_body) = if truck_first {
                (pair.body1, pair.body2)
            } else {
                (pair.body2, pair.body1)
            };
            let moving_at = |body: Option<Entity>| {
                body.and_then(|body| bodies.get(body).ok())
                    .map_or(Vec3::ZERO, |body| velocity_at(body, point))
            };
            let surface = moving_at(other_body);
            let relative = moving_at(truck_body) - surface;
            let sliding = relative.reject_from_normalized(off_surface);
            // Approaching is negative, before the step solved it.
            let hit = manifold
                .points
                .iter()
                .map(|p| -p.normal_speed)
                .fold(0.0, f32::max);
            scrapes.write(Scrape {
                truck,
                local: rotation.0.inverse() * (point - position.0),
                off_surface,
                sliding,
                surface,
                hit,
                step,
            });
        }
    }
}

/// Whether the ground at `point` throws sparks: firm, as road and rock are, and not
/// under or on the water.
fn hard_ground(track: &Track, point: Vec3) -> bool {
    let wet = track
        .water_level
        .is_some_and(|level| point.y < level + ABOVE_WATER);
    !wet && !track.loose_at(point.x, point.z)
}

/// How fast a body goes at `point`, in the world.
fn velocity_at(
    (position, rotation, linear, angular, centre): Body,
    point: Vec3,
) -> Vec3 {
    let centre = position.0 + rotation.0 * centre.0;
    linear.0 + angular.0.cross(point - centre)
}

/// The middle of a manifold's points, in the world.
fn mean_point(manifold: &ContactManifold) -> Option<Vec3> {
    let count = manifold.points.len();
    (count > 0).then(|| {
        manifold
            .points
            .iter()
            .map(|point| point.point)
            .sum::<Vec3>()
            / count as f32
    })
}

/// Throws this frame's sparks from its `Scrape`s.
pub(super) fn throw_sparks(
    time: Res<Time>,
    track: Res<Track>,
    mut scrapes: MessageReader<Scrape>,
    visuals: Query<(&Transform, &TruckVisual)>,
    mut sparks: Query<&mut Particles, With<Sparks>>,
    mut owed: Local<f32>,
    mut random: Local<Random>,
) {
    let Ok(mut sparks) = sparks.single_mut() else {
        scrapes.clear();
        return;
    };
    let now = pool::clock(&time);
    let ground = |at: Vec2| track.heights.height_at(at.x, at.y);
    // Where each truck is drawn this frame, by its physics body.
    let drawn: EntityHashMap<Transform> = visuals
        .iter()
        .map(|(transform, visual)| (visual.truck, *transform))
        .collect();
    for scrape in scrapes.read() {
        let Some(transform) = drawn.get(&scrape.truck) else {
            continue;
        };
        let at = transform.transform_point(scrape.local);
        let scraping = scrape.sliding.length();
        *owed += scrape_rate(scraping) * scrape.step + burst(scrape.hit);
        let along = scrape.sliding.normalize_or_zero();
        let aside = along.cross(scrape.off_surface).normalize_or_zero();
        while *owed >= 1.0 {
            *owed -= 1.0;
            let velocity = if scraping > SCRAPE_SPEED {
                scrape.surface
                    + scrape.sliding * random.between(SPARK_KEEPS[0], SPARK_KEEPS[1])
                    + aside * random.between(-SPARK_SPREAD, SPARK_SPREAD)
                    + scrape.off_surface * random.between(SPARK_OFF[0], SPARK_OFF[1])
            } else {
                // A hit without a scrape: every way off the surface.
                let any = Vec3::new(
                    random.between(-1.0, 1.0),
                    random.between(-1.0, 1.0),
                    random.between(-1.0, 1.0),
                )
                .normalize_or_zero();
                let out = any - scrape.off_surface * any.dot(scrape.off_surface).min(0.0) * 2.0;
                scrape.surface + out * random.between(BURST_SPEED[0], BURST_SPEED[1])
            };
            let heat = random.between(0.0, 1.0);
            let spark = Particle {
                at: at + scrape.off_surface * 0.05,
                velocity,
                size: random.between(SPARK_SIZE[0], SPARK_SIZE[1]),
                turned: 0.0,
                turning: 0.0,
                color: spark_color(heat),
                trail: Vec3::ZERO,
                shape: 0,
            };
            if !sparks.throw(now, spark, ground) {
                *owed = 0.0;
                break;
            }
        }
        // With no room, what is owed is dropped rather than thrown all at once later.
        *owed = owed.min(1.0);
    }
}

/// How many sparks a touch throws each second, sliding over the surface at `speed` m/s.
fn scrape_rate(speed: f32) -> f32 {
    ((speed - SCRAPE_SPEED).max(0.0) * SPARKS_PER_SPEED).min(MOST_SCRAPE_SPARKS)
}

/// How many sparks a hit at `speed` m/s throws at once.
fn burst(speed: f32) -> f32 {
    ((speed - IMPACT_SPEED).max(0.0) * SPARKS_PER_IMPACT).min(MOST_IMPACT_SPARKS)
}

/// A spark's colour, from white-hot (`heat` 1) to orange (0).
fn spark_color(heat: f32) -> LinearRgba {
    let [hot, cool] = SPARK_COLOR;
    LinearRgba::new(
        cool.red + (hot.red - cool.red) * heat,
        cool.green + (hot.green - cool.green) * heat,
        cool.blue + (hot.blue - cool.blue) * heat,
        1.0,
    )
}

/// A spark: a bright speck, solid in the middle and soft at the edge.
fn spark_opacity(point: Vec2, _shape: u32) -> f32 {
    let out = point.length();
    1.0 - smoothstep(0.35, 1.0, out)
}

fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faster_scraping_throws_more_sparks_up_to_a_limit_and_slow_none() {
        assert_eq!(scrape_rate(0.0), 0.0);
        assert_eq!(scrape_rate(SCRAPE_SPEED), 0.0);
        assert!(scrape_rate(10.0) > scrape_rate(5.0));
        assert_eq!(scrape_rate(1000.0), MOST_SCRAPE_SPARKS);
    }

    #[test]
    fn a_hard_hit_throws_a_burst_and_a_touch_none() {
        assert_eq!(burst(0.0), 0.0);
        assert_eq!(burst(IMPACT_SPEED), 0.0);
        assert!(burst(6.0) > burst(3.0));
        assert_eq!(burst(1000.0), MOST_IMPACT_SPARKS);
    }

    #[test]
    fn sparks_glow_hotter_than_the_sun_lights_and_cool_to_orange() {
        let hot = spark_color(1.0);
        let cool = spark_color(0.0);
        assert!(hot.red > 2.0 && cool.red > 2.0);
        assert!(hot.blue > cool.blue && hot.green > cool.green);
        assert!(cool.red > cool.green && cool.green > cool.blue);
    }

    #[test]
    fn only_firm_ground_above_the_water_is_hard() {
        let mut track = Track(crate::track::builtin_track());
        let ground = track.heights.height_at(0.0, 0.0);
        // The built-in track's ground is dirt: loose, so not hard.
        assert!(!hard_ground(&track, Vec3::new(0.0, ground, 0.0)));
        // Firm ground is hard, unless it is under the water.
        track.0.footing = vec![crate::track::Footing::Firm];
        assert!(hard_ground(&track, Vec3::new(0.0, ground, 0.0)));
        track.0.water_level = Some(ground + 1.0);
        assert!(!hard_ground(&track, Vec3::new(0.0, ground, 0.0)));
        assert!(hard_ground(&track, Vec3::new(0.0, ground + 2.0, 0.0)));
    }

    #[test]
    fn a_body_goes_at_a_point_as_it_turns_about_its_centre() {
        let body = (
            &Position(Vec3::ZERO),
            &Rotation(Quat::IDENTITY),
            &LinearVelocity(Vec3::Y),
            &AngularVelocity(Vec3::Y),
            &ComputedCenterOfMass(Vec3::ZERO),
        );
        let moving = velocity_at(body, Vec3::NEG_Z);
        // Turning about Y, a point ahead (-Z) goes along -X, and up with the body.
        assert!((moving - Vec3::new(-1.0, 1.0, 0.0)).length() < 1e-5, "{moving}");
    }

    #[test]
    fn a_spark_is_solid_in_the_middle_and_soft_at_the_edge() {
        assert_eq!(spark_opacity(Vec2::ZERO, 0), 1.0);
        assert_eq!(spark_opacity(Vec2::new(1.0, 0.0), 0), 0.0);
        let half = spark_opacity(Vec2::new(0.6, 0.0), 0);
        assert!(half > 0.0 && half < 1.0);
    }
}
