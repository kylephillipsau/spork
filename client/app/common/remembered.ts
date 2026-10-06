/**
 * How this browser last left a screen: a view, a layer, a panel collapsed.
 *
 * A convenience for one viewer, never state anyone else reads, so storage that
 * can't be read or written (a private window, blocked site data) is nothing
 * remembered rather than a failure.
 */

/** What was kept under `key`, when it is one of `choices`. */
export function recall<T extends string>(key: string, choices: readonly T[]): T | null {
  try {
    const was = localStorage.getItem(key);
    return choices.find((c) => c === was) ?? null;
  } catch {
    return null;
  }
}

/** Keep `value` under `key` for next time. */
export function keep(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // remembered for this visit only
  }
}
