import type { ReactElement, ReactNode } from "react";
import { Tooltip as T } from "radix-ui";

import s from "./overlay.module.css";

/** A short label on hover or keyboard focus. The child must take a ref. */
export function Tooltip({
  content,
  side = "bottom",
  children,
}: {
  content: ReactNode;
  side?: "top" | "right" | "bottom" | "left" | undefined;
  children: ReactElement;
}) {
  return (
    <T.Root>
      <T.Trigger asChild>{children}</T.Trigger>
      <T.Portal>
        <T.Content side={side} sideOffset={6} className={s.tooltip}>
          {content}
        </T.Content>
      </T.Portal>
    </T.Root>
  );
}
