import type { ReactNode } from "react";
import { Chrome } from "./Chrome";
import styles from "./plain-shell.module.css";

/**
 * One column, a wordmark, and no work context.
 *
 * **No site and no operator in the chrome, because the screens that use this
 * are not about work.** The other three shells all name where you are and who
 * you are, which D109's three surfaces earn: a bench, a floor and a desk are
 * all places you are doing a job. Setting a deployment up and changing your own
 * password are neither of those, and the first of them runs before a session
 * exists to name anything at all.
 *
 * It was called `SetupShell` for one commit, which was a name for the first
 * screen that used it rather than for what it is. The password screen is the
 * second, and it says whose password it is in its own content — where it
 * belongs on a screen about an account, rather than in a corner as a tag.
 *
 * Desk density: both of these are done sitting down, deliberately, once.
 */
export function PlainShell({
  title,
  locator,
  align = "top",
  children,
}: {
  title: string;
  /** The chrome's scan bar (D111). */
  locator?: ReactNode;
  /**
   * `centre` for a form of fixed size, which is sign in and nothing else yet.
   * A list belongs at the top: centring one moves every row when a row arrives.
   */
  align?: "top" | "centre";
  children: ReactNode;
}) {
  return (
    <div className={styles.page}>
      <Chrome title={title} locator={locator} />
      <div
        className={align === "centre" ? `${styles.column} ${styles.centred}` : styles.column}
        data-region="work"
      >
        {children}
      </div>
    </div>
  );
}
