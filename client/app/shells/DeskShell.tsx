import type { ReactNode } from "react";
import { Chrome } from "./Chrome";
import { NavSheet } from "@app/nav/NavSheet";
import { useNavSheet } from "@app/nav/useNavSheet";
import { Regions, useRegion } from "./slots";
import styles from "./desk-shell.module.css";

/**
 * The Desk surface: seated, analytical, comparing evidence and deciding.
 *
 * # What makes this a third shell rather than a wide Bench
 *
 * It shares `density="desk"` with Bench, and a reviewer will reasonably ask why
 * it is not `BenchShell` with a `max-width` prop. Two reasons, and the second is
 * the structural one.
 *
 * D109 says the surfaces differ posturally. Bench is *repetition* — several
 * hundred acts a day, one work column, every interaction paid for hundreds of
 * times. Desk is *density* — comparing two things and deciding between them,
 * where the constraint is how much evidence fits in front of you at once.
 *
 * And Desk has a **rail**. D111's third navigation mechanism is *"inspect
 * without leaving"*: a panel that opens beside the work rather than over it, so
 * the queue keeps its place while one row is examined. A shell with a
 * two-column layout and one without are not the same shell with a different
 * width, and collapsing them would put the rail's layout into every screen that
 * does not have one.
 *
 * `rail` is a slot for the same reason `FloorShell`'s `dock` is: what goes in it
 * changes with what is selected, and a shell taking a `finding` prop would be
 * modelling a screen it does not know about.
 *
 * **The rail is beside, never over.** D119 forbids anything translucent laid
 * over text, and an overlay would also hide the row it describes — which is the
 * one thing the operator is comparing it against.
 */
export function DeskShell({
  title,
  site,
  who,
  badge,
  evidence,
  rail,
  locator,
  children,
}: {
  title: string;
  site: string;
  who: string;
  /** D112: work waiting for you, at this site, now. Zero hides — a badge
   *  showing nothing to do is a badge people stop reading. */
  badge?: { label: string; count: number };
  /** The evidence panel, when something is selected.
   *
   *  **Renamed from `rail`.** D110 calls navigation "the work rail", and this
   *  shell was using the same word for the panel beside the work. Two meanings
   *  for one name in one file is a misreading waiting to happen, and the work
   *  rail arriving is what forced the choice. */
  /** Fixtures only. Live screens render [`Evidence`] from inside their own
   *  tree, because what is selected is the screen's state and this shell now
   *  outlives it. */
  evidence?: ReactNode;
  /** The work rail (D110). */
  rail?: ReactNode;
  /** The chrome's scan bar (D111). */
  locator?: ReactNode;
  children: ReactNode;
}) {
  const [evidenceNode, setEvidenceNode] = useRegion();
  const nav = useNavSheet();
  return (
    <div className={styles.page}>
      <Chrome
        title={title}
        site={site}
        who={who}
        locator={locator}
        badge={badge}
        {...(rail ? { nav } : {})}
      />

      <div className={styles.body}>
        {rail && <NavSheet nav={nav}>{rail}</NavSheet>}
        {/* Two columns always, and the stylesheet collapses the second while
            it is empty. The screen inside cannot portal into a container that
            only appears once it has something to put there. */}
        <div className={styles.split}>
          <Regions evidence={evidenceNode}>
            <div className={styles.work} data-region="work">
              {children}
            </div>
            <aside className={styles.rail} ref={setEvidenceNode}>
              {evidence}
            </aside>
          </Regions>
        </div>
      </div>
    </div>
  );
}
