/** Join class names. The only string-building any primitive does. */
export function cx(...parts: Array<string | false | null | undefined>): string {
  let out = "";
  for (const part of parts) {
    if (!part) continue;
    out = out ? `${out} ${part}` : part;
  }
  return out;
}

/** Elevation is the one dimension of a panel a caller may choose, and it
 *  is a closed set rather than a style prop (D123). */
export type Elevation = "flush" | "raised" | "lifted";
