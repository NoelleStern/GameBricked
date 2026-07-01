///
/// Adapted from:
///
///     https://www.shadertoy.com/view/XsjSzR
///


const edge_thickness: f32   = 0.01;
const hardScan: f32         = -8.0;
const hardPix: f32          = -3.0;
const warp: vec2<f32>       = vec2<f32>(1.0/64.0, 1.0/48.0); // The smaller the denominator, the more warped it is
const maskDark: f32         = 0.5;
const maskLight: f32        = 1.5;


// Get texture resolution
fn get_res() -> vec2<f32> {
    return vec2<f32>(textureDimensions(tex));
}

// sRGB to Linear
fn ToLinear1(c: f32) -> f32 {
    if (c <= 0.04045) { return c / 12.92; }
    return pow(max((c + 0.055) / 1.055, 0.0), 2.4); 
}
fn ToLinear(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(ToLinear1(c.r), ToLinear1(c.g), ToLinear1(c.b));
}

// Linear to sRGB
fn ToSRGB1(c: f32) -> f32 {
    if (c < 0.0031308) { return c * 12.92; }
    return 1.055 * pow(max(c, 0.0), 0.41666) - 0.055;
}
fn ToSRGB(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(ToSRGB1(c.r), ToSRGB1(c.g), ToSRGB1(c.b));
}

// Nearest emulated sample given floating point position and texel offset
fn Fetch(pos: vec2<f32>, off: vec2<f32>) -> vec3<f32> {
    let res = get_res();
    let uv = (floor(pos * res + off) + 0.5) / res;
    let c = textureSampleLevel(tex, sam, uv, 0.0).rgb;
    return ToLinear(c);
}

// Distance in emulated pixels to nearest texel
fn Dist(pos: vec2<f32>) -> vec2<f32> {
    let p = pos * get_res();
    return -((p - floor(p)) - vec2<f32>(0.5));
}

// 1D Gaussian
fn Gauss(pos: f32, scale: f32) -> f32 {
    return exp2(scale * pos * pos);
}

// 3-tap Gaussian filter along horizontal line
fn Horizontal3(pos: vec2<f32>, off: f32) -> vec3<f32> {
    let b = Fetch(pos, vec2<f32>(-1.0, off));
    let c = Fetch(pos, vec2<f32>( 0.0, off));
    let d = Fetch(pos, vec2<f32>( 1.0, off));
    
    let dst = Dist(pos).x;
    let scale = hardPix;
    let wb = Gauss(dst - 1.0, scale);
    let wc = Gauss(dst + 0.0, scale);
    let wd = Gauss(dst + 1.0, scale);
    
    return (b * wb + c * wc + d * wd) / (wb + wc + wd);
}

// 5-tap Gaussian filter along horizontal line
fn Horizontal5(pos: vec2<f32>, off: f32) -> vec3<f32> {
    let a = Fetch(pos, vec2<f32>(-2.0, off));
    let b = Fetch(pos, vec2<f32>(-1.0, off));
    let c = Fetch(pos, vec2<f32>( 0.0, off));
    let d = Fetch(pos, vec2<f32>( 1.0, off));
    let e = Fetch(pos, vec2<f32>( 2.0, off));
    
    let dst = Dist(pos).x;
    let scale = hardPix;
    let wa = Gauss(dst - 2.0, scale);
    let wb = Gauss(dst - 1.0, scale);
    let wc = Gauss(dst + 0.0, scale);
    let wd = Gauss(dst + 1.0, scale);
    let we = Gauss(dst + 2.0, scale);
    
    return (a * wa + b * wb + c * wc + d * wd + e * we) / (wa + wb + wc + wd + we);
}

// Return scanline weight
fn Scan(pos: vec2<f32>, off: f32) -> f32 {
    let dst = Dist(pos).y;
    return Gauss(dst + off, hardScan);
}

// Allow nearest three lines to effect pixel
fn Tri(pos: vec2<f32>) -> vec3<f32> {
    let a = Horizontal3(pos, -1.0);
    let b = Horizontal5(pos,  0.0);
    let c = Horizontal3(pos,  1.0);
    let wa = Scan(pos, -1.0);
    let wb = Scan(pos,  0.0);
    let wc = Scan(pos,  1.0);
    return a * wa + b * wb + c * wc;
}

// Distortion of scanlines, and end of screen alpha
fn Warp(pos: vec2<f32>) -> vec2<f32> {
    var p = pos * 2.0 - 1.0;
    p = p * vec2<f32>(1.0 + (p.y * p.y) * warp.x, 1.0 + (p.x * p.x) * warp.y);
    return p * 0.5 + 0.5;
}

// Shadow mask
fn Mask(pos: vec2<f32>) -> vec3<f32> {
    var p = pos;
    p.x = p.x + p.y * 3.0;
    var mask = vec3<f32>(maskDark, maskDark, maskDark);
    p.x = fract(p.x / 6.0);
    
    if (p.x < 0.333) { mask.r = maskLight; } 
    else if (p.x < 0.666) { mask.g = maskLight; } 
    else { mask.b = maskLight; }

    return mask;
}

// Draw dividing bars
fn Bar(pos: f32, bar: f32) -> f32 {
    let p = pos - bar;
    if (p * p < 4.0) { return 0.0; }
    return 1.0;
}

// Set edge transparency
fn Fade(pos: vec2<f32>) -> f32 {
    let fade_x = smoothstep(0.0, edge_thickness, pos.x) * smoothstep(1.0, 1.0 - edge_thickness, pos.x);
    let fade_y = smoothstep(0.0, edge_thickness, pos.y) * smoothstep(1.0, 1.0 - edge_thickness, pos.y);
    return fade_x * fade_y;
}

fn effect(in: VertexOutput) -> vec4<f32> {
    let fc = in.pos.xy;
    let pos = Warp(in.uv); // Warp distort the edges

    // Hard cuts to transparency
    // if (pos.x < 0.0 || pos.x > 1.0 || pos.y < 0.0 || pos.y > 1.0) {
    //    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    // }

    let color = Tri(pos) * Mask(fc);
    
    let bezel_fade = Fade(pos);
    return vec4<f32>(ToSRGB(color), bezel_fade);
}