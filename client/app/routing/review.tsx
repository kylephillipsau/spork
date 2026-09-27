import { pattern } from "@domain/routing";
import type { Screen } from "./Router";
import { UiKit } from "../../src/UiKit";

/**
 * Things that exist to be looked at rather than used.
 *
 * The UI kit page draws the kit's own components (D171). It is not a screen of
 * the product and a customer has no reason to reach it, so it lives with the
 * fixtures in the review build.
 */
export const REVIEW_ONLY: readonly Screen[] = [
  {
    id: "ui-kit",
    path: "/ui-kit",
    title: "UI kit",
    surface: "desk",
    pattern: pattern("/ui-kit"),
    // Its own frame, like every fixture: no session, no network.
    own: true,
    render: () => <UiKit />,
  },
];
