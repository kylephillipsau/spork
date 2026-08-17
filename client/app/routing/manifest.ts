// **Relative, not the `@domain` alias.** This module is imported by
// `node --test`, which resolves neither Vite aliases nor tsconfig paths — and
// the whole reason the manifest is separate from the components is so a test
// runner can read it.
import { pattern } from "../../domain/routing.ts";
import type { Pattern } from "../../domain/routing.ts";

/**
 * Which screens exist, as data.
 *
 * **Separate from the components so that something other than a browser can
 * read it.** The rail's tests ask whether every screen is reachable and whether
 * every rail entry points somewhere; both questions are about this list, and
 * neither can be asked of a module that imports React and a stylesheet.
 *
 * `surface` is not decoration: it says which shell mounts, and it gives the
 * render gate somewhere to check density against instead of a second list kept
 * by hand.
 */

export type Surface = "bench" | "floor" | "desk" | "plain";

export interface ScreenSpec {
  /** Stable, and what the rail marks as current. */
  readonly id: string;
  readonly path: string;
  readonly title: string;
  readonly surface: Surface;
  readonly pattern: Pattern;
  /**
   * This screen owns the scanner (D117).
   *
   * **Static, and that is deliberate.** A claim registered in a mount effect
   * would draw the chrome's locator for one frame and then remove it — a flash,
   * and a gate assertion that depends on timing. Declared here, presence is
   * decided before anything renders and the render gate can assert it.
   */
  readonly claimsScan?: boolean;
  /**
   * Whether a session is needed to be here.
   *
   * Setup runs on a deployment with nobody in it, so it has none — and that is
   * why it carries no locator: resolving an identifier is a read behind a
   * session, and a scan bar that answers 401 is worse than no scan bar.
   */
  readonly session?: "required" | "none";
  /**
   * `centre` for a form of fixed size, which is sign in and nothing else.
   *
   * Here rather than returned from the screen because it is a fact about the
   * screen, not about anything it fetched — and the frame that draws the shell
   * needs it before the screen renders.
   */
  readonly align?: "centre";
}

function spec(
  id: string,
  path: string,
  title: string,
  surface: Surface,
  extra: Partial<Pick<ScreenSpec, "claimsScan" | "session" | "align">> = {},
): ScreenSpec {
  return { id, path, title, surface, pattern: pattern(path), ...extra };
}

/** Whether this screen draws the chrome's locator (D111 against D117). */
export function wantsChromeLocator(screen: ScreenSpec): boolean {
  return screen.session !== "none" && !screen.claimsScan;
}

/**
 * Declaration order is match order — a literal above a pattern where the two
 * could collide. There is no such pair yet; the first will be `/orders/new`
 * beside `/orders/:order`.
 */
export const SCREENS: readonly ScreenSpec[] = [
  spec("home", "/", "Waiting", "bench"),
  // The literal above the pattern: declaration order is match order, and this
  // is the first pair where it matters.
  spec("pack", "/pack", "Pack", "bench"),
  spec("pack-one", "/pack/:fulfilment", "Pack", "bench"),
  spec("despatch", "/despatch", "Despatch", "bench"),
  spec("weigh", "/weigh", "Weigh", "bench"),
  spec("capture", "/capture", "Capture", "floor", { claimsScan: true }),
  // **Claims the scanner now** (D117). The walk grew a scan bar of its own with
  // D166 — one field asking where the goods are going, then whether this is the
  // thing on the row — and a chrome locator beside it is two places to aim a
  // reader, fighting over the caret.
  spec("picking", "/picking", "Picking", "floor", { claimsScan: true }),
  // The other half of the same argument: one scan field, asking whichever
  // question is open — what did you pick up, then which bin is this.
  // The step before it, and the same shape again: scan the bay, then scan each
  // carton. A GS1 label answers item, lot and date in one pass.
  spec("receiving", "/receiving", "Receiving", "floor", { claimsScan: true }),
  spec("putaway", "/putaway", "Put away", "floor", { claimsScan: true }),
  spec("orders", "/orders", "Find an order", "desk"),
  spec("findings", "/findings", "Findings", "desk"),
  // The screen D135 was waiting for: a finding is a row in the database, so a
  // link to one restores the queue, the tab and the evidence panel. Same title
  // as the queue, because it is the same screen with a row open on it.
  spec("finding", "/findings/:finding", "Findings", "desk"),
  spec("where", "/where", "Where you are working", "plain"),
  spec("account", "/account", "Password", "plain"),
  spec("workspace", "/workspace", "Workspace", "desk"),
  spec("import", "/import", "Import", "desk"),
  spec("tokens", "/tokens", "Import tokens", "desk"),
  spec("keys", "/keys", "Passkeys", "plain"),
  spec("sign-in", "/sign-in", "Sign in", "plain", { session: "none", align: "centre" }),
  spec("setup", "/setup", "Setup", "plain", { session: "none" }),
];
