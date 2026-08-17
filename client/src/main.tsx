import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "@design/tokens.css";
import "@design/layers.css";

import { Router } from "@app/routing/Router";
import type { Screen } from "@app/routing/Router";
import { NotFound } from "@app/routing/NotFound";
import { LIVE } from "@app/routing/screens";

/**
 * The application, mounted.
 *
 * # What used to be here
 *
 * A `Record` of literal pathnames, thirty-nine of them, thirty-one pointing at
 * fixtures — including `/ui/`, the front door — and ending `?? FixturePack`, so
 * every typo and every stale link drew a pack bench full of invented data. That
 * is most of why the interface read as a demo: the demo namespace had the good
 * URLs and nothing linked to anything.
 *
 * # Fixtures are a build mode, not a route
 *
 * `import.meta.env.MODE` is substituted with a literal at build time, so in a
 * production build the branch below is `if ("production" === "review")` and
 * Rollup drops it, the dynamic import, and the whole fixture module with it.
 * The review build — what `npm run render` measures — keeps them.
 *
 * The alternative was to ship fixtures and rely on nobody visiting them. This
 * repository has enough experience of rules that hold only while everyone
 * remembers them.
 */

/**
 * **An async bootstrap rather than a top-level await**, and the reason is the
 * device rather than the syntax: top-level await needs a build target above the
 * one this bundle is compiled for, and D5's handhelds are not where to find out
 * which browser a warehouse actually has. Rollup still drops the branch — the
 * condition is a literal after substitution — so the fixture module is absent
 * from production either way.
 */
async function boot(): Promise<void> {
  // **Before anything fetches.** The mobile host rebinds where the server is
  // and what credential reaches it; a screen that mounted first would ask the
  // wrong origin without one. Same build-mode shape as the fixtures below and
  // for the same reason — in every other build this condition is a literal and
  // Rollup drops the branch, this import and the Tauri plugin with it.
  if (import.meta.env.MODE === "mobile") {
    const { bootstrapMobile } = await import("@app/platform/mobile");
    bootstrapMobile();
  }

  const routes: Screen[] = [...LIVE];

  if (import.meta.env.MODE === "review") {
    const { FIXTURES } = await import("@app/routing/fixtures");
    const { REVIEW_ONLY } = await import("@app/routing/review");
    routes.push(...FIXTURES, ...REVIEW_ONLY);
  }

  const host = document.getElementById("root");
  if (!host) throw new Error("no #root to mount into");

  createRoot(host).render(
    <StrictMode>
      <Router routes={routes} notFound={(path) => <NotFound path={path} />} />
    </StrictMode>,
  );
}

void boot();
