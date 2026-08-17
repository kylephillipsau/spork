import { pattern } from "@domain/routing";
import type { Screen } from "./Router";
import { Gallery } from "../../src/Gallery";

/**
 * Things that exist to be looked at rather than used.
 *
 * The gallery draws the design system's own vocabulary. It is not a screen of
 * the product and a customer has no reason to reach it, so it lives with the
 * fixtures in the review build.
 */
export const REVIEW_ONLY: readonly Screen[] = [
  {
    id: "gallery",
    path: "/gallery",
    title: "Gallery",
    surface: "desk",
    pattern: pattern("/gallery"),
    // Its own shell, like every fixture: no session, no rail, no network.
    own: true,
    render: () => <Gallery />,
  },
];
