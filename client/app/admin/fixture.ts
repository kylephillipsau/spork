import type { ApiToken, MintedApiToken } from "@domain/types";
import { blankDraft } from "./useTokens";
import type { TokensBench, TokensState } from "./useTokens";

/**
 * Import tokens, with no network.
 *
 * Four states, and the one worth drawing is the secret: it exists for a few
 * seconds after minting on a live screen and never again, so a fixture is the
 * only way anybody looks at it twice.
 */

const day = 24 * 60 * 60 * 1000;
/** Fixed, because a screenshot that moves every day is a diff nobody reads. */
const NOW = new Date("2026-08-19T00:00:00Z").getTime();

function at(offsetDays: number): string {
  return new Date(NOW + offsetDays * day).toISOString();
}

const PERSON = "77770000-0000-0000-0000-000000000001";

export const TOKENS: readonly ApiToken[] = [
  {
    id: "10000000-0000-0000-0000-000000000001",
    label: "Bin list, from Kyle's laptop",
    created_by_id: PERSON,
    created_at: at(-3),
    expires_at: at(87),
    last_used_at: at(-1),
    revoked_at: null,
  },
  {
    id: "10000000-0000-0000-0000-000000000002",
    label: "Nightly stock reload",
    created_by_id: PERSON,
    created_at: at(-40),
    expires_at: at(50),
    last_used_at: null,
    revoked_at: null,
  },
  {
    id: "10000000-0000-0000-0000-000000000003",
    label: "Laptop that went back to the shop",
    created_by_id: PERSON,
    created_at: at(-120),
    expires_at: at(-30),
    last_used_at: at(-118),
    revoked_at: at(-119),
  },
];

export const MINTED: MintedApiToken = {
  id: "10000000-0000-0000-0000-000000000004",
  label: "Item master, one-off",
  token: "nyl_9f2c41d7a0b3e58c6d1f0a2b4c7e8d93f1a05b6c2d3e4f50617283940a1b2c3d",
  expires_at: at(90),
};

export const READY: TokensState = { kind: "ready", tokens: TOKENS };
export const NONE: TokensState = { kind: "ready", tokens: [] };
export const FAILED: TokensState = {
  kind: "failed",
  message: "The server could not be reached.",
};

export function fixtureTokens(
  state: TokensState,
  over: Partial<TokensBench> = {},
): TokensBench {
  return {
    state,
    draft: blankDraft(),
    busy: false,
    problem: null,
    minted: null,
    type: () => {},
    mint: async () => {},
    revoke: async () => {},
    dismiss: () => {},
    ...over,
  };
}
