import type { ReactNode } from "react";
import { CircleCheck, TriangleAlert } from "lucide-react";

import { cx } from "@ui/index";

import s from "./settings.module.css";

/** An inline message: a failure beside what failed, or a confirmation. */
export function Alert({
  tone,
  onDismiss,
  children,
}: {
  tone: "danger" | "success";
  onDismiss?: (() => void) | undefined;
  children: ReactNode;
}) {
  return (
    <div className={cx(s.alert, tone === "danger" ? s.alertDanger : s.alertSuccess)} role={tone === "danger" ? "alert" : "status"}>
      {tone === "danger" ? <TriangleAlert aria-hidden /> : <CircleCheck aria-hidden />}
      <span className={s.alertText}>{children}</span>
      {onDismiss && (
        <button type="button" className={s.alertClose} onClick={onDismiss}>
          Dismiss
        </button>
      )}
    </div>
  );
}
