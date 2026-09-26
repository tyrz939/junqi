// The light pass (PRESENTATION.md §1.7 T2), one fragment a canvas pixel: the sky's fill, the sun
// or the moon, and the point lights of this pixel's 32 x 32 tile, each by N dot L with the
// light's height as z, a windowed inverse-square falloff, and a soft shadow traced through the
// height field; emissive added unlit. A second target keeps what blooms.
//
// The shadow is a march from the lit point toward the light across the height field, the ray
// rising as the light is high. Where the field comes within `clearance` of the ray at `t` px
// from the point, the light is `k * clearance / t` visible (k is the light's distance over its
// size, or 1 / tan of the sun's spread): the penumbra widens with the distance from what casts
// it, as a real one does, and a thin post's shadow is sharp at its foot and soft at its tip.

@group(0) @binding(1) var galb: texture_2d<f32>;
@group(0) @binding(2) var gnh: texture_2d<f32>;
@group(0) @binding(3) var gem: texture_2d<f32>;
@group(0) @binding(4) var<storage, read> hmap: array<u32>;

struct Light {
    // x, y: its ground point (G-buffer px); z: its height; w: its radius.
    pos: vec4<f32>,
    // Linear colour; w: the glowing thing's size, px.
    col: vec4<f32>,
    // A spot's direction across the ground and the cosine of its half-cone (below -1 for a
    // point); w: 1 if it casts.
    spot: vec4<f32>,
};

@group(0) @binding(5) var<storage, read> lights: array<Light>;
// Per 32 x 32 tile: (first, count) into `tile_lights`.
@group(0) @binding(6) var<storage, read> tiles: array<vec2<u32>>;
@group(0) @binding(7) var<storage, read> tile_lights: array<u32>;

struct LitOut {
    @location(0) colour: vec4<f32>,
    @location(1) bloom: vec4<f32>,
};

fn texel_height(x: i32, y: i32) -> f32 {
    let w = i32(g.full.x);
    if x < 0 || y < 0 || x >= w || y >= i32(g.full.y) {
        return 0.0;
    }
    return f32(hmap[u32(y * w + x)]);
}

// The height field between texels, bilinear: an edge seen at a slant is a slope, not a stair,
// so a penumbra has no steps in it.
fn height_at(q: vec2<f32>) -> f32 {
    let p = q - 0.5;
    let b = floor(p);
    let f = p - b;
    let x = i32(b.x);
    let y = i32(b.y);
    let top = mix(texel_height(x, y), texel_height(x + 1, y), f.x);
    let bottom = mix(texel_height(x, y + 1), texel_height(x + 1, y + 1), f.x);
    return mix(top, bottom, f.y);
}

// The tallest of the four texels round `q`: what a long step samples, so a step of up to three
// px never walks through a thin post it should have hit.
fn height_max(q: vec2<f32>) -> f32 {
    let b = floor(q - 0.5);
    let x = i32(b.x);
    let y = i32(b.y);
    return max(max(texel_height(x, y), texel_height(x + 1, y)), max(texel_height(x, y + 1), texel_height(x + 1, y + 1)));
}

// How much of a light toward `l` (unit, x east, y south, z up) reaches `p`, marching at most
// `max_t` px across the ground from `t0`, with penumbra factor `k`.
fn trace(p: vec3<f32>, l: vec3<f32>, max_t: f32, k: f32, t0: f32, max_step: f32) -> f32 {
    let lxy = length(l.xy);
    if lxy < 0.0005 {
        return 1.0;
    }
    let dir = l.xy / lxy;
    let rise = l.z / lxy;
    var res = 1.0;
    var t = t0;
    var step = 1.0;
    for (var i = 0; i < 160; i++) {
        if t >= max_t {
            break;
        }
        let z = p.z + 0.75 + rise * t;
        if rise >= 0.0 && z > g.hmax {
            break;
        }
        let q = p.xy + dir * t;
        let hq = select(height_at(q), height_max(q), step > 1.25);
        res = min(res, k * (z - hq) / t);
        if res <= 0.0 {
            return 0.0;
        }
        step = clamp(t * 0.05, 1.0, max_step);
        t += step;
    }
    let r = clamp(res, 0.0, 1.0);
    return r * r * (3.0 - 2.0 * r);
}

// The ground's own occlusion by what stands round it: the foot of a wall, a crate's skirt.
fn ground_ao(p: vec3<f32>) -> f32 {
    var occ = 0.0;
    let dirs = array<vec2<f32>, 8>(
        vec2<f32>(1.0, 0.0), vec2<f32>(-1.0, 0.0), vec2<f32>(0.0, 1.0), vec2<f32>(0.0, -1.0),
        vec2<f32>(0.7, 0.7), vec2<f32>(-0.7, 0.7), vec2<f32>(0.7, -0.7), vec2<f32>(-0.7, -0.7),
    );
    for (var i = 0; i < 8; i++) {
        let r = select(4.0, 10.0, i >= 4);
        let hq = height_at(p.xy + dirs[i] * r);
        occ += clamp((hq - p.z) / (r * 2.0), 0.0, 1.0);
    }
    return 1.0 - occ * 0.09;
}

// A windowed inverse square: 1 at the light, 0 at its radius, never a hard rim.
fn falloff(x: f32) -> f32 {
    let w = clamp(1.0 - x * x, 0.0, 1.0);
    return w * w / (1.0 + 4.0 * x * x);
}

@fragment
fn fs_light(i: FullOut) -> LitOut {
    let px = vec2<i32>(floor(i.pos.xy));
    let q = px + vec2<i32>(i32(g.guard));
    let alb = textureLoad(galb, q, 0).rgb;
    let nh = textureLoad(gnh, q, 0);
    let em = textureLoad(gem, q, 0).rgb;
    let h = nh.b * 255.0;
    let depth = nh.a * 255.0;
    let nx = (nh.r * 255.0 - 128.0) / 127.0;
    let ny = (nh.g * 255.0 - 128.0) / 127.0;
    let n = vec3<f32>(nx, ny, sqrt(max(1.0 - nx * nx - ny * ny, 0.0)));
    // Where this pixel is: on the ground under it, `h` above. A standing thing's face is the
    // front of its body, half its depth toward the viewer from the line it stands on, so its
    // own body never shadows its face.
    let front = select(0.0, depth * 0.5 + 1.0, h > 0.5);
    let p = vec3<f32>(f32(q.x) + 0.5, f32(q.y) + 0.5 + h + front, h);
    let t0 = 1.0;

    var light = g.fill.rgb;
    if h < 3.0 {
        light *= ground_ao(p);
    }
    if g.sun_dir.w > 0.5 {
        let l = g.sun_dir.xyz;
        // The sun's colour is its light on flat ground: a face turned to a low sun catches up
        // to two and a half times that.
        let ndl = min(max(dot(n, l), 0.0) / max(l.z, 0.2), 2.5);
        if ndl > 0.0 {
            light += g.sun_col.rgb * ndl * trace(p, l, 2000.0, g.sun_col.w, t0, 3.0);
        }
    }
    let tile = vec2<u32>(px) / 32u;
    let tr = tiles[tile.y * g.tiles_x + tile.x];
    for (var k = 0u; k < tr.y; k++) {
        let lt = lights[tile_lights[tr.x + k]];
        let v = lt.pos.xyz - p;
        let dist = length(v);
        let r = lt.pos.w;
        if dist >= r || dist < 0.001 {
            continue;
        }
        let l = v / dist;
        // A little wrap, so a lamp's pool has no hard terminator on a round thing.
        let ndl = max((dot(n, l) + 0.2) / 1.2, 0.0);
        var att = falloff(dist / r) * ndl;
        if lt.spot.z > -1.5 {
            let across = normalize(-v.xy + vec2<f32>(0.0001, 0.0));
            att *= smoothstep(lt.spot.z, lt.spot.z + 0.12, dot(across, lt.spot.xy));
        }
        if att <= 0.0005 {
            continue;
        }
        var sh = 1.0;
        if lt.spot.w > 0.5 {
            let dxy = length(v.xy);
            sh = trace(p, l, dxy - (lt.col.w + 3.0), clamp(dxy / max(lt.col.w, 1.0), 2.0, 16.0), t0, 1.0);
        }
        light += lt.col.rgb * att * sh;
    }

    let lit = alb * light + em * g.misc.z;
    var o: LitOut;
    o.colour = vec4<f32>(lit, 1.0);
    // What blooms: the emissive, and a little of whatever is lit past white.
    o.bloom = vec4<f32>(em * g.misc.z + max(lit - vec3<f32>(1.1), vec3<f32>(0.0)) * 0.25, 1.0);
    return o;
}
