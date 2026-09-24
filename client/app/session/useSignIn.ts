import { useCallback, useState } from "react";
import { useLive } from "@app/acting";
import { ApiError, api, reason } from "@domain/api";
import {
  declined,
  decodeAuthentication,
  encodeAssertion,
  supportsPasskeys,
} from "@domain/webauthn";
import type { TenantChoice } from "@domain/types";

/**
 * Signing in, as logic.
 *
 * The last page off the server-rendered stack, and the one D113's migration
 * note called *"a real piece of work rather than a port"* — because it runs
 * before a session exists and because the page it replaces could not do one of
 * the two things this has to.
 *
 * # The company question, which the old page could not ask
 *
 * D19 makes `person` global and membership `person_tenant`, so somebody who
 * works for two businesses must say which they are acting for. The server has
 * always answered 409 with the list; the maud page printed the error text at
 * somebody with no field to name one in, so **that person could not sign in at
 * all**. Here the 409 is a question rather than a failure.
 *
 * # Where they are working is not asked here
 *
 * D145: site is chosen after authenticating, on an ordinary session, because
 * acts copy the site at write time and nothing re-reads the session's copy. So
 * this stays about who you are, and `/where` handles where you are.
 *
 * # The key, which the port dropped
 *
 * `/keys` has enrolled passkeys since migration 75 and the server's ceremony has
 * been written and tested for as long, and **no client called it** — so it was
 * possible to register a credential and then have no way to use it. The maud
 * page had the ceremony; the React page that replaced it shipped the password
 * half. Nothing failed when that happened, because no gate asserts that an
 * endpoint has a caller.
 *
 * The email is optional here and absent is the better path: with no email the
 * server starts a discoverable ceremony and the assertion says whose key it is,
 * which is one fewer field to type on a touchscreen with gloves on.
 *
 * **A key and two employers means two prompts**, and that is the server's shape
 * rather than a choice made here. A ceremony is claimed exactly once, so the
 * 409 that lists the companies has already spent it; choosing one and carrying
 * on means starting a second ceremony, which means asking the authenticator
 * again. The screen says so before it happens rather than surprising somebody
 * with a second sheet.
 */

export interface Credentials {
  email: string;
  password: string;
}

/** How somebody is proving who they are. */
export type Method = "password" | "key";

export type SignInState =
  | { kind: "asking" }
  /** They work for more than one company and must say which. */
  | { kind: "choose"; tenants: TenantChoice[] }
  | { kind: "done" };

export interface SignInBench {
  state: SignInState;
  credentials: Credentials;
  /** Which company, once there is a choice to make. */
  tenant: string | null;
  busy: boolean;
  problem: string | null;
  /** Whether this browser can run a ceremony at all. */
  keys: boolean;
  /**
   * Which act produced what is on screen, or null before either has run.
   *
   * Set by whichever act ran last. The screen needs it because the control that
   * carries on from the company question is a different act for each: one sends
   * a password, the other asks the authenticator again.
   *
   * **The screen offers no way to switch once that question stands**, which is
   * deliberate rather than an oversight: the key is drawn only while there is
   * no question, so a second control there would be a second way to answer one
   * question. Somebody who reached it by key and would rather type a password
   * reloads, which is a fair price for a screen that says one thing at a time.
   */
  via: Method | null;
  /** "Keep me signed in on this device". Remembered per browser, so a
   *  personal laptop stays ticked and a shared bench stays clear. */
  remember: boolean;
  setRemember: (next: boolean) => void;
  type: (field: keyof Credentials, next: string) => void;
  choose: (tenant: string) => void;
  submit: () => Promise<void>;
  /** Sign in with a key. */
  useKey: () => Promise<void>;
}

const REMEMBER_KEY = "spork.remember";

/** The last answer on this browser. Storage can be missing or refuse (a
 *  private window), and then the box simply starts clear. */
function readRemember(): boolean {
  try {
    return window.localStorage.getItem(REMEMBER_KEY) === "1";
  } catch {
    return false;
  }
}

function writeRemember(next: boolean) {
  try {
    if (next) window.localStorage.setItem(REMEMBER_KEY, "1");
    else window.localStorage.removeItem(REMEMBER_KEY);
  } catch {
    // Not remembered next time, which is the safe way to be wrong.
  }
}

export function useSignIn(onSignedIn?: () => void): SignInBench {
  const [credentials, setCredentials] = useState<Credentials>({ email: "", password: "" });
  const [state, setState] = useState<SignInState>({ kind: "asking" });
  const [tenant, setTenant] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const [via, setVia] = useState<Method | null>(null);
  const [remember, setRememberState] = useState(readRemember);
  const setRemember = useCallback((next: boolean) => {
    setRememberState(next);
    writeRemember(next);
  }, []);

  const live = useLive();

  /**
   * The company question, which both ways in can be asked.
   *
   * **409 is a question, not a refusal.** The server lists the companies rather
   * than picking one, and a client that treated this as an error would lock out
   * everybody who works for two. Shared because `issue_session` is shared: the
   * passkey path reaches the same sentence, and answering it in one place is
   * what stops the two halves of this screen disagreeing about what a 409 is.
   *
   * Returns whether it was that question.
   */
  const asked = useCallback((error: unknown, method: Method): boolean => {
    if (!(error instanceof ApiError) || error.status !== 409) return false;
    const tenants = (error.body as { tenants?: TenantChoice[] } | null)?.tenants ?? [];
    if (tenants.length === 0) {
      setProblem(error.message);
      return true;
    }
    setState({ kind: "choose", tenants });
    setVia(method);
    setTenant(tenants[0]?.tenant_id ?? null);
    setProblem(
      method === "key"
        ? "You work for more than one company. Choose which, and your key will be asked for again."
        : "You work for more than one company. Choose which.",
    );
    return true;
  }, []);

  const submit = useCallback(async () => {
    if (busy) return;
    setBusy(true);
    setProblem(null);
    setVia("password");
    try {
      await api.signOn({
        email: credentials.email.trim(),
        password: credentials.password,
        remember,
        ...(tenant ? { tenant_id: tenant } : {}),
      });
      if (!live.current) return;
      setState({ kind: "done" });
      onSignedIn?.();
    } catch (error) {
      if (!live.current) return;
      if (asked(error, "password")) return;
      setProblem(
        reason(error, "The server could not be reached."),
      );
    } finally {
      if (live.current) setBusy(false);
    }
  }, [busy, credentials, remember, tenant, onSignedIn, asked]);

  /**
   * Sign in with a key.
   *
   * The same three outcomes the enrolment ceremony has: it succeeds, it fails,
   * or the operator declines — and declining is not a failure. The one
   * difference is what a success is for: this hands back an assertion and gets
   * a session, where enrolment hands back an attestation and gets a row.
   */
  const useKey = useCallback(async () => {
    if (busy) return;
    setBusy(true);
    setProblem(null);
    setVia("key");
    try {
      // Typed, it narrows the offer to that person's keys; empty, the
      // authenticator offers what it holds for this site and the assertion
      // says whose it is.
      const email = credentials.email.trim();
      const begun = await api.beginPasskeyAuthentication(email || null);
      const assertion = (await navigator.credentials.get({
        publicKey: decodeAuthentication(begun.options),
      })) as PublicKeyCredential | null;
      // The spec allows a null return where an exception is not thrown. Nobody
      // signed in and nothing went wrong.
      if (!assertion) return;
      await api.finishPasskeyAuthentication({
        ceremony_id: begun.ceremony_id,
        credential: encodeAssertion(assertion),
        remember,
        ...(tenant ? { tenant_id: tenant } : {}),
      });
      if (!live.current) return;
      setState({ kind: "done" });
      onSignedIn?.();
    } catch (error) {
      if (!live.current) return;
      // Said no. Nothing to report, and the maud page reported it anyway.
      if (declined(error)) return;
      if (asked(error, "key")) return;
      setProblem(
        reason(error, "That key was not accepted."),
      );
    } finally {
      if (live.current) setBusy(false);
    }
  }, [busy, credentials.email, remember, tenant, onSignedIn, asked]);

  return {
    state,
    credentials,
    tenant,
    busy,
    problem,
    keys: supportsPasskeys(),
    via,
    remember,
    setRemember,
    type: (field, next) => setCredentials((c) => ({ ...c, [field]: next })),
    choose: setTenant,
    submit,
    useKey,
  };
}
