// Particles (PRESENTATION.md §2, T2), over what is lit: one instance a part, a quad round its
// shape. A stroke fades toward its tail; a dot is a square; a ring is one px thick and squashed
// to half its height on the ground; a glow is a soft disc. Each is lit where it stands, by the
// sky's light and the lamps of its tile (a raindrop glints as it falls through a lamp's pool),
// and what glows adds its colour unlit, into the bloom too.

struct Light {
    pos: vec4<f32>,
    col: vec4<f32>,
    spot: vec4<f32>,
};

@group(0) @binding(1) var<storage, read> lights: array<Light>;
@group(0) @binding(2) var<storage, read> tiles: array<vec2<u32>>;
@group(0) @binding(3) var<storage, read> tile_lights: array<u32>;

struct PartOut {
    @builtin(position) pos: vec4<f32>,
    // Px from the part's anchor.
    @location(0) local: vec2<f32>,
    // x: shape, y..: its params.
    @location(1) @interpolate(flat) shape: vec4<f32>,
    // xyz: linear colour lit; w: alpha.
    @location(2) @interpolate(flat) col: vec4<f32>,
    // x: glow.
    @location(3) @interpolate(flat) glow: f32,
};

fn canvas_clip(p: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(p.x / g.canvas.x * 2.0 - 1.0, 1.0 - p.y / g.canvas.y * 2.0, 0.0, 1.0);
}

// The light where a part stands: the sky's, and every lamp of its tile, unshadowed.
fn light_at(at: vec2<f32>, height: f32) -> vec3<f32> {
    var light = g.fill.rgb + g.sun_col.rgb * 0.6 + vec3<f32>(0.55, 0.6, 0.8) * g.weather.w;
    let t = vec2<u32>(clamp(at, vec2<f32>(0.0), g.canvas - 1.0)) / 32u;
    let tr = tiles[t.y * g.tiles_x + t.x];
    // The part is `height` px up, over the ground `height` px below it on screen.
    let p = vec3<f32>(at.x + g.guard, at.y + height + g.guard, height);
    for (var k = 0u; k < tr.y; k++) {
        let lt = lights[tile_lights[tr.x + k]];
        let v = lt.pos.xyz - p;
        let dist = length(v);
        let r = lt.pos.w;
        if dist < r {
            let x = dist / r;
            let w = clamp(1.0 - x * x, 0.0, 1.0);
            light += lt.col.rgb * w * w / (1.0 + 4.0 * x * x) * 1.3;
        }
    }
    return light;
}

@vertex
fn vs_part(@builtin(vertex_index) vi: u32, @location(0) a: vec4<f32>, @location(1) b: vec4<f32>, @location(2) c: vec4<f32>) -> PartOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let at = a.xy;
    let shape = a.z;
    // The quad round the shape, px from the anchor.
    var lo = vec2<f32>(0.0);
    var hi = vec2<f32>(1.0);
    if shape < 0.5 {
        let tail = vec2<f32>(a.w, b.x);
        lo = min(vec2<f32>(0.0), tail) - 1.0;
        hi = max(vec2<f32>(0.0), tail) + 2.0;
    } else if shape < 1.5 {
        hi = vec2<f32>(a.w);
    } else if shape < 2.5 {
        lo = vec2<f32>(-a.w - 1.0, -a.w * 0.5 - 1.0);
        hi = vec2<f32>(a.w + 2.0, a.w * 0.5 + 2.0);
    } else {
        lo = vec2<f32>(-a.w);
        hi = vec2<f32>(a.w + 1.0);
    }
    let local = mix(lo, hi, corner);
    var o: PartOut;
    o.pos = canvas_clip(at + local);
    o.local = local;
    o.shape = vec4<f32>(shape, a.w, b.x, 0.0);
    let glow = b.w;
    let lit = c.rgb * light_at(at, b.z);
    o.col = vec4<f32>(mix(lit, c.rgb * g.misc.z * 1.4, glow), c.w);
    o.glow = glow;
    return o;
}

struct PartFrag {
    @location(0) colour: vec4<f32>,
    @location(1) bloom: vec4<f32>,
};

@fragment
fn fs_part(i: PartOut) -> PartFrag {
    // The px's centre, from the anchor.
    let p = floor(i.local) + 0.5;
    var cover = 0.0;
    let shape = i.shape.x;
    if shape < 0.5 {
        // A stroke from the anchor to its tail, one px wide, fading toward the tail.
        let tail = i.shape.yz;
        let len2 = max(dot(tail, tail), 0.0001);
        let t = clamp(dot(p - 0.5, tail) / len2, 0.0, 1.0);
        let d = length(p - 0.5 - tail * t);
        cover = select(0.0, 1.0 - t * 0.85, d < 0.72);
    } else if shape < 1.5 {
        cover = 1.0;
    } else if shape < 2.5 {
        let r = i.shape.y;
        let d = length(vec2<f32>(p.x - 0.5, (p.y - 0.5) * 2.0));
        cover = select(0.0, 1.0, abs(d - r) < 0.8);
    } else {
        let r = max(i.shape.y, 0.5);
        let d2 = dot(p - 0.5, p - 0.5) / (r * r);
        cover = clamp(1.0 - d2, 0.0, 1.0);
        cover *= cover;
    }
    let a = cover * i.col.w;
    if a <= 0.004 {
        discard;
    }
    var o: PartFrag;
    o.colour = vec4<f32>(i.col.rgb, a);
    o.bloom = vec4<f32>(i.col.rgb * i.glow * a * 0.6, 1.0);
    return o;
}
