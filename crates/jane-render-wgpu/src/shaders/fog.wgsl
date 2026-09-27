// Fog and light shafts (PRESENTATION.md §1.9, T2), after the water: the fog volumes in view,
// each a rect of the view with a soft edge, a density, a colour and a height, drawn as two
// drifting layers of the mist tile, the far one only on the ground (under what stands), the near
// one over everything and moving a little faster, as nearer air does. The fog is lit by the sky's
// light and the lamps it stands in, so a lamp in mist has a halo. A ground fog stands `top` px:
// a thing taller shows its head and shoulders over it. Then the light shafts: the air between
// the canopy and the ground lit where the low sun reaches it (the sun-seen target of the light
// pass, sampled up each column toward the sun), strongest in mist.

@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var gnh: texture_2d<f32>;
@group(0) @binding(3) var mist: texture_2d<f32>;
@group(0) @binding(4) var smp: sampler;

struct Volume {
    rect: vec4<f32>,
    // Linear colour; w: density 0..1.
    col: vec4<f32>,
    // x: the soft edge, px; y: a ground fog's top, px (0: full height).
    shape: vec4<f32>,
};

@group(0) @binding(5) var<storage, read> volumes: array<Volume>;

struct Light {
    pos: vec4<f32>,
    col: vec4<f32>,
    spot: vec4<f32>,
};

@group(0) @binding(6) var<storage, read> lights: array<Light>;
@group(0) @binding(7) var<storage, read> tiles: array<vec2<u32>>;
@group(0) @binding(8) var<storage, read> tile_lights: array<u32>;
@group(0) @binding(9) var sun_seen: texture_2d<f32>;

fn soft_falloff(x: f32) -> f32 {
    let w = clamp(1.0 - x * x, 0.0, 1.0);
    return w * w;
}

fn sun_at(p: vec2<i32>) -> f32 {
    let c = clamp(p, vec2<i32>(0), vec2<i32>(g.canvas) - 1);
    return textureLoad(sun_seen, c, 0).r;
}

@fragment
fn fs_fog(i: FullOut) -> @location(0) vec4<f32> {
    let px = vec2<i32>(floor(i.pos.xy));
    let q = px + vec2<i32>(i32(g.guard));
    var c = textureLoad(src, px, 0).rgb;
    let h = textureLoad(gnh, q, 0).b * 255.0;
    let pf = vec2<f32>(px) + 0.5;

    // The volumes over this px: a weighted colour and a summed density.
    var dens = 0.0;
    var ground_dens = 0.0;
    var col = vec3<f32>(0.0);
    let n = u32(g.skyinfo.w);
    for (var k = 0u; k < n; k++) {
        let v = volumes[k];
        let e = max(v.shape.x, 1.0);
        let inside = min(min(pf.x - v.rect.x, v.rect.z - pf.x), min(pf.y - v.rect.y, v.rect.w - pf.y));
        if inside <= 0.0 {
            continue;
        }
        let edge = smoothstep(0.0, 1.0, clamp(inside / e, 0.0, 1.0));
        var d = v.col.w * edge;
        if v.shape.y > 0.0 {
            // A ground fog: thick at the ground, gone by its top.
            let s = clamp(1.0 - h / v.shape.y, 0.0, 1.0);
            ground_dens += d;
            d *= s * s;
        }
        dens += d;
        col += v.col.rgb * d;
    }
    if dens > 0.002 {
        col /= dens;
        // Two layers of the mist tile: far (on the ground only, scaled 1) and near (over all,
        // larger and faster); world-anchored, so they slide as the camera moves.
        let world = pf + g.cam.xy;
        let far = textureSample(mist, smp, (world - g.wind.zw) / 256.0).r;
        let near = textureSample(mist, smp, (world * 0.8 - g.wind.zw * 1.6 + vec2<f32>(97.0, 41.0)) / 256.0).r;
        let on_ground = select(0.35, 1.0, h < 3.0);
        // Banks and clear lanes: the tile's shape squared, over a thin even haze.
        var a = dens * (0.14 + 0.62 * far * far * on_ground + 0.5 * near * near);
        a = clamp(a, 0.0, 0.9);
        // Lit by the sky and by the lamps it stands in: haloes.
        // The air takes the sky's light: the fill, the sun's share, and at dusk and dawn the
        // afterglow, so a fog at sunset is rose and a dawn mist is pale gold.
        var light = g.fill.rgb * 1.15 + g.sun_col.rgb * 0.45 + g.glow.rgb * g.glow.w * 0.45;
        let tile = vec2<u32>(px) / 32u;
        let tr = tiles[tile.y * g.tiles_x + tile.x];
        let p = vec3<f32>(f32(q.x) + 0.5, f32(q.y) + 0.5 + h, 18.0);
        for (var k = 0u; k < tr.y; k++) {
            let lt = lights[tile_lights[tr.x + k]];
            let v = lt.pos.xyz - p;
            let r = lt.pos.w * 0.7;
            let dist = length(v);
            if dist < r {
                light += lt.col.rgb * soft_falloff(dist / r) * 0.35;
            }
        }
        c = mix(c, col * light, a);
    }

    // Light shafts: the air over each px, from the ground up to the canopy, lit where the sun
    // reaches it. On screen, the air `k` px above a ground px stands `k` px up the column, and
    // it is lit if the ground the sun's ray would reach past it is.
    if g.skyinfo.z > 0.0 && g.sun_dir.w > 0.5 {
        let dir = normalize(g.sun_dir.xy + vec2<f32>(0.0001, 0.0));
        let run = 1.0 / max(g.sun_dir.z / length(g.sun_dir.xy), 0.12);
        var seen = 0.0;
        var n2 = 0.0;
        for (var k = 1; k <= 12; k++) {
            let up = f32(k) * 4.0;
            // The air `up` px high over the ground px `up` px below this one on screen; the sun's
            // ray through it lands `up * run` px further from the sun.
            let gp = pf + vec2<f32>(0.0, up) - dir * up * run;
            seen += sun_at(vec2<i32>(floor(gp)));
            n2 += 1.0;
        }
        let lit_air = seen / n2;
        // Only the contrast makes a shaft: fully lit or fully shaded air is just the air.
        let shaft = clamp((lit_air - 0.35) * 1.6, 0.0, 1.0) * (1.0 - sun_at(px) * 0.6);
        c += g.sun_col.rgb * shaft * g.skyinfo.z * (0.18 + g.weather.y * 0.4);
    }
    return vec4<f32>(c, 1.0);
}
