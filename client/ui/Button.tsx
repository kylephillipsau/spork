import type { ComponentPropsWithRef, ReactNode } from "react";

import { cx } from "./cx";
import { Spinner } from "./Display";
import { Tooltip } from "./Tooltip";
import s from "./button.module.css";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

export interface ButtonProps extends ComponentPropsWithRef<"button"> {
  variant?: ButtonVariant | undefined;
  size?: ButtonSize | undefined;
  /** An icon before the label, usually from lucide-react. */
  icon?: ReactNode | undefined;
  iconAfter?: ReactNode | undefined;
  /** Shows a spinner and disables the button while an action runs. */
  loading?: boolean | undefined;
  block?: boolean | undefined;
}

export function Button({
  variant = "secondary",
  size = "md",
  icon,
  iconAfter,
  loading = false,
  block = false,
  className,
  children,
  disabled,
  type = "button",
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      className={cx(s.button, s[variant], s[size], block && s.block, className)}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...rest}
    >
      {loading ? <Spinner size={14} /> : icon}
      {children != null && <span className={s.label}>{children}</span>}
      {iconAfter}
    </button>
  );
}

export interface IconButtonProps extends Omit<ButtonProps, "icon" | "iconAfter" | "children"> {
  /** Required: it is the accessible name and the tooltip. */
  label: string;
  icon: ReactNode;
  /** Hide the tooltip, e.g. inside a menu trigger that has its own. */
  noTooltip?: boolean | undefined;
}

export function IconButton({ label, icon, variant = "ghost", noTooltip, className, ...rest }: IconButtonProps) {
  const button = (
    <Button
      aria-label={label}
      variant={variant}
      icon={icon}
      className={cx(s.iconOnly, className)}
      {...rest}
    />
  );
  return noTooltip ? button : <Tooltip content={label}>{button}</Tooltip>;
}
