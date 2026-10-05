// Bloom on the emissive (a chain of halvings and a tent back up), then the grade per region and
// hour (exposure, a soft shoulder that keeps the art's colours true below white, saturation,
// tint and a coloured lift into the shadows) into the canvas. Then the window's upscale.

@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;
// For the grade: the bloom, full size.
@group(0) @binding(3) var bloom: texture_2d<f32>;

struct Step {
    // The source's texel size (upscale: the canvas's top-left on the window, px).
    texel: vec2<f32>,
    // Upscale: output px per canvas px; the canvas's size in px.
    scale: f32,
    // Upscale: 1 draws nearest (the `sharp` row off).
    nearest: f32,
};

@group(1) @binding(0) var<uniform> st: Step;

// A 13-tap downsample: the four 2 x 2 boxes round the centre and the centre's own, weighted
// so no single bright texel sparkles.
@fragment
fn fs_down(i: FullOut) -> @location(0) vec4<f32> {
    let t = st.texel;
    let uv = i.uv;
    let a = textureSample(src, smp, uv + t * vec2<f32>(-2.0, -2.0)).rgb;
    let b = textureSample(src, smp, uv + t * vec2<f32>(0.0, -2.0)).rgb;
    let c = textureSample(src, smp, uv + t * vec2<f32>(2.0, -2.0)).rgb;
    let d = textureSample(src, smp, uv + t * vec2<f32>(-2.0, 0.0)).rgb;
    let e = textureSample(src, smp, uv).rgb;
    let f = textureSample(src, smp, uv + t * vec2<f32>(2.0, 0.0)).rgb;
    let gg = textureSample(src, smp, uv + t * vec2<f32>(-2.0, 2.0)).rgb;
    let h = textureSample(src, smp, uv + t * vec2<f32>(0.0, 2.0)).rgb;
    let k = textureSample(src, smp, uv + t * vec2<f32>(2.0, 2.0)).rgb;
    let l = textureSample(src, smp, uv + t * vec2<f32>(-1.0, -1.0)).rgb;
    let m = textureSample(src, smp, uv + t * vec2<f32>(1.0, -1.0)).rgb;
    let n = textureSample(src, smp, uv + t * vec2<f32>(-1.0, 1.0)).rgb;
    let o = textureSample(src, smp, uv + t * vec2<f32>(1.0, 1.0)).rgb;
    var s = e * 0.125;
    s += (a + c + gg + k) * 0.03125;
    s += (b + d + f + h) * 0.0625;
    s += (l + m + n + o) * 0.125;
    return vec4<f32>(s, 1.0);
}

// A 3 x 3 tent upsample, added onto the level above.
@fragment
fn fs_up(i: FullOut) -> @location(0) vec4<f32> {
    let t = st.texel;
    let uv = i.uv;
    var s = textureSample(src, smp, uv).rgb * 4.0;
    s += (textureSample(src, smp, uv + vec2<f32>(-t.x, 0.0)).rgb + textureSample(src, smp, uv + vec2<f32>(t.x, 0.0)).rgb
        + textureSample(src, smp, uv + vec2<f32>(0.0, -t.y)).rgb + textureSample(src, smp, uv + vec2<f32>(0.0, t.y)).rgb) * 2.0;
    s += textureSample(src, smp, uv + vec2<f32>(-t.x, -t.y)).rgb + textureSample(src, smp, uv + vec2<f32>(t.x, -t.y)).rgb
        + textureSample(src, smp, uv + vec2<f32>(-t.x, t.y)).rgb + textureSample(src, smp, uv + vec2<f32>(t.x, t.y)).rgb;
    return vec4<f32>(s / 16.0, 1.0);
}

fn shoulder(x: f32) -> f32 {
    let knee = 0.78;
    if x <= knee {
        return x;
    }
    return knee + (1.0 - knee) * (1.0 - exp(-(x - knee) / (1.0 - knee)));
}

@fragment
fn fs_grade(i: FullOut) -> @location(0) vec4<f32> {
    let px = vec2<i32>(floor(i.pos.xy));
    var c = textureLoad(src, px, 0).rgb;
    c += textureSample(bloom, smp, i.uv).rgb * g.misc.x;
    // The afterglow across the frame at dusk and dawn (§1.9): the side toward where the sun
    // went down (came up) warmer and a little lighter, fading across the view; and the far
    // (top) edge of the view a little toward the horizon's colour, the air between.
    if g.glow.w > 0.0 {
        let x = f32(px.x);
        let toward = clamp(1.0 - abs(x - g.horizon.w) / (g.canvas.x * 1.2), 0.0, 1.0);
        let k = g.glow.w * toward * toward;
        let air = air_glow(g.glow.rgb);
        let warm = air / max(max(air.r, air.g), max(air.b, 0.001));
        c = c * mix(vec3<f32>(1.0), 0.8 + warm * 0.45, k * 0.55) + air * k * 0.02;
        let far = clamp(1.0 - f32(px.y) / g.canvas.y, 0.0, 1.0);
        c = mix(c, (g.horizon.rgb + air) * 0.25, far * far * g.glow.w * 0.07);
    }
    c *= g.lift.w;
    c = vec3<f32>(shoulder(c.r), shoulder(c.g), shoulder(c.b));
    let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    c = max(mix(vec3<f32>(luma), c, g.tint.w), vec3<f32>(0.0));
    c *= g.tint.rgb;
    let dark = (1.0 - c) * (1.0 - c);
    c += g.lift.rgb * dark;
    return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// The window: sharp bilinear. Nearest inside a canvas texel, bilinear across the last output
// pixel at its edge, so every pixel is square and no edge swims at 2.5x.
@fragment
fn fs_upscale(i: FullOut) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(src));
    let texel = (i.pos.xy - st.texel) / st.scale;
    let base = floor(texel);
    let f = texel - base - 0.5;
    let region = select(0.5 - 0.5 / st.scale, 0.5, st.nearest > 0.5);
    let off = (f - clamp(f, vec2<f32>(-region), vec2<f32>(region))) * st.scale + 0.5;
    return textureSampleLevel(src, smp, (base + off) / size, 0.0);
}
