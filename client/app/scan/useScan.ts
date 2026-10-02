import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { ApiError, api } from "@domain/api";
import type { Found } from "@domain/types";
import { destinationFor } from "./destination";
import type { Landing } from "./destination";
import { useNavigate, usePath } from "@app/routing/Router";
import { claim, currentHolder, release, subscribe } from "./claim";

export function useScanHolder() {
  return useSyncExternalStore(subscribe, currentHolder, currentHolder);
}

/**
 * A screen declaring that the scanner is its own.
 *
 * Capture calls this. While it holds the claim, the chrome draws no locator on
 * Floor and never takes focus on any surface.
 */
export function useScanClaim(active: boolean): void {
  useEffect(() => {
    if (!active) return;
    claim("screen");
    return () => release("screen");
  }, [active]);
}

export interface ChromeScan {
  value: string;
  busy: boolean;
  landing: Landing | null;
  /** What the global search found for what is typed, as it is typed (D189); null before it answers. */
  results: Found[] | null;
  type: (next: string) => void;
  /** Enter: the result picked with the arrows, or what was typed as one code (D111), or the best result. */
  scan: (picked?: Found | null) => Promise<void>;
  /** Open a result. */
  open: (found: Found) => void;
  dismiss: () => void;
}

/** How long typing pauses before the search is asked: a scan arrives whole and asks once. */
const PAUSE_MS = 120;

/**
 * The chrome's locator.
 *
 * Resolves with no `expect`, because the chrome accepts anything — narrowing is
 * what a screen's own scanner does when it already knows what it is looking at.
 *
 * A single subject navigates and clears. Anything less certain is drawn where
 * the operator can read it, with the scanned string echoed verbatim, rather
 * than resolved by preference (D111).
 */
export function useChromeScan(): ChromeScan {
  const navigate = useNavigate();
  const path = usePath();
  const [value, setValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [landing, setLanding] = useState<Landing | null>(null);
  const [results, setResults] = useState<Found[] | null>(null);
  // The newest question wins: an answer to an earlier one is dropped.
  const asked = useRef(0);

  // **Arriving somewhere else answers the question.** The chrome outlives the
  // screen now, so a scan that came back ambiguous would otherwise keep its
  // panel — and the codes it offers — in the top bar of whatever the operator
  // navigated to next. It used to clear because the whole shell was thrown away
  // on every navigation, which was never a decision anybody made.
  useEffect(() => {
    setLanding(null);
    setValue("");
    setResults(null);
  }, [path]);

  // Search as it is typed, once typing pauses.
  useEffect(() => {
    const q = value.trim();
    const n = ++asked.current;
    if (!q) {
      setResults(null);
      return;
    }
    const t = window.setTimeout(() => {
      api
        .search(q)
        .then((a) => n === asked.current && setResults(a.results))
        .catch(() => n === asked.current && setResults([]));
    }, PAUSE_MS);
    return () => window.clearTimeout(t);
  }, [value]);

  const open = useCallback(
    (found: Found) => {
      setValue("");
      setLanding(null);
      setResults(null);
      navigate(found.path);
    },
    [navigate],
  );

  const scan = useCallback(
    async (picked?: Found | null) => {
      if (picked) return open(picked);
      const scanned = value.trim();
      if (!scanned || busy) return;
      setBusy(true);
      try {
        const landed = destinationFor(await api.resolve(scanned));
        if (landed.kind === "go") {
          setValue("");
          setLanding(null);
          setResults(null);
          navigate(landed.path);
        } else if (landed.kind !== "choose" && results && results.length > 0) {
          // Not one code: the best of what the search found.
          open(results[0]!);
        } else {
          setLanding(landed);
        }
      } catch (error) {
        if (results && results.length > 0) open(results[0]!);
        else
          setLanding({
            kind: "unrecognised",
            scanned: error instanceof ApiError ? error.message : scanned,
          });
      } finally {
        setBusy(false);
      }
    },
    [value, busy, navigate, results, open],
  );

  return {
    value,
    busy,
    landing,
    results,
    type: (next) => {
      setValue(next);
      setLanding(null);
    },
    scan,
    open,
    dismiss: () => {
      setLanding(null);
      setValue("");
      setResults(null);
      release("chrome");
    },
  };
}
