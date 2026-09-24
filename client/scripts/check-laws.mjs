#!/usr/bin/env node
/**
 * THE LAWS, ENFORCED (D124).
 *
 * Every law in docs/interface-design.md is one somebody will have a good
 * local reason to break — amber is convenient for a warning, a wash makes a
 * panel look richer, a margin fixes one screen. Written down, they last
 * about a quarter. These are the ones a script can hold.
 *
 * Each check names the law it enforces and the decision it comes from, so a
 * failure sends you to the reasoning rather than to the regex.
 */

import { readFileSync, readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, relative, extname, sep } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const ROOT = join(here, "..");
const DESIGN = join(ROOT, "design");

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

function exists(dir) {
  try {
    return statSync(dir).isDirectory();
  } catch {
    return false;
  }
}

const designFiles = walk(DESIGN).map((path) => ({
  path,
  rel: posix(relative(ROOT, path)),
  ext: extname(path),
  text: readFileSync(path, "utf8"),
}));

const appFiles = exists(join(ROOT, "app"))
  ? walk(join(ROOT, "app")).map((path) => ({
      path,
      rel: posix(relative(ROOT, path)),
      ext: extname(path),
      text: readFileSync(path, "utf8"),
    }))
  : [];

// `domain/` is not walked with the app: it is the headless half, and only one
// law is about it.
const apiSource = (() => {
  try {
    return readFileSync(join(ROOT, "domain", "api.ts"), "utf8");
  } catch {
    return null;
  }
})();

const css = designFiles.filter((f) => f.ext === ".css");
const tsx = designFiles.filter((f) => f.ext === ".tsx");
const primitives = tsx.filter((f) => f.rel.includes("primitives"));

const failures = [];

function fail(law, decision, where, detail) {
  failures.push({ law, decision, where, detail });
}

/** Strip CSS comments so a rule quoted in prose is not read as a rule. */
function code(text) {
  return text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/.*$/gm, "");
}

// ── 1 ─────────────────────────────────────────────────────────────────
// Every element declares a layer. A file that renders a material without
// one is an element styled outside the stack.
for (const file of tsx) {
  const body = code(file.text);
  if (body.includes("data-material") && !body.includes("data-layer")) {
    fail(
      "Every element declares a layer",
      "D124",
      file.rel,
      "renders data-material with no data-layer",
    );
  }
}
for (const file of primitives) {
  if (!code(file.text).includes("data-layer")) {
    fail("Every element declares a layer", "D124", file.rel, "primitive sets no data-layer");
  }
}

// ── 2 ─────────────────────────────────────────────────────────────────
// Primitives accept no className and no style. There is no subset of a
// material that is still that material, so a call site may not reach past
// the closed enums a primitive exposes.
for (const file of primitives) {
  const body = code(file.text);
  for (const escape of ["className?:", "style?:", "className:", "style:"]) {
    if (body.includes(escape)) {
      fail(
        "Materials own their CSS; primitives accept no class or style",
        "D123",
        file.rel,
        `exposes \`${escape}\` in its props`,
      );
    }
  }
}

// ── 3 ─────────────────────────────────────────────────────────────────
// Spacing belongs to the parent. Components own padding; nothing owns
// margin. A component that can push its neighbours around is not reusable.
//
// Scoped to *.module.css, because the law is about components: `body {
// margin: 0 }` in the global reset is not a component pushing a neighbour
// around, it is the reason components can assume it does not have to.
for (const file of css.filter((f) => f.rel.endsWith(".module.css"))) {
  const body = code(file.text);
  const re = /(^|[;{\s])(margin(-top|-right|-bottom|-left|-inline|-block)?[a-z-]*)\s*:/g;
  let m;
  while ((m = re.exec(body)) !== null) {
    const line = body.slice(0, m.index).split("\n").length;
    fail(
      "Spacing belongs to the parent",
      "D124",
      `${file.rel}:${line}`,
      `\`${m[2]}\` — use the parent's gap, or a growing sibling`,
    );
  }
}

// ── 4 ─────────────────────────────────────────────────────────────────
// No raw colour outside tokens.css. A hex in a component is a value that
// exists in one theme and was never considered in the other.
for (const file of css) {
  if (file.rel.endsWith("tokens.css")) continue;
  const body = code(file.text);
  const re = /#[0-9a-fA-F]{3,8}\b/g;
  let m;
  while ((m = re.exec(body)) !== null) {
    const line = body.slice(0, m.index).split("\n").length;
    fail("No raw colour outside tokens.css", "D124", `${file.rel}:${line}`, `\`${m[0]}\``);
  }
}

// ── 5 ─────────────────────────────────────────────────────────────────
// Metal is opaque. Translucency buys depth by contradicting the material,
// and it is also the version that fails on a handheld.
for (const file of css) {
  const body = code(file.text);
  if (/backdrop-filter\s*:/.test(body)) {
    fail("Metal is opaque", "D118", file.rel, "`backdrop-filter` — depth comes from the ground");
  }
}

// ── 6 ─────────────────────────────────────────────────────────────────
// Amber marks findings and nothing else. The design system may reach it in
// exactly two files — the lamp and the finding — and everywhere else it
// belongs under app/integrity/.
const AMBER_HOMES = ["materials/lamp.module.css", "materials/face.module.css"];
for (const file of [...designFiles, ...appFiles]) {
  if (file.ext !== ".css" && file.ext !== ".tsx" && file.ext !== ".ts") continue;
  // The token source and its generated mirror necessarily name every token.
  if (file.rel.endsWith("tokens.css") || file.rel.endsWith("tokens.gen.ts")) continue;
  if (AMBER_HOMES.some((home) => file.rel.endsWith(home))) continue;
  if (file.rel.includes(join("app", "integrity"))) continue;

  const body = code(file.text);
  if (/--(face-)?(lamp-)?amber/.test(body)) {
    fail(
      "Amber is emitted, and only by findings",
      "D115",
      file.rel,
      "references an amber token outside the finding path",
    );
  }
}

// ── 7 ─────────────────────────────────────────────────────────────────
// Nothing translucent is laid over text. The cover lens is the sole
// exception and it lives in face.module.css, masked and cornered.
for (const file of css) {
  if (file.rel.endsWith("face.module.css")) continue;
  const body = code(file.text);
  const re = /\.(\w+)\s*::(before|after)\s*\{([^}]*)\}/g;
  let m;
  while ((m = re.exec(body)) !== null) {
    const block = m[3];
    const overlays = /position\s*:\s*absolute/.test(block) && /inset\s*:\s*0/.test(block);
    const guarded = /pointer-events\s*:\s*none/.test(block);
    if (overlays && !guarded) {
      const line = body.slice(0, m.index).split("\n").length;
      fail(
        "Nothing translucent is laid over text",
        "D119",
        `${file.rel}:${line}`,
        `\`.${m[1]}::${m[2]}\` covers its element without \`pointer-events: none\``,
      );
    }
  }
}

// ── 8 ─────────────────────────────────────────────────────────────────
// A token defined and never referenced is drift. Every one of them was live
// once and outlived whatever used it — the key risers went with the
// skeuomorphism, the chamfer tokens with the drawn edges — and a palette that
// accumulates values nobody reads is a palette nobody can reason about.
//
// This is also the check that would have caught the edit that silently did
// not apply: a rule rewritten out of existence leaves its tokens behind.
{
  const tokenSource = designFiles.find((f) => f.rel.endsWith("tokens.css"));
  if (tokenSource) {
    const declared = [
      ...new Set([...tokenSource.text.matchAll(/(--[a-z0-9-]+)\s*:/g)].map((m) => m[1])),
    ];
    const consumers = [...designFiles, ...appFiles]
      .filter((f) => [".css", ".ts", ".tsx"].includes(f.ext))
      .filter((f) => !f.rel.endsWith("tokens.css") && !f.rel.endsWith("tokens.gen.ts"))
      .map((f) => f.text)
      .join("\n");

    // Written by the light solver rather than read from a stylesheet.
    const solverWritten = /^--(e-[tblr]|glint|lx|ly)$/;

    for (const name of declared) {
      if (solverWritten.test(name)) continue;
      if (new RegExp(`var\\(${name}[,)\\s]`).test(consumers)) continue;
      fail(
        "A token that nothing reads is drift",
        "D124",
        "design/tokens.css",
        `${name} is declared and never referenced`,
      );
    }
  }
}

// ── 9. the deployment carries no invented data ────────────────────────
//
// **The check this whole exercise exists for.** The client shipped with its
// front door pointing at a fixture and 31 of 39 routes drawing hardcoded data,
// and five gates stayed green throughout, because every one of them measured
// something else.
//
// Fixtures are now a build mode: `main.tsx` reaches them only under
// `import.meta.env.MODE === "review"`, so Rollup drops the module from
// production. That holds only while nothing on the live path imports one
// directly — and an import added in a hurry is exactly how it would stop
// holding. So it is a rule rather than an intention.
{
  const entry = ["src/main.tsx", "app/routing/screens.tsx", "app/routing/Router.tsx"];
  for (const rel of entry) {
    // `exists()` above answers for directories — a file always came back false
    // through it, so this check silently passed over every entry until it was
    // tested by breaking it. Read and catch instead.
    let source;
    try {
      source = readFileSync(join(ROOT, rel), "utf8");
    } catch {
      continue;
    }
    for (const line of source.split("\n")) {
      const m = /^\s*import\s.*from\s+"([^"]*fixture[^"]*)"/.exec(line);
      if (m) {
        fail(
          "The production entry reaches no fixture",
          "D146",
          rel,
          `imports ${m[1]} — fixtures belong to the review build`,
        );
      }
    }
  }
}

// ── 9b. the deployment carries no Tauri either ────────────────────────
//
// The mobile host is a build mode for the same reason fixtures are: the
// alternative is a runtime check and a lazy chunk that ships to every browser
// and is relied upon not to load. This repository already knows how it feels
// about that.
//
// So nothing on the live path may import `@tauri-apps/*` or the host module
// directly — `main.tsx` reaches it only under `MODE === "mobile"`, and that is
// what lets Rollup drop the branch, `app/platform/mobile.ts` and the plugin
// with it. `app/platform/` itself is exempt: it *is* the host.
{
  for (const { rel, text } of appFiles) {
    if (rel.startsWith("app/platform/")) continue;
    for (const line of text.split("\n")) {
      const m = /^\s*import\s.*from\s+"(@tauri-apps\/[^"]*|[^"]*platform\/mobile[^"]*)"/.exec(line);
      if (m) {
        fail(
          "The production entry reaches no native shell",
          "D146",
          rel,
          `imports ${m[1]} — the mobile host belongs to the mobile build`,
        );
      }
    }
  }
}

// ── 10. one anchor, and the router knows about it ─────────────────────
//
// The click interceptor turns a same-origin anchor into a client-side
// navigation only when its path resolves to a screen; everything else falls
// through to the browser, which is what keeps the eleven maud pages still
// standing at `/app/*` reachable. That contract holds only if every anchor in
// the client goes through `Link` — one written by hand somewhere else is one
// navigation nobody designed.
{
  for (const { rel, ext, text } of [...designFiles, ...appFiles]) {
    if (ext !== ".tsx" || rel.endsWith("primitives/Link.tsx")) continue;
    const source = text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/.*$/gm, "");
    if (/<a[\s>]/.test(source)) {
      fail(
        "Anchors come from Link",
        "D146",
        rel,
        "a bare <a> is a navigation the router does not know about",
      );
    }
  }
}

// ── 11. the API mints no identity and reads no clock ──────────────────
//
// **The bug this stops is one this file already had.** `client_event_id:
// uuid()` sat inside eleven request bodies, so every *invocation* minted a new
// one — and the only thing that retries in this client is an operator pressing
// again after a failure. The second press arrived wearing a different name and
// the server recorded a second act: two observations for one measurement, two
// picks, an adjustment applied twice, on a ledger that cannot be corrected.
//
// `occurred_at: now()` was the same bug wearing a different hat, and the worse
// half: `reject_allocation_mismatch` refuses a replay whose body disagrees, so
// a stable id carrying a moving timestamp fails loudly having already
// succeeded.
//
// Both now come from the `Act` the caller supplies. A single `randomUUID` or
// `new Date` back in this file is the whole thing again, so it is a rule rather
// than a convention. `acts.ts` is where they legitimately live.
if (apiSource) {
  for (const [pattern, what] of [
    [/crypto\.randomUUID/, "mints an identity"],
    [/new Date\(/, "reads the clock"],
  ]) {
    if (pattern.test(apiSource)) {
      fail(
        "The API mints no identity and reads no clock",
        "D5",
        "domain/api.ts",
        `${what} — an act's identity and moment come from the caller, or a retry is a second act`,
      );
    }
  }
}

// ── report ────────────────────────────────────────────────────────────
const CHECKS = 11;

if (failures.length === 0) {
console.log(
    `laws ok — ${CHECKS} checks over ${designFiles.length} design files` +
      (appFiles.length ? ` and ${appFiles.length} app files` : ""),
  );
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
