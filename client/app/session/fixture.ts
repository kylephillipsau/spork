import type { Session } from "./SessionContext";
import type { WhereBench, WhereState } from "./useWhere";

/**
 * The session, and the site question, with no network.
 *
 * Four of these five states are hard to reach on purpose. A deployment whose
 * tenant has one warehouse settles silently and shows nothing; one whose person
 * is attached to nowhere is a database problem nobody can reproduce on demand;
 * and the chooser needs a second warehouse that the seed does not have.
 */

const MEL = {
  id: "a5170000-0000-0000-0000-000000000001",
  code: "MEL",
  name: "Melbourne",
  current: false,
};
const SYD = {
  id: "a5170000-0000-0000-0000-000000000009",
  code: "SYD",
  name: "Sydney",
  current: false,
};

const noop = async () => {};

export function fixtureWhere(state: WhereState, over: Partial<WhereBench> = {}): WhereBench {
  return {
    state,
    busy: false,
    problem: null,
    choose: noop,
    dismiss: () => {},
    ...over,
  };
}

/** Two warehouses, so it is a question. */
export const CHOOSE: WhereState = { kind: "choose", sites: [MEL, SYD] };

/** Chosen. What the ordinary single-site deployment reaches without being asked. */
export const SETTLED: WhereState = { kind: "settled", site: { ...MEL, current: true } };

/** Signed in, attached to nowhere. Real, and not the same as loading. */
export const NOWHERE: WhereState = { kind: "nowhere" };

/**
 * A signed-in session for fixture hosts.
 *
 * **This is what keeps the render gate free of a network.** Fixture routes
 * supply it to `SessionProvider`, which then does not fetch — so every screen
 * still draws from literals while the live routes read the server.
 */
export const SIGNED_IN: Session = {
  kind: "signed-in",
  who: {
    person_id: "77770000-0000-0000-0000-000000000001",
    display_name: "Kyle Phillips",
    tenant_id: "11111111-1111-1111-1111-111111111111",
    tenant_name: "Nylonite Pty Ltd",
    site_id: MEL.id,
    site_code: "MEL",
  },
};
