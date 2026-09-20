// The seed viewer. The generator is reviewed by looking at two dozen counties at
// once, not by playing two dozen games (PLAN.md 2.3). Everything drawn here is read
// straight off the Skeleton the game itself will be built from; nothing is restaged.

import { roadDistances } from "@/world/skeleton/roads";
import { at, Biome, buildSkeleton, countBridges, MACRO, REGION_NAMES, ROAD, ROAD_BRIDGE, ROAD_LIT, threatAt, type Skeleton } from "@/world/skeleton";

type Layer = "land" | "threat" | "regions";
const WALK = 7.5; // metres a second: 60 px/s at 8 px to the metre

const BIOME_RGB: Record<number, [number, number, number]> = {
  [Biome.Field]: [111, 154, 74],
  [Biome.Hedge]: [93, 138, 66],
  [Biome.Wood]: [47, 93, 58],
  [Biome.Foothill]: [141, 138, 98],
  [Biome.Reed]: [127, 154, 106],
  [Biome.Marsh]: [85, 119, 94],
  [Biome.WetWood]: [45, 81, 72],
  [Biome.Garden]: [122, 143, 88],
  [Biome.Slag]: [90, 85, 96],
  [Biome.Yard]: [107, 100, 112],
  [Biome.Hill]: [138, 133, 144],
  [Biome.Town]: [160, 150, 130],
};
const THREAT_RGB: [number, number, number][] = [
  [120, 200, 80],
  [196, 204, 150],
  [240, 208, 72],
  [224, 136, 56],
  [200, 64, 60],
  [122, 36, 48],
  [48, 10, 30],
];
const REGION_RGB: [number, number, number][] = [
  [120, 160, 84],
  [84, 132, 150],
  [120, 108, 128],
];

const state = { seed: 1, layer: "land" as Layer, night: false, labels: true, open: -1 };
const cache = new Map<number, Skeleton>();
const $ = <T extends HTMLElement>(id: string): T => document.getElementById(id) as T;

function skeleton(seed: number): Skeleton {
  let s = cache.get(seed);
  if (!s) cache.set(seed, (s = buildSkeleton(seed)));
  return s;
}

function paint(canvas: HTMLCanvasElement, s: Skeleton, scale: number, detail: boolean): void {
  canvas.width = s.w * scale;
  canvas.height = s.h * scale;
  const ctx = canvas.getContext("2d")!;
  const img = ctx.createImageData(s.w, s.h);
  for (let y = 0; y < s.h; y++) {
    for (let x = 0; x < s.w; x++) {
      const i = at(x, y);
      let rgb: [number, number, number];
      const shade = 0.78 + (s.height[i] / 255) * 0.5;
      if (s.water[i]) rgb = s.water[i] === 2 ? [47, 95, 152] : [63, 120, 176];
      else if (state.layer === "threat") {
        const t = THREAT_RGB[threatAt(s, x, y, state.night)];
        rgb = [t[0] * shade * 0.92, t[1] * shade * 0.92, t[2] * shade * 0.92];
      } else {
        const b = state.layer === "regions" ? REGION_RGB[s.region[i]] : BIOME_RGB[s.biome[i]];
        const dark = state.night ? 0.55 : 1;
        rgb = [b[0] * shade * dark, b[1] * shade * dark, b[2] * shade * dark];
      }
      if (s.road[i] & ROAD) rgb = s.road[i] & ROAD_BRIDGE ? [232, 224, 208] : s.road[i] & ROAD_LIT ? [240, 208, 72] : state.night ? [96, 88, 84] : [200, 184, 160];
      const o = i * 4;
      img.data[o] = rgb[0];
      img.data[o + 1] = rgb[1];
      img.data[o + 2] = rgb[2];
      img.data[o + 3] = 255;
    }
  }
  const small = document.createElement("canvas");
  small.width = s.w;
  small.height = s.h;
  small.getContext("2d")!.putImageData(img, 0, 0);
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(small, 0, 0, canvas.width, canvas.height);

  const px = (m: number): number => (m + 0.5) * scale;
  if (detail) {
    ctx.lineWidth = 1;
    for (const a of s.areas) {
      ctx.strokeStyle = a.row.threat <= 2 ? "#e8e2d6aa" : "#1a1420cc";
      ctx.setLineDash([4, 3]);
      ctx.beginPath();
      ctx.arc(px(a.mx), px(a.my), (a.row.radius / MACRO) * scale, 0, 6.2832);
      ctx.stroke();
    }
    ctx.setLineDash([]);
    for (const p of s.pois) {
      ctx.fillStyle = "#1a1420";
      ctx.fillRect(px(p.mx) - 2, px(p.my) - 2, 4, 4);
      ctx.fillStyle = "#e8e2d6";
      ctx.fillRect(px(p.mx) - 1, px(p.my) - 1, 2, 2);
    }
  }
  const r = detail ? 5 : 3;
  for (const site of s.sites) {
    const x = px(site.mx);
    const y = px(site.my);
    ctx.fillStyle = "#1a1420";
    ctx.fillRect(x - r - 1, y - r - 1, r * 2 + 2, r * 2 + 2);
    ctx.fillStyle = site.row.dungeon ? "#e0605a" : site.row.rest ? "#f0a040" : "#f4f0e6";
    ctx.fillRect(x - r, y - r, r * 2, r * 2);
  }
  if (!state.labels) return;
  ctx.font = `${detail ? 12 : 9}px ui-monospace, Consolas, monospace`;
  ctx.textBaseline = "middle";
  const label = (text: string, x: number, y: number, color: string): void => {
    const w = ctx.measureText(text).width;
    const lx = x + w + 10 > canvas.width ? x - w - 8 : x + 8;
    ctx.fillStyle = "#14101ad0";
    ctx.fillRect(lx - 2, y - (detail ? 8 : 6), w + 4, detail ? 16 : 12);
    ctx.fillStyle = color;
    ctx.fillText(text, lx, y + 1);
  };
  for (const site of s.sites) label(detail ? site.name : site.id.replace(/_/g, " "), px(site.mx), px(site.my), "#f4f0e6");
  if (detail) for (const a of s.areas) label(`${a.name} · ${a.row.threat}`, px(a.mx), px(a.my), "#f0d048");
}

function fromStation(s: Skeleton): Map<string, number> {
  const station = s.sites.find((x) => x.id === "station")!;
  const d = roadDistances(s.road, station.mx, station.my);
  return new Map(s.sites.map((x) => [x.id, d[at(x.mx, x.my)]]));
}

const clock = (metres: number): string => {
  if (!Number.isFinite(metres)) return "no road";
  const sec = Math.round(metres / WALK);
  return `${Math.floor(sec / 60)}:${String(sec % 60).padStart(2, "0")}`;
};

function renderGrid(): void {
  const grid = $("grid");
  grid.innerHTML = "";
  $("detail").classList.remove("open");
  grid.style.display = "";
  $("back").style.display = "none";
  let n = 0;
  const started = performance.now();
  const step = (): void => {
    if (state.open >= 0 || n >= 24) {
      $("status").textContent = `24 counties in ${((performance.now() - started) / 1000).toFixed(1)} s`;
      return;
    }
    const seed = (state.seed + n) >>> 0;
    const s = skeleton(seed);
    const d = fromStation(s);
    const card = document.createElement("div");
    card.className = s.ok ? "card" : "card bad";
    const canvas = document.createElement("canvas");
    paint(canvas, s, 2, false);
    const meta = document.createElement("div");
    meta.className = "meta";
    const km = (s.roads.reduce((a, r) => a + r.metres, 0) / 1000).toFixed(1);
    meta.innerHTML = `<span>seed <b>${seed}</b>${s.attempt ? ` · try ${s.attempt + 1}` : ""}</span><span>Julie's <b>${clock(d.get("julie_house")!)}</b> · lake <b>${clock(d.get("lake_statue")!)}</b> · ${km} km · ${countBridges(s)} br</span>`;
    card.append(canvas, meta);
    card.onclick = () => open(seed);
    grid.append(card);
    n++;
    $("status").textContent = `building ${n} / 24`;
    setTimeout(step, 0);
  };
  step();
}

function open(seed: number): void {
  state.open = seed;
  save();
  const s = skeleton(seed);
  $("grid").style.display = "none";
  $("detail").classList.add("open");
  $("back").style.display = "";
  paint($<HTMLCanvasElement>("big"), s, 5, true);
  const d = fromStation(s);
  const rows = (list: string[][]): string => `<table>${list.map((r) => `<tr>${r.map((c, i) => `<td class="${i ? "n" : ""}">${c}</td>`).join("")}</tr>`).join("")}</table>`;
  const sites = [...s.sites].sort((a, b) => (d.get(a.id) ?? 0) - (d.get(b.id) ?? 0));
  $("side").innerHTML =
    `<h2>Seed ${seed}${s.attempt ? ` (attempt ${s.attempt + 1})` : ""}</h2>` +
    `<p class="note">Walking times are from the platform, by road, at Jane's pace (7.5 m/s, no sprint). Off-road places show the crow's distance from Julie's house instead.</p>` +
    `<h2>Places, in the order you reach them</h2>` +
    rows(
      sites.map((x) => {
        const m = d.get(x.id)!;
        const tag = x.row.dungeon ? " ◆" : x.row.rest ? " ●" : "";
        return [`${x.name}${tag}`, Number.isFinite(m) ? `${Math.round(m)} m` : "off road", clock(m), `threat ${s.threat[at(x.mx, x.my)]}`];
      }),
    ) +
    `<h2>Rules (${s.checks.filter((c) => c.ok).length} / ${s.checks.length})</h2>` +
    rows(s.checks.map((c) => [`<span class="${c.ok ? "ok" : "no"}">${c.ok ? "✓" : "✗"}</span> ${c.rule}`, c.detail])) +
    `<h2>Named patches</h2>` +
    rows(s.areas.map((a) => [a.name, REGION_NAMES[s.region[at(a.mx, a.my)]], `threat ${a.row.threat}`, `${a.row.radius} m`])) +
    `<h2>Roads</h2>` +
    rows(s.roads.map((r) => [`${r.from} → ${r.to}`, `${r.metres} m`, clock(r.metres)])) +
    `<h2>Small places</h2>` +
    rows(REGION_NAMES.map((name, r) => [name, `${s.pois.filter((p) => p.region === r).length}`]));
  const sw = (rgb: number[], text: string): string => `<span><i class="sw" style="background: rgb(${rgb.join(",")})"></i>${text}</span>`;
  $("legend").innerHTML =
    state.layer === "threat"
      ? THREAT_RGB.map((c, i) => sw(c, i === 0 ? "0 haven" : `${i}`)).join("")
      : sw([240, 208, 72], "lit road") + sw([200, 184, 160], "dark road") + sw([224, 96, 90], "dungeon") + sw([240, 160, 64], "bed or fire") + sw([244, 240, 230], "place") + sw([232, 226, 214], "· small place");
  $("note").textContent = state.layer === "threat" ? (state.night ? "Night: +1 outside lamplight, +2 in the Works, and an unlit road loses its discount." : "Day. Press Night to see what the lamps are worth.") : "";
}

function save(): void {
  const q = new URLSearchParams({ seed: String(state.seed), layer: state.layer, night: state.night ? "1" : "0", labels: state.labels ? "1" : "0" });
  if (state.open >= 0) q.set("open", String(state.open));
  history.replaceState(null, "", `#${q}`);
}

function load(): void {
  const q = new URLSearchParams(location.hash.slice(1));
  state.seed = Number(q.get("seed") ?? 1) >>> 0;
  state.layer = (q.get("layer") as Layer) ?? "land";
  state.night = q.get("night") === "1";
  state.labels = q.get("labels") !== "0";
  state.open = q.has("open") ? Number(q.get("open")) >>> 0 : -1;
}

function refresh(): void {
  save();
  $<HTMLInputElement>("seed").value = String(state.seed);
  for (const b of document.querySelectorAll<HTMLButtonElement>("#layers button")) b.classList.toggle("on", b.dataset.layer === state.layer);
  $("night").classList.toggle("on", state.night);
  $("labels").classList.toggle("on", state.labels);
  if (state.open >= 0) open(state.open);
  else renderGrid();
}

load();
$("go").onclick = () => {
  state.seed = Number($<HTMLInputElement>("seed").value) >>> 0;
  state.open = -1;
  refresh();
};
$("next").onclick = () => {
  state.seed = (state.seed + 24) >>> 0;
  state.open = -1;
  refresh();
};
$("rand").onclick = () => {
  state.seed = (Math.random() * 0xffffffff) >>> 0;
  state.open = -1;
  refresh();
};
$("back").onclick = () => {
  state.open = -1;
  refresh();
};
$("night").onclick = () => {
  state.night = !state.night;
  refresh();
};
$("labels").onclick = () => {
  state.labels = !state.labels;
  refresh();
};
for (const b of document.querySelectorAll<HTMLButtonElement>("#layers button")) {
  b.onclick = () => {
    state.layer = b.dataset.layer as Layer;
    refresh();
  };
}
refresh();
