import { useCallback, useEffect, useState } from "react";
import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { SiteRow, Uuid } from "@domain/types";

/**
 * Where you are working, as logic.
 *
 * # Why this is not part of signing in
 *
 * It looks like it should be, and the first draft of the plan assumed it had to
 * be. It does not. `client_events` copies the session's site into each act at
 * write time and nothing ever re-reads the session's copy, so a site chosen a
 * moment *after* authenticating is exactly as sound for D11's non-repudiable
 * floor as one chosen during it.
 *
 * That is what lets the sign-in page — which runs before a session exists and
 * carries the passkey ceremony — stay last in the migration while the thing it
 * was blocking ships now. Every badge and the landing screen are defined as
 * "work waiting for you, at this site" (D112), and until this existed there was
 * no site to name.
 *
 * # One site is not a question
 *
 * A tenant with one warehouse gets it chosen silently. Asking somebody to
 * confirm the only possible answer is a screen that exists to be dismissed, and
 * the seed — one site per tenant — means nobody sees this today at all.
 */

export type WhereState =
  | { kind: "asking" }
  /** More than one place, so it is a question. */
  | { kind: "choose"; sites: SiteRow[] }
  /** Chosen, silently or otherwise. The caller reloads the session. */
  | { kind: "settled"; site: SiteRow }
  /** Signed in and attached to nowhere. A real state, and not the same as
   *  loading — the answer is somebody with database access, not a retry. */
  | { kind: "nowhere" }
  | { kind: "failed"; message: string };

export interface WhereBench {
  state: WhereState;
  busy: boolean;
  problem: string | null;
  choose: (site: Uuid) => Promise<void>;
  dismiss: () => void;
}

export function useWhere(onSettled?: () => void): WhereBench {
  const [state, setState] = useState<WhereState>({ kind: "asking" });
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);

  const live = useLive();

  const settle = useCallback(
    async (site: SiteRow) => {
      await api.chooseSite({ site_id: site.id });
      if (!live.current) return;
      setState({ kind: "settled", site });
      onSettled?.();
    },
    [onSettled],
  );

  const ask = useCallback(async () => {
    try {
      const sites = await api.sites();
      if (!live.current) return;

      const current = sites.find((s) => s.current);
      if (current) {
        setState({ kind: "settled", site: current });
        return;
      }
      if (sites.length === 0) {
        setState({ kind: "nowhere" });
        return;
      }
      // The only possible answer is not a question.
      if (sites.length === 1 && sites[0]) {
        await settle(sites[0]);
        return;
      }
      setState({ kind: "choose", sites });
    } catch (error) {
      if (!live.current) return;
      setState({
        kind: "failed",
        message: reason(error, "The server could not be reached."),
      });
    }
  }, [settle]);

  useEffect(() => {
    void ask();
  }, [ask]);

  return {
    state,
    busy,
    problem,
    dismiss: () => setProblem(null),
    choose: async (id) => {
      if (busy || state.kind !== "choose") return;
      const site = state.sites.find((s) => s.id === id);
      if (!site) return;
      setBusy(true);
      setProblem(null);
      try {
        await settle(site);
      } catch (error) {
        if (live.current) {
          setProblem(reason(error, "Request failed."));
        }
      } finally {
        if (live.current) setBusy(false);
      }
    },
  };
}
