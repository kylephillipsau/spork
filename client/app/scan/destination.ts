import type { Resolution, Subject } from "../../domain/types.ts";

/**
 * Where a scan goes (D111).
 *
 * *"One always-present input accepts anything the system can resolve — an SSCC,
 * an item barcode, a location code, a package barcode, an order or fulfilment
 * reference — and goes there."*
 *
 * # Four outcomes, and the fourth is the one that costs
 *
 * D111 is explicit that collapsing them is the failure: *"a well-formed
 * identifier nobody holds is `identifier_unknown`; a string that is not an
 * identifier at all is `identifier_unrecognised`. Collapsing them tells an
 * operator 'no such item' about a smudge, and — worse in the other direction —
 * tells them the same thing about two items when the truth is
 * `identifier_ambiguous`. That last is how a scan gets recorded against the
 * wrong product, so nothing is ever resolved by silent preference."*
 *
 * A pure function so those four can be exercised by a table test, rather than a
 * `switch` inside a component that only a browser can reach.
 */

export type Landing =
  /** One subject, one place to be. Navigate; no results page, ever. */
  | { kind: "go"; path: string }
  /** Several. The operator chooses; the system never prefers silently. */
  | { kind: "choose"; scanned: string; options: readonly Option[] }
  /** A well-formed identifier nobody holds. */
  | { kind: "unknown"; scanned: string }
  /** Not an identifier at all — a smudge, or a keyboard in the wrong field. */
  | { kind: "unrecognised"; scanned: string }
  /** The scan resolved to something this client has no screen for yet. Said
   *  plainly rather than navigated nowhere. */
  | { kind: "nowhere"; scanned: string; what: string };

export interface Option {
  readonly label: string;
  readonly detail: string | null;
  readonly path: string;
}

/**
 * Which screen shows a subject.
 *
 * Returns null for a kind with no screen. **Not a default**: sending a location
 * scan to the capture screen because that is the only one built would be the
 * silent preference D111 forbids, one level up.
 */
export function screenFor(subject: Subject): string | null {
  switch (subject.kind) {
    case "item":
      // An item resolves to what can be measured about it, which is the one
      // screen that takes an item as its subject today.
      return subject.capture.length > 0 ? "/capture" : null;
    default:
      return null;
  }
}

function optionsFor(subjects: readonly Subject[]): Option[] {
  return subjects.flatMap((s) => {
    const path = screenFor(s);
    return path ? [{ label: s.code, detail: s.description, path }] : [];
  });
}

export function destinationFor(resolution: Resolution): Landing {
  const scanned = resolution.scanned;

  switch (resolution.outcome) {
    case "resolved": {
      const options = optionsFor(resolution.subjects);
      const only = options[0];
      if (options.length === 1 && only) return { kind: "go", path: only.path };
      if (options.length > 1) return { kind: "choose", scanned, options };
      // Resolved to something real that no screen takes as a subject — a
      // location, or a package. That is a gap in the client rather than a
      // failure of the scan, and saying which is the difference between
      // "we know what that is and cannot show it" and "no such thing".
      return {
        kind: "nowhere",
        scanned,
        what: resolution.subjects[0]?.kind ?? "thing",
      };
    }
    case "identifier_ambiguous":
      return { kind: "choose", scanned, options: optionsFor(resolution.subjects) };
    case "identifier_unknown":
      return { kind: "unknown", scanned };
    default:
      return { kind: "unrecognised", scanned };
  }
}
