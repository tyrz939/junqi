// The height field shadows are traced against: seen from above. Every G-buffer pixel with a
// height stands on the ground `rows_up(h)` rows below it (the 3/4 view draws a height of h px
// four fifths as tall, `jane_present::rows_up`); it raises that ground point, and the rows its
// depth covers round it, to its height (the tallest wins). An upright sprite's column lands on
// its feet, so a person becomes a thin wall as tall as she is, shaped like her, standing on the
// row she stands on; a wall's face lands on its foot and a roof on the house under it. Each
// texel keeps whose it is (the G-buffer's id) under its height, `h << 16 | id`, so a trace can
// skip the thing it starts on and the thing holding its light.
//
// Each texel has a bottom as well as a top (the field's second half, `256 - lo`, the lowest
// kept): what floats is blocked only between the two. A sprite's px is part of a run of its own
// px straight down the screen; where that run ends above the ground (a canopy over its trunk,
// a lamp's head beside its post, a hand, a bat), the run floats from its lowest px up, and a ray
// passes under it. A run that reaches the ground, and the terrain, stand from 0. Where the run
// goes behind a thing drawn in front of it, it is taken on down through that thing, and if it
// does not come out the other side it is taken to end as low as it could have.

@group(0) @binding(1) var gnh: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> hmap: array<atomic<u32>>;
@group(0) @binding(3) var gid: texture_2d<u32>;

// A run whose lowest px is this high or lower stands on the ground: the feet, a trunk's root.
const FLOAT: f32 = 6.5;
// How far down the screen a run is followed; past it, it is taken to stand on the ground.
const WALK: i32 = 192;

// The bottom of the run of `who`'s px that the px at `(x, y)`, `h` high, is part of: 0 if it
// reaches the ground.
fn run_bottom(x: i32, y: i32, h: f32, who: u32) -> f32 {
    let hh = i32(g.full.y);
    var low = h;
    // Rows walked since the last of its own px, through things drawn in front of it.
    var hidden = 0;
    for (var k = 1; k <= WALK; k++) {
        let yy = y + k;
        if yy >= hh {
            return 0.0;
        }
        let other = textureLoad(gid, vec2<i32>(x, yy), 0).r;
        if other == who {
            low = textureLoad(gnh, vec2<i32>(x, yy), 0).b * 255.0;
            hidden = 0;
            if low <= FLOAT {
                return 0.0;
            }
        } else if other > who {
            // Drawn after it, so in front: the run may go on behind.
            hidden += 1;
        } else {
            // The terrain or a thing behind it: the run ended above this row, as low as an
            // upright run's px could be at the last hidden row.
            let bottom = low - f32(hidden) * 1.25;
            return select(bottom, 0.0, bottom <= FLOAT);
        }
    }
    return 0.0;
}

@compute @workgroup_size(8, 8)
fn scatter(@builtin(global_invocation_id) id: vec3<u32>) {
    let w = u32(g.full.x);
    let hh = u32(g.full.y);
    if id.x >= w || id.y >= hh {
        return;
    }
    let v = textureLoad(gnh, vec2<i32>(id.xy), 0);
    let h = u32(round(v.b * 255.0));
    if f32(h) < GROUND {
        return;
    }
    let d = max(u32(round(v.a * 255.0)), 1u);
    let who = textureLoad(gid, vec2<i32>(id.xy), 0).r & 0xffffu;
    var lo = 0u;
    if who != 0u {
        lo = u32(round(run_bottom(i32(id.x), i32(id.y), f32(h), who)));
    }
    let y0 = i32(id.y + rows_up(h)) - i32(d / 2u);
    let packed = (h << 16u) | who;
    let n = w * hh;
    // A px wider each side: a dithered or combed silhouette stands as one body, and a ray
    // stepping past a thin post still finds it.
    for (var dx = -1; dx <= 1; dx++) {
        let x = i32(id.x) + dx;
        if x < 0 || x >= i32(w) {
            continue;
        }
        for (var k = 0u; k < d; k++) {
            let y = y0 + i32(k);
            if y >= 0 && y < i32(hh) {
                let i = u32(y) * w + u32(x);
                atomicMax(&hmap[i], packed);
                atomicMax(&hmap[n + i], 256u - min(lo, h));
            }
        }
    }
}
