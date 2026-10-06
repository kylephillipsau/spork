import type { MapBin } from "@domain/types";

/**
 * What the bin map colours by (D208), with no renderer in it.
 *
 * Two layers, because two have data behind them: what NetSuite's last count
 * put on each shelf, and whether a picker can reach it from the floor. Pick
 * frequency waits for picks that say which bin they came from: NetSuite's
 * reports don't, and Spork's own are only starting.
 */

export type Layer = "stock" | "reach";

export const LAYERS: { id: Layer; label: string }[] = [
  { id: "stock", label: "What's here" },
  { id: "reach", label: "Reach" },
];

export type Tone = "empty" | "one" | "two" | "many" | "reach" | "ladder";

/** The tone a bin is drawn in on a layer. */
export function toneOf(bin: MapBin, layer: Layer): Tone {
  if (layer === "reach") return bin.within_reach ? "reach" : "ladder";
  const items = Math.max(bin.reported_items, bin.held > 0 ? 1 : 0);
  if (items === 0) return "empty";
  if (items === 1) return "one";
  if (items === 2) return "two";
  return "many";
}

/**
 * Each tone as a theme token mixed into the surface, by how much of the token.
 * The scene mixes the same two colours in three.js, and the legend with CSS
 * `color-mix`, so the two can't drift apart and both follow the theme.
 *
 * A hollow tone is drawn as an empty box (D221): a see-through fill with its
 * edges in the token, so the shelves behind it show through.
 */
export const MIX: Record<Tone, { token: string; share: number; hollow?: true }> = {
  empty: { token: "--ui-border-strong", share: 0.55, hollow: true },
  one: { token: "--ui-info", share: 0.35 },
  two: { token: "--ui-info", share: 0.6 },
  many: { token: "--ui-info", share: 0.9 },
  reach: { token: "--ui-success", share: 0.55 },
  ladder: { token: "--ui-warning", share: 0.6 },
};

/** What each layer's legend says, in the order it says it. */
export const LEGEND: Record<Layer, { tone: Tone; label: string }[]> = {
  stock: [
    { tone: "empty", label: "Empty" },
    { tone: "one", label: "1 item" },
    { tone: "two", label: "2 items" },
    { tone: "many", label: "3 or more" },
  ],
  reach: [
    { tone: "reach", label: "From the floor" },
    { tone: "ladder", label: "Ladder or forklift" },
  ],
};

/** How much of a hollow box's fill shows: enough to see the box, little enough to see through it. */
export const HOLLOW = 0.15;

/** The legend's swatch: the same mix, in CSS, or the same empty box. */
export function swatch(tone: Tone): { background: string; borderColor?: string } {
  const { token, share, hollow } = MIX[tone];
  if (hollow) return { background: `color-mix(in srgb, var(${token}) ${HOLLOW * 100}%, transparent)`, borderColor: `var(${token})` };
  return { background: `color-mix(in srgb, var(${token}) ${Math.round(share * 100)}%, var(--ui-surface))` };
}

/**
 * The bins a search finds, best first: the code exactly, then codes that
 * start with what was typed, then codes that contain it. Case and the dashes
 * people leave out don't matter, so "e3601" finds E-36-01.
 */
export function findBins(bins: readonly MapBin[], typed: string, limit = 8): MapBin[] {
  const want = squash(typed);
  if (!want) return [];
  const exact: MapBin[] = [];
  const starts: MapBin[] = [];
  const within: MapBin[] = [];
  for (const bin of bins) {
    const code = squash(bin.code);
    if (code === want) exact.push(bin);
    else if (code.startsWith(want)) starts.push(bin);
    else if (code.includes(want)) within.push(bin);
  }
  return [...exact, ...starts, ...within].slice(0, limit);
}

function squash(s: string): string {
  return s.toLowerCase().replace(/[^a-z0-9]/g, "");
}
