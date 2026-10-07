#!/usr/bin/env node
/**
 * THE SCREENS, RENDERED AND MEASURED (D171).
 *
 * The other gates read source. This one runs the review build in a browser,
 * visits every fixture, and measures what came out: the things a grep cannot
 * see and a screenshot shows at once.
 *
 * Every fixture is visited, and the list comes from `app/routing/fixtures.tsx`
 * rather than from a copy here. The old gate kept its own list and went on
 * reporting the same count while four new fixtures went unchecked.
 *
 * Per render:
 *
 *  - nothing threw, nothing logged an error, and nothing tried the network;
 *  - the page does not scroll sideways (at the bench, at 390px for every
 *    desk-shaped screen, and on a 2560px monitor for every one in the app
 *    frame), and the widest element is named when it does;
 *  - density: a handheld screen's content is at touch density and every
 *    control on it is a touch target, while the frame (sidebar, header) stays
 *    at desktop density on every screen;
 *  - at most one primary action is visible at a time, the screen's or the
 *    dock's;
 *  - one scan field at most, and never the header's and the screen's together
 *    (D117);
 *  - the sidebar marks exactly one place as current, and no count shows zero
 *    (D112);
 *  - a handheld dock that holds something is inside the viewport (D134).
 *
 *   npm run build:review && npm run render
 */

import { createServer } from "node:http";
import { readFile, mkdir } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join, extname, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
// The review build, not the shipped one: fixtures are excluded from
// production, and `check-laws.mjs` checks that the production entry imports
// none of them.
const DIST = join(ROOT, "dist-review");
const SHOTS = join(ROOT, ".render");

if (!existsSync(join(DIST, "index.html"))) {
  console.error("No dist-review/ to render. Run `npm run build:review` first.");
  process.exit(1);
}

const TYPES = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".svg": "image/svg+xml",
  ".woff2": "font/woff2",
};

/**
 * A stand-in photograph, so fixtures draw their pictures rather than only the
 * missing-file state (D132), and a borrowed picture's label is seen (D141).
 */
const STAND_IN = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAAuUlEQVR42pXNAQaDABgG0O9gMzMz" +
    "MzPJTJIkSZIkSZIkSZIkSZJkJsnMzA64M/zvAg+eyQe2GLly4qtZqBexWaV2k7td6Q91OLXxs0" +
    "+XMX89ys9c/9YWrsH5lhA6UuwpaaDlkVEmVp05beH1VTA20aNL5iFbp+L9rL5LA0e/kxLY2o2U" +
    "wFJZUgJTYUgJDPlKSqBLF1ICTTyTEqjCiZRA4Y+kBDJ3ICWQ7ntSAvG2IyUQ2C0pAc9sSMkflY" +
    "xtEAa/yVoAAAAASUVORK5CYII=",
  "base64",
);

const server = createServer(async (req, res) => {
  const url = new URL(req.url ?? "/", "http://localhost");
  if (url.pathname.startsWith("/api/images/")) {
    res.writeHead(200, { "content-type": "image/png" });
    res.end(STAND_IN);
    return;
  }
  let file = join(DIST, url.pathname);
  if (url.pathname.endsWith("/")) file = join(file, "index.html");
  try {
    const body = await readFile(file);
    res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" });
    res.end(body);
  } catch {
    // Anything else is a client route.
    const body = await readFile(join(DIST, "index.html"));
    res.writeHead(200, { "content-type": "text/html" });
    res.end(body);
  }
});

await new Promise((done) => server.listen(0, "127.0.0.1", done));
const base = `http://127.0.0.1:${server.address().port}/`;
await mkdir(SHOTS, { recursive: true });

/**
 * The bench monitor, the handheld, a desk screen on a phone, and a big office
 * monitor. The content fills the window, so a layout that only ever met the
 * bench's width is seen at a wide one too.
 */
const BENCH = { width: 1440, height: 1000 };
const HANDHELD = { width: 430, height: 932 };
const POCKET = { width: 390, height: 844 };
const WIDE = { width: 2560, height: 1440 };

/** Every fixture, read from the table that declares it. */
const src = await readFile(join(ROOT, "app/routing/fixtures.tsx"), "utf8");
const ROUTES = [
  ...src.matchAll(/\b(app|auth)\(\s*"[^"]*",\s*"\/(fixtures[^"]*)",\s*"[^"]*"(?:,\s*"(\w+)")?/g),
].map(([, frame, path, surface]) => ({ path, frame, floor: surface === "floor" }));
{
  const declared = [...src.matchAll(/"\/(fixtures[^"]*)"/g)].length;
  if (declared !== ROUTES.length || ROUTES.length === 0) {
    console.error(
      `fixtures.tsx declares ${declared} fixture paths and this gate parsed ${ROUTES.length}. ` +
        "The table's shape changed; update the pattern above.",
    );
    process.exit(1);
  }
}

// RENDER_ONLY=item/netsuite,items/list renders just the fixtures whose path
// holds one of those; unset, every one.
const ONLY = (process.env.RENDER_ONLY ?? "").split(",").map((s) => s.trim()).filter(Boolean);
const VISITS = ROUTES.filter((route) => ONLY.length === 0 || ONLY.some((o) => route.path.includes(o))).flatMap((route) =>
  ["light", "dark"]
    .map((scheme) => ({ route, scheme, viewport: route.floor ? HANDHELD : BENCH, as: scheme }))
    .concat(route.floor ? [] : [{ route, scheme: "light", viewport: POCKET, as: "pocket" }])
    .concat(route.floor || route.frame !== "app" ? [] : [{ route, scheme: "light", viewport: WIDE, as: "wide" }]),
);

const failures = [];
const note = (where, what) => failures.push(`${where}: ${what}`);

const browser = await chromium.launch();

for (const { route, scheme, viewport, as } of VISITS) {
  const where = `${route.path}/${as}`;
  const context = await browser.newContext({ viewport, colorScheme: scheme });
  // The theme preference is per browser; ask for the one being measured.
  await context.addInitScript((s) => {
    try {
      localStorage.setItem("spork.theme", s);
    } catch {}
  }, scheme);
  const page = await context.newPage();

  const problems = [];
  page.on("console", (m) => m.type() === "error" && problems.push(m.text()));
  page.on("pageerror", (e) => problems.push(`uncaught: ${e.message}`));
  page.on("requestfailed", (r) => problems.push(`request failed: ${r.url()}`));
  page.on("request", (r) => {
    const p = new URL(r.url()).pathname;
    if (p.startsWith("/api/") && !p.startsWith("/api/images/")) problems.push(`network: ${r.method()} ${p}`);
  });

  await page.goto(`${base}${route.path}`, { waitUntil: "networkidle" });
  await page.waitForTimeout(250);

  const seen = await page.evaluate(() => {
    const visible = (el) => el.checkVisibility() && el.getBoundingClientRect().width > 0;
    const main = document.querySelector("main") ?? document.body;
    const header = document.querySelector("header");
    const dock = document.querySelector('[class*="dock"]:not(:empty)');
    const px = (el, name) => (el ? getComputedStyle(el).getPropertyValue(name).trim() : "");

    const overflow = Math.max(0, document.documentElement.scrollWidth - document.documentElement.clientWidth);
    let widest = "";
    if (overflow > 2) {
      const edge = document.documentElement.scrollWidth;
      for (const el of document.querySelectorAll("body *")) {
        const r = el.getBoundingClientRect();
        if (Math.abs(r.right - edge) < 1.5 && r.width > 0) {
          const cls = (el.className ?? "").toString().split(" ")[0] ?? "";
          const text = (el.textContent ?? "").trim().slice(0, 30);
          widest = `${el.tagName.toLowerCase()}.${cls} (${Math.round(r.width)}px)${text ? ` ${JSON.stringify(text)}` : ""}`;
          break;
        }
      }
    }

    // Controls a thumb has to hit, in the work and the dock.
    const scope = [main, dock].filter(Boolean);
    const controls = scope
      .flatMap((s) => [...s.querySelectorAll('button, input:not([type="checkbox"]):not([type="radio"]):not([type="file"]), [role="tab"], [role="combobox"], label:has(> input[type="file"])')])
      .filter(visible)
      .filter((el) => !el.closest('[class*="alert"]'));
    const small = controls
      .filter((el) => el.getBoundingClientRect().height < 39.5)
      .map((el) => `${el.tagName.toLowerCase()} "${(el.textContent || el.getAttribute("aria-label") || "").trim().slice(0, 20)}" ${Math.round(el.getBoundingClientRect().height)}px`);

    const aside = document.querySelector('aside[aria-label="Main"]');
    return {
      overflow,
      widest,
      contentControl: px(main, "--ui-control"),
      headerControl: px(header, "--ui-control"),
      sidebarControl: px(aside, "--ui-control"),
      small,
      primaries: [...document.querySelectorAll('button[data-variant="primary"]')].filter(visible).length,
      screenScans: [...document.querySelectorAll('[data-scan="screen"]')].filter(visible).length,
      headerScans: [...document.querySelectorAll('[data-scan="header"]')].filter(visible).length,
      current: aside ? aside.querySelectorAll('[aria-current="page"]').length : -1,
      zeroCounts: aside ? [...aside.querySelectorAll('[aria-label$=" waiting"]')].filter((b) => /^0\b/.test(b.getAttribute("aria-label"))).length : 0,
      dockBottom: dock ? dock.getBoundingClientRect().bottom : null,
      innerHeight: window.innerHeight,
      title: document.querySelector("h1")?.textContent?.trim() ?? "",
    };
  });

  for (const p of problems) note(where, p);
  if (seen.overflow > 2) note(where, `${seen.overflow}px wider than the window. Widest: ${seen.widest}`);
  // Every page in the app frame has a heading. The auth ground's loading and
  // empty states are a single card and are allowed not to.
  if (route.frame === "app" && !seen.title) note(where, "no page title (h1)");

  const touch = route.floor;
  const wantContent = touch ? "48px" : "34px";
  if (seen.contentControl !== wantContent) {
    note(where, `content at --ui-control ${seen.contentControl || "(unset)"}, expected ${wantContent}`);
  }
  if (route.frame === "app") {
    if (seen.headerControl !== "34px") note(where, `the header is at --ui-control ${seen.headerControl}, expected desktop 34px`);
    if (seen.sidebarControl !== "34px") note(where, `the sidebar is at --ui-control ${seen.sidebarControl}, expected desktop 34px`);
    // Account and passkeys live in the user menu, so they mark nothing; every
    // other screen marks exactly its own item. `nav.test.ts` holds which is which.
    if (seen.current > 1) note(where, `${seen.current} sidebar items marked current, expected at most 1`);
    if (seen.zeroCounts > 0) note(where, `${seen.zeroCounts} sidebar count(s) at zero; zero hides (D112)`);
  }
  if (touch && seen.small.length) note(where, `controls under the 40px touch target: ${seen.small.join(", ")}`);
  if (seen.primaries > 1) note(where, `${seen.primaries} primary buttons visible; one primary action at a time`);
  if (seen.screenScans > 1) note(where, `${seen.screenScans} scan fields on the screen`);
  if (seen.screenScans > 0 && seen.headerScans > 0) note(where, "the screen claims the scanner and the header offers search too (D117)");
  if (touch && seen.dockBottom !== null && seen.dockBottom > seen.innerHeight + 1) {
    note(where, `the dock ends ${Math.round(seen.dockBottom - seen.innerHeight)}px below the screen (D134)`);
  }

  const shot = `${route.path.replace(/\//g, "-")}-${as}.png`;
  await page.screenshot({ path: join(SHOTS, shot), fullPage: true });
  await context.close();
  console.log(`  ${where.padEnd(40)} ${seen.title}`);
}

await browser.close();
server.close();

if (failures.length) {
  console.error(`\n${failures.length} render failure${failures.length === 1 ? "" : "s"}:\n`);
  for (const f of failures) console.error(`  ✗ ${f}`);
  process.exit(1);
}

console.log(`\nrender ok — ${ROUTES.length} fixtures, ${VISITS.length} renders, screenshots in .render`);
