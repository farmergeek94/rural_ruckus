// The depth of the picture as drawn so far, one sample a pixel, into an `R32Float` image
// (`scene_depth.rs`): what the water and the particles read where they would have read the
// depth prepass. Drawn over the whole picture with the fullscreen triangle.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

#ifdef MULTISAMPLED
@group(0) @binding(0) var depth: texture_depth_multisampled_2d;
#else
@group(0) @binding(0) var depth: texture_depth_2d;
#endif

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) f32 {
    // The first sample of a multisampled picture, as the prepass depth was read: reading
    // each sample would shade each.
    return textureLoad(depth, vec2<i32>(in.position.xy), 0);
}
