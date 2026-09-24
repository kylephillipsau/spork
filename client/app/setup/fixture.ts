import type { SetupBench, SetupState } from "./useSetup";
import { blankDetails } from "./useSetup";

/**
 * The setup screen, reachable with no network.
 *
 * Four states and every one of them is drawn: the form, the form with no token
 * on the server, the deployment that is already set up, and the one just
 * finished. The gate makes the live screen almost impossible to review — a
 * deployment that needs setting up stops needing it the moment somebody does,
 * and `app.nylonite.com` has people in it and correctly never shows this — so
 * fixtures are the only way anybody sees three of these four again.
 */

const filled = {
  ...blankDetails(),
  token: "9f2c41d7a0b3e58c6d1f0a2b4c7e8d93f1a05b6c2d3e4f50617283940a1b2c3d",
  organisation: "Spork Pty Ltd",
  siteName: "Melbourne",
  siteCode: "MEL",
  timezone: "Australia/Melbourne",
  displayName: "Kyle Phillips",
  email: "kyle@example.test",
  password: "a-long-enough-password",
};

const noop = async () => {};

export function fixtureSetup(
  state: SetupState,
  over: Partial<SetupBench> = {},
): SetupBench {
  return {
    state,
    details: filled,
    busy: false,
    problem: null,
    type: () => {},
    dismiss: () => {},
    submit: noop,
    ...over,
  };
}

/** The ordinary case: nobody here, and a token waiting in the log. */
export const NEEDED: SetupState = {
  kind: "needed",
  status: { required: true, token_ready: true },
};

/** Nobody here, and the token has expired — the screen says restart rather
 *  than asking for something that cannot exist. */
export const NO_TOKEN: SetupState = {
  kind: "needed",
  status: { required: true, token_ready: false },
};

/** Somebody is already in it. The state every real deployment is in five
 *  minutes after its first one, and the one nobody would otherwise see. */
export const CLOSED: SetupState = { kind: "closed" };

/** Just finished. */
export const DONE: SetupState = {
  kind: "done",
  result: {
    tenant_id: "11111111-1111-1111-1111-111111111111",
    site_id: "a5170000-0000-0000-0000-000000000001",
    person_id: "77770000-0000-0000-0000-000000000001",
  },
};
