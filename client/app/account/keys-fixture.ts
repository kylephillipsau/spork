import type { Passkey } from "@domain/types";
import type { KeysBench, KeysState } from "./useKeys";

/**
 * Passkeys, with no browser.
 *
 * A ceremony needs an authenticator, so none of these states can be reached in
 * a headless render — including the one that matters most, which is a browser
 * that cannot do passkeys at all.
 */

const day = 24 * 60 * 60 * 1000;
const NOW = new Date("2026-08-19T00:00:00Z").getTime();
const at = (d: number) => new Date(NOW + d * day).toISOString();

export const KEYS: readonly Passkey[] = [
  {
    id: "20000000-0000-0000-0000-000000000001",
    label: "iPhone",
    created_at: at(-60),
    last_used_at: at(-1),
    sign_count: 42,
    backup_state: true,
  },
  {
    id: "20000000-0000-0000-0000-000000000002",
    label: "Bench terminal",
    created_at: at(-12),
    last_used_at: null,
    sign_count: 0,
    backup_state: false,
  },
];

export const READY: KeysState = { kind: "ready", keys: KEYS };
export const NONE: KeysState = { kind: "ready", keys: [] };
export const FAILED: KeysState = {
  kind: "failed",
  message: "The server could not be reached.",
};

export function fixtureKeys(state: KeysState, over: Partial<KeysBench> = {}): KeysBench {
  return {
    state,
    label: "",
    busy: false,
    problem: null,
    supported: true,
    type: () => {},
    add: async () => {},
    revoke: async () => {},
    ...over,
  };
}
