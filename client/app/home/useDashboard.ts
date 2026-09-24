import { useCallback, useEffect, useState } from "react";

import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { DiscrepancyRow, OrderMatch, PackJob, WorkWaiting } from "@domain/types";

/** One read on the dashboard. Each loads and fails on its own. */
export type Read<T> = { kind: "loading" } | { kind: "ready"; data: T } | { kind: "failed"; message: string };

export interface DashboardBench {
  work: Read<WorkWaiting>;
  queue: Read<PackJob[]>;
  findings: Read<DiscrepancyRow[]>;
  orders: Read<OrderMatch[]>;
  refreshing: boolean;
  refresh: () => Promise<void>;
}

const LOADING = { kind: "loading" } as const;

/**
 * The dashboard's four reads, in parallel (D171, phase D).
 *
 * The counts are `/work` — the same read as the sidebar badges, so the two
 * agree by construction (D112). The lists are the reads the Packing, Findings
 * and Orders screens already make; the dashboard shows the top of each.
 */
export function useDashboard(): DashboardBench {
  const live = useLive();
  const [work, setWork] = useState<Read<WorkWaiting>>(LOADING);
  const [queue, setQueue] = useState<Read<PackJob[]>>(LOADING);
  const [findings, setFindings] = useState<Read<DiscrepancyRow[]>>(LOADING);
  const [orders, setOrders] = useState<Read<OrderMatch[]>>(LOADING);
  const [refreshing, setRefreshing] = useState(false);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    const settle = <T,>(set: (r: Read<T>) => void, p: Promise<T>) =>
      p.then(
        (data) => {
          if (live.current) set({ kind: "ready", data });
        },
        (error: unknown) => {
          if (live.current) set({ kind: "failed", message: reason(error, "Could not be loaded.") });
        },
      );
    await Promise.all([
      settle(setWork, api.work()),
      settle(setQueue, api.packingQueue("")),
      settle(setFindings, api.findings("open,investigating")),
      settle(setOrders, api.latestOrders()),
    ]);
    if (live.current) setRefreshing(false);
  }, [live]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { work, queue, findings, orders, refreshing, refresh };
}
