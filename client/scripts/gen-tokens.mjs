#!/usr/bin/env node
/**
 * TOKENS ARE GENERATED, AND THE GENERATION IS CHECKED (D124).
 *
 * `design/tokens.css` is the single source. This emits `tokens.gen.ts` from
 * it and, with --check, fails when the committed file disagrees. That is
 * the same bidirectional-diff mechanism D25 uses for @projection columns:
 * a hand-kept TypeScript copy of a CSS palette is a copy that drifts, and
 * this repository's documented history is largely about numbers that drift
 * when two things state them.
 *
 * It also enforces the theme contract, which is the failure nobody notices
 * in review: every face token declared in :root must have a counterpart in
 * BOTH night blocks, and the two night blocks must agree with each other.
 * A token missing from one of them renders one theme's text on the other
 * theme's ground, in exactly one of the three viewer states.
 *
 *   node scripts/gen-tokens.mjs           write tokens.gen.ts
 *   node scripts/gen-tokens.mjs --check   fail if it would change
 */

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const SOURCE = join(here, "..", "design", "tokens.css");
const OUTPUT = join(here, "..", "design", "tokens.gen.ts");

/**
 * Comments are stripped before anything is located, because the file
 * documents its own selectors in its header and `indexOf` cannot tell a
 * selector from a mention of one. (Found by this script on its first run:
 * both night blocks resolved to `:root`.)
 */
function stripComments(css) {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

/** Read the balanced `{ ... }` body that follows `selector`. */
function blockAfter(css, selector) {
  const at = css.indexOf(selector);
  if (at === -1) return null;
  // Search from `at`, not from the end of the selector: `":root {"` carries
  // its own brace, and skipping past it latches the scan onto the next
  // block entirely. (Also found by this script on itself.)
  const open = css.indexOf("{", at);
  if (open === -1) return null;

  let depth = 0;
  for (let i = open; i < css.length; i++) {
    if (css[i] === "{") depth++;
    else if (css[i] === "}") {
      depth--;
      if (depth === 0) return css.slice(open + 1, i);
    }
  }
  return null;
}

/** `--name: value;` pairs, in source order, comments stripped. */
function declarations(block) {
  const clean = block.replace(/\/\*[\s\S]*?\*\//g, "");
  const out = new Map();
  // Values may contain semicolons inside url(...), so consume url() whole.
  const re = /(--[a-z0-9-]+)\s*:\s*((?:[^;"']|"[^"]*"|'[^']*')*)/gi;
  let m;
  while ((m = re.exec(clean)) !== null) {
    out.set(m[1], m[2].trim());
  }
  return out;
}

const css = stripComments(readFileSync(SOURCE, "utf8"));

const rootBlock = blockAfter(css, ":root {");
const mediaNight = blockAfter(css, ':root:not([data-theme="light"])');
const stampedNight = blockAfter(css, ':root[data-theme="dark"]');

const problems = [];
if (!rootBlock) problems.push("no `:root {` block found in tokens.css");
if (!mediaNight) problems.push('no `:root:not([data-theme="light"])` block found');
if (!stampedNight) problems.push('no `:root[data-theme="dark"]` block found');

if (problems.length) {
  console.error("tokens.css does not have the expected shape:");
  for (const p of problems) console.error(`  · ${p}`);
  process.exit(1);
}

const base = declarations(rootBlock);
const nightA = declarations(mediaNight);
const nightB = declarations(stampedNight);

// ── the theme contract ────────────────────────────────────────────────
const errors = [];

for (const name of nightA.keys()) {
  if (!base.has(name)) {
    errors.push(`${name} is set in the night media block but never in :root`);
  }
}
for (const name of nightB.keys()) {
  if (!base.has(name)) {
    errors.push(`${name} is set in [data-theme="dark"] but never in :root`);
  }
}
for (const name of nightA.keys()) {
  if (!nightB.has(name)) {
    errors.push(`${name} is in the night media block but missing from [data-theme="dark"] — the toggle would not reach it`);
  }
}
for (const name of nightB.keys()) {
  if (!nightA.has(name)) {
    errors.push(`${name} is in [data-theme="dark"] but missing from the night media block — a system-dark viewer would not see it`);
  }
}
for (const [name, value] of nightA) {
  if (nightB.has(name) && nightB.get(name) !== value) {
    errors.push(`${name} disagrees between the two night blocks: ${value} vs ${nightB.get(name)}`);
  }
}

if (errors.length) {
  console.error("tokens.css breaks the theme contract:\n");
  for (const e of errors) console.error(`  ✗ ${e}`);
  console.error(
    "\nEvery themed token needs all three declarations: the day value in\n" +
      ":root, and the night value in BOTH night blocks. Missing one leaves\n" +
      "a viewer state rendering one theme's ink on the other theme's ground.",
  );
  process.exit(1);
}

// ── emit ──────────────────────────────────────────────────────────────
const themed = [...nightA.keys()].sort();
const all = [...base.keys()].sort();
const solverWritten = ["--e-t", "--e-b", "--e-l", "--e-r", "--glint", "--lx", "--ly"];
const fixed = all.filter((n) => !themed.includes(n) && !solverWritten.includes(n));

const q = (names) => names.map((n) => `  "${n}",`).join("\n");

const generated = `// GENERATED FROM design/tokens.css — DO NOT EDIT.
// Run \`npm run tokens\` to regenerate; \`npm run tokens:check\` fails CI when
// this file and the stylesheet disagree (D124).

/** Every custom property the design system declares. */
export const TOKENS = [
${q(all)}
] as const;

export type Token = (typeof TOKENS)[number];

/** Tokens that change between the day face and the night face. The chassis
 *  is a fixed material and is not among them (D118). */
export const THEMED_TOKENS = [
${q(themed)}
] as const;

export type ThemedToken = (typeof THEMED_TOKENS)[number];

/** Tokens with one value in both faces. */
export const FIXED_TOKENS = [
${q(fixed)}
] as const;

/** Written per panel, per frame, by the light solver (D121). Never authored
 *  by a component; the defaults in tokens.css describe the lamp at rest. */
export const SOLVER_TOKENS = [
${q(solverWritten)}
] as const;

export type SolverToken = (typeof SOLVER_TOKENS)[number];

/** \`var(--x)\`, but only for a token that exists. */
export function token(name: Token): string {
  return \`var(\${name})\`;
}
`;

if (process.argv.includes("--check")) {
  let committed;
  try {
    committed = readFileSync(OUTPUT, "utf8");
  } catch {
    console.error("design/tokens.gen.ts is missing. Run `npm run tokens`.");
    process.exit(1);
  }
  if (committed !== generated) {
    console.error(
      "design/tokens.gen.ts is out of date with design/tokens.css.\n" +
        "Run `npm run tokens` and commit the result.",
    );
    process.exit(1);
  }
  console.log(`tokens ok — ${all.length} declared, ${themed.length} themed, contract holds`);
} else {
  writeFileSync(OUTPUT, generated);
  console.log(
    `wrote design/tokens.gen.ts — ${all.length} tokens, ${themed.length} themed, ${fixed.length} fixed`,
  );
}
