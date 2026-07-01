///
/// Simply draws a color-changing triangle. I used it for early testing.
///
/// Adapted from:
///
///     https://google.github.io/tour-of-wgsl
///


struct Uniforms { frame: u32 }


// Set triangle
@group(0) @binding(0) var<uniform> config: Uniforms;
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-0.5, -0.5),
        vec2<f32>( 0.5, -0.5),
        vec2<f32>( 0.0,  0.5),
    );

    return vec4<f32>(pos[i], 0.0, 1.0);
}

// Change color
@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, sin(f32(config.frame) / 128.0), 0.0, 1.0);
}