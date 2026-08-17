/**
 * Who owns the scanner (D117).
 *
 * D111 wants a locator on every surface. D117 says *"focus is claimed by a
 * screen, never stolen by the chrome."* Both hold once presence and focus are
 * separated: **presence is static**, decided by the route table so the gate can
 * assert it; **focus is arbitrated here**, at runtime.
 *
 * Two rules, and the second is the one worth writing down:
 *
 * 1. The chrome's locator never claims focus. Ever. That is D117 applied
 *    literally, and it is why `ScanInput` gained a `focus` prop rather than
 *    keeping an unconditional `autoFocus` — two of them on one page would both
 *    grab the caret and the last mounted would win.
 * 2. A screen's claim is **suspended while the operator is typing in the
 *    chrome, not stolen back**. Yanking the caret out from under somebody
 *    mid-scan is D117's own bug inverted, and it is the one this has to prevent.
 *
 * The same external-store shape as `app/routing/location.ts`, for the same
 * reason: `useSyncExternalStore` is what React has for state that lives outside
 * it, and it removes the class of bug where a claim made in an event handler is
 * read one render stale.
 */

export type Holder = "screen" | "chrome" | null;

const CHANGED = "nylonite:scan-claim";

let holder: Holder = null;

export function currentHolder(): Holder {
  return holder;
}

export function claim(who: Exclude<Holder, null>): void {
  if (holder === who) return;
  holder = who;
  window.dispatchEvent(new Event(CHANGED));
}

/** Give it back. A release by somebody who does not hold it does nothing. */
export function release(who: Exclude<Holder, null>): void {
  if (holder !== who) return;
  holder = null;
  window.dispatchEvent(new Event(CHANGED));
}

export function subscribe(listener: () => void): () => void {
  window.addEventListener(CHANGED, listener);
  return () => window.removeEventListener(CHANGED, listener);
}
