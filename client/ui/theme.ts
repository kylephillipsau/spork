/**
 * Light, dark, or whatever the operating system says (D171).
 *
 * The preference is per browser, in localStorage, because it is a property of
 * the screen somebody is looking at rather than of their account: the same
 * person may want dark on the office PC and light on a bright dock. Light is
 * the default. Storage can throw (private windows, blocked site data), and a
 * theme that failed to save is still a theme, so every access is guarded.
 */

import { useCallback, useEffect, useSyncExternalStore } from "react";

export type ThemePreference = "light" | "dark" | "system";
type Resolved = "light" | "dark";

const KEY = "spork.theme";
const listeners = new Set<() => void>();

function read(): ThemePreference {
  try {
    const v = localStorage.getItem(KEY);
    if (v === "light" || v === "dark" || v === "system") return v;
  } catch {
    // storage unavailable: fall through to the default
  }
  return "light";
}

function systemIsDark(): boolean {
  return typeof matchMedia === "function" && matchMedia("(prefers-color-scheme: dark)").matches;
}

function resolve(p: ThemePreference): Resolved {
  if (p === "system") return systemIsDark() ? "dark" : "light";
  return p;
}

/** Put the resolved theme on <html>. Safe to call before React mounts. */
export function applyTheme(p: ThemePreference = read()): void {
  document.documentElement.dataset.theme = resolve(p);
}

export function setThemePreference(p: ThemePreference): void {
  try {
    localStorage.setItem(KEY, p);
  } catch {
    // not saved, still applied
  }
  applyTheme(p);
  for (const l of listeners) l();
}

function subscribe(l: () => void): () => void {
  listeners.add(l);
  return () => listeners.delete(l);
}

export function useTheme(): {
  preference: ThemePreference;
  resolved: Resolved;
  setPreference: (p: ThemePreference) => void;
} {
  const preference = useSyncExternalStore(subscribe, read, () => "light" as const);

  // "System" follows the OS live, not just at load.
  useEffect(() => {
    if (preference !== "system" || typeof matchMedia !== "function") return;
    const mq = matchMedia("(prefers-color-scheme: dark)");
    const on = () => applyTheme("system");
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, [preference]);

  const setPreference = useCallback((p: ThemePreference) => setThemePreference(p), []);
  return { preference, resolved: resolve(preference), setPreference };
}
