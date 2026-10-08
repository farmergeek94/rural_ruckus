// Tile textures in the shadow maps, and in any depth, normal or motion vector prepass.
// Where a tile has a hole, the pixel is left out, as `tiles.wgsl` leaves it out of the
// picture. Bevy's own prepass shader reads the standard material's texture, which this
// material hasn't got, so under it a cut-out tile cast the shadow of its whole square.
//
// Only a material that may leave pixels out (an alpha mask) runs this. An opaque one is
// drawn into the shadow maps by its depth alone, with no fragment shader at all: see
// `docs/smoothness.md`. Bevy's bindless materials are not handled, as `tiles.wgsl` does
// not handle them: this material extension is not one.

#import bevy_pbr::{
    pbr_bindings::material,
    pbr_functions,
    pbr_prepass_functions,
    pbr_types,
    prepass_io::VertexOutput,
}
#ifdef PREPASS_FRAGMENT
#import bevy_pbr::prepass_io::FragmentOutput
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var tiles: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var tiles_sampler: sampler;

// `track::TileCycles`, as `tiles.wgsl` declares it.
struct TileCycles {
    offsets: array<vec4<u32>, 8>,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var<uniform> cycles: TileCycles;

// Leaves the pixel out where its tile has a hole: where the alpha is under the mask's
// cutoff, as `alpha_discard` leaves it out of the picture. The tile is found as
// `tiles.wgsl` finds it, and a test keeps the two the same.
fn discard_holes(in: VertexOutput) {
#ifdef MAY_DISCARD
#ifdef VERTEX_UVS_B
    let cycle = min(u32(round(in.uv_b.y)), 31u);
    let tile = i32(round(in.uv_b.x)) + i32(cycles.offsets[cycle / 4u][cycle % 4u]);
    let alpha = material.base_color.a * textureSample(tiles, tiles_sampler, in.uv, tile).a;
    if alpha < material.alpha_cutoff {
        discard;
    }
#endif
#endif
}

#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    discard_holes(in);
    var out: FragmentOutput;
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif
#ifdef NORMAL_PREPASS
    // The tiles carry no normal map, so the triangle's own normal, as Bevy's shader gives
    // for a material without one.
    let double_sided = (material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
    let normal = pbr_functions::prepare_world_normal(in.world_normal, double_sided, is_front);
    out.normal = vec4(normal * 0.5 + vec3(0.5), 1.0);
#endif
#ifdef MOTION_VECTOR_PREPASS
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(
        in.world_position,
        in.previous_world_position,
    );
#endif
    return out;
}
#else
@fragment
fn fragment(in: VertexOutput) {
    discard_holes(in);
}
#endif
