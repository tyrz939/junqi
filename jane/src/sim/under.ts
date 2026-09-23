// Things under things. A key under a stone, a tin under a loose flag by the step, a letter under a
// crate: the thing is a hidden prop lying at a cell, and the prop on top of it names it in `under`.
// Push the prop off it and it is there: shown, on the ground, to be picked up like anything else.
//
// Two rules keep it honest:
//   - It is under the prop, cell for cell. While any cell of the top prop still covers it, nothing
//     shows. Nudge the stone one cell sideways and a one-cell stone has uncovered it.
//   - `underWhen` holds, or nothing is there yet. Mrs Bettany's key is under her stone once she has
//     said so; pushed about before that, the stone has earth under it. When the quest is given later
//     with the stone already off the spot, the key is found lying there then (`uncoverAll`, from
//     giveQuest), so no order of doing things loses it.
//
// Deterministic and replay-safe: it runs inside the sim step (a push, a put-down, a quest given),
// reads only sim state, and what changes is saved prop state (`hidden`, `under`).

import { conditionsMet } from "@/sim/actions";
import { touchProp, type World } from "@/sim/runtime";
import type { Prop } from "@/sim/state";

/** Does prop `top` still cover any cell of prop `low`? */
function covers(w: World, top: Prop, low: Prop): boolean {
  const a = w.catalog.props[top.def];
  const b = w.catalog.props[low.def];
  return top.cx < low.cx + b.w && low.cx < top.cx + a.w && top.cy < low.cy + b.h && low.cy < top.cy + a.h;
}

/** `top` has moved (or a condition may have changed): show what it was lying on, if it can be seen now. */
export function uncover(w: World, top: Prop): boolean {
  if (!top.under) return false;
  const low = w.rt.propsByKey.get(top.under);
  if (!low) {
    top.under = undefined;
    return false;
  }
  if (covers(w, top, low) || !conditionsMet(w, top.underWhen)) return false;
  top.under = undefined;
  top.underWhen = undefined;
  if (low.hidden) {
    low.hidden = false;
    touchProp(w, low);
    w.emit({ e: "prop", prop: low.id, change: "show" });
    w.emit({ e: "toast", text: low.label ? `Under the ${w.catalog.props[top.def].name.toLowerCase()}: ${low.label.replace(/^A /, "a ").replace(/^The /, "the ")}` : "There is something under it" });
  }
  return true;
}

/** Every prop in this zone that is off what it lies on: a quest given may make one of them true now. */
export function uncoverAll(w: World): void {
  for (const p of w.zone.props) if (p.under) uncover(w, p);
}
