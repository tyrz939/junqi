// Words are promises. The quest audit (quest-audit.test.ts) asks whether a person who reads a quest
// can find the place; this asks whether what the words say is THERE is there, the way they say it:
// the key is under a stone, the sack is in the pen with the sheep, the tin is by the cart, there are
// five dinner tins and eleven parcels, the thing on the ground looks like what it is, and nothing the
// text calls hidden can be picked up before the quest gives her a reason to look.
//
// Each claim below quotes the words it holds the world to. A new line of text that says where
// something is should get a row here, or be written so it cannot be wrong.

import { describe, expect, it } from "vitest";
import { ICONS } from "@/art/icons";
import { Tile } from "@/sim/grid";
import { Sim } from "@/sim/sim";
import { blueprintFor } from "@/sim/zones";
import type { Blueprint, PropSpawn } from "@/world/blueprint";
import { expandText } from "@/sim/text";
import { pushProp, idle, yardCatalog } from "./bot";

const catalog = yardCatalog();
const SEEDS = [3, 2026, 77];
const LONG = 600_000;

const bps = new Map<number, Blueprint>();
const county = (seed: number): Blueprint => {
  let bp = bps.get(seed);
  if (!bp) bps.set(seed, (bp = blueprintFor("county", seed)));
  return bp;
};
const prop = (bp: Blueprint, key: string): PropSpawn | undefined => bp.props.find((p) => p.key === key);
const size = (p: PropSpawn): { w: number; h: number } => catalog.props[p.def] ?? { w: 1, h: 1 };
/** Cells of open ground between two footprints (0 = touching or overlapping). */
function gap(a: PropSpawn, b: { cx: number; cy: number; w?: number; h?: number } | PropSpawn): number {
  const sa = size(a);
  const sb = "def" in b ? size(b as PropSpawn) : { w: b.w ?? 1, h: b.h ?? 1 };
  const dx = Math.max(0, a.cx - (b.cx + sb.w), b.cx - (a.cx + sa.w));
  const dy = Math.max(0, a.cy - (b.cy + sb.h), b.cy - (a.cy + sa.h));
  return Math.max(dx, dy);
}
const storyHas = (bp: Blueprint, id: string): boolean => !!bp.stories?.[id] && "box" in bp.stories[id];

/** "X is by Y": [thing, what it is by, most cells of ground between them, the words]. */
const BESIDE: [string, string, number, string][] = [
  ["lost_glove_drop", "halt_well_head", 3, "A child's red glove lies by the well wall"],
  ["lost_hat_drop", "halt_signpost_post", 3, "A man's felt hat lies at the foot of the post"],
  ["lost_tin_drop", "halt_cart_body", 3, "A dinner tin lies in the road beside it"],
  ["carters_key_drop", "carters_cart_body", 3, "The carter's key is on the ground by the tailboard"],
  ["nurses_case", "car_wreck", 3, "Her case is on the ground by the car, locked"],
];

describe("the words are true", () => {
  it("what the text says is beside something is beside it", () => {
    for (const seed of SEEDS) {
      const bp = county(seed);
      for (const [a, b, most, words] of BESIDE) {
        const pa = prop(bp, a);
        const pb = prop(bp, b);
        expect(pa && pb, `seed ${seed}: ${a} and ${b}`).toBeTruthy();
        expect(gap(pa!, pb!), `seed ${seed}: "${words}"`).toBeLessThanOrEqual(most);
      }
    }
  }, LONG);

  it("the counts the text gives are the counts in the world", () => {
    for (const seed of SEEDS) {
      const bp = county(seed);
      const count = (re: RegExp): PropSpawn[] => bp.props.filter((p) => re.test(p.key));
      // "Eleven parcels round the step in the nurse's wrapping" (note_crane)
      const parcels = count(/^crane_parcels_\d+$/);
      expect(parcels.length, `seed ${seed}: eleven parcels`).toBe(11);
      expect(parcels.every((p) => p.def === "parcel")).toBe(true);
      // "five dinner tins laid out by the fire" (down_at_five, tally_slate)
      const tins = count(/^gang_tin_\d+$/);
      expect(tins.length).toBe(5);
      expect(tins.every((p) => p.def === "dinner_tin")).toBe(true);
      const camp = bp.marks.quarry_camp;
      for (const t of tins) expect(gap(t, { cx: camp.cx, cy: camp.cy }), `seed ${seed}: tins by the fire`).toBeLessThanOrEqual(12);
      // "Three bottles of milk on the step" (door_pound_7)
      const milk = prop(bp, "pound7_milk")!;
      expect(gap(milk, prop(bp, "door_pound_7")!), `seed ${seed}: the milk is on No. 7's step`).toBeLessThanOrEqual(4);
      // "Take the ones on the ground first" (door_high_7): windfalls under the orchard trees.
      const falls = count(/^orchard_windfall_\d+$/);
      expect(falls.length).toBe(3);
      for (const f of falls) expect(bp.props.some((t) => t.def === "apple_tree" && gap(f, t) <= 4), `seed ${seed}: a windfall under a tree`).toBe(true);
    }
  }, LONG);

  it("stories: the stones, the step, the pen, the garden, the chair and the seats are there", () => {
    let seen = 0;
    for (const seed of SEEDS) {
      const bp = county(seed);
      if (storyHas(bp, "bettany")) {
        seen++;
        // "There are four stones, and they shift if you lean on them." "The spare's under a stone."
        const stones = bp.props.filter((p) => /^bettany_stone_\d$/.test(p.key));
        expect(stones.length, `seed ${seed}: four stones`).toBe(4);
        expect(stones.every((s) => catalog.props[s.def].push)).toBe(true);
        const under = stones.filter((s) => s.under === "bettany_key_drop");
        expect(under.length, `seed ${seed}: the key is under exactly one`).toBe(1);
        const key = prop(bp, "bettany_key_drop")!;
        expect(key.hidden).toBe(true);
        expect([key.cx, key.cy]).toEqual([under[0].cx, under[0].cy]);
        // In the garden: on the rows.
        for (const s of stones) expect([Tile.Crops, Tile.Dirt]).toContain(bp.tiles[s.cy * bp.w + s.cx]);
      }
      if (storyHas(bp, "hackett_house")) {
        // "My mother kept a tin under the front step."
        const step = prop(bp, "hackett_step")!;
        expect(step.under).toBe("hackett_tin_drop");
        expect(catalog.props[step.def].push).toBe(true);
        expect(prop(bp, "hackett_tin_drop")!.hidden).toBe(true);
      }
      if (storyHas(bp, "leckie")) {
        // "It's in the sheep pen." "In the pen with the sheep."
        const sack = prop(bp, "leckie_sack_drop")!;
        const sheep = bp.units.filter((u) => u.def === "sheep" && Math.max(Math.abs(u.cx - sack.cx), Math.abs(u.cy - sack.cy)) <= 7);
        expect(sheep.length, `seed ${seed}: sheep round the sack`).toBeGreaterThanOrEqual(2);
      }
      if (storyHas(bp, "denny_ruin")) {
        // "Candles. And a chair."
        expect(gap(prop(bp, "denny_chair")!, prop(bp, "denny_candles")!)).toBeLessThanOrEqual(3);
      }
      if (storyHas(bp, "voke_brother")) {
        // "Two stones are set by it to sit on."
        const seats = bp.props.filter((p) => /^voke_seat_\d$/.test(p.key));
        expect(seats.length).toBe(2);
      }
      if (storyHas(bp, "farrant_camp")) {
        // "Teaspoons? They're not mine."
        const box = bp.props.find((p) => p.loot?.some((s) => s.item === "farrant_ring"));
        expect(box?.loot?.some((s) => s.item === "crow_spoons")).toBe(true);
      }
      if (storyHas(bp, "vosper_sister")) {
        // "a biscuit tin heavy with paper is put out on the step"
        expect(prop(bp, "vosper_tin_drop")!.hidden).toBe(true);
      }
    }
    expect(seen, "Mrs Bettany has a place on some seed").toBeGreaterThan(0);
  }, LONG);

  it("a thing on the ground looks like what it is, and only paper looks like paper", () => {
    for (const seed of SEEDS) {
      for (const p of county(seed).props) {
        const def = catalog.props[p.def];
        if (!p.loot || p.loot.length === 0 || def.solid) continue;
        if (p.def !== "lost_thing") continue;
        expect(def.showsLoot, `${p.def} draws what it holds`).toBe(true);
        const item = catalog.items[p.loot[0].item];
        expect(ICONS[item.icon], `${p.key}: ${item.icon} is drawn`).toBeDefined();
        if (item.icon === "item_letter") expect(item.name, `${p.key} is paper, so it may look like paper`).toMatch(/letter|diary|note|page/i);
      }
    }
    // The story items each have an icon of their own shape, not a stand-in.
    const standIns: Record<string, string> = { item_iron: "an ingot", item_stone: "a stone", item_glass: "a glass", item_washing: "washing" };
    for (const id of ["ames_spectacles", "tull_billhook", "tobin_scissors", "leckie_eggs", "hurst_fleece", "rendle_coat"]) {
      expect(standIns[catalog.items[id].icon], `${id} is not drawn as ${standIns[catalog.items[id].icon]}`).toBeUndefined();
    }
  }, LONG);

  it("a sign names roads and places, not a person's house: Julie's no more than anybody's", () => {
    const SIGNS = new Set(["sign", "signpost", "fingerpost", "name_board", "milestone", "notice_board"]);
    for (const seed of SEEDS) {
      for (const p of county(seed).props) {
        if (!SIGNS.has(p.def)) continue;
        const words = [p.label ?? "", ...(p.use ?? []).map((a) => (a.do === "read" ? a.text : ""))];
        const tree = p.talk ? catalog.dialogue[p.talk] : undefined;
        if (tree) for (const n of Object.values(tree.nodes)) words.push(...n.lines);
        const text = expandText({ name: "Jane", seed }, words.join(" "));
        expect(/julie/i.test(text), `seed ${seed}: ${p.key} (${p.def}) says "${text.slice(0, 120)}"`).toBe(false);
      }
    }
    const book = catalog.dialogue.lost_property_book;
    for (const id of ["lp_offer", "lp_wait"]) expect(book.nodes[id].lines.join(" ")).not.toMatch(/julie/i);
  }, LONG);
});

describe("nothing the text hides can be found before she is told to look", () => {
  it("Mrs Bettany's key: a stone pushed before she asks has earth under it; asked, the key is there", () => {
    const seed = SEEDS.find((s) => storyHas(county(s), "bettany"))!;
    const sim = Sim.newGame(catalog, seed);
    sim.command({ t: "dev", dev: { op: "god", on: true } });
    sim.command({ t: "dev", dev: { op: "time", hour: 10 } });
    const top = county(seed).props.find((p) => p.under === "bettany_key_drop")!;
    const key = sim.rt.propsByKey.get("bettany_key_drop")!;
    sim.command({ t: "dev", dev: { op: "tp", zone: "county", mark: "story_bettany" } });
    idle(sim, 3);
    expect(key.hidden).toBe(true);
    expect(pushProp(sim, top.key), "the stone moves").toBe(true);
    expect(key.hidden, "nothing under it yet: she has not said").toBe(true);
    // Now she says. The stone is already off the spot, so the key is lying there.
    sim.command({ t: "dev", dev: { op: "quest", quest: "bettany_key" } });
    idle(sim, 2);
    expect(sim.state.quests.active.some((q) => q.quest === "bettany_key"), "the quest is given").toBe(true);
    expect(key.hidden, "the key is there now").toBe(false);
  }, LONG);
});
