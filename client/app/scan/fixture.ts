import type { Landing } from "@app/nav/destination";
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

export function fixtureScan(landing: Landing | null = null, value = ""): ChromeScan {
  return { value, busy: false, landing, type: () => {}, scan: noop, dismiss: () => {} };
}

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
export const NO_SCREEN: Landing = { kind: "nowhere", scanned: "A-01-1", what: "location" };
