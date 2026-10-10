//! The froth that trucks churn up as they go through the water, and the splash as a wheel
//! comes down into it.
//!
//! Where a tire breaks the surface, and whether it came down into it fast, the `water`
//! slice says (`water::TireInWater`). The water a tire pushes aside is in the water's
//! surface itself (`water::field`): the bow wave, the wake and the foam it leaves. What
//! stands up out of the water round the tire is froth: white lumps of broken water, piled
//! up at the front of the tire and round its sides where they meet the water, which go
//! along with the tire, rise a little, and fall behind into its wake. More the faster the
//! tire goes, or its tread spins. Most where the water is a quarter of the way up the tire:
//! little where it only wets the bottom, and less where it is over the hub. A tire that
//! comes down into the water fast throws a crown of froth out from all round its footprint,
//! low and wide rather than up like a fountain, and leaves a ring of foam that spreads on
//! the water. Over it all hangs mist: fine water off the tires' pushing fronts, more the
//! faster they go, that the wind carries off. Drops thrown off the tires were tried and
//! taken out: foam on the water, froth and mist read as water; a scatter of drops read as
//! beads.
//!
//! The froth, the mist and the foam are each a pool of particles (`pool`) that the
//! graphics card moves. Here they are only thrown, and given the wind (`water::Wind`) and
//! the swell (`water::Wind::swell`). One more pool, of drops, is the shore's (`surf`), made
//! here with the picture for it: drops are small and heavy, thrown, pulled down by
//! gravity, and drawn longer the faster they go, as the eye sees a fast drop; mist is fine
//! water that hangs in the air, big, faint puffs that the air soon slows and the wind
//! carries off. Both are gone when they come down into the water or onto the ground. Mist
//! fades out a little where a truck or the ground cuts through it (`Motion::soft`); drops
//! and froth hardly do, since a puff that faded wherever a tire was behind it was gone
//! from round the tires, where it is churned up. They are drawn only, and touch nothing.
//!
//! Each is a flat square turned to the camera, with a soft round picture on it. Balls, as
//! they once were, showed their facets, and their hard edges looked like confetti.
//!
//! Foam is the broken water a splash leaves round where the wheel came down: flat, ragged
//! patches of bubbles that lie on the water, rise and fall with its swell, spread, drift a
//! little downwind, and fade. Past the patch of water round the camera that the swell
//! moves (see `water::surface`), the water is drawn flat and the foam still rises and
//! falls: too far off to see.

use bevy::asset::RenderAssetUsages;
use bevy::ecs::entity::EntityHashMap;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::pool::{self, Air, Motion, Particle, Particles, Waves};
use crate::game_state::GameState;
use crate::track::Track;
use crate::truck::TruckVisual;
use crate::water::{SPLASH_SPEED, TireInWater, WATER_COLOR, WaterSettings, Wind};

/// How far ahead of and behind the hub the crown of a splash leaves the water, as a share
/// of the wheel's radius, and how many lumps of froth go out with it, at the least, and
/// how many more for each m/s the wheel came down at faster than `water::SPLASH_SPEED`.
const CROWN_ALONG: f32 = 0.8;
const SPLASH_FROTH: usize = 14;
const SPLASH_FROTH_PER_SPEED: f32 = 3.0;
/// How long a splash's ring lasts on the water, in seconds, how fast it spreads, in m/s, and
/// how far round the wheel it starts, as a share of the wheel's radius. It spreads over the
/// place where the tire went in, and hides the hard line where the tire meets the water.
const RING_LIFE: f32 = 0.8;
const RING_SPREAD: f32 = 2.5;
const RING_START: f32 = 1.2;
/// How many steps a splash's ring fades out in.
const RING_SHADES: usize = 8;
/// The most drops in the air at once, over the shore, and the most puffs of mist, over the
/// trucks and the shore. Past it, no more are thrown until some have fallen. Each is a
/// square in a mesh, drawn whether it is in the air or not.
const DROP_SLOTS: usize = 2000;
const MIST_SLOTS: usize = 1500;
/// How many patches of foam may lie on the water at once, over all the trucks.
const FOAM_SLOTS: usize = 1000;
/// How many puffs of mist a tire throws each second for each m/s it goes over the ground,
/// and the most; how big a puff is when it is thrown, in metres of radius, the smallest
/// and the largest; how fast it goes up, in m/s, the least and the most, in deep water;
/// how much of the tire's own speed it keeps; and how fast it spreads, in metres of
/// radius each second. How many puffs a wheel coming down throws, the least and the most.
const MIST_PER_SPEED: f32 = 5.0;
const MOST_MIST: f32 = 40.0;
const MIST_SIZE: [f32; 2] = [0.3, 0.6];
const MIST_UP: [f32; 2] = [0.8, 2.5];
const MIST_KEEP: f32 = 0.6;
const MIST_SPREAD: f32 = 0.9;
const SPLASH_MIST: [usize; 2] = [4, 7];
/// How long a puff of mist lasts, in seconds, and how long of that it stays as it was
/// thrown. It fades out over the rest.
const MIST_LIFE: f32 = 1.1;
const MIST_HOLD: f32 = 0.5;
/// How quickly the air slows mist, as a share of its speed through the air each second,
/// and how fast it falls, in m/s², for it is carried more than it falls.
const MIST_DRAG: f32 = 2.5;
const MIST_GRAVITY: f32 = 2.0;
/// How see-through mist is, from 0 (not there) to 1 (solid), when it is thrown, and a drop.
const MIST_OPACITY: f32 = 0.32;
const DROP_OPACITY: f32 = 0.7;
/// How near a truck or the ground behind it mist starts to fade out, in metres, so that a
/// puff cut through by them has no hard edge, and a drop, which is cut off: it is small,
/// and faded out wherever a tire was just behind it, it was gone from round the tires.
const MIST_SOFT: f32 = 0.2;
const DROP_SOFT: f32 = 0.0;

/// The churn round a tire that goes through the water: white froth, piled up at the front
/// of the tire and round its sides where they meet the water, which goes along with the
/// tire, rises a little, and falls behind into its wake. It is what a vehicle driven
/// through deep water is wrapped in.
///
/// How many puffs a tire churns up each second, for each m/s its tread goes (over the
/// ground, or faster where it spins), and the most. Many big ones close together run into
/// one mass of froth.
const FROTH_PER_SPEED: f32 = 14.0;
const MOST_FROTH: f32 = 160.0;
/// How many puffs of froth there may be at once, over all the trucks.
const FROTH_SLOTS: usize = 3000;
/// How big a puff of froth is when it is churned up, in metres of radius, how fast it
/// spreads, in metres of radius each second, and how long it lasts, in seconds, and holds.
const FROTH_SIZE: [f32; 2] = [0.3, 0.55];
const FROTH_SPREAD: f32 = 0.6;
const FROTH_LIFE: f32 = 0.5;
const FROTH_HOLD: f32 = 0.25;
/// How much of its speed through the air froth loses each second: more than a drop, so
/// that it falls behind the tire into its wake.
const FROTH_DRAG: f32 = 1.0;
/// How fast froth goes up, in m/s: however slow the tire, and for each m/s more its tread
/// goes, up to `MOST_PUSH`. At 8 m/s, it rises about 0.4 m, in deep water.
const FROTH_UP: f32 = 0.8;
const FROTH_UP_PER_SPEED: f32 = 0.25;
/// How fast froth is pushed away from the tire, in m/s: the least and the most.
const FROTH_OUT: [f32; 2] = [0.3, 1.2];
/// How much of the tire's speed froth goes on at, the least and the most: about all of it,
/// so that it stays round the tire.
const FROTH_KEEP: [f32; 2] = [0.85, 1.0];
/// How far round from the front of the tire, either way, froth is churned up, in radians:
/// the front and the sides, where the tire pushes the water.
const FROTH_ROUND: f32 = 1.4;
/// How far out from the tire's round at the water froth starts, in metres.
const FROTH_GAP: f32 = 0.1;
/// How see-through froth is, from 0 (not there) to 1 (solid), and how near what is behind
/// it it starts to fade out, in metres: little, since froth is churned up right against
/// the tires, and faded out there it was gone.
const FROTH_OPACITY: f32 = 0.95;
const FROTH_SOFT: f32 = 0.08;

/// How many patches of foam a splash leaves round where the wheel came down.
const SPLASH_FOAM: usize = 6;
/// How big a patch of foam is when it is left, in metres of radius, and how fast it
/// spreads, in metres of radius each second. A patch is flat: a big one on a curved swell
/// has the water come up through its middle.
const FOAM_SIZE: [f32; 2] = [0.45, 0.8];
const FOAM_SPREAD: f32 = 0.35;
/// How long a patch of foam lasts, in seconds, and how long of that it stays as it was
/// left. It fades out over the rest.
const FOAM_LIFE: f32 = 4.0;
const FOAM_HOLD: f32 = 1.0;
/// How quickly the water slows foam, as a share of its speed each second.
const FOAM_DRAG: f32 = 1.5;
/// How much of the wind's speed the foam drifts at. Foam lies in the water, which the wind
/// only drags at.
const FOAM_DRIFT: f32 = 0.03;
/// How high over the water's surface foam lies, in metres. The surface is drawn in flat
/// triangles 1.5 m across, which cut a little under the waves' curve, and a patch is flat
/// across its own width: lower, and the water cuts it into diamonds along the triangles'
/// edges where the swell is high.
const FOAM_LIFT: f32 = 0.1;
/// How see-through foam is, from 0 (not there) to 1 (solid), when it is left.
const FOAM_OPACITY: f32 = 0.85;
/// How many pictures of foam along each side of its picture: each patch is one of four, at
/// random, so that not all look alike.
const FOAM_SHAPES: u32 = 2;
/// The colour of spray, in sRGB from 0 to 1: white where the light catches it, whichever way
/// it is seen from, and not shaded like a solid thing. A droplet is this colour at its
/// edge, where it thins out.
const SPRAY_COLOR: Vec3 = Vec3::new(0.9, 0.95, 1.0);
/// How much of the water's own deep blue (`WATER_COLOR`) the spray takes where it is
/// thick, in the middle of a droplet and in a splash's ring, from 0 (white) to 1 (as blue
/// as the water), so that it looks to be of the same water. Higher is bluer and darker.
const SPRAY_BLUE: f32 = 0.4;

/// Thick spray's colour, as see-through as `opacity` (from 0 to 1).
fn spray_color(opacity: f32) -> Color {
    spray_edge()
        .mix(&WATER_COLOR, SPRAY_BLUE)
        .with_alpha(opacity)
}

/// Thin spray's colour, at the edge of a droplet.
fn spray_edge() -> Color {
    Color::srgb(SPRAY_COLOR.x, SPRAY_COLOR.y, SPRAY_COLOR.z)
}
/// How near the camera a droplet starts to shrink away, in metres. Spray that the truck
/// throws back at the camera would otherwise pass it as big blobs, as if on the lens. Inside
/// it, a droplet also looks smaller the nearer it comes, not only no bigger.
const NEAR_CAMERA: f32 = 6.0;
/// How long the eye, or a camera, sees a drop for, in seconds: a drop is drawn as a streak
/// as long as the way it goes in this time, as a fast drop is seen. A drop at 8 m/s is a
/// streak about 35 cm long. Longer is streakier, and runs the drops of a rooster tail
/// together, but a streak is drawn thinner the longer it is.
const EXPOSURE: f32 = 0.045;
/// How long a droplet may stay up, in seconds, if it doesn't fall back into the water
/// first. A drop shrinks away over the last third of that: spray landing on dry ground.
const DROPLET_LIFE: f32 = 1.6;
/// How much of its speed through the air a droplet loses each second. The air is moving
/// with the wind, so this is also how quickly the wind carries spray off downwind.
const AIR_DRAG: f32 = 0.6;
const GRAVITY: f32 = 9.81;

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
    soft: DROP_SOFT,
    exposure: EXPOSURE,
    near_camera: NEAR_CAMERA,
    lies_on: None,
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
    soft: MIST_SOFT,
    exposure: 0.0,
    near_camera: NEAR_CAMERA,
    lies_on: None,
};

/// How froth moves: along with the tire, up a little, and down, as it spreads and fades.
const FROTH_MOTION: Motion = Motion {
    gravity: GRAVITY,
    drag: FROTH_DRAG,
    life: FROTH_LIFE,
    lands: true,
    spread: FROTH_SPREAD,
    fade_from: FROTH_HOLD,
    shrink_from: FROTH_LIFE,
    shapes: FOAM_SHAPES,
    soft: FROTH_SOFT,
    exposure: 0.0,
    near_camera: NEAR_CAMERA,
    lies_on: None,
};

/// How foam moves: on the water, slowed by it, and drifting a little with the wind, as it
/// spreads and fades. Which waves it lies on is given when its pool is made.
const FOAM_MOTION: Motion = Motion {
    gravity: 0.0,
    drag: FOAM_DRAG,
    life: FOAM_LIFE,
    lands: false,
    spread: FOAM_SPREAD,
    fade_from: FOAM_HOLD,
    shrink_from: FOAM_LIFE,
    shapes: FOAM_SHAPES,
    soft: 0.0,
    exposure: 0.0,
    // Lying flat, it is never a blob over the camera.
    near_camera: 0.5,
    lies_on: Some(Waves::FLAT),
};

/// What the spray is drawn with: the pictures of a drop, of mist and of foam, and for a
/// splash's ring, one square and a material for each step of it fading out, most solid
/// first.
#[derive(Resource)]
pub(super) struct DropletLooks {
    drop: Handle<Image>,
    mist: Handle<Image>,
    foam: Handle<Image>,
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

/// On the pool of foam.
#[derive(Component)]
pub(super) struct Foam;

/// On the pool of froth.
#[derive(Component)]
pub(super) struct Froth;

/// What `throw_spray` remembers about a truck from one frame to the next: per wheel, the
/// puffs of froth it owes, carried over as a fraction.
#[derive(Default)]
pub(super) struct Owed {
    froth: [f32; 4],
    mist: [f32; 4],
}

pub(super) fn make_droplet_looks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut material = |picture: Handle<Image>, opacity: f32| {
        materials.add(StandardMaterial {
            base_color: spray_color(opacity),
            base_color_texture: Some(picture),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            // A ring lies flat, and is seen from either side as the swell tilts under it.
            cull_mode: None,
            ..default()
        })
    };
    // The ring takes its blue from its material, so its picture is white throughout.
    let ring_picture = images.add(picture(
        |point| round(ring_opacity, point),
        Color::WHITE,
        Color::WHITE,
    ));
    let ring = (0..RING_SHADES)
        .map(|shade| {
            let left = 1.0 - shade as f32 / RING_SHADES as f32;
            material(ring_picture.clone(), 0.9 * left)
        })
        .collect();
    commands.insert_resource(DropletLooks {
        // A droplet is blue where it is thick, and white where it thins out at its edge.
        drop: images.add(picture(drop_opacity, spray_color(1.0), spray_edge())),
        mist: images.add(picture(
            |point| round(mist_opacity, point),
            spray_color(1.0),
            spray_edge(),
        )),
        foam: images.add(super::dirt::picture(FOAM_SHAPES, foam_opacity)),
        // Two metres across, so that its scale is its radius.
        square: meshes.add(Rectangle::new(2.0, 2.0)),
        ring,
    });
}

/// Makes the race's pools of drops, mist and foam, on a track with water.
pub(super) fn spawn_spray(
    mut commands: Commands,
    track: Res<Track>,
    looks: Res<DropletLooks>,
    wind: Option<Res<Wind>>,
    settings: Option<Res<WaterSettings>>,
    mut assets: pool::PoolAssets,
) {
    if track.water_level.is_none() {
        return;
    }
    // The wind carries the spray.
    let air = wind.as_deref().map_or(Air::STILL, air);
    let drops = pool::pool(
        &mut assets,
        DROP_SLOTS,
        DROP_MOTION,
        air,
        looks.drop.clone(),
    );
    commands.spawn((drops, Drops, DespawnOnExit(GameState::Racing)));
    let mist = pool::pool(
        &mut assets,
        MIST_SLOTS,
        MIST_MOTION,
        air,
        looks.mist.clone(),
    );
    commands.spawn((mist, Mist, DespawnOnExit(GameState::Racing)));
    let foam = pool::pool(
        &mut assets,
        FOAM_SLOTS,
        Motion {
            lies_on: Some(swell(wind.as_deref(), settings.as_deref())),
            ..FOAM_MOTION
        },
        Air {
            steady: air.steady * FOAM_DRIFT,
            gusts: air.gusts * FOAM_DRIFT,
            ..air
        },
        looks.foam.clone(),
    );
    commands.spawn((foam, Foam, DespawnOnExit(GameState::Racing)));
    // The same pictures as the foam: ragged lumps of bubbles.
    let froth = pool::pool(
        &mut assets,
        FROTH_SLOTS,
        FROTH_MOTION,
        air,
        looks.foam.clone(),
    );
    commands.spawn((froth, Froth, DespawnOnExit(GameState::Racing)));
}

/// The waves the water's surface is drawn with, for the foam to lie on: none where it is
/// drawn flat, or without the water's wind.
fn swell(wind: Option<&Wind>, settings: Option<&WaterSettings>) -> Waves {
    match wind {
        Some(wind) if !settings.is_some_and(|settings| settings.flat) => {
            let waves = wind.swell();
            Waves {
                travel: waves.map(|wave| wave.travel),
                rates: waves.map(|wave| wave.rate),
                heights: waves.map(|wave| wave.height),
            }
        }
        _ => Waves::FLAT,
    }
}

/// Keeps the foam on the water as it is drawn, when it is changed to be drawn flat or with
/// waves.
pub(super) fn keep_foam_on_the_water(
    wind: Option<Res<Wind>>,
    settings: Option<Res<WaterSettings>>,
    mut foam: Query<&mut Particles, With<Foam>>,
    mut materials: ResMut<Assets<pool::ParticleMaterial>>,
) {
    let waves = swell(wind.as_deref(), settings.as_deref());
    for mut foam in &mut foam {
        foam.lie_on(waves, &mut materials);
    }
}

/// The air that `wind` moves, gusts and all, as the shader works it out.
fn air(wind: &Wind) -> Air {
    Air {
        steady: wind.steady(),
        gusts: wind.gusts(),
        periods: Wind::GUST_PERIODS,
        weights: Wind::GUST_WEIGHTS,
    }
}

/// How many texels the droplets' pictures have along a side.
const PICTURE_SIZE: u32 = 32;

/// A picture as solid as `opacity` says at each point of it, from -1 to 1 across (X) and
/// up (Y), as it lies on the square: up is the square's +Y. It is the colour `solid` where
/// it is solid, and goes to `thin` as it thins out.
fn picture(opacity: fn(Vec2) -> f32, solid: Color, thin: Color) -> Image {
    let texels = picture_texels(PICTURE_SIZE, opacity, solid, thin);
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

fn picture_texels(size: u32, opacity: fn(Vec2) -> f32, solid: Color, thin: Color) -> Vec<u8> {
    let middle = size as f32 / 2.0;
    let mut texels = Vec::with_capacity((size * size * 4) as usize);
    for row in 0..size {
        for col in 0..size {
            // The picture's first row is at the square's top.
            let point = Vec2::new(col as f32 + 0.5 - middle, middle - row as f32 - 0.5) / middle;
            let opacity = opacity(point).clamp(0.0, 1.0);
            let [red, green, blue, _] = thin.mix(&solid, opacity).to_srgba().to_u8_array();
            let alpha = (opacity * 255.0).round() as u8;
            texels.extend_from_slice(&[red, green, blue, alpha]);
        }
    }
    texels
}

/// A drop, going up the picture: a thin, soft streak, thickest a little ahead of its
/// middle and fading out to nothing at both ends. Stretched along the way it goes, the
/// streaks of a rooster tail run into each other as one sheet of spray. A round head, as
/// drops once had, made each one a teardrop of its own.
fn drop_opacity(point: Vec2) -> f32 {
    // From 0 at the tail's end to 1 at the front.
    let along = ((point.y + 1.0) / 2.0).clamp(0.0, 1.0);
    // 0 at both ends, and 1 three fifths of the way along.
    let thick = (std::f32::consts::PI * along.powf(1.3)).sin().max(0.0);
    let width = 0.85 * thick.sqrt();
    if width <= 0.0 {
        return 0.0;
    }
    let edge = 1.0 - smoothstep(0.35 * width, width, point.x.abs());
    edge * thick
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

/// How many bubbles a patch of foam, or a puff of froth, is made of.
const FOAM_BUBBLES: u32 = 16;

/// A patch of foam, one of `FOAM_SHAPES` squared, each its own as `shape` says: a cluster
/// of soft round bubbles of many sizes, thick where they crowd together and clear in the
/// gaps between them, so that its edge is ragged and never a clean circle. Round, so that
/// turned at any angle it has no straight edges: noise on a square grid made square lumps,
/// which, turned, showed as diamonds.
fn foam_opacity(point: Vec2, shape: u32) -> f32 {
    let random =
        |bubble: u32, which: u32| hash(Vec2::new((shape * 97 + bubble) as f32, which as f32));
    (0..FOAM_BUBBLES)
        .map(|bubble| {
            // Most near the middle; none reaching the square's edge.
            let middle = Vec2::from_angle(std::f32::consts::TAU * random(bubble, 0))
                * 0.6
                * random(bubble, 1).sqrt();
            let size = 0.12 + 0.2 * random(bubble, 2);
            let inside = 1.0 - point.distance(middle) / size;
            // A soft dome, more solid the bigger the bubble.
            smoothstep(0.0, 0.6, inside) * (0.7 + 0.3 * random(bubble, 3))
        })
        .fold(0.0, f32::max)
}

/// A number from 0 to 1 that only has to look random, the same for the same whole point.
fn hash(point: Vec2) -> f32 {
    let mut n = (point.x as i32 as u32).wrapping_mul(73_856_093)
        ^ (point.y as i32 as u32).wrapping_mul(19_349_663);
    n ^= n >> 13;
    n = n.wrapping_mul(0x5bd1_e995);
    n ^= n >> 15;
    (n & 0xffff) as f32 / 65_535.0
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

/// The spray's pool marked `Of`, which is neither of the other two.
type PoolOf<'w, 's, Of, Other, Third> =
    Query<'w, 's, &'static mut Particles, (With<Of>, Without<Other>, Without<Third>)>;

/// The pool of froth, which is none of the other three.
type FrothPool<'w, 's> = Query<
    'w,
    's,
    &'static mut Particles,
    (With<Froth>, Without<Drops>, Without<Mist>, Without<Foam>),
>;

/// Churns froth up round each tire that breaks the water's surface, and splashes where one
/// came down into it.
#[allow(clippy::too_many_arguments)]
pub(super) fn throw_spray(
    mut commands: Commands,
    time: Res<Time>,
    track: Res<Track>,
    looks: Option<Res<DropletLooks>>,
    trucks: Query<(), With<TruckVisual>>,
    mut tires: MessageReader<TireInWater>,
    mut mist: PoolOf<Mist, Drops, Foam>,
    mut foam: PoolOf<Foam, Drops, Mist>,
    mut froth: FrothPool,
    mut owed: Local<EntityHashMap<Owed>>,
    mut random: Local<Random>,
) {
    let (Some(level), Some(looks)) = (track.water_level, looks) else {
        return;
    };
    let (Ok(mut mists), Ok(mut foam), Ok(mut froth)) =
        (mist.single_mut(), foam.single_mut(), froth.single_mut())
    else {
        return;
    };
    let dt = time.delta_secs();
    let clock = pool::clock(&time);
    let floor = |at: Vec2| level.max(track.heights.height_at(at.x, at.y));
    // A patch of foam at `at`, on the water, going at `velocity` across it.
    let mut leave_foam = |at: Vec2, velocity: Vec2, random: &mut Random| {
        let patch = Particle {
            at: Vec3::new(at.x, level + FOAM_LIFT, at.y),
            velocity: Vec3::new(velocity.x, 0.0, velocity.y),
            size: random.between(FOAM_SIZE[0], FOAM_SIZE[1]),
            turned: random.between(0.0, std::f32::consts::TAU),
            turning: random.between(-0.4, 0.4),
            color: spray_edge().to_linear().with_alpha(FOAM_OPACITY),
            trail: Vec3::ZERO,
            shape: (random.next() * (FOAM_SHAPES * FOAM_SHAPES) as f32) as u32,
        };
        // It lies on the water, so never comes down.
        foam.throw(clock, patch, |_| f32::NEG_INFINITY);
    };

    // Trucks from a race that is over are forgotten.
    owed.retain(|visual, _| trucks.contains(*visual));

    for tire in tires.read() {
        let owed = owed.entry(tire.truck).or_default();
        if tire.entered {
            owed.froth[tire.wheel] = 0.0;
            owed.mist[tire.wheel] = 0.0;
        }
        let (moving, outwards) = (tire.moving, tire.outwards);

        let tread = Tread::of(tire, level);
        let ahead = tread.ahead;
        let ground_speed = moving.xz().length();
        let share = spray_share(tire.wet);
        let reach = reach(tire.wet);

        if let Some(plunge) = tire.plunge {
            let down = plunge.speed;
            let along = moving.with_y(0.0) * 0.3;
            let foot = Vec3::new(tire.hub.x, level, tire.hub.z);
            // Where the crown leaves the water: on the rim of the tire's footprint, `round`
            // radians round from its outside, and the way away from the tire there.
            let rim = |random: &mut Random| {
                let round = random.between(-SPLASH_SPREAD, SPLASH_SPREAD);
                let away = outwards * round.cos() + ahead * round.sin();
                let at = foot
                    + outwards * round.cos() * (0.5 * tire.wheel_width + FROTH_GAP)
                    + ahead * round.sin() * tire.wheel_radius * CROWN_ALONG;
                (at, away)
            };
            // Lumps of the broken water, thrown out low all round.
            for _ in 0..splash_lumps(down) {
                let (at, away) = rim(&mut random);
                let velocity =
                    (away * random.between(0.2, 0.5) + Vec3::Y * random.between(0.2, 0.5)) * down
                        + along;
                let size = random.between(FROTH_SIZE[0], FROTH_SIZE[1]) * 1.3;
                froth.throw(clock, froth_puff(at, velocity, size, &mut random), floor);
            }
            // Mist over it, slow, and only a little up and out.
            let puffs = random.between(SPLASH_MIST[0] as f32, SPLASH_MIST[1] as f32 + 1.0);
            for _ in 0..puffs as usize {
                let (at, away) = rim(&mut random);
                let velocity =
                    (away * 0.3 + Vec3::Y * 0.4) * down * random.between(0.5, 1.0) + along;
                let size = random.between(MIST_SIZE[0], MIST_SIZE[1]) * 1.5;
                throw_droplet(&mut mists, Kind::Mist, clock, floor, at, velocity, size);
            }
            // The water it broke, pushed out all round.
            for _ in 0..SPLASH_FOAM {
                let out = Vec2::from_angle(random.between(0.0, std::f32::consts::TAU));
                let at = tire.hub.xz() + out * tire.wheel_radius * random.between(0.3, 1.0);
                leave_foam(at, out * random.between(0.5, 1.5), &mut random);
            }
            let size = tire.wheel_radius * RING_START;
            commands.spawn((
                SplashRing { age: 0.0, size },
                Transform::from_translation(plunge.under_hub)
                    .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(size)),
                Mesh3d(looks.square.clone()),
                MeshMaterial3d(looks.ring[0].clone()),
                NotShadowCaster,
                DespawnOnExit(GameState::Racing),
            ));
        }

        // Mist off the front of the tire, where it pushes the water, across its width.
        owed.mist[tire.wheel] += mist_rate(ground_speed) * share * dt;
        while owed.mist[tire.wheel] >= 1.0 {
            owed.mist[tire.wheel] -= 1.0;
            let side = random.between(-1.0, 1.0);
            let at = tread.middle
                + ahead * tread.half_chord * random.between(0.5, 1.2)
                + outwards * side * 0.5 * tire.wheel_width;
            let velocity = moving.with_y(0.0) * MIST_KEEP
                + outwards * side * random.between(0.0, 1.0)
                + Vec3::Y * random.between(MIST_UP[0], MIST_UP[1]) * reach;
            let size = random.between(MIST_SIZE[0], MIST_SIZE[1]);
            throw_droplet(&mut mists, Kind::Mist, clock, floor, at, velocity, size);
        }

        // Froth churned up round the front and sides of the tire, where it meets the water.
        let churn = ground_speed.max(tread.spin);
        owed.froth[tire.wheel] += froth_rate(churn) * share * dt;
        while owed.froth[tire.wheel] >= 1.0 {
            owed.froth[tire.wheel] -= 1.0;
            let round = random.between(-FROTH_ROUND, FROTH_ROUND);
            let (at, away) = round_the_tire(tire, &tread, round);
            let velocity = moving.with_y(0.0) * random.between(FROTH_KEEP[0], FROTH_KEEP[1])
                + away * random.between(FROTH_OUT[0], FROTH_OUT[1])
                + Vec3::Y * froth_up(churn, tire.wet) * random.between(0.5, 1.0);
            let size = random.between(FROTH_SIZE[0], FROTH_SIZE[1]) * reach;
            froth.throw(clock, froth_puff(at, velocity, size, &mut random), floor);
        }
    }
}

/// How a tire's tread meets the water.
struct Tread {
    /// Under the hub, at the still level.
    middle: Vec3,
    /// Half the way from where it goes into the water to where it comes out, in metres.
    half_chord: f32,
    /// The way the tire goes over the ground, of length 1, or the way the truck faces when
    /// it stands still.
    ahead: Vec3,
    /// How much faster than the tire goes over the ground the tread goes, in m/s, up to
    /// `MOST_SPIN`.
    spin: f32,
}

impl Tread {
    fn of(tire: &TireInWater, level: f32) -> Self {
        let forward = tire.forward.with_y(0.0).normalize_or(Vec3::NEG_Z);
        let ground_speed = tire.moving.xz().length();
        let ahead = tire.moving.with_y(0.0).normalize_or(forward);
        // Where the tire's round meets the level, either side of the hub.
        let above = tire.hub.y - level;
        let half_chord = (tire.wheel_radius.powi(2) - above.powi(2)).max(0.0).sqrt();
        let spin = (tire.tread.abs() - ground_speed).clamp(0.0, MOST_SPIN);
        Self {
            middle: tire.hub.with_y(level),
            half_chord,
            ahead,
            spin,
        }
    }
}

/// The most a tread's spin counts for, in m/s faster than the tire goes over the ground. A
/// tire that floats free of the bottom spins up towards the truck's top speed (see
/// `truck::Wheel::tread_speed`); water loads a real one down.
const MOST_SPIN: f32 = 5.0;

/// Throws one droplet of `kind` into `pool`, `size` metres in radius, from `at`, going at
/// `velocity`, `now` (`pool::clock`). It falls until it goes below `floor`, the water
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
        // Its picture gives it its colour (see `make_droplet_looks`).
        color: LinearRgba::WHITE.with_alpha(opacity),
        trail: Vec3::ZERO,
        shape: 0,
    };
    pool.throw(now, droplet, floor)
}

/// A puff of froth at `at`, going at `velocity`, `size` metres in radius: one of the foam's
/// pictures, turned any way and turning.
fn froth_puff(at: Vec3, velocity: Vec3, size: f32, random: &mut Random) -> Particle {
    Particle {
        at,
        velocity,
        size,
        turned: random.between(0.0, std::f32::consts::TAU),
        turning: random.between(-1.5, 1.5),
        color: spray_edge().to_linear().with_alpha(FROTH_OPACITY),
        trail: Vec3::ZERO,
        shape: (random.next() * (FOAM_SHAPES * FOAM_SHAPES) as f32) as u32,
    }
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

/// How many puffs of mist a tire throws each second, going over the ground at `speed` m/s.
fn mist_rate(speed: f32) -> f32 {
    (speed * MIST_PER_SPEED).min(MOST_MIST)
}

/// How many lumps of froth a wheel throws coming down into the water at `speed` m/s.
fn splash_lumps(speed: f32) -> usize {
    SPLASH_FROTH + ((speed - SPLASH_SPEED).max(0.0) * SPLASH_FROTH_PER_SPEED) as usize
}

/// The most a tire's speed over the ground counts for, in m/s. Past it, the froth goes no
/// higher.
const MOST_PUSH: f32 = 8.0;

/// How far out and up the water a tire pushes goes, as a share of how far it goes in deep
/// water, for how far up the tire the water comes (`water::TireInWater::wet`): a tire
/// that only wets its bottom pushes little water, and throws it low.
fn reach(wet: f32) -> f32 {
    0.35 + 0.65 * smoothstep(0.05, 0.4, wet)
}

/// How many puffs of froth a tire churns up each second, its tread going `speed` m/s.
fn froth_rate(speed: f32) -> f32 {
    (speed * FROTH_PER_SPEED).min(MOST_FROTH)
}

/// How fast froth goes up, in m/s, off a tire whose tread goes `speed` m/s, `wet` deep.
fn froth_up(speed: f32, wet: f32) -> f32 {
    (FROTH_UP + speed.min(MOST_PUSH) * FROTH_UP_PER_SPEED) * reach(wet)
}

/// A point just outside `tire`'s round where it meets the water, `round` radians round from
/// the front of it (the way it goes over the ground) towards its outside, and the way away
/// from the tire there, on the ground plane. The round at the water is taken as an
/// ellipse, as long as the tread is in the water and as wide as the tire.
fn round_the_tire(tire: &TireInWater, tread: &Tread, round: f32) -> (Vec3, Vec3) {
    let ahead = tire
        .moving
        .with_y(0.0)
        .normalize_or(tire.forward.with_y(0.0).normalize_or(Vec3::NEG_Z));
    let along = tread.half_chord + FROTH_GAP;
    let across = 0.5 * tire.wheel_width + FROTH_GAP;
    let at = tread.middle + ahead * along * round.cos() + tire.outwards * across * round.sin();
    // Square to the ellipse: its normal.
    let away =
        (ahead * round.cos() / along + tire.outwards * round.sin() / across).normalize_or(ahead);
    (at, away)
}

/// How much of the spray it would throw a tire throws, for how far up it the water comes
/// (`water::TireInWater::wet`), from 0 to 1: little where the water only wets its bottom,
/// all of it from a quarter of the way up, and less once the water is over its hub, where
/// the tread comes up under it and only churns it.
fn spray_share(wet: f32) -> f32 {
    smoothstep(0.0, 0.25, wet) * (1.0 - 0.5 * smoothstep(0.5, 1.0, wet))
}

/// How far round the tire's footprint from its outside a splash's crown goes, in radians
/// either way. The rest of the way round is under the truck.
const SPLASH_SPREAD: f32 = 1.8;

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

    #[test]
    fn the_spray_gusts_as_the_wind_does() {
        let wind = Wind::default();
        for tenth in 0..600 {
            let time = tenth as f32 / 10.0;
            assert!(air(&wind).at(time).distance(wind.at(time)) < 1e-4, "{time}");
        }
    }

    #[test]
    fn faster_throws_more_mist_up_to_a_limit() {
        assert_eq!(mist_rate(0.0), 0.0);
        assert!(mist_rate(6.0) > mist_rate(3.0));
        assert_eq!(mist_rate(1000.0), MOST_MIST);
    }

    #[test]
    fn a_harder_landing_throws_more_froth() {
        assert_eq!(splash_lumps(SPLASH_SPEED), SPLASH_FROTH);
        assert!(splash_lumps(6.0) > splash_lumps(3.0));
    }

    /// A tire on a truck facing -Z, its hub 1 m over still water at 0, going `speed` m/s
    /// forwards with its tread going `tread` m/s.
    fn tire(speed: f32, tread: f32) -> TireInWater {
        TireInWater {
            truck: Entity::PLACEHOLDER,
            wheel: 0,
            at: Vec3::new(0.3, 0.0, 0.0),
            hub: Vec3::new(0.0, 0.6, 0.0),
            outwards: Vec3::X,
            forward: Vec3::NEG_Z,
            moving: Vec3::NEG_Z * speed,
            tread,
            wet: 0.4,
            wheel_radius: 1.0,
            wheel_width: 0.6,
            entered: false,
            plunge: None,
        }
    }

    #[test]
    fn the_tread_meets_the_water_either_side_of_the_hub() {
        let tread = Tread::of(&tire(10.0, 10.0), 0.0);
        // Where the tire's round meets the level, 0.6 m under the hub: 0.8 m either side.
        assert!((tread.half_chord - 0.8).abs() < 1e-5);
        assert!(tread.middle.distance(Vec3::ZERO) < 1e-5);
        assert_eq!(tread.spin, 0.0);
        let reversing = Tread::of(&tire(0.0, -4.0), 0.0);
        assert_eq!(reversing.spin, 4.0);
        // A tire floating free, spinning at a truck's top speed, counts for no more than
        // `MOST_SPIN`.
        let floating = Tread::of(&tire(10.0, 60.0), 0.0);
        assert_eq!(floating.spin, MOST_SPIN);
    }

    #[test]
    fn a_tire_in_shallow_water_throws_lower() {
        assert!(reach(0.05) < 0.5 * reach(0.4));
        assert!(froth_up(6.0, 0.05) < froth_up(6.0, 0.4));
    }

    #[test]
    fn the_tread_knows_which_way_it_goes() {
        let tread = Tread::of(&tire(10.0, 10.0), 0.0);
        assert!(tread.ahead.distance(Vec3::NEG_Z) < 1e-5);
        // Standing still with the tread still, the way the truck faces.
        let still = Tread::of(&tire(0.0, 0.0), 0.0);
        assert!(still.ahead.distance(Vec3::NEG_Z) < 1e-5);
    }

    #[test]
    fn froth_is_churned_up_round_the_front_and_sides_of_the_tire() {
        let tire = tire(6.0, 6.0);
        let tread = Tread::of(&tire, 0.0);
        // At the front, just ahead of the tread, going on ahead.
        let (front, away) = round_the_tire(&tire, &tread, 0.0);
        assert!(front.distance(Vec3::new(0.0, 0.0, -0.8 - FROTH_GAP)) < 1e-5);
        assert!(away.distance(Vec3::NEG_Z) < 1e-5);
        // A quarter of the way round, at its outside, going out.
        let (side, away) = round_the_tire(&tire, &tread, std::f32::consts::FRAC_PI_2);
        assert!((side.x - (0.3 + FROTH_GAP)).abs() < 1e-5 && side.z.abs() < 1e-5);
        assert!(away.distance(Vec3::X) < 1e-5);
        // A spinning tire churns too, up to a limit.
        assert!(froth_rate(3.0) > 0.0);
        assert_eq!(froth_rate(1000.0), MOST_FROTH);
    }

    #[test]
    fn a_tire_spinning_where_it_is_churns_froth() {
        let tire = tire(0.0, 10.0);
        let tread = Tread::of(&tire, 0.0);
        assert!(froth_rate(tread.spin) > 0.0);
    }

    #[test]
    fn a_tire_churns_most_part_way_in() {
        assert_eq!(spray_share(0.0), 0.0);
        assert!(spray_share(0.05) < spray_share(0.3));
        assert_eq!(spray_share(0.3), 1.0);
        assert!(spray_share(1.0) < spray_share(0.3) && spray_share(1.0) > 0.0);
    }

    #[test]
    fn the_foam_lies_on_the_swell_unless_the_water_is_drawn_flat() {
        let wind = Wind::default();
        let waves = swell(Some(&wind), None);
        for (wave, swell) in wind.swell().iter().enumerate() {
            assert_eq!(waves.travel[wave], swell.travel);
            assert_eq!(waves.rates[wave], swell.rate);
            assert_eq!(waves.heights[wave], swell.height);
        }
        let flat = WaterSettings {
            flat: true,
            ..default()
        };
        assert_eq!(swell(Some(&wind), Some(&flat)), Waves::FLAT);
        assert_eq!(swell(None, None), Waves::FLAT);
    }

    #[test]
    fn a_patch_of_foam_is_broken_and_clear_at_its_edge() {
        for shape in 0..FOAM_SHAPES * FOAM_SHAPES {
            let samples: Vec<f32> = (0..32 * 32)
                .map(|index| {
                    let point =
                        Vec2::new((index % 32) as f32, (index / 32) as f32) / 15.5 - Vec2::ONE;
                    let opacity = foam_opacity(point, shape);
                    assert!((0.0..=1.0).contains(&opacity), "{opacity}");
                    if point.length() >= 1.0 {
                        assert!(opacity < 1e-3, "{shape} {point}");
                    }
                    opacity
                })
                .collect();
            // Thick in places, and with holes: neither a clean disc nor nothing.
            let thick = samples.iter().filter(|opacity| **opacity > 0.6).count();
            let inside = (0..32 * 32)
                .filter(|index| {
                    let point =
                        Vec2::new((index % 32) as f32, (index / 32) as f32) / 15.5 - Vec2::ONE;
                    point.length() < 0.6 && samples[*index] < 0.4
                })
                .count();
            assert!(thick > 30, "{shape}: {thick}");
            assert!(inside > 0, "{shape}: no holes");
        }
        // The four are not the same.
        let point = Vec2::new(0.2, -0.1);
        assert_ne!(foam_opacity(point, 0), foam_opacity(point, 3));
    }

    #[test]
    fn droplet_pictures_are_solid_in_the_middle_and_clear_at_the_edge() {
        let texels = picture_texels(
            16,
            |point| round(mist_opacity, point),
            Color::WHITE,
            Color::WHITE,
        );
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
    fn a_droplet_is_blue_in_the_middle_and_white_at_the_edge() {
        let texels = picture_texels(
            16,
            |point| round(mist_opacity, point),
            spray_color(1.0),
            spray_edge(),
        );
        let texel = |col: usize| &texels[(8 * 16 + col) * 4..][..3];
        // Blue: much less red than blue.
        let [red, _, blue] = texel(8) else { panic!() };
        assert!(*red + 60 < *blue, "{:?}", texel(8));
        // Out towards the edge, whiter: more red, and as much blue.
        let [edge_red, _, edge_blue] = texel(15) else {
            panic!()
        };
        assert!(edge_red > red && edge_blue >= blue, "{:?}", texel(15));
    }

    #[test]
    fn a_drop_is_a_soft_streak() {
        // Solid a little ahead of its middle, and clear off to the sides.
        let middle = Vec2::new(0.0, 0.17);
        assert!(drop_opacity(middle) > 0.9);
        assert_eq!(drop_opacity(Vec2::new(0.9, 0.17)), 0.0);
        // Fainter and thinner towards both ends: no round head.
        for end in [-0.8, 0.9] {
            assert!(drop_opacity(Vec2::new(0.0, end)) < 0.6 * drop_opacity(middle));
            assert!(drop_opacity(Vec2::new(0.25, end)) < drop_opacity(Vec2::new(0.25, 0.17)));
        }
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
