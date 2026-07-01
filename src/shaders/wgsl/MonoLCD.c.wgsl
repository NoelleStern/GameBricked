///
/// Adapted from:
///
///     https://github.com/LIJI32/SameBoy/blob/master/Shaders/MonoLCD.fsh
///


const SCANLINE_DEPTH: f32 = 0.25;
const BLOOM: f32 = 0.4;


fn scale(
    image: texture_2d<f32>, 
    img_sampler: sampler, 
    position: vec2<f32>, 
    input_resolution: vec2<f32>, 
    output_resolution: vec2<f32>
) -> vec4<f32> {
    var pixel = position * input_resolution - vec2<f32>(0.5, 0.5);

    var q11 = textureSample(image, img_sampler, (floor(pixel) + 0.5) / input_resolution);
    var q12 = textureSample(image, img_sampler, (vec2<f32>(floor(pixel.x), ceil(pixel.y)) + 0.5) / input_resolution);
    var q21 = textureSample(image, img_sampler, (vec2<f32>(ceil(pixel.x), floor(pixel.y)) + 0.5) / input_resolution);
    var q22 = textureSample(image, img_sampler, (ceil(pixel) + 0.5) / input_resolution);

    let s = smoothstep(vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), fract(pixel));

    var r1 = mix(q11, q21, s.x);
    var r2 = mix(q12, q22, s.x);
    
    let pos = fract(position * input_resolution);
    let sub_pos = pos * 6.0;

    var multiplier: f32 = 1.0;
    
    if sub_pos.y < 1.0 {
        multiplier *= sub_pos.y * SCANLINE_DEPTH + (1.0 - SCANLINE_DEPTH);
    } 
    else if sub_pos.y > 5.0 {
        multiplier *= (6.0 - sub_pos.y) * SCANLINE_DEPTH + (1.0 - SCANLINE_DEPTH);
    }
    
    if sub_pos.x < 1.0 {
        multiplier *= sub_pos.x * SCANLINE_DEPTH + (1.0 - SCANLINE_DEPTH);
    } 
    else if sub_pos.x > 5.0 {
        multiplier *= (6.0 - sub_pos.x) * SCANLINE_DEPTH + (1.0 - SCANLINE_DEPTH);
    }

    var pre_shadow = mix(textureSample(image, img_sampler, position) * multiplier, mix(r1, r2, s.y), BLOOM);
    pre_shadow.a = 1.0;
    
    pixel += vec2<f32>(-0.6, -0.8);
    
    q11 = textureSample(image, img_sampler, (floor(pixel) + 0.5) / input_resolution);
    q12 = textureSample(image, img_sampler, (vec2<f32>(floor(pixel.x), ceil(pixel.y)) + 0.5) / input_resolution);
    q21 = textureSample(image, img_sampler, (vec2<f32>(ceil(pixel.x), floor(pixel.y)) + 0.5) / input_resolution);
    q22 = textureSample(image, img_sampler, (ceil(pixel) + 0.5) / input_resolution);
   
    r1 = mix(q11, q21, fract(pixel.x));
    r2 = mix(q12, q22, fract(pixel.x));
    
    let shadow = mix(r1, r2, fract(pixel.y));
    return mix(min(shadow, pre_shadow), pre_shadow, 0.75);
}

fn effect(in: VertexOutput) -> vec4<f32> {
    return scale(tex, sam, in.uv, vec2<f32>(160.0, 144.0), globals.resolution);
}