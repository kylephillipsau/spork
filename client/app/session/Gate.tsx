import { useEffect } from "react";
import type { ReactElement, ReactNode } from "react";
import { Face, Faint, Panel } from "@design/index";
import { useNavigate } from "@app/routing/Router";
import { currentPath } from "@app/routing/location";
import { useSession } from "./SessionContext";

/**
 * Nothing behind here without a session.
 *
 * **The state was computed and never acted on.** `SessionContext` has resolved
 * `anonymous` since D145, and nothing did anything with it — so an unsigned-in
 * visit to `/pack` drew the pack screen and let every call under it fail one at
 * a time. A gate that decides and does not enforce is not a gate.
 *
 * # Replace, never push
 *
 * The redirect uses `replace`, or Back bounces the operator between the form
 * and the screen that rejected them — an unusable loop, and the sort of thing
 * that is obvious once seen and invisible until then.
 *
 * # Where they were going travels with them
 *
 * `?next=` carries the path, so signing in lands on the screen that was asked
 * for rather than dumping everybody at the front door. A bookmarked finding
 * survives an expired session, which is the whole point of the deep links D135
 * waited for.
 *
 * # A session with no warehouse is not a session that can show work
 *
 * `POST /sessions` does not take a site, so every browser session starts with
 * `site_id` null, and `GET /work` answers all zeros with `no_site: true`. The
 * screen that settles it exists and settles silently when there is only one
 * warehouse — but nothing ever sent anybody to it, so the first thing a new
 * operator saw was an empty warehouse and a header reading "no site". Sending
 * them through it once is the whole fix, and after that it never appears again.
 *
 * # Loading is not refused
 *
 * While the session resolves this draws nothing rather than redirecting. A gate
 * that fires during the first render signs everybody out on every reload.
 */
export function Gate({
  needsSession,
  children,
}: {
  needsSession: boolean;
  children: ReactNode;
}): ReactElement {
  const state = useGate(needsSession);
  if (state === "open") return <>{children}</>;
  return (
    <Panel elevation="raised" frame="bezel">
      <Face>
        <Faint>{state === "leaving" ? "Taking you to sign in…" : "…"}</Faint>
      </Face>
    </Panel>
  );
}

/**
 * The gate's decision, and the redirects it runs. Both frames draw their own
 * waiting state from it: the old material one above, and the UI kit's (D171).
 */
export function useGate(needsSession: boolean): "loading" | "leaving" | "open" {
  const session = useSession();
  const navigate = useNavigate();
  const turnedAway = needsSession && session.kind === "anonymous";
  // Signed in, but attached to nowhere yet. `/where` itself is exempt, or the
  // redirect is a loop.
  const unplaced =
    needsSession &&
    session.kind === "signed-in" &&
    session.who.site_id === null &&
    currentPath() !== "/where";

  useEffect(() => {
    if (!turnedAway) return;
    const wanted = currentPath() + window.location.search;
    const next = wanted === "/" ? "" : `?next=${encodeURIComponent(wanted)}`;
    navigate(`/sign-in${next}`, { replace: true });
  }, [turnedAway, navigate]);

  useEffect(() => {
    if (!unplaced) return;
    const wanted = currentPath() + window.location.search;
    const next = wanted === "/" ? "" : `?next=${encodeURIComponent(wanted)}`;
    navigate(`/where${next}`, { replace: true });
  }, [unplaced, navigate]);

  if (session.kind === "loading" && needsSession) return "loading";
  if (turnedAway) return "leaving";
  if (unplaced) return "loading";
  return "open";
}

/** Where to go after signing in: what was asked for, or the front door. */
export function nextAfterSignIn(): string {
  const asked = new URLSearchParams(window.location.search).get("next");
  // Only a path of our own. An absolute URL here would be an open redirect —
  // somebody else's sign-in link landing on somebody else's site.
  return asked && asked.startsWith("/") && !asked.startsWith("//") ? asked : "/";
}
