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
// A trace never meets the thing it starts on (a sprite does not shadow itself: its own field is
// a thin wall at its feet that a low sun would otherwise draw across its own body) nor the thing
// holding its light (a lamp's post, a torch's bracket, her lantern's hand).

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
    // point); w: 0 if it casts nothing, else the px round it that throw no shadow on it.
    spot: vec4<f32>,
};

@group(0) @binding(5) var<storage, read> lights: array<Light>;
// Per 32 x 32 tile: (first, count) into `tile_lights`.
@group(0) @binding(6) var<storage, read> tiles: array<vec2<u32>>;
@group(0) @binding(7) var<storage, read> tile_lights: array<u32>;
@group(0) @binding(8) var gid: texture_2d<u32>;
// What spills (the fence rule, `common.wgsl`): how much of the sun the fences' and hedges' bands
// take off the ground at each canvas px (`Prep::spill`).
@group(0) @binding(9) var spill: texture_2d<f32>;
// Each tile's tallest (`scatter.wgsl`'s `tops`).
@group(0) @binding(10) var<storage, read> tile_tops: array<u32>;

// The tallest top in the tile under `q`, grown by `TOP_GROW` (off the field: nothing).
fn tile_top(q: vec2<f32>) -> f32 {
    let x = i32(floor(q.x));
    let y = i32(floor(q.y));
    if x < 0 || y < 0 || x >= i32(g.full.x) || y >= i32(g.full.y) {
        return 0.0;
    }
    return f32(tile_tops[(y / TOP_TILE) * top_tiles_x() + x / TOP_TILE]);
}

// How far along `dir` from `q` the ray leaves the tile it is in, px.
fn tile_exit(q: vec2<f32>, dir: vec2<f32>) -> f32 {
    let t = f32(TOP_TILE);
    let b = floor(q / t) * t;
    let ex = select(select(1e9, (b.x - q.x) / dir.x, dir.x < -1e-5), (b.x + t - q.x) / dir.x, dir.x > 1e-5);
    let ey = select(select(1e9, (b.y - q.y) / dir.y, dir.y < -1e-5), (b.y + t - q.y) / dir.y, dir.y > 1e-5);
    return max(min(ex, ey), 0.0);
}

// Whose field a trace passes through as if it were not there: the px's own thing and the light's
// holder (0: nothing; the terrain is 0 and is never skipped).
var<private> skip_own: u32;
var<private> skip_holder: u32;
// Whether a trace passes what spills (the fence rule, `common.wgsl`'s `SPILL_ID`) as if it were
// not there: the sun's trace does (its shadow is `spill`); a lamp and the ground's occlusion see
// the whole field.
var<private> skip_spill: bool;

struct LitOut {
    @location(0) colour: vec4<f32>,
    @location(1) bloom: vec4<f32>,
    @location(2) sun: vec4<f32>,
};

// A texel's bottom where nothing stands: higher than anything.
const OPEN: f32 = 4096.0;

// A texel of the field: `(lo, hi)`, what stands there from `lo` px up to `hi` (lo 0: from the
// ground; hi 0: nothing). What floats (a canopy, a lamp's head, a hand) a ray passes under.
fn texel(x: i32, y: i32) -> vec2<f32> {
    let w = i32(g.full.x);
    if x < 0 || y < 0 || x >= w || y >= i32(g.full.y) {
        return vec2<f32>(OPEN, 0.0);
    }
    let i = u32(y * w + x);
    let v = hmap[i];
    let who = v & 0xffffu;
    if v == 0u || (who != 0u && (who == skip_own || who == skip_holder)) {
        return vec2<f32>(OPEN, 0.0);
    }
    if skip_spill && who == SPILL_ID || who == FENCE_ID {
        return vec2<f32>(OPEN, 0.0);
    }
    let lo = hmap[u32(w * i32(g.full.y)) + i];
    return vec2<f32>(select(256.0 - f32(lo), 0.0, lo == 0u), f32(v >> 16u));
}

// How far a ray at height `z` passes clear of a fence's bars at the texel under `q` (the field's
// third word, `scatter.wgsl`'s `fences`): over, under or between them; open where no fence
// stands; under 0 inside a bar.
fn fence_clear(q: vec2<f32>, z: f32) -> f32 {
    let x = i32(floor(q.x));
    let y = i32(floor(q.y));
    let w = i32(g.full.x);
    if x < 0 || y < 0 || x >= w || y >= i32(g.full.y) {
        return OPEN;
    }
    let i = u32(y * w + x);
    if (hmap[i] & 0xffffu) != FENCE_ID {
        return OPEN;
    }
    let m = hmap[2u * u32(w * i32(g.full.y)) + i];
    if m == 0u {
        return OPEN;
    }
    let top = f32(firstLeadingBit(m) + 1u);
    if z >= top {
        return z - top;
    }
    let zi = u32(max(z, 0.0));
    if ((m >> zi) & 1u) == 1u {
        return -1.0;
    }
    var c = OPEN;
    let below = m & ((1u << zi) - 1u);
    if below != 0u {
        c = min(c, z - f32(firstLeadingBit(below) + 1u));
    }
    let above = m >> (zi + 1u);
    if above != 0u {
        c = min(c, f32(zi + 1u + firstTrailingBit(above)) - z);
    }
    return c;
}

// Whether a lamp's trace sees the fences' bars (`fence_clear`): from the ground alone, as T0 and
// T1 lay a fence's lamp shadow (reach 1), so a fence never shadows itself.
var<private> see_fences: bool;

// The field between texels: its top bilinear, so an edge seen at a slant is a slope, not a
// stair, and a penumbra has no steps in it; its bottom the lowest of the four that stand.
fn height_at(q: vec2<f32>) -> vec2<f32> {
    let p = q - 0.5;
    let b = floor(p);
    let f = p - b;
    let x = i32(b.x);
    let y = i32(b.y);
    let a = texel(x, y);
    let c = texel(x + 1, y);
    let d = texel(x, y + 1);
    let e = texel(x + 1, y + 1);
    let top = mix(mix(a.y, c.y, f.x), mix(d.y, e.y, f.x), f.y);
    return vec2<f32>(min(min(a.x, c.x), min(d.x, e.x)), top);
}

// The tallest of the four texels round `q`, and the lowest bottom: what a long step samples, so
// a step of up to three px never walks through a thin post it should have hit.
fn height_max(q: vec2<f32>) -> vec2<f32> {
    let b = floor(q - 0.5);
    let x = i32(b.x);
    let y = i32(b.y);
    let a = texel(x, y);
    let c = texel(x + 1, y);
    let d = texel(x, y + 1);
    let e = texel(x + 1, y + 1);
    return vec2<f32>(min(min(a.x, c.x), min(d.x, e.x)), max(max(a.y, c.y), max(d.y, e.y)));
}

// How much of a light toward `l` (unit, x east, y south, z up) reaches `p`, marching at most
// `max_t` px across the ground from `t0`, with penumbra factor `k`. The ray's clearance at a
// texel is how far it passes over the top or under the bottom.
// A fence's bars (`see_fences`) are seen on up to `fence_t`, the light's own ground point: the
// field stops short of the glowing thing's size, but her lantern right against a fence is still
// on the other side of its rails.
fn trace(p: vec3<f32>, l: vec3<f32>, max_t: f32, fence_t: f32, k: f32, t0: f32, max_step: f32) -> f32 {
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
        let fences = see_fences && t < fence_t;
        if t >= max_t && !fences {
            break;
        }
        let z = p.z + 0.75 + rise * t;
        if rise >= 0.0 && z > g.hmax {
            break;
        }
        let q = p.xy + dir * t;
        var clear = OPEN;
        if t < max_t {
            // A branch, not a `select`: a select reads both.
            var f: vec2<f32>;
            if step > 1.25 {
                f = height_max(q);
            } else {
                f = height_at(q);
            }
            clear = max(z - f.y, f.x - z);
        }
        if fences {
            clear = min(clear, fence_clear(q, z));
        }
        res = min(res, k * clear / t);
        if res <= 0.0 {
            return 0.0;
        }
        step = clamp(t * 0.05, 1.0, max_step);
        t += step;
    }
    let r = clamp(res, 0.0, 1.0);
    return r * r * (3.0 - 2.0 * r);
}

// How much of the sun what spills takes off the canvas px `px` (the fence rule, `common.wgsl`):
// its mask under a tent of 3 x 3 taps `g.zone.w` px apart, 1 : 2 : 1 each way, so its edge is
// about as soft as T0's and T1's feathered one.
fn spill_at(px: vec2<i32>) -> f32 {
    let s = i32(g.zone.w);
    let hi = vec2<i32>(i32(g.canvas.x) - 1, i32(g.canvas.y) - 1);
    var sum = 0.0;
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let q = clamp(px + vec2<i32>(i, j) * s, vec2<i32>(0), hi);
            sum += textureLoad(spill, q, 0).r * f32((2 - abs(i)) * (2 - abs(j)));
        }
    }
    return sum / 16.0;
}

// How far across a side ray of `sun_disc` lies at the most, px.
const SIDE_MAX: f32 = 1.5;

// A texel's `(lo, hi)` at the texel under `q`, unfiltered: what a side ray of `sun_disc` reads.
fn texel_at(q: vec2<f32>) -> vec2<f32> {
    let b = floor(q);
    return texel(i32(b.x), i32(b.y));
}

// How much of the sun's disc reaches `p`: a ray toward its middle and one toward each side across
// it, weighted 1 : 2 : 1, marched together. A ray's penumbra is how far it clears the field's top
// or bottom, so on its own it softens a shadow's tip (the ray over a post's top) and not its sides
// (a ray beside a post clears it by its whole height): the side rays, a third of the sun's radius
// either side (`k` is 1 / tan of it), soften the sides too, widening with the distance from what
// casts, and a thin post's shadow keeps its line near its root and fades far out, as a real one
// does. Steps of 2 px at most, the middle ray's long step reading the tallest of 2 x 2 texels, so
// it never walks past a post 2 px wide; the side rays read the texel under them.
fn sun_disc(p: vec3<f32>, l: vec3<f32>, k: f32, t0: f32) -> f32 {
    let lxy = length(l.xy);
    if lxy < 0.0005 {
        return 1.0;
    }
    let dir = l.xy / lxy;
    let rise = l.z / lxy;
    // Across the ray, and how far the side rays lie from it a px along.
    let across = vec2<f32>(-dir.y, dir.x);
    let side = 0.35 / k;
    var res = vec3<f32>(1.0);
    var t = t0;
    var step = 1.0;
    // Enough steps to cross the longest shadow (`shadow::CAST_MARGIN_MAX`, 416 px, at 2 px a step).
    for (var i = 0; i < 224; i++) {
        let z = p.z + 0.75 + rise * t;
        if rise >= 0.0 && z > g.hmax {
            break;
        }
        let q = p.xy + dir * t;
        // Over the tallest of this tile by more than any penumbra could reach (the clearance
        // over its top is at least `z - top`, and the ray only rises): cross the tile at once.
        let top = tile_top(q);
        if rise >= 0.0 && k * (z - top) >= t {
            t += tile_exit(q, dir) + 0.5;
            step = clamp(t * 0.1, 1.0, 2.0);
            continue;
        }
        var f: vec2<f32>;
        if step > 1.25 {
            f = height_max(q);
        } else {
            f = height_at(q);
        }
        // A px and a half across at most: a crown's shadow a few rows thick keeps its umbra far
        // out, and a side is feathered about as wide as T0's and T1's edge.
        let off = across * min(side * t, SIDE_MAX);
        let a = texel_at(q + off);
        let b = texel_at(q - off);
        let clear = vec3<f32>(max(z - f.y, f.x - z), max(z - a.y, a.x - z), max(z - b.y, b.x - z));
        res = min(res, k * clear / t);
        if max(res.x, max(res.y, res.z)) <= 0.0 {
            return 0.0;
        }
        step = clamp(t * 0.1, 1.0, 2.0);
        t += step;
    }
    let r = clamp(res, vec3<f32>(0.0), vec3<f32>(1.0));
    let s = r * r * (3.0 - 2.0 * r);
    return (2.0 * s.x + s.y + s.z) * 0.25;
}

// The ground's own occlusion by what stands round it: the foot of a wall, a crate's skirt; not
// what floats well over it.
fn ground_ao(p: vec3<f32>) -> f32 {
    var occ = 0.0;
    let dirs = array<vec2<f32>, 8>(
        vec2<f32>(1.0, 0.0), vec2<f32>(-1.0, 0.0), vec2<f32>(0.0, 1.0), vec2<f32>(0.0, -1.0),
        vec2<f32>(0.7, 0.7), vec2<f32>(-0.7, 0.7), vec2<f32>(0.7, -0.7), vec2<f32>(-0.7, -0.7),
    );
    for (var i = 0; i < 8; i++) {
        let r = select(4.0, 10.0, i >= 4);
        let f = height_at(p.xy + dirs[i] * r);
        let hq = select(0.0, f.y, f.x <= p.z + r);
        occ += clamp((hq - p.z) / (r * 2.0), 0.0, 1.0);
    }
    return 1.0 - occ * 0.09;
}

// How near the one holding a light is lit from, of its radius (a hand's length).
const HELD_REACH: f32 = 0.3;
// How much of a lamp's light its umbra keeps (the pool's bounce), and the sharpest its penumbra
// gets (the light's distance over its size is held under this): soft, not a stencil's edge.
const LAMP_BOUNCE: f32 = 0.12;
const PEN_K: f32 = 10.0;

// A windowed inverse square: 1 at the light, 0 at its radius, never a hard rim.
fn falloff(x: f32) -> f32 {
    let w = clamp(1.0 - x * x, 0.0, 1.0);
    return w * w / (1.0 + 4.0 * x * x);
}

// Toward the eye: the 3/4 camera looks down and north, so the eye is south and above.
const EYE = vec3<f32>(0.0, 0.55, 0.835);

// The ground's surface byte (PRESENTATION.md §1.8, `ChunkLayers::surface`), from the albedo's
// alpha: 255 a thing standing, 254 beyond the zone, else `water * 4 + wet`.
fn surface(a: f32) -> u32 {
    return u32(round(a * 255.0));
}

@fragment
fn fs_light(i: FullOut) -> LitOut {
    let px = vec2<i32>(floor(i.pos.xy));
    let q = px + vec2<i32>(i32(g.guard));
    let a4 = textureLoad(galb, q, 0);
    let surf = surface(a4.a);
    let ground = surf < 254u;
    let water = select(0u, surf >> 2u, ground);
    let wet_kind = select(0u, surf & 3u, ground && water == 0u);
    // Wet ground is darker and richer, and shines where it is smooth (§1.8); water always does.
    let wet = g.weather.z * select(0.0, select(0.55, 1.0, wet_kind == 2u), wet_kind > 0u);
    let alb = a4.rgb * (1.0 - 0.4 * wet);
    // A puddle is a mirror for the lamps whatever the ground under it.
    let puddle = puddle_at(surf, vec2<f32>(px) + g.cam.xy, textureLoad(gnh, q, 0).b * 255.0);
    let shine = max(max(wet * select(0.35, 1.0, wet_kind == 2u), select(0.0, 0.9, water > 0u)), puddle * 1.2);
    let nh = textureLoad(gnh, q, 0);
    let em = textureLoad(gem, q, 0).rgb;
    let h = nh.b * 255.0;
    let nx = (nh.r * 255.0 - 128.0) / 127.0;
    let ny = (nh.g * 255.0 - 128.0) / 127.0;
    let n = vec3<f32>(nx, ny, sqrt(max(1.0 - nx * nx - ny * ny, 0.0)));
    // Where this pixel is: on the ground under it, `h` above. A standing thing's face is the
    // front of its body, just in front of the row it stands on (its depth is behind that row,
    // `scatter.wgsl`), so its own body never shadows its face. The ground's own relief (a tuft,
    // a cobble, 4 px and under) is the ground where it is drawn: moved down its few rows, every
    // shadow on the grass would stand that far up the screen from what casts it.
    let lifted = h > GROUND;
    let front = select(0.0, 2.0, lifted);
    let down = select(0.0, f32(rows_up(u32(h + 0.5))), lifted);
    let p = vec3<f32>(f32(q.x) + 0.5, f32(q.y) + 0.5 + down + front, h);
    skip_own = textureLoad(gid, q, 0).r;
    skip_holder = 0u;
    skip_spill = false;
    see_fences = !lifted;
    let t0 = 1.0;

    var light = g.fill.rgb;
    // The sun's shadow and its N dot L, for the debug view (`Wgpu::show_sun`, misc.w).
    var sun_seen = 1.0;
    var sun_ndl = 0.0;
    if !lifted {
        light *= ground_ao(p);
    }
    // What shines back: the lamps' glints and the sky's sheen on what is wet.
    var spec = g.fill.rgb * shine * 0.07;
    if g.sun_dir.w > 0.5 {
        let l = g.sun_dir.xyz;
        // The sun's colour is its light on flat ground: a face turned to a low sun catches up
        // to two and a half times that.
        let ndl = min(max(dot(n, l), 0.0) / max(l.z, 0.2), 2.5);
        if ndl > 0.0 {
            // The umbra takes `strength` of the sun (sun_dir.w - 1, `Directional::strength`): all
            // of it under a high clear sun, less when it is low and its light is scattered,
            // little under cloud.
            // A sun too faint to cast (`light::FAINTEST`) comes with no strength: no trace.
            if g.sun_dir.w > 1.001 {
                skip_spill = true;
                var seen = sun_disc(p, l, g.sun_col.w, t0);
                skip_spill = false;
                // The fence rule (`common.wgsl`): what spills lays the bands T0 and T1 lay, on
                // the ground alone (a fence never shadows its own rails, a hedge its own face).
                // `sun_seen` is the darker of the two, as T0 and T1 keep the strongest band.
                if !lifted {
                    seen = min(seen, 1.0 - spill_at(px));
                }
                sun_seen = 1.0 - (g.sun_dir.w - 1.0) * (1.0 - seen);
            }
            light += g.sun_col.rgb * ndl * sun_seen;
            if shine > 0.0 {
                let hv = normalize(l + EYE);
                spec += g.sun_col.rgb * pow(max(dot(n, hv), 0.0), 48.0) * shine * sun_seen * 2.0;
            }
        }
        sun_ndl = ndl;
    }
    // The night's turn: the sky's light (the fill and the sun or moon) by canvas row; the lamps
    // added below are the presenter's to gutter.
    let turn = band_at(f32(px.y) + 0.5);
    light *= turn;
    spec *= turn;
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
            skip_holder = u32(lt.spot.w + 0.5) - 1u;
            if skip_holder != 0u && skip_own == skip_holder {
                // The one holding a light (her lantern in her hand) is lit by it as from a hand's
                // length, never burnt white by it a px away, and never shadowed by it.
                att = falloff(max(dist, r * HELD_REACH) / r) * ndl;
            } else {
                let dxy = length(v.xy);
                sh = trace(p, l, dxy - (lt.col.w + 3.0), dxy - 0.5, clamp(dxy / max(lt.col.w, 1.0), 2.0, PEN_K), t0, 1.0);
                // A lamp's umbra keeps a little of its light: its pool bounces into its shadows.
                sh = LAMP_BOUNCE + (1.0 - LAMP_BOUNCE) * sh;
            }
        }
        light += lt.col.rgb * att * sh;
        if shine > 0.0 {
            // A wet road glints under a lamp: the lamp's reflection, long toward the eye.
            let hv = normalize(l + EYE);
            let s = pow(max(dot(n, hv), 0.0), 140.0) * falloff(dist / r);
            spec += lt.col.rgb * s * sh * shine * 0.6;
        }
    }

    // Lightning lights everything from the whole sky at once, for its two ticks.
    light += vec3<f32>(0.55, 0.6, 0.8) * g.weather.w * g.weather.w;
    let lit = alb * light + em * g.misc.z + spec;
    var o: LitOut;
    o.colour = vec4<f32>(lit, 1.0);
    if g.misc.w > 0.5 {
        // Red: how much of the sun reaches the px; green: its N dot L; blue: the albedo.
        o.colour = vec4<f32>(sun_seen, sun_ndl * 0.4, dot(alb, vec3<f32>(0.3, 0.5, 0.2)), 1.0);
    }
    // What blooms: the emissive, and a little of whatever is lit past white.
    o.bloom = vec4<f32>(em * g.misc.z + max(lit - vec3<f32>(1.1), vec3<f32>(0.0)) * 0.25 + spec * 0.35, 1.0);
    // Whether the sun reaches this px: what the light shafts are made of.
    o.sun = vec4<f32>(select(sun_seen, 0.0, g.sun_dir.w < 0.5 || sun_ndl <= 0.0), 0.0, 0.0, 1.0);
    return o;
}
