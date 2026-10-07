//! `jane bake --target psp` (PORT.md §13.4, §13.5 `C2`): the canonical pack's albedo as the PSP
//! loads it. Pages of 256 x 256 (a sprite too big for one gets a page of its own, at most 512),
//! 8-bit indexed (`GU_PSM_T8`) with a 256-entry CLUT each in `GU_PSM_8888`, the pixels swizzled
//! for the GE (16-byte x 8-row blocks). Sprites are trimmed to what they draw, identical frames
//! stored once, and shelf-packed a sprite set (a sprite's variant and seat: every frame) at a
//! time, in key order within each category, onto a page whose CLUT can take its colours exactly;
//! only a set that alone has more than 254 colours forces a page to be quantised (integer median
//! cut, every entry an exact colour of the art). The file format is PORT.md §13.4's `JPK1`.
//!
//! Why `8888` and not `5551`: the CLUT is 1 KB a page either way (512 B saved a page is noise),
//! `8888` keeps the master palette's 24-bit colours exactly where `5551` would requantise every
//! ramp to 15 bits, and the contact shadow (index 1) needs a partial alpha that `5551` lacks.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use jane_present::atlas::Atlas;

use crate::bake::{Cat, Item, Pack};

/// The page side the GE is happiest with (its limit is 512).
pub const PAGE: u16 = 256;
/// The largest texture the GE takes.
pub const MAX_TEX: u16 = 512;
/// CLUT slots for colours: 0 is clear and 1 the contact shadow.
const SLOTS: usize = 254;
/// How many of a category's newest pages a set may still go onto.
const OPEN: usize = 4;
/// The contact shadow's CLUT entry, ABGR: a cool near-black at alpha 0x58, which darkens what is
/// under it to about 0.66, where the PC's multiply (`palette::AO_TINT`) gives about 0.65 to 0.77
/// a channel, cooler.
const AO_ABGR: u32 = 0x5830_1010;
/// `GU_PSM_T8`, `GU_PSM_8888`.
const PSM_T8: u8 = 5;
const PSM_8888: u8 = 3;

/// A packed page.
#[derive(Clone, Debug)]
pub struct Page {
    pub cat: Cat,
    /// The page group (`JPK2`): a unit's `SpriteId` (its every variant, seat and frame, so a zone
    /// loads the units it spawns and no others), else `u16::MAX` (the whole category).
    pub group: u16,
    pub w: u16,
    pub h: u16,
    /// ABGR8888, 256 entries.
    pub clut: Vec<u32>,
    /// Linear (not yet swizzled) indices, `w * h`.
    pub px: Vec<u8>,
    /// Shelves: (y, height, x used).
    shelves: Vec<(u16, u16, u16)>,
    bottom: u16,
    /// Pack palette indices on this page.
    colours: BTreeSet<u16>,
    /// Whether the CLUT is a quantisation (more colours than slots), and its worst error.
    pub quantised: bool,
    pub max_err: u32,
}

/// Where a frame was put.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rec {
    pub cat: Cat,
    pub sprite: u16,
    pub frame: u8,
    pub vs: u8,
    /// `u16::MAX` for a frame that draws nothing.
    pub page: u16,
    pub u: u16,
    pub v: u16,
    pub w: u16,
    pub h: u16,
    pub ax: i16,
    pub ay: i16,
    /// The trim: where the stored rect starts in the untrimmed frame (not in the file).
    pub tx: u16,
    pub ty: u16,
}

/// A presenter ref as the PSP pack keeps it: the sprite record it draws (by key) and its own
/// anchor, untrimmed (the presenter's, which for a prop is its footprint's foot, not the look's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RefLink {
    pub key: (Cat, u16, u8, u8),
    pub ax: i16,
    pub ay: i16,
}

/// The PSP pack.
#[derive(Clone, Debug)]
pub struct Psp {
    pub pages: Vec<Page>,
    pub recs: Vec<Rec>,
    /// Every presenter `RefId`, in order: what it draws (`None`: a sprite the GE cannot take).
    pub refs: Vec<Option<RefLink>>,
    /// Per category: frames, unique frames, drawn px and trimmed rect px of the unique frames.
    stats: BTreeMap<Cat, (usize, usize, usize, usize)>,
}

/// A frame trimmed to its drawn rect: offset into the item, size, indices.
struct Trim {
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    px: Vec<u16>,
}

fn trim(it: &Item) -> Trim {
    let (w, h) = (usize::from(it.w), usize::from(it.h));
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if it.albedo[y * w + x] != 0 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
        }
    }
    if x1 == 0 {
        return Trim { x: 0, y: 0, w: 0, h: 0, px: Vec::new() };
    }
    let px = (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).map(|(x, y)| it.albedo[y * w + x]).collect();
    Trim { x: x0 as u16, y: y0 as u16, w: (x1 - x0) as u16, h: (y1 - y0) as u16, px }
}

impl Page {
    fn new(cat: Cat, group: u16, w: u16, h: u16) -> Page {
        Page {
            cat,
            group,
            w,
            h,
            clut: vec![0; 256],
            px: vec![0; usize::from(w) * usize::from(h)],
            shelves: Vec::new(),
            bottom: 0,
            colours: BTreeSet::new(),
            quantised: false,
            max_err: 0,
        }
    }

    /// Room for a `w x h` rect: the lowest shelf tall enough with room left, else a new shelf.
    fn place(&mut self, w: u16, h: u16) -> Option<(u16, u16)> {
        let best = self.shelves.iter_mut().filter(|s| s.1 >= h && s.2 + w <= self.w).min_by_key(|s| (s.1, s.0));
        if let Some(s) = best {
            let at = (s.2, s.0);
            s.2 += w;
            return Some(at);
        }
        if self.bottom + h <= self.h && w <= self.w {
            self.shelves.push((self.bottom, h, w));
            let at = (0, self.bottom);
            self.bottom += h;
            return Some(at);
        }
        None
    }

    /// The height to ship: the next power of two over what was used (the GE wants one).
    fn used_h(&self) -> u16 {
        self.bottom.max(8).next_power_of_two().min(self.h)
    }
}

/// A trimmed frame's size and indices: identical frames are stored once.
type FrameKey = (u16, u16, Vec<u16>);

/// Packs the canonical pack's albedo and the presenter's own atlas for the PSP. Every presenter
/// ref keyed to a look the canonical pack holds must draw the canonical item's pixels exactly (the
/// two cannot drift: a ref that differs fails the bake); a keyed ref the canonical pack lacks (a
/// person's lantern set) joins its sprite's group, and an unkeyed one is a `Scene` sprite keyed by
/// its `RefId`. A scene sprite over the GE's 512 is left out (its ref draws nothing on the PSP).
pub fn pack(src: &Pack, presenter: &Atlas) -> Result<Psp, String> {
    let mut items = src.items.clone();
    let mut refs = Vec::with_capacity(presenter.refs.len());
    let mut extra = Vec::new();
    for (id, r) in presenter.refs.iter().enumerate() {
        let page = &presenter.pages.pages[usize::from(r.page)];
        let (w, h) = (usize::from(r.src.w), usize::from(r.src.h));
        let albedo: Vec<u16> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| page.albedo[(usize::from(r.src.y) + y) * usize::from(page.w) + usize::from(r.src.x) + x])
            .collect();
        let key = match presenter.keys[id] {
            Some(k) => {
                (Cat::of(k.cat).ok_or(format!("bake psp: ref {id} has category {}", k.cat))?, k.sprite, k.vs, k.frame)
            }
            None => (Cat::Scene, id as u16, 0, 0),
        };
        if key.0 == Cat::Scene && (w > usize::from(MAX_TEX) || h > usize::from(MAX_TEX)) {
            refs.push(None);
            continue;
        }
        refs.push(Some(RefLink { key, ax: r.ax, ay: r.ay }));
        match src.items.binary_search_by_key(&key, Item::key) {
            Ok(i) => {
                let it = &src.items[i];
                if (usize::from(it.w), usize::from(it.h)) != (w, h) || it.albedo != albedo {
                    return Err(format!(
                        "bake psp: presenter ref {id} ({}) draws other px than the generators' {}: the two have drifted",
                        key.0.name(),
                        it.name
                    ));
                }
            }
            Err(_) if extra.iter().any(|e: &Item| e.key() == key) => {}
            Err(_) => extra.push(Item {
                cat: key.0,
                sprite: key.1,
                frame: key.3,
                vs: key.2,
                name: format!("ref {id}"),
                w: r.src.w,
                h: r.src.h,
                ax: r.ax,
                ay: r.ay,
                albedo,
                normal: Vec::new(),
                emissive: Vec::new(),
                height: Vec::new(),
            }),
        }
    }
    items.extend(extra);
    items.sort_by_key(Item::key);
    let mut psp = pack_items(&Pack { palette: src.palette.clone(), master: src.master, items });
    psp.refs = refs;
    Ok(psp)
}

/// Packs a pack's items' albedo for the PSP.
pub fn pack_items(src: &Pack) -> Psp {
    let mut pages: Vec<Page> = Vec::new();
    let mut recs = Vec::with_capacity(src.items.len());
    let mut stats: BTreeMap<Cat, (usize, usize, usize, usize)> = BTreeMap::new();
    // Placed frames by their size and indices: (page, u, v).
    let mut seen: BTreeMap<FrameKey, (u16, u16, u16)> = BTreeMap::new();
    // Each pixel's index on its page, filled in once every page's CLUT is known: (page, u, v, trim).
    let mut blits: Vec<(usize, u16, u16, Trim)> = Vec::new();
    let mut i = 0;
    while i < src.items.len() {
        let first = &src.items[i];
        let n = src.items[i..]
            .iter()
            .take_while(|it| (it.cat, it.sprite, it.vs) == (first.cat, first.sprite, first.vs))
            .count();
        let set = &src.items[i..i + n];
        i += n;
        let cat = first.cat;
        let group = if cat == Cat::Units { first.sprite } else { u16::MAX };
        // The set's new frames, trimmed, tallest first.
        let mut fresh: Vec<(usize, Trim)> = Vec::new();
        let mut colours = BTreeSet::new();
        let mut keys: Vec<Option<FrameKey>> = Vec::with_capacity(n);
        for (k, it) in set.iter().enumerate() {
            let t = trim(it);
            let st = stats.entry(cat).or_default();
            st.0 += 1;
            if t.w == 0 {
                keys.push(None);
                continue;
            }
            let key = (t.w, t.h, t.px.clone());
            if !seen.contains_key(&key) && !fresh.iter().any(|(_, f)| (f.w, f.h, &f.px) == (key.0, key.1, &key.2)) {
                st.1 += 1;
                st.2 += t.px.iter().filter(|&&p| p != 0).count();
                st.3 += t.px.len();
                colours.extend(t.px.iter().copied().filter(|&p| p > 1));
                fresh.push((k, t));
            }
            keys.push(Some(key));
        }
        fresh.sort_by_key(|(k, t)| (std::cmp::Reverse(t.h), *k));
        let sizes: Vec<(u16, u16)> = fresh.iter().map(|(_, t)| (t.w, t.h)).collect();
        // A page of this category and group that takes the set exactly, else a new one.
        let open =
            pages.iter().enumerate().filter(|(_, p)| p.cat == cat && p.group == group && p.w == PAGE).map(|(pi, _)| pi);
        let candidates: Vec<usize> = open.collect::<Vec<_>>().into_iter().rev().take(OPEN).collect();
        // Where each fresh frame goes: (page, u, v).
        // A frame at a time, oldest open page first: a set may run on from one page onto the
        // next, so pages fill; a frame too big for a page gets one of its own.
        let mut open: Vec<usize> =
            candidates.into_iter().rev().filter(|&pi| pages[pi].colours.union(&colours).count() <= SLOTS).collect();
        let mut at: Vec<(usize, u16, u16)> = Vec::with_capacity(sizes.len());
        for &(w, h) in &sizes {
            let spot = open.iter().find_map(|&pi| pages[pi].place(w, h).map(|(u, v)| (pi, u, v)));
            let spot = spot.unwrap_or_else(|| {
                let side = |v: u16| v.next_power_of_two().clamp(PAGE, MAX_TEX);
                assert!(
                    w <= MAX_TEX && h <= MAX_TEX,
                    "bake psp: {} sprite {} is over 512 px",
                    cat.name(),
                    first.sprite
                );
                pages.push(Page::new(cat, group, side(w), side(h)));
                let pi = pages.len() - 1;
                open.push(pi);
                let (u, v) = pages[pi].place(w, h).unwrap_or((0, 0));
                (pi, u, v)
            });
            at.push(spot);
        }
        for (pi, _, _) in &at {
            pages[*pi].colours.extend(colours.iter().copied());
        }
        for ((_, t), (pi, u, v)) in fresh.into_iter().zip(at) {
            seen.insert((t.w, t.h, t.px.clone()), (pi as u16, u, v));
            blits.push((pi, u, v, t));
        }
        for (it, key) in set.iter().zip(keys) {
            let rec = match key {
                None => Rec {
                    cat,
                    sprite: it.sprite,
                    frame: it.frame,
                    vs: it.vs,
                    page: u16::MAX,
                    u: 0,
                    v: 0,
                    w: 0,
                    h: 0,
                    ax: it.ax,
                    ay: it.ay,
                    tx: 0,
                    ty: 0,
                },
                Some(key) => {
                    let (page, u, v) = seen[&key];
                    let t = trim(it);
                    Rec {
                        cat,
                        sprite: it.sprite,
                        frame: it.frame,
                        vs: it.vs,
                        page,
                        u,
                        v,
                        w: t.w,
                        h: t.h,
                        ax: it.ax - t.x as i16,
                        ay: it.ay - t.y as i16,
                        tx: t.x,
                        ty: t.y,
                    }
                }
            };
            recs.push(rec);
        }
    }
    // Each page's CLUT, and the pixels through it.
    let mut maps: Vec<BTreeMap<u16, u8>> = Vec::with_capacity(pages.len());
    for p in &mut pages {
        maps.push(clut(p, &src.palette));
    }
    for (pi, u, v, t) in blits {
        let (p, map) = (&mut pages[pi], &maps[pi]);
        for y in 0..t.h {
            for x in 0..t.w {
                let ix = t.px[usize::from(y) * usize::from(t.w) + usize::from(x)];
                let c = match ix {
                    0 => 0,
                    1 => 1,
                    _ => map[&ix],
                };
                p.px[usize::from(v + y) * usize::from(p.w) + usize::from(u + x)] = c;
            }
        }
    }
    for p in &mut pages {
        let h = p.used_h();
        p.px.truncate(usize::from(p.w) * usize::from(h));
        p.h = h;
    }
    Psp { pages, recs, refs: Vec::new(), stats }
}

/// Weighted squared distance, green heaviest.
fn dist(a: [u8; 3], b: [u8; 3]) -> u32 {
    let d = |i: usize, w: u32| {
        let x = u32::from(a[i].abs_diff(b[i]));
        w * x * x
    };
    d(0, 3) + d(1, 4) + d(2, 2)
}

/// Fills the page's CLUT; returns pack index -> CLUT slot.
fn clut(p: &mut Page, palette: &[[u8; 3]]) -> BTreeMap<u16, u8> {
    let abgr = |c: [u8; 3]| 0xff00_0000 | u32::from(c[2]) << 16 | u32::from(c[1]) << 8 | u32::from(c[0]);
    p.clut[0] = 0;
    p.clut[1] = AO_ABGR;
    let colours: Vec<u16> = p.colours.iter().copied().collect();
    let mut map = BTreeMap::new();
    if colours.len() <= SLOTS {
        for (k, &ix) in colours.iter().enumerate() {
            p.clut[2 + k] = abgr(palette[usize::from(ix)]);
            map.insert(ix, (2 + k) as u8);
        }
        return map;
    }
    // More colours than slots: median cut by colour (every pack colour weighs one; the art's
    // ramps matter more than how many px use each tone), each box's entry the member nearest its
    // mean, so every CLUT entry is a colour the art drew.
    p.quantised = true;
    let reps = median_cut(&colours.iter().map(|&ix| palette[usize::from(ix)]).collect::<Vec<_>>(), SLOTS);
    for (k, &c) in reps.iter().enumerate() {
        p.clut[2 + k] = abgr(c);
    }
    for &ix in &colours {
        let c = palette[usize::from(ix)];
        let (k, e) =
            reps.iter().enumerate().map(|(k, &r)| (k, dist(c, r))).min_by_key(|&(k, e)| (e, k)).unwrap_or((0, 0));
        p.max_err = p.max_err.max(e);
        map.insert(ix, (2 + k) as u8);
    }
    map
}

/// Integer median cut of distinct colours into at most `k` boxes; each box's representative is
/// its member nearest the box's mean.
pub fn median_cut(colours: &[[u8; 3]], k: usize) -> Vec<[u8; 3]> {
    let mut boxes: Vec<Vec<[u8; 3]>> = vec![colours.to_vec()];
    let range = |b: &[[u8; 3]], ch: usize| {
        let (lo, hi) = b.iter().fold((255u8, 0u8), |(lo, hi), c| (lo.min(c[ch]), hi.max(c[ch])));
        u32::from(hi.saturating_sub(lo))
    };
    while boxes.len() < k {
        // The box with the widest channel (weighted as `dist`), ties to the first.
        let Some((bi, ch, _)) = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .flat_map(|(bi, b)| [(0, 3), (1, 4), (2, 2)].map(|(ch, w)| (bi, ch, w * range(b, ch) * range(b, ch))))
            .max_by_key(|&(bi, ch, s)| (s, std::cmp::Reverse(bi), std::cmp::Reverse(ch)))
        else {
            break;
        };
        let mut b = std::mem::take(&mut boxes[bi]);
        b.sort_by_key(|c| (c[ch], *c));
        let hi = b.split_off(b.len() / 2);
        boxes[bi] = b;
        boxes.push(hi);
    }
    boxes
        .iter()
        .map(|b| {
            let n = b.len() as u32;
            let sum = b
                .iter()
                .fold([0u32; 3], |s, c| [s[0] + u32::from(c[0]), s[1] + u32::from(c[1]), s[2] + u32::from(c[2])]);
            let mean = sum.map(|v| ((v + n / 2) / n) as u8);
            *b.iter().min_by_key(|&&c| (dist(c, mean), c)).unwrap_or(&mean)
        })
        .collect()
}

/// The GE's swizzle for a `width_bytes`-wide texture: 16-byte x 8-row blocks, row-major blocks.
pub fn swizzle(px: &[u8], width_bytes: usize) -> Vec<u8> {
    let rows = px.len() / width_bytes;
    let mut out = Vec::with_capacity(px.len());
    for by in (0..rows).step_by(8) {
        for bx in (0..width_bytes).step_by(16) {
            for y in by..by + 8 {
                out.extend_from_slice(&px[y * width_bytes + bx..y * width_bytes + bx + 16]);
            }
        }
    }
    out
}

fn align(v: &mut Vec<u8>, to: usize) {
    v.resize(v.len().div_ceil(to) * to, 0);
}

impl Psp {
    /// The page groups (`JPK2`): runs of pages of one category and group, in page order, as
    /// `(category, group, first page, pages)`. A unit's pages are its own; every other category
    /// is one group.
    pub fn groups(&self) -> Vec<(Cat, u16, u16, u16)> {
        let mut out: Vec<(Cat, u16, u16, u16)> = Vec::new();
        for (i, p) in self.pages.iter().enumerate() {
            match out.last_mut() {
                Some(g) if (g.0, g.1) == (p.cat, p.group) => g.3 += 1,
                _ => out.push((p.cat, p.group, i as u16, 1)),
            }
        }
        out
    }

    /// The `JPK2` file (PORT.md §13.4), little-endian as the PSP is.
    pub fn bytes(&self, canonical: u64) -> Vec<u8> {
        let mut recs = self.recs.clone();
        recs.sort_by_key(|r| (r.cat, r.sprite, r.vs, r.frame));
        let groups = self.groups();
        assert_eq!(
            groups.iter().map(|g| (g.0, g.1)).collect::<BTreeSet<_>>().len(),
            groups.len(),
            "bake psp: a page group's pages are not contiguous"
        );
        let page_off = 48usize;
        let rec_off = page_off + 16 * self.pages.len();
        let group_off = rec_off + 20 * recs.len();
        let ref_off = group_off + 8 * groups.len();
        let data_off = (ref_off + 8 * self.refs.len()).div_ceil(64) * 64;
        let mut out = Vec::new();
        out.extend_from_slice(b"JPK2");
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&(self.pages.len() as u16).to_le_bytes());
        for v in [recs.len(), page_off, rec_off, data_off] {
            out.extend_from_slice(&(v as u32).to_le_bytes());
        }
        out.extend_from_slice(&canonical.to_le_bytes());
        out.extend_from_slice(&(group_off as u32).to_le_bytes());
        out.extend_from_slice(&(groups.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(ref_off as u32).to_le_bytes());
        out.extend_from_slice(&(self.refs.len() as u32).to_le_bytes());
        let mut off = data_off;
        for p in &self.pages {
            out.extend_from_slice(&(off as u32).to_le_bytes());
            out.extend_from_slice(&p.w.to_le_bytes());
            out.extend_from_slice(&p.h.to_le_bytes());
            out.extend_from_slice(&[p.cat as u8, PSM_T8, PSM_8888, 1 | u8::from(p.quantised) << 1]);
            out.extend_from_slice(&(p.px.len() as u32).to_le_bytes());
            off += (1024 + p.px.len()).div_ceil(64) * 64;
        }
        for r in &recs {
            out.extend_from_slice(&[r.cat as u8, r.frame]);
            out.extend_from_slice(&r.sprite.to_le_bytes());
            out.extend_from_slice(&[r.vs, 0]);
            for v in [r.page, r.u, r.v, r.w, r.h] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            for v in [r.ax, r.ay] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for (cat, group, first, n) in &groups {
            out.extend_from_slice(&[*cat as u8, 0]);
            for v in [*group, *first, *n] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for link in &self.refs {
            // The record it draws, and its anchor from the trimmed rect's top-left (the trim is
            // the record's: the record's anchor is the item's less the trim).
            let found = link.and_then(|l| {
                let i = recs.binary_search_by_key(&l.key, |r| (r.cat, r.sprite, r.vs, r.frame)).ok()?;
                Some((i, l, recs[i]))
            });
            let (i, ax, ay) = match found {
                Some((i, l, r)) if r.page != u16::MAX => (i as u32, l.ax - r.tx as i16, l.ay - r.ty as i16),
                _ => (u32::MAX, 0, 0),
            };
            out.extend_from_slice(&i.to_le_bytes());
            out.extend_from_slice(&ax.to_le_bytes());
            out.extend_from_slice(&ay.to_le_bytes());
        }
        assert_eq!(out.len(), ref_off + 8 * self.refs.len());
        align(&mut out, 64);
        for p in &self.pages {
            p.clut.iter().for_each(|c| out.extend_from_slice(&c.to_le_bytes()));
            out.extend_from_slice(&swizzle(&p.px, usize::from(p.w)));
            align(&mut out, 64);
        }
        out
    }

    /// Sizes by category against PORT.md §13.2's PSP art budget.
    pub fn report(&self) -> String {
        let mut s = String::from("  category    frames  unique   pages     bytes  rects  drawn  quantised\n");
        let mut total = 0usize;
        for cat in Cat::ALL {
            let pages: Vec<&Page> = self.pages.iter().filter(|p| p.cat == cat).collect();
            let bytes: usize = pages.iter().map(|p| 1024 + p.px.len()).sum();
            let area: usize = pages.iter().map(|p| p.px.len()).sum();
            let (frames, unique, drawn, rects) = self.stats.get(&cat).copied().unwrap_or_default();
            let q = pages.iter().filter(|p| p.quantised).count();
            let worst = pages.iter().map(|p| p.max_err).max().unwrap_or(0);
            total += bytes;
            let _ = writeln!(
                s,
                "  {:<10} {frames:>7} {unique:>7} {:>7} {:>9} {:>5}% {:>5}%  {q} (worst err {worst})",
                cat.name(),
                pages.len(),
                bytes,
                (rects * 100).checked_div(area).unwrap_or(0),
                (drawn * 100).checked_div(area).unwrap_or(0),
            );
        }
        let budget = 6_000_000usize;
        let _ = writeln!(
            s,
            "  total {} bytes in {} pages: {}% of the 3 MB RAM + 3 MB VRAM art budget (PORT.md §13.2)",
            total,
            self.pages.len(),
            total * 100 / budget
        );
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unswizzle(px: &[u8], wb: usize) -> Vec<u8> {
        let mut out = vec![0; px.len()];
        let mut i = 0;
        for by in (0..px.len() / wb).step_by(8) {
            for bx in (0..wb).step_by(16) {
                for y in by..by + 8 {
                    out[y * wb + bx..y * wb + bx + 16].copy_from_slice(&px[i..i + 16]);
                    i += 16;
                }
            }
        }
        out
    }

    #[test]
    fn swizzle_is_blocks_of_16_by_8() {
        let px: Vec<u8> = (0..32 * 16).map(|i| (i * 7 % 251) as u8).collect();
        let s = swizzle(&px, 32);
        // The first block is the top-left 16 x 8, row by row.
        assert_eq!(&s[..16], &px[..16]);
        assert_eq!(&s[16..32], &px[32..48]);
        // The second block is the top-right 16 x 8.
        assert_eq!(&s[128..144], &px[16..32]);
        assert_eq!(unswizzle(&s, 32), px);
    }

    #[test]
    fn median_cut_keeps_few_and_cuts_many() {
        let few: Vec<[u8; 3]> = (0..10u8).map(|i| [i * 20, 0, 0]).collect();
        let mut r = median_cut(&few, 254);
        r.sort_unstable();
        let mut f = few.clone();
        f.sort_unstable();
        assert_eq!(r, f);
        let many: Vec<[u8; 3]> =
            (0..1000u32).map(|i| [(i * 37 % 256) as u8, (i * 91 % 256) as u8, (i % 256) as u8]).collect();
        let r = median_cut(&many, 254);
        assert_eq!(r.len(), 254);
        assert!(r.iter().all(|c| many.contains(c)), "every entry is a colour the art drew");
        assert_eq!(r, median_cut(&many, 254), "deterministic");
    }

    fn item(sprite: u16, w: u16, h: u16, f: impl Fn(u16, u16) -> u16) -> Item {
        let albedo = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| f(x, y)).collect();
        Item {
            cat: Cat::Props,
            sprite,
            frame: 0,
            vs: 0,
            name: String::new(),
            w,
            h,
            ax: 0,
            ay: h as i16,
            albedo,
            normal: Vec::new(),
            emissive: Vec::new(),
            height: Vec::new(),
        }
    }

    #[test]
    fn packs_trimmed_deduped_and_exact() {
        let palette: Vec<[u8; 3]> = (0..400u32).map(|i| [(i % 256) as u8, (i / 2) as u8, 7]).collect();
        let items = vec![
            item(1, 20, 20, |x, y| if (4..10).contains(&x) && (5..15).contains(&y) { 2 + x + y } else { 0 }),
            item(2, 20, 20, |x, y| if (4..10).contains(&x) && (5..15).contains(&y) { 2 + x + y } else { 0 }),
            item(3, 8, 8, |_, _| 0),
            item(4, 300, 40, |x, _| 100 + x),
        ];
        let psp = pack_items(&Pack { palette: palette.clone(), master: 400, items });
        let (a, b, c) = (psp.recs[0], psp.recs[1], psp.recs[2]);
        assert_eq!((a.w, a.h, a.ax, a.ay), (6, 10, -4, 15));
        assert_eq!((a.page, a.u, a.v), (b.page, b.u, b.v), "identical frames stored once");
        assert_eq!(c.page, u16::MAX);
        let big = psp.recs[3];
        assert_eq!(psp.pages[usize::from(big.page)].w, 512);
        // Every drawn px reads back its colour through its page's CLUT.
        let p = &psp.pages[usize::from(a.page)];
        let slot = p.px[usize::from(a.v + 2) * usize::from(p.w) + usize::from(a.u + 3)];
        let abgr = p.clut[usize::from(slot)];
        let c = palette[usize::from(2 + 7 + 7u16)];
        assert_eq!(abgr, 0xff00_0000 | u32::from(c[2]) << 16 | u32::from(c[1]) << 8 | u32::from(c[0]));
        assert!(!p.quantised);
        // The wide one draws 300 colours: its page is cut to 254, every entry one it drew.
        let q = &psp.pages[usize::from(big.page)];
        assert!(q.quantised && q.max_err > 0);
        let drawn: Vec<u32> = (100..400u16)
            .map(|i| palette[usize::from(i)])
            .map(|c| 0xff00_0000 | u32::from(c[2]) << 16 | u32::from(c[1]) << 8 | u32::from(c[0]))
            .collect();
        assert!(q.clut[2..].iter().all(|e| drawn.contains(e)));
        let file = psp.bytes(0);
        assert_eq!(&file[..4], b"JPK2");
        assert_eq!(file.len() % 64, 0);
    }
}
