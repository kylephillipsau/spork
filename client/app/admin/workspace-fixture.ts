import type { Workspace } from "@domain/types";
import type { WorkspaceBench, WorkspaceState } from "./useWorkspace";

/** The workspace as a deployment looks after a bin import. */

const WORKSPACE: Workspace = {
  organisation: {
    id: "01920000-0000-7000-8000-000000000001",
    name: "Harbourline",
    slug: "harbourline",
    active: true,
    created_at: "2026-01-06T09:20:00Z",
  },
  sites: [
    // Perth is the state worth drawing: the import creates a site as soon as
    // a warehouse appears in the export, and leaves its bins out when they
    // state no type. A site with none is expected, not broken.
    { id: "5171e000-0000-0000-0000-000000000001", code: "BRI", name: "Brisbane",
      timezone: "Australia/Brisbane", active: true, locations: 2140, sequenced: 2035, current: false },
    { id: "5171e000-0000-0000-0000-000000000002", code: "PER", name: "Perth",
      timezone: "Australia/Perth", active: true, locations: 0, sequenced: 0, current: false },
    { id: "5171e000-0000-0000-0000-000000000003", code: "MEL", name: "Melbourne",
      timezone: "Australia/Melbourne", active: true, locations: 1620, sequenced: 1620, current: true },
    { id: "5171e000-0000-0000-0000-000000000004", code: "SYD", name: "Sydney",
      timezone: "Australia/Sydney", active: true, locations: 2160, sequenced: 2150, current: false },
  ],
};

export const READY: WorkspaceState = { kind: "ready", workspace: WORKSPACE };
export const EMPTY: WorkspaceState = {
  kind: "ready",
  workspace: { organisation: WORKSPACE.organisation, sites: [] },
};
export const FAILED: WorkspaceState = {
  kind: "failed",
  message: "The server could not be reached.",
};

export function fixtureWorkspace(state: WorkspaceState): WorkspaceBench {
  return { state };
}
