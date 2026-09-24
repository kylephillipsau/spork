import type { ReactElement, ReactNode } from "react";
import { Dialog as D, DropdownMenu as M } from "radix-ui";
import { Check, X } from "lucide-react";

import { cx } from "./cx";
import { Link } from "./Link";
import s from "./overlay.module.css";

interface OverlayProps {
  open?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  /** An element that opens it, when it is not controlled from outside. */
  trigger?: ReactElement | undefined;
  title: ReactNode;
  description?: ReactNode | undefined;
  /** Buttons along the bottom, primary last. */
  footer?: ReactNode | undefined;
  children?: ReactNode | undefined;
}

function rootProps(open: boolean | undefined, onOpenChange: ((open: boolean) => void) | undefined) {
  return {
    ...(open !== undefined ? { open } : {}),
    ...(onOpenChange ? { onOpenChange } : {}),
  };
}

/** A centred modal for a focused task or a confirmation. */
export function Dialog({ open, onOpenChange, trigger, title, description, footer, children }: OverlayProps) {
  return (
    <D.Root {...rootProps(open, onOpenChange)}>
      {trigger && <D.Trigger asChild>{trigger}</D.Trigger>}
      <D.Portal>
        <D.Overlay className={s.overlay} />
        <D.Content className={s.dialog} {...(description ? {} : { "aria-describedby": undefined })}>
          <header className={s.overlayHeader}>
            <div>
              <D.Title className={s.overlayTitle}>{title}</D.Title>
              {description && <D.Description className={s.overlayDescription}>{description}</D.Description>}
            </div>
            <D.Close className={s.close} aria-label="Close">
              <X />
            </D.Close>
          </header>
          {children != null && <div className={s.overlayBody}>{children}</div>}
          {footer && <footer className={s.overlayFooter}>{footer}</footer>}
        </D.Content>
      </D.Portal>
    </D.Root>
  );
}

/** A panel that slides in from the right: details without leaving the list. */
export function Drawer({
  open,
  onOpenChange,
  trigger,
  title,
  description,
  footer,
  width = 480,
  children,
}: OverlayProps & { width?: number | undefined }) {
  return (
    <D.Root {...rootProps(open, onOpenChange)}>
      {trigger && <D.Trigger asChild>{trigger}</D.Trigger>}
      <D.Portal>
        <D.Overlay className={s.overlay} />
        <D.Content
          className={s.drawer}
          style={{ width: `min(${width}px, 100vw)` }}
          {...(description ? {} : { "aria-describedby": undefined })}
        >
          <header className={s.overlayHeader}>
            <div>
              <D.Title className={s.overlayTitle}>{title}</D.Title>
              {description && <D.Description className={s.overlayDescription}>{description}</D.Description>}
            </div>
            <D.Close className={s.close} aria-label="Close">
              <X />
            </D.Close>
          </header>
          <div className={cx(s.overlayBody, s.drawerBody)}>{children}</div>
          {footer && <footer className={s.overlayFooter}>{footer}</footer>}
        </D.Content>
      </D.Portal>
    </D.Root>
  );
}

/* ---- dropdown menu ---- */

export function Menu({
  trigger,
  align = "end",
  children,
}: {
  trigger: ReactElement;
  align?: "start" | "center" | "end" | undefined;
  children: ReactNode;
}) {
  return (
    <M.Root>
      <M.Trigger asChild>{trigger}</M.Trigger>
      <M.Portal>
        <M.Content className={s.menu} align={align} sideOffset={6}>
          {children}
        </M.Content>
      </M.Portal>
    </M.Root>
  );
}

export function MenuItem({
  icon,
  shortcut,
  danger = false,
  onSelect,
  href,
  children,
}: {
  icon?: ReactNode | undefined;
  shortcut?: ReactNode | undefined;
  danger?: boolean | undefined;
  onSelect?: (() => void) | undefined;
  /** Navigates like a link; the Router picks it up. */
  href?: string | undefined;
  children: ReactNode;
}) {
  const inner = (
    <>
      {icon && <span className={s.menuIcon}>{icon}</span>}
      <span className={s.menuText}>{children}</span>
      {shortcut && <span className={s.menuShortcut}>{shortcut}</span>}
    </>
  );
  return (
    <M.Item
      className={cx(s.menuItem, danger && s.menuDanger)}
      {...(onSelect ? { onSelect } : {})}
      asChild={href !== undefined}
    >
      {href !== undefined ? (
        <Link variant="plain" href={href}>
          {inner}
        </Link>
      ) : (
        inner
      )}
    </M.Item>
  );
}

export function MenuLabel({ children }: { children: ReactNode }) {
  return <M.Label className={s.menuLabel}>{children}</M.Label>;
}

export function MenuSeparator() {
  return <M.Separator className={s.menuSeparator} />;
}

/** A set of mutually exclusive choices inside a menu, e.g. the theme. */
export function MenuRadioGroup({
  value,
  onValueChange,
  options,
}: {
  value: string;
  onValueChange: (value: string) => void;
  options: readonly { value: string; label: ReactNode; icon?: ReactNode | undefined }[];
}) {
  return (
    <M.RadioGroup value={value} onValueChange={onValueChange}>
      {options.map((o) => (
        <M.RadioItem key={o.value} value={o.value} className={s.menuItem} onSelect={(e) => e.preventDefault()}>
          {o.icon && <span className={s.menuIcon}>{o.icon}</span>}
          <span className={s.menuText}>{o.label}</span>
          <M.ItemIndicator className={s.menuCheck}>
            <Check />
          </M.ItemIndicator>
        </M.RadioItem>
      ))}
    </M.RadioGroup>
  );
}
