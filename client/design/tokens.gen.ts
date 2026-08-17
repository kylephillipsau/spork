// GENERATED FROM design/tokens.css — DO NOT EDIT.
// Run `npm run tokens` to regenerate; `npm run tokens:check` fails CI when
// this file and the stylesheet disagree (D124).

/** Every custom property the design system declares. */
export const TOKENS = [
  "--ano-400",
  "--ano-500",
  "--ano-700",
  "--ano-900",
  "--ano-950",
  "--band-bg",
  "--e-b",
  "--e-l",
  "--e-r",
  "--e-t",
  "--edge-dim",
  "--edge-face",
  "--face",
  "--face-amber",
  "--face-amber-soft",
  "--face-faint",
  "--face-good",
  "--face-good-soft",
  "--face-ink",
  "--face-lift",
  "--face-nitrile",
  "--face-rule",
  "--face-rule-soft",
  "--face-soft",
  "--face-steel",
  "--face-steel-soft",
  "--face-well",
  "--font-mono",
  "--font-ui",
  "--glint",
  "--hi",
  "--key-live",
  "--key-live-ink",
  "--lamp-amber",
  "--lamp-amber-core",
  "--lamp-good",
  "--lamp-good-core",
  "--lamp-nitrile",
  "--lamp-nitrile-core",
  "--lamp-off",
  "--legend",
  "--legend-dim",
  "--lo",
  "--lx",
  "--ly",
  "--nyl-700",
  "--nyl-dyed",
  "--r-control",
  "--r-face",
  "--r-panel",
  "--r-pill",
  "--r-well",
  "--tex-blast",
  "--tex-fine",
  "--tex-pits",
  "--track-legend",
  "--track-mark",
] as const;

export type Token = (typeof TOKENS)[number];

/** Tokens that change between the day face and the night face. The chassis
 *  is a fixed material and is not among them (D118). */
export const THEMED_TOKENS = [
  "--band-bg",
  "--face",
  "--face-amber",
  "--face-amber-soft",
  "--face-faint",
  "--face-good",
  "--face-good-soft",
  "--face-ink",
  "--face-lift",
  "--face-nitrile",
  "--face-rule",
  "--face-rule-soft",
  "--face-soft",
  "--face-steel",
  "--face-steel-soft",
  "--face-well",
] as const;

export type ThemedToken = (typeof THEMED_TOKENS)[number];

/** Tokens with one value in both faces. */
export const FIXED_TOKENS = [
  "--ano-400",
  "--ano-500",
  "--ano-700",
  "--ano-900",
  "--ano-950",
  "--edge-dim",
  "--edge-face",
  "--font-mono",
  "--font-ui",
  "--hi",
  "--key-live",
  "--key-live-ink",
  "--lamp-amber",
  "--lamp-amber-core",
  "--lamp-good",
  "--lamp-good-core",
  "--lamp-nitrile",
  "--lamp-nitrile-core",
  "--lamp-off",
  "--legend",
  "--legend-dim",
  "--lo",
  "--nyl-700",
  "--nyl-dyed",
  "--r-control",
  "--r-face",
  "--r-panel",
  "--r-pill",
  "--r-well",
  "--tex-blast",
  "--tex-fine",
  "--tex-pits",
  "--track-legend",
  "--track-mark",
] as const;

/** Written per panel, per frame, by the light solver (D121). Never authored
 *  by a component; the defaults in tokens.css describe the lamp at rest. */
export const SOLVER_TOKENS = [
  "--e-t",
  "--e-b",
  "--e-l",
  "--e-r",
  "--glint",
  "--lx",
  "--ly",
] as const;

export type SolverToken = (typeof SOLVER_TOKENS)[number];

/** `var(--x)`, but only for a token that exists. */
export function token(name: Token): string {
  return `var(${name})`;
}
