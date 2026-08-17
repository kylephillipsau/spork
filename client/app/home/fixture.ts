import type { HomeBench, HomeState } from "./useHome";

/**
 * The front door, with no network.
 *
 * Three states, and the two quiet ones are the interesting fixtures: a site
 * with nothing waiting and a session with no site chosen are opposite
 * instructions that a screen must never draw the same way.
 */
const noop = async () => {};

export function fixtureHome(state: HomeState): HomeBench {
  return { state, refresh: noop };
}

/** An ordinary morning. */
export const BUSY: HomeState = {
  kind: "ready",
  work: { pack: 4, pick: 12, despatch: 1, findings: 7, no_site: false },
};

/** Everything cleared. The system working, not a screen that failed to load. */
export const QUIET: HomeState = {
  kind: "ready",
  work: { pack: 0, pick: 0, despatch: 0, findings: 0, no_site: false },
};

/** Signed in, nowhere named. Every count would be a lie, so none is drawn. */
export const NO_SITE: HomeState = {
  kind: "ready",
  work: { pack: 0, pick: 0, despatch: 0, findings: 0, no_site: true },
};
