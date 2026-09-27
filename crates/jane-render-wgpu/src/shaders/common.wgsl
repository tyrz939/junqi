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
    // Toward the sun (x east, y south, z up), unit; w is 1 when there is a sun or moon.
    sun_dir: vec4<f32>,
    // The sun's light on flat ground, linear; w is its penumbra factor, 1 / tan(its spread).
    sun_col: vec4<f32>,
    // Grade: tint (w: saturation, 1 is as lit).
    tint: vec4<f32>,
    // Grade: lift (w: exposure).
    lift: vec4<f32>,
    // x: bloom strength, y: ticks, z: emissive gain, w: unused.
    misc: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

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
