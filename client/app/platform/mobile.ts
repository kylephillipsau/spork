/**
 * The client, running inside the Tauri shell (`mobile/`).
 *
 * # Why this is a build mode and not a runtime check
 *
 * The obvious thing is `if ("__TAURI_INTERNALS__" in window)` and a lazy
 * import. That ships the host to every browser as an unloaded chunk, and this
 * repository has already decided how it feels about code that is present in the
 * deployed bundle and relied upon not to run: fixtures are a build mode (D146)
 * for exactly that reason, and *"this repository has enough experience of rules
 * that hold only while everyone remembers them."*
 *
 * So `vite build --mode mobile` produces the bundle `src-tauri` embeds, and
 * `import.meta.env.MODE` is a literal in every other build — Rollup drops the
 * branch in `main.tsx`, this module, and `@tauri-apps/plugin-http` with it. The
 * deployed web bundle contains no Tauri code at all rather than none that runs.
 *
 * # What it rebinds, and what it does not
 *
 * Only the transport: where the server is, which HTTP client reaches it, and
 * what credential it carries. Every screen, hook and component is the same code
 * the browser runs — which is the whole argument for a webview shell over a
 * second native client, and it only holds while this file stays small.
 */
import { useTransport } from "@domain/api";
import { fetch as nativeFetch } from "@tauri-apps/plugin-http";
import { serverUrl } from "./server";

/**
 * Where the session token lives between launches.
 *
 * **`localStorage`, and that is a debt rather than a decision.** The token is
 * the session on a device, so this is a credential at rest in the webview's
 * store, readable by anything that achieves script execution in it. Nosdesk
 * answers the same question with a Keychain/Keystore plugin
 * (`tauri-plugin-secure-store`, 189 lines of Rust and about a hundred each of
 * Swift and Kotlin) and that is where this should end up before anybody who is
 * not Kyle installs the app.
 *
 * Named here rather than in a document nobody opens: the next person to read
 * this file is the person who should move it.
 */
const TOKEN = "spork.session-token";

function held(): string | null {
  try {
    return window.localStorage.getItem(TOKEN);
  } catch {
    // A webview with storage disabled signs in every launch, which is a worse
    // application and not a broken one.
    return null;
  }
}

export function bootstrapMobile(): void {
  useTransport({
    base: `${serverUrl()}/api`,
    // **The native client, not the webview's `fetch`.** From a `tauri://`
    // origin every request to the API is cross-origin, and a mobile WebView's
    // CORS behaviour is not something to discover in an aisle. This issues the
    // request from Rust instead, scoped by `capabilities/default.json` to
    // HTTPS.
    fetch: nativeFetch as unknown as typeof globalThis.fetch,
    headers: () => {
      const token = held();
      return token ? { authorization: `Bearer ${token}` } : {};
    },
    // There is no cookie jar to send from and nothing would accept one across
    // origins. Saying so is better than letting the default decide.
    credentials: "omit",
    remember: (token) => {
      try {
        if (token === null) window.localStorage.removeItem(TOKEN);
        else window.localStorage.setItem(TOKEN, token);
      } catch {
        /* see `held` */
      }
    },
  });
}
