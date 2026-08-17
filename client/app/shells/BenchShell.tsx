import type { ReactNode } from "react";
import { Stack } from "@design/index";
import { Chrome } from "./Chrome";
import { NavSheet } from "@app/nav/NavSheet";
import { useNavSheet } from "@app/nav/useNavSheet";
import styles from "./bench-shell.module.css";

/**
 * The Bench surface: standing, stationary, scanner wedge and keyboard, several
 * hundred jobs a day.
 *
 * **Desk density, and the primary key at the top.** A bench monitor is read
 * top-down; the Floor shell puts its key at the bottom instead, because a
 * handheld is held at waist height and driven with a thumb. Most warehouse
 * software puts the button in the same place on both and is wrong on one of
 * them — so the difference lives in the shell rather than in every screen.
 */
export function BenchShell({
  title,
  site,
  who,
  rail,
  locator,
  children,
}: {
  title: string;
  site: string;
  who: string;
  /** The work rail. A slot rather than a prop the shell builds, for the same
   *  reason `dock` and the evidence panel are slots: what goes in it depends on
   *  a session this shell does not have. */
  rail?: ReactNode;
  /** The chrome's scan bar (D111). */
  locator?: ReactNode;
  children: ReactNode;
}) {
  const nav = useNavSheet();
  return (
    <div className={styles.page}>
      <Stack gap={5}>
        <Chrome
          title={title}
          site={site}
          who={who}
          locator={locator}
          {...(rail ? { nav } : {})}
        />
        <div className={styles.body}>
          {rail && <NavSheet nav={nav}>{rail}</NavSheet>}
          <div className={styles.work} data-region="work">
            {children}
          </div>
        </div>
      </Stack>
    </div>
  );
}
