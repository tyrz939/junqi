// The Ui pass (PRESENTATION.md §3.1; jane_present::ui::cmd): fills, atlas sprites and run-time
// images as quads over the graded canvas, unlit, straight alpha. Sampling is nearest by
// integer division, as soft does: destination px (dx, dy) of a quad reads source texel
// (src.x + dx * src.w / dst.w, src.y + dy * src.h / dst.h).

struct UiGlobals {
    // The canvas, px.
    canvas: vec2<f32>,
    pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: UiGlobals;
@group(0) @binding(1) var albedo: texture_2d_array<u32>;
@group(0) @binding(2) var clut: texture_2d<f32>;
@group(1) @binding(0) var image: texture_2d<f32>;

struct Inst {
    // Destination rect, canvas px: x, y, w, h.
    @location(0) dst: vec4<i32>,
    // Source rect, texels: x, y, w, h.
    @location(1) src: vec4<u32>,
    // kind (0 fill, 1 sprite, 2 image), page, ink (0: the texel's own), alpha | mirror << 8.
    @location(2) info: vec4<u32>,
    // A fill's colour, 0xAARRGGBB.
    @location(3) argb: u32,
};

struct Out {
    @builtin(position) pos: vec4<f32>,
    // Where in the destination rect, px from its top-left.
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) dst: vec4<i32>,
    @location(2) @interpolate(flat) src: vec4<u32>,
    @location(3) @interpolate(flat) info: vec4<u32>,
    @location(4) @interpolate(flat) argb: u32,
};

@vertex
fn vs_ui(@builtin(vertex_index) vi: u32, i: Inst) -> Out {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let size = vec2<f32>(f32(i.dst.z), f32(i.dst.w));
    let p = vec2<f32>(f32(i.dst.x), f32(i.dst.y)) + corner * size;
    var o: Out;
    o.pos = vec4<f32>(p.x / u.canvas.x * 2.0 - 1.0, 1.0 - p.y / u.canvas.y * 2.0, 0.0, 1.0);
    o.local = corner * size;
    o.dst = i.dst;
    o.src = i.src;
    o.info = i.info;
    o.argb = i.argb;
    return o;
}

// The canvas is written through a plain view: colours go out sRGB-encoded, as stored.
fn linear_to_srgb(c: f32) -> f32 {
    if (c <= 0.0031308) {
        return c * 12.92;
    }
    return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}

fn unpack(argb: u32) -> vec4<f32> {
    let r = f32((argb >> 16u) & 255u) / 255.0;
    let g = f32((argb >> 8u) & 255u) / 255.0;
    let b = f32(argb & 255u) / 255.0;
    let a = f32((argb >> 24u) & 255u) / 255.0;
    return vec4<f32>(r, g, b, a);
}

// The source texel for this fragment: nearest, by integer division.
fn texel(in: Out) -> vec2<i32> {
    let d = vec2<i32>(floor(in.local));
    var dx = d.x;
    if ((in.info.w >> 8u) != 0u) {
        dx = in.dst.z - 1 - dx;
    }
    let sx = i32(in.src.x) + dx * i32(in.src.z) / max(in.dst.z, 1);
    let sy = i32(in.src.y) + d.y * i32(in.src.w) / max(in.dst.w, 1);
    return vec2<i32>(sx, sy);
}

@fragment
fn fs_ui(in: Out) -> @location(0) vec4<f32> {
    let alpha = f32(in.info.w & 255u) / 255.0;
    let kind = in.info.x;
    if (kind == 0u) {
        return unpack(in.argb);
    }
    if (kind == 1u) {
        let t = texel(in);
        let ix = textureLoad(albedo, t, i32(in.info.y), 0).r;
        if (ix == 0u) {
            discard;
        }
        if (ix == 1u) {
            // The contact shadow: what is under it to three quarters (in the stored bytes).
            return vec4<f32>(0.0, 0.0, 0.0, 0.25 * alpha);
        }
        var at = ix;
        if (in.info.z != 0u) {
            at = in.info.z;
        }
        // The CLUT is an sRGB texture: loaded, it is linear; stored bytes are sRGB.
        let c = textureLoad(clut, vec2<i32>(i32(at), 0), 0);
        return vec4<f32>(linear_to_srgb(c.r), linear_to_srgb(c.g), linear_to_srgb(c.b), alpha);
    }
    let c = textureLoad(image, texel(in), 0);
    if (c.a == 0.0) {
        discard;
    }
    return vec4<f32>(c.rgb, c.a * alpha);
}
