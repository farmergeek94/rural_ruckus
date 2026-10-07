//! The dirt that trucks throw up as they drive: clods off the tread, and dust.
//!
//! A tire on the ground throws dirt while it moves: more the faster it goes, the more it
//! slides sideways, and the harder it is driven from slow, when it spins. Mud comes off the
//! tread low on the back of the tire and goes the way the tread goes there: straight back,
//! low, faster the faster the tread turns. A monster truck's tire is about
//! 2 m high. Clods are dark lumps, up to the size of a football, that tumble, fall, and are
//! gone when they come down on the ground; with each go small bits of splatter, the same
//! shapes, which fly further. Dust is big, faint puffs that rise a little, spread, and
//! fade, each drawn long behind its tire, back into the one before, so that the puffs run
//! together in a stream. The dirt is the colour of the ground under the tire: the average
//! of its tile on a track that has ground textures, plain brown on one that has not.
//!
//! A tire touches the ground when the bottom of it is within `TOUCHING` of the ground's
//! height. On a bridge, on another truck or in the air it throws nothing, nor in water,
//! which throws its own spray (see the `water` slice). It throws up only loose ground
//! (`track::TrackData::loose_at`): dirt, mud, sand, grass and rocky ground, as the track's
//! texture types say, or, where they say nothing, as the ground's colour does. Road, rock,
//! water, ice and snow throw nothing.
//!
//! Each piece is a flat square turned to the camera, drawn only, which touches nothing.
//! Clods, splatter and dust are three pools of particles (`crate::particles`), which the
//! graphics card moves: here they are only thrown. Where the truck is comes from how it is drawn
//! (`TruckVisual`), so that the dirt leaves the tires where they are seen; how fast it
//! goes, from its body.
//!
//! Dust rises only off dry ground: in clear or overcast weather. Clods come only off wet
//! ground: in fog, rain, storms and snow.
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
/// How many puffs of dust a tire throws for each clod it would throw on wet ground. Many
/// faint puffs close together, so that the stream leaves the tire smoothly: a few strong
/// ones far apart left it in steps, which flickered by the tire.
const DUST_PER_CLOD: f32 = 1.0;
/// How far a puff of dust trails back from where it is thrown, along the way its tire goes,
/// as a share of the gap to the tire's puff before. Past 1 it reaches into the one before,
/// so that the puffs run together in a stream; higher overlaps them more. And the furthest
/// it trails, in metres, so that the few puffs of a slow tire are not long thin streaks.
const DUST_TRAIL: f32 = 1.5;
const DUST_MOST_TRAIL: f32 = 2.0;
/// How fast a truck goes, in m/s, before its tires stop spinning at full throttle.
const SPIN_SPEED: f32 = 8.0;
/// The most clods, drops of splatter and puffs of dust in the air at once, over all the
/// trucks. Past it, no more are thrown until some have gone. Each is a square in a mesh,
/// drawn whether it is in the air or not.
const CLOD_SLOTS: usize = 1200;
const SPLATTER_SLOTS: usize = 2400;
const DUST_SLOTS: usize = 2000;

/// How big a clod of mud is, in metres of radius for each metre of its tire's radius: the
/// smallest and the largest. A monster truck's tire, about 1 m, throws lumps from the size
/// of a fist to that of a football.
const CLOD_SIZE: [f32; 2] = [0.05, 0.16];
/// Where round the back of its tire mud comes off the tread, in radians up from the bottom:
/// low, just behind where the tread leaves the ground. It goes the way the tread goes there,
/// which is back and a little up: higher throws it up, towards a rooster tail.
const MUD_RELEASE: [f32; 2] = [0.08, 0.4];
/// How much of the tread's speed a clod keeps as it comes off, the least and the most: mud
/// slips on the tread. Splatter, which is thin, keeps more.
const CLOD_FLING: [f32; 2] = [0.4, 0.8];
const SPLATTER_FLING: [f32; 2] = [0.6, 1.0];
/// How fast a tire's tread goes round its hub at full throttle from standing, where it
/// spins, in m/s; and the fastest that throws mud, in m/s, so that at speed the mud does not
/// go out of sight.
const SPIN_TREAD: f32 = 12.0;
const MOST_FLING: f32 = 14.0;
/// How long a clod may stay up, in seconds, if it doesn't come down on the ground first,
/// and how fast it tumbles at the most, in radians a second.
const CLOD_LIFE: f32 = 2.0;
const CLOD_TUMBLE: f32 = 12.0;
/// How dark a clod is beside the ground it came from: the damp dirt under the top.
const CLOD_SHADE: f32 = 0.6;
/// How much of its speed through the air a clod loses each second.
const CLOD_DRAG: f32 = 0.3;
/// How many bits of splatter a tire throws with each clod: small bits of wet mud, the same
/// shapes as the clods. Drawn as streaks, they looked unnatural.
const SPLATTER_PER_CLOD: usize = 2;
/// How big a bit of splatter is, in metres of radius: the smallest and the largest.
const SPLATTER_SIZE: [f32; 2] = [0.03, 0.07];
/// How long a bit of splatter may stay up, in seconds, and how much of its speed through
/// the air it loses each second.
const SPLATTER_LIFE: f32 = 1.2;
const SPLATTER_DRAG: f32 = 0.6;

/// How big a puff of dust is when it is thrown, in metres of radius, and how fast it
/// spreads, in metres of radius each second.
const DUST_SIZE: [f32; 2] = [0.3, 0.6];
const DUST_SPREAD: f32 = 1.2;
/// How long a puff of dust lasts, in seconds, and how long of that it stays as it was
/// thrown. It fades out over the rest.
const DUST_LIFE: f32 = 1.6;
const DUST_HOLD: f32 = 0.3;
/// How near the ground or a truck behind it a puff of dust starts to fade out, in metres.
/// At 0, a puff that a tire or the ground cuts through has a hard edge there; higher is
/// softer, but thins the dust near the ground.
const DUST_SOFT: f32 = 0.5;
/// How quickly the air slows dust, as a share of its speed each second, and how fast it
/// rises, in m/s²: it is warm, fine, and carried more than it falls.
const DUST_DRAG: f32 = 2.0;
const DUST_RISE: f32 = 0.3;
/// How see-through each puff of dust is when it is thrown, from 0 (not there) to 1 (solid),
/// and how much paler than the ground it is. Puffs overlap about twice as many deep as at
/// 0.2 puffs for each clod and 0.3 here, which this matches.
const DUST_OPACITY: f32 = 0.15;
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

/// On the pool of splatter.
#[derive(Component)]
struct Splatter;

/// On the pool of dust.
#[derive(Component)]
struct Dust;

/// The pool marked `A`, and not `B` or `C`, so that the three can be borrowed at once.
type Only<A, B, C> = (With<A>, Without<B>, Without<C>);

/// How clods move: thrown, and pulled down, until they come down on the ground.
const CLOD_MOTION: Motion = Motion {
    gravity: GRAVITY,
    drag: CLOD_DRAG,
    life: CLOD_LIFE,
    lands: true,
    spread: 0.0,
    fade_from: CLOD_LIFE,
    shrink_from: CLOD_LIFE,
    shapes: SHAPES_ACROSS,
    soft: 0.0,
    exposure: 0.0,
    near_camera: NEAR_CAMERA,
};

/// How splatter moves: thrown, and pulled down, until it comes down on the ground.
const SPLATTER_MOTION: Motion = Motion {
    gravity: GRAVITY,
    drag: SPLATTER_DRAG,
    life: SPLATTER_LIFE,
    lands: true,
    spread: 0.0,
    fade_from: SPLATTER_LIFE,
    shrink_from: SPLATTER_LIFE,
    shapes: SHAPES_ACROSS,
    soft: 0.0,
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
    shapes: SHAPES_ACROSS,
    soft: DUST_SOFT,
    exposure: 0.0,
    near_camera: NEAR_CAMERA,
};

/// What dirt is drawn with: the clods' shapes, which splatter shares, and the dust's.
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
        clod: images.add(picture(SHAPES_ACROSS, clod_opacity)),
        dust: images.add(picture(SHAPES_ACROSS, dust_opacity)),
    });
}

/// Makes the race's pools of dirt, and works out the colours of the ground of its track.
fn spawn_dirt(
    mut commands: Commands,
    track: Res<Track>,
    looks: Res<DirtLooks>,
    mut assets: particles::PoolAssets,
) {
    let mut spawn = |slots, motion, picture: &Handle<Image>| {
        let pool = particles::pool(&mut assets, slots, motion, Air::STILL, picture.clone());
        commands
            .spawn((pool, Dirt, DespawnOnExit(GameState::Racing)))
            .id()
    };
    let pools = [
        spawn(CLOD_SLOTS, CLOD_MOTION, &looks.clod),
        spawn(SPLATTER_SLOTS, SPLATTER_MOTION, &looks.clod),
        spawn(DUST_SLOTS, DUST_MOTION, &looks.dust),
    ];
    commands.entity(pools[0]).insert(Clods);
    commands.entity(pools[1]).insert(Splatter);
    commands.entity(pools[2]).insert(Dust);
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
    mut clods: Query<&mut Particles, Only<Clods, Splatter, Dust>>,
    mut splatter: Query<&mut Particles, Only<Splatter, Clods, Dust>>,
    mut dust: Query<&mut Particles, Only<Dust, Clods, Splatter>>,
    mut owed: Local<EntityHashMap<Owed>>,
    mut random: Local<Random>,
) {
    if !settings.on {
        return;
    }
    let (Ok(mut clods), Ok(mut splatter), Ok(mut dust)) =
        (clods.single_mut(), splatter.single_mut(), dust.single_mut())
    else {
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
            let rate = if dusty {
                dust_rate(speed, slide, throttle)
            } else {
                clod_rate(speed, slide, throttle)
            };
            owed.0[index] += rate * dt;
            if owed.0[index] < 1.0 {
                continue;
            }
            let color = ground_color(&track, bottom.xz());
            let ground_rgb = colors.0.get(color).copied().unwrap_or(PLAIN_DIRT);
            let (clod_color, dust_color) = dirt_colors(ground_rgb);
            // Behind the tire, where the tread leaves the ground, on its outside.
            let outwards = right * rest.x.signum();
            // The way the tire rolls: from the throttle when it is too slow to tell.
            let rolls_forward = if speed > 1.0 {
                moving.dot(forward) >= 0.0
            } else {
                throttle >= 0.0
            };
            let back = if rolls_forward { -forward } else { forward };
            let tread = tread_speed(speed, throttle);
            let at = bottom
                + back * config.wheel_radius * 0.5
                + outwards * config.wheel_width * 0.3
                + up * 0.1;
            while owed.0[index] >= 1.0 {
                owed.0[index] -= 1.0;
                let thrown = if dusty {
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
                        // Back from the tire: one that reached forward came in over the
                        // tire, which flickered as each one appeared.
                        trail: dust_trail(moving, rate),
                        shape: random.shape(),
                    };
                    dust.throw(now, puff, ground)
                } else {
                    // Off the tread low on the back of the tire, across its width.
                    let release = random.between(MUD_RELEASE[0], MUD_RELEASE[1]);
                    let across = outwards * config.wheel_width * random.between(-0.4, 0.5);
                    let (off_hub, velocity) = fling(
                        release,
                        config.wheel_radius,
                        moving,
                        back,
                        up,
                        tread * random.between(CLOD_FLING[0], CLOD_FLING[1]),
                    );
                    let clod = Particle {
                        at: hub + off_hub + across,
                        velocity,
                        size: config.wheel_radius * random.between(CLOD_SIZE[0], CLOD_SIZE[1]),
                        turned: random.between(0.0, std::f32::consts::TAU),
                        turning: random.between(-CLOD_TUMBLE, CLOD_TUMBLE),
                        color: clod_color,
                        trail: Vec3::ZERO,
                        shape: random.shape(),
                    };
                    for _ in 0..SPLATTER_PER_CLOD {
                        let release = random.between(MUD_RELEASE[0], MUD_RELEASE[1]);
                        let across = outwards * config.wheel_width * random.between(-0.5, 0.5);
                        let (off_hub, velocity) = fling(
                            release,
                            config.wheel_radius,
                            moving,
                            back,
                            up,
                            tread * random.between(SPLATTER_FLING[0], SPLATTER_FLING[1]),
                        );
                        let drop = Particle {
                            at: hub + off_hub + across,
                            velocity,
                            size: random.between(SPLATTER_SIZE[0], SPLATTER_SIZE[1]),
                            turned: random.between(0.0, std::f32::consts::TAU),
                            turning: random.between(-CLOD_TUMBLE, CLOD_TUMBLE),
                            color: clod_color,
                            trail: Vec3::ZERO,
                            shape: random.shape(),
                        };
                        splatter.throw(now, drop, ground);
                    }
                    clods.throw(now, clod, ground)
                };
                if !thrown {
                    break;
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

/// How many puffs of dust a tire throws each second, going `speed` m/s over the ground,
/// sliding `slide` m/s sideways, at `throttle` (-1 to 1).
fn dust_rate(speed: f32, slide: f32, throttle: f32) -> f32 {
    clod_rate(speed, slide, throttle) * DUST_PER_CLOD
}

/// The trail of a puff of dust from a tire going at `moving`, which throws `rate` puffs each
/// second: back the way the tire came, towards the puff before, `speed / rate` metres back.
fn dust_trail(moving: Vec3, rate: f32) -> Vec3 {
    let along = moving.with_y(0.0);
    if rate <= 0.0 {
        return Vec3::ZERO;
    }
    let gap = along.length() / rate;
    -along.normalize_or_zero() * (gap * DUST_TRAIL).min(DUST_MOST_TRAIL)
}

/// How fast a tire's tread goes round its hub, in m/s, going `speed` m/s over the ground at
/// `throttle` (-1 to 1): as fast as the tire rolls, and faster the harder it is driven from
/// slow, where it spins. No faster than `MOST_FLING`.
fn tread_speed(speed: f32, throttle: f32) -> f32 {
    let spin = throttle.abs() * (1.0 - speed / SPIN_SPEED).max(0.0) * SPIN_TREAD;
    (speed + spin).min(MOST_FLING)
}

/// Where mud comes off a tire, from its hub, and how fast it goes: `angle` radians round
/// the back from the bottom, off a tire of `radius` metres whose hub goes at `moving`, with
/// `back` pointing behind the way it rolls and `up` up from its hub, and whose tread
/// throws it at `thrown` m/s round the hub. It goes the way the tread goes there.
fn fling(angle: f32, radius: f32, moving: Vec3, back: Vec3, up: Vec3, thrown: f32) -> (Vec3, Vec3) {
    let (sin, cos) = angle.sin_cos();
    let off_hub = (back * sin - up * cos) * radius;
    let tread_way = back * cos + up * sin;
    (off_hub, moving + tread_way * thrown)
}

/// How many texels each picture has along a side.
const PICTURE_SIZE: u32 = 32;
/// How many shapes of clod, and of puff of dust, along each side of their pictures: each
/// clod or puff is one of `SHAPES_ACROSS` squared, at random, so that not all look alike.
const SHAPES_ACROSS: u32 = 2;

/// A white picture of `across` by `across` shapes, each as solid as `opacity` says at each
/// point of it, from -1 to 1 across and up, for that shape, counted across the rows from the
/// top left.
fn picture(across: u32, opacity: fn(Vec2, u32) -> f32) -> Image {
    let side = PICTURE_SIZE * across;
    Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        picture_texels(PICTURE_SIZE, across, opacity),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn picture_texels(size: u32, across: u32, opacity: fn(Vec2, u32) -> f32) -> Vec<u8> {
    let middle = size as f32 / 2.0;
    let side = size * across;
    let mut texels = Vec::with_capacity((side * side * 4) as usize);
    for row in 0..side {
        for col in 0..side {
            let shape = (row / size) * across + col / size;
            let (row, col) = (row % size, col % size);
            let point = Vec2::new(col as f32 + 0.5 - middle, middle - row as f32 - 0.5) / middle;
            let alpha = (opacity(point, shape).clamp(0.0, 1.0) * 255.0).round() as u8;
            texels.extend_from_slice(&[255, 255, 255, alpha]);
        }
    }
    texels
}

/// A clod: a lump with a ragged edge, round, long or knobbly, or an angular stone.
fn clod_opacity(point: Vec2, shape: u32) -> f32 {
    // How much longer than wide, how many bumps round its edge, big and small, and where
    // they start, in radians.
    let (squash, bumps, start) = match shape % 4 {
        0 => (1.0, [3.0, 7.0], [0.0, 1.0]),
        1 => (1.35, [2.0, 5.0], [1.3, 0.4]),
        2 => (1.1, [5.0, 9.0], [2.1, 2.9]),
        _ => return stone_opacity(point),
    };
    // Kept inside its square however long it is.
    let point = Vec2::new(point.x * squash, point.y / squash) * squash;
    let angle = point.y.atan2(point.x);
    let edge = 0.72
        + 0.12 * (bumps[0] * angle + start[0]).sin()
        + 0.06 * (bumps[1] * angle + start[1]).sin();
    1.0 - smoothstep(edge - 0.1, edge, point.length())
}

/// An angular stone: five flat sides, a little lopsided.
fn stone_opacity(point: Vec2) -> f32 {
    const SIDES: f32 = 5.0;
    let point = Vec2::new(point.x * 1.15, point.y / 1.15);
    let sector = std::f32::consts::TAU / SIDES;
    let angle = (point.y.atan2(point.x) + 0.4).rem_euclid(sector) - sector / 2.0;
    let edge = 0.8 * (sector / 2.0).cos() / angle.cos();
    1.0 - smoothstep(edge - 0.06, edge, point.length())
}

/// Dust: a soft round puff, thickest in the middle and thinning all the way out, or a
/// billow of a few such puffs run together.
fn dust_opacity(point: Vec2, shape: u32) -> f32 {
    // Each puff of a shape: where its middle is and its radius, inside the square.
    const BILLOWS: [[(Vec2, f32); 3]; 4] = [
        [(Vec2::ZERO, 1.0), (Vec2::ZERO, 0.0), (Vec2::ZERO, 0.0)],
        [
            (Vec2::new(0.05, 0.05), 0.7),
            (Vec2::new(-0.4, -0.25), 0.5),
            (Vec2::new(0.45, -0.3), 0.45),
        ],
        [
            (Vec2::new(-0.1, 0.0), 0.72),
            (Vec2::new(0.4, 0.35), 0.5),
            (Vec2::new(0.3, -0.45), 0.4),
        ],
        [
            (Vec2::new(0.0, -0.1), 0.7),
            (Vec2::new(-0.35, 0.45), 0.45),
            (Vec2::new(0.5, 0.2), 0.45),
        ],
    ];
    BILLOWS[shape as usize % BILLOWS.len()]
        .iter()
        .filter(|(_, radius)| *radius > 0.0)
        .map(|(middle, radius)| {
            let thinning = (1.0 - ((point - *middle) / *radius).length_squared()).max(0.0);
            thinning * thinning
        })
        .fold(0.0, f32::max)
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

    /// One of the `SHAPES_ACROSS` squared shapes of a picture.
    fn shape(&mut self) -> u32 {
        let shapes = SHAPES_ACROSS * SHAPES_ACROSS;
        ((self.next() * shapes as f32) as u32).min(shapes - 1)
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
    fn a_puff_of_dust_reaches_into_the_one_before_along_the_way_its_tire_goes() {
        let moving = Vec3::new(0.0, 1.0, -10.0);
        let rate = dust_rate(10.0, 0.0, 0.0);
        let trail = dust_trail(moving, rate);
        let gap = 10.0 / rate;
        assert!(trail.length() > gap, "{trail} {gap}");
        assert!(trail.normalize().dot(Vec3::Z) > 0.999);
        // A slow tire's few puffs trail only so far; a still one's not at all.
        let slow = Vec3::new(0.0, 0.0, -2.1);
        let slow_trail = dust_trail(slow, dust_rate(2.1, 0.0, 0.0));
        assert_eq!(slow_trail.length(), DUST_MOST_TRAIL);
        assert_eq!(dust_trail(Vec3::ZERO, dust_rate(0.0, 0.0, 1.0)), Vec3::ZERO);
        assert_eq!(dust_trail(slow, 0.0), Vec3::ZERO);
    }

    #[test]
    fn mud_comes_off_low_on_the_tire_and_goes_out_behind() {
        let moving = Vec3::new(0.0, 0.0, -10.0);
        let (back, up) = (Vec3::Z, Vec3::Y);
        let tread = tread_speed(10.0, 0.0);
        assert_eq!(tread, 10.0);
        for angle in MUD_RELEASE {
            let thrown = tread * CLOD_FLING[1];
            let (off_hub, velocity) = fling(angle, 1.0, moving, back, up, thrown);
            // Behind the hub and low on the tire, going a little up, and slower than the
            // truck: back more than up, from where the truck is.
            assert!(off_hub.z > 0.0 && off_hub.y < -0.5, "{off_hub}");
            assert!(velocity.y > 0.0, "{velocity}");
            assert!(velocity.z - moving.z > velocity.y, "{velocity}");
            // Low: under a metre up.
            assert!(
                velocity.y * velocity.y / (2.0 * GRAVITY) < 1.0,
                "{velocity}"
            );
        }
        // The tread at the bottom of a tire that rolls is still on the ground.
        let (_, still) = fling(0.0, 1.0, moving, back, up, tread);
        assert!(still.length() < 1e-5, "{still}");
        // A tire spun from standing throws it back.
        let spun = tread_speed(0.0, 1.0);
        assert_eq!(spun, SPIN_TREAD);
        let (_, velocity) = fling(0.3, 1.0, Vec3::ZERO, back, up, spun * 0.6);
        assert!(velocity.z > 0.0 && velocity.y > 0.0);
        assert_eq!(tread_speed(100.0, 1.0), MOST_FLING);
    }

    #[test]
    fn a_tiles_colour_is_its_average() {
        let tile = [255, 0, 0, 255, 0, 0, 255, 255];
        assert_eq!(average_color(&tile), Vec3::new(0.5, 0.0, 0.5));
    }

    #[test]
    fn pictures_are_solid_in_the_middle_and_clear_at_the_corners() {
        for opacity in [clod_opacity as fn(Vec2, u32) -> f32, dust_opacity] {
            let texels = picture_texels(16, SHAPES_ACROSS, opacity);
            let side = 16 * SHAPES_ACROSS as usize;
            let alpha = |col: usize, row: usize| texels[(row * side + col) * 4 + 3];
            // Every shape, each in its own square of the picture.
            for (first_col, first_row) in [(0, 0), (16, 0), (0, 16), (16, 16)] {
                let at = |col: usize, row: usize| alpha(first_col + col, first_row + row);
                assert!(at(8, 8) > 200, "{first_col} {first_row}");
                for corner in [(0, 0), (15, 0), (0, 15), (15, 15)] {
                    assert_eq!(at(corner.0, corner.1), 0, "{first_col} {first_row}");
                }
            }
        }
        // A clod is solid most of the way out; dust thins all the way.
        let halfway = Vec2::new(0.5, 0.0);
        assert!(clod_opacity(halfway, 0) > dust_opacity(halfway, 0));
        // The shapes differ.
        let aside = Vec2::new(0.6, 0.0);
        assert!(clod_opacity(aside, 0) > 0.9 && clod_opacity(aside, 1) == 0.0);
        assert_ne!(
            dust_opacity(Vec2::new(0.6, 0.3), 0),
            dust_opacity(Vec2::new(0.6, 0.3), 2)
        );
    }
}
