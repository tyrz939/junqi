// The G-buffer (PRESENTATION.md §1.7 T2): terrain chunks and sprites in pass order into three
// targets, the albedo (linear), normal and height (nx, ny, height / 255, depth / 255), the
// emissive (linear) and whose each px is (a sprite's index in the frame plus one; 0 terrain). Sprites read four atlas layers in one fragment: albedo and emissive are
// master-palette indices into the 1024-entry CLUT (PRESENTATION.md §1.4, "Why 16-bit").

@group(0) @binding(1) var clut: texture_2d<f32>;
@group(0) @binding(2) var atlas_albedo: texture_2d_array<u32>;
@group(0) @binding(3) var atlas_normal: texture_2d_array<f32>;
@group(0) @binding(4) var atlas_emissive: texture_2d_array<u32>;
@group(0) @binding(5) var atlas_height: texture_2d_array<f32>;
@group(0) @binding(6) var chunk_albedo: texture_2d_array<f32>;
@group(0) @binding(7) var chunk_nh: texture_2d_array<f32>;
@group(0) @binding(8) var chunk_emissive: texture_2d_array<f32>;

struct GOut {
    @location(0) albedo: vec4<f32>,
    @location(1) nh: vec4<f32>,
    @location(2) emissive: vec4<f32>,
    @location(3) id: u32,
};

fn to_clip(p: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(p.x / g.full.x * 2.0 - 1.0, 1.0 - p.y / g.full.y * 2.0, 0.0, 1.0);
}

// ---- Terrain chunks: one instance a chunk, 256 px square, its layers at `at.z` of the arrays.

struct ChunkOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) layer: i32,
};

@vertex
fn vs_chunk(@builtin(vertex_index) vi: u32, @location(0) at: vec4<i32>) -> ChunkOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let p = vec2<f32>(f32(at.x), f32(at.y)) + g.guard + corner * 256.0;
    var o: ChunkOut;
    o.pos = to_clip(p);
    o.local = corner * 256.0;
    o.layer = at.z;
    return o;
}

@fragment
fn fs_chunk(i: ChunkOut) -> GOut {
    let t = vec2<i32>(floor(i.local));
    var o: GOut;
    // Its alpha is the ground's surface byte: water, wetness, or the sky beyond the zone.
    o.albedo = textureLoad(chunk_albedo, t, i.layer, 0);
    o.nh = textureLoad(chunk_nh, t, i.layer, 0);
    o.emissive = vec4<f32>(textureLoad(chunk_emissive, t, i.layer, 0).rgb, 1.0);
    o.id = 0u;
    return o;
}

// ---- Sprites: one instance a sprite, in draw order.

struct SpriteOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    // Source rect x, y, w, h in the page.
    @location(1) @interpolate(flat) src: vec4<u32>,
    // Page, flags (bit 0 mirror, bits 8..16 tint amount, bits 16..18 tint kind), depth px, id.
    @location(2) @interpolate(flat) info: vec4<u32>,
    // How much its heights count under what is drawn (`Caster::sink`), px.
    @location(3) @interpolate(flat) sink: u32,
    // The rows it burns, first | last << 16 (`Caster::burn`): they stand in no field.
    @location(4) @interpolate(flat) burn: u32,
};

@vertex
fn vs_sprite(
    @builtin(vertex_index) vi: u32,
    @location(0) src: vec4<u32>,
    @location(1) dst: vec4<i32>,
    @location(2) extra: vec4<u32>,
) -> SpriteOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let size = vec2<f32>(f32(src.z), f32(src.w));
    let p = vec2<f32>(f32(dst.x), f32(dst.y)) + g.guard + corner * size;
    var o: SpriteOut;
    o.pos = to_clip(p);
    o.local = corner * size;
    o.src = src;
    o.info = vec4<u32>(u32(dst.z), u32(dst.w), extra.x, extra.y);
    o.sink = extra.z;
    o.burn = extra.w;
    return o;
}

// The texel of the sprite under this fragment (a mirrored frame walks its columns backwards).
fn texel(i: SpriteOut) -> vec2<i32> {
    let l = vec2<u32>(floor(i.local));
    let mirror = (i.info.y & 1u) != 0u;
    let sx = select(l.x, i.src.z - 1u - l.x, mirror);
    return vec2<i32>(i32(i.src.x + sx), i32(i.src.y + l.y));
}

fn clut_at(ix: u32) -> vec3<f32> {
    return textureLoad(clut, vec2<i32>(i32(ix), 0), 0).rgb;
}

// Opaque texels: all three targets. Clear and the contact shadow are the other pipelines'.
@fragment
fn fs_sprite(i: SpriteOut) -> GOut {
    let t = texel(i);
    let page = i32(i.info.x);
    let ix = textureLoad(atlas_albedo, t, page, 0).r;
    if ix <= 1u {
        discard;
    }
    var c = clut_at(ix);
    let kind = (i.info.y >> 16u) & 3u;
    let a = f32((i.info.y >> 8u) & 255u) / 255.0;
    if kind == 1u {
        c = mix(c, vec3<f32>(1.0), a);
    }
    var n = textureLoad(atlas_normal, t, page, 0).rg;
    if (i.info.y & 1u) != 0u {
        // Mirrored: the normal's x flips about 128.
        n.x = 256.0 / 255.0 - n.x;
    }
    // It stands on what is drawn: its heights less what they counted under its lowest drawn px.
    var h = textureLoad(atlas_height, t, page, 0).r;
    if h > 0.0 {
        h = max(h - f32(i.sink) / 255.0, 1.0 / 255.0);
    }
    let e = textureLoad(atlas_emissive, t, page, 0).r;
    var ec = vec3<f32>(0.0);
    if e > 1u {
        ec = clut_at(e);
    }
    var o: GOut;
    o.albedo = vec4<f32>(c, 1.0);
    // A row it burns (a flame, and the line round it) is light, not matter: no depth, no field.
    let row = u32(floor(i.local.y));
    let burns = row >= (i.burn & 0xffffu) && row <= (i.burn >> 16u);
    o.nh = vec4<f32>(n, h, select(f32(i.info.z) / 255.0, 0.0, burns));
    o.emissive = vec4<f32>(ec, 1.0);
    o.id = i.info.w;
    return o;
}

// The contact shadow (index 1): a cool multiply of the albedo under it (`AO_TINT`, never grey,
// ART.md §3.1), softened by how much of each texel's 3 x 3 the index-1 mask covers, so a clear
// texel beside the mask takes a little of it (`jane_art::palette::ao`, in linear light). Drawn
// for a whole pass before its opaque texels, so it lies on the ground and never on the thing
// standing behind. Blended as `dst * src`.
@fragment
fn fs_contact(i: SpriteOut) -> @location(0) vec4<f32> {
    let page = i32(i.info.x);
    let t = texel(i);
    let ix = textureLoad(atlas_albedo, t, page, 0).r;
    if ix > 1u {
        discard;
    }
    let lo = vec2<i32>(i32(i.src.x), i32(i.src.y));
    let hi = lo + vec2<i32>(i32(i.src.z), i32(i.src.w)) - 1;
    var cover = 0.0;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let q = t + vec2<i32>(dx, dy);
            if all(q >= lo) && all(q <= hi) && textureLoad(atlas_albedo, q, page, 0).r == 1u {
                cover += 1.0;
            }
        }
    }
    if cover == 0.0 {
        discard;
    }
    let f = 1.0 - (1.0 - AO_TINT) * cover / 9.0;
    return vec4<f32>(pow(f, vec3<f32>(2.2)), 1.0);
}

// A ghost: the albedo alone, over what is under it; it neither casts nor catches a height.
@fragment
fn fs_ghost(i: SpriteOut) -> @location(0) vec4<f32> {
    let ix = textureLoad(atlas_albedo, texel(i), i32(i.info.x), 0).r;
    if ix <= 1u {
        discard;
    }
    let a = f32((i.info.y >> 8u) & 255u) / 255.0;
    return vec4<f32>(clut_at(ix), a);
}
