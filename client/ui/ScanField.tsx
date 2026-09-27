import { useEffect, useId, useRef } from "react";
import { ScanBarcode } from "lucide-react";

import { Field } from "./Forms";
import { cx } from "./cx";
import s from "./forms.module.css";

/**
 * The field a handheld screen scans into (D111, D117).
 *
 * A scanner is a keyboard: it types the decoded string and presses Enter, so
 * this is a text input that fires `onScan` on Enter. It takes the caret on
 * mount and back after each scan (`refocus`), because a scanner types wherever
 * the caret is.
 *
 * **It is never disabled.** A scan typed into a disabled input is dropped with
 * nothing on screen, and `focus()` on a disabled element does nothing. `busy`
 * gates the handler instead, and the refocus waits for it to clear.
 *
 * **Cleared on Enter**, not when the answer arrives: a wedge appends, so a box
 * still holding the last scan turns the next one into `AAAABBBB`.
 */
export function ScanField({
  label,
  value,
  onChange,
  onScan,
  hint,
  placeholder,
  busy = false,
  refocus = 0,
  claim = true,
}: {
  label: string;
  value: string;
  onChange: (next: string) => void;
  /** Enter, or the scanner's terminator. Never fired for an empty string or while busy. */
  onScan: (value: string) => void;
  hint?: string | undefined;
  placeholder?: string | undefined;
  /** A scan is in flight. Gates the handler, never the element. */
  busy?: boolean | undefined;
  /** Bump to take focus back after a scan resolves. */
  refocus?: number | undefined;
  /** Take the caret on mount. Only one field on a screen may claim it. */
  claim?: boolean | undefined;
}) {
  const id = useId();
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (refocus > 0 && !busy) input.current?.focus();
  }, [refocus, busy]);

  return (
    <Field label={label} hint={hint} htmlFor={id}>
      <div className={cx(s.control, s.fieldBox, s.scan)}>
        <span className={s.leading}>
          <ScanBarcode aria-hidden />
        </span>
        <input
          ref={input}
          id={id}
          className={s.input}
          value={value}
          placeholder={placeholder}
          // Text, never numeric: an SSCC is digits and a SKU is not.
          inputMode="text"
          autoComplete="off"
          autoCorrect="off"
          autoCapitalize="off"
          spellCheck={false}
          // eslint-disable-next-line jsx-a11y/no-autofocus
          autoFocus={claim}
          onChange={(e) => onChange(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key !== "Enter") return;
            e.preventDefault();
            const scanned = e.currentTarget.value.trim();
            if (!scanned) return;
            onChange("");
            if (busy) return;
            onScan(scanned);
          }}
        />
      </div>
    </Field>
  );
}
