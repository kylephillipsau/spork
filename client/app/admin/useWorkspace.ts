import { useCallback, useEffect, useState } from "react";
import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { Workspace } from "@domain/types";

/**
 * The organisation and its warehouses, and the two things a site says about
 * packing: where it packs, and whose stock it holds (migrations 95 and 97).
 * Both are settings rather than acts, so a change is followed by a re-read
 * and nothing is kept locally.
 */

export type WorkspaceState =
  | { kind: "loading" }
  | { kind: "ready"; workspace: Workspace }
  | { kind: "failed"; message: string };

export interface WorkspaceBench {
  state: WorkspaceState;
  busy: boolean;
  /** Why the last change was refused, until dismissed. */
  problem: string | null;
  dismiss: () => void;
  setPackLocation: (siteId: string, code: string) => Promise<void>;
  setOwner: (siteId: string) => Promise<void>;
}

export function useWorkspace(): WorkspaceBench {
  const [state, setState] = useState<WorkspaceState>({ kind: "loading" });
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);

  const live = useLive();

  const read = useCallback(async () => {
    try {
      const workspace = await api.workspace();
      if (live.current) setState({ kind: "ready", workspace });
    } catch (error) {
      const message = reason(error, "The server could not be reached.");
      if (live.current) setState({ kind: "failed", message });
    }
  }, [live]);

  useEffect(() => {
    void read();
  }, [read]);

  const change = useCallback(
    async (act: () => Promise<unknown>, failure: string) => {
      setBusy(true);
      setProblem(null);
      try {
        await act();
        await read();
      } catch (error) {
        if (live.current) setProblem(reason(error, failure));
      } finally {
        if (live.current) setBusy(false);
      }
    },
    [read, live],
  );

  const setPackLocation = useCallback(
    (siteId: string, code: string) => change(() => api.setPackLocation(siteId, code), "Could not set where this site packs."),
    [change],
  );
  const setOwner = useCallback(
    (siteId: string) => change(() => api.setSiteOwner(siteId), "Could not set who owns this site's stock."),
    [change],
  );

  return { state, busy, problem, dismiss: () => setProblem(null), setPackLocation, setOwner };
}
