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
  tone: "danger" | "warning" | "success";
  onDismiss?: (() => void) | undefined;
  children: ReactNode;
}) {
  return (
    <div
      className={cx(s.alert, tone === "danger" ? s.alertDanger : tone === "warning" ? s.alertWarning : s.alertSuccess)}
      role={tone === "success" ? "status" : "alert"}
    >
      {tone === "success" ? <CircleCheck aria-hidden /> : <TriangleAlert aria-hidden />}
      <span className={s.alertText}>{children}</span>
      {onDismiss && (
        <button type="button" className={s.alertClose} onClick={onDismiss}>
          Dismiss
        </button>
      )}
    </div>
  );
}
