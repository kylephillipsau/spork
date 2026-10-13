import { useCallback, useEffect, useState } from "react";

import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { PackedByPerson } from "@domain/types";

/**
 * Administration (D254): how the workspace's people are doing, for its
 * administrators. Today one read, the orders each person packed, as NetSuite's
 * Packed By says.
 */

export type AdministrationState =
  | { kind: "loading" }
  | { kind: "ready"; packed: PackedByPerson }
  | { kind: "failed"; message: string };

/** The spans the counts come in, as the server sends them. */
export type Span = "today" | "this_week" | "last_30_days";

export interface AdministrationBench {
  state: AdministrationState;
  span: Span;
  choose: (span: Span) => void;
  refreshing: boolean;
  refresh: () => Promise<void>;
}

export function useAdministration(): AdministrationBench {
  const live = useLive();
  const [state, setState] = useState<AdministrationState>({ kind: "loading" });
  const [span, choose] = useState<Span>("today");
  const [refreshing, setRefreshing] = useState(false);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    try {
      const packed = await api.packedBy();
      if (live.current) setState({ kind: "ready", packed });
    } catch (error) {
      if (live.current) setState({ kind: "failed", message: reason(error, "The server could not be reached.") });
    }
    if (live.current) setRefreshing(false);
  }, [live]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { state, span, choose, refreshing, refresh };
}
