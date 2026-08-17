import { useCallback, useEffect, useState } from "react";
import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { ApiToken, MintedApiToken } from "@domain/types";

/**
 * Import tokens, as logic (D158).
 *
 * The rules are all the server's — the label, the expiry range, who may mint —
 * and this holds a draft, a list, and the one piece of state the API cannot
 * hold for us: the secret, which exists in the answer to `POST /tokens` and
 * nowhere afterwards. Losing it before it is copied means minting another, so
 * it lives here until the operator dismisses it rather than being cleared by
 * the next render.
 */

export interface Draft {
  label: string;
  days: string;
}

export function blankDraft(): Draft {
  return { label: "", days: "" };
}

export type TokensState =
  | { kind: "loading" }
  | { kind: "ready"; tokens: readonly ApiToken[] }
  | { kind: "failed"; message: string };

export interface TokensBench {
  state: TokensState;
  draft: Draft;
  busy: boolean;
  problem: string | null;
  /** The secret, until it is dismissed. Null the rest of the time. */
  minted: MintedApiToken | null;
  type: (field: keyof Draft, next: string) => void;
  mint: () => Promise<void>;
  revoke: (id: string) => Promise<void>;
  dismiss: () => void;
}

/** Live, spent, or withdrawn — the three states worth telling apart. */
export function standing(token: ApiToken, now = new Date()): "live" | "expired" | "revoked" {
  if (token.revoked_at) return "revoked";
  return new Date(token.expires_at) <= now ? "expired" : "live";
}

export function useTokens(): TokensBench {
  const [state, setState] = useState<TokensState>({ kind: "loading" });
  const [draft, setDraft] = useState<Draft>(blankDraft);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const [minted, setMinted] = useState<MintedApiToken | null>(null);

  const live = useLive();

  const read = useCallback(async () => {
    try {
      const tokens = await api.apiTokens();
      if (live.current) setState({ kind: "ready", tokens });
    } catch (error) {
      const message = reason(error, "The server could not be reached.");
      if (live.current) setState({ kind: "failed", message });
    }
  }, []);

  useEffect(() => {
    void read();
  }, [read]);

  const type = useCallback((field: keyof Draft, next: string) => {
    setDraft((d) => ({ ...d, [field]: next }));
    setProblem(null);
  }, []);

  const mint = useCallback(async () => {
    setBusy(true);
    setProblem(null);
    try {
      // `days` blank means "the server's default", which is a real answer and
      // not a value to invent here.
      const days = draft.days.trim();
      const made = await api.mintApiToken({
        label: draft.label,
        ...(days ? { days: Number(days) } : {}),
      });
      if (!live.current) return;
      setMinted(made);
      setDraft(blankDraft());
      await read();
    } catch (error) {
      const message = reason(error, "The server could not be reached.");
      if (live.current) setProblem(message);
    } finally {
      if (live.current) setBusy(false);
    }
  }, [draft, read]);

  const revoke = useCallback(
    async (id: string) => {
      setBusy(true);
      setProblem(null);
      try {
        await api.revokeApiToken(id);
        if (live.current) await read();
      } catch (error) {
        const message = reason(error, "The server could not be reached.");
        if (live.current) setProblem(message);
      } finally {
        if (live.current) setBusy(false);
      }
    },
    [read],
  );

  const dismiss = useCallback(() => setMinted(null), []);

  return { state, draft, busy, problem, minted, type, mint, revoke, dismiss };
}
