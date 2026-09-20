import { getItem, getSpell } from "@/game/systems/catalog";

const SCHOOL: Record<string, number> = {
  frost: 0x6aa8d8,
  fire: 0xd86830,
  nature: 0x5a9a3c,
  physical: 0xc4a878,
  heal: 0x6cbc78,
};

export type IconStyle = { fill: number; glyph: string; label: string };

export function spellIcon(id: string): IconStyle {
  const s = getSpell(id);
  const fill = SCHOOL[s.school ?? (s.castAni === "attacking" ? "physical" : "frost")] ?? 0x8899aa;
  return { fill, glyph: glyph(s.name), label: s.name };
}

export function itemIcon(id: string): IconStyle {
  const item = getItem(id);
  let fill = 0x8a7a58;
  if (item.opens) fill = 0xc4a024;
  else if (item.effect?.includes("heal") || id === "apple") fill = 0xcc4444;
  else if (item.effect) fill = 0x8866cc;
  else if (id.includes("gold")) fill = 0xd4b020;
  else if (id.includes("water") || id.includes("vial")) fill = 0x4488bb;
  else if (id.includes("key")) fill = 0xc4a024;
  return { fill, glyph: glyph(item.name), label: item.name };
}

export function iconFor(source: "item" | "spell", id: string): IconStyle {
  return source === "spell" ? spellIcon(id) : itemIcon(id);
}

function glyph(name: string): string {
  const parts = name.replace(/_/g, " ").split(" ").filter(Boolean);
  if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase();
  return name.slice(0, 2).toUpperCase();
}
