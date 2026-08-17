import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import type { ReactElement, ReactNode } from "react";
import { ApiError, api, onUnauthorised, reason } from "@domain/api";
import type { CurrentSession, Uuid } from "@domain/types";

/**
 * Who is signed in, asked once.
 *
 * Every shell has been drawing `site="MEL" who="d.stooke"` — a literal, and an
 * operator who is not in the seed. Every live screen fetched one hardcoded
 * site's data regardless of who signed in and labelled it with a made-up name.
 * `GET /sessions/current` has existed the whole time and nothing called it.
 *
 * # The shells do not read this, and that is deliberate
 *
 * They keep taking `site` and `who` as props and stay presentational. If a
 * shell read the context, every fixture route would need a provider around it,
 * and the render gate's no-network property — the thing that makes 29 screens
 * reviewable without a server — would become conditional. The `session` prop
 * below is the same escape hatch by another route: a fixture host supplies a
 * signed-in session and no fetch happens.
 *
 * # Loading is absence, not a placeholder
 *
 * There is no boot screen. A splash trades a correct loading state for a flash,
 * and the chrome is not the slow part. While the session is loading the chrome
 * draws no site and no operator — the same rule `EmptySlot` and D127 apply to
 * data, applied to identity.
 */

export type Session =
  | { kind: "loading" }
  | { kind: "signed-in"; who: CurrentSession }
  /** The server said 401. There is nobody here. */
  | { kind: "anonymous" }
  | { kind: "unreachable"; message: string };

export interface SessionBench {
  session: Session;
  /** Re-ask. Called after choosing a site, which mints a different session. */
  refresh: () => Promise<void>;
}

const Ctx = createContext<SessionBench>({
  session: { kind: "loading" },
  refresh: async () => {},
});

export function useSession(): Session {
  return useContext(Ctx).session;
}

export function useSessionBench(): SessionBench {
  return useContext(Ctx);
}

/**
 * The site this caller is working at, or `null` when it is not known yet.
 *
 * **`null` means "not yet", never "none".** A screen that reads this must hold
 * its own loading state rather than fetching for an absent site — a picking
 * screen drawing "Nothing to pick" for 200ms because the session had not
 * resolved is a screen telling an operator to go home.
 *
 * It stays null for a signed-in person genuinely attached to no site, which is
 * a real state: `session.site_id` is nullable and every browser session created
 * before this existed has it null.
 */
export function useSite(): Uuid | null {
  const session = useSession();
  return session.kind === "signed-in" ? session.who.site_id : null;
}

export function SessionProvider({
  session: override,
  children,
}: {
  /** Supplied by fixtures and by the gate, which have no network. */
  session?: Session;
  children: ReactNode;
}): ReactElement {
  const [session, setSession] = useState<Session>(override ?? { kind: "loading" });
  const live = useRef(true);
  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
    };
  }, []);

  const refresh = useCallback(async () => {
    if (override) return;
    try {
      const who = await api.currentSession();
      if (live.current) setSession({ kind: "signed-in", who });
    } catch (error) {
      if (!live.current) return;
      setSession(
        error instanceof ApiError && error.status === 401
          ? { kind: "anonymous" }
          : {
              kind: "unreachable",
              message:
                reason(error, "The server could not be reached."),
            },
      );
    }
  }, [override]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // **One 401 policy, registered once.** A session expiring mid-shift is the
  // case that actually happens, and it surfaces on whatever call the operator
  // made next rather than on this one. The alternative is every hook checking
  // `error.status === 401` for itself — nine copies of one rule, free to
  // disagree, which is the shape half this repository's decision register is
  // made of.
  useEffect(() => {
    if (override) return;
    onUnauthorised(() => {
      if (live.current) setSession({ kind: "anonymous" });
    });
  }, [override]);

  return <Ctx.Provider value={{ session, refresh }}>{children}</Ctx.Provider>;
}

/** What the chrome shows for a site: a code, or nothing while it is unknown. */
export function siteLabel(session: Session): string {
  return session.kind === "signed-in" ? (session.who.site_code ?? "no site") : "";
}

/** What the chrome shows for a person. Empty while unknown, never a guess. */
export function whoLabel(session: Session): string {
  return session.kind === "signed-in" ? session.who.display_name : "";
}
