// The water round the camera as a field of heights, which the trucks disturb and which
// the graphics card steps: a wave spreads out from wherever a truck pushes the water, so
// that a moving truck leaves a wake, a plunging wheel starts a ring, waves cross, slow in
// the shallows and reflect off the shore, and the foam the trucks churn up lies on the
// water and fades. `field.rs` says what the trucks do to it (`Source`), and `water.wgsl`
// draws it from `look`.
//
// The field is a square window of `TEXELS` texels `SPACING` metres apart, centred on the
// camera. Texel `i` holds every cell of the world whose index is `i` modulo `TEXELS`, and
// stands for the one nearest the window's middle, so that when the camera moves the
// texels need not: a texel that comes in at the leading edge is one that went out at the
// trailing edge, and the outer band of the window (`BAND`) eases the water flat, so that
// what it held is nothing by then.
//
// The height follows the wave equation on a grid, one step at a time (`Field::step`
// seconds), with each texel read from `before` and written to `after`: waves go at the
// speed of waves on shallow water, `sqrt(g * depth)`, and no faster than `RIPPLE_SPEED`.
// Where the land stands out of the water the height is held at nothing, which waves
// reflect off. Foam is churned up where the water is steep, spreads, and fades.

struct Source {
    // Where, on the ground plane (world X and Z), in metres.
    at: vec2<f32>,
    // Which way it goes over the water, times how fast, in m/s.
    along: vec2<f32>,
    // How far out from `at` it reaches, in metres: a bell, a third as strong there.
    radius: f32,
    // How fast it piles the water up ahead of it and draws it down behind, in metres of
    // height each second at the bell's peak.
    push: f32,
    // How much it lifts the water at once, in metres, at the bell's peak. Below 0, it
    // pushes it down.
    lift: f32,
    // How much foam it churns up each second, and at once.
    foam: f32,
    burst: f32,
}

struct Sources {
    // As long as `MAX_SOURCES` in `field.rs`.
    sources: array<Source, 64>,
    count: u32,
}

struct Shore {
    // Where the depth map's first texel is (world X and Z), in metres.
    origin: vec2<f32>,
    // How far apart its texels are, in metres, and how many there are along a side.
    spacing: f32,
    resolution: f32,
    // 1 where the ground repeats beyond the edges of the map.
    repeats: u32,
}

struct Field {
    // Where the window's middle is, on the ground plane (world X and Z), in metres.
    middle: vec2<f32>,
    // How long one step is, in seconds.
    step: f32,
    // 1 to forget everything: the camera has jumped.
    clear: u32,
    shore: Shore,
}

@group(0) @binding(0) var before: texture_storage_2d<rgba16float, read>;
@group(0) @binding(1) var after: texture_storage_2d<rgba16float, write>;
// What `water.wgsl` reads: the height, how it slopes along X and Z, and the foam.
@group(0) @binding(2) var look: texture_storage_2d<rgba16float, write>;
// How deep the still water is over the ground, in metres (see `shore.rs`).
@group(0) @binding(3) var depth_map: texture_2d<f32>;
@group(0) @binding(4) var<uniform> field: Field;
@group(0) @binding(5) var<storage, read> sources: Sources;

// As in `field.rs`: how many texels the window is along a side, and how far apart they
// are, in metres.
const TEXELS: i32 = 512;
const SPACING: f32 = 0.5;
// How much of the way out from the window's middle to its edge the water starts to ease
// flat.
const BAND: f32 = 0.85;
// The fastest a wave goes, in m/s: the speed of a wave on water about a metre deep. As
// `RIPPLE_SPEED` in `shore.rs`, which a test checks.
const RIPPLE_SPEED: f32 = 3.0;
const GRAVITY: f32 = 9.81;
// How much of its rate of rise the water loses each second in deep water, and how much
// more at the water's edge, where the waves break and spend themselves, which is within
// `SHALLOW` metres of depth.
const DAMPING: f32 = 0.1;
const SHALLOW_DAMPING: f32 = 2.5;
const SHALLOW: f32 = 0.8;
// How steep the water must be, in metres of rise for each metre across, for it to break
// into foam, and how much foam it makes each second for each unit steeper; and how fast
// it must rise or fall, in m/s, to churn into foam, and how much each second for each
// m/s faster.
const STEEP: f32 = 0.1;
const BREAKING: f32 = 4.0;
const CHURNING: f32 = 0.2;
const TURBULENCE: f32 = 3.0;
// How high the water must be piled up, in metres, to churn into foam, and how much foam
// each second for each metre higher: water shoved up by a truck is broken water.
const PILED: f32 = 0.06;
const PILING: f32 = 4.0;
// How long foam takes to fade to a third, in seconds, and how much of the way towards
// its neighbours' foam a texel's goes each second: foam spreads as it fades.
const FOAM_LIFE: f32 = 5.0;
const FOAM_SPREAD: f32 = 0.5;
// The most foam a texel holds: more than solid white is no whiter.
const MOST_FOAM: f32 = 2.0;

// The texel `TEXELS` wide window round, for a texel index that may be off it.
fn wrap(texel: vec2<i32>) -> vec2<i32> {
    return (texel % TEXELS + TEXELS) % TEXELS;
}

// Where `texel` is in the world: the cell nearest the window's middle of those it holds.
fn world_of(texel: vec2<i32>) -> vec2<f32> {
    let middle = vec2<i32>(floor(field.middle / SPACING));
    var offset = (texel - middle) % TEXELS;
    offset = (offset + TEXELS + TEXELS / 2) % TEXELS - TEXELS / 2;
    return (vec2<f32>(middle + offset) + 0.5) * SPACING;
}

// How deep the still water is over the ground at `position`, in metres. Below the
// ground it is less than 0. As `still_depth` in `water.wgsl`.
fn still_depth(position: vec2<f32>) -> f32 {
    let last = i32(field.shore.resolution) - 1;
    var from_origin = position - field.shore.origin;
    if field.shore.repeats != 0u {
        let size = field.shore.spacing * f32(last);
        from_origin -= floor(from_origin / size) * size;
    }
    let texels = clamp(from_origin / field.shore.spacing, vec2(0.0), vec2(f32(last)));
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

@compute @workgroup_size(8, 8, 1)
fn step(@builtin(global_invocation_id) id: vec3<u32>) {
    let texel = vec2<i32>(id.xy);
    if field.clear == 1u {
        textureStore(after, texel, vec4(0.0));
        textureStore(look, texel, vec4(0.0));
        return;
    }
    let position = world_of(texel);
    let dt = field.step;
    let here = textureLoad(before, texel);
    let left = textureLoad(before, wrap(texel + vec2(-1, 0)));
    let right = textureLoad(before, wrap(texel + vec2(1, 0)));
    let up = textureLoad(before, wrap(texel + vec2(0, -1)));
    let down = textureLoad(before, wrap(texel + vec2(0, 1)));
    var height = here.r;
    // How fast it rises, in m/s.
    var rising = here.g;
    var foam = here.b;

    let depth = still_depth(position);
    let wave_speed2 = min(RIPPLE_SPEED * RIPPLE_SPEED, GRAVITY * max(depth, 0.0));
    let laplacian = (left.r + right.r + up.r + down.r - 4.0 * height) / (SPACING * SPACING);
    let damping = DAMPING + SHALLOW_DAMPING * (1.0 - smoothstep(0.0, SHALLOW, depth));
    rising += (wave_speed2 * laplacian - damping * rising) * dt;
    height += rising * dt;

    foam = mix(foam, (left.b + right.b + up.b + down.b) * 0.25, min(FOAM_SPREAD * dt, 1.0));
    foam *= max(1.0 - dt / FOAM_LIFE, 0.0);

    for (var index = 0u; index < sources.count; index++) {
        let source = sources.sources[index];
        let offset = position - source.at;
        // Most texels are nowhere near most sources.
        let reach = 2.0 * source.radius;
        if abs(offset.x) > reach || abs(offset.y) > reach {
            continue;
        }
        let bell = exp(-dot(offset, offset) / (source.radius * source.radius));
        let going = length(source.along);
        var bow = 0.0;
        if going > 0.01 {
            bow = clamp(dot(offset, source.along) / (going * source.radius), -1.0, 1.0);
        }
        height += (source.lift + source.push * bow * dt) * bell;
        foam += (source.burst + source.foam * dt) * bell;
    }

    // Steep water breaks into foam.
    let slope = vec2(right.r - left.r, down.r - up.r) / (2.0 * SPACING);
    foam += max(length(slope) - STEEP, 0.0) * BREAKING * dt
        + max(abs(rising) - CHURNING, 0.0) * TURBULENCE * dt
        + max(height - PILED, 0.0) * PILING * dt;

    // The land stands still, and the waves reflect off it.
    if depth <= 0.0 {
        height = 0.0;
        rising = 0.0;
        foam = 0.0;
    }
    // The outer band of the window eases flat, so that nothing is left in a texel by the
    // time it comes round to the other side.
    let out = max(abs(position.x - field.middle.x), abs(position.y - field.middle.y))
        / (f32(TEXELS) * SPACING / 2.0);
    let keep = 1.0 - smoothstep(BAND, 1.0, out);
    height *= keep;
    rising *= keep;
    foam = min(foam * keep, MOST_FOAM);

    textureStore(after, texel, vec4(height, rising, foam, 0.0));
    textureStore(look, texel, vec4(height, slope.x, slope.y, foam));
}
