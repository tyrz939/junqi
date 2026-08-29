import catalog from "@/data/triggers.json";

export type TriggerWhen = "enter" | "clear";

export type TriggerDef = {
  id: string;
  when: TriggerWhen;
  actions: string[];
  once?: boolean;
  watch?: string[];
};

export type TriggerSpec = TriggerDef & {
  rect: { x: number; y: number; w: number; h: number };
};

const triggerCatalog = catalog as Record<string, TriggerDef>;

export function stampTrigger(
  id: string,
  x: number,
  y: number,
  w = 48,
  h = 48,
): TriggerSpec {
  const row = triggerCatalog[id];
  if (!row) throw new Error(`Unknown trigger: ${id}`);
  return {
    ...row,
    rect: { x: x - w / 2, y: y - h / 2, w, h },
  };
}

export function makeTrigger(
  id: string,
  when: TriggerWhen,
  actions: string[],
  extra?: { once?: boolean; watch?: string[]; x?: number; y?: number; w?: number; h?: number },
): TriggerSpec {
  const w = extra?.w ?? 48;
  const h = extra?.h ?? 48;
  const x = extra?.x ?? 0;
  const y = extra?.y ?? 0;
  return {
    id,
    when,
    actions,
    once: extra?.once ?? true,
    watch: extra?.watch,
    rect: { x: x - w / 2, y: y - h / 2, w, h },
  };
}

export function inTriggerRect(
  x: number,
  y: number,
  rect: { x: number; y: number; w: number; h: number },
): boolean {
  return x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h;
}
