/// 
/// Adapted from:
///
///     https://www.shadertoy.com/view/lsSSD1
/// 


const SCANLINE_SPEED: f32 = 0.05;
const GB_RES = vec2<f32>(160.0, 144.0);


fn effect(in: VertexOutput) -> vec4<f32> {
    let pixel_size = 1.0 / GB_RES;
    var pixel_pos = in.uv;
    
    // Grab color
    let cell_index = floor(pixel_pos * GB_RES);
    let tex_pos = (cell_index + vec2<f32>(0.5, 0.5)) * pixel_size;
    var color = textureSample(tex, sam, tex_pos).rgb;
    
    // Brighten scanlines
    let line_y = fract(globals.time * SCANLINE_SPEED);
    if (abs(pixel_pos.y - line_y) < (pixel_size.y / 4.0)) { color *= 1.5; }
    
    // Make circular
    var mod_pos = fract(pixel_pos * GB_RES);
    let centered_mod = mod_pos - vec2<f32>(0.5, 0.5);
    let dist_sq = centered_mod.x * centered_mod.x + centered_mod.y * centered_mod.y;
    color *= (1.0 - sqrt(dist_sq));
    
    // Color shift
    let r_mask = pow(1.0 - abs(mod_pos.x - 0.25), 2.0);
    let g_mask = pow(1.0 - abs(mod_pos.x - 0.50), 2.0);
    let b_mask = pow(1.0 - abs(mod_pos.x - 0.75), 2.0);
    color *= vec3<f32>(r_mask, g_mask, b_mask);
    color *= vec3<f32>(0.8, 0.75, 0.9);
    color *= 3; // Brightness compensation
    
    // Edge darken
    let edge_dist = abs(pixel_pos.x - 0.5);
    color *= (0.9 - edge_dist);
    
    return vec4<f32>(color, 1.0);
}