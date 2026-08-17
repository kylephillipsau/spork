/**
 * Paths, as a pure function.
 *
 * The client had no router: a `Record` keyed on literal pathnames, matched
 * against `window.location.pathname`, listening for `popstate` and never
 * calling `pushState`. Every navigation was a full page load typed by hand,
 * and every unmatched path fell through to a fixture.
 *
 * D135 deferred the router until *"a screen whose state is on the server — a
 * fulfilment, a receipt, a finding — where a deep link restores everything it
 * names."* Findings is that screen and it is built, so this is due.
 *
 * # Why this rather than a router library
 *
 * The fraction of one this client would use is the fraction that is cheap to
 * write: match, params, and a 404. There are no nested layouts — a shell is
 * chosen per route rather than composed — no lazy routes, because D130's cache
 * contract wants one document naming today's hashes, and no loaders, because
 * this codebase already decided how fetching works and a second idiom would be
 * two ways to do one thing.
 *
 * What it costs is the edge cases a library has already met. Two of them bite
 * here and both are handled below: a base path, so this is correct under `/ui/`
 * and under `/` and the mount can move in one line; and match order, which is
 * declaration order and is asserted rather than left to chance.
 *
 * Everything here is pure and has no DOM in it, which is the point. The history
 * and the React are next door in `app/routing/`.
 */

export type Params = Readonly<Record<string, string>>;

type Segment = { kind: "literal"; text: string } | { kind: "param"; name: string };

export interface Pattern {
  /** As written: `/pack/:fulfilment`. Kept for messages and for `build`. */
  readonly source: string;
  readonly segments: readonly Segment[];
}

/** Compile once, at module load. Never per render. */
export function pattern(source: string): Pattern {
  const segments = split(source).map<Segment>((text) =>
    text.startsWith(":") ? { kind: "param", name: text.slice(1) } : { kind: "literal", text },
  );
  return { source, segments };
}

function split(path: string): string[] {
  return path.split("/").filter((s) => s.length > 0);
}

/**
 * Collapse repeated slashes and drop a trailing one, except at the root.
 *
 * Case is left alone. A path is not a word and lowercasing one would make
 * `/pack/F01F…` and `/pack/f01f…` the same route while the server treats them
 * as different subjects.
 */
export function normalise(path: string): string {
  const parts = split(path);
  return parts.length === 0 ? "/" : `/${parts.join("/")}`;
}

/**
 * Remove the mount from a path.
 *
 * **This is what lets the router land before the mount moves.** The bundle
 * answers under `/ui/` today and under `/` shortly; written this way it is
 * correct under both, and the change is one constant rather than a rewrite.
 */
export function stripBase(base: string, path: string): string {
  const b = normalise(base);
  if (b === "/") return normalise(path);
  const p = normalise(path);
  if (p === b) return "/";
  return p.startsWith(`${b}/`) ? normalise(p.slice(b.length)) : p;
}

/** Put the mount back on. The inverse of `stripBase`, for hrefs. */
export function withBase(base: string, path: string): string {
  const b = normalise(base);
  const p = normalise(path);
  if (b === "/") return p;
  return p === "/" ? `${b}/` : `${b}${p}`;
}

/**
 * The params this pattern draws out of the path, or `null` for no match.
 *
 * A param matches exactly one segment and is percent-decoded; a malformed
 * escape is left as written rather than throwing, because a URI somebody typed
 * badly is a 404 and not a crash.
 */
export function match(p: Pattern, path: string): Params | null {
  const parts = split(path);
  if (parts.length !== p.segments.length) return null;
  const params: Record<string, string> = {};
  for (let i = 0; i < parts.length; i++) {
    const seg = p.segments[i];
    const part = parts[i];
    if (seg === undefined || part === undefined) return null;
    if (seg.kind === "literal") {
      if (seg.text !== part) return null;
    } else {
      params[seg.name] = decode(part);
    }
  }
  return params;
}

function decode(s: string): string {
  try {
    return decodeURIComponent(s);
  } catch {
    return s;
  }
}

/**
 * The path this pattern names, with its params filled in.
 *
 * **The only way a link is built.** Concatenating a path by hand is how a route
 * and the thing linking to it drift apart, and the components are encoded here
 * so an identifier containing a slash cannot invent a segment.
 */
export function build(p: Pattern, params: Params = {}): string {
  const parts = p.segments.map((seg) => {
    if (seg.kind === "literal") return seg.text;
    const value = params[seg.name];
    if (value === undefined) {
      throw new Error(`${p.source} needs a ${seg.name}`);
    }
    return encodeURIComponent(value);
  });
  return parts.length === 0 ? "/" : `/${parts.join("/")}`;
}

export interface Located<T> {
  readonly route: T;
  readonly params: Params;
}

/**
 * The first route that matches, in declaration order.
 *
 * **Declaration order, decided rather than configurable.** Specificity sorting
 * is machinery for a problem the table's author solves by putting the literal
 * above the pattern — `/orders/new` before `/orders/:order`. A rule nobody can
 * see in the table is a rule that surprises somebody later.
 */
export function resolve<T extends { pattern: Pattern }>(
  routes: readonly T[],
  path: string,
): Located<T> | null {
  for (const route of routes) {
    const params = match(route.pattern, path);
    if (params !== null) return { route, params };
  }
  return null;
}

/** Whether a string could be an identifier this system mints. */
export function isUuid(s: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(s);
}
