import type { AdministrationBench, AdministrationState, Span } from "./useAdministration";

/** Who packed what, with no network: three people and some nobody named. */
export const ADMINISTRATION_READY: AdministrationState = {
  kind: "ready",
  packed: {
    packers: [
      { packed_by: "Sam Rivera", today: 21, this_week: 64, last_30_days: 248 },
      { packed_by: "Jo Okafor", today: 18, this_week: 71, last_30_days: 239 },
      { packed_by: "Alex Moreau", today: 0, this_week: 4, last_30_days: 12 },
      { packed_by: null, today: 1, this_week: 2, last_30_days: 9 },
    ],
    as_at: "2026-10-13T03:55:00Z",
    no_site: false,
  },
};

/** Before the Bridge has sent anything. */
export const ADMINISTRATION_EMPTY: AdministrationState = {
  kind: "ready",
  packed: { packers: [], as_at: null, no_site: false },
};

export function fixtureAdministration(state: AdministrationState, span: Span = "today"): AdministrationBench {
  return { state, span, choose: () => {}, refreshing: false, refresh: async () => {} };
}
