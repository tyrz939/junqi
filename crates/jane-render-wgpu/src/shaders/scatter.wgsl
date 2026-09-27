// The height field shadows are traced against: seen from above. Every G-buffer pixel with a
// height stands on the ground at (x, y + h); it raises that ground point, and the rows its depth
// covers round it, to its height (the tallest wins). An upright sprite's column lands on its
// feet, so a person becomes a thin wall as tall as she is, shaped like her; a roof lands on the
// house under it.

@group(0) @binding(1) var gnh: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> hmap: array<atomic<u32>>;

@compute @workgroup_size(8, 8)
fn scatter(@builtin(global_invocation_id) id: vec3<u32>) {
    let w = u32(g.full.x);
    let hh = u32(g.full.y);
    if id.x >= w || id.y >= hh {
        return;
    }
    let v = textureLoad(gnh, vec2<i32>(id.xy), 0);
    let h = u32(round(v.b * 255.0));
    if h < 2u {
        return;
    }
    let d = max(u32(round(v.a * 255.0)), 1u);
    let y0 = i32(id.y + h) - i32(d / 2u);
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
                atomicMax(&hmap[u32(y) * w + u32(x)], h);
            }
        }
    }
}
