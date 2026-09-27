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
    gl_FragColor = vec4(srgb(l.r) * 0.5, srgb(l.g) * 0.5, srgb(l.b) * 0.5, 1.0);
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

/// Added into the light target: the colour by the falloff (`(1 - d2 / r2)^3` across the ground,
/// T0's pool), by N dot L relative to flat ground (a face turned to the lamp catches up to
/// twice what the ground does, a face turned away nothing; a little wrap so a round thing has
/// no hard terminator), by the spot's cone, and by the shadow mask: the mask holds how high the
/// shadow reaches over each ground point, and a px lower than that is dark.
pub const POINT_FS: &str = r"
uniform sampler2D u_mask_a;
uniform sampler2D u_mask_b;
varying vec4 v_l0;
varying vec4 v_l1;
varying vec4 v_l2;
void main() {
    vec2 cp = canvas_px();
    vec4 nh = texture2D(u_nh, cp / u_canvas);
    float h = byte(nh.b);
    // The ground's own relief (4 px and under) is the ground where it is drawn.
    float lifted = h > 4.5 ? 1.0 : 0.0;
    float front = lifted * (byte(nh.a) * 0.5 + 1.0);
    vec3 p = vec3(cp.x, cp.y + lifted * fdiv(h * 4.0 + 4.0, 5.0) + front, h);
    vec2 dxy = v_l0.xy - p.xy;
    float r2 = v_l0.w * v_l0.w;
    float d2 = dot(dxy, dxy);
    if (d2 >= r2) discard;
    float x = 1.0 - d2 / r2;
    float k = x * x * x;
    if (u_normals > 0.5) {
        vec3 n = normal_of(nh);
        vec3 l = normalize(vec3(dxy, v_l0.z - h) + vec3(0.0, 0.0, 0.0001));
        vec3 flat_l = normalize(vec3(dxy, v_l0.z) + vec3(0.0, 0.0, 0.0001));
        float lit = max((dot(n, l) + 0.4) / 1.4, 0.0);
        float ground = max((flat_l.z + 0.4) / 1.4, 0.3);
        k *= min(lit / ground, 2.0);
    }
    if (v_l2.z > -1.5) {
        vec2 across = normalize(-dxy + vec2(0.0001, 0.0));
        k *= smoothstep(v_l2.z, v_l2.z + 0.12, dot(across, v_l2.xy));
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
        if (h + 0.5 < z) k = 0.0;
    }
    gl_FragColor = vec4(v_l1.rgb * k * 0.5, 1.0);
}
";

/// The canvas: albedo by light (display values, so the multiply is the linear one), emissive added
/// unlit, then the tint (T1's grade: a multiply and a lift into the shadows).
pub const COMPOSE_FS: &str = r"
uniform sampler2D u_alb;
uniform sampler2D u_light;
uniform sampler2D u_emi;
uniform vec2 u_size;
uniform vec3 u_tint;
uniform vec3 u_lift;
uniform float u_egain;
void main() {
    vec2 uv = gl_FragCoord.xy / u_size;
    vec3 a = texture2D(u_alb, uv).rgb;
    vec3 l = texture2D(u_light, uv).rgb * 2.0;
    vec3 e = texture2D(u_emi, uv).rgb;
    vec3 c = clamp(a * l + e * u_egain, 0.0, 1.0) * u_tint;
    c += u_lift * (1.0 - c) * (1.0 - c);
    gl_FragColor = vec4(clamp(c, 0.0, 1.0), 1.0);
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
