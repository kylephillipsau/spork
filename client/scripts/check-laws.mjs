#!/usr/bin/env node
/**
 * THE LAWS, ENFORCED (D124, D171).
 *
 * Every rule here is one somebody will have a good local reason to break: a
 * hex that matches the design, a font size that fixes one screen, an import of
 * a fixture made in a hurry. Written down, rules like these last about a
 * quarter. These are the ones a script can hold.
 *
 * Each check names the decision it comes from, so a failure sends you to the
 * reasoning rather than to the regex.
 *
 *   npm run laws
 */

import { readFileSync, readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, relative, extname, sep } from "node:path";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

// The laws name files with forward slashes; on Windows `relative` returns
// backslashes, and every allowlist would silently stop matching.
const posix = (p) => p.split(sep).join("/");

function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) out.push(...walk(full));
    else out.push(full);
  }
  return out;
}

const read = (dir) =>
  walk(join(ROOT, dir)).map((path) => ({
    path,
    rel: posix(relative(ROOT, path)),
    ext: extname(path),
    text: readFileSync(path, "utf8"),
  }));

const files = ["ui", "app", "src"].flatMap(read);
const css = files.filter((f) => f.ext === ".css");
const source = files.filter((f) => f.ext === ".tsx" || f.ext === ".ts");
// Everything but the one file allowed raw values.
const styled = css.filter((f) => f.rel !== "ui/tokens.css");

const failures = [];
function fail(law, decision, where, detail) {
  failures.push({ law, decision, where, detail });
}

/** Strip comments so a rule quoted in prose is not read as a rule. */
function code(text) {
  return text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/.*$/gm, "");
}

// ── 1. the values come from the tokens ────────────────────────────────
// A colour written into a component is a colour the theme cannot change and
// dark mode does not know about.
for (const file of styled) {
  const body = code(file.text);
  const hex = body.match(/#[0-9a-fA-F]{3,8}\b/);
  const fn = body.match(/\b(?:rgba?|hsla?)\(\s*[\d.]/);
  if (hex || fn) {
    fail(
      "The values come from the tokens",
      "D171",
      file.rel,
      `raw colour \`${(hex ?? fn)[0]}\` — name it in ui/tokens.css and read the token`,
    );
  }
}

// ── 2. type sizes come from the type scale ────────────────────────────
// A pixel font size is how the sidebar ended up two sizes on two screens.
for (const file of styled) {
  for (const m of code(file.text).matchAll(/font-size:\s*([^;}]+)/g)) {
    const v = m[1].trim();
    if (!/^var\(--ui-text-[\w-]+\)$/.test(v) && !/^(inherit|[\d.]+em|[\d.]+%)$/.test(v)) {
      fail("Type sizes come from the type scale", "D171", file.rel, `font-size: ${v}`);
    }
  }
}

// ── 3. spacing comes from the spacing scale ───────────────────────────
// A 1px hairline is a border's width, not a space, and is the one literal
// allowed.
for (const file of styled) {
  for (const m of code(file.text).matchAll(/\b(padding|margin|gap|row-gap|column-gap)(-[a-z-]+)?:\s*([^;}]+)/g)) {
    const literal = m[3].replace(/var\([^)]*\)/g, "").match(/(?<![\w.-])-?\d+(?:\.\d+)?px/g) ?? [];
    const bad = literal.filter((px) => !/^-?1px$/.test(px) && !/^0px$/.test(px));
    if (bad.length) {
      fail("Spacing comes from the spacing scale", "D171", file.rel, `${m[1]}${m[2] ?? ""}: ${m[3].trim()}`);
    }
  }
}

// ── 4. a token nothing reads is drift ─────────────────────────────────
// Every token was live once and outlived whatever used it. A palette that
// accumulates values nobody reads is a palette nobody can reason about, and
// a rule rewritten out of existence leaves its tokens behind.
{
  const tokens = files.find((f) => f.rel === "ui/tokens.css");
  const declared = [...new Set([...code(tokens.text).matchAll(/(--ui-[a-z0-9-]+)\s*:/g)].map((m) => m[1]))];
  const consumers = files.map((f) => f.text).join("\n");
  for (const name of declared) {
    if (!new RegExp(`var\\(${name}[,)\\s]`).test(consumers)) {
      fail("A token nothing reads is drift", "D124", "ui/tokens.css", `${name} is declared and never read`);
    }
  }
}

// ── 5. every dark value overrides a light one ─────────────────────────
// Dark mode is one block, `:root[data-theme="dark"]`, which `theme.ts` sets
// for Dark and for System when the system is dark. A name in it that the
// light theme never declares is a typo that renders nothing in either theme.
{
  const tokens = code(files.find((f) => f.rel === "ui/tokens.css").text);
  const blocks = [...tokens.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({ selector: m[1].trim(), body: m[2] }));
  const names = (b) => [...b.body.matchAll(/(--ui-[a-z0-9-]+)\s*:/g)].map((m) => m[1]);
  const dark = blocks.filter((b) => b.selector === ':root[data-theme="dark"]');
  const light = new Set(blocks.filter((b) => !b.selector.includes("data-theme")).flatMap(names));
  if (dark.length !== 1) {
    fail("Every dark value overrides a light one", "D171", "ui/tokens.css", `${dark.length} dark blocks, expected 1`);
  }
  for (const name of dark.flatMap(names)) {
    if (!light.has(name)) {
      fail("Every dark value overrides a light one", "D171", "ui/tokens.css", `${name} is dark-only`);
    }
  }
}

// ── 6. a class nothing uses is drift ──────────────────────────────────
// As a token nothing reads is. A module's users are the files that import it
// by name, or through the name ui/index.ts re-exports it under (`materials`).
{
  const reexports = new Map();
  for (const f of source.filter((x) => x.rel === "ui/index.ts")) {
    for (const m of f.text.matchAll(/export \{ default as (\w+) \} from "\.\/([\w.-]+)"/g)) reexports.set(m[2], m[1]);
  }
  for (const file of css.filter((f) => f.rel.endsWith(".module.css"))) {
    const name = file.rel.split("/").pop();
    const alias = reexports.get(name);
    const users = source.filter((s) => s.text.includes(name) || (alias && new RegExp(`\\b${alias}\\b`).test(s.text)));
    const text = users.map((u) => u.text).join("\n");
    const dynamic = /\b\w+\[`[^`]*\$\{|\b\w+\[[a-zA-Z]/.test(text);
    const classes = new Set([...code(file.text).matchAll(/\.([a-zA-Z_][\w-]*)/g)].map((m) => m[1]));
    for (const cls of classes) {
      const used = new RegExp(`\\.${cls}\\b|["'\`]${cls}["'\`]`).test(text);
      // A class reached as styles[tone] or styles[`dot_${state}`] is used when
      // its prefix is, and cannot be proven unused by reading.
      if (!used && !(dynamic && /_|^(neutral|accent|success|warning|danger|info|primary|secondary|ghost|sm|md|lg|left|right|center|cols\d)$/.test(cls))) {
        fail("A class nothing uses is drift", "D171", file.rel, `.${cls} is styled and never applied`);
      }
    }
  }
}

// ── 7. the frame declares desktop density ─────────────────────────────
// A handheld screen's touch density sizes its content and nothing around it;
// the sidebar grew on handheld views while density was a property of <body>.
for (const [rel, what] of [
  ["app/shell/AppShell.tsx", ["<aside", "<header"]],
  ["app/shell/frames.tsx", ["s.auth"]],
]) {
  const file = source.find((f) => f.rel === rel);
  if (!file) {
    fail("The frame declares desktop density", "D171", rel, "missing");
    continue;
  }
  for (const el of what) {
    const line = file.text.split("\n").find((l) => l.includes(el));
    if (!line || !line.includes('data-density="desktop"')) {
      fail("The frame declares desktop density", "D171", rel, `${el} does not carry data-density="desktop"`);
    }
  }
}

// ── 8. nothing laid over content catches the pointer ──────────────────
// A decorative ::before or ::after that covers its element must let clicks
// through, or it silently disables what is under it.
for (const file of styled) {
  const body = code(file.text);
  for (const m of body.matchAll(/\.(\w+)\s*::(before|after)\s*\{([^}]*)\}/g)) {
    const overlays = /position\s*:\s*absolute/.test(m[3]) && /inset\s*:\s*0/.test(m[3]);
    if (overlays && !/pointer-events\s*:\s*none/.test(m[3])) {
      fail(
        "Nothing laid over content catches the pointer",
        "D119",
        file.rel,
        `\`.${m[1]}::${m[2]}\` covers its element without \`pointer-events: none\``,
      );
    }
  }
}

// ── 9. the deployment carries no invented data ────────────────────────
// Fixtures are a build mode: `main.tsx` reaches them only under
// `MODE === "review"`, so Rollup drops them from production. That holds only
// while nothing on the live path imports one directly.
for (const rel of ["src/main.tsx", "app/routing/screens.tsx", "app/routing/Router.tsx", "app/shell/frames.tsx", "app/shell/AppShell.tsx"]) {
  const file = source.find((f) => f.rel === rel);
  if (!file) continue;
  for (const line of file.text.split("\n")) {
    const m = /^\s*import\s.*from\s+"([^"]*fixture[^"]*)"/.exec(line);
    if (m) fail("The production entry reaches no fixture", "D146", rel, `imports ${m[1]}`);
  }
}

// ── 10. the deployment carries no Tauri either ────────────────────────
// The mobile host is a build mode for the same reason; `app/platform/` is the
// host and is exempt.
for (const { rel, text } of source) {
  if (rel.startsWith("app/platform/")) continue;
  for (const line of text.split("\n")) {
    const m = /^\s*import\s.*from\s+"(@tauri-apps\/[^"]*|[^"]*platform\/mobile[^"]*)"/.exec(line);
    if (m) fail("The production entry reaches no native shell", "D146", rel, `imports ${m[1]}`);
  }
}

// ── 11. anchors come from Link ────────────────────────────────────────
// The router turns a same-origin anchor into a client-side navigation only
// when its path is a screen. A bare <a> elsewhere is a navigation nobody
// designed.
for (const { rel, ext, text } of source) {
  if (ext !== ".tsx" || rel === "ui/Link.tsx") continue;
  if (/<a[\s>]/.test(code(text))) {
    fail("Anchors come from Link", "D146", rel, "a bare <a> is a navigation the router does not know about");
  }
}

// ── 12. the API mints no identity and reads no clock ──────────────────
// A request body that minted its own id made a retry a second act on a ledger
// that cannot be corrected. An act's identity and moment come from the caller.
{
  const api = readFileSync(join(ROOT, "domain", "api.ts"), "utf8");
  for (const [pattern, what] of [
    [/crypto\.randomUUID/, "mints an identity"],
    [/new Date\(/, "reads the clock"],
  ]) {
    if (pattern.test(api)) {
      fail("The API mints no identity and reads no clock", "D5", "domain/api.ts", `${what} — it comes from the caller`);
    }
  }
}

// ── report ────────────────────────────────────────────────────────────
const CHECKS = 12;

if (failures.length === 0) {
  console.log(`laws ok — ${CHECKS} checks over ${css.length} stylesheets and ${source.length} source files`);
  process.exit(0);
}

console.error(`${failures.length} law violation${failures.length === 1 ? "" : "s"}:\n`);
let currentLaw = "";
for (const f of failures) {
  if (f.law !== currentLaw) {
    currentLaw = f.law;
    console.error(`  ${f.law} (${f.decision})`);
  }
  console.error(`    ✗ ${f.where}\n        ${f.detail}`);
}
console.error("\nThe reasoning for each is in docs/interface-design.md.");
process.exit(1);
