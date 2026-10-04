/**
 * Which server this device talks to.
 *
 * **A device has to be told, and a browser never does.** The web client is
 * served by the server it calls, so `/api` is always right; the app is a bundle
 * on a tablet and the server could be the work computer on the warehouse wifi,
 * a laptop over Tailscale, or a deployment.
 *
 * **Set on the device, and kept there.** There are two servers now (the work
 * computer and a laptop), so the sign-in screen of the app asks for the
 * address and this keeps it in the webview's storage. A build can still bake
 * one in (`VITE_SPORK_SERVER`), which the device's own choice outranks.
 */
const DEFAULT_SERVER = "https://spork.warehouseutilities.com";

/** Overridden at build time for a laptop build: `VITE_SPORK_SERVER=... npm run build:mobile`. */
const CONFIGURED = import.meta.env["VITE_SPORK_SERVER"] as string | undefined;

const CHOSEN = "spork.server";

function chosen(): string | null {
  try {
    return window.localStorage.getItem(CHOSEN);
  } catch {
    return null;
  }
}

/** No trailing slash: the API path is appended to it. */
export function serverUrl(): string {
  return (chosen() ?? CONFIGURED ?? DEFAULT_SERVER).replace(/\/+$/, "");
}

/** Whether anybody has said which server, on the device or in the build. */
export function serverSaid(): boolean {
  return chosen() !== null || CONFIGURED !== undefined;
}

/**
 * An address as typed into a full one: `192.168.1.20:8080` becomes
 * `http://192.168.1.20:8080`, because a server on the warehouse network is
 * plain HTTP. Null when it isn't an address at all.
 */
export function tidyServer(typed: string): string | null {
  const t = typed.trim().replace(/\/+$/, "");
  if (!t) return null;
  const withScheme = /^https?:\/\//i.test(t) ? t : `http://${t}`;
  try {
    const u = new URL(withScheme);
    return `${u.protocol}//${u.host}`;
  } catch {
    return null;
  }
}

/**
 * Talk to another server from now on. The transport is bound once, when the
 * app starts, so the app starts again on the new one; a session belongs to the
 * server that issued it, so the old one is forgotten.
 */
export function setServer(url: string, forgetSession: () => void): void {
  try {
    window.localStorage.setItem(CHOSEN, url);
  } catch {
    /* without storage the build's address stands */
  }
  forgetSession();
  window.location.replace("/");
}
