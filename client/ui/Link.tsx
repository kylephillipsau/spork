import type { ComponentPropsWithRef } from "react";

import { cx } from "./cx";
import s from "./misc.module.css";

/**
 * The kit's only anchor. The Router intercepts clicks on any <a> whose path is
 * a screen, so this is a plain link that navigates without a page load.
 */
export function Link({
  variant = "text",
  className,
  ...rest
}: ComponentPropsWithRef<"a"> & { variant?: "text" | "plain" | undefined }) {
  return <a className={cx(variant === "text" ? s.link : s.plainLink, className)} {...rest} />;
}
