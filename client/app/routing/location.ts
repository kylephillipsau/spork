import { normalise, stripBase } from "@domain/routing";

/**
 * Where the browser is, as a store React can subscribe to.
 *
 * The old route table read `window.location.pathname` into `useState` and
 * listened for `popstate`. It never called `pushState`, so nothing could
 * navigate without a full page load — and reading the DOM into state is how a
 * navigation fired in an event handler gets rendered one frame stale.
 * `useSyncExternalStore` is React's answer for exactly this shape.
 */

/** Where the bundle answers. One constant, and the mount move is this line. */
export const BASE: string = import.meta.env.BASE_URL || "/";

/** A private event, because `pushState` fires nothing of its own. */
const CHANGED = "nylonite:navigate";

let snapshot = read();

function read(): string {
  return stripBase(BASE, window.location.pathname);
}

function announce(): void {
  const next = read();
  // Same string, same snapshot: `useSyncExternalStore` compares by identity and
  // a fresh equal string would re-render every subscriber on every popstate.
  if (next !== snapshot) snapshot = next;
  window.dispatchEvent(new Event(CHANGED));
}

/**
 * **Back and Forward come through here too, and they did not.**
 *
 * `subscribe` listened for `popstate` directly, so React was told something had
 * changed and then asked `currentPath()` — which returns `snapshot`, and only
 * `announce` writes it. The value was the one from before the navigation, React
 * compared it as unchanged and rendered nothing: the address bar moved and the
 * screen did not. Nothing noticed while every screen was its own page load and
 * no route had a subject in it; `/findings/:finding` is the first place where
 * Back is the way out of something.
 *
 * One listener at module load rather than one per subscriber, so the snapshot is
 * written once and every subscriber is notified from the same `CHANGED`.
 */
window.addEventListener("popstate", announce);

export function currentPath(): string {
  return snapshot;
}

export function subscribe(listener: () => void): () => void {
  // `popstate` is not listened for here: it is announced above, which is what
  // makes the snapshot right by the time this listener asks for it.
  window.addEventListener(CHANGED, listener);
  return () => window.removeEventListener(CHANGED, listener);
}

/**
 * Go somewhere.
 *
 * `replace` matters in one place and it is not cosmetic: a redirect to sign-in
 * must replace, or Back bounces the operator between the form and the screen
 * that rejected them.
 */
export function go(to: string, options?: { replace?: boolean }): void {
  const url = withBaseFor(to);
  if (options?.replace) window.history.replaceState(null, "", url);
  else window.history.pushState(null, "", url);
  announce();
  // A new screen starts at the top. Arriving already scrolled reads as a
  // rendering fault rather than as a navigation.
  window.scrollTo(0, 0);
}

function withBaseFor(to: string): string {
  const b = normalise(BASE);
  const p = normalise(to);
  if (b === "/") return p;
  return p === "/" ? `${b}/` : `${b}${p}`;
}

/** The href an anchor carries: the route path with the mount put back on. */
export const href = withBaseFor;
