import type { ReactNode } from "react";
import { cx } from "../cx";
import styles from "../materials/anodise.module.css";

/**
 * L4 — a key.
 *
 * `live` is the one key on a screen that is lit from within: the thing the
 * operator is about to do. Two lit keys on one screen means neither is.
 *
 * The prop list is deliberately short. No `className`, no `style`, and no
 * `variant` escape hatch — a control that can be restyled at the call site
 * is a control that stops being recognisable across thirty screens.
 */
export function Key({
  live = false,
  size = "regular",
  block = false,
  type = "button",
  expanded,
  disabled,
  onClick,
  children,
}: {
  live?: boolean;
  size?: "regular" | "small";
  block?: boolean;
  type?: "button" | "submit";
  /**
   * This key reveals something, and whether it is revealed now.
   *
   * A named prop rather than a spread of arbitrary ARIA, for D123's reason: a
   * control the call site can decorate is a control that means something
   * different on each screen. Omitted, the attribute is absent — a key that
   * reveals nothing must not claim to.
   */
  expanded?: boolean;
  disabled?: boolean;
  onClick?: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type={type}
      data-layer="key"
      data-material="anodise"
      className={cx(
        styles.key,
        live && styles.live,
        size === "small" && styles.keySmall,
        block && styles.keyBlock,
      )}
      disabled={disabled ?? false}
      {...(expanded === undefined ? {} : { "aria-expanded": expanded })}
      onClick={onClick ?? undefined}
    >
      {children}
    </button>
  );
}
