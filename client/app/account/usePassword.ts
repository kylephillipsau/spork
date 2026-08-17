import { useCallback, useEffect, useState } from "react";
import { useLive } from "@app/acting";
import { ApiError, api, reason } from "@domain/api";
import type { CurrentSession, PasswordChanged } from "@domain/types";

/**
 * Changing your own password, as logic.
 *
 * D143. The screen is small and the rules it enforces are all the server's —
 * this holds three strings, a state, and the one judgement worth making on the
 * client: that the two copies of the new password match.
 *
 * # Why the confirmation is checked here and nowhere else
 *
 * The server never sees it, and should not: a mistyped new password is not a
 * refusal to record, it is a question the operator can answer before they ask.
 * Every other rule — length, the current password, whether it changed at all —
 * is the server's, because a client-side copy of a policy is a copy that drifts
 * and a policy the server does not enforce is not a policy.
 */

export interface Draft {
  current: string;
  next: string;
  again: string;
}

export function blankDraft(): Draft {
  return { current: "", next: "", again: "" };
}

export type PasswordState =
  | { kind: "asking" }
  /** Signed in, and here to change it. */
  | { kind: "ready"; who: CurrentSession }
  | { kind: "done"; who: CurrentSession; result: PasswordChanged }
  /** No session, or the server could not be reached. */
  | { kind: "failed"; message: string };

export interface PasswordBench {
  state: PasswordState;
  draft: Draft;
  busy: boolean;
  problem: string | null;
  /** True when the two copies of the new password differ and both have been
   *  typed into. Not a refusal until then: reporting a mismatch against a field
   *  somebody has not reached yet is a screen shouting at somebody typing. */
  mismatched: boolean;
  type: (field: keyof Draft, next: string) => void;
  dismiss: () => void;
  submit: () => Promise<void>;
}

export function usePassword(): PasswordBench {
  const [state, setState] = useState<PasswordState>({ kind: "asking" });
  const [draft, setDraft] = useState<Draft>(blankDraft);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);

  const live = useLive();

  const ask = useCallback(async () => {
    try {
      const who = await api.currentSession();
      if (live.current) setState({ kind: "ready", who });
    } catch (error) {
      const message =
        error instanceof ApiError
          ? "You are not signed in."
          : "The server could not be reached.";
      if (live.current) setState({ kind: "failed", message });
    }
  }, []);

  useEffect(() => {
    void ask();
  }, [ask]);

  const mismatched =
    draft.next.length > 0 && draft.again.length > 0 && draft.next !== draft.again;

  return {
    state,
    draft,
    busy,
    problem,
    mismatched,

    type: (field, next) => setDraft((d) => ({ ...d, [field]: next })),

    dismiss: () => setProblem(null),

    submit: async () => {
      if (busy || state.kind !== "ready") return;
      if (draft.next !== draft.again) {
        setProblem("The two new passwords do not match.");
        return;
      }
      setBusy(true);
      setProblem(null);
      try {
        const result = await api.changePassword({
          current_password: draft.current,
          new_password: draft.next,
        });
        if (live.current) {
          // **Cleared on success, rather than left in the fields.** Three
          // passwords sitting in a form after the act is done is three
          // passwords on a screen somebody walks away from.
          setDraft(blankDraft());
          setState({ kind: "done", who: state.who, result });
        }
      } catch (error) {
        const message = reason(error, "That did not work.");
        if (live.current) setProblem(message);
      } finally {
        if (live.current) setBusy(false);
      }
    },
  };
}
