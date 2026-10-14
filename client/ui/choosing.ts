/**
 * Choosing several things to drag together (D245): which are chosen after a
 * click or a key, and what a drag carries. No React here, so a test runner can
 * read it.
 */

/** How a press changes what is chosen: only it, it in or out, or it as well. */
export type ChooseMode = "only" | "toggle" | "add";

export function choose(chosen: readonly string[], key: string, mode: ChooseMode): string[] {
  switch (mode) {
    case "only":
      return [key];
    case "toggle":
      return chosen.includes(key) ? chosen.filter((k) => k !== key) : [...chosen, key];
    case "add":
      return chosen.includes(key) ? [...chosen] : [...chosen, key];
  }
}

/**
 * What a drag that starts on `pressed` carries: everything chosen when it is
 * one of them, else it alone, as a file manager does.
 */
export function carried(chosen: readonly string[], pressed: string): string[] {
  return chosen.includes(pressed) ? [...chosen] : [pressed];
}

/** How a drop puts things in (settled 5): as many as fit, one with Shift, asking how many with Alt. */
export interface DropHow {
  one: boolean;
  ask: boolean;
}

export const dropHow = (e: { shiftKey: boolean; altKey: boolean }): DropHow => ({ one: e.shiftKey, ask: e.altKey });
