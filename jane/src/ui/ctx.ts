// What every widget gets handed. Widgets talk to the game only through `host`.

import type { DragManager } from "@/ui/drag";
import type { UiAction, UiHost } from "@/ui/host";
import type { Tooltip } from "@/ui/tooltip";

export type Ctx = {
  readonly host: UiHost;
  readonly root: HTMLElement;
  readonly drag: DragManager;
  readonly tip: Tooltip;
  /** Cached host.iconUrl. "" for an unknown icon. */
  icon(id: string): string;
  toast(text: string): void;
  /** Toasts pushed so far: lets a caller see whether the host already reported something. */
  toastCount(): number;
  /** Same entry point the input layer uses, for on-screen buttons. */
  act(a: UiAction): boolean;
};
