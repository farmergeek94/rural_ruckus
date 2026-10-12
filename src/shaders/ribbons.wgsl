// Ribbons laid on the ground, placed here, on the graphics card, every frame.
//
// Each piece of ribbon is four corners of the mesh, four in a row, which read their piece
// from `pieces` by their vertex number. The piece says where each corner is in the world,
// so the vertex shader only has to pick the right one and fade it by its age, as
// `ribbon.rs` explains. A slot with no piece in it now puts its four corners at one
// point, which draws nothing.
//
// The fragment multiplies what is drawn under it (`AlphaMode::Multiply`): the picture is
// how much of the ground's colour is left under the ribbon, white for all of it. The
// ribbon's strength fades with its age, in the distance, and in the fog, where the ground
// under it is hidden: a dark streak over fogged ground would hang in the fog.

#import bevy_pbr::{
    mesh_view_bindings::{globals, view},
    view_transformations::position_world_to_clip,
}
// The view binds its fog only when it has one.
#ifdef DISTANCE_FOG
#import bevy_pbr::{
    fog::{atmospheric_fog, exponential_fog, exponential_squared_fog, linear_fog},
    mesh_view_bindings::fog,
    mesh_view_types::{FOG_MODE_ATMOSPHERIC, FOG_MODE_EXPONENTIAL, FOG_MODE_EXPONENTIAL_SQUARED, FOG_MODE_LINEAR},
}
#endif

struct Look {
    // How long a piece lasts, in seconds, and how old it is when it starts to fade out.
    life: f32,
    fade_from: f32,
    // How many metres of ribbon one height of the picture covers.
    period: f32,
    // How far from the camera a piece starts to fade out, in metres, and where it is gone.
    seen_to: f32,
    gone_at: f32,
}

// One piece of ribbon, as `Piece` in `ribbon.rs` writes it.
struct Slot {
    // Its four corners, in metres: the two of its first edge, left then right, and the two
    // of its last. Each `w` is how far along the ribbon that edge is, in metres.
    a: vec4<f32>,
    b: vec4<f32>,
    c: vec4<f32>,
    d: vec4<f32>,
    // How much of the ground's colour the ribbon leaves at its darkest, in red, green and
    // blue, and how strong the piece is, from 0 (not there) to 1.
    color: vec4<f32>,
    // When it was laid and when it goes, on `globals.time`.
    life: vec2<f32>,
    padding: vec2<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> look: Look;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<storage, read> pieces: array<Slot>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var picture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var picture_sampler: sampler;

struct Vertex {
    @builtin(vertex_index) index: u32,
    // The mesh's own position, all zeros: the piece says where the corner is.
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    // Across the ribbon from 0 to 1, and along it in heights of the picture.
    @location(1) uv: vec2<f32>,
    // The darkest the ribbon gets, and how strong it is here.
    @location(2) color: vec4<f32>,
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let piece = pieces[vertex.index / 4u];
    let corner = vertex.index % 4u;
    let now = globals.time;
    let laid = piece.life.x;
    let goes = piece.life.y;
    let age = now - laid;
    if age < 0.0 || now >= goes {
        out.position = vec4(0.0, 0.0, 0.0, 1.0);
        out.world_position = vec3(0.0);
        out.uv = vec2(0.0);
        out.color = vec4(0.0);
        return out;
    }
    // The corners go round the piece: first edge left, first edge right, last edge
    // right, last edge left.
    var at: vec4<f32>;
    var across: f32;
    switch corner {
        case 0u: { at = piece.a; across = 0.0; }
        case 1u: { at = piece.b; across = 1.0; }
        case 2u: { at = piece.d; across = 1.0; }
        default: { at = piece.c; across = 0.0; }
    }
    let fade = 1.0 - clamp((age - look.fade_from) / max(look.life - look.fade_from, 1e-4), 0.0, 1.0);
    let distance = length(view.world_position - at.xyz);
    let near = 1.0 - clamp((distance - look.seen_to) / max(look.gone_at - look.seen_to, 1e-4), 0.0, 1.0);
    out.world_position = at.xyz;
    out.position = position_world_to_clip(at.xyz);
    out.uv = vec2(across, at.w / look.period);
    out.color = vec4(piece.color.rgb, piece.color.a * fade * near);
    return out;
}

// How much of the contrast of the ground `distance` metres off the fog leaves: the
// difference the fog leaves between white and black there. 1 without fog.
fn fog_contrast(distance: f32) -> f32 {
#ifdef DISTANCE_FOG
    let white = vec4(1.0, 1.0, 1.0, 1.0);
    let black = vec4(0.0, 0.0, 0.0, 1.0);
    let none = vec3(0.0);
    var lit: vec4<f32>;
    var dark: vec4<f32>;
    if fog.mode == FOG_MODE_LINEAR {
        lit = linear_fog(fog, white, distance, none);
        dark = linear_fog(fog, black, distance, none);
    } else if fog.mode == FOG_MODE_EXPONENTIAL {
        lit = exponential_fog(fog, white, distance, none);
        dark = exponential_fog(fog, black, distance, none);
    } else if fog.mode == FOG_MODE_EXPONENTIAL_SQUARED {
        lit = exponential_squared_fog(fog, white, distance, none);
        dark = exponential_squared_fog(fog, black, distance, none);
    } else if fog.mode == FOG_MODE_ATMOSPHERIC {
        lit = atmospheric_fog(fog, white, distance, none);
        dark = atmospheric_fog(fog, black, distance, none);
    } else {
        return 1.0;
    }
    return clamp(dot(lit.rgb - dark.rgb, vec3(1.0 / 3.0)), 0.0, 1.0);
#else
    return 1.0;
#endif
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // The picture is how dark the ribbon is at each point, from 0 (not at all) to 1.
    let darkness = textureSample(picture, picture_sampler, in.uv).r;
    let distance = length(view.world_position - in.world_position);
    let strength = in.color.a * darkness * fog_contrast(distance);
    // The blend is `result = dst * src.rgb + (1 - src.a) * dst`, so the colour is given
    // already multiplied by the strength, as the standard material's multiply mode does.
    return vec4(in.color.rgb * strength, strength);
}
