// The height field shadows are traced against: seen from above. Every G-buffer pixel with a
// height stands on the ground `rows_up(h)` rows below it (the 3/4 view draws a height of h px
// four fifths as tall, `jane_present::rows_up`); it raises that ground point, and the rows its
// depth covers round it, to its height (the tallest wins). An upright sprite's column lands on
// its feet, so a person becomes a thin wall as tall as she is, shaped like her, standing on the
// row she stands on and the rows of her depth behind it; a wall's face lands on its foot and a
// roof on the house under it. Each
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
@group(0) @binding(4) var gem: texture_2d<f32>;
@group(0) @binding(5) var<storage, read_write> tile_tops: array<atomic<u32>>;

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

// How many of `who`'s px run on from `(x, y)` in the direction `dx`, counted to `most` at most.
fn run_on(x: i32, y: i32, dx: i32, who: u32, most: i32) -> i32 {
    let w = i32(g.full.x);
    var e = 0;
    for (; e < most; e++) {
        let xx = x + dx * (e + 1);
        if xx < 0 || xx >= w || textureLoad(gid, vec2<i32>(xx, y), 0).r != who {
            break;
        }
    }
    return e;
}

// How many rows deep a sprite px's footprint is, of its caster's depth `d`, from its foot row
// back. A thing is taken to be no deeper than it is wide, and round: its row, `wide` px, is
// `2 ((wide - 1) / 2) + 2` rows deep at most (the silhouettes' band, `shadow::bands`), and a
// column toward the row's ends a half ellipse's depth there (2 rows at the least). So a post 2 px
// wide stands 2 deep, and a bush's footprint rounds off behind its ends.
fn footprint(x: i32, y: i32, who: u32, d: u32) -> u32 {
    let l = run_on(x, y, -1, who, 64);
    let r = run_on(x, y, 1, who, 64);
    let mid = min(d, 2u * (u32(l + r) / 2u) + 2u);
    let u = (f32(min(l, r)) + 0.5) / (f32(l + r + 1) * 0.5);
    let round_off = sqrt(max(1.0 - (1.0 - u) * (1.0 - u), 0.0));
    return clamp(u32(round(f32(mid) * round_off)), min(mid, 2u), mid);
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
    let depth_byte = u32(round(v.a * 255.0));
    let depth = depth_byte & (FENCE - 1u);
    var who = textureLoad(gid, vec2<i32>(id.xy), 0).r & 0xffffu;
    // The terrain's relief up to `RELIEF` (a cobble, a tuft, a kerb) is its texture, not a
    // caster: it stands in no field, as it throws no block on T0 and T1 (`shadow::RELIEF`).
    if who == 0u && f32(h) <= RELIEF {
        return;
    }
    // A sprite of depth 0 is not a caster of the frame's (the dead, a spell's glow in her hands, a
    // prop lying flat or set into a wall): the presenter decides what casts, and it casts
    // nothing here as on T0 and T1 (PRESENTATION.md §1.7).
    if who != 0u && depth == 0u {
        return;
    }
    // What glows on a sprite (a flame, a lamp's lit glass, her lantern's) is light, not matter:
    // it stands in no field, as T0 and T1 leave its rows out (`Caster::burn`). The terrain's lit
    // windows are its walls, and stand.
    if who != 0u && any(textureLoad(gem, vec2<i32>(id.xy), 0).rgb > vec3<f32>(0.0)) {
        return;
    }
    var d = max(depth, 1u);
    var lo = 0u;
    if who != 0u {
        lo = u32(round(run_bottom(i32(id.x), i32(id.y), f32(h), who)));
        d = footprint(i32(id.x), i32(id.y), who, d);
    } else if (depth_byte & FENCE) != 0u || f32(h) <= SPILL_LOW {
        // What spills (the fence rule, `common.wgsl`): the sun's trace passes it.
        who = SPILL_ID;
    }
    // Its footprint is behind the row it stands on: a sprite's lowest px is the front of what it
    // stands on, so the ground drawn in front of its foot is never inside it (a bush's footprint
    // round its foot put the grass in front of it in its shadow, cut off square at its ends).
    let y0 = i32(id.y + rows_up(h)) - i32(d - 1u);
    let packed = (h << 16u) | who;
    let n = w * hh;
    // A sprite stands as wide as it is drawn: a post 2 px wide throws a shadow 2 px wide, as on
    // T0 and T1. The terrain a px wider each side, so a wall's end is never stepped past.
    let spread = select(0, 1, who == 0u || who == SPILL_ID);
    for (var dx = -spread; dx <= spread; dx++) {
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

// After `scatter`: each tile's tallest top (`common.wgsl`'s `TOP_TILE`), a texel counted in its
// own tile and in each tile within `TOP_GROW` px of it (the trace reads a texel or two round its
// ray, the side rays a px and a half across).
@compute @workgroup_size(8, 8)
fn tops(@builtin(global_invocation_id) id: vec3<u32>) {
    let w = u32(g.full.x);
    if id.x >= w || id.y >= u32(g.full.y) {
        return;
    }
    let top = atomicLoad(&hmap[id.y * w + id.x]) >> 16u;
    if top == 0u {
        return;
    }
    let tw = top_tiles_x();
    let th = (i32(g.full.y) + TOP_TILE - 1) / TOP_TILE;
    let x = i32(id.x);
    let y = i32(id.y);
    let tx0 = max((x - TOP_GROW) / TOP_TILE, 0);
    let tx1 = min((x + TOP_GROW) / TOP_TILE, tw - 1);
    let ty0 = max((y - TOP_GROW) / TOP_TILE, 0);
    let ty1 = min((y + TOP_GROW) / TOP_TILE, th - 1);
    for (var ty = ty0; ty <= ty1; ty++) {
        for (var tx = tx0; tx <= tx1; tx++) {
            atomicMax(&tile_tops[ty * tw + tx], top);
        }
    }
}
