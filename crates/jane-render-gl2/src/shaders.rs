//! The T1 shaders (PRESENTATION.md §1.7 T1), written once in the common subset of GLSL 1.20 and
//! GLSL ES 1.00: `attribute`, `varying`, `texture2D`, `gl_FragColor`, loops with constant bounds,
//! no bit operations, no integer textures, no const arrays. [`source`] puts the dialect's first
//! lines on (`#version 120`, or `#version 100` and a float precision).
//!
//! Every target stores rows canvas row 0 first: a vertex at canvas `(x, y)` goes to
//! `(x / w * 2 - 1, y / h * 2 - 1)`, so `gl_FragCoord.y - 0.5` is the canvas row and a read-back
//! needs no flip. Only the window's upscale turns it the right way up.
//!
//! The albedo pass is integer arithmetic done in floats: every value is a whole byte count well
//! under 2^24, `floor((a + 0.5) / b)` is `a / b` for whole `a` and `b` whatever the driver's
//! division rounds, and a byte read from an 8-bit texture is `floor(v * 255 + 0.5)`. So the exact
//! albedo mode writes the bytes `soft` writes (`jane-render-soft::blit`), bit for bit.

/// The first lines for a dialect: `es` is GLSL ES 1.00, else GLSL 1.20.
pub fn source(es: bool, fragment: bool, body: &str) -> String {
    let head = match (es, fragment) {
        (false, _) => "#version 120\n",
        (true, false) => "#version 100\nprecision highp float;\n",
        (true, true) => {
            "#version 100\n#ifdef GL_FRAGMENT_PRECISION_HIGH\nprecision highp float;\n#else\nprecision mediump float;\n#endif\n"
        }
    };
    let common = if fragment { FS_COMMON } else { "" };
    format!("{head}{common}{body}")
}

/// Helpers every fragment shader has.
const FS_COMMON: &str = r"
float byte(float v) { return floor(v * 255.0 + 0.5); }
vec3 bytes3(vec3 v) { return floor(v * 255.0 + 0.5); }
// a / b for whole a >= 0 and b > 0, whatever the division rounds.
float fdiv(float a, float b) { return floor((a + 0.5) / b); }
";

/// Attribute names by location, per program.
pub const RECT_ATTRS: [&str; 1] = ["a_pos"];
pub const CHUNK_ATTRS: [&str; 2] = ["a_pos", "a_uv"];
pub const SPRITE_ATTRS: [&str; 4] = ["a_pos", "a_uv", "a_rect", "a_info"];
pub const SPAN_ATTRS: [&str; 2] = ["a_pos", "a_val"];
pub const LIGHT_ATTRS: [&str; 4] = ["a_pos", "a_l0", "a_l1", "a_l2"];
pub const UI_ATTRS: [&str; 5] = ["a_pos", "a_loc", "a_src", "a_dst", "a_col"];

/// Floats a vertex per program, attribute by attribute.
pub const RECT_SIZES: [i32; 1] = [2];
pub const CHUNK_SIZES: [i32; 2] = [2, 2];
pub const SPRITE_SIZES: [i32; 4] = [2, 2, 4, 4];
pub const SPAN_SIZES: [i32; 2] = [2, 2];
pub const LIGHT_SIZES: [i32; 4] = [2, 4, 4, 4];
pub const UI_SIZES: [i32; 5] = [2, 2, 4, 4, 4];

/// A rect in the target's own px (`u_size`), for the full-target passes.
pub const RECT_VS: &str = r"
attribute vec2 a_pos;
uniform vec2 u_size;
void main() { gl_Position = vec4(a_pos / u_size * 2.0 - 1.0, 0.0, 1.0); }
";

/// Terrain chunks: a 256 px square of the chunk atlas, texel for px.
pub const CHUNK_VS: &str = r"
attribute vec2 a_pos;
attribute vec2 a_uv;
uniform vec2 u_canvas;
uniform vec2 u_tex;
varying vec2 v_uv;
void main() {
    v_uv = a_uv / u_tex;
    gl_Position = vec4(a_pos / u_canvas * 2.0 - 1.0, 0.0, 1.0);
}
";

pub const CHUNK_FS: &str = r"
uniform sampler2D u_src;
varying vec2 v_uv;
void main() { gl_FragColor = texture2D(u_src, v_uv); }
";

/// Sprites: `a_uv` is the page texel at the corner (a mirrored sprite's runs backwards), `a_rect`
/// the source rect (x0, y0, x1, y1, end exclusive), `a_info` the tint kind (0 none, 1 flash, 2
/// ghost), its weight of 256, the mirror flag and the depth across the ground in px.
pub const SPRITE_VS: &str = r"
attribute vec2 a_pos;
attribute vec2 a_uv;
attribute vec4 a_rect;
attribute vec4 a_info;
uniform vec2 u_canvas;
varying vec2 v_uv;
varying vec4 v_rect;
varying vec4 v_info;
void main() {
    v_uv = a_uv;
    v_rect = a_rect;
    v_info = a_info;
    gl_Position = vec4(a_pos / u_canvas * 2.0 - 1.0, 0.0, 1.0);
}
";

/// One fragment program, six modes (`u_mode`):
///
/// 0. albedo, exact: `soft`'s blit. Opaque texels their CLUT colour (flash toward white, ghost over
///    the snapshot of what is under it); clear and contact-shadow texels darken the snapshot by
///    their 3 x 3 cover (`jane_art::palette::ao`), or are skipped when the cover is 0.
/// 1. albedo, opaque texels only (the fast mode, which blends instead of reading the snapshot).
/// 2. the contact shadow as a multiply factor (fast mode, blended `dst * src`).
/// 3. a ghost at its weight (fast mode, blended over).
/// 4. normal and height: `(nx, ny, height, depth) / 255` (a mirrored normal's x flips).
/// 5. emissive: the CLUT colour of the emissive index, black where it is 0.
///
/// A page wider than the GL texture allows is stored as strips side by side (`u_page`: the page's
/// width, the strip height, the texture's size), so a texel is found by its strip.
pub const SPRITE_FS: &str = r"
uniform sampler2D u_alb;
uniform sampler2D u_pnh;
uniform sampler2D u_pem;
uniform sampler2D u_clut;
uniform sampler2D u_snap;
uniform vec4 u_page;
uniform vec2 u_canvas;
uniform vec3 u_ao;
uniform float u_mode;
varying vec2 v_uv;
varying vec4 v_rect;
varying vec4 v_info;

vec2 page_uv(vec2 t) {
    float s = floor((t.y + 0.5) / u_page.y);
    return (vec2(t.x + s * u_page.x, t.y - s * u_page.y) + 0.5) / u_page.zw;
}
float index_at(vec2 t) {
    vec4 v = texture2D(u_alb, page_uv(t));
    return byte(v.r) + 256.0 * byte(v.a);
}
vec3 clut(float i) { return bytes3(texture2D(u_clut, vec2((i + 0.5) / 1024.0, 0.5)).rgb); }
vec3 under() { return bytes3(texture2D(u_snap, gl_FragCoord.xy / u_canvas).rgb); }
float cover(vec2 t) {
    float c = 0.0;
    for (int dy = -1; dy <= 1; dy++) {
        for (int dx = -1; dx <= 1; dx++) {
            vec2 q = t + vec2(float(dx), float(dy));
            if (q.x >= v_rect.x && q.y >= v_rect.y && q.x < v_rect.z && q.y < v_rect.w) {
                if (abs(index_at(q) - 1.0) < 0.5) c += 1.0;
            }
        }
    }
    return c;
}
vec3 ao_factor(float k) {
    vec3 d = 256.0 - u_ao;
    return 256.0 - vec3(fdiv(d.r * k, 9.0), fdiv(d.g * k, 9.0), fdiv(d.b * k, 9.0));
}
void main() {
    vec2 t = clamp(floor(v_uv), v_rect.xy, v_rect.zw - 1.0);
    float ix = index_at(t);
    float kind = floor(v_info.x + 0.5);
    float w = floor(v_info.y + 0.5);
    float mode = floor(u_mode + 0.5);
    if (mode < 0.5) {
        if (ix > 1.5) {
            vec3 c = clut(ix);
            if (kind > 1.5) {
                c = floor((under() * (256.0 - w) + c * w) * (1.0 / 256.0));
            } else if (kind > 0.5) {
                c = floor((c * (256.0 - w) + 255.0 * w) * (1.0 / 256.0));
            }
            gl_FragColor = vec4(c / 255.0, 1.0);
        } else {
            float k = cover(t);
            if (k < 0.5) discard;
            gl_FragColor = vec4(floor(under() * ao_factor(k) * (1.0 / 256.0)) / 255.0, 1.0);
        }
    } else if (mode < 1.5) {
        if (ix < 1.5) discard;
        vec3 c = clut(ix);
        if (kind > 0.5) c = floor((c * (256.0 - w) + 255.0 * w) * (1.0 / 256.0));
        gl_FragColor = vec4(c / 255.0, 1.0);
    } else if (mode < 2.5) {
        if (ix > 1.5) discard;
        float k = cover(t);
        if (k < 0.5) discard;
        gl_FragColor = vec4(ao_factor(k) / 256.0, 1.0);
    } else if (mode < 3.5) {
        if (ix < 1.5) discard;
        gl_FragColor = vec4(clut(ix) / 255.0, w / 256.0);
    } else if (mode < 4.5) {
        if (ix < 1.5) discard;
        vec4 n = texture2D(u_pnh, page_uv(t));
        float nx = byte(n.r);
        if (v_info.z > 0.5) nx = min(256.0 - nx, 255.0);
        gl_FragColor = vec4(nx / 255.0, n.g, n.b, floor(v_info.w + 0.5) / 255.0);
    } else {
        if (ix < 1.5) discard;
        vec4 e = texture2D(u_pem, page_uv(t));
        float ei = byte(e.r) + 256.0 * byte(e.a);
        gl_FragColor = ei > 1.5 ? vec4(clut(ei) / 255.0, 1.0) : vec4(0.0, 0.0, 0.0, 1.0);
    }
}
";

/// Mask spans: two values per quad, each the largest kept (a silhouette's strength and its reach
/// in red and green; a point-light shadow's reach in both, its light's channel picked by the
/// colour mask).
pub const SPAN_VS: &str = r"
attribute vec2 a_pos;
attribute vec2 a_val;
uniform vec2 u_canvas;
varying vec2 v_val;
void main() {
    v_val = a_val;
    gl_Position = vec4(a_pos / u_canvas * 2.0 - 1.0, 0.0, 1.0);
}
";

pub const SPAN_FS: &str = r"
varying vec2 v_val;
void main() { gl_FragColor = vec4(v_val, v_val) / 255.0; }
";

/// The silhouettes' mask applied to the albedo (`jane-render-soft::silhouette::apply`, bit for
/// bit): a px takes the mask at the ground under it (the terrain's height from `u_height`, a px
/// `h` up standing `rows_up(h)` rows lower; 4 px and under is the ground) where the shadow there
/// reaches its height; a covered px toward `shade` by its strength, an edge px by five eighths of
/// it, the ring just outside by three eighths on the dither's odd squares. `u_box` is the mask's
/// box `(x0, y0, x1, y1)`: a px whose ground lies outside it takes nothing.
pub const SILHOUETTE_FS: &str = r"
uniform sampler2D u_mask;
uniform sampler2D u_snap;
uniform sampler2D u_height;
uniform vec2 u_size;
uniform vec3 u_k;
uniform vec4 u_box;
float need;
float m_at(vec2 p) {
    if (p.x < 0.0 || p.y < 0.0 || p.x >= u_size.x || p.y >= u_size.y) return 0.0;
    vec4 v = texture2D(u_mask, (p + 0.5) / u_size);
    return byte(v.g) >= need ? byte(v.r) : 0.0;
}
void main() {
    vec2 q = floor(gl_FragCoord.xy);
    float h = byte(texture2D(u_height, (q + 0.5) / u_size).b);
    need = h > 4.5 ? h : 0.0;
    vec2 p = vec2(q.x, h > 4.5 ? q.y + fdiv(h * 4.0 + 4.0, 5.0) : q.y);
    if (p.y > u_box.w || p.y < u_box.y - 1.0) discard;
    float m = m_at(p);
    float a = m_at(p + vec2(-1.0, 0.0));
    float b = m_at(p + vec2(1.0, 0.0));
    float c = m_at(p + vec2(0.0, -1.0));
    float d = m_at(p + vec2(0.0, 1.0));
    float s;
    if (m > 0.5) {
        s = min(min(a, b), min(c, d)) < 0.5 ? floor(m * 5.0 * 0.125) : m;
    } else {
        float most = max(max(a, b), max(c, d));
        if (most < 0.5 || mod(q.x + q.y, 2.0) > 0.5) discard;
        s = floor(most * 3.0 * 0.125);
    }
    s = s + floor(s * (1.0 / 128.0));
    vec3 v = bytes3(texture2D(u_snap, (q + 0.5) / u_size).rgb);
    vec3 f = 256.0 - floor(u_k * s * (1.0 / 256.0));
    gl_FragColor = vec4(floor(v * f * (1.0 / 256.0)) / 255.0, 1.0);
}
";

/// What the light passes share: where a light-target px is on the canvas and in the county, and
/// its normal. A px `h` up stands on the ground `rows_up(h)` rows lower; a standing thing's face
/// is the front of its body, half its depth toward the viewer, so its own shadow never covers its
/// lit face.
pub const LIGHT_COMMON: &str = r"
uniform sampler2D u_nh;
uniform vec2 u_canvas;
uniform float u_scale;
uniform float u_normals;
vec2 canvas_px() { return floor(gl_FragCoord.xy) * u_scale + 0.5 * u_scale; }
vec3 normal_of(vec4 nh) {
    if (u_normals < 0.5) return vec3(0.0, 0.0, 1.0);
    float nx = (byte(nh.r) - 128.0) / 127.0;
    float ny = (byte(nh.g) - 128.0) / 127.0;
    return vec3(nx, ny, sqrt(max(1.0 - nx * nx - ny * ny, 0.0)));
}
float srgb(float c) { return c <= 0.0031308 ? c * 12.92 : 1.055 * pow(c, 1.0 / 2.4) - 0.055; }
";

/// The sky: the fill, and the sun or moon by N dot L over its elevation's sine (its colour is its
/// light on flat ground, up to two and a half times that on a face turned to it), summed in
/// linear light and written as display values, halved (the target holds 0 to 2). The relief is
/// eased toward flat by `RELIEF` (a face turned away keeps two fifths of the sun): T1 has no
/// sky light from all round, and pixel art read at 0 goes to mud.
pub const AMBIENT_FS: &str = r"
uniform vec3 u_fill;
uniform vec4 u_sun;
uniform vec3 u_suncol;
void main() {
    vec2 cp = canvas_px();
    vec3 n = normal_of(texture2D(u_nh, cp / u_canvas));
    vec3 l = u_fill;
    if (u_sun.w > 0.5) {
        float ndl = min(max(dot(n, u_sun.xyz), 0.0) / max(u_sun.z, 0.2), 2.5);
        l += u_suncol * (1.0 + (ndl - 1.0) * 0.6);
    }
    // Alpha is the lamps' glint on what is wet (`POINT_FS`), none yet.
    gl_FragColor = vec4(srgb(l.r) * 0.5, srgb(l.g) * 0.5, srgb(l.b) * 0.5, 0.0);
}
";

/// A point light's quad, over its disc and the 72 px above it (what stands in the disc reaches
/// up the screen). `a_l0`: ground point x, y, height, radius; `a_l1`: colour (display values,
/// its gain in), unused; `a_l2`: a spot's direction and the cosine of its half cone (below -1:
/// a point), and its shadow mask slot (0 none, 1 to 4 the channels of mask A, 5 to 8 of B).
pub const LIGHT_VS: &str = r"
attribute vec2 a_pos;
attribute vec4 a_l0;
attribute vec4 a_l1;
attribute vec4 a_l2;
uniform vec2 u_canvas;
varying vec4 v_l0;
varying vec4 v_l1;
varying vec4 v_l2;
void main() {
    v_l0 = a_l0;
    v_l1 = a_l1;
    v_l2 = a_l2;
    gl_Position = vec4(a_pos / u_canvas * 2.0 - 1.0, 0.0, 1.0);
}
";

/// Added into the light target: T2's point light (`light.wgsl`), its colour in linear light
/// (`v_l1`, T2's gain and warm lean) by T2's windowed inverse square over the distance to the
/// light, by N dot L with a little wrap so a round thing has no hard terminator, by the spot's
/// cone, and by the shadow mask: the mask holds how high the shadow reaches over each ground
/// point, and a px lower than that is dark. The target holds display values, so what is added is
/// how much the lamp lifts the sky's light at this px once both are summed in linear light (the
/// sky as `AMBIENT_FS` lights the px): the pool is T2's in size and heart, and fades into the
/// night as T2's does, never a second exposure. Alpha is the lamp's glint on what is wet
/// (PRESENTATION.md §1.8, T2's Blinn term toward the 3/4 eye) in linear light, halved: the surface
/// byte in the emissive target's alpha says how the px takes rain, `u_wet` how wet it is.
pub const POINT_FS: &str = r"
uniform sampler2D u_mask_a;
uniform sampler2D u_mask_b;
uniform sampler2D u_emi;
uniform float u_wet;
uniform vec3 u_fill;
uniform vec4 u_sun;
uniform vec3 u_suncol;
varying vec4 v_l0;
varying vec4 v_l1;
varying vec4 v_l2;
vec3 srgb3(vec3 c) { return vec3(srgb(c.r), srgb(c.g), srgb(c.b)); }
void main() {
    vec2 cp = canvas_px();
    vec4 nh = texture2D(u_nh, cp / u_canvas);
    float h = byte(nh.b);
    // The ground's own relief (4 px and under) is the ground where it is drawn.
    float lifted = h > 4.5 ? 1.0 : 0.0;
    float front = lifted * (byte(nh.a) * 0.5 + 1.0);
    vec3 p = vec3(cp.x, cp.y + lifted * fdiv(h * 4.0 + 4.0, 5.0) + front, h);
    vec2 dxy = v_l0.xy - p.xy;
    vec3 v = vec3(dxy, v_l0.z - h);
    float dist = length(v);
    if (dist >= v_l0.w || dist < 0.001) discard;
    float x = dist / v_l0.w;
    float w = clamp(1.0 - x * x, 0.0, 1.0);
    // What the glint keeps of the falloff: the pool's, the cone's and the shadow's, not N dot L.
    float g = w * w / (1.0 + 4.0 * x * x);
    vec3 n = normal_of(nh);
    float k = g * max((dot(n, v / dist) + 0.2) / 1.2, 0.0);
    if (v_l2.z > -1.5) {
        vec2 across = normalize(-dxy + vec2(0.0001, 0.0));
        float cone = smoothstep(v_l2.z, v_l2.z + 0.12, dot(across, v_l2.xy));
        k *= cone;
        g *= cone;
    }
    float slot = floor(v_l2.w + 0.5);
    if (slot > 0.5) {
        // A standing thing (deeper than the terrain's 2) never takes its own shadow: its px looks
        // the mask up past its own footprint on the side toward the light, where its own shadow
        // (thrown away from the light) is not, and another's still is.
        float deep = byte(nh.a);
        vec2 q = p.xy;
        if (deep > 2.5) q += normalize(dxy + vec2(0.0001, 0.0)) * (deep + 1.5);
        vec2 uv = q / u_canvas;
        vec4 m = slot < 4.5 ? texture2D(u_mask_a, uv) : texture2D(u_mask_b, uv);
        float c = mod(slot - 1.0, 4.0);
        float z = byte(c < 0.5 ? m.r : (c < 1.5 ? m.g : (c < 2.5 ? m.b : m.a)));
        if (h + 0.5 < z) {
            k = 0.0;
            g = 0.0;
        }
    }
    float spec = 0.0;
    float s = byte(texture2D(u_emi, cp / u_canvas).a);
    if (s < 253.5 && g > 0.0) {
        float water = floor(s / 4.0 + 0.001);
        float kind = s - water * 4.0;
        float wet = water < 0.5 ? u_wet * (kind > 1.5 ? 1.0 : (kind > 0.5 ? 0.55 : 0.0)) : 0.0;
        float shine = max(wet * (kind > 1.5 ? 1.0 : 0.35), water > 0.5 ? 0.9 : 0.0);
        if (shine > 0.0) {
            vec3 hv = normalize(v / dist + vec3(0.0, 0.55, 0.835));
            float c = max(max(v_l1.r, v_l1.g), v_l1.b);
            spec = pow(max(dot(n, hv), 0.0), 140.0) * g * shine * 0.6 * c;
        }
    }
    // The sky's light at this px (`AMBIENT_FS`), and what the lamp adds to it.
    vec3 sky = u_fill;
    if (u_sun.w > 0.5) {
        float ndl = min(max(dot(n, u_sun.xyz), 0.0) / max(u_sun.z, 0.2), 2.5);
        sky += u_suncol * (1.0 + (ndl - 1.0) * 0.6);
    }
    vec3 lit = srgb3(sky + v_l1.rgb * k) - srgb3(sky);
    gl_FragColor = vec4(lit * 0.5, spec * 0.5);
}
";

/// T1's grade (the frame's `Post`; its bloom is the post pass's, `POST_FS`), for every program
/// that writes the canvas: the compose, and the fog and particles laid over it, each graded as it
/// is drawn. T2's `fs_grade` term for term in T2's order, in linear light (a display value
/// squared, near enough, and its root back): the afterglow across the frame at dusk and dawn (the
/// side toward the sun's bearing warmer, the far edge toward the horizon: `u_aglow` the air's glow
/// and its strength, `u_ahorizon` the horizon and the glow's canvas x, `u_gsize` the canvas), the
/// exposure, the soft shoulder, the saturation (`u_grade`), the tint and the coloured lift, so a
/// T1 frame is T2's hour and mood (decided 2026-09-27: the tiers are one look). Nothing is clamped
/// before the exposure: a lamp's heart past white goes through the shoulder as on T2.
pub const GRADE: &str = r"
uniform vec3 u_tint;
uniform vec3 u_lift;
uniform vec2 u_grade;
uniform vec4 u_aglow;
uniform vec4 u_ahorizon;
uniform vec2 u_gsize;
vec3 afterglow(vec3 l) {
    if (u_aglow.w <= 0.0) return l;
    vec2 p = floor(gl_FragCoord.xy);
    float toward = clamp(1.0 - abs(p.x - u_ahorizon.w) / (u_gsize.x * 1.2), 0.0, 1.0);
    float k = u_aglow.w * toward * toward;
    vec3 air = u_aglow.rgb;
    vec3 warm = air / max(max(air.r, air.g), max(air.b, 0.001));
    l = l * mix(vec3(1.0), 0.8 + warm * 0.45, k * 0.55) + air * k * 0.02;
    float far = clamp(1.0 - p.y / u_gsize.y, 0.0, 1.0);
    return mix(l, (u_ahorizon.rgb + air) * 0.25, far * far * u_aglow.w * 0.07);
}
vec3 grade_lin(vec3 l) {
    l = afterglow(max(l, 0.0)) * u_grade.x;
    vec3 over = max(l - 0.78, 0.0);
    l = min(l, 0.78) + 0.22 * (1.0 - exp(-over / 0.22));
    float y = dot(l, vec3(0.2126, 0.7152, 0.0722));
    l = max(mix(vec3(y), l, u_grade.y), 0.0) * u_tint;
    l += u_lift * (1.0 - l) * (1.0 - l);
    return sqrt(clamp(l, 0.0, 1.0));
}
vec3 grade(vec3 c) {
    c = max(c, 0.0);
    return grade_lin(c * c);
}
";

/// Display values to linear light and back, and the sky backdrop's colour (`jane_present::atmos::
/// sky_at`, T2's `sky_colour`): the zenith down to the horizon, most of the change low, the
/// afterglow low over its bearing. Colours in linear light.
pub const SKY_COMMON: &str = r"
uniform vec3 u_zenith;
uniform vec3 u_horizon;
uniform vec4 u_glow;
uniform float u_glow_x;
vec3 lin3(vec3 c) {
    return vec3(
        c.r <= 0.04045 ? c.r / 12.92 : pow((c.r + 0.055) / 1.055, 2.4),
        c.g <= 0.04045 ? c.g / 12.92 : pow((c.g + 0.055) / 1.055, 2.4),
        c.b <= 0.04045 ? c.b / 12.92 : pow((c.b + 0.055) / 1.055, 2.4));
}
float srgb1(float c) { return c <= 0.0031308 ? c * 12.92 : 1.055 * pow(c, 1.0 / 2.4) - 0.055; }
vec3 srgb3(vec3 c) { c = clamp(c, 0.0, 1.0); return vec3(srgb1(c.r), srgb1(c.g), srgb1(c.b)); }
vec3 sky_colour(float x, float up) {
    float t = clamp(up / 240.0, 0.0, 1.0);
    float k = 1.0 - (1.0 - t) * (1.0 - t);
    vec3 c = mix(u_horizon, u_zenith, k);
    float across = clamp(1.0 - abs(x - u_glow_x) / 520.0, 0.0, 1.0);
    float low = clamp(1.0 - up / 150.0, 0.0, 1.0);
    return mix(c, u_glow.rgb, across * low * u_glow.w * 0.75);
}
";

/// The sky backdrop's gradient into its target (the canvas's width, 256 rows, row `y` `y` px
/// over the horizon: the sky already mirrored as the water shows it).
pub const SKY_FS: &str = r"
void main() {
    vec2 p = floor(gl_FragCoord.xy);
    gl_FragColor = vec4(srgb3(sky_colour(p.x, p.y)), 1.0);
}
";

/// The far things on the backdrop (the School, the treeline): sprites standing on the horizon,
/// flipped as the backdrop is, lit by the sky alone from behind (a silhouette a little lighter than
/// its own dark), hazed toward the sky behind by their distance, their windows glowing (T2's
/// `fs_far`). `SPRITE_VS`'s layout.
pub const FAR_FS: &str = r"
uniform sampler2D u_alb;
uniform sampler2D u_pem;
uniform sampler2D u_clut;
uniform vec4 u_page;
uniform vec3 u_fill;
uniform float u_haze;
uniform float u_egain;
varying vec2 v_uv;
varying vec4 v_rect;
varying vec4 v_info;
vec2 page_uv(vec2 t) {
    float s = floor((t.y + 0.5) / u_page.y);
    return (vec2(t.x + s * u_page.x, t.y - s * u_page.y) + 0.5) / u_page.zw;
}
float index_of(vec4 v) { return byte(v.r) + 256.0 * byte(v.a); }
vec3 clut(float i) { return texture2D(u_clut, vec2((i + 0.5) / 1024.0, 0.5)).rgb; }
void main() {
    vec2 t = clamp(floor(v_uv), v_rect.xy, v_rect.zw - 1.0);
    float ix = index_of(texture2D(u_alb, page_uv(t)));
    if (ix < 1.5) discard;
    vec2 p = floor(gl_FragCoord.xy);
    vec3 behind = sky_colour(p.x, p.y);
    vec3 c = lin3(clut(ix)) * (u_fill * 0.7 + u_horizon * 0.35);
    c = mix(c, behind, u_haze);
    float e = index_of(texture2D(u_pem, page_uv(t)));
    if (e > 1.5) c += lin3(clut(e)) * u_egain * 1.2;
    gl_FragColor = vec4(srgb3(c), 1.0);
}
";

/// Particles, stars and the moon (PRESENTATION.md §2, §1.9): one quad a shape round its anchor.
/// `a_pos` the corner in the target's px, `a_loc` the corner's px from the anchor, `a_shape` the
/// shape (0 a stroke to its tail `yz`, 1 a square, 2 a ring of radius `y` squashed to half height on
/// the ground, 3 a soft disc of radius `y`, 4 the moon: a disc of radius `y` in phase `z` of 16),
/// `a_col` the colour (display values) and its opacity.
pub const SHAPE_ATTRS: [&str; 4] = ["a_pos", "a_loc", "a_shape", "a_col"];
pub const SHAPE_SIZES: [i32; 4] = [2, 2, 4, 4];

pub const SHAPE_VS: &str = r"
attribute vec2 a_pos;
attribute vec2 a_loc;
attribute vec4 a_shape;
attribute vec4 a_col;
uniform vec2 u_canvas;
varying vec2 v_loc;
varying vec4 v_shape;
varying vec4 v_col;
void main() {
    v_loc = a_loc;
    v_shape = a_shape;
    v_col = a_col;
    gl_Position = vec4(a_pos / u_canvas * 2.0 - 1.0, 0.0, 1.0);
}
";

/// A shape's cover of the px (T2's `fs_part`, and the moon), its colour lit by the light target
/// where the part lies under the light (`u_lit`: the rain and the sparks that do not glow, lit by
/// the lamps as the ground under them is), graded, blended over.
pub const SHAPE_FS: &str = r"
uniform sampler2D u_light;
uniform vec2 u_canvas;
uniform float u_lit;
uniform float u_bloom;
varying vec2 v_loc;
varying vec4 v_shape;
varying vec4 v_col;
void main() {
    vec2 p = floor(v_loc) + 0.5;
    vec2 o = p - 0.5;
    float shape = v_shape.x;
    float cover = 0.0;
    vec3 c = v_col.rgb;
    if (shape < 0.5) {
        vec2 tail = v_shape.yz;
        float len2 = max(dot(tail, tail), 0.0001);
        float t = clamp(dot(o, tail) / len2, 0.0, 1.0);
        float d = length(o - tail * t);
        cover = d < 0.72 ? 1.0 - t * 0.85 : 0.0;
    } else if (shape < 1.5) {
        cover = 1.0;
    } else if (shape < 2.5) {
        float d = length(vec2(o.x, o.y * 2.0));
        cover = abs(d - v_shape.y) < 0.8 ? 1.0 : 0.0;
    } else if (shape < 3.5) {
        float r = max(v_shape.y, 0.5);
        float d2 = dot(o, o) / (r * r);
        cover = clamp(1.0 - d2, 0.0, 1.0);
        cover *= cover;
    } else {
        // The moon: lit on the right while it waxes, on the left as it wanes; the dark of it is
        // the sky's.
        float r = max(v_shape.y, 1.0);
        vec2 q = o / r;
        float d = length(q);
        if (d > 1.0) discard;
        float f = v_shape.z / 16.0;
        float term = cos(6.2831853 * f) * sqrt(max(1.0 - q.y * q.y, 0.0));
        bool lit = f < 0.5 ? q.x > term : q.x < -term;
        if (!lit) discard;
        cover = 1.0;
        c *= 0.86 + 0.14 * (1.0 - d * d);
    }
    float a = cover * v_col.a;
    if (a <= 0.004) discard;
    // Lit where it lies as T2 lights a part, by the sky and the lamps, but for what glows of it,
    // which keeps its colour at T2's emissive gain (1.37 is the root of T2's 1.89).
    if (u_lit > 0.5) {
        vec3 l = texture2D(u_light, gl_FragCoord.xy * u_bloom / u_canvas).rgb * 2.0;
        c *= mix(l * 1.1, vec3(1.37), v_shape.w);
    }
    // Into the bloom's source (`u_bloom` > 1: the half-size target, MAX-blended): what glows of
    // it, as T2's particles write their glow beside their colour.
    if (u_bloom > 1.5) {
        if (v_shape.w <= 0.0) discard;
        gl_FragColor = enc(c * c * v_shape.w * a * 0.6);
        return;
    }
    gl_FragColor = vec4(grade(c), a);
}
";

/// Fog volumes (PRESENTATION.md §1.9, T2's `fs_fog`): per px the volumes' summed density (a ground
/// fog thinning to nothing by its top, so a thing taller shows its head over it), two drifting
/// layers of the mist tile, the far one on the ground only and the near one over everything,
/// larger and faster; its colour lit as T2 lights it (`u_air`: the sky's fill, the sun's share and
/// the afterglow) and by the lamps it stands in, taken from the light target above the sky's flat
/// light (`u_base`): a lamp in mist has a halo. Mixed with the canvas under it (`u_under`) in
/// linear light as T2 mixes it, the fog graded as it is drawn, into the other canvas target; a px
/// with no fog is copied.
pub const FOG_FS: &str = r"
uniform sampler2D u_nh;
uniform sampler2D u_light;
uniform sampler2D u_mist;
uniform sampler2D u_under;
uniform vec2 u_size;
uniform vec4 u_vrect[8];
uniform vec4 u_vcol[8];
uniform vec4 u_vshape[8];
uniform float u_n;
uniform vec4 u_move;
uniform vec3 u_base;
uniform vec3 u_air;
void main() {
    vec2 pf = floor(gl_FragCoord.xy) + 0.5;
    vec2 uv = pf / u_size;
    vec3 under = texture2D(u_under, uv).rgb;
    float h = byte(texture2D(u_nh, uv).b);
    float dens = 0.0;
    vec3 col = vec3(0.0);
    for (int k = 0; k < 8; k++) {
        if (float(k) >= u_n) break;
        vec4 r = u_vrect[k];
        float inside = min(min(pf.x - r.x, r.z - pf.x), min(pf.y - r.y, r.w - pf.y));
        if (inside <= 0.0) continue;
        float e = max(u_vshape[k].x, 1.0);
        float d = u_vcol[k].a * smoothstep(0.0, 1.0, clamp(inside / e, 0.0, 1.0));
        float top = u_vshape[k].y;
        if (top > 0.0) {
            float s = clamp(1.0 - h / top, 0.0, 1.0);
            d *= s * s;
        }
        dens += d;
        col += u_vcol[k].rgb * d;
    }
    if (dens < 0.002) {
        gl_FragColor = vec4(under, 1.0);
        return;
    }
    col /= dens;
    vec2 world = pf + u_move.xy;
    float far = texture2D(u_mist, (world - u_move.zw) / 256.0).r;
    float near = texture2D(u_mist, (world * 0.8 - u_move.zw * 1.6 + vec2(97.0, 41.0)) / 256.0).r;
    float on_ground = h < 3.0 ? 1.0 : 0.35;
    float a = clamp(dens * (0.14 + 0.62 * far * far * on_ground + 0.5 * near * near), 0.0, 0.9);
    vec3 l = texture2D(u_light, uv).rgb * 2.0;
    vec3 lamp = max(l * l - u_base, 0.0);
    vec3 fog = grade_lin(col * col * (u_air + lamp * 0.35));
    gl_FragColor = vec4(sqrt(mix(under * under, fog * fog, a)), 1.0);
}
";

/// The canvas: albedo by light, emissive added unlit, in linear light (a display value squared,
/// so the multiply by the light is the linear one) as T2's light pass sums them: `u_egain` is T2's
/// emissive gain; wet ground darker, its lamps' glints (the light target's alpha, linear and
/// halved, in its light's hue) and the sky's sheen on it; the water shimmering, refracting what
/// lies under it and mirroring what stands above it (`u_info.w`: the reflection's search, off on
/// tile GPUs) or the sky backdrop, puddles mirroring too, their rims dithered into the ground a
/// few px wide as a drift's edge is (ART.md §3.1); the sky above the zone's top edge; then T1's
/// grade. T2's water pass (`water.wgsl`) in the same terms.
///
/// `u_info`: the zone's top edge on the canvas, 1 if the sky backdrop is drawn, 1 if water is in
/// view, 1 to search for what stands above the water. `u_weather`: rain, wet, the wind (px a
/// tick), the tick. `u_cam`: the camera (the view's top-left in the zone, px).
pub const COMPOSE_FS: &str = r"
uniform sampler2D u_alb;
uniform sampler2D u_light;
uniform sampler2D u_emi;
uniform sampler2D u_nh;
uniform sampler2D u_sky;
uniform vec2 u_size;
uniform float u_egain;
uniform vec4 u_info;
uniform vec4 u_weather;
uniform vec2 u_cam;
uniform vec3 u_fill;
float surf(vec2 q) { return byte(texture2D(u_emi, (q + 0.5) / u_size).a); }
bool is_water(float s) { return s < 253.5 && s > 3.5; }
vec3 lit_at(vec2 q) {
    vec2 uv = (clamp(q, vec2(0.0), u_size - 1.0) + 0.5) / u_size;
    vec3 c = texture2D(u_alb, uv).rgb * texture2D(u_light, uv).rgb * 2.0;
    vec3 e = texture2D(u_emi, uv).rgb;
    return c * c + e * e * u_egain;
}
vec3 sky_px(float x, float up) {
    if (u_info.y < 0.5) return u_fill;
    vec2 t = vec2(clamp(x, 0.0, u_size.x - 1.0), clamp(up, 0.0, 255.0));
    return texture2D(u_sky, (t + 0.5) / vec2(u_size.x, 256.0)).rgb;
}
float hash(vec2 p) { return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.547); }
float vnoise(vec2 p) {
    vec2 b = floor(p);
    vec2 f = p - b;
    vec2 s = f * f * (3.0 - 2.0 * f);
    float a = hash(b);
    float c = hash(b + vec2(1.0, 0.0));
    float d = hash(b + vec2(0.0, 1.0));
    float e = hash(b + vec2(1.0, 1.0));
    return mix(mix(a, c, s.x), mix(d, e, s.x), s.y);
}
float bayer2(vec2 p) { return mod(2.0 * mod(p.x, 2.0) + 3.0 * mod(p.y, 2.0), 4.0); }
// The 4 x 4 ordered dither's threshold at px `p`, 0..1.
float bayer4(vec2 p) { return (4.0 * bayer2(p) + bayer2(floor(p / 2.0)) + 0.5) / 16.0; }
void main() {
    vec2 q = floor(gl_FragCoord.xy);
    vec2 uv = (q + 0.5) / u_size;
    vec4 em = texture2D(u_emi, uv);
    float s = byte(em.a);
    // Beyond the zone's top edge: the sky.
    if (u_info.y > 0.5 && abs(s - 254.0) < 0.5 && q.y < u_info.x) {
        gl_FragColor = vec4(grade(sky_px(q.x, u_info.x - q.y - 1.0)), 1.0);
        return;
    }
    vec4 lt = texture2D(u_light, uv);
    vec3 l = lt.rgb * 2.0;
    float water = s < 253.5 ? floor(s / 4.0 + 0.001) : 0.0;
    float kind = s < 253.5 && water < 0.5 ? s - water * 4.0 : 0.0;
    float wet = u_weather.y * (kind > 1.5 ? 1.0 : (kind > 0.5 ? 0.55 : 0.0));
    float shine = max(wet * (kind > 1.5 ? 1.0 : 0.35), water > 0.5 ? 0.9 : 0.0);
    vec3 a = texture2D(u_alb, uv).rgb * (1.0 - 0.4 * wet) * l;
    vec3 c = a * a + em.rgb * em.rgb * u_egain;
    // What shines back: each lamp's glint in its own light's hue, and a sheen of the sky.
    vec3 spec = l / max(max(l.r, l.g), max(l.b, 0.02)) * lt.a * 2.0 + u_fill * u_fill * shine * 0.07;
    float h = byte(texture2D(u_nh, uv).b);
    vec2 w = q + u_cam;
    float puddle = 0.0;
    if (kind > 0.5 && u_weather.y >= 0.35 && h < 1.5) {
        float n = vnoise(vec2(w.x, w.y * 1.6) / 22.0) * 0.7 + vnoise(w / 7.0) * 0.3;
        float edge = 0.72 - (u_weather.y - 0.35) * 0.3 + (kind < 1.5 ? 0.06 : 0.0);
        puddle = step(bayer4(w), smoothstep(edge - 0.045, edge + 0.025, n));
    }
    if (!(u_info.z > 0.5 && water > 0.5) && puddle < 0.5) {
        gl_FragColor = vec4(grade_lin(c + spec), 1.0);
        return;
    }
    // The ripple: two slow swells across the rows, quickened and roughened by the wind and the
    // rain, rounded to whole px so the reflection breaks into pixel-art bands.
    float t = u_weather.w;
    float wind = abs(u_weather.z) / 8.0 + u_weather.x * 1.5;
    float rip = sin(w.y * 0.55 + t * 0.045) * 0.9 + sin(w.y * 1.7 - t * 0.11 + w.x * 0.04) * (0.5 + wind * 0.6);
    rip += (hash(vec2(floor(w.x / 3.0), floor(w.y) + floor(t / 6.0))) - 0.5) * u_weather.x * 3.0;
    float dx = floor(rip + 0.5);
    // What stands above the water line, mirrored: a thing h tall standing at y shows at y + h,
    // so this px looks for a px 2h above it that is h tall and not water.
    vec3 refl = vec3(0.0);
    bool found = false;
    if (u_info.w > 0.5) {
        for (int k = 2; k <= 72; k++) {
            float hk = float(k);
            float y = q.y - 2.0 * hk;
            if (y < 0.0) break;
            vec2 qq = vec2(clamp(q.x + dx, 0.0, u_size.x - 1.0), y);
            float hh = byte(texture2D(u_nh, (qq + 0.5) / u_size).b);
            if (hh > 2.0 && abs(hh - hk) <= max(1.5, hk * 0.06) && !is_water(surf(qq))) {
                refl = lit_at(qq);
                found = true;
                break;
            }
        }
    }
    // Else the sky, laid down the screen with its horizon along the top edge, so the far things
    // on it hang in whatever water lies toward the top of the view.
    if (!found) {
        vec3 sp = sky_px(q.x + dx, q.y * 0.62 - 2.0);
        refl = sp * sp;
    }
    if (puddle > 0.5) {
        gl_FragColor = vec4(grade_lin(mix(c * 0.8, refl * 0.9, 0.5) + spec), 1.0);
        return;
    }
    // The water's own colour, refracted half as far as the reflection.
    vec3 under = lit_at(vec2(q.x - sign(dx) * floor(abs(dx) / 2.0), q.y));
    float deep = clamp(water / 12.0, 0.0, 1.0);
    float k = 0.55 + 0.3 * deep;
    vec3 tint = mix(vec3(0.85, 0.92, 0.95), vec3(0.62, 0.74, 0.82), deep);
    // The shimmer: a row here and there catches the sky brighter, another lies in a trough,
    // wandering with the swell and quickened by the wind.
    float crest = sin(w.y * 0.83 + t * 0.05 + sin(w.x * 0.045 + w.y * 0.13) * 2.4);
    float band = crest > 0.93 - wind * 0.05 ? 1.28 : (crest < -0.9 ? 0.86 : 1.0);
    vec3 wc = mix(under * (1.0 - 0.3 * deep), refl * tint * band, k);
    if (water < 1.5) wc += refl * 0.12;
    gl_FragColor = vec4(grade_lin(wc + spec), 1.0);
}
";

/// The bloom (T2's `post.wgsl`): what glows, a chain of halvings and a tent back up, added to the
/// frame in linear light before its grade. The chain's targets are RGBA8, so each holds its light
/// as `sqrt(v / 4)` (0 to 4 in linear light, fine in the dark where a glow fades out); every pass
/// decodes what it reads and encodes what it writes, and the tents are summed in the shader
/// rather than by blending.
pub const BLOOM_COMMON: &str = r"
vec3 dec(vec4 e) { return e.rgb * e.rgb * 4.0; }
vec4 enc(vec3 v) { return vec4(sqrt(clamp(v * 0.25, 0.0, 1.0)), 1.0); }
";

/// The bloom's source at half the canvas each way: T2's light pass's second target, per canvas px
/// the emissive at T2's gain, a quarter of whatever is lit past 1.1, and a third of the glints,
/// the four px of each 2 x 2 averaged (T2's first halving).
pub const BLOOM_SRC_FS: &str = r"
uniform sampler2D u_alb;
uniform sampler2D u_light;
uniform sampler2D u_emi;
uniform vec2 u_canvas;
uniform float u_egain;
vec3 src_at(vec2 q) {
    vec2 uv = (q + 0.5) / u_canvas;
    vec4 lt = texture2D(u_light, uv);
    vec3 l = lt.rgb * 2.0;
    vec3 a = texture2D(u_alb, uv).rgb * l;
    vec3 e = texture2D(u_emi, uv).rgb;
    vec3 em = e * e * u_egain;
    vec3 spec = l / max(max(l.r, l.g), max(l.b, 0.02)) * lt.a * 2.0;
    return em + max(a * a + em + spec - 1.1, 0.0) * 0.25 + spec * 0.35;
}
void main() {
    vec2 q = floor(gl_FragCoord.xy) * 2.0;
    vec3 s = src_at(q) + src_at(q + vec2(1.0, 0.0)) + src_at(q + vec2(0.0, 1.0)) + src_at(q + vec2(1.0, 1.0));
    gl_FragColor = enc(s * 0.25);
}
";

/// A 13-tap halving (T2's `fs_down`): the four 2 x 2 boxes round the centre and the centre's own,
/// weighted so no single bright texel sparkles. `u_texel` is the source's texel size, `u_size` the
/// target's.
pub const BLOOM_DOWN_FS: &str = r"
uniform sampler2D u_src;
uniform vec2 u_texel;
uniform vec2 u_size;
vec3 at(vec2 uv, float x, float y) { return dec(texture2D(u_src, uv + u_texel * vec2(x, y))); }
void main() {
    vec2 uv = (floor(gl_FragCoord.xy) + 0.5) / u_size;
    vec3 s = at(uv, 0.0, 0.0) * 0.125;
    s += (at(uv, -2.0, -2.0) + at(uv, 2.0, -2.0) + at(uv, -2.0, 2.0) + at(uv, 2.0, 2.0)) * 0.03125;
    s += (at(uv, 0.0, -2.0) + at(uv, -2.0, 0.0) + at(uv, 2.0, 0.0) + at(uv, 0.0, 2.0)) * 0.0625;
    s += (at(uv, -1.0, -1.0) + at(uv, 1.0, -1.0) + at(uv, -1.0, 1.0) + at(uv, 1.0, 1.0)) * 0.125;
    gl_FragColor = enc(s);
}
";

/// A 3 x 3 tent of the level below (`u_src`, `u_texel` its texel size) added to this level's own
/// halving (`u_base`, the target's size `u_size`): T2's `fs_up` and its additive blend.
pub const BLOOM_UP_FS: &str = r"
uniform sampler2D u_src;
uniform sampler2D u_base;
uniform vec2 u_texel;
uniform vec2 u_size;
vec3 at(vec2 uv, float x, float y) { return dec(texture2D(u_src, uv + u_texel * vec2(x, y))); }
void main() {
    vec2 uv = (floor(gl_FragCoord.xy) + 0.5) / u_size;
    vec3 s = at(uv, 0.0, 0.0) * 4.0;
    s += (at(uv, -1.0, 0.0) + at(uv, 1.0, 0.0) + at(uv, 0.0, -1.0) + at(uv, 0.0, 1.0)) * 2.0;
    s += at(uv, -1.0, -1.0) + at(uv, 1.0, -1.0) + at(uv, -1.0, 1.0) + at(uv, 1.0, 1.0);
    gl_FragColor = enc(dec(texture2D(u_base, uv)) + s / 16.0);
}
";

/// Light shafts (T2's in `fs_fog`), at a quarter of the canvas each way: over each px the air up
/// its column to 48 px, lit where the sun's ray past it reaches the ground, shown against shade.
/// Where the sun reaches is the silhouettes' mask (`u_mask`, the canvas's size: its red is how
/// dark the sun's shadow is over each ground px), the shade T1 has; `u_dir` is the sun's direction
/// across the ground and how far its ray runs per px it falls. Red holds the shaft, 0..1.
pub const RAYS_FS: &str = r"
uniform sampler2D u_mask;
uniform vec2 u_canvas;
uniform vec2 u_size;
uniform vec3 u_dir;
float sun_at(vec2 p) {
    vec2 c = clamp(floor(p), vec2(0.0), u_canvas - 1.0);
    return 1.0 - texture2D(u_mask, (c + 0.5) / u_canvas).r;
}
void main() {
    vec2 pf = (floor(gl_FragCoord.xy) + 0.5) * u_canvas / u_size;
    float seen = 0.0;
    for (int k = 1; k <= 12; k++) {
        float up = float(k) * 4.0;
        seen += sun_at(pf + vec2(0.0, up) - u_dir.xy * up * u_dir.z);
    }
    float shaft = clamp((seen / 12.0 - 0.35) * 1.6, 0.0, 1.0) * (1.0 - sun_at(pf));
    gl_FragColor = vec4(shaft, 0.0, 0.0, 1.0);
}
";

/// The last pass over the graded canvas (`u_src`) into the other canvas target: the bloom (the
/// chain's top, `u_bloom`, at `u_strength`) and the light shafts (`u_rays`, their light `u_ray`,
/// 0 with none) added in linear light as T2 adds them before its grade: the canvas taken back
/// through T2's shoulder, the light added at the grade's exposure and tint (`u_gain`), and the
/// shoulder again, so a lamp's heart blooms to white softly and the dark round it takes its glow.
pub const POST_FS: &str = r"
uniform sampler2D u_src;
uniform sampler2D u_bloom;
uniform sampler2D u_rays;
uniform vec2 u_size;
uniform float u_strength;
uniform vec3 u_gain;
uniform vec3 u_ray;
float unshoulder(float y) {
    if (y <= 0.78) return y;
    return 0.78 - 0.22 * log(max(1.0 - (y - 0.78) / 0.22, 0.0001));
}
float shoulder(float x) {
    if (x <= 0.78) return x;
    return 0.78 + 0.22 * (1.0 - exp(-(x - 0.78) / 0.22));
}
void main() {
    vec2 uv = (floor(gl_FragCoord.xy) + 0.5) / u_size;
    vec3 d = texture2D(u_src, uv).rgb;
    vec3 add = dec(texture2D(u_bloom, uv)) * u_strength + u_ray * texture2D(u_rays, uv).r;
    vec3 l = d * d;
    l = vec3(unshoulder(l.r), unshoulder(l.g), unshoulder(l.b)) + add * u_gain;
    gl_FragColor = vec4(sqrt(clamp(vec3(shoulder(l.r), shoulder(l.g), shoulder(l.b)), 0.0, 1.0)), 1.0);
}
";

/// The `Ui` pass (`jane_present::ui::cmd`): fills, atlas sprites and run-time images as quads over
/// the canvas, unlit, straight alpha, sampled nearest by integer division as `soft` does.
/// `a_loc` is the px inside the destination rect, `a_src` the source rect, `a_dst` the
/// destination's width and height, the kind (0 fill, 1 sprite, 2 image) and the mirror flag,
/// `a_col` a fill's colour, or a sprite's ink index and coverage.
pub const UI_VS: &str = r"
attribute vec2 a_pos;
attribute vec2 a_loc;
attribute vec4 a_src;
attribute vec4 a_dst;
attribute vec4 a_col;
uniform vec2 u_canvas;
varying vec2 v_loc;
varying vec4 v_src;
varying vec4 v_dst;
varying vec4 v_col;
void main() {
    v_loc = a_loc;
    v_src = a_src;
    v_dst = a_dst;
    v_col = a_col;
    gl_Position = vec4(a_pos / u_canvas * 2.0 - 1.0, 0.0, 1.0);
}
";

pub const UI_FS: &str = r"
uniform sampler2D u_alb;
uniform sampler2D u_clut;
uniform sampler2D u_img;
uniform vec4 u_page;
uniform vec2 u_img_size;
varying vec2 v_loc;
varying vec4 v_src;
varying vec4 v_dst;
varying vec4 v_col;
vec2 page_uv(vec2 t) {
    float s = floor((t.y + 0.5) / u_page.y);
    return (vec2(t.x + s * u_page.x, t.y - s * u_page.y) + 0.5) / u_page.zw;
}
vec2 texel() {
    vec2 d = floor(v_loc);
    if (v_dst.w > 0.5) d.x = v_dst.x - 1.0 - d.x;
    return v_src.xy + vec2(fdiv(d.x * v_src.z, max(v_dst.x, 1.0)), fdiv(d.y * v_src.w, max(v_dst.y, 1.0)));
}
void main() {
    float kind = floor(v_dst.z + 0.5);
    if (kind < 0.5) {
        gl_FragColor = v_col;
    } else if (kind < 1.5) {
        vec4 v = texture2D(u_alb, page_uv(texel()));
        float ix = byte(v.r) + 256.0 * byte(v.a);
        if (ix < 0.5) discard;
        float alpha = v_col.y;
        if (ix < 1.5) {
            gl_FragColor = vec4(0.0, 0.0, 0.0, 0.25 * alpha);
        } else {
            float at = v_col.x > 0.5 ? v_col.x : ix;
            gl_FragColor = vec4(texture2D(u_clut, vec2((at + 0.5) / 1024.0, 0.5)).rgb, alpha);
        }
    } else {
        vec4 c = texture2D(u_img, (texel() + 0.5) / u_img_size);
        if (c.a <= 0.0) discard;
        gl_FragColor = vec4(c.rgb, c.a * v_col.y);
    }
}
";

/// The window: sharp bilinear. Nearest inside a canvas texel, bilinear across the last output
/// px at its edge, so every px is square and no edge swims at 2.5x. `u_scale` is output px per
/// canvas px; the window's rows run top down, the canvas's bottom up.
pub const UPSCALE_FS: &str = r"
uniform sampler2D u_src;
uniform vec2 u_src_size;
uniform float u_scale;
uniform float u_win_h;
uniform float u_sharp;
void main() {
    vec2 px = vec2(gl_FragCoord.x, u_win_h - gl_FragCoord.y);
    vec2 texel = px / u_scale;
    vec2 base = floor(texel);
    vec2 f = texel - base - 0.5;
    float region = u_sharp > 0.5 ? 0.5 - 0.5 / u_scale : 0.5;
    vec2 off = (f - clamp(f, vec2(-region), vec2(region))) * u_scale + 0.5;
    gl_FragColor = texture2D(u_src, (base + off) / u_src_size);
}
";
