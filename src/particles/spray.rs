//! The water that trucks throw up: the spray their tires throw, the foam they churn up, and
//! the splash as a wheel comes down into it.
//!
//! Where a tire breaks the surface, and whether it came down into it fast, the `water`
//! slice says (`water::TireInWater`). A tire that goes through the water pushes it aside, as
//! the bow of a boat does: the water piles up in front of the tread, and peels off round
//! its shoulders in a wing either side, which opens out in a V from the front of the tire,
//! arcs over and pours back down beside it. More, higher and further the faster the tire
//! goes over the ground. Most where the water is a quarter of the way up the tire: little
//! where it only wets the bottom, and less where it is over the hub. The wing on the inside,
//! under the truck, is fainter. A tire that spins in the water without going anywhere only
//! churns it into foam. A tire that comes down into the water fast throws a ring of it up
//! all round, and leaves a ring of foam that spreads on the water.
//!
//! The wings are sheets of water (`sheet`), not drops: a tire pours a lip of water every
//! `SHEET_EVERY` seconds, from the front of its tread (its root) round to its shoulder (its
//! crest), and the lips one after the other make one surface. The root is shoved on ahead
//! at about the tire's speed, and the crest is thrown out and up and falls behind
//! (`wing_lip`). It tears into strands as it goes, and a little mist comes off it.
//!
//! The rest of the spray is in three layers, each a pool of particles (`pool`) that the
//! graphics card moves: drops, mist, and foam. Here they are only thrown, and given the
//! wind (`water::Wind`) and the swell (`water::Wind::swell`).
//!
//! Spray is droplets of two kinds. Drops are small and heavy: thrown, pulled down by
//! gravity, and drawn longer the faster they go, as the eye sees a fast drop. Mist is fine
//! water that hangs in the air: big, faint puffs, thrown with the drops, that the air soon
//! slows and the wind carries off, and that spread and fade as they go. Mist is what makes
//! spray look like a body of water rather than a scatter of beads. Both are gone when they
//! come down into the water or onto the ground, and both fade out where a truck or the
//! ground cuts through them (`Motion::soft`). They are drawn only, and touch nothing.
//!
//! Each is a flat square turned to the camera, with a soft round picture on it. Balls, as
//! they once were, showed their facets, and their hard edges looked like confetti.
//!
//! Foam is the broken water a tire leaves behind it: flat, ragged patches of bubbles that
//! lie on the water, rise and fall with its swell, spread, drift a little downwind, and
//! fade. A tire leaves one every metre or so, and more while it spins. Past the patch of
//! water round the camera that the swell moves (see `water::surface`), the water is drawn
//! flat and the foam still rises and falls: too far off to see.

use bevy::asset::RenderAssetUsages;
use bevy::ecs::entity::EntityHashMap;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::pool::{self, Air, Motion, Particle, Particles, Waves};
use super::sheet::{self, Flow, Lip, Sheets};
use crate::game_state::GameState;
use crate::track::Track;
use crate::truck::TruckVisual;
use crate::water::{SPLASH_SPEED, TireInWater, WATER_COLOR, WaterSettings, Wind};

/// How fast a tire must be going over the ground to throw spray, in m/s, and how much
/// faster its sheets are as solid as they get.
const SPRAY_SPEED: f32 = 1.5;
const FULL_SHEET_SPEED: f32 = 4.0;
/// How many drops a tire's wings tear into each second for each m/s it goes faster than
/// `SPRAY_SPEED`, and the most, however fast it goes. More is a thicker spray, and costs
/// more.
const SPRAY_PER_SPEED: f32 = 10.0;
const MOST_SPRAY: f32 = 120.0;
/// How often a tire pours a lip of each of its sheets, in seconds. Less is a smoother sheet
/// where it turns, and more sent to the graphics card.
const SHEET_EVERY: f32 = 1.0 / 30.0;
/// How long a lip lasts, in seconds, and how long of that it stays as it was poured. It
/// fades out over the rest, as it tears into drops. Water holds together as a sheet only
/// for a moment: a long-lived sheet looks like a sheet of plastic.
const SHEET_LIFE: f32 = 0.7;
const SHEET_HOLD: f32 = 0.35;
/// How far over the still water a sheet starts to fade out, and how far under it it is
/// gone, in metres: it fades out as it pours in. It is gone before it comes down onto the
/// water, which is drawn in flat triangles on the swell and would cut it along their edges.
const SHEET_FLOAT: f32 = 0.15;
const SHEET_SINK: f32 = 0.2;
/// How much of a sheet's thin water has torn away by the end of a lip's life, from 0 to 1.
/// More breaks it into strands sooner.
const SHEET_TEAR: f32 = 0.6;
/// How old the water of a lip is when it tears into drops, in seconds: the least and the
/// most. The drops are thrown where the water is then, so that they go on from the sheet.
const SHEET_TEARS: [f32; 2] = [0.3, 0.6];
/// How far across a lip, from its root (0) to its crest (1), the drops tear from: all of
/// it, the plume in front as well as the wing.
const TEARS_FROM: f32 = 0.0;
/// How fast the drops are scattered from where the sheet went, in m/s at most.
const TEAR_SCATTER: f32 = 0.7;
/// How big a torn drop is, in metres: the smallest and the largest radius. Clumps, not
/// beads.
const TORN_SIZE: [f32; 2] = [0.05, 0.1];
/// How near a truck or the ground behind it a sheet starts to fade out, in metres, and how
/// near the camera.
const SHEET_SOFT: f32 = 0.3;
const SHEET_NEAR_CAMERA: f32 = 3.0;
/// How many times the sheets' picture goes by for each second a tire pours. More is
/// shorter strands.
const SHEET_PICTURE_RATE: f32 = 4.0;
/// How many sheets there may be at once: a wing either side of each wheel of eight
/// trucks. Past it, no more are poured until one is gone.
const SHEET_STRIPS: usize = 8 * 4 * 2;
/// How many points across each lip is: more is a smoother curve from root to crest.
const SHEET_COLUMNS: usize = 8;
/// How solid a tire's wing of water is, from 0 to 1, at its thickest.
const WING_OPACITY: f32 = 1.0;
/// How many drops a splash throws: the least, when the wheel only just comes down fast
/// enough to splash (`water::SPLASH_SPEED`), how many more for each m/s faster, and the most.
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
const DROP_SLOTS: usize = 5000;
const MIST_SLOTS: usize = 800;
/// How many patches of foam may lie on the water at once, over all the trucks.
const FOAM_SLOTS: usize = 4000;
/// How big a drop of a splash is, in metres: the smallest and the largest radius. Its edge
/// is soft, so it looks a little smaller than this. A splash throws water up in lumps.
const SPLASH_SIZE: [f32; 2] = [0.08, 0.22];
/// How many puffs of mist come with each drop a tire's wings tear into, from 0 to 1. More
/// is a thicker, whiter spray.
const MIST_PER_SPRAY: f32 = 0.1;
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
const DROP_OPACITY: f32 = 0.7;
/// How near a truck or the ground behind it mist, and a drop, start to fade out, in
/// metres, so that one cut through by them has no hard edge. Wider for mist, whose puffs
/// are big.
const MIST_SOFT: f32 = 0.5;
const DROP_SOFT: f32 = 0.1;

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
const FROTH_SIZE: [f32; 2] = [0.25, 0.45];
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
const FROTH_ROUND: f32 = 1.9;
/// How far out from the tire's round at the water froth starts, in metres.
const FROTH_GAP: f32 = 0.1;
/// How see-through froth is, from 0 (not there) to 1 (solid), and how near what is behind
/// it it starts to fade out, in metres.
const FROTH_OPACITY: f32 = 0.95;
const FROTH_SOFT: f32 = 0.3;

/// How far a tire goes for each patch of foam it leaves, in metres, how many more it leaves
/// each second for each m/s its tread goes faster than it rolls, and the most each second.
/// Fewer, bigger patches cost less than many small ones, and overlap less.
const FOAM_SPACING: f32 = 0.35;
const FOAM_PER_SPIN: f32 = 2.0;
const MOST_FOAM: f32 = 50.0;
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

/// What the spray is drawn with: the pictures of a drop, of mist, of foam and of a sheet,
/// and for a splash's ring, one square and a material for each step of it fading out, most
/// solid first.
#[derive(Resource)]
pub(super) struct DropletLooks {
    drop: Handle<Image>,
    sheet: Handle<Image>,
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
/// drops and the patches of foam it owes, carried over as a fraction.
#[derive(Default)]
pub(super) struct Owed {
    spray: [f32; 4],
    froth: [f32; 4],
    foam: [f32; 4],
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
        sheet: images.add(sheet_picture()),
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

/// Makes the race's sheets, on a track with water. Its own system: its assets are the
/// pools' too.
pub(super) fn spawn_sheets(
    mut commands: Commands,
    track: Res<Track>,
    looks: Res<DropletLooks>,
    wind: Option<Res<Wind>>,
    mut assets: sheet::SheetAssets,
) {
    let Some(level) = track.water_level else {
        return;
    };
    let flow = Flow {
        gravity: GRAVITY,
        drag: AIR_DRAG,
        air: wind.as_deref().map_or(Vec3::ZERO, Wind::steady),
        life: SHEET_LIFE,
        fade_from: SHEET_HOLD,
        every: SHEET_EVERY,
        floor: level + SHEET_FLOAT,
        sink: SHEET_SINK,
        tear: SHEET_TEAR,
        soft: SHEET_SOFT,
        near_camera: SHEET_NEAR_CAMERA,
        picture_rate: SHEET_PICTURE_RATE,
    };
    let sheets = sheet::sheets(
        &mut assets,
        SHEET_STRIPS,
        SHEET_COLUMNS,
        flow,
        looks.sheet.clone(),
    );
    commands.spawn((sheets, DespawnOnExit(GameState::Racing)));
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

/// How many texels the sheets' picture has across, from root to crest, and along.
const SHEET_PICTURE_SIZE: [u32; 2] = [64, 128];
/// How much of the spray's blue (`spray_color`) the sheets take where they are thick, from
/// 0 (white) to 1. Water torn up by a tire is full of air, and white.
const SHEET_BLUE: f32 = 0.1;

/// The sheets' picture: as solid as `sheet_opacity` says, blue where it is thick and white
/// where it thins out. It repeats along the sheet.
fn sheet_picture() -> Image {
    let [across, along] = SHEET_PICTURE_SIZE;
    let mut texels = Vec::with_capacity((across * along * 4) as usize);
    for row in 0..along {
        for col in 0..across {
            let point = Vec2::new(
                (col as f32 + 0.5) / across as f32,
                (row as f32 + 0.5) / along as f32,
            );
            let opacity = sheet_opacity(point).clamp(0.0, 1.0);
            let [red, green, blue, _] = spray_edge()
                .mix(&spray_color(1.0), opacity * SHEET_BLUE)
                .to_srgba()
                .to_u8_array();
            texels.extend_from_slice(&[red, green, blue, (opacity * 255.0).round() as u8]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: across,
            height: along,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        texels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

/// The water of a sheet at `point`: across (X) from its root, at 0, to its crest, at 1, and
/// along (Y), from 0 to 1, round which it repeats. Froth: mostly solid, in lumps and
/// bubbles, which tear apart as it ages, and a ragged crest where it thins out.
fn sheet_opacity(point: Vec2) -> f32 {
    let lumps = noise(Vec2::new(point.x * 10.0, point.y * 8.0), Some(8.0));
    let bubbles = noise(Vec2::new(point.x * 36.0 + 5.0, point.y * 28.0), Some(28.0));
    let ragged = noise(Vec2::new(3.0, point.y * 20.0), Some(20.0));
    let crest = 1.0 - smoothstep(0.55 + 0.35 * ragged, 1.0, point.x);
    let root = smoothstep(0.0, 0.04, point.x);
    root * crest * (0.5 + 0.3 * lumps + 0.2 * bubbles)
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

/// Smooth noise from 0 to 1, the same at the same point: a number at each whole point,
/// blended between them, and if `period` says, the same again every `period` along Y.
fn noise(point: Vec2, period: Option<f32>) -> f32 {
    let cell = point.floor();
    let t = point - cell;
    let t = t * t * (Vec2::splat(3.0) - 2.0 * t);
    let at = |x: f32, y: f32| {
        let corner = cell + Vec2::new(x, y);
        hash(period.map_or(corner, |period| corner.with_y(corner.y.rem_euclid(period))))
    };
    let bottom = at(0.0, 0.0) + (at(1.0, 0.0) - at(0.0, 0.0)) * t.x;
    let top = at(0.0, 1.0) + (at(1.0, 1.0) - at(0.0, 1.0)) * t.x;
    bottom + (top - bottom) * t.y
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

/// Pours sheets of water off each tire that breaks the water's surface, with mist off them,
/// leaves foam behind it, and splashes where one came down into it.
#[allow(clippy::too_many_arguments)]
pub(super) fn throw_spray(
    mut commands: Commands,
    time: Res<Time>,
    track: Res<Track>,
    looks: Option<Res<DropletLooks>>,
    trucks: Query<(), With<TruckVisual>>,
    mut tires: MessageReader<TireInWater>,
    mut drops: PoolOf<Drops, Mist, Foam>,
    mut mist: PoolOf<Mist, Drops, Foam>,
    mut foam: PoolOf<Foam, Drops, Mist>,
    mut froth: FrothPool,
    mut sheets: Query<&mut Sheets>,
    mut owed: Local<EntityHashMap<Owed>>,
    mut random: Local<Random>,
) {
    let (Some(level), Some(looks)) = (track.water_level, looks) else {
        return;
    };
    let (Ok(mut drops), Ok(mut mists), Ok(mut foam), Ok(mut froth), Ok(mut sheets)) = (
        drops.single_mut(),
        mist.single_mut(),
        foam.single_mut(),
        froth.single_mut(),
        sheets.single_mut(),
    ) else {
        return;
    };
    let dt = time.delta_secs();
    let clock = pool::clock(&time);
    let floor = |at: Vec2| level.max(track.heights.height_at(at.x, at.y));
    // A droplet of `kind`, and now and then as `mist` says, a puff of mist with it.
    let mut throw = |kind: Kind,
                     at: Vec3,
                     velocity: Vec3,
                     [smallest, largest]: [f32; 2],
                     mist: f32,
                     random: &mut Random| {
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
            owed.spray[tire.wheel] = 0.0;
            owed.froth[tire.wheel] = 0.0;
            owed.foam[tire.wheel] = 0.0;
        }
        let (moving, outwards) = (tire.moving, tire.outwards);

        if let Some(plunge) = tire.plunge {
            let down = plunge.speed;
            let along = moving.with_y(0.0) * 0.3;
            for _ in 0..splash_droplets(down) {
                let velocity = crown_velocity(&mut random, down, outwards) + along;
                throw(Kind::Drop, tire.at, velocity, SPLASH_SIZE, 0.0, &mut random);
            }
            let puffs = random.between(SPLASH_MIST[0] as f32, SPLASH_MIST[1] as f32 + 1.0);
            for _ in 0..puffs as usize {
                // Slow, and only a little up and out.
                let velocity = crown_velocity(&mut random, down, outwards) * 0.2 + along;
                throw(
                    Kind::Mist,
                    tire.at,
                    velocity,
                    SPLASH_MIST_SIZE,
                    0.0,
                    &mut random,
                );
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

        let tread = Tread::of(tire, level);
        let share = spray_share(tire.wet);
        let ground_speed = moving.xz().length();
        let strength = WING_OPACITY * share * sheet_share(ground_speed);
        let outer = wing_lip(tire, &tread, 1.0, strength);
        let inner = wing_lip(tire, &tread, -1.0, strength * INNER_SHARE);
        // Each sheet is its own: the truck, its wheel, and which side.
        let source = (tire.truck.to_bits() << 3) | (tire.wheel as u64) << 1;
        for (lip, which) in [(outer, 0), (inner, 1)] {
            // A sheet too thin to see is not poured, and starts again when it is not.
            if lip.strength > 0.01 {
                sheets.pour(source | which, clock, lip);
            }
        }
        // The wings tear into drops, which go on as the water went: each where a lip's
        // water is as it tears, going as it goes there, a little scattered, with mist.
        owed.spray[tire.wheel] += spray_rate(ground_speed) * share * dt;
        while owed.spray[tire.wheel] >= 1.0 {
            owed.spray[tire.wheel] -= 1.0;
            let lip = if random.next() < 1.0 / (1.0 + INNER_SHARE) {
                outer
            } else {
                inner
            };
            let across = random.between(TEARS_FROM, 1.0);
            let (at, velocity) = DROP_MOTION.at(
                lip.root.lerp(lip.crest, across),
                lip.root_velocity.lerp(lip.crest_velocity, across),
                Vec3::ZERO,
                random.between(SHEET_TEARS[0], SHEET_TEARS[1]),
            );
            let scatter = Vec3::new(
                random.between(-1.0, 1.0),
                random.between(-0.5, 1.0),
                random.between(-1.0, 1.0),
            ) * TEAR_SCATTER;
            throw(
                Kind::Drop,
                at,
                velocity + scatter,
                TORN_SIZE,
                MIST_PER_SPRAY,
                &mut random,
            );
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
            let puff = Particle {
                at,
                velocity,
                size: random.between(FROTH_SIZE[0], FROTH_SIZE[1]) * reach(tire.wet),
                turned: random.between(0.0, std::f32::consts::TAU),
                turning: random.between(-1.5, 1.5),
                color: spray_edge().to_linear().with_alpha(FROTH_OPACITY),
                trail: Vec3::ZERO,
                shape: (random.next() * (FOAM_SHAPES * FOAM_SHAPES) as f32) as u32,
            };
            froth.throw(clock, puff, floor);
        }

        owed.foam[tire.wheel] +=
            foam_rate(moving.xz().length(), tread.spin) * foam_share(tire.wet) * dt;
        while owed.foam[tire.wheel] >= 1.0 {
            owed.foam[tire.wheel] -= 1.0;
            // Anywhere in the tire's footprint on the water and just beside it, pushed out
            // to its own side: the two arms of a wake.
            let side = random.between(-0.8, 0.8);
            let across = outwards * side * tire.wheel_width;
            let at = tire.hub + tread.back * random.between(-0.5, 1.0) * tread.half_chord + across;
            let velocity = moving.with_y(0.0) * random.between(0.0, 0.2)
                + outwards * side.signum() * random.between(0.5, 2.0)
                + tread.back * tread.spin * random.between(0.1, 0.3);
            leave_foam(at.xz(), velocity.xz(), &mut random);
        }
    }
}

/// How a tire's tread meets the water.
struct Tread {
    /// Under the hub, at the still level.
    middle: Vec3,
    /// Half the way from where it goes into the water to where it comes out, in metres.
    half_chord: f32,
    /// Behind the way the tread turns, on the ground plane, of length 1.
    back: Vec3,
    /// How much faster than the tire goes over the ground the tread goes, in m/s, up to
    /// `MOST_SPIN`.
    spin: f32,
}

impl Tread {
    fn of(tire: &TireInWater, level: f32) -> Self {
        let forward = tire.forward.with_y(0.0).normalize_or(Vec3::NEG_Z);
        let ground_speed = tire.moving.xz().length();
        // The way the tread turns: from the truck's way over the ground when the tread is
        // too slow to tell, as when it stops in the water.
        let back = if tire.tread.abs() > 0.5 {
            -forward * tire.tread.signum()
        } else {
            -tire.moving.with_y(0.0).normalize_or(forward)
        };
        // Where the tire's round meets the level, either side of the hub.
        let above = tire.hub.y - level;
        let half_chord = (tire.wheel_radius.powi(2) - above.powi(2)).max(0.0).sqrt();
        let spin = (tire.tread.abs() - ground_speed).clamp(0.0, MOST_SPIN);
        Self {
            middle: tire.hub.with_y(level),
            half_chord,
            back,
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

/// How many droplets a tire throws each second, going over the ground at `speed` m/s.
fn spray_rate(speed: f32) -> f32 {
    ((speed - SPRAY_SPEED).max(0.0) * SPRAY_PER_SPEED).min(MOST_SPRAY)
}

/// How many droplets a wheel throws coming down into the water at `speed` m/s.
fn splash_droplets(speed: f32) -> usize {
    let more = ((speed - SPLASH_SPEED).max(0.0) * SPLASH_PER_SPEED) as usize;
    (LEAST_SPLASH + more).min(MOST_SPLASH)
}

/// How fast the water a tire pushes through goes, as shares of how fast the tire goes over
/// the ground (up to `MOST_PUSH`): at the front of the tread, where it piles up (a lip's
/// root), and at the tire's shoulder, where it peels off the side (its crest). The tire
/// is blunt: it shoves the water in front of it on a little faster than it goes itself,
/// and turns the water at its shoulder out to the side. So the water at the front is a
/// low wall of white water pushed on just in front of the tire, and that at the shoulder
/// fans out low to the side and falls a little behind: wings of froth from the front
/// corners, as a vehicle driven through deep water throws. Along the way the tire goes, out
/// from its side, and up.
const WING_ALONG: [f32; 2] = [1.15, 0.85];
const WING_OUT: [f32; 2] = [0.1, 0.45];
const WING_UP: [f32; 2] = [0.25, 0.4];
/// How fast the wing goes up however slow the tire, in m/s: at its root and its crest.
const WING_BASE: [f32; 2] = [0.3, 0.3];
/// The most a tire's speed over the ground counts for, in m/s. Past it, the wing is no
/// bigger. At 8 m/s, the front rises about 0.25 m, a little in front of the tire, and the
/// crest about 0.5 m, and comes down about 2 m out from the tire, 0.7 s later. The crest
/// goes up about as fast as out: a curtain, which seen from behind and above is not a
/// sheet laid over the water.
const MOST_PUSH: f32 = 8.0;
/// How far round from the front of the tread to its side the crest starts, as a share of
/// half the tire's width out, and of half the way the tread is in the water back.
const SHOULDER_OUT: f32 = 1.0;
const SHOULDER_BACK: f32 = 0.35;
/// How solid the wing on the inside of a tire is, under the truck, as a share of the one
/// on the outside. The truck's body is over it, and it runs into the other tires.
const INNER_SHARE: f32 = 0.5;

/// The wing of water off `tire`'s `side` (1 for the outside, away from the truck, and -1
/// for the inside), where its tread meets the water as `tread` says, as solid as
/// `strength`: from the front of the tread, the way the tire goes over the ground, round
/// to its shoulder on that side.
fn wing_lip(tire: &TireInWater, tread: &Tread, side: f32, strength: f32) -> Lip {
    let along = tire.moving.with_y(0.0);
    let ahead = along.normalize_or(tire.forward.with_y(0.0).normalize_or(Vec3::NEG_Z));
    let speed = along.length().min(MOST_PUSH);
    let out = tire.outwards * side;
    let root = tread.middle + ahead * tread.half_chord;
    let crest = root + out * 0.5 * tire.wheel_width * SHOULDER_OUT
        - ahead * tread.half_chord * SHOULDER_BACK;
    let reach = reach(tire.wet);
    let velocity = |end: usize| {
        ahead * speed * WING_ALONG[end]
            + (out * speed * WING_OUT[end] + Vec3::Y * (WING_BASE[end] + speed * WING_UP[end]))
                * reach
    };
    Lip {
        root,
        crest,
        root_velocity: velocity(0),
        crest_velocity: velocity(1),
        strength,
    }
}

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

/// How solid a sheet is that is poured at `speed` m/s, from 0 to 1: none until
/// `SPRAY_SPEED`, and all of it `FULL_SHEET_SPEED` faster.
fn sheet_share(speed: f32) -> f32 {
    smoothstep(SPRAY_SPEED, SPRAY_SPEED + FULL_SHEET_SPEED, speed)
}

/// How much of the spray it would throw a tire throws, for how far up it the water comes
/// (`water::TireInWater::wet`), from 0 to 1: little where the water only wets its bottom,
/// all of it from a quarter of the way up, and less once the water is over its hub, where
/// the tread comes up under it and only churns it.
fn spray_share(wet: f32) -> f32 {
    smoothstep(0.0, 0.25, wet) * (1.0 - 0.5 * smoothstep(0.5, 1.0, wet))
}

/// How many patches of foam a tire leaves each second, going `speed` m/s over the ground
/// with its tread going `spin` m/s faster than that.
fn foam_rate(speed: f32, spin: f32) -> f32 {
    (speed / FOAM_SPACING + spin * FOAM_PER_SPIN).min(MOST_FOAM)
}

/// How much of the foam it would leave a tire leaves, for how far up it the water comes,
/// from 0 to 1: more the deeper it churns.
fn foam_share(wet: f32) -> f32 {
    smoothstep(0.05, 0.5, wet)
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

    #[test]
    fn the_spray_gusts_as_the_wind_does() {
        let wind = Wind::default();
        for tenth in 0..600 {
            let time = tenth as f32 / 10.0;
            assert!(air(&wind).at(time).distance(wind.at(time)) < 1e-4, "{time}");
        }
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
        // Back is +Z, behind a truck facing -Z, and in front, spinning backwards.
        assert!(tread.back.z > 0.0);
        let reversing = Tread::of(&tire(0.0, -4.0), 0.0);
        assert!(reversing.back.z < 0.0);
        assert_eq!(reversing.spin, 4.0);
        // A tire floating free, spinning at a truck's top speed, counts for no more than
        // `MOST_SPIN`.
        let floating = Tread::of(&tire(10.0, 60.0), 0.0);
        assert_eq!(floating.spin, MOST_SPIN);
    }

    /// The outer and the inner wing off `tire`.
    fn wings(tire: &TireInWater) -> (Lip, Lip) {
        let tread = Tread::of(tire, 0.0);
        (
            wing_lip(tire, &tread, 1.0, 1.0),
            wing_lip(tire, &tread, -1.0, 1.0),
        )
    }

    #[test]
    fn a_wing_starts_at_the_front_of_the_tire_and_goes_round_to_its_side() {
        // Facing -Z, with the outside at +X.
        let (outer, inner) = wings(&tire(10.0, 10.0));
        for wing in [outer, inner] {
            // At the front of the tread, where it goes into the water.
            assert!(wing.root.distance(Vec3::new(0.0, 0.0, -0.8)) < 1e-5);
            // The shoulder: out to the side, and a little back from the front.
            assert!(wing.crest.z > wing.root.z && wing.crest.z < 0.0);
            assert!(wing.root.y == 0.0 && wing.crest.y == 0.0);
        }
        assert!(outer.crest.x > 0.0 && inner.crest.x < 0.0);
        // Nothing goes out the back.
        for velocity in [outer.root_velocity, outer.crest_velocity] {
            assert!(velocity.z < 0.0, "{velocity}");
        }
        // Out to its own side.
        assert!(outer.crest_velocity.x > 0.0 && inner.crest_velocity.x < 0.0);
    }

    #[test]
    fn the_front_is_thrown_up_ahead_and_the_shoulder_flies_out_and_falls_behind() {
        let speed = 6.0;
        let (wing, _) = wings(&tire(speed, speed));
        // Shoved on ahead, faster than the tire, and up, and hardly out to the side.
        let front = wing.root_velocity;
        assert!(front.z < -1.05 * speed, "{front}");
        assert!(front.y > 0.2 * speed, "{front}");
        assert!(front.x.abs() < 0.2 * speed, "{front}");
        // Slower on than the tire, so that it falls behind, and out and up faster than
        // the root, so that the wing opens out.
        assert!(wing.crest_velocity.z > -speed);
        assert!(wing.crest_velocity.x > wing.root_velocity.x);
        assert!(wing.crest_velocity.y > wing.root_velocity.y);
        // Up about as fast as out: a curtain, not a sheet laid over the water.
        let (out, up) = (wing.crest_velocity.x, wing.crest_velocity.y);
        assert!(up > 0.7 * out && up < 1.5 * out, "{}", wing.crest_velocity);
    }

    #[test]
    fn a_faster_tire_throws_its_wings_higher_up_to_a_limit() {
        let highest = |speed: f32| wings(&tire(speed, speed)).0.crest_velocity.y;
        assert!(highest(10.0) > highest(3.0) * 2.0);
        assert_eq!(highest(MOST_PUSH), highest(MOST_PUSH * 3.0));
    }

    #[test]
    fn a_tire_going_backwards_pushes_its_wings_out_behind_the_truck() {
        let mut backwards = tire(0.0, -6.0);
        backwards.moving = Vec3::Z * 6.0;
        let (wing, _) = wings(&backwards);
        assert!(wing.root.z > 0.0 && wing.root_velocity.z > 0.0);
    }

    #[test]
    fn a_tire_in_shallow_water_throws_its_wings_lower_and_less_far() {
        let deep = tire(6.0, 6.0);
        let shallow = TireInWater { wet: 0.05, ..deep };
        let (deep, _) = wings(&deep);
        let (shallow, _) = wings(&shallow);
        assert!(shallow.crest_velocity.y < 0.5 * deep.crest_velocity.y);
        assert!(shallow.crest_velocity.x < 0.5 * deep.crest_velocity.x);
        // On just as fast: the tire still goes through it.
        assert_eq!(shallow.root_velocity.z, deep.root_velocity.z);
        assert!(froth_up(6.0, 0.05) < froth_up(6.0, 0.4));
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
    fn a_tire_spinning_where_it_is_throws_no_wings_but_churns_foam() {
        let tire = tire(0.0, 10.0);
        let tread = Tread::of(&tire, 0.0);
        assert_eq!(sheet_share(tire.moving.length()), 0.0);
        assert!(foam_rate(0.0, tread.spin) > foam_rate(0.0, 0.0));
    }

    #[test]
    fn a_tire_throws_most_spray_part_way_in_and_more_foam_deeper() {
        assert_eq!(spray_share(0.0), 0.0);
        assert!(spray_share(0.05) < spray_share(0.3));
        assert_eq!(spray_share(0.3), 1.0);
        assert!(spray_share(1.0) < spray_share(0.3) && spray_share(1.0) > 0.0);
        assert_eq!(foam_share(0.0), 0.0);
        assert!(foam_share(0.8) > foam_share(0.2));
        // A patch of foam every `FOAM_SPACING` metres, up to a limit.
        assert!((foam_rate(FOAM_SPACING, 0.0) - 1.0).abs() < 1e-5);
        assert_eq!(foam_rate(1000.0, 0.0), MOST_FOAM);
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
