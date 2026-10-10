// The water's surface. An ordinary PBR one, lit and coloured by its standard material,
// except that waves move it and bend its normal, so that the light off it moves.
//
// Three kinds of wave:
// - The swell: long waves rolling downwind, which really move the surface up and down on
//   the patch round the camera (see `surface.rs`), and only bend the light further off.
// - The chop: short waves on top, which only bend the light. The wind raises them, and
//   more in its gusts, which sweep over the water as dark, ruffled patches (cat's paws).
//   In a strong gust the swell's crests break into whitecaps.
// - The field of heights round the camera that the trucks disturb (`field.rs`): their
//   bow waves, wakes and rings, which the graphics card spreads, and the foam they churn
//   up, which lies on the water and fades. It really moves the surface on the patch, and
//   bends the light and whitens the water everywhere it is shown.
//
// And where the water is shallow, from a map of its depth (`shore.rs`): foam along the
// water's edge, which moves up and down the ground with the swell, lines of surf that roll
// in towards it, and foam where the rings break in the shallows.
//
// At a glancing angle it mirrors what is round it. The reflected ray is followed across
// the picture drawn so far, in steps that grow longer as it goes, until it goes behind
// something drawn there, which it shows (screen-space reflections). A ray that leaves the
// picture first shows what is at the edge where it left: what it would have met past the
// edge is not in the picture, and is most often more of the same, a cliff that goes on up.
// The sky's colour there showed as bright patches under dark cliffs. A ray that meets
// nothing shows the sky's colour. The water is drawn solid after everything else solid,
// and mixes in the picture under it itself (see `surface.rs`).

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
    mesh_view_bindings::{view_transmission_texture, view_transmission_sampler},
    view_transformations::{depth_ndc_to_view_z, frag_coord_to_uv, ndc_to_uv},
}
#ifdef DEPTH_PREPASS
#import bevy_pbr::prepass_utils::prepass_depth
#endif
#endif

// The field of heights (`field.wgsl`): per texel the height, in metres, how much it
// rises for each metre along X and Z, and the foam. Texel `i` holds every cell of the
// world whose index is `i` modulo `FIELD_TEXELS`, so it is read round and round.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var field: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var field_sampler: sampler;
struct Mirroring {
    // The sky's colour, in linear light.
    sky: vec4<f32>,
    // 1 to mirror the scenery as well, 0 for only the sky (`WaterSettings::reflections`).
    scenery: u32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> mirroring: Mirroring;

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
    // 1 where the ground repeats beyond the edges of the map.
    repeats: u32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(103) var<uniform> shore: Shore;
// How deep the still water is over the ground, in metres. 32-bit, so not filterable.
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var depth_map: texture_2d<f32>;

// As in `surface.rs`: how far the patch round the camera reaches, and how far apart its
// vertices are, in metres; and the same for the fine patch ahead of the camera, round the
// truck, which shows the bow wave and the wake as a shape, and how far ahead of the camera
// its middle is.
const PATCH_SIZE: f32 = 240.0;
const PATCH_SPACING: f32 = 1.5;
const FINE_SIZE: f32 = 36.0;
const FINE_SPACING: f32 = 0.5;
const FINE_AHEAD: f32 = 10.0;
// How much of the way out from the fine patch's middle to its edge its surface starts to
// ease towards the coarse patch's, which it meets at the edge without a crack.
const FINE_EASE: f32 = 0.8;
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
// How much of what it mirrors ruffled water loses. Ruffled water looks darker.
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

// As in `field.rs`: how many texels the field's window round the camera is along a side,
// and how far apart they are, in metres.
const FIELD_TEXELS: f32 = 512.0;
const FIELD_SPACING: f32 = 0.5;
// How much of the way out from the camera to the window's edge the field is shown in
// full. Past that it fades to nothing by the edge, where the window comes round.
const FIELD_SHOWN: f32 = 0.8;
// How white the field's foam is, for how much there is.
const FIELD_FOAM: f32 = 1.0;
// How far the foam's lumps swirl about, in metres, and how long one swirl takes, in
// seconds: the foam turns over on the water, and is never the same picture twice.
const FOAM_SWIRL: f32 = 0.5;
const FOAM_SWIRL_PERIOD: f32 = 6.0;
// How much of the foam's whiteness is left between its lumps where the foam is thin: the
// dark water shows through there. Where it is thick, from `SOLID_FOAM` on, the gaps close:
// churned water is white all over.
const FOAM_GAPS: f32 = 0.3;
const SOLID_FOAM: vec2<f32> = vec2(0.8, 1.4);
// What the water is like where it is shallow: its colour, in linear light, lighter and
// greener than deep water (`WATER_COLOR` in `surface.rs`), and how deep it is where the
// deep colour has taken over, in metres.
const SHALLOW_COLOR: vec3<f32> = vec3(0.02, 0.12, 0.11);
const SHALLOW_DEPTH: f32 = 2.5;
// How rough foam is, from 0 (as smooth as the water) to 1: foam does not glint.
const FOAM_ROUGHNESS: f32 = 0.7;
// How much the field's waves bend the light, for how steep they are: more than they
// are, so that a low wake still catches the light.
const FIELD_SLOPE: f32 = 2.5;
// How soft the edge of the field's foam is: a patch of foam covers the water where the
// foam's noise is under how much foam there is, over this much of the noise.
const FOAM_EDGE: f32 = 0.3;
// How much the lumps of foam stand up out of the water, as the light shades them. Flat
// foam is a white blanket.
const FOAM_BUMP: f32 = 4.0;
// The bump is found over a few centimetres of the foam's noise rather than over a pixel,
// which is steeper: this brings it to about what a pixel's gave.
const FOAM_BUMP_SCALE: f32 = 0.02;
// How big the patches of foam are, in metres. Foam is never an even band.
const FOAM_PATCH: f32 = 0.35;
// How much of what is round it the water mirrors, seen edge on. Looked straight down
// into, it mirrors almost none.
const MIRROR: f32 = 0.25;
// How much of what is round it the water must mirror for the reflected ray to be
// followed; less shows the sky's colour. Higher saves the cost of following it on more
// of the water seen from above, where the reflection hardly shows.
const LEAST_REFLECTION: f32 = 0.05;
// How much the waves bend what the water mirrors, from 0 (a still mirror); at 1 as much
// as they bend the light off it, and above that more. Higher is a more broken reflection.
const REFLECTION_WAVINESS: f32 = 1.6;
// What the scenery the water mirrors is multiplied by, in linear light: water mirrors
// it bluer than it is. Lower red and green is bluer.
const REFLECTION_TINT: vec3<f32> = vec3(0.7, 0.84, 1.0);
// How much of the scenery the water mirrors is lost in haze, the sky's colour, from 0 (a
// sharp mirror) to 1 (only the sky). Real water never mirrors as sharply as glass.
const REFLECTION_HAZE: f32 = 0.55;
// How the reflected ray is followed: its first step, in metres, how much longer each step
// is than the one before, and how many there are. 16 steps from 0.3 m reach about 400 m.
// Each step is a read of the depth for every pixel of water that mirrors: more steps, or
// slower growth, find thinner things further off, at that cost.
const REFLECTION_FIRST_STEP: f32 = 0.3;
const REFLECTION_GROWTH: f32 = 1.5;
const REFLECTION_STEPS: u32 = 16u;
// How many times the step in which the ray went behind something is halved, to find
// where it did. Fewer leaves the reflection's edges ragged.
const REFLECTION_HALVINGS: u32 = 4u;
// How thick things are taken to be behind what is drawn of them, in metres, on top of
// the length of a step. A ray that is further behind than that has passed behind them,
// not met them. Thinner leaves holes in the reflections of steep things.
const REFLECTION_THICKNESS: f32 = 1.0;
// How far the view goes through the water before it hides almost two thirds of what is
// under it, in metres. Higher is clearer water: a ford stays plain to see deeper down.
const WATER_CLARITY: f32 = 0.3;
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
// How much foam a crest of the field carries for its height where it breaks in the
// shallows, and how shallow that is, in metres.
const RING_BREAK: f32 = 30.0;
const RING_BREAK_DEPTH: f32 = 0.8;
// How far off the waves are half as strong, in metres. Far away they are smaller than a
// pixel, and drawn at full strength they would only shimmer.
const WAVE_DISTANCE: f32 = 60.0;

// Smooth noise from 0 to 1, varying over about a unit.
fn hash(cell: vec2<f32>) -> f32 {
    // Within a few hundred of 0: `sin` of a large number is not to be trusted.
    let near = cell - floor(cell / 289.0) * 289.0;
    return fract(sin(dot(near, vec2(127.1, 311.7))) * 43758.5453);
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
// it is less than 0. Past an edge of ground that repeats, as deep as inside the other.
fn still_depth(position: vec2<f32>) -> f32 {
    let last = i32(shore.resolution) - 1;
    var from_origin = position - shore.origin;
    if shore.repeats != 0u {
        let size = shore.spacing * f32(last);
        from_origin -= floor(from_origin / size) * size;
    }
    let texels = clamp(from_origin / shore.spacing, vec2(0.0), vec2(f32(last)));
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

// The field at `position`: its height (x), how it slopes along X and Z (y, z) and its
// foam (w), fading to nothing towards the edge of its window round the camera.
fn field_at(position: vec2<f32>) -> vec4<f32> {
    let out = max(
        abs(position.x - view.world_position.x),
        abs(position.y - view.world_position.z),
    ) / (FIELD_TEXELS * FIELD_SPACING / 2.0);
    let shown = 1.0 - smoothstep(FIELD_SHOWN, 1.0, out);
    if shown <= 0.0 {
        return vec4(0.0);
    }
    let uv = position / (FIELD_TEXELS * FIELD_SPACING);
    return textureSampleLevel(field, field_sampler, uv, 0.0) * shown;
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

// Where the middle of the fine patch is: `FINE_AHEAD` metres ahead of the camera, where
// the truck it follows is, on the coarse patch's grid, so that the fine patch's edge lies
// along the coarse patch's rows of vertices.
fn fine_middle() -> vec2<f32> {
    // The view's own Z points backwards.
    let back = view.world_from_view[2].xz;
    var ahead = vec2(0.0);
    if length(back) > 0.1 {
        ahead = -normalize(back);
    }
    return round((view.world_position.xz + ahead * FINE_AHEAD) / PATCH_SPACING) * PATCH_SPACING;
}

// How high the moving water stands over its level at `position`, in metres: the swell
// and the field.
fn lift(position: vec2<f32>) -> f32 {
    return swell(position, globals.time).x + field_at(position).x;
}

// How high the coarse patch's surface stands at `position`: its vertices are `PATCH_SPACING`
// apart on the grid, and straight between them.
fn coarse_lift(position: vec2<f32>) -> f32 {
    let corner = floor(position / PATCH_SPACING) * PATCH_SPACING;
    let within = (position - corner) / PATCH_SPACING;
    let near = mix(lift(corner), lift(corner + vec2(PATCH_SPACING, 0.0)), within.x);
    let far = mix(
        lift(corner + vec2(0.0, PATCH_SPACING)),
        lift(corner + vec2(PATCH_SPACING, PATCH_SPACING)),
        within.x,
    );
    return mix(near, far, within.y);
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
        world_position.y += lift(world_position.xz) * ease;
    } else if wind.round_camera == 2u {
        world_position = vec4(world_position.xyz + vec3(fine_middle().x, 0.0, fine_middle().y), 1.0);
        let out_from_middle = max(abs(mesh_vertex.position.x), abs(mesh_vertex.position.z)) / (FINE_SIZE / 2.0);
        // Its own surface in the middle, the coarse patch's at the edge. The fine patch is
        // well inside where the coarse one eases flat.
        let to_coarse = smoothstep(FINE_EASE, 1.0, out_from_middle);
        world_position.y += mix(lift(world_position.xz), coarse_lift(world_position.xz), to_coarse);
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

#ifndef PREPASS_PIPELINE
#ifdef DEPTH_PREPASS
// The reflected ray, as the picture sees it: where it starts in clip space and how that
// changes for each metre along it, and the same for how far it is ahead of the camera,
// along the view (view space's Z). Both change in a straight line along the ray, so are
// worked out once for it, and each step only adds to them.
struct Ray {
    clip_start: vec4<f32>,
    clip_way: vec4<f32>,
    view_start: f32,
    view_way: f32,
}

// Where the ray is `along` metres out, in the picture drawn so far, and how far it is
// behind what is drawn there, in metres along the view: below 0 in front of it, or where
// only the sky is.
struct Probe {
    uv: vec2<f32>,
    // Whether it is in front of the camera.
    ahead: bool,
    on_screen: bool,
    behind: f32,
}

fn probe(ray: Ray, along: f32) -> Probe {
    var out: Probe;
    let clip = ray.clip_start + ray.clip_way * along;
    let ndc = clip.xy / clip.w;
    out.uv = ndc_to_uv(ndc);
    out.ahead = clip.w > 0.0;
    out.on_screen = out.ahead && all(abs(ndc) < vec2(1.0));
    out.behind = -1.0e9;
    if out.on_screen {
        let drawn = prepass_depth(vec4(out.uv * view.viewport.zw + view.viewport.xy, 0.0, 0.0), 0u);
        // A depth of 0 is the sky, as far off as can be.
        if drawn > 0.0 {
            out.behind = depth_ndc_to_view_z(drawn) - (ray.view_start + ray.view_way * along);
        }
    }
    return out;
}

// What is drawn at `uv`, in linear light.
fn drawn_at(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(view_transmission_texture, view_transmission_sampler, uv, 0.0).rgb;
}
#endif

// What the water at `start`, which is at `uv` in the picture, mirrors along `direction`,
// in linear light.
fn reflection(start: vec3<f32>, uv: vec2<f32>, direction: vec3<f32>) -> vec3<f32> {
#ifdef DEPTH_PREPASS
    var ray: Ray;
    ray.clip_start = view.clip_from_world * vec4(start, 1.0);
    ray.clip_way = view.clip_from_world * vec4(direction, 0.0);
    ray.view_start = (view.view_from_world * vec4(start, 1.0)).z;
    ray.view_way = (view.view_from_world * vec4(direction, 0.0)).z;
    var step = REFLECTION_FIRST_STEP;
    // How far along the ray it is still in front of what is drawn, in metres, and where
    // that is in the picture.
    var clear = 0.0;
    var clear_uv = uv;
    for (var index = 0u; index < REFLECTION_STEPS; index++) {
        let reached = clear + step;
        let here = probe(ray, reached);
        if !here.on_screen {
            // What is at the edge where it left the picture. Not searched for: the edge
            // nearest where the step ended is near enough to it.
            if here.ahead {
                return drawn_at(clamp(here.uv, vec2(0.0), vec2(1.0)));
            }
            return drawn_at(clear_uv);
        }
        if here.behind > 0.0 && here.behind < step + REFLECTION_THICKNESS {
            var low = clear;
            var high = reached;
            var found = here.uv;
            for (var halving = 0u; halving < REFLECTION_HALVINGS; halving++) {
                let middle = (low + high) * 0.5;
                let half = probe(ray, middle);
                if half.behind > 0.0 {
                    high = middle;
                    found = half.uv;
                } else {
                    low = middle;
                }
            }
            return drawn_at(found);
        }
        clear = reached;
        clear_uv = here.uv;
        step *= REFLECTION_GROWTH;
    }
#endif
    return mirroring.sky.rgb;
}
#endif

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    let position = in.world_position.xz;
    // The sheet leaves the water round the camera to the patch, the patch leaves the rest
    // to the sheet and what is round the truck to the fine patch, by the same tests (see
    // `surface.rs`).
    let from_middle = abs(position - patch_middle());
    let in_patch = max(from_middle.x, from_middle.y) < PATCH_SIZE / 2.0;
    let from_fine = abs(position - fine_middle());
    let in_fine = max(from_fine.x, from_fine.y) < FINE_SIZE / 2.0;
    let drawn_by = select(select(0u, 1u, in_patch), 2u, in_fine);
    if drawn_by != wind.round_camera {
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
    let field_here = field_at(position);
    // How deep the water is here, as it is drawn: on the patch, the swell moves it.
    var depth = still_depth(position);
    if wind.round_camera != 0u {
        depth += (swell_here.x + field_here.x) * patch_ease(max(from_middle.x, from_middle.y) / (PATCH_SIZE / 2.0));
    }
    let breaking = 1.0 - smoothstep(0.0, RING_BREAK_DEPTH, depth);
    var slope = swell_here.yz + field_here.yz * FIELD_SLOPE + chop(position, time) * breeze * mix(0.5, 1.8, gust);
    let foam = field_here.w * FIELD_FOAM + max(field_here.x, 0.0) * RING_BREAK * breaking;
    slope *= nearness;

    // The field's foam: ragged patches, in lumps of a few sizes, which cover the water
    // where there is most foam and break up into specks where there is little. Only
    // worked out where there is foam at all: most of the water has none, and the noise
    // for it is the dearest thing in this shader on a built-in GPU.
    var covered = 0.0;
    var foam_bump = vec2(0.0);
    if foam > 0.002 {
        let swirl = vec2(
            noise(position / 3.0 + time / FOAM_SWIRL_PERIOD),
            noise(position / 3.0 + 11.0 - time / FOAM_SWIRL_PERIOD),
        ) * (2.0 * FOAM_SWIRL) - FOAM_SWIRL;
        let swirled = position + swirl;
        let lumps = noise(swirled / 0.25 + 7.0) * 0.5 + noise(swirled / 0.7 + 31.0) * 0.3
            + noise(position / 2.1 + 57.0) * 0.2;
        // Thinner between the lumps, where the dark water shows through the bubbles.
        let gaps = mix(FOAM_GAPS, 1.0, smoothstep(SOLID_FOAM.x, SOLID_FOAM.y, foam));
        covered = smoothstep(0.0, FOAM_EDGE, foam - (1.0 - lumps))
            * mix(gaps, 1.0, smoothstep(0.3, 0.8, lumps));
        // The lumps of foam are lit as lumps: the normal leans with the slope of their
        // noise, found a little way along X and Z, which is near enough for a bump. (Not
        // from the picture's derivatives: those are not to be had inside a branch.)
        let step = 0.03;
        let along_x = noise((swirled + vec2(step, 0.0)) / 0.25 + 7.0) * 0.5
            + noise((swirled + vec2(step, 0.0)) / 0.7 + 31.0) * 0.3;
        let along_z = noise((swirled + vec2(0.0, step)) / 0.25 + 7.0) * 0.5
            + noise((swirled + vec2(0.0, step)) / 0.7 + 31.0) * 0.3;
        let fine = noise(swirled / 0.25 + 7.0) * 0.5 + noise(swirled / 0.7 + 31.0) * 0.3;
        foam_bump = vec2(along_x - fine, along_z - fine) / step * FOAM_BUMP * covered * FOAM_BUMP_SCALE;
    }
    var normal = normalize(vec3(-slope.x + foam_bump.x, 1.0, -slope.y + foam_bump.y));
    // Seen from under the water, the surface faces down.
    if !is_front {
        normal = -normal;
    }
    pbr_input.N = normal;
    pbr_input.world_normal = normal;

    // Whitecaps: the swell's highest crests, where a strong gust is blowing.
    let crest = swell_here.x / (SWELL_HEIGHTS.x + SWELL_HEIGHTS.y + SWELL_HEIGHTS.z);
    let whitecaps = smoothstep(0.6, 0.9, crest) * gust
        * smoothstep(WHITECAP_WIND.x, WHITECAP_WIND.y, wind_now);
    // The patches of foam on whitecaps and along the shore: only worked out there.
    var patches = 0.5;
    if whitecaps > 0.0 || depth < SURF_DEPTH {
        patches = noise(position / FOAM_PATCH) * 0.6 + noise(position / (FOAM_PATCH * 3.1)) * 0.4;
    }
    // The shore: a band of foam along the water's edge, never quite even, and lines of surf
    // rolling in to it, which come in at constant depths, so run along the shore.
    let edge = (1.0 - smoothstep(0.0, EDGE_FOAM_DEPTH, depth)) * (0.7 + 0.5 * patches);
    let surf_line = pow(max(cos(TAU * (depth / SURF_SPACING + time / SURF_PERIOD)), 0.0), 6.0);
    let surf = surf_line * (1.0 - smoothstep(0.2, SURF_DEPTH, depth)) * SURF_FOAM
        * min(breeze, 1.5) * mix(0.7, 1.3, gust) * mix(0.5, 1.1, patches);
    let whiteness = clamp(
        covered + whitecaps * smoothstep(0.35, 0.7, patches) + edge + surf,
        0.0,
        0.85,
    );
    // Water looked into from above is clear, and seen at a glancing angle mirrors the sky
    // and hides what is under it (Fresnel).
    let facing = abs(dot(normal, pbr_input.V));
    let glancing = pow(1.0 - facing, 3.0);
    var color = pbr_input.material.base_color;
    // Shallow water is lighter and greener than deep.
    color = vec4(mix(SHALLOW_COLOR, color.rgb, smoothstep(0.0, SHALLOW_DEPTH, depth)), color.a);
    // The deeper the water the view goes through, the less of the ground under it shows:
    // the alpha is how much deep water hides. Seen from under the water, the view goes
    // out of it, into the air.
    var murk = color.a;
    if is_front {
        let through = max(depth, 0.0) / max(facing, 0.2);
        murk = color.a * (1.0 - exp(-through / WATER_CLARITY));
    }
    let opacity = max(mix(murk, 0.95, glancing), whiteness);
    pbr_input.material.base_color = vec4(mix(color.rgb, vec3(0.95, 0.97, 1.0), whiteness), opacity);
    pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, FOAM_ROUGHNESS, whiteness);
    // Only there to put the water in the transmissive pass (see `surface.rs`).
    pbr_input.material.specular_transmission = 0.0;

    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    // Foam is white whatever it mirrors.
    let ruffled = 1.0 - GUST_DARKENING * gust * min(breeze, 1.5);
    let mirror = MIRROR * glancing * (1.0 - whiteness) * ruffled;
    var mirrored = mirroring.sky.rgb;
    let uv = frag_coord_to_uv(in.position.xy);
    // From under the water, and where it would hardly show, the sky will do.
    if is_front && mirror > LEAST_REFLECTION && mirroring.scenery == 1u {
        var direction = reflect(-pbr_input.V, normalize(mix(vec3(0.0, 1.0, 0.0), normal, REFLECTION_WAVINESS)));
        // A wave can turn the ray into the water; it skims it instead.
        direction = normalize(vec3(direction.x, max(direction.y, 0.02), direction.z));
        mirrored = mix(
            reflection(in.world_position.xyz, uv, direction) * REFLECTION_TINT,
            mirroring.sky.rgb,
            REFLECTION_HAZE,
        );
    }
    out.color = vec4(mix(out.color.rgb, mirrored, mirror), 1.0);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    if mirroring.scenery == 1u {
        // In the transmissive pass, drawn solid: what is under the water, through it, as a
        // see-through material would show it.
        let under = textureSampleLevel(view_transmission_texture, view_transmission_sampler, uv, 0.0).rgb;
        out.color = vec4(mix(under, out.color.rgb, opacity), 1.0);
    } else {
        // In the transparent pass, blended over what is under it (see `surface.rs`).
        out.color = vec4(out.color.rgb, opacity);
    }
#endif

    return out;
}
