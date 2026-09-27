// Shared by every stage: the frame's globals. Coordinates are px of the G-buffer, which is the
// canvas with a guard band of `guard` px round it (so a caster just off screen still casts).
// A pixel at G-buffer (x, y) whose height is h px stands on the ground at world (x, y + h): the
// 3/4 view lifts a thing by its height straight up the screen.

struct Globals {
    // G-buffer size, px (the canvas plus the guard band on every side).
    full: vec2<f32>,
    // The canvas, px.
    canvas: vec2<f32>,
    guard: f32,
    // The tallest thing in the height field, px: a shadow ray above it is free.
    hmax: f32,
    n_lights: u32,
    tiles_x: u32,
    // The sky's own light, linear.
    fill: vec4<f32>,
    // Toward the sun (x east, y south, z up), unit; w is 0 with no sun or moon, else 1 and its
    // shadow's strength over that (1..2: how much of it an umbra takes away).
    sun_dir: vec4<f32>,
    // The sun's light on flat ground, linear; w is its penumbra factor, 1 / tan(its spread).
    sun_col: vec4<f32>,
    // Grade: tint (w: saturation, 1 is as lit).
    tint: vec4<f32>,
    // Grade: lift (w: exposure).
    lift: vec4<f32>,
    // x: bloom strength, y: ticks, z: emissive gain, w: 1 to draw the sun's term alone (debug).
    misc: vec4<f32>,
    // The weather (PRESENTATION.md §1.9): rain, mist, how wet the ground is, a lightning flash,
    // each 0..1.
    weather: vec4<f32>,
    // x: the wind, px a tick; y: the presentation tick; zw: the mist tile's drift, px.
    wind: vec4<f32>,
    // The sky backdrop: the zenith (w: how many stars show), the horizon (w: the afterglow's
    // canvas x), the afterglow (w: its strength), all linear.
    zenith: vec4<f32>,
    horizon: vec4<f32>,
    glow: vec4<f32>,
    // x: the zone's top edge on the canvas (the horizon), y: 1 if there is a sky, z: light shafts'
    // strength, w: fog volumes.
    skyinfo: vec4<f32>,
    // xy: the camera (the view's top-left in the zone, canvas px), z: 1 if water is in view,
    // w: particles.
    cam: vec4<f32>,
    // The zone's left, right and bottom edges on the canvas.
    zone: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.547);
}

// Smooth noise 0..1 on a unit lattice.
fn value_noise(p: vec2<f32>) -> f32 {
    let b = floor(p);
    let f = p - b;
    let s = f * f * (3.0 - 2.0 * f);
    let a = hash(b);
    let c = hash(b + vec2<f32>(1.0, 0.0));
    let d = hash(b + vec2<f32>(0.0, 1.0));
    let e = hash(b + vec2<f32>(1.0, 1.0));
    return mix(mix(a, c, s.x), mix(d, e, s.x), s.y);
}

// A puddle (PRESENTATION.md §1.8): 1 where ground that takes rain (the surface byte `s`: not
// water, its wet kind 1 or 2) has had a good deal of it, in low drifts of a noise anchored to the
// world px `w`, wider across than deep as the 3/4 view foreshortens them, and only on the ground
// (`h`, the px's height: never on a roof or a wall); else 0. The light pass
// makes a puddle shine and the water pass makes it a mirror.
fn puddle_at(s: u32, w: vec2<f32>, h: f32) -> f32 {
    let kind = select(0u, s & 3u, s < 254u && (s >> 2u) == 0u);
    if kind == 0u || g.weather.z < 0.35 || h > 1.5 {
        return 0.0;
    }
    let n = value_noise(vec2<f32>(w.x, w.y * 1.6) / 22.0) * 0.7 + value_noise(w / 7.0) * 0.3;
    let edge = 0.72 - (g.weather.z - 0.35) * 0.3 + select(0.06, 0.0, kind == 1u);
    return select(0.0, 1.0, n > edge);
}
// How many rows up the screen a thing `h` px tall is drawn: four fifths, rounded up
// (`jane_present::rows_up`, the 3/4 view's one projection).
fn rows_up(h: u32) -> u32 {
    return (h * 4u + 4u) / 5u;
}

// A px at or under this height is the ground's own relief (`jane_present::shadow::GROUND`): it
// neither stands in the height field nor is lifted off the row it is drawn on.
const GROUND: f32 = 4.5;

// A full-canvas triangle.
struct FullOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> FullOut {
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var o: FullOut;
    o.pos = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    o.uv = uv;
    return o;
}
