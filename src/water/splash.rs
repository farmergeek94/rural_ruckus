//! What a truck does to the water it drives through: the spray its tires throw, the splash
//! as a wheel comes down into it, and the ripples it leaves.
//!
//! A tire breaks the surface where the water is partway up its column (see `forces`),
//! neither below it nor over it. Such a tire throws spray while it moves: a rooster tail
//! up and behind, and some out to the side, more the faster it goes. A tire that comes
//! down into the water fast throws a ring of it up all round, and starts a big ripple.
//! And the truck leaves a trail of ripples behind it, one every few metres, which spread
//! and cross into a wake; standing in the water, it laps at it now and then.
//!
//! Spray is droplets of two kinds. Drops are small and heavy: thrown, pulled down by
//! gravity, and drawn longer the faster they go, as the eye sees a fast drop. Mist is fine
//! water that hangs in the air: big, faint puffs, thrown with the drops, that the air soon
//! slows and the wind carries off, and that spread and fade as they go. Mist is what makes
//! spray look like a body of water rather than a scatter of beads. Both are gone when they
//! come down into the water or onto the ground. They are drawn only, and touch nothing.
//!
//! Each is a flat square turned to the camera, with a soft round picture on it. Balls, as
//! they once were, showed their facets, and their hard edges looked like confetti. Drops
//! and mist are two pools of particles (`crate::particles`), which the graphics card moves:
//! here they are only thrown, and the wind given to them.
//!
//! Where the truck is comes from how it is drawn (`TruckVisual`), so that the spray leaves
//! the tires where they are seen. How fast it goes comes from its body.

use avian3d::prelude::{AngularVelocity, LinearVelocity};
use bevy::asset::RenderAssetUsages;
use bevy::ecs::entity::EntityHashMap;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::WaterSettings;
use super::forces::Column;
use super::ripples::{Ripple, Ripples};
use super::shore::swell_height;
use super::wind::Wind;
use crate::game_state::GameState;
use crate::particles::{self, Motion, Particle, Particles};
use crate::track::Track;
use crate::truck::{TruckConfig, TruckVisual};

/// How fast a tire must be going over the ground to throw spray, in m/s.
const SPRAY_SPEED: f32 = 1.5;
/// How many droplets a tire throws each second for each m/s it goes faster than
/// `SPRAY_SPEED`. More is a thicker spray, and costs more.
const SPRAY_PER_SPEED: f32 = 14.0;
/// The most droplets a tire throws each second, however fast it goes.
const MOST_SPRAY: f32 = 320.0;
/// How fast a wheel must come down into the water to splash, in m/s.
const SPLASH_SPEED: f32 = 2.0;
/// How many drops a splash throws: the least, when the wheel only just comes down fast
/// enough to splash, how many more for each m/s faster, and the most.
const LEAST_SPLASH: usize = 30;
const SPLASH_PER_SPEED: f32 = 5.0;
const MOST_SPLASH: usize = 50;
/// How far a splash's drops lean out from straight up, in radians: the least and the most.
/// They go up in a cone, not a flat ring.
const SPLASH_CONE: [f32; 2] = [0.15, 0.7];
/// How many puffs of mist a splash throws, the least and the most, and how big they are,
/// in metres of radius. Wide and slow: the foam of the broken water, hanging over it.
const SPLASH_MIST: [usize; 2] = [3, 5];
const SPLASH_MIST_SIZE: [f32; 2] = [0.6, 1.0];
/// How long a splash's ring lasts on the water, in seconds, how fast it spreads, in m/s, and
/// how far round the wheel it starts, as a share of the wheel's radius. It spreads over the
/// place where the tire went in, and hides the hard line where the tire meets the water.
const RING_LIFE: f32 = 0.8;
const RING_SPREAD: f32 = 2.5;
const RING_START: f32 = 1.2;
/// How many steps a splash's ring fades out in.
const RING_SHADES: usize = 8;
/// The most drops, and the most puffs of mist, in the air at once, over all the trucks and
/// the shore. Past it, no more are thrown until some have fallen. Each is a square in a
/// mesh, drawn whether it is in the air or not.
const DROP_SLOTS: usize = 2000;
const MIST_SLOTS: usize = 800;
/// How big a drop of spray is, in metres: the smallest and the largest radius. Its edge is
/// soft, so it looks a little smaller than this. Big drops read as marbles.
const SPRAY_SIZE: [f32; 2] = [0.04, 0.1];
/// The same for a splash, which throws water up in lumps.
const SPLASH_SIZE: [f32; 2] = [0.08, 0.22];
/// How many puffs of mist a tire throws for each drop of spray, and a splash for each of
/// its drops, from 0 to 1. More is a thicker, whiter spray.
const MIST_PER_SPRAY: f32 = 0.25;
/// How big a puff of mist is when it is thrown, in metres of radius, and how fast it
/// spreads, in metres of radius each second.
const MIST_SIZE: [f32; 2] = [0.25, 0.5];
const MIST_SPREAD: f32 = 0.9;
/// How long a puff of mist lasts, in seconds, and how long of that it stays as it was
/// thrown. It fades out over the rest.
const MIST_LIFE: f32 = 1.1;
const MIST_HOLD: f32 = 0.5;
/// How quickly the air slows mist, as a share of its speed through the air each second,
/// and how fast it falls, in m/s², for it is carried more than it falls.
const MIST_DRAG: f32 = 2.5;
const MIST_GRAVITY: f32 = 2.0;
/// How see-through mist is, from 0 (not there) to 1 (solid), when it is thrown, and a drop.
const MIST_OPACITY: f32 = 0.45;
const DROP_OPACITY: f32 = 0.9;
/// The colour of spray, in sRGB from 0 to 1: white where the light catches it, whichever way
/// it is seen from, and not shaded like a solid thing.
const SPRAY_COLOR: Vec3 = Vec3::new(0.9, 0.95, 1.0);
/// How near the camera a droplet starts to shrink away, in metres. Spray that the truck
/// throws back at the camera would otherwise pass it as big blobs, as if on the lens. Inside
/// it, a droplet also looks smaller the nearer it comes, not only no bigger.
const NEAR_CAMERA: f32 = 6.0;
/// How long the eye, or a camera, sees a drop for, in seconds: a drop is drawn as a streak
/// as long as the way it goes in this time, as a fast drop is seen. A drop at 8 m/s is a
/// streak about 30 cm long. Longer is streakier.
const EXPOSURE: f32 = 0.04;
/// How long a droplet may stay up, in seconds, if it doesn't fall back into the water
/// first. A drop shrinks away over the last third of that: spray landing on dry ground.
const DROPLET_LIFE: f32 = 1.6;
/// How much of its speed through the air a droplet loses each second. The air is moving
/// with the wind, so this is also how quickly the wind carries spray off downwind.
const AIR_DRAG: f32 = 0.6;
const GRAVITY: f32 = 9.81;

/// How far a truck goes between the ripples it leaves, in metres, and how soon after
/// one it may leave the next at the least, in seconds. The two keep a fast truck from
/// using up every ripple the shader has.
const WAKE_SPACING: f32 = 2.0;
const WAKE_INTERVAL: f32 = 0.15;
/// How often a truck standing in the water laps at it, in seconds.
const LAPPING_INTERVAL: f32 = 1.2;
/// How strong a truck's ripples are, in metres: the least, standing, and how much more for
/// each m/s it goes, up to the most.
const WAKE_STRENGTH: [f32; 2] = [0.015, 0.1];
const WAKE_STRENGTH_PER_SPEED: f32 = 0.006;
/// How strong a splash's ripple is for each m/s the wheel came down at, and the most.
const SPLASH_RIPPLE_PER_SPEED: f32 = 0.03;
const MOST_SPLASH_RIPPLE: f32 = 0.25;

/// How drops move: thrown, pulled down, carried by the wind, and drawn as a streak.
const DROP_MOTION: Motion = Motion {
    gravity: GRAVITY,
    drag: AIR_DRAG,
    life: DROPLET_LIFE,
    lands: true,
    spread: 0.0,
    fade_from: DROPLET_LIFE,
    shrink_from: DROPLET_LIFE * 2.0 / 3.0,
    shapes: 1,
    soft: 0.0,
    exposure: EXPOSURE,
    near_camera: NEAR_CAMERA,
};

/// How mist moves: soon slowed, and carried off by the wind as it spreads and fades.
const MIST_MOTION: Motion = Motion {
    gravity: MIST_GRAVITY,
    drag: MIST_DRAG,
    life: MIST_LIFE,
    lands: true,
    spread: MIST_SPREAD,
    fade_from: MIST_HOLD,
    shrink_from: MIST_LIFE,
    shapes: 1,
    soft: 0.0,
    exposure: 0.0,
    near_camera: NEAR_CAMERA,
};

/// What the spray is drawn with: the pictures of a drop and of mist, and for a splash's
/// ring, one square and a material for each step of it fading out, most solid first.
#[derive(Resource)]
pub(super) struct DropletLooks {
    drop: Handle<Image>,
    mist: Handle<Image>,
    square: Handle<Mesh>,
    ring: Vec<Handle<StandardMaterial>>,
}

/// The ring a splash leaves on the water, lying flat on it.
#[derive(Component)]
pub(super) struct SplashRing {
    age: f32,
    /// Its radius when it started, in metres.
    size: f32,
}

/// What a droplet is: see the module's notes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Drop,
    Mist,
}

/// On the pool of drops.
#[derive(Component)]
pub(super) struct Drops;

/// On the pool of mist.
#[derive(Component)]
pub(super) struct Mist;

/// What `make_waves` remembers about a truck from one frame to the next.
#[derive(Default)]
pub(super) struct Wading {
    /// Per wheel: whether its tire was in the water, and the droplets it owes, carried
    /// over as a fraction from frame to frame.
    wheels: [WheelWading; 4],
    /// Where it left its last ripple, and how long ago.
    last_ripple: Option<(Vec2, f32)>,
}

#[derive(Default, Clone, Copy)]
struct WheelWading {
    wet: bool,
    owed: f32,
}

pub(super) fn make_droplet_looks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut material = |picture: Handle<Image>, opacity: f32| {
        materials.add(StandardMaterial {
            base_color: Color::srgba(SPRAY_COLOR.x, SPRAY_COLOR.y, SPRAY_COLOR.z, opacity),
            base_color_texture: Some(picture),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            // A ring lies flat, and is seen from either side as the swell tilts under it.
            cull_mode: None,
            ..default()
        })
    };
    let ring_picture = images.add(picture(|point| round(ring_opacity, point)));
    let ring = (0..RING_SHADES)
        .map(|shade| {
            let left = 1.0 - shade as f32 / RING_SHADES as f32;
            material(ring_picture.clone(), 0.9 * left)
        })
        .collect();
    commands.insert_resource(DropletLooks {
        drop: images.add(picture(drop_opacity)),
        mist: images.add(picture(|point| round(mist_opacity, point))),
        // Two metres across, so that its scale is its radius.
        square: meshes.add(Rectangle::new(2.0, 2.0)),
        ring,
    });
}

/// Makes the race's pools of drops and mist, on a track with water.
pub(super) fn spawn_spray(
    mut commands: Commands,
    track: Res<Track>,
    looks: Res<DropletLooks>,
    wind: Res<Wind>,
    mut assets: particles::PoolAssets,
) {
    if track.water_level.is_none() {
        return;
    }
    // The wind carries the spray.
    let air = wind.air();
    let drops = particles::pool(
        &mut assets,
        DROP_SLOTS,
        DROP_MOTION,
        air,
        looks.drop.clone(),
    );
    commands.spawn((drops, Drops, DespawnOnExit(GameState::Racing)));
    let mist = particles::pool(
        &mut assets,
        MIST_SLOTS,
        MIST_MOTION,
        air,
        looks.mist.clone(),
    );
    commands.spawn((mist, Mist, DespawnOnExit(GameState::Racing)));
}

/// How many texels the droplets' pictures have along a side.
const PICTURE_SIZE: u32 = 32;

/// A white picture, as solid as `opacity` says at each point of it, from -1 to 1 across
/// (X) and up (Y), as it lies on the square: up is the square's +Y.
fn picture(opacity: fn(Vec2) -> f32) -> Image {
    let texels = picture_texels(PICTURE_SIZE, opacity);
    Image::new(
        Extent3d {
            width: PICTURE_SIZE,
            height: PICTURE_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        texels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn picture_texels(size: u32, opacity: fn(Vec2) -> f32) -> Vec<u8> {
    let middle = size as f32 / 2.0;
    let mut texels = Vec::with_capacity((size * size * 4) as usize);
    for row in 0..size {
        for col in 0..size {
            // The picture's first row is at the square's top.
            let point = Vec2::new(col as f32 + 0.5 - middle, middle - row as f32 - 0.5) / middle;
            let alpha = (opacity(point).clamp(0.0, 1.0) * 255.0).round() as u8;
            texels.extend_from_slice(&[255, 255, 255, alpha]);
        }
    }
    texels
}

/// A drop, going up the picture: a round head at the top, and a tail that tapers and fades
/// behind it. Stretched along the way it goes, it is a streak; not stretched, a teardrop.
fn drop_opacity(point: Vec2) -> f32 {
    // From 0 at the tail's end to 1 at the top.
    let along = ((point.y + 1.0) / 2.0).clamp(0.0, 1.0);
    let mut width = 0.8 * along.sqrt();
    // The head rounds off over the top fifth.
    if along > 0.8 {
        let over = (along - 0.8) / 0.2;
        width *= (1.0 - over * over).max(0.0).sqrt();
    }
    if width <= 0.0 {
        return 0.0;
    }
    let edge = 1.0 - smoothstep(0.5 * width, width, point.x.abs());
    // Softly, at the very top, where the head's width comes down to nothing.
    let top = 1.0 - smoothstep(0.93, 1.0, along);
    edge * top * (0.3 + 0.7 * along)
}

/// A picture that is the same all round: `opacity` at each distance from its middle, as a
/// share of the way to its edge.
fn round(opacity: fn(f32) -> f32, point: Vec2) -> f32 {
    opacity(point.length().min(1.0))
}

/// Mist: thickest in the middle and thinning all the way out.
fn mist_opacity(out: f32) -> f32 {
    let thinning = 1.0 - out * out;
    thinning * thinning
}

/// A splash's ring: a band of foam near its edge, and thinner foam inside it.
fn ring_opacity(out: f32) -> f32 {
    let band = (out - 0.75) / 0.12;
    ((-band * band).exp() + 0.25 * (1.0 - smoothstep(0.5, 0.8, out)))
        * (1.0 - smoothstep(0.9, 1.0, out))
}

fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn make_waves(
    mut commands: Commands,
    time: Res<Time>,
    track: Res<Track>,
    looks: Res<DropletLooks>,
    wind: Res<Wind>,
    settings: Res<WaterSettings>,
    trucks: Query<(Entity, &Transform, &TruckVisual)>,
    bodies: Query<(&LinearVelocity, &AngularVelocity, &TruckConfig)>,
    mut drops: Query<&mut Particles, (With<Drops>, Without<Mist>)>,
    mut mist: Query<&mut Particles, (With<Mist>, Without<Drops>)>,
    mut ripples: ResMut<Ripples>,
    mut wading: Local<EntityHashMap<Wading>>,
    mut random: Local<Random>,
) {
    let Some(level) = track.water_level else {
        return;
    };
    let (Ok(mut drops), Ok(mut mists)) = (drops.single_mut(), mist.single_mut()) else {
        return;
    };
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let clock = particles::clock(&time);
    let floor = |at: Vec2| level.max(track.heights.height_at(at.x, at.y));
    // A droplet of `kind`, and now and then as `mist` says, a puff of mist with it. With
    // splashes off, none: the ripples and the wake go on.
    let mut throw = |kind: Kind,
                     at: Vec3,
                     velocity: Vec3,
                     [smallest, largest]: [f32; 2],
                     mist: f32,
                     random: &mut Random| {
        if !settings.splashes {
            return;
        }
        let size = random.between(smallest, largest);
        let pool = match kind {
            Kind::Drop => &mut *drops,
            Kind::Mist => &mut *mists,
        };
        if !throw_droplet(pool, kind, clock, floor, at, velocity, size) {
            return;
        }
        if random.next() < mist {
            let size = random.between(MIST_SIZE[0], MIST_SIZE[1]);
            throw_droplet(
                &mut mists,
                Kind::Mist,
                clock,
                floor,
                at,
                velocity * 0.6,
                size,
            );
        }
    };

    // Trucks from a race that is over are forgotten.
    wading.retain(|visual, _| trucks.contains(*visual));

    for (visual, transform, truck) in &trucks {
        let Ok((linear, angular, config)) = bodies.get(truck.truck) else {
            continue;
        };
        let state = wading.entry(visual).or_default();
        let up = transform.up().as_vec3();
        let right = transform.right().as_vec3();
        let mut breaking = Vec::new();
        for (index, rest) in config.wheel_rest.iter().enumerate() {
            let hub = transform.transform_point(*rest);
            let column = Column::over(hub, up, config.wheel_radius);
            let wheel = &mut state.wheels[index];
            let was_wet = wheel.wet;
            wheel.wet = breaks_the_surface(&column, level);
            if !wheel.wet {
                wheel.owed = 0.0;
                continue;
            }
            // Where the tire meets the water, on the outside of the tire.
            let outwards = right * rest.x.signum();
            let at_surface = Vec3::new(hub.x, level, hub.z) + outwards * config.wheel_width / 2.0;
            breaking.push(at_surface.xz());
            let moving = linear.0 + angular.0.cross(hub - transform.translation);

            if !was_wet && -moving.y > SPLASH_SPEED {
                let down = -moving.y;
                let along = moving.with_y(0.0) * 0.3;
                for _ in 0..splash_droplets(down) {
                    let velocity = crown_velocity(&mut random, down, outwards) + along;
                    throw(
                        Kind::Drop,
                        at_surface,
                        velocity,
                        SPLASH_SIZE,
                        0.0,
                        &mut random,
                    );
                }
                let puffs = random.between(SPLASH_MIST[0] as f32, SPLASH_MIST[1] as f32 + 1.0);
                for _ in 0..puffs as usize {
                    // Slow, and only a little up and out.
                    let velocity = crown_velocity(&mut random, down, outwards) * 0.2 + along;
                    throw(
                        Kind::Mist,
                        at_surface,
                        velocity,
                        SPLASH_MIST_SIZE,
                        0.0,
                        &mut random,
                    );
                }
                if settings.splashes {
                    // On the swell where it is drawn, as `water.wgsl` moves it.
                    let swell = swell_height(hub.xz(), time.elapsed_secs_wrapped(), wind.towards);
                    commands.spawn((
                        SplashRing {
                            age: 0.0,
                            size: config.wheel_radius * RING_START,
                        },
                        Transform::from_xyz(hub.x, level + swell, hub.z)
                            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                            .with_scale(Vec3::splat(config.wheel_radius * RING_START)),
                        Mesh3d(looks.square.clone()),
                        MeshMaterial3d(looks.ring[0].clone()),
                        NotShadowCaster,
                        DespawnOnExit(GameState::Racing),
                    ));
                }
                ripples.add(Ripple {
                    center: hub.xz(),
                    born: now,
                    born_on_clock: clock,
                    strength: (down * SPLASH_RIPPLE_PER_SPEED).min(MOST_SPLASH_RIPPLE),
                });
            }

            let ground_speed = moving.xz().length();
            wheel.owed += spray_rate(ground_speed) * dt;
            while wheel.owed >= 1.0 {
                wheel.owed -= 1.0;
                let velocity = spray_velocity(&mut random, moving, outwards);
                throw(
                    Kind::Drop,
                    at_surface,
                    velocity,
                    SPRAY_SIZE,
                    MIST_PER_SPRAY,
                    &mut random,
                );
            }
        }

        if breaking.is_empty() {
            state.last_ripple = None;
            continue;
        }
        let center = breaking.iter().sum::<Vec2>() / breaking.len() as f32;
        let speed = linear.0.xz().length();
        let due = match state.last_ripple {
            None => true,
            Some((last, when)) => ripple_due(center.distance(last), now - when),
        };
        if due {
            ripples.add(Ripple {
                center,
                born: now,
                born_on_clock: clock,
                strength: wake_strength(speed),
            });
            state.last_ripple = Some((center, now));
        }
    }
}

/// Throws one droplet of `kind` into `pool`, `size` metres in radius, from `at`, going at
/// `velocity`, `now` (`particles::clock`). It falls until it goes below `floor`, the water
/// or the ground under a point. Returns whether there was room for it.
pub(super) fn throw_droplet(
    pool: &mut Particles,
    kind: Kind,
    now: f32,
    floor: impl Fn(Vec2) -> f32,
    at: Vec3,
    velocity: Vec3,
    size: f32,
) -> bool {
    let opacity = match kind {
        Kind::Drop => DROP_OPACITY,
        Kind::Mist => MIST_OPACITY,
    };
    let droplet = Particle {
        at,
        velocity,
        size,
        turned: 0.0,
        turning: 0.0,
        color: Color::srgba(SPRAY_COLOR.x, SPRAY_COLOR.y, SPRAY_COLOR.z, opacity).into(),
        trail: Vec3::ZERO,
        shape: 0,
    };
    pool.throw(now, droplet, floor)
}

/// Which of `shades` steps something that fades out over `life` seconds is at, `age`
/// seconds in.
fn shade(age: f32, life: f32, shades: usize) -> usize {
    ((age.max(0.0) / life * shades as f32) as usize).min(shades - 1)
}

/// Spreads the rings that splashes leave, and fades them out.
pub(super) fn spread_rings(
    mut commands: Commands,
    time: Res<Time>,
    looks: Res<DropletLooks>,
    mut rings: Query<(
        Entity,
        &mut SplashRing,
        &mut Transform,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    for (entity, mut ring, mut transform, mut material) in &mut rings {
        ring.age += time.delta_secs();
        if ring.age >= RING_LIFE {
            commands.entity(entity).despawn();
            continue;
        }
        transform.scale = Vec3::splat(ring.size + RING_SPREAD * ring.age);
        let shade = &looks.ring[shade(ring.age, RING_LIFE, RING_SHADES)];
        if material.0 != *shade {
            material.0 = shade.clone();
        }
    }
}

/// Whether the water is partway up this column: the tire is in it, and the top is out.
fn breaks_the_surface(column: &Column, level: f32) -> bool {
    column.under(level).is_some_and(|depth| depth < 1.0)
}

/// How many droplets a tire throws each second, going over the ground at `speed` m/s.
fn spray_rate(speed: f32) -> f32 {
    ((speed - SPRAY_SPEED).max(0.0) * SPRAY_PER_SPEED).min(MOST_SPRAY)
}

/// How many droplets a wheel throws coming down into the water at `speed` m/s.
fn splash_droplets(speed: f32) -> usize {
    let more = ((speed - SPLASH_SPEED).max(0.0) * SPLASH_PER_SPEED) as usize;
    (LEAST_SPLASH + more).min(MOST_SPLASH)
}

/// Whether a truck that has gone `travelled` metres in `since` seconds since its last
/// ripple leaves another.
fn ripple_due(travelled: f32, since: f32) -> bool {
    (travelled >= WAKE_SPACING && since >= WAKE_INTERVAL) || since >= LAPPING_INTERVAL
}

/// How strong a ripple a truck going at `speed` m/s leaves.
fn wake_strength(speed: f32) -> f32 {
    (WAKE_STRENGTH[0] + speed * WAKE_STRENGTH_PER_SPEED).min(WAKE_STRENGTH[1])
}

/// How much of a tire's spray goes up off its tread as a rooster tail, from 0 to 1. The rest
/// goes out to the side as a bow wave.
const ROOSTER_SHARE: f32 = 0.6;
/// How fast the rooster tail leaves the tread, for each m/s the tire goes, straight up: the
/// least and the most. The tread comes up off the water at the tire's own speed, and throws
/// it up behind. At 0.21 to 0.46, a truck at 10 m/s throws it a quarter of a metre to a
/// metre up. How high goes as the square of this: 0.7 times it is half as high.
const ROOSTER_LIFT: [f32; 2] = [0.21, 0.46];

/// How fast a droplet of spray leaves a tire that is going at `moving`, with `outwards`
/// pointing away from the truck's side. Either up off the tread, on with the truck at a
/// little of its speed, so that it hangs behind as a rooster tail; or out to the side as a
/// bow wave, lower, and on with the truck at more of it.
fn spray_velocity(random: &mut Random, moving: Vec3, outwards: Vec3) -> Vec3 {
    let along = moving.with_y(0.0);
    let speed = along.length();
    if random.next() < ROOSTER_SHARE {
        along * random.between(0.1, 0.5)
            + outwards * random.between(0.0, 0.6)
            + Vec3::Y
                * (random.between(0.7, 1.05)
                    + speed * random.between(ROOSTER_LIFT[0], ROOSTER_LIFT[1]))
    } else {
        along * random.between(0.2, 0.8)
            + outwards * random.between(0.3, 1.0) * (1.0 + 0.15 * speed)
            + Vec3::Y * (random.between(0.7, 1.4) + speed * random.between(0.06, 0.16))
    }
}

/// How far round from straight out a splash's droplets may go, in radians either way. The
/// water that would go the other way is under the truck.
const SPLASH_SPREAD: f32 = 1.8;

/// How fast a drop of a splash leaves the water, round a wheel that came down at `speed`
/// m/s, with `outwards` pointing away from the truck's side: up, in a cone that leans out
/// away from the truck, and about as fast as the wheel came down.
fn crown_velocity(random: &mut Random, speed: f32, outwards: Vec3) -> Vec3 {
    let turn = Quat::from_rotation_y(random.between(-SPLASH_SPREAD, SPLASH_SPREAD));
    let across = (turn * outwards.with_y(0.0)).normalize_or_zero();
    let lean = random.between(SPLASH_CONE[0], SPLASH_CONE[1]);
    (across * lean.sin() + Vec3::Y * lean.cos()) * speed * random.between(0.6, 1.1)
}

/// A small, quick source of numbers that only have to look random (xorshift).
pub(super) struct Random(u64);

impl Default for Random {
    fn default() -> Self {
        Self(0x9E37_79B9_7F4A_7C15)
    }
}

impl Random {
    /// A number from 0 up to 1.
    pub(super) fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    pub(super) fn between(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(bottom: f32, top: f32) -> Column {
        Column {
            bottom: Vec3::Y * bottom,
            top: Vec3::Y * top,
        }
    }

    #[test]
    fn only_a_tire_at_the_surface_breaks_it() {
        assert!(!breaks_the_surface(&column(1.0, 3.0), 0.5));
        assert!(breaks_the_surface(&column(1.0, 3.0), 1.5));
        // Wholly under, it is below the surface, not breaking it.
        assert!(!breaks_the_surface(&column(1.0, 3.0), 4.0));
    }

    #[test]
    fn faster_throws_more_spray_up_to_a_limit() {
        assert_eq!(spray_rate(0.0), 0.0);
        assert_eq!(spray_rate(SPRAY_SPEED), 0.0);
        assert!(spray_rate(10.0) > spray_rate(5.0));
        assert_eq!(spray_rate(1000.0), MOST_SPRAY);
    }

    #[test]
    fn a_harder_landing_splashes_more_up_to_a_limit() {
        assert_eq!(splash_droplets(SPLASH_SPEED), LEAST_SPLASH);
        assert!(splash_droplets(6.0) > splash_droplets(3.0));
        assert_eq!(splash_droplets(1000.0), MOST_SPLASH);
    }

    #[test]
    fn a_truck_leaves_ripples_by_distance_and_laps_when_still() {
        assert!(!ripple_due(0.5, 0.2));
        assert!(ripple_due(WAKE_SPACING, 0.2));
        // However far it has gone, not too soon after the last.
        assert!(!ripple_due(50.0, WAKE_INTERVAL / 2.0));
        assert!(ripple_due(0.0, LAPPING_INTERVAL));
    }

    #[test]
    fn a_faster_wake_is_stronger_up_to_a_limit() {
        assert_eq!(wake_strength(0.0), WAKE_STRENGTH[0]);
        assert!(wake_strength(10.0) > wake_strength(2.0));
        assert_eq!(wake_strength(1000.0), WAKE_STRENGTH[1]);
    }

    #[test]
    fn spray_goes_up_out_and_along() {
        let mut random = Random::default();
        let moving = Vec3::new(0.0, 0.0, -15.0);
        for _ in 0..100 {
            let velocity = spray_velocity(&mut random, moving, Vec3::X);
            assert!(velocity.y > 0.0);
            assert!(velocity.x >= 0.0);
            // Slower than the truck, so that it falls behind.
            assert!(velocity.z < 0.0 && velocity.z > moving.z);
        }
    }

    #[test]
    fn a_faster_tire_throws_its_rooster_tail_higher() {
        let mut random = Random::default();
        let highest = |speed: f32, random: &mut Random| {
            (0..200)
                .map(|_| spray_velocity(random, Vec3::new(0.0, 0.0, -speed), Vec3::X).y)
                .fold(0.0, f32::max)
        };
        let slow = highest(3.0, &mut random);
        let fast = highest(15.0, &mut random);
        assert!(fast > slow * 2.0, "{slow} {fast}");
        // Up off the tread at most of the tire's speed.
        assert!(fast > 15.0 * ROOSTER_LIFT[0], "{fast}");
    }

    #[test]
    fn a_splash_goes_up_and_away_from_the_truck() {
        let mut random = Random::default();
        for _ in 0..100 {
            let velocity = crown_velocity(&mut random, 5.0, Vec3::X);
            assert!(velocity.y > 0.0);
            // Nowhere near straight back in under the truck.
            assert!(velocity.with_y(0.0).normalize().x > SPLASH_SPREAD.cos() - 1e-4);
            // Up, in a cone.
            let lean = velocity.angle_between(Vec3::Y);
            assert!(
                (SPLASH_CONE[0] - 1e-4..=SPLASH_CONE[1] + 1e-4).contains(&lean),
                "{lean}"
            );
        }
    }

    #[test]
    fn droplet_pictures_are_solid_in_the_middle_and_clear_at_the_edge() {
        let texels = picture_texels(16, |point| round(mist_opacity, point));
        let alpha = |col: usize, row: usize| texels[(row * 16 + col) * 4 + 3];
        assert!(alpha(8, 8) > 200);
        assert_eq!(alpha(0, 0), 0);
        // The edge texel's middle is just inside the circle.
        assert!(alpha(0, 8) < 16, "{}", alpha(0, 8));
        assert!(alpha(8, 8) > alpha(12, 8) && alpha(12, 8) >= alpha(15, 8));
        // A ring is a band near its edge, clearer inside it.
        assert!(ring_opacity(0.75) > ring_opacity(0.3));
        assert!(ring_opacity(0.3) > 0.1);
        assert!(ring_opacity(1.0) < 1e-3);
    }

    #[test]
    fn a_drop_is_a_head_with_a_tail() {
        // Solid in its head, near the top, and clear off to the sides.
        assert!(drop_opacity(Vec2::new(0.0, 0.5)) > 0.8);
        assert_eq!(drop_opacity(Vec2::new(0.95, 0.5)), 0.0);
        // Its tail is fainter and thinner.
        assert!(drop_opacity(Vec2::new(0.0, -0.6)) < drop_opacity(Vec2::new(0.0, 0.5)));
        assert!(drop_opacity(Vec2::new(0.3, -0.6)) < drop_opacity(Vec2::new(0.3, 0.5)));
        // Nothing past its ends.
        assert!(drop_opacity(Vec2::new(0.0, 1.0)) < 1e-3);
        assert!(drop_opacity(Vec2::new(0.0, -1.0)) < 1e-3);
    }

    #[test]
    fn random_numbers_stay_in_range_and_vary() {
        let mut random = Random::default();
        let numbers: Vec<f32> = (0..1000).map(|_| random.next()).collect();
        assert!(numbers.iter().all(|n| (0.0..1.0).contains(n)));
        let mean = numbers.iter().sum::<f32>() / numbers.len() as f32;
        assert!((mean - 0.5).abs() < 0.05, "{mean}");
    }
}
