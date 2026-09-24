import type { PasswordBench, PasswordState } from "./usePassword";
import { blankDraft } from "./usePassword";

/**
 * The password screen, reachable with no network.
 *
 * Five states, and four of them are hard to reach on purpose: a mismatch needs
 * two different strings typed, a refusal needs a wrong current password, and
 * the two endings differ only by a number that depends on how many sessions
 * happened to be open. A live screen shows one of these at a time and only when
 * something has gone right or wrong; the gate needs to see all of them.
 */

const who = {
  person_id: "77770000-0000-0000-0000-000000000001",
  display_name: "Kyle Phillips",
  tenant_id: "11111111-1111-1111-1111-111111111111",
  tenant_name: "Spork Pty Ltd",
  site_id: "a5170000-0000-0000-0000-000000000001",
  site_code: "MEL",
};

const noop = async () => {};

export function fixturePassword(
  state: PasswordState,
  over: Partial<PasswordBench> = {},
): PasswordBench {
  return {
    state,
    // **Filled, both copies.** A fixture with the confirmation blank draws the
    // commit key dimmed, which is a real state but the wrong one to make the
    // default picture — the mismatch route already covers a key that cannot be
    // pressed, and this one is what the screen looks like about to be used.
    draft: {
      ...blankDraft(),
      current: "a-current-password",
      next: "a-new-password",
      again: "a-new-password",
    },
    busy: false,
    problem: null,
    mismatched: false,
    type: () => {},
    dismiss: () => {},
    submit: noop,
    ...over,
  };
}

/** The ordinary case: signed in, and here to change it. */
export const READY: PasswordState = { kind: "ready", who };

/** No session. The screen does not offer a form it knows will be refused. */
export const NO_SESSION: PasswordState = {
  kind: "failed",
  message: "You are not signed in.",
};

/** Done, and something else was signed out — the case that makes this worth
 *  having for somebody who thinks a session was stolen. */
export const DONE_ENDED: PasswordState = {
  kind: "done",
  who,
  result: { other_sessions_ended: 3 },
};

/** Done, and nothing else was open. Drawn separately because the sentence is
 *  different and a screen that says "other sessions have been ended" either way
 *  has told the operator nothing. */
export const DONE_ALONE: PasswordState = {
  kind: "done",
  who,
  result: { other_sessions_ended: 0 },
};
