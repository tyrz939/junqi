// Water (PRESENTATION.md §1.8, T2), after the light pass: every water px reflects what stands
// above the water line by the height layer (a thing `h` px tall standing at `y` shows at
// `y + h`, so the water px at `y` looks for a px `2h` above it that is `h` tall), and where
// nothing stands it reflects the sky backdrop, the far shore its horizon: the School's silhouette
// hangs in the lake from its far bank. The reflection is rippled by the wind and the rain,
// darkened by depth, and laid over the water's own lit colour, which is refracted a px by the
// same ripple. Beyond the zone's top edge the sky itself shows. Everything else passes through.

@group(0) @binding(1) var lit: texture_2d<f32>;
@group(0) @binding(2) var gnh: texture_2d<f32>;
@group(0) @binding(3) var galb: texture_2d<f32>;
@group(0) @binding(4) var sky: texture_2d<f32>;

// The surface byte of G-buffer px `q`: 255 a thing, 254 beyond the zone, else `water * 4 + wet`.
fn surf_at(q: vec2<i32>) -> u32 {
    return u32(round(textureLoad(galb, q, 0).a * 255.0));
}

fn is_water(s: u32) -> bool {
    return s < 254u && (s >> 2u) > 0u;
}

fn sky_at(x: f32, up: f32) -> vec3<f32> {
    let p = vec2<i32>(i32(clamp(x, 0.0, g.canvas.x - 1.0)), i32(clamp(up, 0.0, 255.0)));
    return textureLoad(sky, p, 0).rgb;
}


@fragment
fn fs_water(i: FullOut) -> @location(0) vec4<f32> {
    let px = vec2<i32>(floor(i.pos.xy));
    let q = px + vec2<i32>(i32(g.guard));
    let here = textureLoad(lit, px, 0);
    let s = surf_at(q);
    // Beyond the zone: the sky above its top edge.
    if s == 254u && g.skyinfo.y > 0.5 && f32(px.y) < g.skyinfo.x {
        return vec4<f32>(sky_at(f32(px.x), g.skyinfo.x - f32(px.y) - 1.0), 1.0);
    }
    let t = g.wind.y;
    let wy = f32(px.y) + g.cam.y;
    let wx = f32(px.x) + g.cam.x;
    // A puddle: where the ground takes rain and has had a good deal of it, in low-lying drifts
    // of a noise anchored to the world, a mirror like the water's.
    let puddle = puddle_at(s, vec2<f32>(wx, wy), textureLoad(gnh, q, 0).b * 255.0);
    if g.cam.z < 0.5 && puddle == 0.0 || !is_water(s) && puddle == 0.0 {
        return here;
    }
    let depth = select(f32(s >> 2u), 0.0, puddle > 0.0);
    // The ripple: two slow swells across the rows, quickened and roughened by the wind and the
    // rain, rounded to whole px so the reflection breaks into pixel-art bands.
    let wind = abs(g.wind.x) / 8.0 + g.weather.x * 1.5;
    var rip = sin(wy * 0.55 + t * 0.045) * 0.9 + sin(wy * 1.7 - t * 0.11 + wx * 0.04) * (0.5 + wind * 0.6);
    rip += (hash(vec2<f32>(floor(wx / 3.0), floor(wy) + floor(t / 6.0))) - 0.5) * g.weather.x * 3.0;
    let dx = i32(round(rip));

    // What stands above the water line, mirrored.
    var refl = vec3<f32>(0.0);
    var found = false;
    for (var h = 2; h <= 72; h++) {
        let y = px.y - 2 * h;
        if y < -i32(g.guard) {
            break;
        }
        let qq = vec2<i32>(q.x + dx, q.y - 2 * h);
        let nh = textureLoad(gnh, qq, 0);
        let hh = nh.b * 255.0;
        if hh > 2.0 && abs(hh - f32(h)) <= max(1.5, f32(h) * 0.06) && !is_water(surf_at(qq)) {
            let sp = clamp(vec2<i32>(px.x + dx, y), vec2<i32>(0), vec2<i32>(g.canvas) - 1);
            refl = textureLoad(lit, sp, 0).rgb;
            found = true;
            break;
        }
    }
    // Else the sky. Seen from above every water px would mirror the same sky; the view instead
    // lays the backdrop down the screen, its horizon along the top edge, so the far things on it
    // (the School) hang in whatever water lies toward the top of the view, and the colour runs
    // from the horizon's to the zenith's as the eye comes nearer. One mapping for every px, so
    // no spit of land cuts a seam in it.
    if !found {
        // Held under the zenith's band of stars, which lay a line of specks along the bottom
        // rows; the water's own stars are the presenter's (`Atmosphere::water_night`).
        refl = sky_at(f32(px.x + dx), min(f32(px.y) * 0.62 - 2.0, 196.0));
    }
    if puddle > 0.0 {
        // A puddle is shallow and dark-bottomed: the ground under it, and over it a good share
        // of what it mirrors, the lamps above all.
        return vec4<f32>(mix(here.rgb * 0.8, refl * 0.9, 0.5), 1.0);
    }
    // The water's own colour, refracted a px by the ripple.
    let under = textureLoad(lit, clamp(vec2<i32>(px.x - dx / 2, px.y), vec2<i32>(0), vec2<i32>(g.canvas) - 1), 0).rgb;
    // Deep water is dark and holds the reflection best; the shallows show their bed.
    let deep = clamp(depth / 12.0, 0.0, 1.0);
    let k = 0.55 + 0.3 * deep;
    let tint = mix(vec3<f32>(0.85, 0.92, 0.95), vec3<f32>(0.52, 0.66, 0.72), deep);
    // The surface: a row here and there catches the sky brighter, another lies in a trough,
    // wandering with the swell and quickened by the wind; the pixel-art water line.
    let crest = sin(wy * 0.83 + t * 0.05 + sin(wx * 0.045 + wy * 0.13) * 2.4);
    let band = select(select(1.0, 0.86, crest < -0.9), 1.28, crest > 0.93 - wind * 0.05);
    var c = mix(under * (1.0 - 0.3 * deep), refl * tint * band, k);
    // A lip of light along the shore, brighter where the reflection is bright.
    if depth <= 1.0 {
        c += refl * 0.12;
    }
    return vec4<f32>(c, 1.0);
}
