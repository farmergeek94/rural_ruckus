// Sheets of water placed here, on the graphics card, every frame.
//
// Each point of the mesh says how far across its lip it is and whether it is on the newer
// or the older lip of its band (`uv`), and which band it is (`uv_b.x`). From the band's two
// lips, the time and the camera, this works out where the point is, as `sheet.rs` explains.
// A band whose lips were not poured one after the other by one source, or that is gone,
// puts its points at one point, which draws nothing.
//
// The fragment is the standard material's, unlit: the picture, the colour, and the fog.
// As a lip ages, the thin water of the picture tears away, and it fades out where it sinks
// under the water. With the camera's depth prepass,
// it fades out near what is behind it.

#import bevy_pbr::{
    mesh_view_bindings::{globals, view},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::main_pass_post_lighting_processing,
    view_transformations::{depth_ndc_to_view_z, position_world_to_clip},
}

#ifdef DEPTH_PREPASS
#import bevy_pbr::prepass_utils::prepass_depth
#endif

// Only the main pass: see-through things are not drawn in the prepass.
#import bevy_pbr::forward_io::{FragmentOutput, Vertex, VertexOutput}

struct Flow {
    // How fast the air goes, in m/s.
    air: vec3<f32>,
    // How fast the water falls, in m/s².
    gravity: f32,
    // How much of its speed through the air it loses each second.
    drag: f32,
    // How long a lip lasts, and how old it is when it starts to fade out, in seconds.
    life: f32,
    fade_from: f32,
    // The height it pours back down into, and how far under it it is gone, in metres.
    floor: f32,
    sink: f32,
    // How much of the picture's thin water has torn away at the end of a lip's life.
    tear: f32,
    // How near what is behind it it starts to fade out, in metres. At 0, it doesn't.
    soft: f32,
    // How near the camera it starts to fade out, in metres.
    near_camera: f32,
    // How many times the picture goes by along the sheet for each second of pouring.
    picture_rate: f32,
    // How many lips each strip holds.
    rows: u32,
}

// One lip, as `Row` in `sheet.rs` writes it.
struct Row {
    // Where its root starts, in metres, and when it was poured, on `globals.time`.
    root: vec3<f32>,
    poured: f32,
    // Where its crest starts, and when the lip before it was poured (-1 for none).
    crest: vec3<f32>,
    follows: f32,
    // How fast its root goes, in m/s, and when it is gone.
    root_velocity: vec3<f32>,
    goes: f32,
    // How fast its crest goes, and how solid it is, from 0 to 1.
    crest_velocity: vec3<f32>,
    strength: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> flow: Flow;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<storage, read> rows: array<Row>;

@vertex
fn vertex(mesh_vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    out.world_normal = vec3(0.0, 1.0, 0.0);
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = mesh_vertex.instance_index;
#endif

    let band = u32(mesh_vertex.uv_b.x);
    let in_strip = band % flow.rows;
    let older = rows[band];
    let newer = rows[band - in_strip + (in_strip + 1u) % flow.rows];
    let now = globals.time;
    let joined = newer.follows == older.poured && now >= newer.poured && now < older.goes;
    if !joined {
        out.world_position = vec4(0.0, 0.0, 0.0, 1.0);
        out.position = vec4(0.0, 0.0, 0.0, 1.0);
#ifdef VERTEX_UVS_A
        out.uv = vec2(0.0);
#endif
#ifdef VERTEX_UVS_B
        out.uv_b = vec2(0.0);
#endif
#ifdef VERTEX_COLORS
        out.color = vec4(0.0);
#endif
        return out;
    }
    var row = older;
    if mesh_vertex.uv.y > 0.5 {
        row = newer;
    }

    // As `pool::Motion::at`: gravity, and drag towards the air.
    let across = mesh_vertex.uv.x;
    let age = now - row.poured;
    let poured_from = mix(row.root, row.crest, across);
    let start = mix(row.root_velocity, row.crest_velocity, across);
    let settle = flow.air - vec3(0.0, flow.gravity / flow.drag, 0.0);
    let decay = exp(-flow.drag * age);
    let at = poured_from + settle * age + (start - settle) * (1.0 - decay) / flow.drag;

    let fade = 1.0 - smoothstep(flow.fade_from, flow.life, age);
    let near = clamp(length(view.world_position - at) / flow.near_camera, 0.0, 1.0);

    out.world_position = vec4(at, 1.0);
    out.position = position_world_to_clip(at);
#ifdef VERTEX_UVS_A
    // Along the sheet by when the water was poured, so that the picture flows with it.
    out.uv = vec2(across, row.poured * flow.picture_rate);
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vec2(mesh_vertex.uv_b.x, clamp(age / flow.life, 0.0, 1.0));
#endif
#ifdef VERTEX_COLORS
    out.color = vec4(1.0, 1.0, 1.0, row.strength * fade * near * near);
#endif
    return out;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    var color = pbr_input.material.base_color;
#ifdef VERTEX_COLORS
#ifdef VERTEX_UVS_B
    // The picture's own opacity, without the lip's: the thin water goes first, and more of
    // it the older the lip.
    let body = max(in.color.a, 1e-4);
    let tear = flow.tear * in.uv_b.y * in.uv_b.y;
    let water = clamp((color.a / body - tear) / max(1.0 - tear, 1e-4), 0.0, 1.0);
    color.a = in.color.a * water;
#endif
#endif
    // Gone as it sinks under the water: for each pixel, as a fade at the corners of a band
    // cut the sheet into triangles where it lies on the water.
    color.a *= 1.0 - clamp((flow.floor - in.world_position.y) / max(flow.sink, 1e-4), 0.0, 1.0);
    var out: FragmentOutput;
#ifdef DEPTH_PREPASS
    if flow.soft > 0.0 {
        // How far behind this point the nearest solid thing is, in metres. The first
        // sample only, as in `particles.wgsl`.
        let behind = depth_ndc_to_view_z(in.position.z) - depth_ndc_to_view_z(prepass_depth(in.position, 0u));
        color.a *= clamp(behind / flow.soft, 0.0, 1.0);
    }
#endif
    out.color = main_pass_post_lighting_processing(pbr_input, color);
    return out;
}
