/**
 * Which server this device talks to.
 *
 * **A device has to be told, and a browser never does.** The web client is
 * served by the server it calls, so `/api` is always right; the app is a bundle
 * on a phone and the server could be the deployment, a laptop on the same wifi,
 * or somebody else's tenancy entirely.
 *
 * One constant and one override for now, because there is one deployment and
 * inventing a server-picker screen before there are two servers is inventing a
 * screen. When there are two, this is where `setServer()` goes and
 * `mobile/README.md` says so.
 */
const DEFAULT_SERVER = "https://app.nylonite.com";

/** Overridden at build time for a laptop build: `NYLONITE_SERVER=... npm run build:mobile`. */
const CONFIGURED = import.meta.env["VITE_NYLONITE_SERVER"] as string | undefined;

/** No trailing slash: the API path is appended to it. */
export function serverUrl(): string {
  return (CONFIGURED ?? DEFAULT_SERVER).replace(/\/+$/, "");
}
