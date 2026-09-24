import { useMemo, useState, type ReactNode } from "react";
import { ArrowDown, ArrowUp, ArrowUpDown } from "lucide-react";

import { cx } from "./cx";
import { Skeleton } from "./Display";
import s from "./table.module.css";

export interface Column<T> {
  key: string;
  header: ReactNode;
  cell: (row: T) => ReactNode;
  /** Makes the header a sort button. Return what to compare by. */
  sort?: ((row: T) => string | number | null | undefined) | undefined;
  align?: "left" | "right" | "center" | undefined;
  /** CSS width, e.g. "120px" or "20%". */
  width?: string | undefined;
  /** Monospace and tabular figures: codes, references, quantities. */
  mono?: boolean | undefined;
  /**
   * Takes the width the other columns leave, and truncates with an ellipsis
   * rather than wrapping. Usually the one name-like column: customer, item.
   */
  grow?: boolean | undefined;
}

type Direction = "asc" | "desc";

/**
 * A sortable table with a sticky header, row hover, and loading and empty
 * states. Sorting is client-side over the rows given.
 */
export function DataTable<T>({
  columns,
  rows,
  rowKey,
  onRowClick,
  selectedKey,
  loading = false,
  empty,
  initialSort,
  "aria-label": ariaLabel,
}: {
  columns: readonly Column<T>[];
  rows: readonly T[];
  rowKey: (row: T) => string;
  onRowClick?: ((row: T) => void) | undefined;
  selectedKey?: string | undefined;
  loading?: boolean | undefined;
  /** Shown in place of the body when there are no rows. */
  empty?: ReactNode | undefined;
  initialSort?: { key: string; direction: Direction } | undefined;
  "aria-label"?: string | undefined;
}) {
  const [sort, setSort] = useState(initialSort);

  const sorted = useMemo(() => {
    const col = sort && columns.find((c) => c.key === sort.key);
    if (!col?.sort || !sort) return rows;
    const by = col.sort;
    const dir = sort.direction === "asc" ? 1 : -1;
    return [...rows].sort((a, b) => {
      const x = by(a);
      const y = by(b);
      if (x == null && y == null) return 0;
      if (x == null) return 1;
      if (y == null) return -1;
      if (typeof x === "number" && typeof y === "number") return (x - y) * dir;
      return String(x).localeCompare(String(y), undefined, { numeric: true }) * dir;
    });
  }, [rows, columns, sort]);

  function toggle(key: string) {
    setSort((cur) =>
      cur?.key !== key ? { key, direction: "asc" } : cur.direction === "asc" ? { key, direction: "desc" } : undefined,
    );
  }

  return (
    <div className={s.wrap}>
      <table className={s.table} aria-label={ariaLabel} aria-busy={loading || undefined}>
        <thead>
          <tr>
            {columns.map((c) => {
              const active = sort?.key === c.key;
              return (
                <th
                  key={c.key}
                  scope="col"
                  className={cx(s.th, c.align && s[c.align], c.grow && s.growHead)}
                  style={c.width ? { width: c.width } : undefined}
                  aria-sort={active ? (sort.direction === "asc" ? "ascending" : "descending") : undefined}
                >
                  {c.sort ? (
                    <button type="button" className={s.sortButton} onClick={() => toggle(c.key)}>
                      {c.header}
                      {active ? (
                        sort.direction === "asc" ? (
                          <ArrowUp className={s.sortIcon} />
                        ) : (
                          <ArrowDown className={s.sortIcon} />
                        )
                      ) : (
                        <ArrowUpDown className={cx(s.sortIcon, s.sortIdle)} />
                      )}
                    </button>
                  ) : (
                    c.header
                  )}
                </th>
              );
            })}
          </tr>
        </thead>
        <tbody>
          {loading
            ? Array.from({ length: 5 }, (_, i) => (
                <tr key={`sk-${i}`}>
                  {columns.map((c) => (
                    <td key={c.key} className={s.td}>
                      <Skeleton width={c.align === "right" ? 48 : "70%"} />
                    </td>
                  ))}
                </tr>
              ))
            : sorted.map((row) => {
                const key = rowKey(row);
                return (
                  <tr
                    key={key}
                    className={cx(onRowClick && s.clickable, selectedKey === key && s.selected)}
                    onClick={onRowClick ? () => onRowClick(row) : undefined}
                    aria-selected={selectedKey !== undefined ? selectedKey === key : undefined}
                  >
                    {columns.map((c) => (
                      <td key={c.key} className={cx(s.td, c.align && s[c.align], c.mono && s.mono, c.grow && s.grow)}>
                        {c.grow ? <span className={s.truncate}>{c.cell(row)}</span> : c.cell(row)}
                      </td>
                    ))}
                  </tr>
                );
              })}
        </tbody>
      </table>
      {!loading && rows.length === 0 && empty && <div className={s.emptyRow}>{empty}</div>}
    </div>
  );
}
