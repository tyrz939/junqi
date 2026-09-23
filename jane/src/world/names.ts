// Names for the places the county generates: the hamlets, farms, inns, cottages and woodcutters'
// clearings world/country.ts builds by the hundred, and the camps and ruins a story sends her to.
// Every one of them gets a board with its name on it (world/stories.ts), so a quest can say
// "the hen house at Hollins Farm" and she can walk the road until she reads HOLLINS FARM.
//
// A name is a pure function of the seed and the story, never of the built county: text that says
// {place:<story>} can be shown anywhere (the quest log in a dungeon, a save loaded underground)
// without building anything. The county gives the story's name to whichever place the story
// claims, and the places nobody claimed take the rest of the list in build order.

import namesJson from "@/data/names.json";
import { hashString, irandom, rngSeed } from "@/sim/rng";

/** Kinds of generated place a story may claim. */
export type StoryKind = "hamlet" | "farmstead" | "cottage" | "inn" | "woodcutter" | "camp" | "ruin";

/** Every hamlet, farm, inn, cottage and woodcutters' clearing is named; a camp or a ruin only when a story claims it. */
export const NAMED_KINDS: ReadonlySet<string> = new Set(["hamlet", "farmstead", "cottage", "inn", "woodcutter"]);

/**
 * One story: the place it needs, and the quests that are told there. Rows live in
 * data/stories/*.json; what the story puts at its place is data/placements rows with
 * `at: { place: <id>, slot }`, its words are data/quests and data/dialogue rows that say
 * {place:<id>} wherever they name it.
 */
export type StoryRow = {
  id: string;
  kind: StoryKind;
  /** Default "lowfields". */
  region?: "lowfields" | "waters" | "works";
  /** The threat of the ground under it, inclusive. Default [0, 2]. */
  threat?: [number, number];
  /** Cells from the first walk (the station road and the Castle road), inclusive. */
  first?: [number, number];
  /** Within `max` cells (centre to centre) of the place another story claimed: the second half of a chain. */
  near?: { story: string; max: number };
  /** A camp of these, at least `count` of them standing. "a|b": any mix of those. */
  hostile?: string;
  count?: number;
  /** The quests told here, in the order she meets them. */
  quests: string[];
  /**
   * The tales (data/stories/tales.json, QUEST-TREE.md "Tales"): the few stories with somebody at them
   * who is more than a neighbour. A tale's place has its own name, the same on every seed, and its own
   * words on the board; it takes no name from the seed's list.
   */
  tale?: boolean;
  name?: string;
  /** What the board at the place says, instead of the kind's usual line. {NAME} is the name in capitals. */
  board?: string;
};

const FILES = import.meta.glob("../data/stories/*.json", { eager: true, import: "default" }) as Record<string, StoryRow[]>;

/** Every story, in file then row order. Order decides who gets a contested place. */
export const STORIES: StoryRow[] = Object.keys(FILES)
  .sort()
  .flatMap((path) => FILES[path]);

const STORY_BY_ID = new Map(STORIES.map((s) => [s.id, s]));

type Names = typeof namesJson;

/** The full list for a kind, before the seed shuffles it. The woodcutters' clearings are roots x ends. */
function baseList(kind: string): string[] {
  const n = namesJson as unknown as Record<string, unknown>;
  if (kind === "woodcutter") {
    const w = (namesJson as Names).woodcutter;
    const out: string[] = [];
    for (const end of w.ends) for (const root of w.roots) if (!end.includes(root)) out.push(`${root} ${end}`);
    return out;
  }
  const list = n[kind];
  return Array.isArray(list) ? (list as string[]) : [];
}

/** Every name any seed could give a place of this kind (the first time round the list). For the tests. */
export function allNames(kind: string): string[] {
  return baseList(kind);
}

const pools = new Map<string, string[]>();

/** The seed's order for a kind's names. */
function pool(seed: number, kind: string): string[] {
  const key = `${seed >>> 0}:${kind}`;
  let p = pools.get(key);
  if (!p) {
    p = [...baseList(kind)];
    const rng = rngSeed((seed ^ hashString(`names:${kind}`)) >>> 0, 7);
    for (let i = p.length - 1; i > 0; i--) {
      const j = irandom(rng, i);
      [p[i], p[j]] = [p[j], p[i]];
    }
    if (pools.size > 64) pools.clear();
    pools.set(key, p);
  }
  return p;
}

const PAST = ["Upper", "Lower", "Little", "Great", "Far", "Old"];

/** The n-th name of a kind on a seed. Past the end of the list, the list again with a word in front. */
export function nthName(seed: number, kind: string, n: number): string {
  const p = pool(seed, kind);
  if (p.length === 0) return "";
  // A kind with one name has it everywhere: every inn on the road is the Halfway House (the
  // innkeepers' own line in data/dialogue/country.json: "it was halfway when it was painted").
  if (n < p.length || p.length === 1) return p[Math.min(n, p.length - 1)];
  const lap = Math.floor(n / p.length) - 1;
  const base = p[n % p.length];
  const word = PAST[lap % PAST.length];
  return base.startsWith("The ") ? `The ${word} ${base.slice(4)}` : `${word} ${base}`;
}

/** How many stories take their names from a kind's list before the unclaimed places start on it. */
export function storiesOfKind(kind: string): number {
  return STORIES.filter((s) => s.kind === kind && !s.name).length;
}

/** The name a story's place has on this seed, whether or not the seed found it a place. A tale's is its own. */
export function storyName(seed: number, id: string): string | undefined {
  const s = STORY_BY_ID.get(id);
  if (!s) return undefined;
  if (s.name) return s.name;
  const n = STORIES.filter((x) => x.kind === s.kind && !x.name).indexOf(s);
  return nthName(seed, s.kind, n);
}

/** What the board at a named place says: the name in capitals and a line a parish or a farmer would paint. */
export function boardText(kind: string, name: string, n: number): string {
  const lines = ((namesJson as Names).boards as Record<string, string[]>)[kind] ?? ["{NAME}."];
  return lines[n % lines.length].replace("{NAME}", name.toUpperCase());
}
