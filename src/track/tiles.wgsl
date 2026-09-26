// Tile textures. A surface with this material is an ordinary PBR one except for where its
// colour comes from: one layer of a texture array per tile, instead of a single texture.
// Each vertex names its tile in the first number of the second UV channel, and every
// vertex of a triangle names the same one. The second number names an animated texture,
// counting from 1, or is 0 for a still one: an animated texture's frames are consecutive
// tiles, and `cycles` says how many tiles on from the first frame each one is now.
//
// An unlit material (the backdrop's) shows the tiles' own colours, as Bevy's own shader
// does for one.
//
// The ground's material can also carry a normal map of the smooth surface through its
// heights (`track/shading.rs`), covering the whole track. With `ground.lit_smoothly` set, each
// pixel is lit by the normal there instead of by its flat triangle's.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var tiles: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var tiles_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var ground_normals: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var ground_normals_sampler: sampler;

// `track::GroundShading`.
struct GroundShading {
    // Side of the square the normal map covers, in metres, centred on the origin.
    size: f32,
    // 1 to light by the normal map.
    lit_smoothly: u32,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var<uniform> ground: GroundShading;

// `track::TileCycles`: four to a vector, the first always 0.
struct TileCycles {
    offsets: array<vec4<u32>, 8>,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var<uniform> cycles: TileCycles;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

#ifdef VERTEX_UVS_B
    // The tile number doesn't vary across a triangle. Rounding guards against it arriving
    // as 6.9999.
    let cycle = min(u32(round(in.uv_b.y)), 31u);
    let tile = i32(round(in.uv_b.x)) + i32(cycles.offsets[cycle / 4u][cycle % 4u]);
    pbr_input.material.base_color *= textureSample(tiles, tiles_sampler, in.uv, tile);
#endif

#ifndef PREPASS_PIPELINE
    if ground.lit_smoothly != 0u {
        // Stored from 0 to 1 for -1 to 1. Blending between texels and mipmaps shortens a
        // normal, so it is made a unit one again.
        let place = in.world_position.xz / ground.size + 0.5;
        let stored = textureSample(ground_normals, ground_normals_sampler, place).xyz;
        let normal = normalize(stored * 2.0 - 1.0);
        pbr_input.world_normal = normal;
        pbr_input.N = normal;
        pbr_input.clearcoat_N = normal;
    }
#endif

    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
