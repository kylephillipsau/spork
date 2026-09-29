import { useCallback, useRef, useState } from "react";
import { api, reason } from "@domain/api";
import type { ImportReport, ItemImportReport } from "@domain/types";

/**
 * Loading a bin list from the browser (D158).
 *
 * # The token, and why the screen mints its own
 *
 * D158 gives the import endpoints one credential kind and refuses sessions
 * there: the kind *is* the scope, and a scope a session also satisfies is a
 * suggestion. A browser upload is a session, so rather than weaken that, the
 * screen does what any other client would — it asks for a token, uses it, and
 * withdraws it.
 *
 * **No new server surface.** Mint, import, withdraw are three endpoints that
 * already exist. The withdrawn token stays on the list, which is the audit
 * trail rather than clutter: a load happened, this person authorised it, and
 * the credential that did it is dead.
 *
 * The withdrawal runs in a `finally`, because the token must not outlive the
 * upload whether the upload succeeded, failed, or the server never answered.
 * If even that fails the token is on the list, labelled and dated, for somebody
 * to withdraw by hand — which is the whole reason a label is required.
 */

export interface Options {
  /** What to call bins that state no type. Empty means leave them out. */
  assumeKind: string;
  /** Include warehouses that look like somebody else's premises. */
  includeExternal: boolean;
}

/** Which export this is. The two files have nothing in common but being CSV. */
export type Which = "bins" | "items";

export type ImportState =
  | { kind: "idle" }
  | { kind: "working"; what: "dry" | "apply" }
  | { kind: "reported"; which: "bins"; report: ImportReport; applied: boolean }
  | { kind: "reported"; which: "items"; report: ItemImportReport; applied: boolean }
  | { kind: "failed"; message: string };

export interface ImportBench {
  state: ImportState;
  which: Which;
  file: File | null;
  options: Options;
  pick: (which: Which) => void;
  choose: (file: File | null) => void;
  set: <K extends keyof Options>(key: K, value: Options[K]) => void;
  run: (apply: boolean) => Promise<void>;
  dismiss: () => void;
}

export function useImport(): ImportBench {
  const [state, setState] = useState<ImportState>({ kind: "idle" });
  const [file, setFile] = useState<File | null>(null);
  const [options, setOptions] = useState<Options>({ assumeKind: "", includeExternal: false });
  const [which, setWhich] = useState<Which>("bins");
  const live = useRef(true);

  const pick = useCallback((next: Which) => {
    setWhich(next);
    setState({ kind: "idle" });
  }, []);

  const choose = useCallback((next: File | null) => {
    setFile(next);
    setState({ kind: "idle" });
  }, []);

  const set = useCallback(<K extends keyof Options>(key: K, value: Options[K]) => {
    setOptions((o) => ({ ...o, [key]: value }));
  }, []);

  const run = useCallback(
    async (apply: boolean) => {
      if (!file) return;
      setState({ kind: "working", what: apply ? "apply" : "dry" });

      let tokenId: string | null = null;
      try {
        const stamp = new Date().toISOString().slice(0, 16).replace("T", " ");
        const minted = await api.mintApiToken({
          label: `Browser import · ${file.name} · ${stamp}`,
          days: 1,
        });
        tokenId = minted.id;

        const csv = await file.text();
        if (which === "items") {
          const report = await api.importItems(minted.token, csv, { apply });
          if (live.current) setState({ kind: "reported", which: "items", report, applied: apply });
        } else {
          const report = await api.importBins(minted.token, csv, {
            apply,
            ...(options.assumeKind.trim() ? { assumeKind: options.assumeKind.trim() } : {}),
            includeExternal: options.includeExternal,
          });
          if (live.current) setState({ kind: "reported", which: "bins", report, applied: apply });
        }
      } catch (error) {
        const message = reason(error, "The server could not be reached.");
        if (live.current) setState({ kind: "failed", message });
      } finally {
        // **Whatever happened above.** A token that outlives the upload it was
        // minted for is a credential nobody is watching.
        if (tokenId) {
          try {
            await api.revokeApiToken(tokenId);
          } catch {
            // It is on the list, labelled and dated. Withdrawable by hand.
          }
        }
      }
    },
    [file, options, which],
  );

  const dismiss = useCallback(() => setState({ kind: "idle" }), []);

  return { state, which, file, options, pick, choose, set, run, dismiss };
}
