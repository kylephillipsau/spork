import { useEffect, useState } from "react";

/**
 * The browser tab's title: what is open, then which screen, then Spork —
 * "SO-1042 · Acme Foods · Order · Spork" — so tabs side by side, and the back
 * button's history, tell one order or item from another.
 *
 * The screen is the Router's to say, as soon as the path resolves; what it is
 * about is each page's, once it has read it. Something open in front of a
 * page, as an item's drawer over the list, is said before the page.
 */

type Said = { text: string; front: boolean };

const said = new Map<symbol, Said>();
let screen = "";

/** The title from what is said and the screen: the front first, then the page, then the screen and Spork. */
export function titleOf(screenName: string, parts: readonly Said[]): string {
  const ordered = [...parts.filter((p) => p.front), ...parts.filter((p) => !p.front)].map((p) => p.text);
  return [...ordered, screenName, "Spork"].filter(Boolean).join(" · ");
}

function show(): void {
  document.title = titleOf(screen, [...said.values()]);
}

/** Which screen this is, from the Router. */
export function titleScreen(name: string): void {
  screen = name;
  show();
}

/**
 * Say what this page is about in the tab's title: its order number, an item's
 * code and description, a bin's code. Nothing said while it is still being
 * read; unsaid when it goes. `front` for something open in front of a page.
 */
export function usePageTitle(parts: readonly (string | null | undefined | false)[], front = false): void {
  const text = parts.filter(Boolean).join(" · ");
  const [me] = useState(() => Symbol("title"));
  useEffect(() => {
    if (text) said.set(me, { text, front });
    else said.delete(me);
    show();
    return () => {
      said.delete(me);
      show();
    };
  }, [me, text, front]);
}
