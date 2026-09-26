// The water's surface. An ordinary PBR one, lit and coloured by its standard material,
// except that waves move it and bend its normal, so that the light off it moves.
//
// Three kinds of wave:
// - The swell: long waves rolling downwind, which really move the surface up and down on
//   the patch round the camera (see `surface.rs`), and only bend the light further off.
// - The chop: short waves on top, which only bend the light. The wind raises them, and
//   more in its gusts, which sweep over the water as dark, ruffled patches (cat's paws).
//   In a strong gust the swell's crests break into whitecaps.
// - Rings that spread out from where trucks disturb the water (`ripples.rs`), each a
//   short train of waves that widens and fades as it goes, with foam on its crests while
//   it is young.
//
// And where the water is shallow, from a map of its depth (`shore.rs`): foam along the
// water's edge, which moves up and down the ground with the swell, lines of surf that roll
// in towards it, and foam where the rings break in the shallows.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    mesh_view_bindings::{globals, view},
    mesh_functions,
    view_transformations::position_world_to_clip,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{Vertex, VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

struct Ripples {
    // Per ripple: where it started (X, Z), when on `globals.time`, in seconds, and its
    // strength in metres.
    // As long as `MAX_RIPPLES` in `ripples.rs`.
    ripples: array<vec4<f32>, 48>,
    count: u32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<storage, read> water: Ripples;
// The sky's colour, in linear light.
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> sky: vec4<f32>;

struct Wind {
    // Which way it blows, on the ground plane (world X and Z), of length 1. How hard, is
    // `wind_speed`.
    towards: vec2<f32>,
    // 1 on the patch round the camera, 0 on the sheet everywhere else.
    round_camera: u32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> wind: Wind;

struct Shore {
    // Where the depth map's first texel is (world X and Z), in metres.
    origin: vec2<f32>,
    // How far apart its texels are, in metres, and how many there are along a side.
    spacing: f32,
    resolution: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(103) var<uniform> shore: Shore;
// How deep the still water is over the ground, in metres. 32-bit, so not filterable.
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var depth_map: texture_2d<f32>;

// As in `surface.rs`: how far the patch round the camera reaches, and how far apart its
// vertices are, in metres.
const PATCH_SIZE: f32 = 240.0;
const PATCH_SPACING: f32 = 1.5;
// How much of the way out from its middle the patch starts to ease its waves flat, so
// that it meets the flat sheet round it without a step. Past `PATCH_SIZE` it is flat.
const PATCH_EASE: f32 = 0.7;

// The swell, three long waves rolling downwind, each at an angle to the wind (radians),
// with its length and the height of its crests above the still water (metres). Its
// highest crest is the sum of the three.
const SWELL_ANGLES: vec3<f32> = vec3(0.0, 0.45, -0.6);
const SWELL_LENGTHS: vec3<f32> = vec3(16.0, 10.5, 6.5);
const SWELL_HEIGHTS: vec3<f32> = vec3(0.26, 0.15, 0.08);
// The wind that the chop is as written for, in m/s. Harder raises it in proportion.
const CHOP_WIND: f32 = 6.0;
// How big the gusts' patches on the water are, in metres, and how fast they sweep
// downwind, in m/s.
const GUST_SIZE: f32 = 45.0;
const GUST_DRIFT: f32 = 5.0;
// How much of its mirrored sky ruffled water loses. Ruffled water looks darker.
const GUST_DARKENING: f32 = 0.45;
// How hard the wind must blow for the swell's crests to break, in m/s: they start to at
// the first, and all do at the second.
const WHITECAP_WIND: vec2<f32> = vec2(7.5, 9.5);
// How hard the wind blows, in m/s, and how far its gusts take it either side of that, and
// how long they take to come and go, in seconds. As in `wind.rs`, which a test checks.
const WIND_SPEED: f32 = 6.0;
const GUSTINESS: f32 = 3.5;
const GUST_PERIODS: vec3<f32> = vec3(11.0, 4.7, 2.3);

const TAU: f32 = 6.2831853;
const GRAVITY: f32 = 9.81;

// How fast a ring spreads, in m/s. Real ripples of about a metre go at 1 to 2 m/s. A little
// quicker reads better at the speed trucks go.
const RIPPLE_SPEED: f32 = 3.0;
// The length of the waves in a ring, in metres.
const RIPPLE_WAVELENGTH: f32 = 0.8;
// How wide a ring's train of waves is when it starts, in metres, and how much wider it
// grows for each metre it spreads. A train a few wavelengths wide is a few thin crests,
// as real ripples are, rather than one broad band.
const RIPPLE_WIDTH: f32 = 1.1;
const RIPPLE_SPREAD: f32 = 0.15;
// How long a ring takes to fade to a third, in seconds.
const RIPPLE_FADE: f32 = 1.2;
// How much a ring thins for each metre it spreads, as it shares itself round a longer
// circle.
const RIPPLE_THINNING: f32 = 0.25;
// How much foam a ring's crests carry for their height, and how soon it goes, in seconds
// (to a third). Foam is churned up where the water is broken, and does not travel far.
const FOAM: f32 = 3.0;
const FOAM_FADE: f32 = 0.5;
// How big the patches of foam are, in metres. Foam is never an even band.
const FOAM_PATCH: f32 = 0.35;
// How much of the sky the water mirrors, seen edge on. Looked straight down into, it
// mirrors almost none.
const SKY_MIRROR: f32 = 0.55;
// How deep the water is at the outer edge of the band of foam along the shore, in metres.
const EDGE_FOAM_DEPTH: f32 = 0.45;
// The surf: how deep it starts to roll in, in metres, how much deeper each line is than
// the one ahead of it, and how often a line comes in, in seconds. On a gentle beach the
// lines are far apart, on a steep bank close together.
const SURF_DEPTH: f32 = 1.4;
const SURF_SPACING: f32 = 0.45;
const SURF_PERIOD: f32 = 3.2;
// How much foam there is on a line of surf, in the wind that the chop is written for.
const SURF_FOAM: f32 = 0.8;
// How much more foam a ring's crest carries where it breaks in the shallows, and how
// shallow that is, in metres.
const RING_BREAK: f32 = 30.0;
const RING_BREAK_DEPTH: f32 = 0.8;
// How far off the waves are half as strong, in metres. Far away they are smaller than a
// pixel, and drawn at full strength they would only shimmer.
const WAVE_DISTANCE: f32 = 60.0;

// Smooth noise from 0 to 1, varying over about a unit.
fn hash(cell: vec2<f32>) -> f32 {
    return fract(sin(dot(cell, vec2(127.1, 311.7))) * 43758.5453);
}

fn noise(position: vec2<f32>) -> f32 {
    let cell = floor(position);
    let within = fract(position);
    let blend = within * within * (3.0 - 2.0 * within);
    return mix(
        mix(hash(cell), hash(cell + vec2(1.0, 0.0)), blend.x),
        mix(hash(cell + vec2(0.0, 1.0)), hash(cell + vec2(1.0, 1.0)), blend.x),
        blend.y,
    );
}

// The slope of one long wave, going `direction` at the speed of real deep-water waves of
// its length.
fn wave(position: vec2<f32>, time: f32, direction: vec2<f32>, wavelength: f32, height: f32) -> vec2<f32> {
    let k = TAU / wavelength;
    let speed = sqrt(GRAVITY / k);
    let phase = k * (dot(direction, position) - speed * time);
    return direction * (height * k * cos(phase));
}

// `direction` turned by `angle` radians about the vertical.
fn turned(direction: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec2(c * direction.x - s * direction.y, s * direction.x + c * direction.y);
}

// Where along its travel one wave is, at `position` and `time`, in radians.
fn phase(position: vec2<f32>, time: f32, direction: vec2<f32>, wavelength: f32) -> f32 {
    let k = TAU / wavelength;
    return k * (dot(direction, position) - sqrt(GRAVITY / k) * time);
}

// One wave of the swell at `position`: how high it stands (x), and how it slopes (y, z).
fn swell_wave(position: vec2<f32>, time: f32, angle: f32, wavelength: f32, height: f32) -> vec3<f32> {
    let direction = turned(wind.towards, angle);
    let at = phase(position, time, direction, wavelength);
    let k = TAU / wavelength;
    return vec3(height * sin(at), direction * (height * k * cos(at)));
}

// The swell at `position`: how high it stands above the still water, in metres (x), and
// how much it rises for each metre along X and Z (y, z).
fn swell(position: vec2<f32>, time: f32) -> vec3<f32> {
    return swell_wave(position, time, SWELL_ANGLES.x, SWELL_LENGTHS.x, SWELL_HEIGHTS.x)
        + swell_wave(position, time, SWELL_ANGLES.y, SWELL_LENGTHS.y, SWELL_HEIGHTS.y)
        + swell_wave(position, time, SWELL_ANGLES.z, SWELL_LENGTHS.z, SWELL_HEIGHTS.z);
}

// The chop's slope: short waves across the wind as well as with it.
fn chop(position: vec2<f32>, time: f32) -> vec2<f32> {
    let towards = wind.towards;
    return wave(position, time, turned(towards, 0.3), 4.1, 0.025)
        + wave(position, time, turned(towards, -0.7), 2.7, 0.015)
        + wave(position, time, turned(towards, 1.1), 1.6, 0.008)
        + wave(position, time, turned(towards, -0.2), 1.0, 0.005);
}

// How gusty the wind is over `position` just now, from 0 (a lull) to 1 (a gust): patches
// that sweep downwind.
fn gustiness(position: vec2<f32>, time: f32) -> f32 {
    let swept = position - wind.towards * GUST_DRIFT * time;
    let gust = noise(swept / GUST_SIZE) * 0.65 + noise(swept / (GUST_SIZE * 0.37) + 17.0) * 0.35;
    return smoothstep(0.4, 0.75, gust);
}

// How deep the still water is over the ground at `position`, in metres. Below the ground
// it is less than 0.
fn still_depth(position: vec2<f32>) -> f32 {
    let last = i32(shore.resolution) - 1;
    let texels = clamp((position - shore.origin) / shore.spacing, vec2(0.0), vec2(f32(last)));
    let corner = min(vec2<i32>(floor(texels)), vec2(last - 1));
    let within = texels - vec2<f32>(corner);
    let near = mix(
        textureLoad(depth_map, corner, 0).r,
        textureLoad(depth_map, corner + vec2(1, 0), 0).r,
        within.x,
    );
    let far = mix(
        textureLoad(depth_map, corner + vec2(0, 1), 0).r,
        textureLoad(depth_map, corner + vec2(1, 1), 0).r,
        within.x,
    );
    return mix(near, far, within.y);
}

// How far the swell moves the patch's vertices up and down at a point `out_from_middle` of
// the way from its middle to its edge: all the way, easing to none.
fn patch_ease(out_from_middle: f32) -> f32 {
    return 1.0 - smoothstep(PATCH_EASE, 1.0, out_from_middle);
}

// Where the middle of the patch round the camera is: under the camera, on the grid of its
// vertices.
fn patch_middle() -> vec2<f32> {
    return round(view.world_position.xz / PATCH_SPACING) * PATCH_SPACING;
}

#ifndef PREPASS_PIPELINE
@vertex
fn vertex(mesh_vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(mesh_vertex.instance_index);
    var world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4(mesh_vertex.position, 1.0),
    );
    if wind.round_camera == 1u {
        world_position = vec4(world_position.xyz + vec3(patch_middle().x, 0.0, patch_middle().y), 1.0);
        let out_from_middle = max(abs(mesh_vertex.position.x), abs(mesh_vertex.position.z)) / (PATCH_SIZE / 2.0);
        let ease = patch_ease(out_from_middle);
        world_position.y += swell(world_position.xz, globals.time).x * ease;
    }
    out.world_position = world_position;
    out.position = position_world_to_clip(world_position.xyz);
    // The fragment shader works out the normal for itself.
    out.world_normal = vec3(0.0, 1.0, 0.0);
#ifdef VERTEX_UVS_A
    out.uv = mesh_vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = mesh_vertex.uv_b;
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        mesh_vertex.tangent,
        mesh_vertex.instance_index,
    );
#endif
#ifdef VERTEX_COLORS
    out.color = mesh_vertex.color;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = mesh_vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        mesh_vertex.instance_index,
        world_from_local[3],
    );
#endif
    return out;
}
#endif

// How hard the wind blows at `time`, in m/s. As the speed of `Wind::air` in `wind.rs`.
fn wind_speed(time: f32) -> f32 {
    let swing = sin(TAU * time / GUST_PERIODS);
    let slow = swing.x;
    let middle = swing.y;
    let quick = swing.z;
    let gust = 0.55 * slow + 0.3 * middle + 0.15 * quick;
    return WIND_SPEED + GUSTINESS * gust;
}

// The slope of one ring at `position` (x and y), the foam on it (z), and how high its
// crest stands there (w), for foam where it breaks in the shallows.
fn ring(position: vec2<f32>, ripple: vec4<f32>) -> vec4<f32> {
    let offset = position - ripple.xy;
    let from_center = length(offset);
    let age = globals.time - ripple.z;
    let radius = RIPPLE_SPEED * age;
    let width = RIPPLE_WIDTH + RIPPLE_SPREAD * radius;
    let across = from_center - radius;
    // Most of the water is nowhere near most rings.
    if abs(across) > 3.0 * width || from_center < 0.001 {
        return vec4(0.0);
    }
    let height = ripple.w * exp(-age / RIPPLE_FADE) / (1.0 + RIPPLE_THINNING * radius);
    let envelope = exp(-(across * across) / (width * width));
    let k = TAU / RIPPLE_WAVELENGTH;
    // The ring's height is height * envelope * cos(k * across); this is how it changes
    // going outwards.
    let outwards = height * envelope
        * (-2.0 * across / (width * width) * cos(k * across) - k * sin(k * across));
    let crest = height * envelope * max(cos(k * across), 0.0);
    return vec4(offset / from_center * outwards, crest * exp(-age / FOAM_FADE), crest);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    let position = in.world_position.xz;
    // The sheet leaves the water round the camera to the patch, and the patch leaves the
    // rest to the sheet, by the same test (see `surface.rs`).
    let from_middle = abs(position - patch_middle());
    let in_patch = max(from_middle.x, from_middle.y) < PATCH_SIZE / 2.0;
    if in_patch != (wind.round_camera == 1u) {
        discard;
    }

    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let time = globals.time;
    let far = distance(view.world_position, in.world_position.xyz);
    let nearness = WAVE_DISTANCE / (WAVE_DISTANCE + far);
    let gust = gustiness(position, time);
    let wind_now = wind_speed(time);
    let breeze = wind_now / CHOP_WIND;

    let swell_here = swell(position, time);
    // How deep the water is here, as it is drawn: on the patch, the swell moves it.
    var depth = still_depth(position);
    if wind.round_camera == 1u {
        depth += swell_here.x * patch_ease(max(from_middle.x, from_middle.y) / (PATCH_SIZE / 2.0));
    }
    let breaking = 1.0 - smoothstep(0.0, RING_BREAK_DEPTH, depth);
    var slope = swell_here.yz + chop(position, time) * breeze * mix(0.5, 1.8, gust);
    var foam = 0.0;
    for (var index = 0u; index < water.count; index++) {
        let found = ring(position, water.ripples[index]);
        slope += found.xy;
        foam += found.z + found.w * RING_BREAK * breaking;
    }
    slope *= nearness;

    var normal = normalize(vec3(-slope.x, 1.0, -slope.y));
    // Seen from under the water, the surface faces down.
    if !is_front {
        normal = -normal;
    }
    pbr_input.N = normal;
    pbr_input.world_normal = normal;

    let patches = noise(position / FOAM_PATCH) * 0.6 + noise(position / (FOAM_PATCH * 3.1)) * 0.4;
    // Whitecaps: the swell's highest crests, where a strong gust is blowing.
    let crest = swell_here.x / (SWELL_HEIGHTS.x + SWELL_HEIGHTS.y + SWELL_HEIGHTS.z);
    let whitecaps = smoothstep(0.6, 0.9, crest) * gust
        * smoothstep(WHITECAP_WIND.x, WHITECAP_WIND.y, wind_now);
    // The shore: a band of foam along the water's edge, never quite even, and lines of surf
    // rolling in to it, which come in at constant depths, so run along the shore.
    let edge = (1.0 - smoothstep(0.0, EDGE_FOAM_DEPTH, depth)) * (0.7 + 0.5 * patches);
    let surf_line = pow(max(cos(TAU * (depth / SURF_SPACING + time / SURF_PERIOD)), 0.0), 6.0);
    let surf = surf_line * (1.0 - smoothstep(0.2, SURF_DEPTH, depth)) * SURF_FOAM
        * min(breeze, 1.5) * mix(0.7, 1.3, gust) * mix(0.5, 1.1, patches);
    let whiteness = clamp(
        (foam * FOAM + whitecaps) * smoothstep(0.35, 0.7, patches) + edge + surf,
        0.0,
        0.85,
    );
    // Water looked into from above is clear, and seen at a glancing angle mirrors the sky
    // and hides what is under it (Fresnel).
    let facing = abs(dot(normal, pbr_input.V));
    let glancing = pow(1.0 - facing, 3.0);
    let color = pbr_input.material.base_color;
    pbr_input.material.base_color = vec4(
        mix(color.rgb, vec3(0.95, 0.97, 1.0), whiteness),
        max(mix(color.a, 0.95, glancing), whiteness),
    );

    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    // Foam is white whatever it mirrors.
    let ruffled = 1.0 - GUST_DARKENING * gust * min(breeze, 1.5);
    let mirror = SKY_MIRROR * glancing * (1.0 - whiteness) * ruffled;
    out.color = vec4(mix(out.color.rgb, sky.rgb, mirror), out.color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
