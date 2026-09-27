import { useCallback, useEffect, useSyncExternalStore } from "react";
import { KitFrame, LostFrame, frameFor } from "@app/shell/frames";
import type { ReactElement } from "react";
import { resolve } from "@domain/routing";
import type { Params, Pattern } from "@domain/routing";
import { BASE, currentPath, go, href, subscribe } from "./location";

/**
 * The router: which screen, and how you get to another one.
 *
 * # Params reach a screen as an argument, not through a hook
 *
 * `render` takes them. That is how everything else in this client moves data —
 * props, never ambient state — and it means a screen is renderable from a
 * fixture with a literal param, which is what keeps every state reviewable
 * without a server.
 */

export type Surface = "bench" | "floor" | "desk" | "plain";

export interface Screen {
  /** Stable, and what the rail marks as current. */
  readonly id: string;
  /**
   * This screen draws its own frame and the Router should not frame it.
   * Fixtures do: each shows a page in one state, with a literal session and
   * no network, so it supplies its own shell.
   */
  readonly own?: boolean;
  readonly path: string;
  readonly pattern: Pattern;
  /** The browser tab, and the shell's title. */
  readonly title: string;
  readonly surface: Surface;
  /**
   * Whether a session is needed here. From the manifest, so the gate is hoisted
   * with the frame rather than repeated per screen — sign-in and setup are the
   * two that run without one.
   */
  readonly session?: "required" | "none";
  /** Plain: `centre` for a form of fixed size. A list belongs at the top. */
  readonly align?: "centre";
  /**
   * The work, and only the work.
   *
   * A handheld screen's dock is drawn by the screen too, with [`Dock`]: it
   * holds the screen's state, so it is rendered where that state is and lands
   * where the frame put the container.
   */
  readonly render: (params: Params) => ReactElement;
}

export function usePath(): string {
  return useSyncExternalStore(subscribe, currentPath, currentPath);
}

export function useNavigate(): (to: string, options?: { replace?: boolean }) => void {
  return useCallback((to, options) => go(to, options), []);
}

/**
 * One document-level click listener, so no link needs a handler.
 *
 * **It falls through unless the path resolves to a screen**, and that is the
 * migration rather than an optimisation. Eleven maud pages are still standing
 * at `/app/*` and the packing list stays there permanently because it is
 * genuinely a document (D113). An interceptor that swallowed every same-origin
 * anchor would turn each of those into a client-side 404 and quietly break
 * *"nothing is deleted for tidiness"*.
 */
function useIntercept(routes: readonly Screen[]): void {
  useEffect(() => {
    const onClick = (e: MouseEvent) => {
      if (e.defaultPrevented || e.button !== 0) return;
      if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
      const anchor = (e.target as Element | null)?.closest?.("a");
      if (!anchor) return;
      if (anchor.target && anchor.target !== "_self") return;
      if (anchor.hasAttribute("download")) return;
      const url = new URL(anchor.href, window.location.href);
      if (url.origin !== window.location.origin) return;
      const path = url.pathname.startsWith(BASE)
        ? url.pathname.slice(BASE.length - 1) || "/"
        : url.pathname;
      if (resolve(routes, path) === null) return;
      e.preventDefault();
      go(path);
    };
    document.addEventListener("click", onClick);
    return () => document.removeEventListener("click", onClick);
  }, [routes]);
}

export function Router({
  routes,
  notFound,
}: {
  routes: readonly Screen[];
  notFound: (path: string) => ReactElement;
}): ReactElement {
  const path = usePath();
  useIntercept(routes);
  const found = resolve(routes, path);

  // The tab says which screen this is. It said "Pack · Spork" on all of them.
  useEffect(() => {
    document.title = found ? `${found.route.title} · Spork` : "Not found · Spork";
  }, [found]);

  // No screen means no session and no frame to put it in: the sign-in ground.
  if (!found) return <LostFrame>{notFound(path)}</LostFrame>;

  // Fixtures draw their own frame, with a literal session and no network.
  if (found.route.own) return found.route.render(found.params);

  // One element type for every live screen, so a navigation reconciles the
  // frame (sidebar, header, session) rather than remounting it.
  return (
    <KitFrame screen={found.route} frame={frameFor(found.route)}>
      {found.route.render(found.params)}
    </KitFrame>
  );
}

export { href, go };
