import type { ReactNode } from "react";
import { Key } from "@design/index";
import type { NavSheet as NavSheetState } from "./useNavSheet";
import styles from "./nav-sheet.module.css";

/**
 * The rail, presented as a column or as a sheet.
 *
 * One element at both widths — see [`useNavSheet`] for why a second copy is not
 * an option. Above 62rem this is the ordinary navigation column and everything
 * below is inert markup the stylesheet ignores.
 */
export function NavSheet({ nav, children }: { nav: NavSheetState; children: ReactNode }) {
  const asSheet = nav.narrow && nav.open;
  return (
    <div
      // The shells' grids place it by this, at both widths.
      data-region="chrome"
      className={styles.sheet}
      data-open={asSheet ? "" : undefined}
      ref={nav.sheetRef}
      // **Only a dialog while it is one.** A column beside the work that
      // claimed `aria-modal` would tell a screen reader the rest of the page
      // does not exist, on the width where all of it does.
      {...(asSheet ? { role: "dialog", "aria-modal": true, "aria-label": "Menu" } : {})}
    >
      {/* `display: contents` above the breakpoint, so the column's grid
          placement is exactly what it was before the sheet existed. */}
      <div className={styles.scroll}>{children}</div>
      {asSheet && (
        <div className={styles.close}>
          {/* At the bottom, for the reason D134 puts the Floor dock there: this
              is a phone held low and driven with a thumb, and the top corner of
              it is the least reachable place on the device. Escape does the
              same thing for anybody with a keyboard. */}
          <Key size="small" block onClick={nav.hide}>
            Close
          </Key>
        </div>
      )}
    </div>
  );
}

/**
 * The control that opens it. Drawn by [`Chrome`], and only where there is a rail.
 *
 * **"Menu", not "Work".** The rail's landmark is `Work`, because that is what
 * the list is (D110 navigates by job, not by entity). This says what pressing
 * it does, which is what a control's label is for — and it is the word somebody
 * holding a scanner recognises without reading.
 *
 * An ordinary small `Key`. The rail already puts one in the chrome for Sign
 * out, so this is not a new kind of control, and an unlit key leaves D118's
 * rule — the chrome draws no *lit* key — and every one of the render gate's
 * hand-counted expectations exactly where they were.
 */
export function NavKey({ nav }: { nav: NavSheetState }) {
  return (
    <span className={styles.trigger}>
      <Key size="small" expanded={nav.open} onClick={nav.open ? nav.hide : nav.show}>
        Menu
      </Key>
    </span>
  );
}
