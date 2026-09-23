// The people of the tales (data/units/tales.json, QUEST-TREE.md "Tales"): the few in the county who are
// plainly more than the neighbour at the gate. Each is the country drawing (art/units/country.ts: a 16x20
// person, a little shorter and broader than {name}) with one thing about them nobody else has, and that
// one thing is what their story turns on:
//
//   Miss Hanney     a bee veil down to her shoulders
//   the man at the hives, after the bell: the same veil, and a coat gone the colour of night
//   Mrs Pollard     curlers and a quilted dressing gown, out of doors in the day
//   Mr Pollard      pyjamas under his overcoat, a bell on his ankle, a dinner under a cloth in his hands
//   Mr Pargeter     a panama and a cardigan, very old
//   Mr Coker        a leather apron and a flat cap, hands like a spade
//   Mr Rook         shirt-sleeves and braces, a claw hammer, and bare feet (his boots are on the step)
//   Mary Rook       a nightdress to the ground, long wet hair, bare feet
//   Mr Denholm      a brass diving helmet and a canvas suit; later the same suit, and a bare wet head
//   Mrs Wakes       sat in her rocking chair, a shawl, and the scarf in her lap
//   Mr Voysey       a railwayman's cap with the Company's badge, a watch chain
//   Mrs Voysey      a cloche hat and her good coat, a suitcase in her hand
//   Mr Sorrell      a long coat, and his pole over his shoulder with what he has caught tied on it
//
// Frames as the base sheet: down / up / side (side faces east) and the walk alternates.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

// --- the country drawing (as art/units/country.ts) ----------------------------------------------------
//   h H hair   s S skin   a A coat   v front   e E trousers or skirt   T boots   z Z hat

type Head = { down: string[]; up: string[]; side: string[] };

const HEAD_BARE: Head = {
  down: ["................", ".....kkkkkk.....", "....khhhhhHk....", "...khhhhhhhHk...", "...khhhhhhhHk...", "...khssssssHk...", "...kssksskssk...", "...kSssssssSk...", "....kSssssSk...."],
  up: ["................", ".....kkkkkk.....", "....khhhhhHk....", "...khhhhhhhHk...", "...khhhhhhhHk...", "...khhhhhhhHk...", "...khhhhhhhHk...", "...kHhhhhhHHk...", "....kHHHHHHk...."],
  side: ["................", ".....kkkkk......", "....khhhhhk.....", "...khhhhhhhk....", "...khhhhhhhk....", "...khhhhsssk....", "...khhhssksk....", "...kHhhsssssk...", "....kHhssSkk...."],
};

const HEAD_HAT: Head = {
  down: ["................", "......kkkk......", ".....kzzzZk.....", "..kkkzzzzzZkkk..", "..kZZZZZZZZZZk..", "...kkssssssSk...", "...kssksskssk...", "...kSssssssSk...", "....kSssssSk...."],
  up: ["................", "......kkkk......", ".....kzzzZk.....", "..kkkzzzzzZkkk..", "..kZZZZZZZZZZk..", "...khhhhhhhHk...", "...khhhhhhhHk...", "...kHhhhhhHHk...", "....kHHHHHHk...."],
  side: ["................", ".....kkkk.......", "....kzzzZk......", "...kzzzzzZkk....", "..kZZZZZZZZZk...", "...khhhhsssk....", "...khhhssksk....", "...kHhhsssssk...", "....kHhssSkk...."],
};

// A bee veil: a wide hat and a net that hangs to the shoulders, face only a shadow through it.
const HEAD_VEIL: Head = {
  down: ["................", "......kkkk......", ".....kzzzZk.....", "..kkkzzzzzZkkk..", ".kZZZZZZZZZZZZk.", "..kgWgWgWgWgWk..", "...kWgSgSgWgk...", "...kgWgWgWgWk...", "....kWgWgWgk...."],
  up: ["................", "......kkkk......", ".....kzzzZk.....", "..kkkzzzzzZkkk..", ".kZZZZZZZZZZZZk.", "..kgWgWgWgWgWk..", "...kWgWgWgWgk...", "...kgWgWgWgWk...", "....kWgWgWgk...."],
  side: ["................", ".....kkkk.......", "....kzzzZk......", "...kzzzzzZkk....", ".kZZZZZZZZZZZk..", "..kgWgWgWgWk....", "..kWgWgSgSgk....", "..kgWgWgWgWk....", "...kWgWgWgk....."],
};

// A railwayman's peaked cap, the Company's badge in brass on the front.
const HEAD_CAP: Head = {
  down: ["................", "......kkkk......", "....kzzzzzZk....", "...kzzzzyzzZk...", "...kZZZZZZZZk...", "..kkkkssssSkkk..", "...kssksskssk...", "...kSssssssSk...", "....kSssssSk...."],
  up: ["................", "......kkkk......", "....kzzzzzZk....", "...kzzzzzzzZk...", "...kZZZZZZZZk...", "...khhhhhhhHk...", "...khhhhhhhHk...", "...kHhhhhhHHk...", "....kHHHHHHk...."],
  side: ["................", ".....kkkk.......", "....kzzzzZk.....", "...kzzzzzyZk....", "...kZZZZZZZZkkk.", "...khhhhsssk....", "...khhhssksk....", "...kHhhsssssk...", "....kHhssSkk...."],
};

// A brass diving helmet: the round faceplate, dark glass with the light on it, the bolts at the collar.
const HEAD_HELMET: Head = {
  down: ["....kkkkkkkk....", "...kyyyyyyYYk...", "..kyykkkkkkYYk..", "..kykxxbxxxkYk..", "..kykxxxxxxkYk..", "..kykxxxxxxkYk..", "..kyykkkkkkYYk..", "..kYyyyyyyYYYk..", "..kdkYYYYYYkdk.."],
  up: ["....kkkkkkkk....", "...kyyyyyyYYk...", "..kyyyyyyyyYYk..", "..kyyyyYyyyyYk..", "..kyyyyyyyyyYk..", "..kyyyyyyyyyYk..", "..kyyyyyyyyYYk..", "..kYyyyyyyYYYk..", "..kdkYYYYYYkdk.."],
  side: ["....kkkkkkk.....", "...kyyyyyyYk....", "..kyyyyykkkYk...", "..kyyyyykxbkk...", "..kyyyyykxxkk...", "..kyyyyykkkYk...", "..kyyyyyyyYYk...", "..kYyyyyyYYYk...", "...kdYYYYYkd...."],
};

type Body = { down: string[]; down2: string[]; up: string[]; up2: string[]; side: string[]; side2: string[] };

const BODY: Body = {
  down: ["....kkaaaakk....", "...kaavvvvaAk...", "..kaAavvvvaAAk..", "..kaAavvvvaAAk..", "..ksAavvvvaAsk..", "..kkkavvvvakkk..", "...kaavvvvaAk...", "...keeeeeeeEk...", "....keEkkeEk....", "....kTTkkTTk....", "....kkkkkkkk...."],
  down2: ["....kkaaaakk....", "...kaavvvvaAk...", "..kaAavvvvaAAk..", "..kaAavvvvaAAk..", "..ksAavvvvaAsk..", "..kkkavvvvakkk..", "...kaavvvvaAk...", "...keeeeeeeEk...", "....keEkkkkk....", "....kTTk........", "....kkkk........"],
  up: ["....kkaaaakk....", "...kaaaaaaaAk...", "..kaAaaaaaaAAk..", "..kaAaaaaaaAAk..", "..ksAaaaaaaAsk..", "..kkkaaaaaakkk..", "...kaaaaaaaAk...", "...keeeeeeeEk...", "....keEkkeEk....", "....kTTkkTTk....", "....kkkkkkkk...."],
  up2: ["....kkaaaakk....", "...kaaaaaaaAk...", "..kaAaaaaaaAAk..", "..kaAaaaaaaAAk..", "..ksAaaaaaaAsk..", "..kkkaaaaaakkk..", "...kaaaaaaaAk...", "...keeeeeeeEk...", "....kkkkkeEk....", "........kTTk....", "........kkkk...."],
  side: [".....kkaaakk....", "....kaaaavvk....", "....kaAaavvk....", "....kaAaavvk....", "....kaAsSavk....", "....kkaaaavk....", "....kaaaavvk....", "....keeeeeEk....", ".....keEeEk.....", ".....kTTTTk.....", ".....kkkkkk....."],
  side2: [".....kkaaakk....", "....kaaaavvk....", "....kaAaavvk....", "....kaAaavvk....", "....kaAsSavk....", "....kkaaaavk....", "....kaaaavvk....", "....keeeeeEk....", "....keEk.keEk...", "...kTTk..kTTk...", "...kkkk..kkkk..."],
};

// A long skirt or gown to the ground: no gap between the legs, the feet just showing.
const GOWN: Body = {
  ...BODY,
  down: [...BODY.down.slice(0, 7), "...keeeeeeeEk...", "...keeeeeeeEk...", "....kTTkkTTk....", "....kkkkkkkk...."],
  down2: [...BODY.down.slice(0, 7), "...keeeeeeeEk...", "...keeeeeeeEk...", "....kTTkkTTk....", "....kkkkkkkk...."],
  up: [...BODY.up.slice(0, 7), "...keeeeeeeEk...", "...keeeeeeeEk...", "....kTTkkTTk....", "....kkkkkkkk...."],
  up2: [...BODY.up.slice(0, 7), "...keeeeeeeEk...", "...keeeeeeeEk...", "....kTTkkTTk....", "....kkkkkkkk...."],
  side: [...BODY.side.slice(0, 7), "....keeeeeEk....", "....keeeeeEk....", ".....kTTTTk.....", ".....kkkkkk....."],
  side2: [...BODY.side.slice(0, 7), "....keeeeeEk....", "....keeeeeEk....", "....kTTkkTTk....", "....kkkkkkkk...."],
};

function person(head: Head, body: Body, palette: Palette): SpriteSrc {
  const frames: Record<string, string[]> = {
    down: [...head.down, ...body.down],
    down2: [...head.down, ...body.down2],
    up: [...head.up, ...body.up],
    up2: [...head.up, ...body.up2],
    side: [...head.side, ...body.side],
    side2: [...head.side, ...body.side2],
  };
  return check({ w: 16, h: 20, ax: 8, ay: 18, palette: { ...PAL, ...palette }, frames });
}

function check(s: SpriteSrc): SpriteSrc {
  for (const [name, rows] of Object.entries(s.frames)) {
    if (rows.length !== s.h || rows.some((r) => r.length !== s.w)) throw new Error(`tales units: frame ${name} is not ${s.w}x${s.h}: ${rows.map((r) => r.length).join(",")}`);
  }
  return s;
}

/** The same sprite with `patch` laid over the named frames at (x, y). "." leaves what was there. */
function holding(s: SpriteSrc, frames: Record<string, [number, number, string[]]>): SpriteSrc {
  const out: Record<string, string[]> = { ...s.frames };
  for (const [name, [x, y, patch]] of Object.entries(frames)) {
    for (const f of Object.keys(out).filter((k) => k === name || k === `${name}2`)) {
      const rows = out[f].map((r) => [...r]);
      patch.forEach((line, j) => [...line].forEach((ch, i) => {
        if (ch !== "." && rows[y + j] && rows[y + j][x + i] !== undefined) rows[y + j][x + i] = ch;
      }));
      out[f] = rows.map((r) => r.join(""));
    }
  }
  return check({ ...s, frames: out });
}

const SKIN = { s: "#e8b890", S: "#c08860" };
const SKIN_OLD = { s: "#dcae8c", S: "#a8785a" };
const SKIN_PALE = { s: "#d8c8bc", S: "#a8988c" };
const BOOTS = { T: "#3e2c20" };

// --- the tales' people -------------------------------------------------------------------------------

const hanney = person(HEAD_VEIL, BODY, { ...SKIN, ...BOOTS, h: "#8a6a48", H: "#5e4630", z: "#e8e0c8", Z: "#c4b89a", g: "#6a6a6a", W: "#c8c4b8", a: "#b8b08a", A: "#8a8462", v: "#d8d2b0", e: "#5a4a3a", E: "#3a2e24" });

// The man at the hives after the bell: the same veil, gone grey, and a coat the colour of the night.
const coram = person(HEAD_VEIL, BODY, { ...SKIN_PALE, ...BOOTS, h: "#8a8a8a", H: "#5e5e5e", z: "#a8a498", Z: "#7e7a70", g: "#3e3e44", W: "#8a8890", a: "#3a3e4a", A: "#262a34", v: "#4a4e5a", e: "#2e3038", E: "#1e2026" });

// Curlers under a net, and a quilted dressing gown to the ankle.
const pollardWife = holding(
  person(HEAD_BARE, GOWN, { ...SKIN, h: "#8a6a58", H: "#5e4a3e", a: "#c8889a", A: "#9a5e70", v: "#e0b0bc", e: "#c8889a", E: "#9a5e70", T: "#6a4a3a" }),
  { down: [4, 2, ["..p.p.p.", ".p.p.p.p"]], up: [4, 2, ["..p.p.p.", ".p.p.p.p", "..p.p.p."]], side: [4, 2, ["..p.p.", ".p.p.p"]] },
);

// Pyjamas under his overcoat, slippers, a little brass bell on his ankle.
const PYJAMAS = { ...SKIN_OLD, h: "#b8b4ac", H: "#8a867e", a: "#4a4438", A: "#2e2a22", v: "#a8c0d8", e: "#a8c0d8", E: "#7890a8", T: "#6a3a2a" };
const pollardBare = person(HEAD_BARE, BODY, PYJAMAS);
const bell: Record<string, [number, number, string[]]> = { down: [3, 18, ["y"]], up: [3, 18, ["y"]], side: [4, 18, ["y"]] };
const pollardStood = holding(pollardBare, bell);
// With the dinner: a plate under a white cloth, a blue rim, held in front of him.
const pollard = holding(pollardStood, {
  down: [4, 11, ["..kkkk..", ".kwwwWk.", "kBwwWWBk", ".kBBBBk."]],
  side: [8, 11, [".kkkk.", "kwwwWk", "BwwWWB", "kBBBBk"]],
});

const pargeter = person(HEAD_HAT, BODY, { ...SKIN_OLD, ...BOOTS, h: "#eeeae2", H: "#c4c0b8", z: "#ece2bc", Z: "#c8b88c", a: "#c89a3a", A: "#946e24", v: "#f0ece0", e: "#8a8a8a", E: "#5e5e5e" });

// The waller: a flat cap, a leather apron over his shirt, big hands.
const coker = person(HEAD_HAT, BODY, { ...SKIN, ...BOOTS, h: "#5a4a3a", H: "#3a2e24", z: "#6a5a48", Z: "#4a3e30", a: "#d8d0c0", A: "#aaa292", v: "#9a6a3a", e: "#6a5a42", E: "#483c2c", s: "#d8a07a" });

// Shirt-sleeves and braces, a claw hammer, and bare feet.
const rook = holding(
  person(HEAD_BARE, BODY, { ...SKIN, h: "#2a2420", H: "#1a1614", a: "#e8e4dc", A: "#b8b4ac", v: "#e8e4dc", e: "#3a3a44", E: "#24242c", T: "#e0b090", Q: "#3a2a24", d: "#3a3e4a" }),
  {
    down: [6, 9, ["Q..Q", "Q..Q", "Q..Q", "Q..Q", "Q..Q", "Q..Q"]],
    side: [11, 11, ["kggk", ".kd.", ".kd."]],
  },
);
// And the hammer in his right hand, where a person facing you holds it.
const rookHammer = holding(rook, { down: [0, 12, ["kggk", ".kd.", ".kd."]] });

// A nightdress to the ground, long wet hair down her back and over her shoulders, bare feet.
const mary = holding(
  person(HEAD_BARE, GOWN, { ...SKIN_PALE, h: "#1e1a20", H: "#121014", a: "#e8e6e0", A: "#bcbab4", v: "#f4f2ec", e: "#e8e6e0", E: "#bcbab4", T: "#d8c8bc" }),
  { down: [3, 9, ["kh.......hk", "kh.......hk", "kH.......Hk"]], up: [3, 9, ["khhhhhhhhhk", "kHhhhhhhhHk", ".kHhhhhhHk."]] },
);

// The Company's diver: the helmet, a canvas suit, lead boots.
const DIVER = { ...SKIN, y: "#d8a838", Y: "#9a7424", b: "#a8e0f8", x: "#1a2430", d: "#4a4e5a", a: "#9a9280", A: "#6e685a", v: "#aaa290", e: "#9a9280", E: "#6e685a", T: "#2e3038", h: "#6a6a6a", H: "#4a4a4a" };
const denholm = person(HEAD_HELMET, BODY, DIVER);
// The same suit with the helmet off: an old man's bare head, the hair wet and flat.
const denholmBare = person(HEAD_BARE, BODY, { ...DIVER, ...SKIN_PALE, h: "#9a9a9a", H: "#6e6e6e" });

// Mrs Wakes, sat in her rocking chair with the scarf in her lap. She does not get up.
const WAKES_ROWS = [
  "................",
  "......kkkk......",
  ".....kWWWWk.....",
  "..kk.kWWWWk.kk..",
  "..kt.kssssk.kt..",
  "..kt.kskskk.kt..",
  "..kt.kSssSk.kt..",
  "..ktkppppppkkt..",
  "..ktpppPPpppkt..",
  "..kpppPPPPpppk..",
  ".kkppsrrrrsppkk.",
  ".kTtgsrrrrsgtTk.",
  ".kTtrrrRrrrrtTk.",
  ".kTtrRrrrRrrtTk.",
  ".kkkkrrrrrrkkkk.",
  "..kek.rRrr.kek..",
  "..kek.rrr..kek..",
  ".kTkk..r...kkTk.",
  "kTTTTTTTTTTTTTTk",
  ".kkkkkkkkkkkkkk.",
];
const wakes = check({
  w: 16,
  h: 20,
  ax: 8,
  ay: 18,
  palette: { ...PAL, ...SKIN_OLD, W: "#e4e0d8", p: "#7a5a8a", P: "#523c60", r: "#c8403c", R: "#8a2a2a", g: "#b8b8c0", e: "#4a4050", T: "#8a5a34", t: "#6a4428" },
  frames: { down: WAKES_ROWS, up: WAKES_ROWS, side: WAKES_ROWS },
});

const voysey = person(HEAD_CAP, BODY, { ...SKIN_OLD, ...BOOTS, h: "#7a7a7a", H: "#5a5a5a", z: "#2a3450", Z: "#1a2034", y: "#e0b848", a: "#2e3854", A: "#1e2438", v: "#3e4a6a", e: "#2e3854", E: "#1e2438" });
// The watch chain across the waistcoat, and brass buttons.
const voyseyChained = holding(voysey, { down: [7, 10, ["y..", "...", "yyy", "..y", "y.."]] });

// A cloche hat, her good coat, and a suitcase with a label.
const voyseyWife = holding(
  person(HEAD_HAT, GOWN, { ...SKIN, h: "#6a3a2a", H: "#4a2a1e", z: "#6a2a3a", Z: "#4a1e28", a: "#3a5a4a", A: "#264034", v: "#4a6a5a", e: "#3a5a4a", E: "#264034", T: "#2a2020" }),
  {
    down: [11, 13, ["..k..", ".kkkk", "kqqqk", "kqwqk", "kqqqk", "kkkkk"]],
    side: [2, 13, ["..k..", ".kkk.", "kqqqk", "kqwqk", "kkkkk"]],
    up: [0, 13, ["..k..", ".kkkk", "kqqqk", "kqqqk", "kqqqk", "kkkkk"]],
  },
);
const WIFE_CASE = { q: "#8a5a34" };

// The vermin man: a long coat to the knee, a flat cap, and his pole over his shoulder with his catch on it.
const sorrell = holding(
  person(HEAD_HAT, BODY, { ...SKIN, ...BOOTS, h: "#4a3a2a", H: "#302418", z: "#5a5040", Z: "#3e362a", a: "#6a5236", A: "#4a3822", v: "#7a6040", e: "#6a5236", E: "#4a3822", x: "#2a2228" }),
  {
    down: [11, 0, ["....k", "...kt", "...kt", "..kkt", "..xkt", "..x.t", "...kt", "..kt.", "..kt.", "..kt.", ".kt..", ".kt..", ".ks.."]],
    side: [10, 0, ["....k", "...kt", "...kt", "..kkt", "..xkt", "..x.t", "...kt", "..kt.", "..kt.", "..kt.", ".kt..", ".kt..", "ks..."]],
  },
);

const sheet: SpriteSheet = {
  tale_hanney: hanney,
  tale_coram: coram,
  tale_pollard_wife: pollardWife,
  tale_pollard: pollard,
  tale_pollard_stood: pollardStood,
  tale_pargeter: pargeter,
  tale_coker: coker,
  tale_rook: rookHammer,
  tale_mary: mary,
  tale_denholm: denholm,
  tale_denholm_bare: denholmBare,
  tale_wakes: wakes,
  tale_voysey: voyseyChained,
  tale_voysey_wife: { ...voyseyWife, palette: { ...voyseyWife.palette, ...WIFE_CASE } },
  tale_sorrell: sorrell,
};

export default sheet;
