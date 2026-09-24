import { useId, type ComponentPropsWithRef, type ReactNode } from "react";
import { Checkbox as C, Select as S, Tabs as T } from "radix-ui";
import { Check, ChevronDown, Search } from "lucide-react";

import { cx } from "./cx";
import s from "./forms.module.css";

/** A label, the control, and an optional hint or error beneath it. */
export function Field({
  label,
  hint,
  error,
  htmlFor,
  children,
}: {
  label?: ReactNode | undefined;
  hint?: ReactNode | undefined;
  error?: ReactNode | undefined;
  htmlFor?: string | undefined;
  children: ReactNode;
}) {
  return (
    <div className={s.field}>
      {label && (
        <label className={s.label} htmlFor={htmlFor}>
          {label}
        </label>
      )}
      {children}
      {error ? (
        <p className={s.error} role="alert">
          {error}
        </p>
      ) : (
        hint && <p className={s.hint}>{hint}</p>
      )}
    </div>
  );
}

export interface TextFieldProps extends Omit<ComponentPropsWithRef<"input">, "prefix"> {
  label?: ReactNode | undefined;
  hint?: ReactNode | undefined;
  error?: ReactNode | undefined;
  /** An icon inside the field, before the text. */
  leading?: ReactNode | undefined;
  /** A unit or short text inside the field, after the value. */
  trailing?: ReactNode | undefined;
}

export function TextField({ label, hint, error, leading, trailing, id, className, ...input }: TextFieldProps) {
  const auto = useId();
  const fieldId = id ?? auto;
  return (
    <Field label={label} hint={hint} error={error} htmlFor={fieldId}>
      <div className={cx(s.control, error != null && s.invalid, className)}>
        {leading && <span className={s.leading}>{leading}</span>}
        <input id={fieldId} className={s.input} aria-invalid={error != null || undefined} {...input} />
        {trailing && <span className={s.trailing}>{trailing}</span>}
      </div>
    </Field>
  );
}

/** A text field with a search icon; the usual list filter. */
export function SearchField(props: Omit<TextFieldProps, "leading" | "type">) {
  return <TextField type="search" leading={<Search />} {...props} />;
}

export interface Option {
  value: string;
  label: ReactNode;
}

export function Select({
  label,
  hint,
  value,
  onValueChange,
  options,
  placeholder = "Select…",
  size = "md",
  disabled,
  "aria-label": ariaLabel,
}: {
  label?: ReactNode | undefined;
  hint?: ReactNode | undefined;
  value: string | undefined;
  onValueChange: (value: string) => void;
  options: readonly Option[];
  placeholder?: string | undefined;
  size?: "sm" | "md" | undefined;
  disabled?: boolean | undefined;
  "aria-label"?: string | undefined;
}) {
  const id = useId();
  return (
    <Field label={label} hint={hint} htmlFor={id}>
      <S.Root {...(value !== undefined ? { value } : {})} onValueChange={onValueChange} disabled={disabled ?? false}>
        <S.Trigger id={id} className={cx(s.control, s.selectTrigger, size === "sm" && s.sm)} aria-label={ariaLabel}>
          <S.Value placeholder={placeholder} />
          <S.Icon className={s.selectIcon}>
            <ChevronDown />
          </S.Icon>
        </S.Trigger>
        <S.Portal>
          <S.Content className={s.selectContent} position="popper" sideOffset={4}>
            <S.Viewport>
              {options.map((o) => (
                <S.Item key={o.value} value={o.value} className={s.selectItem}>
                  <S.ItemText>{o.label}</S.ItemText>
                  <S.ItemIndicator className={s.selectCheck}>
                    <Check />
                  </S.ItemIndicator>
                </S.Item>
              ))}
            </S.Viewport>
          </S.Content>
        </S.Portal>
      </S.Root>
    </Field>
  );
}

export function Checkbox({
  label,
  checked,
  onCheckedChange,
  disabled,
}: {
  label: ReactNode;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean | undefined;
}) {
  const id = useId();
  return (
    <div className={s.checkRow}>
      <C.Root
        id={id}
        className={s.checkbox}
        checked={checked}
        onCheckedChange={(c) => onCheckedChange(c === true)}
        disabled={disabled ?? false}
      >
        <C.Indicator className={s.checkIndicator}>
          <Check />
        </C.Indicator>
      </C.Root>
      <label htmlFor={id} className={s.checkLabel}>
        {label}
      </label>
    </div>
  );
}

export interface TabItem {
  value: string;
  label: ReactNode;
  count?: number | undefined;
}

/** A row of tabs that switches a view. The caller renders the view. */
export function Tabs({
  value,
  onValueChange,
  items,
  "aria-label": ariaLabel,
}: {
  value: string;
  onValueChange: (value: string) => void;
  items: readonly TabItem[];
  "aria-label"?: string | undefined;
}) {
  return (
    <T.Root value={value} onValueChange={onValueChange} activationMode="manual">
      <T.List className={s.tabs} aria-label={ariaLabel}>
        {items.map((t) => (
          <T.Trigger key={t.value} value={t.value} className={s.tab}>
            {t.label}
            {t.count != null && <span className={s.tabCount}>{t.count}</span>}
          </T.Trigger>
        ))}
      </T.List>
    </T.Root>
  );
}
