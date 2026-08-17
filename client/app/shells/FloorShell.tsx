import type { ReactNode } from "react";
import { Chrome } from "./Chrome";
import { Regions, useRegion } from "./slots";
import styles from "./floor-shell.module.css";

/**
 * The Floor surface: walking, gloved, one-handed, a handheld at waist height.
 *
 * **The primary key is at the bottom, and that is the whole difference.** The
 * Bench shell puts it at the top because a bench monitor is read top-down at
 * eye level. A handheld is held low and driven with the thumb of the hand
 * holding it, so the reachable third of the screen is the bottom third — and
 * the top of a five-inch screen is where the least reachable control on the
 * device is. Most warehouse software puts the button in the same place on both
 * surfaces and is therefore wrong on one of them.
 *
 * D109 puts that difference in the shell rather than in every screen, which is
 * what stops it being decided again per screen and decided differently.
 *
 * `dock` is that reachable third. It is a slot rather than a `primary` prop
 * because the action at the bottom of a capture session changes as the session
 * moves — record the figures, then finish — and a shell that took a label and
 * a callback would be modelling one step of a flow it does not know about.
 *
 * A live screen fills it with [`Dock`] rather than this prop, because the shell
 * now outlives the screen and the buttons in there hold the screen's state. The
 * prop stays for fixtures, which draw their own shell with no frame above it.
 */
export function FloorShell({
  title,
  site,
  who,
  dock,
  locator,
  children,
}: {
  title: string;
  site: string;
  who: string;
  /** Fixtures only. Live screens render [`Dock`] from inside their own tree. */
  dock?: ReactNode;
  /** The chrome's scan bar (D111). Absent on the screen that owns the scanner:
   *  vertical space is the scarce resource here, and two locators stacked on a
   *  handheld is a worse answer to D111 than one. */
  locator?: ReactNode;
  children: ReactNode;
}) {
  const [dockNode, setDockNode] = useRegion();
  return (
    <div className={styles.page}>
      {/* Thin, because a handheld's vertical space is the scarce thing and
          the chrome is not what anybody came for. One line, no wrap: on a
          narrow screen the identity tags are what give way, not the title. */}
      <Chrome title={title} site={site} who={who} locator={locator} />

      {/* The scrolling half. `min-height: 0` on a flex child is what lets it
          scroll instead of growing the page — without it the dock is pushed
          off the bottom of a long worklist, which is the one place it may
          never be. */}
      <Regions dock={dockNode}>
        <div className={styles.body} data-region="work">
          {children}
        </div>

        {/* Drawn whether or not anything fills it: the screen inside needs
            somewhere to portal into, and it cannot be told about a container
            that only exists once it has said it wants one. Empty, the
            stylesheet takes it out of the layout entirely. */}
        <div className={styles.dock} data-region="dock" ref={setDockNode}>
          {dock}
        </div>
      </Regions>
    </div>
  );
}
