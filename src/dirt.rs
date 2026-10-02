//! The dirt that trucks throw up as they drive: clods off the tread, and dust.
//!
//! A tire on the ground throws dirt while it moves: more the faster it goes, the more it
//! slides sideways, and the harder it is driven from slow, when it spins. Clods are small
//! dark lumps, thrown up behind and out to the side, that tumble, fall, and are gone when
//! they come down on the ground. Dust is big, faint puffs that rise a little, spread, and
//! fade. The dirt is the colour of the ground under the tire: the average of its tile on a
//! track that has ground textures, plain brown on one that has not.
//!
//! A tire touches the ground when the bottom of it is within `TOUCHING` of the ground's
//! height. On a bridge, on another truck or in the air it throws nothing, nor in water,
//! which throws its own spray (see the `water` slice). It throws up only loose ground
//! (`track::TrackData::loose_at`): dirt, mud, sand, grass and rocky ground, as the track's
//! texture types say, or, where they say nothing, as the ground's colour does. Road, rock,
//! water, ice and snow throw nothing.
//!
//! Each piece is a flat square turned to the camera, drawn only, which touches nothing.
//! Clods and dust are two pools of particles (`crate::particles`), which the graphics card
//! moves: here they are only thrown. Where the truck is comes from how it is drawn
//! (`TruckVisual`), so that the dirt leaves the tires where they are seen; how fast it
//! goes, from its body.
//!
//! Dust rises only off dry ground: in clear or overcast weather. In fog, rain and snow the
//! tires throw clods alone.
//!
//! Uses the `track` slice for the ground, the `truck` slice for the trucks, and the
//! `weather` slice, if it is there, for whether the ground is dry. Does nothing in an app
//! that cannot draw. The values are the game's own: Monster Truck
//! Madness 2's dirt has not been measured.

use avian3d::prelude::{AngularVelocity, LinearVelocity};
use bevy::asset::RenderAssetUsages;
use bevy::ecs::entity::EntityHashMap;
use bevy::pbr::PbrPlugin;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::game_state::GameState;
use crate::particles::{self, Air, Motion, Particle, Particles};
use crate::track::{Track, TrackSystems};
use crate::truck::{TruckConfig, TruckInput, TruckSystems, TruckVisual};
use crate::weather::WeatherSettings;

/// How near the ground the bottom of a tire must be to throw dirt, in metres. A tire sinks
/// into the ground as its suspension works, and rides over bumps smaller than a cell.
const TOUCHING: f32 = 0.3;
/// How fast a tire must be going over the ground to throw dirt, in m/s.
const DIRT_SPEED: f32 = 2.0;
/// How many clods a tire throws each second: for each m/s it goes faster than
/// `DIRT_SPEED`, for each m/s it slides sideways, and at full throttle from standing,
/// where the tire spins. And the most, however it is driven.
const CLODS_PER_SPEED: f32 = 4.0;
const CLODS_PER_SLIDE: f32 = 12.0;
const CLODS_SPINNING: f32 = 40.0;
const MOST_CLODS: f32 = 120.0;
/// How many puffs of dust a tire throws for each clod.
const DUST_PER_CLOD: f32 = 0.2;
/// How fast a truck goes, in m/s, before its tires stop spinning at full throttle.
const SPIN_SPEED: f32 = 8.0;
/// The most clods, and the most puffs of dust, in the air at once, over all the trucks.
/// Past it, no more are thrown until some have gone. Each is a square in a mesh, drawn
/// whether it is in the air or not.
const CLOD_SLOTS: usize = 1200;
const DUST_SLOTS: usize = 400;

/// How big a clod is, in metres of radius: the smallest and the largest.
const CLOD_SIZE: [f32; 2] = [0.04, 0.11];
/// How fast a clod leaves the tire, for each m/s the tire goes over the ground, straight
/// up: the least and the most. A truck at 10 m/s throws them a few tenths of a metre up.
const CLOD_LIFT: [f32; 2] = [0.08, 0.2];
/// How fast a spinning tire throws clods back, at full throttle, in m/s: the least and the
/// most.
const CLOD_KICK: [f32; 2] = [1.0, 4.0];
/// How long a clod may stay up, in seconds, if it doesn't come down on the ground first,
/// and how fast it tumbles at the most, in radians a second.
const CLOD_LIFE: f32 = 2.0;
const CLOD_TUMBLE: f32 = 12.0;
/// How dark a clod is beside the ground it came from: the damp dirt under the top.
const CLOD_SHADE: f32 = 0.6;
/// How much of its speed through the air a clod loses each second.
const CLOD_DRAG: f32 = 0.3;

/// How big a puff of dust is when it is thrown, in metres of radius, and how fast it
/// spreads, in metres of radius each second.
const DUST_SIZE: [f32; 2] = [0.3, 0.6];
const DUST_SPREAD: f32 = 1.2;
/// How long a puff of dust lasts, in seconds, and how long of that it stays as it was
/// thrown. It fades out over the rest.
const DUST_LIFE: f32 = 1.6;
const DUST_HOLD: f32 = 0.3;
/// How quickly the air slows dust, as a share of its speed each second, and how fast it
/// rises, in m/s²: it is warm, fine, and carried more than it falls.
const DUST_DRAG: f32 = 2.0;
const DUST_RISE: f32 = 0.3;
/// How see-through dust is when it is thrown, from 0 (not there) to 1 (solid), and how
/// much paler than the ground it is.
const DUST_OPACITY: f32 = 0.3;
const DUST_PALENESS: f32 = 0.4;
/// How much brighter dust is than its ground's colour. Dust is not lit, and sunlit ground
/// is brighter than its colour: at 1, dust looked darker than the ground, like smoke.
const DUST_BRIGHTNESS: f32 = 1.5;

/// The colour of the dirt on a track without ground textures.
const PLAIN_DIRT: Vec3 = Vec3::new(0.45, 0.36, 0.25);
/// How near the camera a piece starts to look smaller the nearer it comes, in metres, so
/// that dirt thrown back at the camera does not pass it as big blobs.
const NEAR_CAMERA: f32 = 6.0;
const GRAVITY: f32 = 9.81;

pub struct DirtPlugin;

impl Plugin for DirtPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DirtSettings>();
        // Dirt is only a look. An app that cannot draw has no use for it.
        if !app.is_plugin_added::<PbrPlugin>() {
            return;
        }
        particles::add(app);
        // Made on entering the first race, not at `Startup`: a race begun from the command
        // line enters `Racing` before `Startup` runs.
        app.add_systems(
            OnEnter(GameState::Racing),
            (
                make_looks.run_if(not(resource_exists::<DirtLooks>)),
                spawn_dirt,
            )
                .chain()
                .after(TrackSystems::Prepare),
        )
        .add_systems(
            Update,
            throw_dirt
                .after(TruckSystems::PlaceVisuals)
                .run_if(in_state(GameState::Racing)),
        );
    }
}

/// Choices about the dirt. Change it at any time.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct DirtSettings {
    /// Whether trucks throw dirt up. Dirt in the air when it is turned off falls as it
    /// would.
    pub on: bool,
}

impl Default for DirtSettings {
    fn default() -> Self {
        Self { on: true }
    }
}

/// On each of the race's pools of dirt: the clods, and the dust.
#[derive(Component)]
pub struct Dirt;

/// On the pool of clods.
#[derive(Component)]
struct Clods;

/// On the pool of dust.
#[derive(Component)]
struct Dust;

/// How clods move: thrown, and pulled down, until they come down on the ground.
const CLOD_MOTION: Motion = Motion {
    gravity: GRAVITY,
    drag: CLOD_DRAG,
    life: CLOD_LIFE,
    lands: true,
    spread: 0.0,
    fade_from: CLOD_LIFE,
    shrink_from: CLOD_LIFE,
    exposure: 0.0,
    near_camera: NEAR_CAMERA,
};

/// How dust moves: soon slowed, rising a little, spreading, and fading after a while.
const DUST_MOTION: Motion = Motion {
    gravity: -DUST_RISE,
    drag: DUST_DRAG,
    life: DUST_LIFE,
    lands: false,
    spread: DUST_SPREAD,
    fade_from: DUST_HOLD,
    shrink_from: DUST_LIFE,
    exposure: 0.0,
    near_camera: NEAR_CAMERA,
};

/// What dirt is drawn with: one picture of each kind.
#[derive(Resource)]
struct DirtLooks {
    clod: Handle<Image>,
    dust: Handle<Image>,
}

/// The colours of this track's ground: one for each ground tile, or `PLAIN_DIRT` alone.
#[derive(Resource, Default)]
struct GroundColors(Vec<Vec3>);

/// What `throw_dirt` remembers of each truck from one frame to the next: per wheel, the
/// pieces it owes, carried over as a fraction.
#[derive(Default)]
struct Owed([f32; 4]);

fn make_looks(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(DirtLooks {
        clod: images.add(picture(clod_opacity)),
        dust: images.add(picture(dust_opacity)),
    });
}

/// Makes the race's pools of dirt, and works out the colours of the ground of its track.
fn spawn_dirt(
    mut commands: Commands,
    track: Res<Track>,
    looks: Res<DirtLooks>,
    mut assets: particles::PoolAssets,
) {
    for (slots, motion, picture, is_clods) in [
        (CLOD_SLOTS, CLOD_MOTION, &looks.clod, true),
        (DUST_SLOTS, DUST_MOTION, &looks.dust, false),
    ] {
        let pool = particles::pool(&mut assets, slots, motion, Air::STILL, picture.clone());
        let mut pool = commands.spawn((pool, Dirt, DespawnOnExit(GameState::Racing)));
        if is_clods {
            pool.insert(Clods);
        } else {
            pool.insert(Dust);
        }
    }
    let colors = match &track.ground {
        Some(ground) => ground
            .tiles
            .iter()
            .map(|tile| average_color(tile))
            .collect(),
        None => vec![PLAIN_DIRT],
    };
    commands.insert_resource(GroundColors(colors));
}

/// Which of `GroundColors` the ground at `at` (world X and Z) is.
fn ground_color(track: &Track, at: Vec2) -> usize {
    let Some(ground) = &track.ground else {
        return 0;
    };
    let cells = ground.cells_per_side();
    let cell = |world: f32| {
        let across = (world / track.heights.size() + 0.5).clamp(0.0, 1.0);
        ((across * cells as f32) as usize).min(cells - 1)
    };
    ground.cells[cell(at.y) * cells + cell(at.x)].tile
}

/// A tile's average colour, as it appears on screen, from 0 to 1.
fn average_color(rgba: &[u8]) -> Vec3 {
    let pixels = (rgba.len() / 4).max(1) as f32;
    let sum = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .fold(Vec3::ZERO, |sum, [r, g, b, _]| {
            sum + Vec3::new(*r as f32, *g as f32, *b as f32)
        });
    sum / pixels / 255.0
}

#[allow(clippy::too_many_arguments)]
fn throw_dirt(
    time: Res<Time>,
    track: Res<Track>,
    colors: Res<GroundColors>,
    settings: Res<DirtSettings>,
    weather: Option<Res<WeatherSettings>>,
    trucks: Query<(Entity, &Transform, &TruckVisual)>,
    bodies: Query<(
        &LinearVelocity,
        &AngularVelocity,
        &TruckConfig,
        Option<&TruckInput>,
    )>,
    mut clods: Query<&mut Particles, (With<Clods>, Without<Dust>)>,
    mut dust: Query<&mut Particles, (With<Dust>, Without<Clods>)>,
    mut owed: Local<EntityHashMap<Owed>>,
    mut random: Local<Random>,
) {
    if !settings.on {
        return;
    }
    let (Ok(mut clods), Ok(mut dust)) = (clods.single_mut(), dust.single_mut()) else {
        return;
    };
    let dt = time.delta_secs();
    let now = particles::clock(&time);
    // Without the weather, the ground is dry.
    let dusty = weather.is_none_or(|weather| weather.weather.dusty());
    let ground = |at: Vec2| track.heights.height_at(at.x, at.y);
    // Trucks from a race that is over are forgotten.
    owed.retain(|visual, _| trucks.contains(*visual));

    for (visual, transform, truck) in &trucks {
        let Ok((linear, angular, config, input)) = bodies.get(truck.truck) else {
            continue;
        };
        let throttle = input.map_or(0.0, |input| input.throttle);
        let owed = owed.entry(visual).or_default();
        let up = transform.up().as_vec3();
        let right = transform.right().as_vec3();
        let forward = transform.forward().as_vec3();
        for (index, rest) in config.wheel_rest.iter().enumerate() {
            let hub = transform.transform_point(*rest);
            let bottom = hub - up * config.wheel_radius;
            let height = track.heights.height_at(bottom.x, bottom.z);
            let under_water = track.water_level.is_some_and(|level| level > height);
            let loose = track.loose_at(bottom.x, bottom.z);
            if !touching(bottom.y - height, up) || under_water || !loose {
                owed.0[index] = 0.0;
                continue;
            }
            let moving = linear.0 + angular.0.cross(hub - transform.translation);
            let speed = moving.xz().length();
            let slide = moving.dot(right).abs();
            owed.0[index] += clod_rate(speed, slide, throttle) * dt;
            if owed.0[index] < 1.0 {
                continue;
            }
            let color = ground_color(&track, bottom.xz());
            let ground_rgb = colors.0.get(color).copied().unwrap_or(PLAIN_DIRT);
            let (clod_color, dust_color) = dirt_colors(ground_rgb);
            // Behind the tire, where the tread leaves the ground, on its outside.
            let outwards = right * rest.x.signum();
            let back = if moving.dot(forward) >= 0.0 {
                -forward
            } else {
                forward
            };
            let at = bottom
                + back * config.wheel_radius * 0.5
                + outwards * config.wheel_width * 0.3
                + up * 0.1;
            while owed.0[index] >= 1.0 {
                owed.0[index] -= 1.0;
                let clod = Particle {
                    at,
                    velocity: clod_velocity(&mut random, moving, back, outwards, throttle),
                    size: random.between(CLOD_SIZE[0], CLOD_SIZE[1]),
                    turned: random.between(0.0, std::f32::consts::TAU),
                    turning: random.between(-CLOD_TUMBLE, CLOD_TUMBLE),
                    color: clod_color,
                };
                if !clods.throw(now, clod, ground) {
                    break;
                }
                if dusty && random.next() < DUST_PER_CLOD {
                    let velocity = moving.with_y(0.0) * random.between(0.1, 0.4)
                        + outwards * random.between(0.0, 1.0)
                        + Vec3::Y * random.between(0.3, 1.0);
                    let puff = Particle {
                        at: at + up * 0.2,
                        velocity,
                        size: random.between(DUST_SIZE[0], DUST_SIZE[1]),
                        turned: random.between(0.0, std::f32::consts::TAU),
                        turning: random.between(-0.5, 0.5),
                        color: dust_color,
                    };
                    dust.throw(now, puff, ground);
                }
            }
            // With no room, what is owed is dropped rather than thrown all at once later.
            owed.0[index] = owed.0[index].min(1.0);
        }
    }
}

/// The colours of the clods and the dust from ground of `ground`'s colour (sRGB, 0 to 1),
/// with how solid each is.
fn dirt_colors(ground: Vec3) -> (LinearRgba, LinearRgba) {
    let clod = ground * CLOD_SHADE;
    let dust = (ground.lerp(Vec3::splat(ground.max_element()), DUST_PALENESS) * DUST_BRIGHTNESS)
        .min(Vec3::ONE);
    (
        Color::srgb(clod.x, clod.y, clod.z).into(),
        Color::srgba(dust.x, dust.y, dust.z, DUST_OPACITY).into(),
    )
}

/// Whether a tire whose bottom is `above` metres over the ground, on a truck whose up is
/// `up`, is on it. A truck on its side or its roof has no tire on the ground.
fn touching(above: f32, up: Vec3) -> bool {
    above.abs() < TOUCHING && up.y > 0.5
}

/// How many clods a tire throws each second, going `speed` m/s over the ground, sliding
/// `slide` m/s sideways, at `throttle` (-1 to 1).
fn clod_rate(speed: f32, slide: f32, throttle: f32) -> f32 {
    let rolling = (speed - DIRT_SPEED).max(0.0) * CLODS_PER_SPEED;
    let sliding = slide * CLODS_PER_SLIDE;
    let spinning = throttle.abs() * (1.0 - speed / SPIN_SPEED).max(0.0) * CLODS_SPINNING;
    (rolling + sliding + spinning).min(MOST_CLODS)
}

/// How fast a clod leaves a tire that is going at `moving`, with `back` pointing behind
/// the way it goes and `outwards` away from the truck's side, at `throttle`: on with the
/// truck at some of its speed, so that it falls behind; out to the side; a little up; and,
/// the harder the tire is driven, kicked back.
fn clod_velocity(
    random: &mut Random,
    moving: Vec3,
    back: Vec3,
    outwards: Vec3,
    throttle: f32,
) -> Vec3 {
    let along = moving.with_y(0.0);
    let speed = along.length();
    along * random.between(0.2, 0.6)
        + back * random.between(CLOD_KICK[0], CLOD_KICK[1]) * throttle.abs()
        + outwards * random.between(0.2, 1.2)
        + Vec3::Y * (random.between(0.8, 1.6) + speed * random.between(CLOD_LIFT[0], CLOD_LIFT[1]))
}

/// How many texels the pictures have along a side.
const PICTURE_SIZE: u32 = 32;

/// A white picture, as solid as `opacity` says at each point of it, from -1 to 1 across
/// and up.
fn picture(opacity: fn(Vec2) -> f32) -> Image {
    Image::new(
        Extent3d {
            width: PICTURE_SIZE,
            height: PICTURE_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        picture_texels(PICTURE_SIZE, opacity),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn picture_texels(size: u32, opacity: fn(Vec2) -> f32) -> Vec<u8> {
    let middle = size as f32 / 2.0;
    let mut texels = Vec::with_capacity((size * size * 4) as usize);
    for row in 0..size {
        for col in 0..size {
            let point = Vec2::new(col as f32 + 0.5 - middle, middle - row as f32 - 0.5) / middle;
            let alpha = (opacity(point).clamp(0.0, 1.0) * 255.0).round() as u8;
            texels.extend_from_slice(&[255, 255, 255, alpha]);
        }
    }
    texels
}

/// A clod: a lump, round but lopsided, with a ragged edge.
fn clod_opacity(point: Vec2) -> f32 {
    let angle = point.y.atan2(point.x);
    let edge = 0.75 + 0.12 * (3.0 * angle).sin() + 0.06 * (7.0 * angle + 1.0).sin();
    1.0 - smoothstep(edge - 0.1, edge, point.length())
}

/// Dust: thickest in the middle and thinning all the way out.
fn dust_opacity(point: Vec2) -> f32 {
    let thinning = (1.0 - point.length_squared()).max(0.0);
    thinning * thinning
}

fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A small, quick source of numbers that only have to look random (xorshift).
struct Random(u64);

impl Default for Random {
    fn default() -> Self {
        Self(0x2545_F491_4F6C_DD1D)
    }
}

impl Random {
    /// A number from 0 up to 1.
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    fn between(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_tire_on_the_ground_the_right_way_up_throws_dirt() {
        assert!(touching(0.1, Vec3::Y));
        assert!(touching(-0.2, Vec3::Y));
        assert!(!touching(1.0, Vec3::Y));
        assert!(!touching(0.0, -Vec3::Y));
        assert!(!touching(0.0, Vec3::X));
    }

    #[test]
    fn faster_sliding_and_spinning_throw_more_up_to_a_limit() {
        assert_eq!(clod_rate(0.0, 0.0, 0.0), 0.0);
        assert_eq!(clod_rate(DIRT_SPEED, 0.0, 0.0), 0.0);
        assert!(clod_rate(15.0, 0.0, 0.0) > clod_rate(6.0, 0.0, 0.0));
        assert!(clod_rate(6.0, 3.0, 0.0) > clod_rate(6.0, 0.0, 0.0));
        // Full throttle from standing spins the tire; at speed it grips.
        assert!(clod_rate(0.0, 0.0, 1.0) > 0.0);
        assert_eq!(
            clod_rate(SPIN_SPEED, 0.0, 1.0),
            clod_rate(SPIN_SPEED, 0.0, 0.0)
        );
        assert_eq!(clod_rate(1000.0, 1000.0, 1.0), MOST_CLODS);
    }

    #[test]
    fn clods_go_up_out_and_fall_behind() {
        let mut random = Random::default();
        let moving = Vec3::new(0.0, 0.0, -10.0);
        for _ in 0..100 {
            let velocity = clod_velocity(&mut random, moving, Vec3::Z, Vec3::X, 1.0);
            assert!(velocity.y > 0.0);
            assert!(velocity.x > 0.0);
            // Slower than the truck, so that it falls behind.
            assert!(velocity.z > moving.z);
            // Low: under a metre and a half up.
            assert!(
                velocity.y * velocity.y / (2.0 * GRAVITY) < 1.5,
                "{velocity}"
            );
        }
        // A spinning tire kicks them back.
        let still = clod_velocity(&mut random, Vec3::ZERO, Vec3::Z, Vec3::X, 1.0);
        assert!(still.z > 0.0);
    }

    #[test]
    fn a_tiles_colour_is_its_average() {
        let tile = [255, 0, 0, 255, 0, 0, 255, 255];
        assert_eq!(average_color(&tile), Vec3::new(0.5, 0.0, 0.5));
    }

    #[test]
    fn pictures_are_solid_in_the_middle_and_clear_at_the_corners() {
        for opacity in [clod_opacity as fn(Vec2) -> f32, dust_opacity] {
            let texels = picture_texels(16, opacity);
            let alpha = |col: usize, row: usize| texels[(row * 16 + col) * 4 + 3];
            assert!(alpha(8, 8) > 200);
            assert_eq!(alpha(0, 0), 0);
            assert_eq!(alpha(15, 15), 0);
        }
        // A clod is solid most of the way out; dust thins all the way.
        assert!(clod_opacity(Vec2::new(0.5, 0.0)) > dust_opacity(Vec2::new(0.5, 0.0)));
    }
}
