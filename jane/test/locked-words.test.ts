// What a locked thing says when it is tried. The trunk at the Lost Property desk is a railway
// trunk (not the dungeon's gold-and-blue prize chest) and "The trunk is locked"; a page held down
// by a mechanism is held fast, not locked; a gate with its sign for a label reads the sign first.

import { describe, expect, it } from "vitest";
import { buildCatalog } from "@/sim/catalog";
import { lockedToast } from "@/sim/interact";
import { PLACEMENTS } from "@/world/placements";

const catalog = buildCatalog();

describe("locked things say what holds them", () => {
  it("the left-luggage trunk is a trunk, locked to its own key, and says so", () => {
    const row = { prop: PLACEMENTS.find((r) => r.key === "left_luggage_trunk")!.prop! };
    expect(row.prop.def).toBe("luggage_trunk");
    expect(row.prop.locked).toBe(true);
    expect(row.prop.keyTag).toBe("left_luggage");
    const def = catalog.props.luggage_trunk;
    expect(def.name).toBe("Trunk");
    expect(lockedToast(def, row.prop.label)).toBe("The trunk is locked");
  });

  it("a thing with no lock is not called locked, and a sign is read off the gate", () => {
    expect(lockedToast(catalog.props.leaf_page, "The page")).toBe("The page is held fast");
    expect(lockedToast(catalog.props.jar, "A jar in a niche")).toBe("A jar in a niche is out of reach");
    expect(lockedToast(catalog.props.gate_h, "No. 1 LINE")).toBe("No. 1 LINE: the gate is locked");
    expect(lockedToast(catalog.props.pipes_penstock_h, "EAST RUN")).toBe("EAST RUN: the penstock is shut");
    expect(lockedToast(catalog.props.door, "Julie's door")).toBe("Julie's door is locked");
    expect(lockedToast(catalog.props.chest, undefined)).toBe("Locked");
  });
});
