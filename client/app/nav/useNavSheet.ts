import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { usePath } from "@app/routing/Router";

/**
 * The width at which the rail is a column beside the work.
 *
 * Below it there is no room for one, and the same rail is drawn as a sheet.
 * **The number is here and repeated in `nav-sheet.module.css` and in the two
 * shells' stylesheets**, because a media query cannot read a custom property
 * and this client has no CSS build step that would let it. Changing it means
 * changing all four, and the comment in each says so.
 */
export const RAIL_IS_A_COLUMN = "(min-width: 62rem)";

export interface NavSheet {
  /** Drawn as a sheet rather than as a column. */
  readonly narrow: boolean;
  readonly open: boolean;
  readonly show: () => void;
  readonly hide: () => void;
  readonly sheetRef: React.RefObject<HTMLDivElement | null>;
}

/** Whether the rail has room to be a column, as a store rather than a hook of
 *  its own: `matchMedia` is an event source, which is what this is for. */
function useWide(): boolean {
  return useSyncExternalStore(
    (notify) => {
      const query = window.matchMedia(RAIL_IS_A_COLUMN);
      query.addEventListener("change", notify);
      return () => query.removeEventListener("change", notify);
    },
    () => window.matchMedia(RAIL_IS_A_COLUMN).matches,
    // Server-rendered nothing: there is no document to measure, and a column
    // is the layout that needs no interaction to be usable.
    () => true,
  );
}

/**
 * Opening and closing the navigation on a screen too narrow for a column.
 *
 * # Why a sheet at all
 *
 * The rail used to stack *under* the work on a phone (`order: 2`), which was
 * chosen over stacking above it — eleven navigation rows between the top of the
 * page and the thing anybody came for. Both are the same failure at different
 * ends: to navigate you scroll the whole worklist. Fourteen destinations in five
 * groups rule out a tab bar, and a bottom bar would sit exactly where D134 puts
 * the Floor dock. So: the same rail, in a sheet, with the chrome pinned so the
 * way to open it is on the screen wherever you have scrolled to.
 *
 * # It is the same element either way
 *
 * Not a second copy shown at narrow width. That was tried once for the landing
 * screen and left two `nav` landmarks in the document, twenty links where there
 * are ten destinations, and a hidden Sign out that a click found before the
 * visible one. Here the shell draws one rail and this decides how it is
 * presented, so there is exactly one of everything at every width.
 */
export function useNavSheet(): NavSheet {
  const wide = useWide();
  const [open, setOpen] = useState(false);
  const sheetRef = useRef<HTMLDivElement | null>(null);
  // **Whatever had focus, not a ref on the trigger.** Restoring to the element
  // that opened it is the same answer whether that was the Menu key or a
  // keyboard shortcut somebody adds later, and it keeps a ref out of a
  // primitive's prop list (D123).
  const opener = useRef<HTMLElement | null>(null);
  const path = usePath();
  const narrow = !wide;

  const hide = useCallback(() => {
    setOpen(false);
    opener.current?.focus();
  }, []);

  // **Arriving is what closes it.** Every row in the rail is an anchor the
  // router intercepts, so nothing here is told about the navigation — the path
  // changing is the signal. Without this the sheet would still be over the
  // screen it just took you to.
  useEffect(() => {
    setOpen(false);
  }, [path]);

  // A viewport that grows past the breakpoint while the sheet is up — a phone
  // turned sideways — leaves the rail a column again, and a column is not a
  // dialog. Keyed off the width rather than off a close, because nobody closed
  // it.
  useEffect(() => {
    if (wide) setOpen(false);
  }, [wide]);

  useEffect(() => {
    if (!open || !narrow) return;

    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";

    const focusable = () =>
      [
        ...(sheetRef.current?.querySelectorAll<HTMLElement>("a[href], button:not([disabled])") ??
          []),
      ].filter((node) => node.offsetParent !== null);

    focusable()[0]?.focus();

    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        hide();
        return;
      }
      if (event.key !== "Tab") return;
      // The sheet is opaque and covers the whole screen, so tabbing out of it
      // moves focus onto things nobody can see. `aria-modal` says as much to a
      // screen reader; this is the same statement to a keyboard.
      const nodes = focusable();
      if (nodes.length === 0) return;
      const first = nodes[0];
      const last = nodes[nodes.length - 1];
      const here = document.activeElement;
      if (event.shiftKey && (here === first || !sheetRef.current?.contains(here))) {
        event.preventDefault();
        last?.focus();
      } else if (!event.shiftKey && here === last) {
        event.preventDefault();
        first?.focus();
      }
    };

    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = previous;
    };
  }, [open, narrow, hide]);

  const show = useCallback(() => {
    opener.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setOpen(true);
  }, []);

  return { narrow, open, show, hide, sheetRef };
}
