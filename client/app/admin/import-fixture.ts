import type { ImportReport } from "@domain/types";
import type { ImportBench, ImportState } from "./useImport";

/**
 * The bin import, with no network.
 *
 * The numbers are a worked example at the scale this is built for: 6,310 rows
 * in, 5,980 bins out, 330 left out. A fixture whose numbers do not add up is a
 * fixture nobody can check against a terminal.
 */

const REPORT: ImportReport = {
  survey: {
    bins: 6310,
    sequenced: 5870,
    zeroed: 112,
    disagree: 3540,
    untyped: 322,
    sites: [
      { warehouse: "Brisbane Warehouse", bins: 2140, typed: 2140, untyped: 0,
        note: "Australia/Brisbane", skipped: false },
      { warehouse: "Partner Warehouse", bins: 8, typed: 8, untyped: 0,
        note: "not yours? skipped", skipped: true },
      { warehouse: "Bonded Store", bins: 1, typed: 0, untyped: 1,
        note: "not yours? skipped", skipped: true },
      { warehouse: "Perth Warehouse", bins: 321, typed: 0, untyped: 321,
        note: "Australia/Perth", skipped: false },
      { warehouse: "Melbourne Warehouse", bins: 1620, typed: 1620, untyped: 0,
        note: "Australia/Melbourne", skipped: false },
      { warehouse: "Sydney Warehouse", bins: 2220, typed: 2220, untyped: 0,
        note: "Australia/Sydney", skipped: false },
    ],
  },
  loaded: {
    sites_created: 3,
    sites_matched: 1,
    bins_created: 5980,
    bins_corrected: 0,
    bins_left_out: 330,
    applied: false,
  },
  // A dry run keeps no arrival, because the loader rolls its writes back and a
  // receipt that outlived that rollback would say a file arrived which was
  // never kept. It still reports one when the bytes are already on file.
  arrival: null,
};

export const IDLE: ImportState = { kind: "idle" };
export const DRY: ImportState = { kind: "reported", which: "bins", report: REPORT, applied: false };
export const APPLIED: ImportState = {
  kind: "reported",
  which: "bins",
  report: {
    ...REPORT,
    loaded: { ...REPORT.loaded, applied: true },
    arrival: { party_message_id: "01991f3a-4c2e-7b10-9d44-6c1f0a83be27", replay: false },
  },
  applied: true,
};

/** The item master: nine thousand codes, and the second run of it. */
export const ITEMS: ImportState = {
  kind: "reported",
  which: "items",
  report: {
    survey: { items: 9016, unnamed: 0, duplicated: 2 },
    loaded: { items_created: 9016, items_present: 0, applied: false },
    arrival: null,
  },
  applied: false,
};
export const FAILED: ImportState = {
  kind: "failed",
  message: "no rows with a `Bin Number` — is this the bin export?",
};

export function fixtureImport(state: ImportState, file = "bins.csv"): ImportBench {
  const which = state.kind === "reported" ? state.which : "bins";
  return {
    state,
    which,
    file: state.kind === "idle" ? null : ({ name: which === "items" ? "items.csv" : file } as File),
    options: { assumeKind: "", includeExternal: false },
    pick: () => {},
    choose: () => {},
    set: () => {},
    run: async () => {},
    dismiss: () => {},
  };
}
