import type { ReactNode } from "react";
import { cx } from "../cx";
import styles from "../materials/anodise.module.css";

/**
 * L4 — somewhere to go.
 *
 * **A real `<a href>`, and that is the whole point.** Until this existed there
 * was not one anchor in the client: every screen was an isolated URL somebody
 * had to know and type. A `Key` with an `onClick` would have drawn the same
 * thing and given up middle-click, ctrl-click, open-in-new-tab, "copy link
 * address", and a screen reader saying *link* rather than *button*. None of
 * those is optional in software people use all day.
 *
 * It is also what keeps the render gate's existing numbers meaningful. The gate
 * asserts at most one lit key per screen and 29 routes carry hand-counted
 * `litKeys`; navigation built from `Key` would have added buttons to every
 * screen and invalidated all of them at once.
 *
 * **Never lit.** `Tabs` settled this argument already — *"a tab is not a thing
 * you are about to do, it is where you already are"* — and the same holds for a
 * destination. `current` is drawn with ground and an edge (D128), never with
 * the lamp, so a nav can never be mistaken for the one action on the screen.
 *
 * The prop list is short for D123's reason: no `className`, no `style`.
 */
export function Link({
  href,
  on = "face",
  current = false,
  block = false,
  children,
}: {
  href: string;
  /**
   * Which material it sits on (D118).
   *
   * The system is metal chassis and paper data, and a link behaves differently
   * on each. On a **face** it sits among prose and has to be findable, so it is
   * underlined. On the **chassis** it sits in a rail of its own, where an
   * underline under every row is noise — the row's position is what says it is
   * a destination.
   *
   * A closed enum rather than a `className`: D123 lets a primitive name its own
   * variants and forbids the call site inventing one.
   */
  on?: "face" | "chassis";
  /** Where you already are. Renders `aria-current="page"`. */
  current?: boolean;
  /** Fill the row — a handheld target rather than a word in a line. */
  block?: boolean;
  children: ReactNode;
}) {
  return (
    <a
      href={href}
      data-layer="key"
      data-material="anodise"
      className={cx(
        styles.link,
        on === "chassis" && styles.linkChassis,
        current && styles.linkHere,
        block && styles.linkBlock,
      )}
      {...(current ? { "aria-current": "page" as const } : {})}
    >
      {children}
    </a>
  );
}
