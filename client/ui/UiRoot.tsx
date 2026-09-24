import { useEffect, type ReactNode } from "react";
import { Tooltip } from "radix-ui";

import "@fontsource-variable/inter";
import "./tokens.css";
import "./base.css";

import { applyTheme } from "./theme";
import { ToastProvider } from "./Toast";

/**
 * The root for every screen on the UI kit (D171).
 *
 * Marks <body> so the kit's base styles apply only while a kit screen is
 * mounted — screens still on the old material system keep theirs — and holds
 * the providers the kit's overlays need.
 */
export function UiRoot({
  density = "desktop",
  children,
}: {
  density?: "desktop" | "touch" | undefined;
  children: ReactNode;
}) {
  useEffect(() => {
    applyTheme();
    const body = document.body;
    body.dataset.ui = "kit";
    return () => {
      delete body.dataset.ui;
    };
  }, []);

  useEffect(() => {
    document.body.dataset.density = density;
    return () => {
      delete document.body.dataset.density;
    };
  }, [density]);

  return (
    <Tooltip.Provider delayDuration={400} skipDelayDuration={200}>
      <ToastProvider>{children}</ToastProvider>
    </Tooltip.Provider>
  );
}
