# Chunks: the county's set places

One file per place the skeleton positions and the county stamps whole: the halt, Julie's yard, the
town, the farm, the mine mouth, the graveyard, the burial stair, the car in the wood, the two
outposts and the six landmarks (PORT.md §6.f). They were code in `jane/src/world/chunks.ts`; they
are grids now, and the county reads only two things from them: their **gates** and their
**slots**. Everything else is what they draw.

The full grammar is the doc comment at the top of
`crates/jane-schema/src/compile/tables/chunks/parse.rs`. In short:

```
// comments, in the header and the legend
id      farm              the site it stands at (sites.json); the file is named for it
box     60x44             cells; every coordinate below is in box cells, (0, 0) top-left
anchor  centre            the box cell on the site's origin (`centre`, or `X Y`)
pin     w 4               optional: a side held at a county coordinate (the halt on the line)
face    w                 the side her way in looks out of; some gate is on it
gates   w22, e22          a walkable cell one step outside the box: side, then cell along it
slots   farm_door 10 11   optional: named cells inside the box a placement row or a door takes
around  name n30 e30 s30 w4   optional: a rect of the box grown, the only ground outside it named
grid      the ground, one tile per cell, every cell
things    optional: props (a block of one character the size of the def) and units
names     optional: marks (one cell), rects (two corners), fill regions (any cells)
claims    optional: x where nothing placed later by name may stand
legend    ch tile|prop|unit|mark|rect|fill ...
```

`.` is empty ground in every layer but `grid`. A legend character means one thing in the whole
file; anonymous props and units of one kind share a character, keyed ones have their own. The town
needs more characters than ASCII has, so grid characters may be any non-space character: the
converter took the thing's initial, then any free ASCII letter or digit, then accented letters. A
prop's JSON takes the fields of a placement row's prop (`talk`, `label`, `locked`, `keyTag`, `to`,
`loot`, `use`, ...), typed at build.

A chunk is a pure grid. The one procedural part is a **fill**: cells a row of
`data/tuning/chunks.json` places props in, seed by seed. Today there is one, the graveyard's
coffins. The same file holds `boxMargin`, the room kept between a box and the county's edge.

At build (`jane check`) every chunk is linted: rectangular layers the box's size, every character
defined and every entry drawn, each kind in its own layer, props whole footprints of real defs,
keyed things drawn once, units on open ground, nothing outside the box, gates on the ring with open
ground inside, a door's `to` a mark its zone provides. The marks, rects and keyed props and units a
chunk draws are name providers (ARCHITECTURE.md §5.3): a quest that names `house_door` is checked
against the chunk that draws it.

`tools/gen/chunks-from-ts.ts` and `tools/gen/chunks-from-ts.py` made these files from the
TypeScript and check them against it; they are for re-checking the conversion, not for editing.
Edit the `.chunk` files.
