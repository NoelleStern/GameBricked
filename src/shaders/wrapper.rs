//!
//! This is a unified wrapper for WGPU shaders:
//! 
//!     It makes it easier for me to write shaders.
//!     It also opens up a possibility of adding custom user shaders.
//!     I think it's pretty clean and straightforward - kinda cool!
//! 


const API_WRAPPER: &str =
r#"struct Globals {
    resolution: vec2<f32>,
    time: f32,
    frame: u32,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var sam: sampler;

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn resolution() -> vec2<f32> {
    return globals.resolution;
}

fn time() -> f32 {
    return globals.time;
}

fn sample_texture(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(tex, sam, uv);
}

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(i & 1u) << 2u) - 1.0;
    let y = f32(i32(i & 2u) << 1u) - 1.0;
    
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, 1.0 - (y * 0.5 + 0.5));
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Force bindings to remain active
    _ = globals.time;
    _ = textureDimensions(tex);

    return effect(in);
}
"#;

pub struct WgslWrapper;
impl WgslWrapper {
    pub fn wrap(user_function: &str) -> String {
        format!("{}\r\n\r\n{}", user_function, API_WRAPPER) // In this order so that it's easier to debug
    }
}