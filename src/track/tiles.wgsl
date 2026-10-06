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
//
// It can also carry a map of its cells (`track/blend.rs`). With `ground.blend_width` above
// 0, a pixel near the edge of its cell fades into the tile across it, blurred
// (`blend_ground_edges`).
//
// With `lighting.simple` set, a lit material is lit the cheap way (`simple_lighting`): the
// sun, its shadows and the light from all round, on a plain matte surface. No shine, no
// lamps, no environment map. See `lighting.rs`.

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
    pbr_types::PbrInput,
    mesh_view_bindings as view_bindings,
    mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT,
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
    shadows::fetch_directional_shadow,
}
#import bevy_render::maths::PI
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var tiles: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var tiles_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var ground_normals: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var ground_normals_sampler: sampler;

// `track::GroundShading`.
struct GroundShading {
    // Side of the square the normal map and the cells cover, in metres, centred on the
    // origin.
    size: f32,
    // 1 to light by the normal map.
    lit_smoothly: u32,
    // Cells along each side of the track.
    cells: u32,
    // How far into a cell its neighbours' tiles are faded in, as a share of its side. 0
    // for not at all.
    blend_width: f32,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var<uniform> ground: GroundShading;

// `track::TileCycles`: four to a vector, the first always 0.
struct TileCycles {
    offsets: array<vec4<u32>, 8>,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var<uniform> cycles: TileCycles;

// `track::TileLighting`.
struct TileLighting {
    // 1 to light the cheap way.
    simple: u32,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var<uniform> lighting: TileLighting;

// Two texels for each ground cell: see `track/blend.rs`.
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var ground_cells: texture_2d<f32>;

// A ground cell, as `ground_cells` holds it.
struct Cell {
    tile: i32,
    // Where its lowest corner falls in its tile.
    origin: vec2<f32>,
    // How far through its tile one cell's side along X, and along Z, moves.
    along_x: vec2<f32>,
    along_z: vec2<f32>,
}

fn ground_cell(at: vec2<i32>) -> Cell {
    let first = textureLoad(ground_cells, vec2<i32>(at.x * 2, at.y), 0);
    let second = textureLoad(ground_cells, vec2<i32>(at.x * 2 + 1, at.y), 0);
    return Cell(i32(round(first.x)), first.yz, second.xy, second.zw);
}

// The tile of the cell `at` drawn at `place` (in cells from the track's corner). Where
// `place` is just across one or two of the cell's edges, the tile is mirrored back across
// them, so that at the edge it shows the same colours as it does in its own cell. `ddx`
// and `ddy` are how `place` changes from pixel to pixel: given, because which cell a
// pixel reads differs from its neighbours', and the graphics card can't work them out
// itself. Larger ones read a smaller, blurrier copy of the tile.
fn tile_at(cell: Cell, at: vec2<i32>, place: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>) -> vec4<f32> {
    var local = place - vec2<f32>(at);
    var flip = vec2<f32>(1.0);
    if local.x < 0.0 {
        local.x = -local.x;
        flip.x = -1.0;
    } else if local.x > 1.0 {
        local.x = 2.0 - local.x;
        flip.x = -1.0;
    }
    if local.y < 0.0 {
        local.y = -local.y;
        flip.y = -1.0;
    } else if local.y > 1.0 {
        local.y = 2.0 - local.y;
        flip.y = -1.0;
    }
    let uv = cell.origin + cell.along_x * local.x + cell.along_z * local.y;
    let gx = ddx * flip;
    let gy = ddy * flip;
    return textureSampleGrad(
        tiles, tiles_sampler, uv, cell.tile,
        cell.along_x * gx.x + cell.along_z * gx.y,
        cell.along_x * gy.x + cell.along_z * gy.y,
    );
}

// How much blurrier the tiles are at a cell's edge than in its middle, as how many times
// further apart the texels they are read from are. Each doubling is one mipmap smaller.
// 4 is two mipmaps: soft, but the tiles' patterns still show. 1 for no blur, only the
// fade. Without mipmaps there are no smaller copies, and only the fade shows.
const BLEND_BLUR: f32 = 4.0;

// The colour of the ground at `world` (X and Z, in metres), given `own`, the colour of the
// pixel's own tile there. Near an edge of its cell, the tile across it is faded in, half
// and half at the edge and to nothing `ground.blend_width` into the cell, and all of them
// are read blurred. Near a corner, the cell along X is faded in first, then the row of
// cells along Z, so that all four cells round a corner agree about it whichever one the
// pixel is in.
fn blend_ground_edges(own: vec4<f32>, world: vec2<f32>) -> vec4<f32> {
    let cells = i32(ground.cells);
    let place = (world / ground.size + 0.5) * f32(ground.cells);
    // Here, before the branches below that differ from pixel to pixel.
    let ddx = dpdx(place);
    let ddy = dpdy(place);

    let at = clamp(vec2<i32>(floor(place)), vec2<i32>(0), vec2<i32>(cells - 1));
    let within = place - vec2<f32>(at);
    let upper = within >= vec2<f32>(0.5);
    let from_edge = select(within, 1.0 - within, upper);
    // 1 at an edge, 0 from `blend_width` in.
    let nearness = 1.0 - smoothstep(vec2<f32>(0.0), vec2<f32>(ground.blend_width), from_edge);
    if all(nearness <= vec2<f32>(0.0)) {
        return own;
    }
    let share = 0.5 * nearness;
    let blur = mix(1.0, BLEND_BLUR, max(nearness.x, nearness.y));
    let blur_x = ddx * blur;
    let blur_y = ddy * blur;

    // Across the nearer edge along X (b), the nearer one along Z (c), and the corner (d).
    // Off the track, a cell's neighbour is itself, mirrored.
    let toward = select(vec2<i32>(-1), vec2<i32>(1), upper);
    let b_at = clamp(at + vec2<i32>(toward.x, 0), vec2<i32>(0), vec2<i32>(cells - 1));
    let c_at = clamp(at + vec2<i32>(0, toward.y), vec2<i32>(0), vec2<i32>(cells - 1));
    let d_at = clamp(at + toward, vec2<i32>(0), vec2<i32>(cells - 1));

    var row = tile_at(ground_cell(at), at, place, blur_x, blur_y);
    if share.x > 0.0 {
        row = mix(row, tile_at(ground_cell(b_at), b_at, place, blur_x, blur_y), share.x);
    }
    if share.y > 0.0 {
        var other_row = tile_at(ground_cell(c_at), c_at, place, blur_x, blur_y);
        if share.x > 0.0 {
            other_row = mix(other_row, tile_at(ground_cell(d_at), d_at, place, blur_x, blur_y), share.x);
        }
        row = mix(row, other_row, share.y);
    }
    return row;
}

#ifndef PREPASS_PIPELINE
// A matte surface lit by each directional light, through its shadows, and by the ambient
// light: Bevy's own diffuse terms with the shine and everything else left out. Lambert's
// 1/pi stands in for Burley's, which it equals on a rough surface seen square on.
fn simple_lighting(in: PbrInput) -> vec4<f32> {
    let diffuse_color = in.material.base_color.rgb * (1.0 - in.material.metallic);
    let view_z = dot(vec4<f32>(
        view_bindings::view.view_from_world[0].z,
        view_bindings::view.view_from_world[1].z,
        view_bindings::view.view_from_world[2].z,
        view_bindings::view.view_from_world[3].z
    ), in.world_position);

    var light = view_bindings::lights.ambient_color.rgb * in.diffuse_occlusion;
    let receives_shadows = (in.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u;
    for (var i: u32 = 0u; i < view_bindings::lights.n_directional_lights; i = i + 1u) {
        let sun = &view_bindings::lights.directional_lights[i];
        let n_dot_l = dot(in.N, (*sun).direction_to_light);
        // Facing away, it is in its own shade, and no shadow map need be read.
        if n_dot_l <= 0.0 {
            continue;
        }
        var shadow = 1.0;
        if receives_shadows && ((*sun).flags & DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            shadow = fetch_directional_shadow(i, in.world_position, in.world_normal, view_z, in.frag_coord.xy);
        }
        light += (*sun).color.rgb * (n_dot_l * shadow / PI);
    }

    let emissive = in.material.emissive;
    let emissive_light = emissive.rgb * mix(1.0, view_bindings::view.exposure, emissive.a);
    return vec4<f32>(
        view_bindings::view.exposure * diffuse_color * light + emissive_light,
        in.material.base_color.a,
    );
}
#endif

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
    var tile_color = textureSample(tiles, tiles_sampler, in.uv, tile);
    if ground.blend_width > 0.0 {
        tile_color = blend_ground_edges(tile_color, in.world_position.xz);
    }
    pbr_input.material.base_color *= tile_color;
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
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) != 0u {
        out.color = pbr_input.material.base_color;
    } else if lighting.simple != 0u {
        out.color = simple_lighting(pbr_input);
    } else {
        out.color = apply_pbr_lighting(pbr_input);
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
