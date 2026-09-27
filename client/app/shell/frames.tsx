import type { ReactElement, ReactNode } from "react";

import { Spinner, UiRoot, cx, materials } from "@ui/index";
import { SessionProvider } from "@app/session/SessionContext";
import { useGate } from "@app/session/Gate";
import { DockHost } from "./dock";
import { wantsChromeLocator } from "@app/routing/manifest";
import type { Screen } from "@app/routing/Router";

import { AppShell } from "./AppShell";
import s from "./frames.module.css";

export type Frame = "app" | "auth";

/** Which frame a screen gets (D171): sign-in and first choices on the anodised ground, the rest in the app shell. */
export function frameFor(screen: Screen): Frame {
  if (screen.id === "sign-in" || screen.id === "where" || screen.id === "setup") return "auth";
  return "app";
}

export function KitFrame({ screen, frame, children }: { screen: Screen; frame: Frame; children: ReactNode }) {
  const touch = screen.surface === "floor";
  const body = touch ? <DockHost>{children}</DockHost> : children;
  const framed =
    frame === "auth" ? (
      <AuthLayout>{body}</AuthLayout>
    ) : (
      <AppShell screenId={screen.id} title={screen.title} showSearch={wantsChromeLocator(screen)}>
        {body}
      </AppShell>
    );
  return (
    <UiRoot density={touch ? "touch" : "desktop"}>
      {screen.session === "none" ? framed : (
        <SessionProvider>
          <KitGate>{framed}</KitGate>
        </SessionProvider>
      )}
    </UiRoot>
  );
}

function KitGate({ children }: { children: ReactNode }): ReactElement {
  const state = useGate(true);
  if (state === "open") return <>{children}</>;
  return (
    <div className={s.waiting}>
      <Spinner size={22} label={state === "leaving" ? "Redirecting to sign in" : "Loading"} />
    </div>
  );
}

/** A path with no screen: no session to ask about, so the sign-in ground. */
export function LostFrame({ children }: { children: ReactNode }) {
  return (
    <UiRoot>
      <AuthLayout>{children}</AuthLayout>
    </UiRoot>
  );
}

/** Sign-in and first choices: a centred card on the anodised ground. */
export function AuthLayout({ children }: { children: ReactNode }) {
  return (
    <div className={cx(s.auth, materials.anodise)} data-density="desktop">
      <div className={s.authColumn}>
        <div className={s.authBrand}>
          <span className={cx(s.authMark, materials.nylon, materials.dyed)} aria-hidden>
            S
          </span>
          <span className={s.authWordmark}>Spork</span>
        </div>
        {children}
      </div>
    </div>
  );
}
