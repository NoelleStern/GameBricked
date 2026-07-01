///
/// Adapted from:
///
///     https://github.com/LIJI32/SameBoy/blob/master/Shaders/Scale2x.fsh
///


fn texture_relative(
    image: texture_2d<f32>, 
    img_sampler: sampler, 
    position: vec2<f32>, 
    offset: vec2<f32>, 
    input_resolution: vec2<f32>
) -> vec4<f32> {
    let uv = position + (offset / input_resolution);
    return textureSample(image, img_sampler, uv);
}

// Vector comparison helpers
fn equal(v1: vec4<f32>, v2: vec4<f32>) -> bool { return all(v1 == v2); }
fn unequal(v1: vec4<f32>, v2: vec4<f32>) -> bool { return any(v1 != v2); }

fn scale(
    image: texture_2d<f32>, 
    img_sampler: sampler, 
    position: vec2<f32>, 
    input_resolution: vec2<f32>, 
    output_resolution: vec2<f32>
) -> vec4<f32> {
    // texel arrangement
    // A B C
    // D E F
    // G H I

    let B = texture_relative(image, img_sampler, position, vec2<f32>(0.0, 1.0), input_resolution);
    let D = texture_relative(image, img_sampler, position, vec2<f32>(-1.0, 0.0), input_resolution);
    let E = texture_relative(image, img_sampler, position, vec2<f32>(0.0, 0.0), input_resolution);
    let F = texture_relative(image, img_sampler, position, vec2<f32>(1.0, 0.0), input_resolution);
    let H = texture_relative(image, img_sampler, position, vec2<f32>(0.0, -1.0), input_resolution);
    
    var p = position * input_resolution;
    // p = the position within a pixel [0...1]
    p = fract(p);
    
    if p.x > 0.5 {
        if p.y > 0.5 { // Top Right
            let cond = equal(B, F) && unequal(B, D) && unequal(F, H);
            return select(E, F, cond);
        } else { // Bottom Right
            let cond = equal(H, F) && unequal(D, H) && unequal(B, F);
            return select(E, F, cond);
        }
    } else {
        if p.y > 0.5 { // Top Left
            let cond = equal(D, B) && unequal(B, F) && unequal(D, H);
            return select(E, D, cond);
        } else { // Bottom Left
            let cond = equal(D, H) && unequal(D, B) && unequal(H, F);
            return select(E, D, cond);
        }
    }
}

fn effect(in: VertexOutput) -> vec4<f32> {
    return scale(tex, sam, in.uv, vec2<f32>(160.0, 144.0), globals.resolution);
}