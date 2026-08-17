/**
 * Navigation by work, not by entity.
 *
 * D110: *"the entity-oriented menu is the disease. Orders. Fulfilments.
 * Packages. Items. Locations. Stock. It is a rendering of the schema, it
 * requires the operator to know which noun holds the thing they want, and it is
 * why finding one order takes four navigations today."*
 *
 * So the groups are jobs. The grouping is load-bearing rather than decorative:
 * it means a new screen has an obvious home, and it means somebody who knows
 * the warehouse but not the software can read it.
 *
 * # Absent, not disabled
 *
 * A destination with no screen behind it is not in this list. A greyed row is a
 * promise, and a rail full of promises is a rail people stop reading — the same
 * argument D112 makes about a badge showing nought. `Inbound` and `Reference`
 * are missing entirely for that reason, and their absence is visible, which is
 * the point.
 *
 * # `external` is what keeps the migration honest
 *
 * Eleven maud pages are still standing, and D113's doctrine is that each one
 * lives until its replacement ships. A rail entry pointing at one is marked
 * external so the router's click interceptor lets the browser handle it — the
 * alternative is a client-side 404 on a page that works perfectly well.
 */

export type Group = "inbound" | "outbound" | "integrity" | "reference" | "admin" | "you";

/** Which count, if any, belongs beside a destination. */
export type BadgeKey = "pack" | "pick" | "despatch" | "findings";

export interface Destination {
  /** Matches `Screen.id` for anything the router owns. */
  readonly id: string;
  readonly label: string;
  /** Param-free: a rail entry goes to a list, never to one subject. */
  readonly path: string;
  readonly badge?: BadgeKey;
  /** A maud page, still standing. The interceptor falls through to it. */
  readonly external?: boolean;
  /** Not somewhere to go: something to do. The rail draws it as a control. */
  readonly act?: boolean;
}

export interface RailGroup {
  readonly group: Group;
  readonly label: string;
  readonly items: readonly Destination[];
}

export const RAIL: readonly RailGroup[] = [
  {
    // **The group `Group` has always named and nothing has ever filled.** Goods
    // are checked in at the dock and put away afterwards, so put-away is a
    // daily step here rather than an exception — and it is the first inbound
    // screen this system has had. Receiving itself is still an API call.
    group: "inbound",
    label: "Inbound",
    items: [
      { id: "receiving", label: "Receive", path: "/receiving" },
      { id: "putaway", label: "Put away", path: "/putaway" },
    ],
  },
  {
    group: "outbound",
    label: "Outbound",
    items: [
      { id: "picking", label: "Pick", path: "/picking", badge: "pick" },
      { id: "pack", label: "Pack", path: "/pack", badge: "pack" },
      { id: "despatch", label: "Despatch", path: "/despatch", badge: "despatch" },
      { id: "orders", label: "Orders", path: "/orders" },
    ],
  },
  {
    group: "integrity",
    label: "Integrity",
    items: [
      // The group that does not exist in comparable products, and where this
      // system's premise lives. First in its own group for that reason.
      { id: "findings", label: "Findings", path: "/findings", badge: "findings" },
      { id: "weigh", label: "Weigh", path: "/weigh" },
      { id: "capture", label: "Capture", path: "/capture" },
    ],
  },
  {
    group: "admin",
    label: "Administration",
    items: [
      { id: "workspace", label: "Workspace", path: "/workspace" },
      { id: "import", label: "Import", path: "/import" },
      { id: "tokens", label: "Import tokens", path: "/tokens" },
    ],
  },
  {
    group: "you",
    label: "You",
    items: [
      { id: "where", label: "Where you are", path: "/where" },
      { id: "account", label: "Password", path: "/account" },
      { id: "keys", label: "Passkeys", path: "/keys" },
      // **An act, not a destination.** Signing out revokes the session record
      // — the thing a server-side session buys over a signed claim — so it is
      // a call rather than a page to visit. The rail draws it last and the
      // shell turns it into one.
      { id: "sign-out", label: "Sign out", path: "/sign-out", act: true },
    ],
  },
];

/**
 * Screens the rail deliberately does not carry, and how you reach them.
 *
 * **Written down so that adding a screen forces a decision.** A screen nobody
 * can reach is the demo failure in miniature, and it is invisible unless
 * something asks the question — so `everyScreenIsReachable` below asks it, and
 * this is where the answers live.
 */
export const REACHED_ANOTHER_WAY: Readonly<Record<string, string>> = {
  home: "the wordmark, on every surface",
  "pack-one":
    "a commitment on the order screen, or a scan — a rail entry cannot name one fulfilment",
  finding:
    "a row on the findings queue, or a link somebody sent — a rail entry cannot name one finding",
  setup: "printed in the server's log; a deployment with anybody in it has no use for it",
};

/** Every destination, flattened. */
export function destinations(): readonly Destination[] {
  return RAIL.flatMap((g) => g.items);
}

/**
 * Which entry is "here".
 *
 * Prefix-matched rather than compared, because `/pack/f01f…` is still Pack —
 * that is the whole reason this is a function and not `===`.
 */
export function currentDestination(path: string): Destination | null {
  let best: Destination | null = null;
  for (const d of destinations()) {
    if (d.external || d.act) continue;
    if (path === d.path || path.startsWith(`${d.path}/`)) {
      // Longest wins, so a nested destination beats its parent.
      if (!best || d.path.length > best.path.length) best = d;
    }
  }
  return best;
}

/**
 * Every screen is reachable, or says how it is reached instead.
 *
 * Returns the ids that are neither in the rail nor accounted for. Used by a
 * test rather than at runtime: the answer must be empty when the code is
 * written, not discovered by an operator who cannot find a screen.
 */
export function unreachable(screenIds: readonly string[]): string[] {
  const inRail = new Set(destinations().map((d) => d.id));
  // Sign-in is not in the rail and must not be: it is where the gate sends
  // somebody who has none, and a rail is drawn only for somebody who has one.
  inRail.add("sign-in");
  return screenIds.filter((id) => !inRail.has(id) && !(id in REACHED_ANOTHER_WAY));
}

/** Rail entries pointing at a path the router does not have and is not maud. */
export function deadLinks(livePaths: readonly string[]): string[] {
  const known = new Set(livePaths);
  return destinations()
    .filter((d) => !d.external && !d.act && !known.has(d.path))
    .map((d) => d.path);
}
