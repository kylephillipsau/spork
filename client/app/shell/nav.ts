/**
 * The sidebar, as data (D171).
 *
 * Grouped by the job, as the work rail was (D110), under the names a
 * warehouse uses. Account and the site are not here: they belong to the
 * header's user menu and site switcher, where every desktop app keeps them.
 */

import type { LucideIcon } from "lucide-react";
import {
  ArrowDownToLine,
  Building2,
  Camera,
  ClipboardList,
  KeyRound,
  LayoutDashboard,
  Package,
  PackageOpen,
  ScanLine,
  Scale,
  TriangleAlert,
  Truck,
  Upload,
} from "lucide-react";

import type { WorkWaiting } from "@domain/types";

export type Badge = Exclude<keyof WorkWaiting, "no_site">;

export interface NavItem {
  /** The manifest id of the screen it opens. */
  readonly id: string;
  readonly label: string;
  readonly path: string;
  readonly icon: LucideIcon;
  /** Which `/work` count to show beside it (D112: work waiting here, now). */
  readonly badge?: Badge;
}

export interface NavGroup {
  /** Null for the ungrouped items at the top. */
  readonly label: string | null;
  readonly items: readonly NavItem[];
}

export const NAV: readonly NavGroup[] = [
  {
    label: null,
    items: [{ id: "home", label: "Dashboard", path: "/", icon: LayoutDashboard }],
  },
  {
    label: "Outbound",
    items: [
      { id: "orders", label: "Orders", path: "/orders", icon: ClipboardList },
      { id: "picking", label: "Picking", path: "/picking", icon: ScanLine, badge: "pick" },
      { id: "pack", label: "Packing", path: "/pack", icon: Package, badge: "pack" },
      { id: "despatch", label: "Despatch", path: "/despatch", icon: Truck, badge: "despatch" },
    ],
  },
  {
    label: "Inbound",
    items: [
      { id: "receiving", label: "Receiving", path: "/receiving", icon: PackageOpen },
      { id: "putaway", label: "Put away", path: "/putaway", icon: ArrowDownToLine },
    ],
  },
  {
    label: "Inventory",
    items: [
      { id: "findings", label: "Findings", path: "/findings", icon: TriangleAlert, badge: "findings" },
      { id: "weigh", label: "Weigh", path: "/weigh", icon: Scale },
      { id: "capture", label: "Capture", path: "/capture", icon: Camera },
    ],
  },
];

/** Pinned to the bottom of the sidebar. */
export const SETTINGS: NavGroup = {
  label: "Settings",
  items: [
    { id: "workspace", label: "Workspace", path: "/workspace", icon: Building2 },
    { id: "import", label: "Import", path: "/import", icon: Upload },
    { id: "tokens", label: "Import tokens", path: "/tokens", icon: KeyRound },
  ],
};

/** Reached from the user menu rather than the sidebar. */
export const USER_MENU_SCREENS = ["account", "keys"] as const;

/**
 * Reached another way, so deliberately in no menu: a job opened from its
 * queue, a finding opened from the list, and the screens before sign-in.
 */
export const REACHED_ANOTHER_WAY = ["pack-one", "finding", "sign-in", "setup", "where"] as const;

export function allItems(): NavItem[] {
  return [...NAV, SETTINGS].flatMap((g) => g.items);
}

/** The item for a path: the longest item path that prefixes it. */
export function currentItem(path: string): { item: NavItem; group: NavGroup } | null {
  let best: { item: NavItem; group: NavGroup } | null = null;
  for (const group of [...NAV, SETTINGS]) {
    for (const item of group.items) {
      const hit = item.path === "/" ? path === "/" : path === item.path || path.startsWith(item.path + "/");
      if (hit && (!best || item.path.length > best.item.path.length)) best = { item, group };
    }
  }
  return best;
}
