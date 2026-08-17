import { useCallback, useEffect, useState } from "react";
import { useLive } from "@app/acting";
import { ApiError, api, reason } from "@domain/api";
import { declined, decodeRegistration, encodeAttestation, supportsPasskeys } from "@domain/webauthn";
import type { Passkey } from "@domain/types";

/**
 * Passkeys, as logic.
 *
 * # Three outcomes, not two
 *
 * A ceremony can succeed, fail, or be **declined** — the operator dismisses the
 * platform sheet, or the key is already enrolled on this authenticator and the
 * browser refuses before asking. Declining is not an error and must not be
 * drawn as one: `NotAllowedError` and `InvalidStateError` both arrive as
 * exceptions and neither means anything is wrong.
 *
 * `declined` and `supportsPasskeys` are in `domain/webauthn.ts` because sign-in
 * asks the same two questions of the same browser, and a second copy of "what
 * the operator saying no looks like" is the shape of bug that gets one of the
 * two copies right.
 *
 * # There is no decision number for this, and seven places said D144
 *
 * Passkeys arrived with migration 75 and were never written up as a decision.
 * D144 is *"the API answers under `/api`"*, adopted the same week, and it got
 * cited here and in six other places as if it were the passkeys one — the
 * failure the handover calls *"citing a decision you have not opened"*, which
 * nothing checks. The citations are gone rather than repointed: migration 75
 * and the README's Passkeys section are where this is written down.
 */

export type KeysState =
  | { kind: "loading" }
  | { kind: "ready"; keys: readonly Passkey[] }
  | { kind: "failed"; message: string };

export interface KeysBench {
  state: KeysState;
  label: string;
  busy: boolean;
  problem: string | null;
  /** False where the browser has no WebAuthn at all. */
  supported: boolean;
  type: (next: string) => void;
  add: () => Promise<void>;
  revoke: (id: string) => Promise<void>;
}

/** Whether this authenticator already holds a key for this person. */
function alreadyEnrolled(error: unknown): boolean {
  return error instanceof Error && error.name === "InvalidStateError";
}

export function useKeys(): KeysBench {
  const [state, setState] = useState<KeysState>({ kind: "loading" });
  const [label, setLabel] = useState("");
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);

  const live = useLive();

  const read = useCallback(async () => {
    try {
      const keys = await api.passkeys();
      if (live.current) setState({ kind: "ready", keys });
    } catch (error) {
      const message = reason(error, "The server could not be reached.");
      if (live.current) setState({ kind: "failed", message });
    }
  }, []);

  useEffect(() => {
    void read();
  }, [read]);

  const type = useCallback((next: string) => {
    setLabel(next);
    setProblem(null);
  }, []);

  const add = useCallback(async () => {
    setBusy(true);
    setProblem(null);
    const named = label.trim() || null;
    try {
      const begun = await api.beginPasskeyRegistration(named);
      const credential = (await navigator.credentials.create({
        publicKey: decodeRegistration(begun.options),
      })) as PublicKeyCredential | null;
      if (!credential) {
        // The spec allows a null return where an exception is not thrown.
        // Nothing was enrolled and nothing went wrong.
        if (live.current) setBusy(false);
        return;
      }
      await api.finishPasskeyRegistration({
        ceremony_id: begun.ceremony_id,
        label: named,
        credential: encodeAttestation(credential),
      });
      if (!live.current) return;
      setLabel("");
      await read();
    } catch (error) {
      if (live.current) {
        if (declined(error)) {
          // Said no. Nothing to report.
        } else if (alreadyEnrolled(error)) {
          setProblem("This device already holds a key for you.");
        } else if (error instanceof ApiError) {
          setProblem(error.message);
        } else {
          setProblem("That key was not accepted.");
        }
      }
    } finally {
      if (live.current) setBusy(false);
    }
  }, [label, read]);

  const revoke = useCallback(
    async (id: string) => {
      setBusy(true);
      setProblem(null);
      try {
        await api.revokePasskey(id);
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

  return {
    state,
    label,
    busy,
    problem,
    supported: supportsPasskeys(),
    type,
    add,
    revoke,
  };
}
