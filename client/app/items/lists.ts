/**
 * A list of items to work through, made from a sheet (D179), in words with no
 * React in it so a test runner can read it.
 */

/**
 * Codes as pasted: one a line, or split by tabs, commas or semicolons, as a
 * spreadsheet's column or a typed list comes. Not by spaces, which some codes
 * have in them.
 */
export function codesFrom(pasted: string): string[] {
  return pasted
    .split(/[\r\n\t,;]+/)
    .map((c) => c.trim())
    .filter((c) => c !== "");
}
