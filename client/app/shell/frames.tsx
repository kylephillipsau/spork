import type { ReactElement, ReactNode } from "react";

import { Spinner, UiRoot, cx, materials } from "@ui/index";
import { SessionProvider } from "@app/session/SessionContext";
import { useGate } from "@app/session/Gate";
import { Regions, useRegion } from "@app/shells/slots";
import { wantsChromeLocator } from "@app/routing/manifest";
import type { Screen } from "@app/routing/Router";

import { AppShell } from "./AppShell";
import s from "./frames.module.css";

export type Frame = "app" | "auth";

/**
 * Which frame a screen gets (D171). Null keeps the old material frame, for the
 * screens not moved yet: handheld ones (phase E), first-run setup, and
 * fixtures, which draw their own shells.
 */
export function frameFor(screen: Screen): Frame | null {
  if (screen.own || screen.surface === "floor" || screen.id === "setup") return null;
  if (screen.id === "sign-in" || screen.id === "where") return "auth";
  return "app";
}

/** Screens whose bodies are built on the UI kit. The rest are wrapped. */
const KIT_NATIVE: ReadonlySet<string> = new Set(["sign-in", "where"]);

export function KitFrame({ screen, frame, children }: { screen: Screen; frame: Frame; children: ReactNode }) {
  const body = KIT_NATIVE.has(screen.id) ? children : <LegacyBody>{children}</LegacyBody>;
  const framed =
    frame === "auth" ? (
      <AuthLayout>{body}</AuthLayout>
    ) : (
      <AppShell screenId={screen.id} title={screen.title} showSearch={wantsChromeLocator(screen)}>
        {body}
      </AppShell>
    );
  return (
    <UiRoot>
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
      <Spinner size={22} label={state === "leaving" ? "Taking you to sign in" : "Loading"} />
    </div>
  );
}

/** Sign-in and first choices: a centred card on the anodised ground. */
export function AuthLayout({ children }: { children: ReactNode }) {
  return (
    <div className={cx(s.auth, materials.anodise)}>
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

/**
 * A screen body still on the old material system, inside the new frame until
 * it moves over (phase D). It keeps the dark chassis it was drawn for, and the
 * evidence region DeskShell used to give it.
 */
export function LegacyBody({ children }: { children: ReactNode }) {
  const [evidence, setEvidence] = useRegion();
  return (
    <div className={s.legacy} data-density="desk">
      <Regions evidence={evidence}>
        <div className={s.legacySplit}>
          <div className={s.legacyWork}>{children}</div>
          <aside ref={setEvidence} className={s.legacyEvidence} />
        </div>
      </Regions>
    </div>
  );
}
