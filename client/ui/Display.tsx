import type { ReactNode } from "react";
import { ChevronRight } from "lucide-react";

import { cx } from "./cx";
import { Count } from "./Badge";
import { Link } from "./Link";
import s from "./misc.module.css";

export function Spinner({ size = 16, label }: { size?: number | undefined; label?: string | undefined }) {
  return (
    <span
      className={s.spinner}
      style={{ width: size, height: size }}
      role={label ? "status" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    />
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className={s.kbd}>{children}</kbd>;
}

/** A grey placeholder bar while something loads. */
export function Skeleton({ width = "100%", height = 14 }: { width?: number | string | undefined; height?: number | undefined }) {
  return <span className={s.skeleton} style={{ width, height }} aria-hidden />;
}

/** Initials in a circle. */
export function Avatar({ name, size = 28 }: { name: string; size?: number | undefined }) {
  const initials =
    name
      .split(/[\s.@_-]+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((p) => p[0]?.toUpperCase())
      .join("") || "?";
  return (
    <span className={s.avatar} style={{ width: size, height: size, fontSize: size * 0.4 }} aria-hidden>
      {initials}
    </span>
  );
}

/** What a list or page shows when there is nothing in it. */
export function EmptyState({
  icon,
  title,
  description,
  action,
}: {
  icon?: ReactNode | undefined;
  title: string;
  description?: ReactNode | undefined;
  action?: ReactNode | undefined;
}) {
  return (
    <div className={s.empty}>
      {icon && <div className={s.emptyIcon}>{icon}</div>}
      <p className={s.emptyTitle}>{title}</p>
      {description && <p className={s.emptyText}>{description}</p>}
      {action && <div className={s.emptyAction}>{action}</div>}
    </div>
  );
}

/** A bordered surface. `title` and `actions` make a header row. */
export function Card({
  title,
  count,
  description,
  actions,
  padded = true,
  children,
}: {
  title?: ReactNode | undefined;
  /** Shown beside the title: how many rows the card holds. */
  count?: number | undefined;
  description?: ReactNode | undefined;
  actions?: ReactNode | undefined;
  padded?: boolean | undefined;
  children?: ReactNode | undefined;
}) {
  return (
    <section className={s.card}>
      {(title || actions) && (
        <header className={s.cardHeader}>
          <div>
            {title && (
              <h2 className={s.cardTitle}>
                {title}
                {count !== undefined && <Count value={count} showZero />}
              </h2>
            )}
            {description && <p className={s.cardDescription}>{description}</p>}
          </div>
          {actions && <div className={s.actions}>{actions}</div>}
        </header>
      )}
      {children != null && <div className={cx(padded && s.cardBody)}>{children}</div>}
    </section>
  );
}

/** The top of every desktop page: title, one-line description, actions right. */
export function PageHeader({
  title,
  description,
  actions,
}: {
  title: ReactNode;
  description?: ReactNode | undefined;
  actions?: ReactNode | undefined;
}) {
  return (
    <div className={s.pageHeader}>
      <div className={s.pageHeading}>
        <h1 className={s.pageTitle}>{title}</h1>
        {description && <p className={s.pageDescription}>{description}</p>}
      </div>
      {actions && <div className={s.actions}>{actions}</div>}
    </div>
  );
}

export interface Crumb {
  label: string;
  href?: string | undefined;
}

export function Breadcrumbs({ items }: { items: readonly Crumb[] }) {
  return (
    <nav aria-label="Breadcrumb" className={s.crumbs}>
      {items.map((c, i) => {
        const last = i === items.length - 1;
        return (
          <span key={`${c.label}-${i}`} className={s.crumb}>
            {c.href && !last ? (
              <Link variant="plain" href={c.href} className={s.crumbLink}>
                {c.label}
              </Link>
            ) : (
              <span className={last ? s.crumbCurrent : undefined} aria-current={last ? "page" : undefined}>
                {c.label}
              </span>
            )}
            {!last && <ChevronRight className={s.crumbSep} aria-hidden />}
          </span>
        );
      })}
    </nav>
  );
}

/** Vertical rhythm without margins. */
export function Stack({ gap = 4, children }: { gap?: 1 | 2 | 3 | 4 | 5 | 6 | 8 | undefined; children: ReactNode }) {
  return <div style={{ display: "flex", flexDirection: "column", gap: `var(--ui-space-${gap})` }}>{children}</div>;
}

/** A horizontal row that wraps. */
export function Inline({
  gap = 2,
  align = "center",
  justify = "start",
  children,
}: {
  gap?: 1 | 2 | 3 | 4 | 6 | undefined;
  align?: "start" | "center" | "end" | "baseline" | undefined;
  justify?: "start" | "between" | "end" | undefined;
  children: ReactNode;
}) {
  return (
    <div
      style={{
        display: "flex",
        flexWrap: "wrap",
        gap: `var(--ui-space-${gap})`,
        alignItems: align === "start" || align === "end" ? `flex-${align}` : align,
        justifyContent: justify === "between" ? "space-between" : justify === "end" ? "flex-end" : "flex-start",
      }}
    >
      {children}
    </div>
  );
}
