import { beatSpine } from "@/game/systems/catalog";
import type { BeatDef } from "@/game/types";

export class BeatTracker {
  readonly spine: BeatDef[];
  private flags = new Set<string>();

  constructor() {
    this.spine = beatSpine;
  }

  has(id: string): boolean {
    return this.flags.has(id);
  }

  complete(id: string): boolean {
    const index = this.spine.findIndex((beat) => beat.id === id);
    if (index < 0) return false;
    if (index > 0 && !this.flags.has(this.spine[index - 1].id)) return false;
    this.flags.add(id);
    return true;
  }

  current(): BeatDef {
    for (const beat of this.spine) {
      if (!this.flags.has(beat.id)) return beat;
    }
    return this.spine[this.spine.length - 1];
  }

  index(): number {
    return this.spine.findIndex((beat) => !this.flags.has(beat.id));
  }

  debugLine(): string {
    const i = this.index();
    const n = i < 0 ? this.spine.length : i;
    const beat = this.current();
    return `beat ${Math.min(n + 1, this.spine.length)}/${this.spine.length}  ${beat.title}`;
  }

  list(): string[] {
    return [...this.flags];
  }

  restore(ids: string[]): void {
    this.flags = new Set(ids);
  }

  grant(id: string): void {
    this.flags.add(id);
  }
}
