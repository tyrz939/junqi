// The height field shadows are traced against: seen from above. Every G-buffer pixel with a
// height stands on the ground `rows_up(h)` rows below it (the 3/4 view draws a height of h px
// four fifths as tall, `jane_present::rows_up`); it raises that ground point, and the rows its
// depth covers round it, to its height (the tallest wins). An upright sprite's column lands on
// its feet, so a person becomes a thin wall as tall as she is, shaped like her, standing on the
// row she stands on; a wall's face lands on its foot and a roof on the house under it. Each
// texel keeps whose it is (the G-buffer's id) under its height, `h << 16 | id`, so a trace can
// skip the thing it starts on and the thing holding its light.

@group(0) @binding(1) var gnh: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> hmap: array<atomic<u32>>;
@group(0) @binding(3) var gid: texture_2d<u32>;

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
    let y0 = i32(id.y + rows_up(h)) - i32(d / 2u);
    let packed = (h << 16u) | who;
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
                atomicMax(&hmap[u32(y) * w + u32(x)], packed);
            }
        }
    }
}
