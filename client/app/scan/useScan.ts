import { useCallback, useEffect, useState, useSyncExternalStore } from "react";
import { ApiError, api } from "@domain/api";
import { destinationFor } from "@app/nav/destination";
import type { Landing } from "@app/nav/destination";
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
  type: (next: string) => void;
  scan: () => Promise<void>;
  dismiss: () => void;
}

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

  // **Arriving somewhere else answers the question.** The chrome outlives the
  // screen now, so a scan that came back ambiguous would otherwise keep its
  // panel — and the codes it offers — in the top bar of whatever the operator
  // navigated to next. It used to clear because the whole shell was thrown away
  // on every navigation, which was never a decision anybody made.
  useEffect(() => {
    setLanding(null);
    setValue("");
  }, [path]);

  const scan = useCallback(async () => {
    const scanned = value.trim();
    if (!scanned || busy) return;
    setBusy(true);
    try {
      const landed = destinationFor(await api.resolve(scanned));
      if (landed.kind === "go") {
        setValue("");
        setLanding(null);
        navigate(landed.path);
      } else {
        setLanding(landed);
      }
    } catch (error) {
      setLanding({
        kind: "unrecognised",
        scanned: error instanceof ApiError ? error.message : scanned,
      });
    } finally {
      setBusy(false);
    }
  }, [value, busy, navigate]);

  return {
    value,
    busy,
    landing,
    type: setValue,
    scan,
    dismiss: () => {
      setLanding(null);
      setValue("");
      release("chrome");
    },
  };
}
