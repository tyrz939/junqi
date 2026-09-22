// Castle's townsfolk, and the animals that live among them. One hand with Jane: 16 x 20, feet on
// (8, 18), 1 px outline, light from the top-left. A person is a HEAD and a BODY drawn once each
// and put together, then dressed in a palette, so thirty people are seven heads, six bodies and
// thirty wardrobes rather than thirty drawings that slowly stop agreeing with each other.
//
// Palette characters a person may use (on top of PAL):
//   h H  hair and its highlight      c C q  coat or dress, its shade, its highlight
//   a A  hat, cap, scarf or apron    d D    trousers        z  shoes      x  cheek
//
// Import PAL and types only: `@/art/units` merges this file, so importing it here is a cycle.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

type Rows = string[];

// --- heads: 9 rows, facing down ----------------------------------------------------------

const HEADS: Record<string, Rows> = {
  short: [
    "................",
    ".....kkkkkk.....",
    "....khHHHhhk....",
    "...khHhhhhhhk...",
    "...khhhhhhhhk...",
    "...khsssssshk...",
    "...ksskssksSk...",
    "...ksssssssSk...",
    "....kksssskk....",
  ],
  bun: [
    "......kkkk......",
    ".....khHHhk.....",
    "....kkhhhhkk....",
    "...khHhhhhhhk...",
    "...khhhsshhhk...",
    "...khsssssshk...",
    "...khsksskshk...",
    "...khxssssxhk...",
    "....kksssskk....",
  ],
  long: [
    ".....kkkkkk.....",
    "....khHHhhhk....",
    "...khHHhhhhhk...",
    "...khHhhhhhhk...",
    "...khhhsshhhk...",
    "...khsksskshk...",
    "...khsssssshk...",
    "...khxssssxhk...",
    "...khhsssshhk...",
  ],
  cap: [
    "................",
    ".....kkkkkk.....",
    "....kaaaaaAk....",
    "...kaaaaaaaAk...",
    "..kkkkkkkkkkkk..",
    "...khsssssshk...",
    "...ksskssksSk...",
    "...ksssssssSk...",
    "....kksssskk....",
  ],
  bald: [
    "................",
    ".....kkkkkk.....",
    "....ksssssSk....",
    "...ksssssssSk...",
    "...khsssssShk...",
    "...khsksskshk...",
    "...khsWWWWshk...",
    "...khsssssshk...",
    "....kksssskk....",
  ],
  hat: [
    "......kkkk......",
    ".....kaAAak.....",
    "....kaAAAAak....",
    "..kkaaaaaaaakk..",
    "..kkkkkkkkkkkk..",
    "...khsssssshk...",
    "...khsksskshk...",
    "...ksssssssSk...",
    "....kksssskk....",
  ],
  scarf: [
    ".....kkkkkk.....",
    "....kaaaaaAk....",
    "...kaaaaaaaAk...",
    "...kaAaaaaaAk...",
    "...kasssssSak...",
    "...kaskssksak...",
    "...kasssssSak...",
    "...kaaSsssaak...",
    "....kkaaaakk....",
  ],
  helmet: [
    "......kkkk......",
    ".....kaaaAk.....",
    "....kaaayaAk....",
    "...kaaaaaaaAk...",
    "..kkkkkkkkkkkk..",
    "...khsssssshk...",
    "...ksskssksSk...",
    "...ksssssssSk...",
    "....kkskkskk....",
  ],
  pigtails: [
    ".....kkkkkk.....",
    "....khHHhhhk....",
    "...khHhhhhhhk...",
    "..kkhhhsshhhkk..",
    ".khkhsssssshkhk.",
    ".khkhsksskshkhk.",
    ".kakhxssssxhkak.",
    "...khhsssshhk...",
    "....kksssskk....",
  ],
};

/** What the back of each head is made of, where the face was. */
const BACK: Record<string, string> = { short: "h", bun: "h", long: "h", cap: "h", bald: "h", hat: "h", scarf: "a", helmet: "h", pigtails: "h" };
const FACE = new Set(["s", "S", "x", "W"]);

// --- bodies: 10 rows below the head, facing down; and the same body side-on --------------

const BODIES: Record<string, { down: Rows; side: Rows }> = {
  coat: {
    down: [
      "...kkcqwwqckk...",
      "..kqcccwwcccCk..",
      "..kqcccwwcccCk..",
      "..kqccccyccCCk..",
      "..kskccccccksk..",
      "...kcccccccCk...",
      "....kddkkdDk....",
      "....kddkkdDk....",
      "....kzzkkzzk....",
      "....kkkkkkkk....",
    ],
    side: [
      "....kkcwcck.....",
      "....kqcccCk.....",
      "....kqcccCk.....",
      "....kqcccCk.....",
      "....kqcsCCk.....",
      "....kcccCCk.....",
      ".....kddDk......",
      ".....kddDk......",
      ".....kzzzzk.....",
      ".....kkkkkk.....",
    ],
  },
  dress: {
    down: [
      "...kkcwwwwckk...",
      "..kqccccccccCk..",
      "..kqccccccccCk..",
      "..kskcccccCksk..",
      "...kcccccccCk...",
      "..kcccccccccCk..",
      "..kcccccccccCk..",
      "..kkkkkkkkkkkk..",
      ".....kzkkzk.....",
      ".....kkkkkk.....",
    ],
    side: [
      "....kkcwcck.....",
      "....kqcccCk.....",
      "....kqcccCk.....",
      "....kqcsCCk.....",
      "...kqcccCCCk....",
      "...kccccCCCk....",
      "...kccccCCCk....",
      "...kkkkkkkkk....",
      ".....kzzzk......",
      ".....kkkkk......",
    ],
  },
  apron: {
    down: [
      "...kkcwwwwckk...",
      "..kqcaaaaaacCk..",
      "..kqcaaaaaacCk..",
      "..kskaaaaaaksk..",
      "...kcaaaaaaCk...",
      "..kccaaaaaacCk..",
      "..kccaaaaaacCk..",
      "..kkkkkkkkkkkk..",
      ".....kzkkzk.....",
      ".....kkkkkk.....",
    ],
    side: [
      "....kkcwcck.....",
      "....kqcaaCk.....",
      "....kqcaaCk.....",
      "....kqcsaCk.....",
      "...kqcccaaCk....",
      "...kccccaaCk....",
      "...kccccaaCk....",
      "...kkkkkkkkk....",
      ".....kzzzk......",
      ".....kkkkk......",
    ],
  },
  smock: {
    down: [
      "...kkcqwwqckk...",
      "..kqcaaaaaacCk..",
      "..kqcaaaaaacCk..",
      "..kqcaaaaaacCk..",
      "..kskaaaaaaksk..",
      "...kcaaaaaaCk...",
      "....kddkkdDk....",
      "....kddkkdDk....",
      "....kzzkkzzk....",
      "....kkkkkkkk....",
    ],
    side: [
      "....kkcwcck.....",
      "....kqcaaCk.....",
      "....kqcaaCk.....",
      "....kqcaaCk.....",
      "....kqcsaCk.....",
      "....kccaaCk.....",
      ".....kddDk......",
      ".....kddDk......",
      ".....kzzzzk.....",
      ".....kkkkkk.....",
    ],
  },
  child: {
    down: [
      "....kkcwwckk....",
      "...kqccccccCk...",
      "...kskccccksk...",
      "....kccccCCk....",
      ".....kdkkdk.....",
      ".....kkkkkk.....",
    ],
    side: [
      "....kkcwck......",
      "....kqccCk......",
      "....kqcsCk......",
      "....kcccCk......",
      ".....kdDk.......",
      ".....kkkk.......",
    ],
  },
};

function up(head: string, rows: Rows): Rows {
  const back = BACK[head];
  return rows.map((r, y) => {
    if (y < 4) return r;
    const cs = r.split("");
    for (let x = 4; x <= 11; x++) if (FACE.has(cs[x]) || (cs[x] === "k" && y >= 5 && y <= 7)) cs[x] = back;
    return cs.join("");
  });
}

/** Side-on: the back half of the face is hair, the eye moves forward, and a nose sticks out. */
function side(head: string, rows: Rows): Rows {
  const back = BACK[head];
  let eyeRow = -1;
  const out = rows.map((r, y) => {
    const cs = r.split("");
    if (y >= 4 && cs.some((c) => FACE.has(c))) {
      for (let x = 4; x <= 7; x++) if (FACE.has(cs[x]) || cs[x] === "k") cs[x] = back;
      if (cs[9] === "k" && y <= 7) {
        eyeRow = y;
        cs[9] = "s";
        cs[10] = "k";
      }
    }
    return cs.join("");
  });
  if (eyeRow >= 0 && eyeRow + 1 < out.length) {
    const cs = out[eyeRow + 1].split("");
    if (cs[12] === "k") {
      cs[12] = "s";
      cs[13] = "k";
    }
    out[eyeRow + 1] = cs.join("");
  }
  return out;
}

function bodyUp(rows: Rows): Rows {
  return rows.map((r) => r.replace(/[wy]/g, "c").replace(/a/g, "c"));
}

/** The walking frame: the whole figure one pixel lower, as Jane's own second frame is. */
function bob(rows: Rows): Rows {
  return [".".repeat(rows[0].length), ...rows.slice(0, rows.length - 1)];
}

const BLANK = "................";

function person(head: string, body: string, palette: Palette): SpriteSrc {
  const h = HEADS[head];
  const b = BODIES[body];
  const small = body === "child";
  const pad = small ? [BLANK, BLANK, BLANK, BLANK] : [];
  const frame = (hr: Rows, br: Rows): Rows => [...pad, ...hr, ...br, BLANK];
  const down = frame(h, b.down);
  const upF = frame(up(head, h), bodyUp(b.down));
  const sideF = frame(side(head, h), b.side);
  return { w: 16, h: 20, ax: 8, ay: 18, palette, frames: { down, down2: bob(down), up: upF, up2: bob(upF), side: sideF, side2: bob(sideF) } };
}

type Wardrobe = { h: string; H: string; c: string; C: string; q: string; a?: string; A?: string; d?: string; D?: string; z?: string };

function dress(w: Wardrobe): Palette {
  return {
    ...PAL,
    h: w.h,
    H: w.H,
    c: w.c,
    C: w.C,
    q: w.q,
    a: w.a ?? "#e8e0d0",
    A: w.A ?? "#b8b0a0",
    d: w.d ?? "#4a4438",
    D: w.D ?? "#2e2a24",
    z: w.z ?? "#3a2a20",
    x: "#e89c88",
  };
}

// Hair.
const GREY = { h: "#a8a4a0", H: "#d4d0cc" };
const WHITE = { h: "#dcd8d0", H: "#f4f0e6" };
const BROWN = { h: "#6a4a30", H: "#8e6a48" };
const DARK = { h: "#2e2630", H: "#4e4450" };
const FAIR = { h: "#c89a50", H: "#e8c070" };
const RED = { h: "#a0482c", H: "#c86a44" };

// Cloth.
const NAVY = { c: "#2e3a5c", C: "#1e2640", q: "#4a5a84" };
const BLACK = { c: "#2a2830", C: "#18161c", q: "#44424c" };
const TWEED = { c: "#7a6a4a", C: "#54482e", q: "#9c8c68" };
const MOSS = { c: "#5a6e3a", C: "#3c4c24", q: "#7c925a" };
const ROSE = { c: "#a8606a", C: "#7a4048", q: "#c8848c" };
const PLUMB = { c: "#6a4a6e", C: "#48304c", q: "#8c6a90" };
const SKY = { c: "#5a86a8", C: "#3c6080", q: "#80a8c8" };
const MUSTARD = { c: "#b08a38", C: "#806224", q: "#d0aa58" };
const BRICK = { c: "#9a5038", C: "#6c3424", q: "#bc7050" };
const GREYC = { c: "#7a7c80", C: "#56585c", q: "#9a9ca0" };
const CREAM = { c: "#d8ccb0", C: "#aca088", q: "#ece4d0" };
const TEAL = { c: "#3c7a78", C: "#285452", q: "#5c9c98" };

const sheet: SpriteSheet = {
  // The square.
  town_tilly: person("pigtails", "child", dress({ ...FAIR, ...ROSE, a: "#e05060" })),
  town_robin: person("short", "child", dress({ ...BROWN, ...SKY, d: "#5a4a38" })),
  town_nell: person("pigtails", "child", dress({ ...DARK, ...MUSTARD, a: "#58a8e8" })),
  town_grocer: person("cap", "apron", dress({ ...BROWN, ...MOSS, a: "#e8e4d8", A: "#b8b4a8" })),
  town_seedwoman: person("hat", "dress", dress({ ...GREY, ...PLUMB, a: "#6a5a40", A: "#4a3c28" })),
  town_baker: person("bun", "apron", dress({ ...FAIR, ...CREAM, a: "#f4f0e6", A: "#cfc8b8", c: "#c8a878", C: "#9c7c54", q: "#e0c498" })),
  town_shopper: person("hat", "dress", dress({ ...BROWN, ...TEAL, a: "#a84838", A: "#7a3024" })),
  town_old_man: person("bald", "coat", dress({ ...WHITE, ...TWEED })),
  town_sweeper: person("cap", "coat", dress({ ...DARK, ...GREYC, a: "#4a4c50", A: "#34363a" })),
  town_constable: person("helmet", "coat", dress({ ...DARK, ...NAVY, a: "#26304c", A: "#161c30", d: "#1e2640", D: "#141a2c", z: "#141018" })),
  town_postmistress: person("bun", "dress", dress({ ...GREY, ...NAVY, q: "#a84838" })),
  town_vicar: person("bald", "coat", dress({ ...GREY, ...BLACK, d: "#1e1c22", D: "#141216" })),
  town_gossip_a: person("scarf", "dress", dress({ ...GREY, ...BRICK, a: "#7c6a8c", A: "#5a4a68" })),
  town_gossip_b: person("hat", "dress", dress({ ...WHITE, ...SKY, a: "#3c4c24", A: "#2a3618" })),
  // The lanes and gardens.
  town_washerwoman: person("scarf", "apron", dress({ ...RED, ...SKY, a: "#e8e4d8", A: "#b8b4a8" })),
  town_gardener: person("hat", "coat", dress({ ...GREY, ...MOSS, a: "#b89a58", A: "#8a7038" })),
  town_painter: person("cap", "coat", dress({ ...RED, ...CREAM, a: "#e8e4d8", A: "#b8b4a8", d: "#d8ccb0", D: "#aca088" })),
  town_smith: person("short", "smock", dress({ ...DARK, ...GREYC, a: "#5a3a26", A: "#3c2618" })),
  town_drinker: person("cap", "coat", dress({ ...BROWN, ...BRICK, a: "#54482e", A: "#3a3020" })),
  town_doctor: person("short", "coat", dress({ ...GREY, ...GREYC, d: "#3a3c40", D: "#26282c" })),
  town_courting_a: person("short", "coat", dress({ ...FAIR, ...NAVY })),
  town_courting_b: person("long", "dress", dress({ ...DARK, ...ROSE })),
  town_widow: person("scarf", "dress", dress({ ...WHITE, ...BLACK, a: "#44424c", A: "#2a2830" })),
  town_milkman: person("cap", "coat", dress({ ...FAIR, ...CREAM, a: "#e8e4d8", A: "#b8b4a8", d: "#4a5a84", D: "#2e3a5c" })),
  town_caller: person("short", "coat", dress({ ...RED, ...MUSTARD })),
  town_dot: person("pigtails", "child", dress({ ...RED, ...TEAL, a: "#f0d048" })),
  town_reader: person("short", "coat", dress({ ...BROWN, ...TWEED, q: "#b8a878" })),
  town_waterer: person("bun", "apron", dress({ ...WHITE, ...MOSS, a: "#e8e4d8", A: "#b8b4a8" })),
  // Inside the Arms, and the Halt.
  town_landlady: person("bun", "apron", dress({ ...RED, ...PLUMB, a: "#e8e4d8", A: "#b8b4a8" })),
  town_regular: person("bald", "coat", dress({ ...GREY, ...MOSS })),
  town_traveller: person("hat", "dress", dress({ ...BROWN, ...GREYC, a: "#2e3a5c", A: "#1e2640" })),
  // The farm.
  town_farmhand: person("cap", "smock", dress({ ...BROWN, ...TWEED, a: "#8a7a5a", A: "#6a5a3c", d: "#5a4a38" })),
};

// --- animals ------------------------------------------------------------------------------

function beast(w: number, h: number, palette: Palette, down: Rows, sideF: Rows, upF: Rows): SpriteSrc {
  return { w, h, ax: Math.floor(w / 2), ay: h - 1, palette, frames: { down, down2: bob(down), up: upF, up2: bob(upF), side: sideF, side2: bob(sideF) } };
}

const CAT_DOWN = [
  "............",
  "..k......k..",
  ".kck....kck.",
  ".kcckkkkcck.",
  ".kcycccyyck.",
  ".kcccaaccck.",
  "..kcccccck..",
  ".kcccccccck.",
  ".kcCccccCck.",
  ".kkkkkkkkkk.",
];
const CAT_UP = CAT_DOWN.map((r) => r.replace(/[ya]/g, "c"));
const CAT_SIDE = [
  "............",
  "........k.k.",
  ".......kccck",
  "k......kcyck",
  "kc.....kccck",
  ".kckkkkkcck.",
  "..kcccccccck",
  "..kccccccck.",
  "..kck..kck..",
  "..kkk..kkk..",
];
// Sixpence: black, with the eyes of a cat who has been out since Sunday.
sheet.town_cat_black = beast(12, 10, { ...PAL, c: "#2a2530", C: "#141018", a: "#c87080" }, CAT_DOWN, CAT_SIDE, CAT_UP);
sheet.town_cat_ginger = beast(12, 10, { ...PAL, c: "#d88838", C: "#a05a20", a: "#e8a0a0" }, CAT_DOWN, CAT_SIDE, CAT_UP);

const HEN_DOWN = [
  "....rr....",
  "...kwwk...",
  "...kyyk...",
  "..kwwwwk..",
  ".kwwwwwwk.",
  ".kwwwwwWk.",
  ".kwwwwWWk.",
  "..kWWWWk..",
  "...y..y...",
  "..yy..yy..",
];
const HEN_UP = HEN_DOWN.map((r, y) => (y === 2 ? "...kwwk..." : r));
const HEN_SIDE = [
  "......rr..",
  ".....kwwk.",
  ".....kwkyy",
  "k....kwwk.",
  "kwk.kwwwk.",
  "kwwkwwwWk.",
  ".kwwwwWWk.",
  "..kWWWWk..",
  "....yky...",
  "...yy.yy..",
];
sheet.town_hen = beast(10, 10, PAL, HEN_DOWN, HEN_SIDE, HEN_UP);
sheet.town_hen_brown = beast(10, 10, { ...PAL, w: "#b87848", W: "#8a5430" }, HEN_DOWN, HEN_SIDE, HEN_UP);

const SHEEP_DOWN = [
  "................",
  "....kkkkkkkk....",
  "...kwwwwwwwwk...",
  "..kwwkGGGGkwwk..",
  "..kwkGkGGkGkwk..",
  "..kwwkGGGGkwWk..",
  "..kwwwkGGkwwWk..",
  "..kWwwwkkwwWWk..",
  "...kWWWWWWWWk...",
  "....kGk..kGk....",
  "....kGk..kGk....",
  "....kkk..kkk....",
];
const SHEEP_UP = [
  "................",
  "....kkkkkkkk....",
  "...kwwwwwwwwk...",
  "..kwwwwwwwwwwk..",
  "..kwwwwwwwwwwk..",
  "..kwwwwwWwwwWk..",
  "..kwwWwwwwwwWk..",
  "..kWwwwwwwwWWk..",
  "...kWWWWWWWWk...",
  "....kGk..kGk....",
  "....kGk..kGk....",
  "....kkk..kkk....",
];
const SHEEP_SIDE = [
  "................",
  "...kkkkkkk......",
  "..kwwwwwwwkkkk..",
  ".kwwwwWwwwwkGGk.",
  ".kwwwwwwwwWkGkGk",
  ".kwWwwwwwwwkGGGk",
  ".kwwwwwwWwwWkkk.",
  "..kWwwWwwwWWk...",
  "...kkkkkkkkk....",
  "...kGk...kGk....",
  "...kGk...kGk....",
  "...kkk...kkk....",
];
sheet.town_sheep = beast(16, 12, { ...PAL, G: "#3a3438" }, SHEEP_DOWN, SHEEP_SIDE, SHEEP_UP);

export default sheet;
