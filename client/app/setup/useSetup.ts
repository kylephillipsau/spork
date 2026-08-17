import { useCallback, useEffect, useState } from "react";
import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { SetupDone, SetupStatus } from "@domain/types";

/**
 * Setting a deployment up, as logic.
 *
 * The one screen that runs before anything exists — no session, no tenant, no
 * site — so it asks the server one question first: is there anybody here? A
 * deployment that has somebody must not draw a setup form at all, and the
 * server refuses one anyway (D142), but a screen that offers a form the server
 * will refuse is a screen that lies about what it can do.
 */

/** What the operator has typed. Everything is a string; the server validates. */
export interface Details {
  token: string;
  organisation: string;
  siteName: string;
  siteCode: string;
  timezone: string;
  displayName: string;
  email: string;
  password: string;
}

/**
 * A sensible starting point, and the timezone is the browser's.
 *
 * **Guessed, and shown rather than assumed.** A site's timezone decides when
 * "today" starts for every despatch and every count, so it is a field with a
 * default rather than a thing derived silently — the operator can see what was
 * guessed and change it.
 */
export function blankDetails(): Details {
  let zone = "UTC";
  try {
    zone = Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
  } catch {
    /* an environment with no zone database keeps UTC */
  }
  return {
    token: "",
    organisation: "",
    siteName: "",
    siteCode: "",
    timezone: zone,
    displayName: "",
    email: "",
    password: "",
  };
}

export type SetupState =
  | { kind: "asking" }
  | { kind: "needed"; status: SetupStatus }
  | { kind: "done"; result: SetupDone }
  /** Somebody is already in this deployment, so there is nothing to do here. */
  | { kind: "closed" }
  | { kind: "failed"; message: string };

export interface SetupBench {
  state: SetupState;
  details: Details;
  busy: boolean;
  problem: string | null;
  type: (field: keyof Details, next: string) => void;
  dismiss: () => void;
  submit: () => Promise<void>;
}

export function useSetup(): SetupBench {
  const [state, setState] = useState<SetupState>({ kind: "asking" });
  const [details, setDetails] = useState<Details>(blankDetails);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);

  const live = useLive();

  const ask = useCallback(async () => {
    try {
      const status = await api.setupStatus();
      if (!live.current) return;
      setState(status.required ? { kind: "needed", status } : { kind: "closed" });
    } catch (error) {
      const message = reason(error, "The server could not be reached.");
      if (live.current) setState({ kind: "failed", message });
    }
  }, []);

  useEffect(() => {
    void ask();
  }, [ask]);

  return {
    state,
    details,
    busy,
    problem,

    type: (field, next) => setDetails((d) => ({ ...d, [field]: next })),

    dismiss: () => setProblem(null),

    submit: async () => {
      if (busy) return;
      setBusy(true);
      setProblem(null);
      try {
        const result = await api.setUp({
          token: details.token.trim(),
          organisation: details.organisation.trim(),
          site_name: details.siteName.trim(),
          site_code: details.siteCode.trim(),
          timezone: details.timezone.trim(),
          display_name: details.displayName.trim(),
          email: details.email.trim(),
          password: details.password,
        });
        if (live.current) setState({ kind: "done", result });
      } catch (error) {
        const message = reason(error, "That did not work.");
        if (live.current) {
          setProblem(message);
          // **Re-ask, because the interesting failure is somebody else winning.**
          // Two people setting the same deployment up is the race D142's lock
          // exists for; the loser should be told the deployment is set up
          // rather than left staring at a form that will never be accepted.
          void ask();
        }
      } finally {
        if (live.current) setBusy(false);
      }
    },
  };
}
