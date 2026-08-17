import { useEffect, useRef } from "react";
import { cx } from "../cx";
import styles from "../materials/face.module.css";

/**
 * The locator: one input that resolves any scannable identifier (D111).
 *
 * **A wedge, not a camera.** A warehouse scanner is a keyboard: it types the
 * decoded string and presses Enter. So this is an ordinary text input with an
 * `onSubmit`, and it works with a handheld's laser engine, a bench wedge and a
 * person typing a code they read off a box — three things that would otherwise
 * be three implementations.
 *
 * **Whether it claims focus is now a choice, and the default is yes** (D117).
 * The chrome's locator passes `focus="none"`: two inputs on one page both
 * calling `autoFocus` would fight, and the last mounted would win — which is a
 * caret landing somewhere the operator did not put it. A screen's locator keeps
 * the claiming behaviour, because that is the one this was built for.
 *
 Claiming exists because a scanner fires its keystrokes wherever the caret
 * happens to be, so an input that has to be clicked first is an input that
 * loses the first scan of every session — and the operator has gloves on.
 * `refocus` takes the caret back after each scan, because the common case is a
 * stack of boxes rather than one.
 *
 * # It is never disabled, and that is the whole of the stack-of-boxes case
 *
 * The obvious thing is `disabled` while the last scan resolves. It breaks
 * exactly the case this exists for: a trigger pull during the round trip types
 * into a disabled input and **is dropped entirely**, with no error and nothing
 * on screen — the operator's box is scanned and the screen disagrees. And a
 * `focus()` on a disabled element is a no-op that does not retry, so the
 * refocus after the first scan can silently fail depending on whether the
 * state updates land in one render.
 *
 * So the input stays live and `busy` gates the *handler* instead: a scan
 * arriving mid-flight is ignored deliberately, at a point where something could
 * queue it later, rather than swallowed by the DOM. `busy` also gates the
 * refocus, because taking focus back before the answer arrives puts the caret
 * somewhere the next keystroke has nothing to do with.
 *
 * **A scan during the round trip is still dropped**, and that is a limitation
 * rather than a design: it is dropped in one place, in the open, instead of by
 * a disabled element. Resolution is one indexed read and the window is
 * milliseconds, so a queue would be machinery for a case nobody has hit — if
 * the round trip ever gets slow enough to matter, this handler is where the
 * queue goes.
 *
 * `value` stays controlled so the screen can show what was read while it
 * resolves, which is the difference between a slow answer and no answer.
 */
export function ScanInput({
  label,
  value,
  onChange,
  onScan,
  hint,
  busy = false,
  refocus = 0,
  focus = "claim",
}: {
  label: string;
  value: string;
  onChange: (next: string) => void;
  /** Enter, or the scanner's terminator. Never fired for an empty string, and
   *  never while the last one is still in flight. */
  onScan: (value: string) => void;
  /** What the last scan turned out to be, or what to do. */
  hint?: string;
  /** A scan is in flight. Gates the handler, never the element. */
  busy?: boolean;
  /** `claim` takes the caret on mount and back after each scan; `none` never
   *  touches it, which is what the chrome's locator must do (D117). */
  focus?: "claim" | "none";
  /** Bump to take focus back — after a scan resolves, or after a session ends.
   *  A number rather than a ref so the parent says *when* without holding a
   *  handle on the element, which is the escape hatch D123 closes. */
  refocus?: number;
}) {
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // `busy` is in the condition and in the deps: if the bump lands in the same
    // render as the answer this fires once, and if it lands a render early it
    // fires again when `busy` clears. Either ordering ends with the caret here,
    // which is the property rather than the implementation.
    if (refocus > 0 && !busy) input.current?.focus();
  }, [refocus, busy]);

  return (
    <label data-layer="instrument" className={cx(styles.field, styles.scan)}>
      <span className={styles.fieldLabel}>{label}</span>
      <input
        ref={input}
        className={cx(styles.fieldInput, styles.scanInput)}
        value={value}
        // Text, and never `numeric`: an SSCC is digits and a SKU is not, and a
        // numeric keypad on the one screen where somebody may have to type
        // `STY-7720-08` is the wrong help.
        inputMode="text"
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        spellCheck={false}
        // The scanner is the primary input on this surface, so the caret starts
        // here rather than waiting to be asked.
        // eslint-disable-next-line jsx-a11y/no-autofocus
        autoFocus={focus === "claim"}
        onChange={(e) => onChange(e.currentTarget.value)}
        onKeyDown={(e) => {
          if (e.key !== "Enter") return;
          e.preventDefault();
          const scanned = e.currentTarget.value.trim();
          if (!scanned) return;
          // **Cleared on Enter, not when the answer arrives.** A wedge appends,
          // so a box still holding the last scan turns the next one into
          // `AAAABBBB`. Emptying it here is what makes a stack of boxes work
          // and does not depend on how long the round trip takes.
          onChange("");
          if (busy) return;
          onScan(scanned);
        }}
      />
      {hint && <span className={styles.scanHint}>{hint}</span>}
    </label>
  );
}
