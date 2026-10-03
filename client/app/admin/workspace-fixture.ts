import type { PackageTypeRow, Workspace } from "@domain/types";
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
      timezone: "Australia/Brisbane", active: true, locations: 2140, sequenced: 2035, current: false,
      pack_location: "DESP-1", owner: "Harbourline" },
    { id: "5171e000-0000-0000-0000-000000000002", code: "PER", name: "Perth",
      timezone: "Australia/Perth", active: true, locations: 0, sequenced: 0, current: false,
      pack_location: null, owner: null },
    { id: "5171e000-0000-0000-0000-000000000003", code: "MEL", name: "Melbourne",
      timezone: "Australia/Melbourne", active: true, locations: 1620, sequenced: 1620, current: true,
      pack_location: null, owner: null },
    { id: "5171e000-0000-0000-0000-000000000004", code: "SYD", name: "Sydney",
      timezone: "Australia/Sydney", active: true, locations: 2160, sequenced: 2150, current: false,
      pack_location: "PACK", owner: "Harbourline" },
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

const noop = async () => {};

/** Boxes as a prepack import leaves them: a shovel box is one nobody wants suggested (D196). */
const box = (name: string, code: string | null, size: [number, number, number] | null, suggested = true): PackageTypeRow => ({
  id: `9a7e0000-0000-0000-0000-${name.replace(/[^a-z]/gi, "").padEnd(12, "0").slice(0, 12).toLowerCase().replace(/[^0-9a-f]/g, "0")}`,
  name,
  carrier_package_code: code,
  dimensions_fixed: size !== null,
  length_mm: size?.[0] ?? null,
  width_mm: size?.[1] ?? null,
  height_mm: size?.[2] ?? null,
  tare_weight_g: null,
  reusable: false,
  max_payload_g: null,
  tenant_owned: true,
  suggested,
});

export const BOXES: PackageTypeRow[] = [
  box("Extra Small Box", "CTN", [320, 230, 160]),
  box("Small Box", "CTN", [390, 310, 300]),
  box("Medium Box", "CTN", [450, 340, 410]),
  { ...box("Large Box", "CTN", [660, 440, 460]), max_payload_g: 25000 },
  box("Shovel Box", "CTN", [1400, 340, 400], false),
  box("Satchel", null, null),
  box("Pallet", "PAL", [1200, 1200, 1300]),
];

export function fixtureWorkspace(state: WorkspaceState): WorkspaceBench {
  return {
    state,
    busy: false,
    problem: null,
    dismiss: () => {},
    setPackLocation: noop,
    setOwner: noop,
    boxes: state.kind === "ready" ? BOXES : null,
    suggest: noop,
    boxWeight: noop,
  };
}
