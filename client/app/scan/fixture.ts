import type { Found } from "@domain/types";
import type { Landing } from "./destination";
import type { ChromeScan } from "./useScan";

/**
 * The locator, with no network.
 *
 * D111's four outcomes are the reason this exists. A live screen shows one of
 * them for a moment, only after a scan, and three of the four need a warehouse
 * with the wrong labels in it — so a fixture is the only way anybody looks at
 * them twice.
 */
const noop = async () => {};

export function fixtureScan(landing: Landing | null = null, value = "", results: Found[] | null = null): ChromeScan {
  return { value, busy: false, landing, results, type: () => {}, scan: noop, open: () => {}, dismiss: () => {} };
}

const found = (kind: Found["kind"], n: number, title: string, detail: string): Found => ({
  kind,
  id: `01990000-0000-7000-8000-00000000f${String(n).padStart(3, "0")}`,
  title,
  detail,
  path: kind === "item" ? "/items/x" : kind === "bin" ? "/bins/x" : "/orders/x",
  score: 10 - n,
});

/** What the search finds for "30", as it is typed (D189). */
export const SEARCHED: Found[] = [
  found("item", 1, "SKU-3010-10", "Cut liner glove, white, XL"),
  found("item", 2, "SKU-3010-08", "Cut liner glove, white, M"),
  found("bin", 3, "C-30-01", "Bin · Rack C"),
  found("order", 4, "S100300", "North Foods · PO 4430"),
];

/** Two things answer to one code. Nothing is resolved by preference (D111). */
export const AMBIGUOUS: Landing = {
  kind: "choose",
  scanned: "9312345678907",
  options: [
    { label: "STY-7720-08", detail: "Gumboots, AU8", path: "/capture" },
    { label: "STY-7720-12", detail: "Gumboots, AU12", path: "/capture" },
  ],
};

/** Well-formed, and nobody holds it. Not the same as a smudge. */
export const UNKNOWN: Landing = { kind: "unknown", scanned: "09312345678907" };

/** Not an identifier at all. */
export const UNRECOGNISED: Landing = { kind: "unrecognised", scanned: "j@#f0 8" };

/** Read correctly, and there is no screen for what it is. */
export const NO_SCREEN: Landing = { kind: "nowhere", scanned: "PALLET-A", what: "package" };
