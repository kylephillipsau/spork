/**
 * The findings tabs, and which one a given finding can be seen from.
 *
 * **Separate from `useFindings` because a link has to choose a tab before
 * anything renders**, and that choice is a rule rather than a behaviour. It is
 * also the half of this screen a test runner can read: `node --test` resolves
 * neither Vite aliases nor React, and the hook next door imports both.
 *
 * Relative imports for the same reason `manifest.ts` uses them.
 */

/** The tabs, and the state sets behind them. Order is the order they are drawn. */
export const VIEWS = [
  { key: "live", label: "Live", states: "open,investigating" },
  { key: "open", label: "Open", states: "open" },
  { key: "investigating", label: "Investigating", states: "investigating" },
  { key: "closed", label: "Closed", states: "resolved,accepted" },
] as const;

export type ViewKey = (typeof VIEWS)[number]["key"];

/** What this tab asks the server for. Comma-separated, as `?state=` wants it. */
export function statesOf(view: ViewKey): string {
  return VIEWS.find((v) => v.key === view)?.states ?? "open";
}

/** Whether a finding in this state would appear under this tab. */
export function shows(view: ViewKey, state: string): boolean {
  return statesOf(view).split(",").includes(state);
}

/**
 * Which tab to be on to see a finding in this state, given where you are.
 *
 * **The tab you are already on, whenever it contains the finding.** Following a
 * link to a second open finding while reading the Open tab should not move
 * anybody to Live: a link names a finding, not a place to look at it from, and
 * moving the tab under somebody is the thing the rail was built to avoid.
 *
 * It moves only when it must — a closed finding cannot be seen from Live, and a
 * queue that does not contain the row the panel is showing is a screen
 * contradicting itself. A state no tab covers leaves the tab alone: that is a
 * finding this client does not know how to file, and guessing would be worse
 * than showing it on the rail with the queue where the operator left it.
 */
export function viewFor(state: string, current: ViewKey): ViewKey {
  if (shows(current, state)) return current;
  return VIEWS.find((v) => shows(v.key, state))?.key ?? current;
}
