import type { SignInBench, SignInState } from "./useSignIn";

/**
 * Signing in, with no network.
 *
 * The company question is the fixture worth having: it needs a person who works
 * for two businesses, which the seed has exactly one of and which no deployment
 * will show on demand. It is also the state the page this replaces could not
 * draw at all.
 *
 * It now has a second version, reached by a key rather than a password: the
 * ceremony was spent answering the question, so carrying on means asking the
 * authenticator again — a different act behind a differently named control, and
 * one that must not want a password it will never send.
 */
const noop = async () => {};

export function fixtureSignIn(
  state: SignInState,
  over: Partial<SignInBench> = {},
): SignInBench {
  return {
    state,
    credentials: { email: "kyle@example.test", password: "a-password" },
    tenant: null,
    busy: false,
    problem: null,
    // A browser that can run a ceremony, because that is the ordinary one. The
    // other is its own fixture: the control is absent and a sentence stands in
    // its place, which is a layout nothing else here draws.
    keys: true,
    via: null,
    remember: false,
    setRemember: () => {},
    type: () => {},
    choose: () => {},
    submit: noop,
    useKey: noop,
    ...over,
  };
}

export const ASKING: SignInState = { kind: "asking" };

/** Two employers, and the server will not choose (D19). */
export const CHOOSE_COMPANY: SignInState = {
  kind: "choose",
  tenants: [
    { tenant_id: "11111111-1111-1111-1111-111111111111", name: "Alpha Foods" },
    { tenant_id: "22222222-2222-2222-2222-222222222222", name: "Beta Supply" },
  ],
};

/** The one generic refusal: a wrong password, an unknown address, a locked
 *  account and an inactive person all land here, because any distinction is an
 *  oracle for whether somebody works here. */
export const REFUSED = "That email and password do not match.";
