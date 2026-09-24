import type { DashboardBench } from "./useDashboard";
import { QUEUE } from "@app/outbound/pack/queue-fixture";
import { FINDINGS_FIXTURE } from "@app/integrity/findings/fixture";
import { LATEST } from "@app/outbound/orders/fixture";

const noop = async () => {};

/** The dashboard with every panel filled, from the screens' own fixtures. */
export const DASHBOARD: DashboardBench = {
  work: { kind: "ready", data: { pack: 4, pick: 12, despatch: 1, findings: 3, no_site: false } },
  queue: { kind: "ready", data: QUEUE.kind === "ready" ? QUEUE.jobs : [] },
  findings: { kind: "ready", data: FINDINGS_FIXTURE.filter((f) => f.state === "open" || f.state === "investigating") },
  orders: { kind: "ready", data: LATEST.kind === "listed" ? LATEST.orders : [] },
  refreshing: false,
  refresh: noop,
};

export const DASHBOARD_LOADING: DashboardBench = {
  work: { kind: "loading" },
  queue: { kind: "loading" },
  findings: { kind: "loading" },
  orders: { kind: "loading" },
  refreshing: true,
  refresh: noop,
};
