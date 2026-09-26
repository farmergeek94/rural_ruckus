// Rain and snow round the camera, placed here, on the graphics card, every frame.
//
// The mesh never changes. Each drop is four corners that carry the same place in the box of
// rain (its position, as a share of the box across, up and along), which of the four
// corners it is (`uv`), and a number of its own (`uv_b.x`) that sets its speed, size and
// sway. From those, the time and the camera, this works out where the corner is, as
// `fall.rs` explains. What is under the ground is hidden by the ground, as anything is.
//
// The rest is the standard material's: the picture, the colour, and the fog.

#import bevy_pbr::{
    mesh_view_bindings::{globals, view},
    view_transformations::position_world_to_clip,
}

// Only the main pass: see-through things are not drawn in the prepass.
#import bevy_pbr::forward_io::{Vertex, VertexOutput}

struct Fall {
    // How fast the drops fall and are blown along, in m/s.
    velocity: vec3<f32>,
    // 1 for snow, 0 for rain.
    snow: u32,
    // The size of the box of rain round the camera, in metres.
    box_size: vec3<f32>,
    // How far above the camera the middle of the box is, in metres.
    raised: f32,
    // Rain: how long the eye sees a drop for, in seconds, and how wide its streak is.
    exposure: f32,
    width: f32,
    // How much one drop's speed differs from another's, as a share either side.
    speed_spread: f32,
    // How near the camera a drop starts to look smaller the nearer it comes, in metres.
    near_camera: f32,
    // Snow: the smallest and the largest flake, in metres of radius, and how far and how
    // fast a flake sways, in metres and radians a second.
    flake_size: vec2<f32>,
    sway: f32,
    sway_rate: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> fall: Fall;

@vertex
fn vertex(mesh_vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let t = globals.time;
    let own = mesh_vertex.uv_b.x;
    let corner = mesh_vertex.uv * 2.0 - 1.0;
    let eye = view.world_position;

    let velocity = fall.velocity * (1.0 + fall.speed_spread * (2.0 * own - 1.0));
    var at = mesh_vertex.position * fall.box_size + velocity * t;
    if fall.snow == 1u {
        let phase = own * 40.0;
        let rate = fall.sway_rate * (0.7 + 0.6 * own);
        at += vec3(sin(rate * t + phase), 0.0, cos(rate * t + phase)) * fall.sway;
    }
    // Into the box round the camera: the tile of it that the camera is in.
    let low = eye + vec3(0.0, fall.raised, 0.0) - fall.box_size * 0.5;
    let into = at - low;
    let from_eye = low + into - fall.box_size * floor(into / fall.box_size) - eye;

    // Smaller the nearer it comes, by the square, inside `near_camera`.
    let share = min(length(from_eye) / fall.near_camera, 1.0);
    let near = share * share;
    var offset: vec3<f32>;
    if fall.snow == 1u {
        // Turned to the camera; the top of the picture up.
        let right = view.world_from_view[0].xyz;
        let up = view.world_from_view[1].xyz;
        let size = near * mix(fall.flake_size.x, fall.flake_size.y, own);
        offset = (right * corner.x - up * corner.y) * size;
    } else {
        // Along the way it falls, turned about its length to face the camera, with its
        // tail, the top of the picture, behind.
        let along = velocity * fall.exposure * 0.5 * near;
        let across = normalize(cross(along, from_eye)) * fall.width * 0.5 * near;
        offset = along * corner.y + across * corner.x;
    }
    let world_position = eye + from_eye + offset;

    out.world_position = vec4(world_position, 1.0);
    out.position = position_world_to_clip(world_position);
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
    return out;
}
