/** Colours that tell one line's things from another's, in both themes: token names, `--ui-…`. */
export const TONES = ["accent", "info", "success", "warning", "danger"] as const;

export function tone(index: number): (typeof TONES)[number] {
  return TONES[index % TONES.length]!;
}
