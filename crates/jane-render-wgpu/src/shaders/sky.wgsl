// The sky backdrop (PRESENTATION.md §1.9 `Sky`, `FarLandmark`, `FarTreeline`): a texture the
// canvas's width and 256 rows tall whose row `y` is `y` px above the horizon, so it is the sky
// already mirrored as water shows it; the canvas beyond the zone's top edge reads it the other
// way up. The gradient by hour, the afterglow low in the sun's bearing, the stars, then the moon,
// the School and the treeline as sprites standing on the horizon, unlit but for the sky's own
// light and a haze of distance, their lit windows glowing.

@group(0) @binding(1) var clut: texture_2d<f32>;
@group(0) @binding(2) var atlas_albedo: texture_2d_array<u32>;
@group(0) @binding(3) var atlas_emissive: texture_2d_array<u32>;

// Px from the horizon to the zenith (`jane_present::frame::ZENITH_PX`).
const ZENITH = 240.0;

fn sky_colour(x: f32, up: f32) -> vec3<f32> {
    let t = clamp(up / ZENITH, 0.0, 1.0);
    // Most of the change is low: the horizon's band is narrow.
    let k = 1.0 - (1.0 - t) * (1.0 - t);
    var c = mix(g.horizon.rgb, g.zenith.rgb, k);
    let across = clamp(1.0 - abs(x - g.horizon.w) / 520.0, 0.0, 1.0);
    let low = clamp(1.0 - up / 150.0, 0.0, 1.0);
    c = mix(c, g.glow.rgb, across * low * g.glow.w * 0.75);
    return c;
}

@fragment
fn fs_sky(i: FullOut) -> @location(0) vec4<f32> {
    let p = floor(i.pos.xy);
    return vec4<f32>(sky_colour(p.x, p.y), 1.0);
}

// ---- Stars: one instance a star, a px (the brightest a small cross), added.

struct StarOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) bright: f32,
};

fn sky_clip(p: vec2<f32>, size: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(p.x / size.x * 2.0 - 1.0, 1.0 - p.y / size.y * 2.0, 0.0, 1.0);
}

@vertex
fn vs_star(@builtin(vertex_index) vi: u32, @location(0) s: vec4<f32>) -> StarOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let size = vec2<f32>(g.canvas.x, 256.0);
    let r = select(0.5, 1.0, s.z > 0.8);
    let c = vec2<f32>(s.x + 0.5, s.y + 0.5);
    var o: StarOut;
    o.pos = sky_clip(c + (corner * 2.0 - 1.0) * r, size);
    o.bright = s.z;
    return o;
}

@fragment
fn fs_star(i: StarOut) -> @location(0) vec4<f32> {
    return vec4<f32>(vec3<f32>(0.9, 0.92, 1.0) * i.bright * 1.4, 1.0);
}

// ---- The far things: sprites standing on the horizon, flipped as the backdrop is.

struct FarOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) src: vec4<u32>,
    @location(2) @interpolate(flat) page: i32,
    @location(3) @interpolate(flat) haze: f32,
};

@vertex
fn vs_far(
    @builtin(vertex_index) vi: u32,
    @location(0) src: vec4<u32>,
    @location(1) dst: vec4<i32>,
    @location(2) extra: vec4<u32>,
) -> FarOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let size = vec2<f32>(f32(src.z), f32(src.w));
    // Its top is `-dst.y` px up; row `sy` of it is `-dst.y - sy` up, which is the texture's row.
    let p = vec2<f32>(f32(dst.x) + corner.x * size.x, -f32(dst.y) - corner.y * size.y);
    var o: FarOut;
    o.pos = sky_clip(p, vec2<f32>(g.canvas.x, 256.0));
    o.local = corner * size;
    o.src = src;
    o.page = dst.z;
    o.haze = f32(extra.x) / 255.0;
    return o;
}

@fragment
fn fs_far(i: FarOut) -> @location(0) vec4<f32> {
    let l = vec2<u32>(floor(i.local));
    let t = vec2<i32>(i32(i.src.x + l.x), i32(i.src.y + l.y));
    let ix = textureLoad(atlas_albedo, t, i.page, 0).r;
    if ix <= 1u {
        discard;
    }
    let up = floor(i.pos.y);
    let x = floor(i.pos.x);
    let behind = sky_colour(x, up);
    let a = textureLoad(clut, vec2<i32>(i32(ix), 0), 0).rgb;
    // Lit by the sky alone, from behind: a silhouette a little lighter than its own dark, and
    // hazed toward the sky it stands against by its distance.
    var c = a * (g.fill.rgb * 0.7 + g.horizon.rgb * 0.35);
    c = mix(c, behind, i.haze);
    let e = textureLoad(atlas_emissive, t, i.page, 0).r;
    if e > 1u {
        c += textureLoad(clut, vec2<i32>(i32(e), 0), 0).rgb * g.misc.z * 1.2;
    }
    return vec4<f32>(c, 1.0);
}
