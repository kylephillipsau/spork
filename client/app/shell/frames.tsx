import type { ReactElement, ReactNode } from "react";

import { PageHeader, Spinner, UiRoot, cx, materials } from "@ui/index";
import { SessionProvider } from "@app/session/SessionContext";
import { useGate } from "@app/session/Gate";
import { Regions, useRegion } from "@app/shells/slots";
import { wantsChromeLocator } from "@app/routing/manifest";
import type { Screen } from "@app/routing/Router";

import { AppShell } from "./AppShell";
import s from "./frames.module.css";
import legacy from "./legacy.module.css";

export type Frame = "app" | "auth";

/**
 * Which frame a screen gets (D171). Null keeps the old material frame, for
 * fixtures only, which draw their own shells. Handheld (floor) screens get the
 * app frame at touch density with a dock.
 */
export function frameFor(screen: Screen): Frame | null {
  if (screen.own) return null;
  if (screen.id === "sign-in" || screen.id === "where" || screen.id === "setup") return "auth";
  return "app";
}

/** Screens whose bodies are built on the UI kit. The rest are wrapped. */
const KIT_NATIVE: ReadonlySet<string> = new Set(["sign-in", "where", "home", "pack", "orders", "findings", "finding", "tokens", "workspace", "account", "keys", "import", "despatch", "weigh", "pack-one"]);

export function KitFrame({ screen, frame, children }: { screen: Screen; frame: Frame; children: ReactNode }) {
  const touch = screen.surface === "floor";
  const inner = KIT_NATIVE.has(screen.id) ? (
    children
  ) : (
    // On the auth ground the page title would be dark ink on metal; the card
    // the body draws carries its own heading there.
    <LegacyBody title={frame === "auth" ? null : screen.title} touch={touch}>
      {children}
    </LegacyBody>
  );
  const body = touch ? <DockHost>{inner}</DockHost> : inner;
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

/**
 * A screen body still on the old material system, inside the new frame until
 * it moves over (phase D). The legacy adapter flattens its panels into kit
 * cards, and it keeps the evidence region DeskShell used to give it.
 */
export function LegacyBody({
  title,
  touch = false,
  children,
}: {
  title: string | null;
  touch?: boolean;
  children: ReactNode;
}) {
  const [evidence, setEvidence] = useRegion();
  return (
    <div className={cx(s.legacy, legacy.legacy)} data-density={touch ? "floor" : "desk"}>
      {title && <PageHeader title={title} />}
      <Regions evidence={evidence}>
        <div className={s.legacySplit}>
          <div className={s.legacyWork}>{children}</div>
          <aside ref={setEvidence} className={s.legacyEvidence} />
        </div>
      </Regions>
    </div>
  );
}

/**
 * Handheld screens put their primary action in a dock pinned to the bottom of
 * the screen (D134), where a thumb is. The screen renders it through the
 * `Dock` slot; this draws the bar it lands in — a raised surface, light so the
 * fields and figures in it read — and hides it while nothing is in it.
 */
export function DockHost({ children }: { children: ReactNode }) {
  const [dock, setDock] = useRegion();
  return (
    <Regions dock={dock}>
      <div className={s.handheld}>
        {children}
        <div ref={setDock} className={cx(s.dock, legacy.legacy)} data-density="floor" />
      </div>
    </Regions>
  );
}
