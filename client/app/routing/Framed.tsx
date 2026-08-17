import type { ReactElement, ReactNode } from "react";

import { api } from "@domain/api";
import { BenchShell } from "@app/shells/BenchShell";
import { DeskShell } from "@app/shells/DeskShell";
import { FloorShell } from "@app/shells/FloorShell";
import { PlainShell } from "@app/shells/PlainShell";
import { WorkRail } from "@app/nav/WorkRail";
import { Locator } from "@app/scan/Locator";
import { useChromeScan } from "@app/scan/useScan";
import { currentDestination } from "@app/nav/rail";
import { SessionProvider, siteLabel, useSession, whoLabel } from "@app/session/SessionContext";
import { Gate } from "@app/session/Gate";
import { useWork } from "@app/nav/useWork";
import { wantsChromeLocator } from "./manifest";
import { href, usePath } from "./Router";
import type { Screen } from "./Router";

/**
 * The frame every live screen is drawn in, mounted once above the route switch.
 *
 * # Why this is not inside each screen
 *
 * It was. Every route returned its own `BenchShell` or `DeskShell`, each of
 * which mounted its own `LightRoom`, and each was wrapped in its own
 * `SessionProvider`. React reconciles by element type at a position, so moving
 * from `/pack` to `/despatch` swapped `LivePack` for `LiveDespatch` at the top
 * of the tree and unmounted everything under it: the room, its canvas, the light
 * solver, the chrome, the rail and the session. Every navigation therefore
 * refetched `/sessions/current`, redrew the hangar, emptied the badges and
 * started a fresh critically damped spring whose own comment says it takes "a
 * little under a second" to settle. That was the transition.
 *
 * Now the provider, the gate, the chrome and the shell all sit above the screen
 * and the screen is the only thing that changes. Moving between two Bench
 * screens reconciles one `BenchShell` and swaps its children; moving between
 * surfaces swaps the shell but keeps the room, because `LightRoom` is above this
 * and `density` is an attribute rather than a component.
 *
 * The screen's own regions — Floor's dock, Desk's evidence panel — reach the
 * shell through [`../shells/slots`], which is what lets the shell outlive them.
 */

/** The chrome, from the session rather than from a literal. */
function useChrome(screen: Screen): {
  site: string;
  who: string;
  rail: ReactElement;
  locator: ReactElement | undefined;
} {
  const session = useSession();
  const path = usePath();
  // The screen rather than the path: opening a finding is an arrival, and
  // moving from that one to the next is not. A badge count does not change
  // because somebody read a different row.
  const counts = useWork(screen.id);
  const scan = useChromeScan();
  const here = currentDestination(path);
  return {
    site: siteLabel(session),
    who: whoLabel(session),
    rail: (
      <WorkRail
        here={here?.id ?? null}
        counts={counts}
        onSignOut={() => {
          // Revoke first, then a full load: the cookie is dead server-side
          // either way, and reloading means nothing behind the gate is holding
          // data from a session that no longer exists.
          void api.signOff().finally(() => window.location.assign(href("/sign-in")));
        }}
      />
    ),
    // **Presence from the table, not from a runtime claim.** On Floor, vertical
    // space is the scarce resource and two locators stacked on a 430px screen is
    // a worse answer to D111 than one, so on the screen that owns the scanner
    // the screen's locator *is* the chrome's.
    locator: wantsChromeLocator(screen) ? <Locator scan={scan} /> : undefined,
  };
}

/**
 * The three shells that name where you are and who you are.
 *
 * Separate from the plain one because `useChrome` reads the session and asks
 * the server for the badge counts, and `PlainShell` has nowhere to put either.
 * A single component would have to call the hook for screens that discard the
 * result — and hooks cannot be called conditionally.
 */
function Chromed({ screen, children }: { screen: Screen; children: ReactNode }): ReactElement {
  const { site, who, rail, locator } = useChrome(screen);
  const title = screen.title;

  if (screen.surface === "floor") {
    // No rail: 430px with the dock spoken for by D134's primary action, so
    // Floor's navigation is the wordmark (D110).
    return (
      <FloorShell locator={locator} title={title} site={site} who={who}>
        {children}
      </FloorShell>
    );
  }
  if (screen.surface === "desk") {
    return (
      <DeskShell rail={rail} locator={locator} title={title} site={site} who={who}>
        {children}
      </DeskShell>
    );
  }
  return (
    <BenchShell rail={rail} locator={locator} title={title} site={site} who={who}>
      {children}
    </BenchShell>
  );
}

/**
 * One session fetch for the application, not one per navigation.
 *
 * **Sign-in and setup get neither provider nor gate.** Both run when there is no
 * session, and asking the server who you are on the screen whose job is to
 * establish that answers 401 by design — then the gate would redirect sign-in to
 * sign-in. They are the two `session: "none"` entries in the manifest.
 *
 * **Fixtures never come through here at all.** A provider around them would
 * fetch `/sessions/current` on every one, and the render gate counts a failed
 * request as a failure — the no-network property is what makes sixty screens
 * reviewable without a server.
 */
export function Framed({
  screen,
  children,
}: {
  screen: Screen;
  children: ReactNode;
}): ReactElement {
  const shell =
    screen.surface === "plain" ? (
      // No site and no operator: these screens are about an account rather than
      // about work, and setup runs before a session exists to name either.
      <PlainShell title={screen.title} align={screen.align ?? "top"}>
        {children}
      </PlainShell>
    ) : (
      <Chromed screen={screen}>{children}</Chromed>
    );

  if (screen.session === "none") return shell;

  return (
    <SessionProvider>
      <Gate needsSession>{shell}</Gate>
    </SessionProvider>
  );
}
