// Particles placed here, on the graphics card, every frame.
//
// Each particle is four corners of the mesh, which say which corner they are (`uv`) and
// which slot of `slots` holds their particle (`uv_b.x`). From the particle, the time and
// the camera, this works out where the corner is, as `mod.rs` explains. A slot with no
// particle in it now puts its four corners at one point, which draws nothing.
//
// The fragment is the standard material's, unlit: the picture, the colour, and the fog.
// With the camera's depth prepass, it fades out near what is behind it.

#import bevy_pbr::{
    mesh_view_bindings::{globals, view},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, main_pass_post_lighting_processing},
    view_transformations::{depth_ndc_to_view_z, position_world_to_clip},
}

#ifdef DEPTH_PREPASS
#import bevy_pbr::prepass_utils::prepass_depth
#endif

// Only the main pass: see-through things are not drawn in the prepass.
#import bevy_pbr::forward_io::{FragmentOutput, Vertex, VertexOutput}

struct Motion {
    // How fast the air goes between gusts, in m/s.
    air: vec3<f32>,
    // How fast the particles fall, in m/s² (below 0, they rise).
    gravity: f32,
    // How far a full gust takes the air from `air`, in m/s.
    gusts: vec3<f32>,
    // How much of their speed through the air they lose each second.
    drag: f32,
    // How long each of the gusts' three swings takes, in seconds, and how much of `gusts`
    // each is.
    gust_periods: vec3<f32>,
    // How long one lasts, in seconds, if it doesn't come down first.
    life: f32,
    gust_weights: vec3<f32>,
    // How fast they spread, in metres of radius each second.
    spread: f32,
    // How old one is when it starts to fade out, and to shrink away, in seconds.
    fade_from: f32,
    shrink_from: f32,
    // How long the eye sees one for, in seconds: above 0, it is a streak.
    exposure: f32,
    // How near the camera one starts to look smaller the nearer it comes, in metres.
    near_camera: f32,
    // How near what is behind it one starts to fade out, in metres. At 0, it doesn't.
    soft: f32,
}

// One particle, as `Slot` in `mod.rs` writes it.
struct Slot {
    // Where it starts, in metres, and its radius.
    at: vec3<f32>,
    size: f32,
    // How fast it goes, in m/s, and how far it is turned in the picture, in radians.
    velocity: vec3<f32>,
    turned: f32,
    color: vec4<f32>,
    // When it was thrown and when it goes, on `globals.time`.
    life: vec2<f32>,
    // How fast it turns, in radians a second.
    turning: f32,
    // The line it trails along from where it is, in metres.
    trail: vec3<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> motion: Motion;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<storage, read> slots: array<Slot>;

const TAU: f32 = 6.283185307179586;

// How fast the air goes when the clock reads `time`, gusts and all. As `Air::at` in
// `mod.rs`.
fn air_now(time: f32) -> vec3<f32> {
    return motion.air + motion.gusts * dot(motion.gust_weights, sin(TAU * time / motion.gust_periods));
}

@vertex
fn vertex(mesh_vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    out.world_normal = vec3(0.0, 1.0, 0.0);
#ifdef VERTEX_UVS_A
    out.uv = mesh_vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = mesh_vertex.uv_b;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = mesh_vertex.instance_index;
#endif

    let particle = slots[u32(mesh_vertex.uv_b.x)];
    let now = globals.time;
    let thrown = particle.life.x;
    let goes = particle.life.y;
    let age = now - thrown;
    if age < 0.0 || now >= goes {
        out.world_position = vec4(0.0, 0.0, 0.0, 1.0);
        out.position = vec4(0.0, 0.0, 0.0, 1.0);
#ifdef VERTEX_COLORS
        out.color = vec4(0.0);
#endif
        return out;
    }

    // As `Motion::at` in `mod.rs`, in the air as it moves now: a gust moves every particle
    // in the pool a little.
    let settle = air_now(now) - vec3(0.0, motion.gravity / motion.drag, 0.0);
    let decay = exp(-motion.drag * age);
    let start = particle.velocity;
    let at = particle.at + settle * age + (start - settle) * (1.0 - decay) / motion.drag;
    let velocity = settle + (start - settle) * decay;

    let eye = view.world_position;
    let distance = max(length(eye - at), 1e-4);
    // Smaller the nearer it comes, by the square, inside `near_camera`.
    let share = min(distance / motion.near_camera, 1.0);
    let near = share * share;
    let shrink = sqrt(clamp((motion.life - age) / max(motion.life - motion.shrink_from, 1e-4), 0.0, 1.0));
    let fade = 1.0 - clamp((age - motion.fade_from) / max(motion.life - motion.fade_from, 1e-4), 0.0, 1.0);
    let size = (particle.size + motion.spread * age) * shrink * near;

    // Across and up the picture, from -1 to 1, the top of the picture up.
    let corner = vec2(mesh_vertex.uv.x * 2.0 - 1.0, 1.0 - mesh_vertex.uv.y * 2.0);
    var offset: vec3<f32>;
    if motion.exposure > 0.0 {
        // Turned to the camera, with the top of the picture along the way it seems to go,
        // and as long as the way it goes while the eye sees it: thinner the longer, as a
        // streak of the same water is.
        let towards_eye = (eye - at) / distance;
        let across = velocity - towards_eye * dot(velocity, towards_eye);
        let speed = length(across);
        var along: vec3<f32>;
        if speed > 1e-3 {
            along = across / speed;
        } else {
            let other = select(vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), abs(towards_eye.x) > 0.9);
            along = normalize(cross(towards_eye, other));
        }
        let side = cross(along, towards_eye);
        let long = max(size + speed * motion.exposure * 0.5, 1e-6);
        let width = size * sqrt(size / long);
        offset = side * corner.x * width + along * corner.y * long;
    } else {
        // Half its trail, as the camera sees it. Seen end on, it is as long as it is wide.
        let towards_eye = (eye - at) / distance;
        let half = particle.trail * 0.5 * shrink * near;
        let across = half - towards_eye * dot(half, towards_eye);
        let long = length(across);
        if long > 1e-3 {
            // Its middle half way along its trail, turned to the camera, with the top of the
            // picture along the trail, and as wide as it would be without one.
            let along = across / long;
            let side = cross(along, towards_eye);
            offset = half + side * corner.x * size + along * corner.y * (size + long);
        } else {
            // Flat to the camera, turned in its own plane.
            let turned = particle.turned + particle.turning * age;
            let c = cos(turned);
            let s = sin(turned);
            let turn = vec2(corner.x * c - corner.y * s, corner.x * s + corner.y * c);
            let right = view.world_from_view[0].xyz;
            let up = view.world_from_view[1].xyz;
            offset = (right * turn.x + up * turn.y) * size;
        }
    }
    let world_position = at + offset;

    out.world_position = vec4(world_position, 1.0);
    out.position = position_world_to_clip(world_position);
#ifdef VERTEX_COLORS
    out.color = vec4(particle.color.rgb, particle.color.a * fade);
#endif
    return out;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    var out: FragmentOutput;
    // Particles are not lit (see `mod.rs`).
    out.color = pbr_input.material.base_color;
#ifdef DEPTH_PREPASS
    if motion.soft > 0.0 {
        // How far behind this point the nearest solid thing is, in metres. The first
        // sample only: reading each sample of a multisampled picture would shade each.
        let behind = depth_ndc_to_view_z(in.position.z) - depth_ndc_to_view_z(prepass_depth(in.position, 0u));
        out.color.a *= clamp(behind / motion.soft, 0.0, 1.0);
    }
#endif
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
