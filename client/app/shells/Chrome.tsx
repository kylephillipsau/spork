import type { ReactNode } from "react";
import { Link, Tag } from "@design/index";
import { href } from "@app/routing/location";
import { NavKey } from "@app/nav/NavSheet";
import type { NavSheet } from "@app/nav/useNavSheet";
import styles from "./chrome.module.css";

/**
 * The top bar, once, for all four shells.
 *
 * # It was written four times, and the copies had drifted
 *
 * Every shell hand-rolled the same row and every shell carried its own `.mark`
 * and `.title` rules. Three of the four were byte-identical and the fourth
 * differed by two font sizes — which is the shape drift always has: not a
 * disagreement anybody decided, just the second copy nobody updated. The
 * wordmark's letter-spacing was a bare `0.24em` sitting next to a `--track-legend`
 * token doing the same job for the line beneath it.
 *
 * # What it draws, and what it never draws
 *
 * D118: the chrome is chassis. No `Face`, no `EmptySlot`, no `Chooser`, no
 * camera, no *lit* key — those are the things the render gate counts, and
 * keeping them out of here is why every per-route expectation survived the
 * chrome getting a locator on every surface.
 *
 * Exactly one link home, on every surface, and it is the wordmark. On Floor it
 * is the whole of the navigation.
 *
 * `site` and `who` are optional because [`PlainShell`] legitimately has
 * neither: setup runs before a session exists, and a password screen is about
 * an account rather than about work.
 */
export function Chrome({
  title,
  site,
  who,
  locator,
  badge,
  nav,
}: {
  title: string;
  /** Absent on `PlainShell`, which is about an account rather than about work. */
  site?: string | undefined;
  who?: string | undefined;
  /** The scan bar (D111). */
  locator?: ReactNode | undefined;
  badge?: { label: string; count: number } | undefined;
  /**
   * The sheet this bar opens, on the shells that have a rail.
   *
   * **The bar pins itself below 48rem because of this.** A control that opens
   * the navigation is worth nothing at the top of a worklist somebody has
   * scrolled to the bottom of — which is the same failure as the rail being
   * below the work, arrived at from the other end. Absent on Floor and Plain,
   * which have no rail, and there the bar scrolls away as it always did.
   */
  nav?: NavSheet | undefined;
}) {
  return (
    <div data-region="chrome" className={nav ? `${styles.chrome} ${styles.pinned}` : styles.chrome}>
      {nav && <NavKey nav={nav} />}
      <span className={styles.mark}>
        <Link href={href("/")} on="chassis">
          Nylonite
        </Link>
      </span>
      <span className={styles.title}>{title}</span>
      {/* **Ahead of the tags in the DOM**, because that is where it sits when
          there is room for one line. Below 48rem it is ordered last and given a
          full row of its own — see the stylesheet. */}
      {locator && <span className={styles.locator}>{locator}</span>}
      {/* **One unit, so they wrap together or not at all.** As separate flex
          items the site went right on the first line and the operator wrapped
          alone onto a second — where and who are one statement and reading
          half of it on its own line is worse than reading both a line down. */}
      <span className={styles.tags}>
        {badge && badge.count > 0 && (
          <Tag>
            {badge.label} {badge.count}
          </Tag>
        )}
        {site && <Tag dyed>{site}</Tag>}
        {who && <Tag>{who}</Tag>}
      </span>
    </div>
  );
}
