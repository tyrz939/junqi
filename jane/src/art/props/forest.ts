// Butterfly Forest's furniture. Everything here is either a thing that grows when the light
// reaches it, or the light itself. Props that answer a verb have an `on` frame, which is the
// same thing after: the bud open, the bank green, the bridge mended, the lamp alight.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  q: "#fff0bc", // sunlight
  Q: "#ffe08a", // sunlight, deeper
  c: "#b4d4ff", // moonlight
  C: "#7fa8e0", // moonlight, deeper
  v: "#6a4a30", // dry earth
  f: "#f4a0c0", // petal
  F: "#c06890", // petal shade
  h: "#2c6a38", // hedge dark
  x: "#0c0a12", // a hole in something
  A: "#8a8078", // ash on a burnt branch
  a: "#c08050", // an ember still alive in the char
  // Contact shadow, translucent so a plant sits on whatever it is standing on.
  "-": "#00000030",
  "=": "#00000050",
};

/** footH is the footprint height in 8 px cells. */
function prop(w: number, h: number, footH: number, frames: Record<string, string[]>): SpriteSrc {
  return { w, h, ax: 0, ay: h - footH * 8, palette: PX, frames };
}

const sheet: SpriteSheet = {
  // A closed bud on a leaning stem, two leaves at the foot. Black outlines ate an 8x8
  // sprite alive, so the edge is the darkest green instead.
  bud: prop(8, 8, 1, {
    base: [
      "..lln...",
      ".llnhN..",
      ".llnhN..",
      ".lnnhN..",
      "...nh...",
      "ln.n.nl.",
      ".lNnhn..",
      "..-==-..",
    ],
    // Open, and it is the light: five petals round a pale centre, which is what the
    // prop's warm little lamp is shining out of.
    on: [
      "..f..f..",
      ".ffFfFf.",
      "fFfwqfFf",
      ".FfqyfF.",
      "..NnhN..",
      "ln.n.nl.",
      ".lNnhn..",
      "..-==-..",
    ],
  }),

  // Half buried in turned earth, the way a thing waiting to be grown should look.
  hedge_seed: prop(8, 8, 1, {
    base: [
      "........",
      "........",
      "..TTT...",
      ".TmmtT..",
      ".TmttTT.",
      "..TttT..",
      ".e==ee..",
      "..-=-...",
    ],
    on: [
      "...l....",
      "..lnl...",
      ".lnhlnl.",
      "..lnh...",
      "..TnTT..",
      "..TttT..",
      ".e==ee..",
      "..-=-...",
    ],
  }),

  // A dead root lying across the path. It was one brown lump; now it is knuckled, lit
  // along the top and sunk in its own shadow.
  vine_root: prop(16, 8, 1, {
    base: [
      "................",
      "................",
      "....TTT....TT...",
      "..TTmmTTTTTmtT..",
      ".TmmtvvTmmtvvTT.",
      "TtvvTTvvTtvTTvvT",
      ".=vv==vv=v==vv=.",
      "..-=====-==-=-..",
    ],
    on: [
      "..l....l...ln...",
      ".lnl..lnl.lnh...",
      "..TTThTTTThmtT..",
      "..TTmmTTTTTmtT..",
      ".TmmtnnTmmtnnTT.",
      "TtnnTTnnTtnTTnnT",
      ".=nn==nn=n==nn=.",
      "..-=====-==-=-..",
    ],
  }),

  summon_stone: prop(16, 16, 2, {
    base: [
      "................",
      "......kkkk......",
      ".....kggggk.....",
      "....kgggggGk....",
      "....kggwggGk....",
      "....kgwgwgGk....",
      "....kggwggGk....",
      "....kgggggGk....",
      "...kggwwwggGk...",
      "...kgggggggGk...",
      "...kggggggGGk...",
      "..kggggggggGGk..",
      "..kGGGGGGGGGGk..",
      "..kkkkkkkkkkkk..",
      "...eeeeeeeeee...",
      "................",
    ],
  }),

  forest_sunbeam: prop(16, 16, 2, {
    base: [
      "....qqqqqqqq....",
      "..qqQQQQQQQQqq..",
      ".qQQQQQQQQQQQQq.",
      "qQQQQQQQQQQQQQQq",
      "qQQQQQQQQQQQQQQq",
      "qQQQQQQQQQQQQQQq",
      "qQQQQQQQQQQQQQQq",
      "qQQQQQQQQQQQQQQq",
      "qQQQQQQQQQQQQQQq",
      "qQQQQQQQQQQQQQQq",
      "qQQQQQQQQQQQQQQq",
      "qQQQQQQQQQQQQQQq",
      ".qQQQQQQQQQQQQq.",
      "..qqQQQQQQQQqq..",
      "....qqqqqqqq....",
      "................",
    ],
  }),

  forest_moonbeam: prop(16, 16, 2, {
    base: [
      ".....cccccc.....",
      "...ccCCCCCCcc...",
      "..cCCCCCCCCCCc..",
      ".cCCCCCCCCCCCCc.",
      ".cCCCCCCCCCCCCc.",
      "cCCCCCCCCCCCCCCc",
      "cCCCCCCCCCCCCCCc",
      "cCCCCCCCCCCCCCCc",
      "cCCCCCCCCCCCCCCc",
      "cCCCCCCCCCCCCCCc",
      "cCCCCCCCCCCCCCCc",
      ".cCCCCCCCCCCCCc.",
      ".cCCCCCCCCCCCCc.",
      "..cCCCCCCCCCCc..",
      "...ccCCCCCCcc...",
      ".....cccccc.....",
    ],
  }),

  forest_dry_bank: prop(24, 16, 2, {
    base: [
      "........................",
      "........................",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kvvvvvvvvvvvvvvvvvvvvvvk",
      "kvvevvvvevvvvvvevvvvevvk",
      "kvvvvvvvvvvevvvvvvvvvvvk",
      "kevvvvevvvvvvvvvvevvvvek",
      "kvvvvvvvvvvvvvvvvvvvvvvk",
      "kvvvvevvvvvvvevvvvvvvvvk",
      "kvvvvvvvvvvvvvvvvvvvvvvk",
      "kevvvvvvvevvvvvvvvvevvvk",
      "kvvvvvvvvvvvvvvvvvvvvvvk",
      "kvvvvvvvvvvvvvvvvvvvvvvk",
      "keeeeeeeeeeeeeeeeeeeeeek",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "........................",
    ],
    on: [
      "..k...k....k....k...k...",
      ".klk.klk..klk..klk.klk..",
      "kknkkknkkkknkkkknkkknkkk",
      "klnnlnnllnnllnnllnnllnnk",
      "knnlnnllnnllnnllnnllnnnk",
      "klnnllnnllnnllnnllnnlnnk",
      "knnllnnllnnllnnllnnllnnk",
      "klnnllnnllnnllnnllnnlnnk",
      "knnllnnllnnllnnllnnllnnk",
      "klnnllnnllnnllnnllnnlnnk",
      "knnllnnllnnllnnllnnllnnk",
      "kNNNNNNNNNNNNNNNNNNNNNNk",
      "kNNNNNNNNNNNNNNNNNNNNNNk",
      "keeeeeeeeeeeeeeeeeeeeeek",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "........................",
    ],
  }),

  forest_rockfall: prop(24, 16, 2, {
    base: [
      "........................",
      "....kkkk......kkkk......",
      "...kggGk....kkgggkk.....",
      "..kgggGGk..kgggggGGk....",
      ".kggggGGGkkggggggGGGk...",
      ".kgggGGGGkkgggGGGGGGk...",
      "kkggGGGGGkkggGGGGGGGkk..",
      "kggggGGGGkkgGGGGGGGGGk..",
      "kgggGGGGkkkkGGGGGGGGGkk.",
      "kkgGGGGkkggkkGGGGGGGGGk.",
      ".kkGGGkkgggGkkGGGGGGGGk.",
      "..kkkkkgggGGGkkGGGGGGGk.",
      "....kkkgGGGGGGkkGGGGGkk.",
      ".....kkkGGGGGGGkkGGGkk..",
      "......kkkkkkkkkkkkkkk...",
      "........................",
    ],
  }),

  forest_footbridge: prop(24, 16, 2, {
    base: [
      "........................",
      "kk....................kk",
      "kTk..................kTk",
      "kTkkkkkkkk....kkkkkkkkTk",
      "kTtttttttk....ktttttttTk",
      "kkkkkkkkkk....kkkkkkkkkk",
      "kmmmmmmmk......kmmmmmmmk",
      "kTmmmmmTk......kTmmmmmTk",
      "kmmmmmmmk......kmmmmmmmk",
      "kTmmmmmTk......kTmmmmmTk",
      "kmmmmmmmk......kmmmmmmmk",
      "kkkkkkkkk......kkkkkkkkk",
      "kTk...................Tk",
      "kTk...................Tk",
      "kk.....................k",
      "........................",
    ],
    on: [
      "........................",
      "kk....................kk",
      "kTk..................kTk",
      "kTkkkkkkkkkkkkkkkkkkkkTk",
      "kTttttttttttttttttttttTk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kmmmmmmmmmmmmmmmmmmmmmmk",
      "kTmmmmmTmmmmmmTmmmmmmTmk",
      "kmmmmmmmmmmmmmmmmmmmmmmk",
      "kTmmmmmTmmmmmmTmmmmmmTmk",
      "kmmmmmmmmmmmmmmmmmmmmmmk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kTk...................Tk",
      "kTk...................Tk",
      "kk.....................k",
      "........................",
    ],
  }),

  forest_lamp_dead: prop(24, 24, 2, {
    base: [
      "........................",
      "........................",
      "........................",
      "........................",
      "...............kkkk.....",
      "..............kGxxGk....",
      "..............kxxxxk....",
      "..............kGxxGk....",
      "...............kkkk.....",
      "................kk......",
      "...............kGGk.....",
      "...........kkkkkGGk.....",
      "........kkkGGGGGGGk.....",
      "....kkkkGGGGkkkkkkk.....",
      "..kkGGGGkkkk............",
      ".kGGGkkk................",
      ".kkkk...................",
      "........................",
      "..eeee..................",
      ".eeeeeee................",
      "........................",
      "........................",
      "........................",
      "........................",
    ],
    on: [
      "........................",
      "........................",
      "........................",
      "........................",
      "...............kkkk.....",
      "..............kGqqGk....",
      "..............kqyyqk....",
      "..............kGqqGk....",
      "...............kkkk.....",
      "................kk......",
      "...............kGGk.....",
      "...........kkkkkGGk.....",
      "........kkkGGGGGGGk.....",
      "....kkkkGGGGkkkkkkk.....",
      "..kkGGGGkkkk............",
      ".kGGGkkk................",
      ".kkkk...................",
      "........................",
      "..eeee..................",
      ".eeeeeee................",
      "........................",
      "........................",
      "........................",
      "........................",
    ],
  }),

  // The silhouette was always right; it was flat. Every limb now has a lit side and a
  // dark side, the trunk turns from light to shade across its width, and the whole thing
  // stands in a shadow instead of on nothing.
  dry_tree: prop(24, 24, 2, {
    base: [
      "........................",
      ".....k.........k........",
      "....ktk.......ktk.......",
      "...ktTk.k.k..ktTk.......",
      "...ktk.ktktk.ktk....k...",
      "....kTkktktkkTk....ktk..",
      ".....ktTTkttTk....ktTk..",
      "......ktTktTk....ktTk...",
      ".......ktTTk....ktTk....",
      "........ktTk..kktTk.....",
      "........ktTkkktTTk......",
      ".........ktTTTTvk.......",
      ".........ktTTTvk........",
      ".........ktTTvk.........",
      ".........kmtTvk.........",
      ".........kmtTvk.........",
      ".........kmtTvk.........",
      ".........kmtTvk.........",
      "........kmttTvvk........",
      ".......kmmttTvvvk.......",
      "......kkmttTTvvvkk......",
      ".....keekkkkkkkkeek.....",
      "....keeeeeeeeeeeeeek....",
      "....================....",
    ],
    // Answered with fire: charred, not deleted. Ash sits on the lit side of each limb
    // and two embers are still going at the root.
    on: [
      "........................",
      ".....k.........k........",
      "....kKk.......kKk.......",
      "...kKKk.k.k..kKKk.......",
      "...kKk.kKkKk.kKk....k...",
      "....kKkkKkKkkKk....kKk..",
      ".....kKKKkKKKk....kKKk..",
      "......kKKkKKk....kKKk...",
      ".......kKKKk....kKKk....",
      "........kKKk..kkKKk.....",
      "........kKKkkkKKKk......",
      ".........kAKKKKkk.......",
      ".........kAKKKkk........",
      ".........kAKKkk.........",
      ".........kAKKkk.........",
      ".........kAKKkk.........",
      ".........kAKKkk.........",
      ".........kAKKkk.........",
      "........kAKKKKkk........",
      ".......kAAKKaKkkk.......",
      "......kkAAKaaKKkkk......",
      ".....keekkkkkkkkeek.....",
      "....keeeeeeeeeeeeeek....",
      "....================....",
    ],
  }),

  forest_cases: prop(16, 16, 2, {
    base: [
      "kkkkkkkkkkkkkkkk",
      "kTTTTTTTTTTTTTTk",
      "kTkkkkkkkkkkkkTk",
      "kTkbbbkkbbbkkbTk",
      "kTkbfbkkbFbkkbTk",
      "kTkbbbkkbbbkkbTk",
      "kTkkkkkkkkkkkkTk",
      "kTwwwkkwwwkkwwTk",
      "kTkkkkkkkkkkkkTk",
      "kTkbbbkkbbbkkbTk",
      "kTkbFbkkbfbkkbTk",
      "kTkbbbkkbbbkkbTk",
      "kTkkkkkkkkkkkkTk",
      "kTwwwkkwwwkkwwTk",
      "kTTTTTTTTTTTTTTk",
      "kkkkkkkkkkkkkkkk",
    ],
  }),
};

export default sheet;
