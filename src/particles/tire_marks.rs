//! The tracks that tires leave on loose ground: dirt, mud, sand, grass and snow.
//!
//! A tire on loose ground presses a track into it as it rolls: a strip as wide as the
//! tire, with the tread's lugs printed along it, darker than the ground round it, as
//! pressed and turned ground is. A tire that slides sideways smears a wider one, and one
//! spun from standing digs a darker one. Road, rock and ice take no track: the ground
//! must be loose (`track::TrackData::loose_at`), as for the dirt that tires throw up
//! (`dirt`), and above the water. A tire in the air, or on a truck on its side or its
//! roof, leaves nothing.
//!
//! The tracks are ribbons (`ribbon`): one chain of pieces for each tire while it stays
//! on the ground, a piece every `SEGMENT` metres of its path, each laid flat on the ground
//! between its edges, a little above it (`LIFT`) so that the ground does not show through
//! it. Where the truck is comes from how it is drawn (`TruckVisual`), so that the track
//! comes out from under the tire where the tire is seen. A tire that leaves the ground, or
//! the loose ground, or jumps across the map (`wrap`), starts a new chain. The pieces
//! fade out with age, and the oldest are written over when the slots are full, so that a
//! track is gone in a minute or two.
//!
//! One tread picture serves every tire: Monster Truck Madness 2's tracks are not
//! measured, and the values here are the game's own.

use avian3d::prelude::{AngularVelocity, LinearVelocity};
use bevy::asset::RenderAssetUsages;
use bevy::ecs::entity::EntityHashMap;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use crate::mipmaps::data_mip_chain;

use super::ribbon::{self, Look, Piece, Ribbons};
use crate::game_state::GameState;
use crate::track::Track;
use crate::truck::{TruckConfig, TruckInput, TruckVisual};

/// How near the ground the bottom of a tire must be to press a track into it, in metres:
/// as for the dirt it throws (`dirt::TOUCHING`).
const TOUCHING: f32 = 0.3;
/// How far a tire goes between the edges of its track, in metres. Shorter follows a
/// tight turn closer and costs more pieces; at 1 m, a turn of 8 m radius is 1.6 cm off.
const SEGMENT: f32 = 1.0;
/// How far a tire may go between one edge and the next for them to be joined, in metres.
/// Further is a jump: a truck put back on the course, or moved across a world that
/// repeats, and the track starts again.
const BREAK: f32 = 4.0;
/// How far above the ground a track lies, in metres: enough that the ground, which the
/// track is tested against by depth, does not show through it where a piece crosses a
/// crease between two of the ground's triangles.
const LIFT: f32 = 0.04;
/// The most pieces of track on the ground at once, over all the tires. Past it, the
/// oldest are written over. Each is a four-sided patch in a mesh, drawn whether a piece
/// lies in it or not. At 1 m a piece, 32 tires at 30 m/s lay 960 a second.
const MARK_SLOTS: usize = 32_768;
/// How long a piece of track lasts, in seconds, and how old it is when it starts to fade.
const MARK_LIFE: f32 = 90.0;
const MARK_FADE_FROM: f32 = 40.0;
/// How far from the camera the tracks start to fade out, in metres, and where they are
/// gone: far enough to see the course ahead printed, near enough that the far ground is
/// not peppered.
const SEEN_TO: f32 = 150.0;
const GONE_AT: f32 = 250.0;
/// How many metres of track one height of the tread picture covers: two lugs. A monster
/// truck's lugs are about 30 cm apart.
const TREAD_PERIOD: f32 = 0.6;
/// How much of the ground's colour a track leaves under a lug: the ground pressed and
/// turned over is darker. Lower is darker.
const MARK_SHADE: f32 = 0.55;
/// How strong a track is, from 0 (not there) to 1 (as dark as `MARK_SHADE`): rolling
/// straight, and the most, sliding or spinning.
const MARK_STRENGTH: f32 = 0.55;
const MOST_STRENGTH: f32 = 0.9;
/// How much stronger a track is for each m/s the tire slides sideways, and at full
/// throttle from standing, where it spins (see `dirt::SPIN_SPEED`).
const STRENGTH_PER_SLIDE: f32 = 0.08;
const STRENGTH_SPINNING: f32 = 0.3;
const SPIN_SPEED: f32 = 8.0;
/// How long the patch of tread on the ground is, as a share of the tire's radius: how
/// wide a track a tire sliding straight sideways smears.
const PATCH_LENGTH: f32 = 0.5;

/// Choices about the tracks. Change it at any time.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct TireMarkSettings {
    /// Whether tires leave tracks on loose ground. Tracks on the ground when it is turned
    /// off fade as they would.
    pub on: bool,
}

impl Default for TireMarkSettings {
    fn default() -> Self {
        Self { on: true }
    }
}

/// On the race's ribbons of tire tracks.
#[derive(Component)]
pub struct TireMarks;

/// The tread picture, made once.
#[derive(Resource)]
pub(super) struct TreadLooks(Handle<Image>);

/// The last edge of a tire's track: where the track goes on from.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Edge {
    left: Vec3,
    right: Vec3,
    /// How far along the track it is, in metres.
    along: f32,
}

/// What `lay_tire_marks` remembers of each truck from one frame to the next: per wheel,
/// the last edge of its track, if it is on one.
#[derive(Default)]
pub(super) struct Tracks([Option<Edge>; 4]);

pub(super) fn make_looks(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(TreadLooks(images.add(tread_picture())));
}

/// Makes the race's ribbons of tire tracks.
pub(super) fn spawn_tire_marks(
    mut commands: Commands,
    looks: Res<TreadLooks>,
    mut assets: ribbon::RibbonAssets,
) {
    let ribbons = ribbon::ribbons(
        &mut assets,
        MARK_SLOTS,
        Look {
            life: MARK_LIFE,
            fade_from: MARK_FADE_FROM,
            period: TREAD_PERIOD,
            seen_to: SEEN_TO,
            gone_at: GONE_AT,
        },
        looks.0.clone(),
    );
    commands.spawn((ribbons, TireMarks, DespawnOnExit(GameState::Racing)));
}

pub(super) fn lay_tire_marks(
    time: Res<Time>,
    track: Res<Track>,
    settings: Res<TireMarkSettings>,
    trucks: Query<(Entity, &Transform, &TruckVisual)>,
    bodies: Query<(
        &LinearVelocity,
        &AngularVelocity,
        &TruckConfig,
        Option<&TruckInput>,
    )>,
    mut marks: Query<&mut Ribbons, With<TireMarks>>,
    mut tracks: Local<EntityHashMap<Tracks>>,
) {
    let Ok(mut marks) = marks.single_mut() else {
        return;
    };
    let now = ribbon::clock(&time);
    // Trucks from a race that is over are forgotten.
    tracks.retain(|visual, _| trucks.contains(*visual));

    for (visual, transform, truck) in &trucks {
        let Ok((linear, angular, config, input)) = bodies.get(truck.truck) else {
            continue;
        };
        let throttle = input.map_or(0.0, |input| input.throttle);
        let tracks = tracks.entry(visual).or_default();
        let up = transform.up().as_vec3();
        let right = transform.right().as_vec3();
        let forward = transform.forward().as_vec3();
        for (index, rest) in config.wheel_rest.iter().enumerate() {
            let hub = transform.transform_point(*rest);
            let bottom = hub - up * config.wheel_radius;
            let height = track.heights.height_at(bottom.x, bottom.z);
            let under_water = track.water_level.is_some_and(|level| level > height);
            let on_the_ground = (bottom.y - height).abs() < TOUCHING && up.y > 0.5;
            if !settings.on
                || !on_the_ground
                || under_water
                || !track.loose_at(bottom.x, bottom.z)
            {
                tracks.0[index] = None;
                continue;
            }
            let centre = Vec3::new(bottom.x, height, bottom.z);
            let moving = linear.0 + angular.0.cross(hub - transform.translation);
            let last = tracks.0[index];
            let Some(way) = next_edge_way(last, centre) else {
                // A new track starts here, across the tire.
                if last.is_none() || last.is_some_and(|last| last.far_from(centre)) {
                    tracks.0[index] = Some(edge_at(centre, right, config.wheel_width, 0.0));
                }
                continue;
            };
            let last = last.expect("a way goes on from an edge");
            let along = last.along + way.length();
            let across = Vec3::Y.cross(way).normalize_or_zero();
            let width = mark_width(config, way.normalize_or_zero(), forward);
            let edge = edge_at(centre, across, width, along);
            let on_ground = |corner: Vec3| {
                Vec3::new(
                    corner.x,
                    track.heights.height_at(corner.x, corner.z) + LIFT,
                    corner.z,
                )
            };
            let slide = moving.dot(right).abs();
            let strength = mark_strength(moving.xz().length(), slide, throttle);
            marks.lay(
                now,
                Piece {
                    corners: [
                        on_ground(last.left),
                        on_ground(last.right),
                        on_ground(edge.left),
                        on_ground(edge.right),
                    ],
                    along: [last.along, along],
                    color: LinearRgba::new(MARK_SHADE, MARK_SHADE, MARK_SHADE, strength),
                },
            );
            tracks.0[index] = Some(edge);
        }
    }
}

impl Edge {
    /// The middle of the edge.
    fn centre(&self) -> Vec3 {
        (self.left + self.right) / 2.0
    }

    /// Whether `at` is too far from this edge to be the next one of the same track.
    fn far_from(&self, at: Vec3) -> bool {
        self.centre().xz().distance(at.xz()) > BREAK
    }
}

/// The way from the `last` edge of a track, if there is one, to the next, which is to
/// be laid at `centre`: level, and only once the tire has gone `SEGMENT` since the last,
/// and not so far that the track breaks (`BREAK`). Nothing to lay otherwise.
fn next_edge_way(last: Option<Edge>, centre: Vec3) -> Option<Vec3> {
    let last = last?;
    let way = (centre - last.centre()).with_y(0.0);
    let gone = way.length();
    (SEGMENT..=BREAK).contains(&gone).then_some(way)
}

/// An edge of a track `width` metres wide across `across` at `centre`, `along` metres
/// along the track.
fn edge_at(centre: Vec3, across: Vec3, width: f32, along: f32) -> Edge {
    let half = across * width / 2.0;
    Edge {
        left: centre - half,
        right: centre + half,
        along,
    }
}

/// How wide a track a tire leaves going `way` (level, of length 1) on a truck facing
/// `forward`, in metres: its tread's width rolling straight, and the length of the patch
/// of tread on the ground sliding straight sideways, between the two as the tire goes.
fn mark_width(config: &TruckConfig, way: Vec3, forward: Vec3) -> f32 {
    let straight = way.dot(forward.with_y(0.0).normalize_or_zero()).abs().min(1.0);
    let sideways = (1.0 - straight * straight).sqrt();
    config.wheel_width * straight + PATCH_LENGTH * config.wheel_radius * sideways
}

/// How strong a track is, from a tire going `speed` m/s over the ground, sliding `slide`
/// m/s sideways, at `throttle` (-1 to 1).
fn mark_strength(speed: f32, slide: f32, throttle: f32) -> f32 {
    let spinning = throttle.abs() * (1.0 - speed / SPIN_SPEED).max(0.0) * STRENGTH_SPINNING;
    (MARK_STRENGTH + slide * STRENGTH_PER_SLIDE + spinning).min(MOST_STRENGTH)
}

/// How many texels the tread picture has along each side. Square, so that it can have
/// mipmaps; it holds two lugs, one above the other.
const PICTURE_SIZE: usize = 64;

/// The tread picture: how dark the track is at each point, in the red channel. Repeats
/// down the track, so its top joins its bottom.
fn tread_picture() -> Image {
    let rgba = tread_texels(PICTURE_SIZE);
    let chain = data_mip_chain(&rgba, PICTURE_SIZE);
    let levels = chain.len() as u32;
    let mut image = Image {
        data: Some(chain.into_iter().flatten().collect()),
        asset_usage: RenderAssetUsages::RENDER_WORLD,
        sampler: ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::Repeat,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            ..default()
        }),
        ..default()
    };
    image.texture_descriptor.size = Extent3d {
        width: PICTURE_SIZE as u32,
        height: PICTURE_SIZE as u32,
        depth_or_array_layers: 1,
    };
    image.texture_descriptor.dimension = TextureDimension::D2;
    image.texture_descriptor.mip_level_count = levels;
    // Numbers, not colours.
    image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    image
}

fn tread_texels(size: usize) -> Vec<u8> {
    let mut texels = Vec::with_capacity(size * size * 4);
    for row in 0..size {
        for col in 0..size {
            let point = Vec2::new(
                (col as f32 + 0.5) / size as f32,
                (row as f32 + 0.5) / size as f32,
            );
            let dark = (tread_darkness(point).clamp(0.0, 1.0) * 255.0).round() as u8;
            texels.extend_from_slice(&[dark, dark, dark, 255]);
        }
    }
    texels
}

/// How dark a track is `point` across (0 to 1, left to right) and down (0 to 1, which
/// repeats) the tread picture: full under a lug, less between them, where the tire's
/// body still presses the ground, and nothing past the tread's edges. The lugs are
/// chevrons, two to a picture.
fn tread_darkness(point: Vec2) -> f32 {
    /// How much of the width the tread's edges are soft over, on each side.
    const EDGE: f32 = 0.12;
    /// How much of a lug's spacing the lug takes up, and how far the chevron's point leads
    /// its ends, as a share of the spacing.
    const LUG: f32 = 0.45;
    const CHEVRON: f32 = 0.3;
    /// How dark the track is between the lugs.
    const BETWEEN: f32 = 0.45;
    let side = (point.x - 0.5).abs() * 2.0;
    let edge = smoothstep(1.0, 1.0 - EDGE, side);
    // Two lugs down the picture: each has its point in the middle, ahead of its ends.
    let down = (point.y * 2.0 + CHEVRON * side).fract();
    let lug = smoothstep(LUG / 2.0 + 0.04, LUG / 2.0, (down - 0.5).abs());
    edge * (BETWEEN + (1.0 - BETWEEN) * lug)
}

fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_track_goes_on_a_segment_at_a_time_and_breaks_at_a_jump() {
        let start = edge_at(Vec3::ZERO, Vec3::X, 0.7, 0.0);
        assert_eq!(start.left, Vec3::new(-0.35, 0.0, 0.0));
        assert_eq!(start.right, Vec3::new(0.35, 0.0, 0.0));
        // Not yet a segment on: nothing.
        assert_eq!(next_edge_way(Some(start), Vec3::new(0.0, 0.0, -0.5)), None);
        // A segment on: the way there, level.
        let way = next_edge_way(Some(start), Vec3::new(0.0, 0.3, -1.2)).unwrap();
        assert_eq!(way, Vec3::new(0.0, 0.0, -1.2));
        // A jump: nothing, and the edge is far from it.
        let far = Vec3::new(0.0, 0.0, -BREAK - 1.0);
        assert_eq!(next_edge_way(Some(start), far), None);
        assert!(start.far_from(far));
        assert!(!start.far_from(Vec3::new(0.0, 0.0, -1.2)));
        // Without a track there is nothing to go on from.
        assert_eq!(next_edge_way(None, Vec3::ZERO), None);
    }

    #[test]
    fn a_track_is_as_wide_as_the_tread_rolling_straight_and_as_the_patch_sideways() {
        let config = TruckConfig::default();
        let forward = Vec3::NEG_Z;
        assert_eq!(mark_width(&config, Vec3::NEG_Z, forward), config.wheel_width);
        assert_eq!(mark_width(&config, Vec3::Z, forward), config.wheel_width);
        let sideways = mark_width(&config, Vec3::X, forward);
        assert!((sideways - PATCH_LENGTH * config.wheel_radius).abs() < 1e-5);
    }

    #[test]
    fn sliding_and_spinning_press_a_darker_track_up_to_a_limit() {
        let rolling = mark_strength(10.0, 0.0, 0.0);
        assert_eq!(rolling, MARK_STRENGTH);
        assert!(mark_strength(10.0, 3.0, 0.0) > rolling);
        assert!(mark_strength(0.0, 0.0, 1.0) > rolling);
        assert_eq!(mark_strength(SPIN_SPEED, 0.0, 1.0), rolling);
        assert_eq!(mark_strength(100.0, 100.0, 1.0), MOST_STRENGTH);
    }

    #[test]
    fn the_tread_picture_is_dark_in_the_middle_and_clear_at_the_edges() {
        let middle = tread_darkness(Vec2::new(0.5, 0.25));
        assert!(middle > 0.9, "{middle}");
        assert_eq!(tread_darkness(Vec2::new(0.0, 0.25)), 0.0);
        assert_eq!(tread_darkness(Vec2::new(1.0, 0.25)), 0.0);
        // Between the lugs the track is lighter, and not gone.
        let between = tread_darkness(Vec2::new(0.5, 0.0));
        assert!(between > 0.3 && between < middle, "{between}");
        // The picture's rows are a repeat: the top joins the bottom.
        let top = tread_darkness(Vec2::new(0.5, 0.001));
        let bottom = tread_darkness(Vec2::new(0.5, 0.999));
        assert!((top - bottom).abs() < 0.05, "{top} {bottom}");
        let texels = tread_texels(8);
        assert_eq!(texels.len(), 8 * 8 * 4);
    }
}
