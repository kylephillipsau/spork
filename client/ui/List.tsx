import type { ReactNode } from "react";

import { cx } from "./cx";
import s from "./list.module.css";

/**
 * Rows of work on a handheld: a location or tag first, the item and its
 * badges, a line of detail, and one action at the end. Where a desktop screen
 * uses a DataTable, a handheld screen uses this, because at scanner width a
 * table's columns are too narrow to read.
 *
 * Inside a Card with `padded={false}`, it runs edge to edge between hairlines.
 */
export function List({ label, children }: { label?: string | undefined; children: ReactNode }) {
  return (
    <ul className={s.list} aria-label={label}>
      {children}
    </ul>
  );
}

export function ListItem({
  lead,
  title,
  badges,
  description,
  meta,
  action,
  current = false,
  done = false,
}: {
  /** What you look for first: a bin, a location, a count. */
  lead?: ReactNode | undefined;
  /** The item, usually its code. */
  title: ReactNode;
  /** Short tags beside the title. */
  badges?: ReactNode | undefined;
  /** One line under the title: a description or a note. */
  description?: ReactNode | undefined;
  /** Figures or reasons, in small muted text. */
  meta?: ReactNode | undefined;
  /** One button, at the end of the row. */
  action?: ReactNode | undefined;
  /** The row the screen is working on now. */
  current?: boolean | undefined;
  /** Finished: dimmed, still readable. */
  done?: boolean | undefined;
}) {
  return (
    <li className={cx(s.item, current && s.current, done && s.done)} aria-current={current || undefined}>
      {lead != null && <div className={s.lead}>{lead}</div>}
      <div className={s.body}>
        <div className={s.head}>
          <span className={s.title}>{title}</span>
          {badges}
        </div>
        {description != null && <div className={s.description}>{description}</div>}
        {meta != null && <div className={s.meta}>{meta}</div>}
      </div>
      {action != null && <div className={s.action}>{action}</div>}
    </li>
  );
}
