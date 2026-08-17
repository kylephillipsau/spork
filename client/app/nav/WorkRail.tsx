import { Badge, Key, Link, Stack } from "@design/index";
import { href } from "@app/routing/location";
import { RAIL } from "./rail";
import type { BadgeKey, Destination } from "./rail";
import styles from "./nav.module.css";

/**
 * The work rail (D110).
 *
 * `WorkRail` rather than `Rail`, because `rail.ts` beside it holds the data and
 * a case-only difference between two filenames is a build that works on Linux
 * and not on the laptop it was written on.
 *
 * Presentational: it is handed the groups, where you are, and the counts. That
 * keeps it drawable from a fixture with no session and no network, which is
 * what the render gate needs.
 *
 * # Two layouts, one component
 *
 * `rail` is the persistent column on Bench and Desk. `page` is full-width rows,
 * which is what the landing screen draws — and which is **Floor's navigation**.
 * A handheld is 430px wide with its dock already spoken for by D134's primary
 * action, so a squeezed column or an overlaid sheet would both be worse
 * answers than the screen the product already needs. The way there on Floor is
 * the wordmark, which every shell header already has.
 */
export function WorkRail({
  here,
  counts,
  onSignOut,
  layout = "rail",
}: {
  /** `Destination.id`, or null when you are somewhere the rail does not name. */
  here: string | null;
  counts: Readonly<Partial<Record<BadgeKey, number>>>;
  /** Signing out is an act rather than a destination, so the rail is handed
   *  one rather than building it — the same reason it takes `counts`. */
  onSignOut?: () => void;
  layout?: "rail" | "page";
}) {
  return (
    <nav className={layout === "rail" ? styles.rail : styles.page} aria-label="Work">
      <Stack gap={4}>
        {RAIL.map((group) => (
          <Stack key={group.group} gap={2}>
            <span className={styles.group}>{group.label}</span>
            <Stack gap={1}>
              {group.items.map((item) => (
                <Item
                  key={item.id}
                  item={item}
                  here={here === item.id}
                  count={item.badge ? counts[item.badge] : undefined}
                  block={layout === "page"}
                  {...(item.act && onSignOut ? { onAct: onSignOut } : {})}
                />
              ))}
            </Stack>
          </Stack>
        ))}
      </Stack>
    </nav>
  );
}

function Item({
  item,
  here,
  count,
  block,
  onAct,
}: {
  item: Destination;
  here: boolean;
  count: number | undefined;
  block: boolean;
  onAct?: () => void;
}) {
  if (item.act) {
    return (
      <span className={styles.item}>
        <Key size="small" block={block} onClick={onAct ?? (() => {})}>
          {item.label}
        </Key>
      </span>
    );
  }
  return (
    <span className={styles.item}>
      {/* An external destination keeps its own href untouched: it is a maud
          page the router does not own, and the click interceptor lets the
          browser have it. That is what "nothing is deleted for tidiness"
          costs, and it is cheap. */}
      <Link
        href={item.external ? item.path : href(item.path)}
        on="chassis"
        current={here}
        block={block}
      >
        {item.label}
        {count !== undefined && <Badge count={count} />}
      </Link>
    </span>
  );
}
