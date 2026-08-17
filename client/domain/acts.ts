/**
 * One press, and every attempt at it.
 *
 * # The bug this exists to make unwritable
 *
 * D5: *"Every write carries a `client_event_id` the client mints. The ledger is
 * append-only and idempotent by design: sending the same act twice changes
 * nothing … so the identifier is generated here, before the request, **and
 * reused if it has to be sent again**."*
 *
 * It was not reused. `client_event_id: uuid()` sat inside the request body at
 * eleven call sites, so every *invocation* minted a new one — and the only
 * thing that retries in this client is an operator pressing again after a
 * failure. Which means the three conditions were: the request reaches the
 * server and is recorded, the response is lost, the operator presses again.
 * The second press then arrived wearing a different name and the server had no
 * way to know it was the same act. Two observations for one measurement. Two
 * picks. An adjustment applied twice, on a ledger that is append-only and
 * therefore cannot be corrected — only compensated by a further act.
 *
 * A browser at a bench on office wifi rarely loses a response. A phone in an
 * aisle does, which is what made this worth fixing before the write path a
 * phone is for.
 *
 * # An act is not a request
 *
 * The unit here is what the operator would call one thing they did, which is
 * sometimes not one call: the bench's weigh sends a weight off an instrument
 * and a height off a keyboard as two `observation_event`s, because `method` is
 * on the event and one event cannot say both. So the key names the act rather
 * than the button — `weigh:{carton}:weight` and `weigh:{carton}:height` are two
 * acts from one press, and both are stable across a retry.
 *
 * And it is not only the event id. `POST /allocations` is *"retry-safe on a
 * client-minted `id` — replay returns the prior claim, a reused id with a
 * different body is refused"*, and `POST /packages` takes one too. Everything
 * the client mints for one act has to be the same on the second attempt, or the
 * retry is a different act in every way that counts. Hence `id(name)` rather
 * than a bare `client_event_id`.
 *
 * # And not only the ids
 *
 * `occurred_at` moved too, and the reason is weaker than it first looks —
 * checked rather than assumed. `client_events.rs` mentions `occurred_at`
 * nowhere: the replay branches load the prior facts and return them, so a
 * stable id carrying a moving clock is **silently absorbed** rather than
 * refused. The second timestamp is simply discarded.
 *
 * It is hoisted anyway, because the column means *when the operator acted* and
 * not *when this attempt was sent*, and a client that re-reads the clock per
 * attempt is a client that would record the wrong moment the first time
 * anything does compare it. `reject_allocation_mismatch` is the shape that
 * would — *"A retry is the same act arriving twice, not a second act wearing
 * the first one's name"* — and it compares `stock_id`, `fulfilment_line_id`
 * and `quantity`, because the allocation body has no timestamp to compare.
 *
 * An act has an identity **and a moment**, read once. The identity is what the
 * server checks today; the moment is what makes the record true.
 *
 * Pure and free of React, so `node --test` can read it: the rule is the
 * testable part and the hooks are the plumbing.
 */
import type { Uuid } from "./types";

export interface Act {
  /**
   * A uuid this act mints, by name, and the same one on every attempt at it.
   *
   * `event` is the `client_event_id` every ledger write carries. A row the
   * client names — an allocation's `id`, a package's — is any other name.
   */
  id: (name: string) => Uuid;
  /**
   * When the operator acted, which is **not** when this attempt was sent.
   * Re-reading the clock on a retry is what turns a replay into a mismatch.
   */
  at: string;
}

/** Everything one screen is in the middle of doing. */
export interface Pressing {
  /**
   * The act this press is. The same act until [`landed`] is told it worked.
   *
   * The key is *(what is being done, to what)* — `accept:{finding}`,
   * `carton:{fulfilment}`. Pressing the same thing again is the same act;
   * pressing it against something else is not.
   */
  attempt: (key: string) => Act;
  /** It landed. The next press against that key is a different act. */
  landed: (key: string) => void;
  /** How many acts are open. For a test, and for nothing else. */
  open: () => number;
}

const uuid = (): Uuid => crypto.randomUUID();
const now = (): string => new Date().toISOString();

/**
 * A fresh act.
 *
 * The clock and the id source are arguments so a test can be about the rule
 * rather than about entropy — nothing in the application passes them.
 */
export function anAct(mintId: () => Uuid = uuid, clock: () => string = now): Act {
  const minted = new Map<string, Uuid>();
  const at = clock();
  return {
    id: (name) => {
      const found = minted.get(name);
      if (found !== undefined) return found;
      const fresh = mintId();
      minted.set(name, fresh);
      return fresh;
    },
    at,
  };
}

/**
 * Somewhere for a screen to keep the acts it has not landed yet.
 *
 * Held per screen rather than globally, which is the honest boundary: an
 * operator who navigates away is not retrying that press. **A reload loses
 * them**, and that is a real limit rather than a bug — surviving one means
 * persisting acts to storage, which is a larger question than this and belongs
 * with D5's offline story rather than tucked in here.
 */
export function pressing(mint: () => Act = () => anAct()): Pressing {
  const held = new Map<string, Act>();
  return {
    attempt: (key) => {
      const found = held.get(key);
      if (found !== undefined) return found;
      const fresh = mint();
      held.set(key, fresh);
      return fresh;
    },
    landed: (key) => {
      held.delete(key);
    },
    open: () => held.size,
  };
}
