#!/usr/bin/env node
/**
 * THE FRAME, AS THE PRODUCTION BUNDLE ACTUALLY BUILDS IT (D171).
 *
 * The render gate measures `dist-review`, where every route is a fixture that
 * draws its own frame with a literal session. So the frame the deployment
 * uses, the one with the session and the network in it, is checked here.
 *
 * This serves `dist` (what the Dockerfile copies), answers the API with canned
 * JSON, and asks what the persistent frame is for:
 *
 *  - a handheld screen's dock reaches the frame's bar, sits in the reachable
 *    third, and takes no room while it is empty (D134);
 *  - the sidebar and header are the same DOM nodes after a navigation, so a
 *    move between screens reconciles the frame rather than remounting it;
 *  - `/sessions/current` is fetched once, not once per screen, and choosing a
 *    finding does not re-read the counts;
 *  - a link to one finding opens it, Back closes it, and a reload restores it
 *    (D135);
 *  - on a phone the sidebar is a drawer behind the Menu button;
 *  - the passkey button runs a real WebAuthn ceremony.
 *
 * None of that is visible to a grep.
 *
 *   npm run frame        after `npm run build`
 */

import { createServer } from "node:http";
import { generateKeyPairSync, randomBytes } from "node:crypto";
import { readFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join, extname, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const DIST = join(ROOT, "dist");

if (!existsSync(join(DIST, "index.html"))) {
  console.error("No dist/ to check. Run `npm run build` first.");
  process.exit(1);
}

const TYPES = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".woff2": "font/woff2",
  ".svg": "image/svg+xml",
};

const SITE = "01a0-s";

/** Unpadded base64url, which is what every WebAuthn field on the wire is. */
const b64url = (buf) => buf.toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");

/**
 * Enough of an answer for each screen to draw.
 *
 * Deliberately empty lists: what is under test is the frame, not the work, and
 * an empty screen is the one that still has to put its dock in the right place.
 */
const CANNED = {
  "/api/sessions/current": {
    person_id: "01a0-p",
    display_name: "K. Phillips",
    tenant_id: "01a0-t",
    tenant_name: "Harbourline",
    site_id: SITE,
    site_code: "MEL",
  },
  "/api/work": { pack: 3, pick: 1, despatch: 0, findings: 2, no_site: false },
  "/api/capture": { site: "MEL", walk: [] },
  [`/api/sites/${SITE}/picking`]: { site: "MEL", lines: [] },
  // One finding, so the evidence panel has something to open with. Every
  // optional field is null on purpose: what is under test is the panel arriving
  // in the shell's column, not what a finding looks like.
  // **Two dozen, so the page scrolls.** The sticky chrome is only worth
  // asserting against a page long enough to scroll past, and a gate that
  // checks it on a screen that fits is a gate that checks nothing.
  "/api/discrepancies?state=open%2Cinvestigating": Array.from({ length: 24 }, (_, i) => ({
      id: `01a0-d${i}`,
      kind: "count_variance",
      state: "open",
      item_id: null,
      item_code: `NYL-44${i}`,
      holder_location_id: null,
      location_code: "A-01-1",
      holder_package_id: null,
      package_barcode: null,
      expected_quantity: "12",
      observed_quantity: "11",
      variance: "-1",
      detail: null,
      stock_count_id: null,
      stock_movement_id: null,
      detected_at: "2026-08-21T00:00:00Z",
      detected_by_id: null,
      resolving_movement_id: null,
      resolved_at: null,
      resolved_by_id: null,
      resolution_reason: null,
      detected_by_name: "K. Phillips",
      resolved_by_name: null,
      evidence: [],
  })),
  // One finding by id, for the link D135 waited for. An object rather than a
  // list: the `?? []` fallback below would answer an array to a read that wants
  // a row, and the screen would draw a panel with nothing in it.
  "/api/discrepancies/01a0-d7": {
    id: "01a0-d7",
    kind: "damage",
    state: "accepted",
    item_id: null,
    item_code: "NYL-447",
    holder_location_id: null,
    location_code: "A-01-1",
    holder_package_id: null,
    package_barcode: null,
    expected_quantity: "12",
    observed_quantity: "9",
    variance: "-3",
    detail: null,
    stock_count_id: null,
    stock_movement_id: null,
    detected_at: "2026-08-21T00:00:00Z",
    detected_by_id: null,
    resolving_movement_id: null,
    resolved_at: "2026-08-22T00:00:00Z",
    resolved_by_id: null,
    resolution_reason: "Written off against the carrier claim",
    detected_by_name: "K. Phillips",
    resolved_by_name: "K. Phillips",
    evidence: [],
  },
  // The Closed tab, which is where a link to that one has to land: it is
  // accepted, and Live cannot show it.
  "/api/discrepancies?state=resolved%2Caccepted": [],
  // The first row of the queue, by id, for the refresh: a reload of a chosen
  // row reads the finding rather than waiting to find it in the list.
  "/api/discrepancies/01a0-d0": {
    id: "01a0-d0",
    kind: "count_variance",
    state: "open",
    item_id: null,
    item_code: "NYL-440",
    holder_location_id: null,
    location_code: "A-01-1",
    holder_package_id: null,
    package_barcode: null,
    expected_quantity: "12",
    observed_quantity: "11",
    variance: "-1",
    detail: null,
    stock_count_id: null,
    stock_movement_id: null,
    detected_at: "2026-08-21T00:00:00Z",
    detected_by_id: null,
    resolving_movement_id: null,
    resolved_at: null,
    resolved_by_id: null,
    resolution_reason: null,
    detected_by_name: "K. Phillips",
    resolved_by_name: null,
    evidence: [],
  },
  // **The passkey ceremony, which had no caller at all until today.** `/keys`
  // enrolled credentials and the server's authentication endpoints were written
  // and tested, and the React sign-in page shipped the password half only — so
  // it was possible to register a key that could not then be used. A canned
  // challenge is enough to prove the client's half: it decodes the options,
  // asks the authenticator, and encodes what comes back.
  "/api/passkeys/authentication/begin": {
    ceremony_id: "01a0-c1",
    options: {
      publicKey: {
        // Real random bytes, base64url and unpadded, because the browser
        // decodes this before it will ask an authenticator anything.
        challenge: b64url(randomBytes(32)),
        rpId: "localhost",
        // Empty: the discoverable path, where the authenticator offers what it
        // holds for this origin and the assertion says whose it is.
        allowCredentials: [],
        userVerification: "preferred",
        timeout: 60_000,
      },
    },
  },
  "/api/passkeys/authentication/finish": {
    person_id: "01a0-p",
    display_name: "K. Phillips",
    tenant_id: "01a0-t",
    site_id: SITE,
    expires_at: "2026-09-30T00:00:00Z",
    token: "not-read-by-a-browser",
  },
  [`/api/sites/${SITE}/despatch`]: {
    site: "MEL",
    waiting: [],
    booked: [],
    gone_today: [],
    carriers: [],
    providers: [],
  },
};

const server = createServer(async (req, res) => {
  const url = new URL(req.url ?? "/", "http://localhost");
  if (url.pathname.startsWith("/api/")) {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify(CANNED[url.pathname + url.search] ?? CANNED[url.pathname] ?? []));
    return;
  }
  // The same shape the server serves: assets under their prefix, the page for
  // everything else, so client routing is exercised rather than bypassed.
  const file = url.pathname.startsWith("/assets/")
    ? join(DIST, url.pathname)
    : join(DIST, "index.html");
  try {
    const bytes = await readFile(file);
    res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" });
    res.end(bytes);
  } catch {
    res.writeHead(404).end();
  }
});

await new Promise((resolve) => server.listen(0, resolve));
const base = `http://localhost:${server.address().port}`;

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });

const failures = [];
const crashes = [];
// A screen that threw renders nothing, and every assertion after it fails for
// the wrong reason. Say which it was.
page.on("pageerror", (error) => crashes.push(String(error).split("\n")[0]));

function say(ok, what) {
  console.log(`  ${ok ? "ok  " : "FAIL"}  ${what}`);
  if (!ok) failures.push(what);
}

const SIDEBAR = 'aside[aria-label="Main"]';
const DOCK = '[data-region="dock"]';
const DRAWER = '[role="dialog"]';
const ROWS = 'table[aria-label="Findings"] tbody tr';

/** The frame is up: the header is the first thing it draws. */
const settle = () => page.waitForSelector("header", { timeout: 15_000 });
/** The findings have arrived, not just the route. */
const findings = () => page.waitForSelector(ROWS, { timeout: 15_000 });
const titled = (t) => page.waitForFunction((want) => document.title.startsWith(want), t, { timeout: 15_000 });
const drawerShown = (shown) =>
  page
    .waitForFunction((want) => (document.querySelector('[role="dialog"]') !== null) === want, shown, { timeout: 15_000 })
    .catch(() => {});
const sidebarShown = (shown) =>
  page
    .waitForFunction((want) => document.querySelector('aside[aria-label="Main"]')?.checkVisibility() === want, shown, {
      timeout: 5_000,
    })
    .catch(() => {});

// ── a handheld screen's dock reaches the frame ─────────────────────────────
await page.setViewportSize({ width: 430, height: 932 });
await page.goto(`${base}/picking`);
await settle();
say((await page.locator(DOCK).count()) === 1, "picking draws one dock");
await page
  .waitForFunction((sel) => (document.querySelector(sel)?.textContent ?? "").trim().length > 0, DOCK, { timeout: 15_000 })
  .catch(() => {});
say((await page.locator(DOCK).innerText()).trim().length > 0, "the screen's own state is in the frame's dock");
const dock = await page.locator(DOCK).boundingBox();
say(dock !== null && dock.y + dock.height > 932 - 80, "and the dock is in the reachable third (D134)");

// ── and takes nothing while it holds nothing ───────────────────────────────
await page.goto(`${base}/capture`);
await settle();
say((await page.locator(DOCK).count()) === 1, "capture's dock container is there with nothing in it");
say(!(await page.locator(DOCK).isVisible()), "and out of the layout, so an empty dock costs nothing");

// ── the frame is the same frame afterwards ─────────────────────────────────
await page.setViewportSize({ width: 1440, height: 900 });
await page.goto(`${base}/pack`);
await settle();
await page.evaluate((sel) => {
  document.querySelector(sel).dataset["stamp"] = "sidebar";
  document.querySelector("header").dataset["stamp"] = "header";
}, SIDEBAR);
await page.click(`${SIDEBAR} a[href="/despatch"]`);
await titled("Despatch");
say(new URL(page.url()).pathname === "/despatch", "a sidebar link navigates without a page load");
say(
  await page.evaluate(
    (sel) =>
      document.querySelector(sel)?.dataset["stamp"] === "sidebar" &&
      document.querySelector("header")?.dataset["stamp"] === "header",
    SIDEBAR,
  ),
  "the sidebar and header are the same DOM nodes after it",
);
say(
  (await page.locator(`${SIDEBAR} [aria-current="page"]`).getAttribute("href")) === "/despatch",
  "and the sidebar marks where it went",
);

// ── one session for the application ────────────────────────────────────────
let sessions = 0;
page.on("request", (request) => {
  if (new URL(request.url()).pathname === "/api/sessions/current") sessions += 1;
});
await page.click(`${SIDEBAR} a[href="/weigh"]`);
await titled("Weigh");
await page.click(`${SIDEBAR} a[href="/findings"]`);
await titled("Findings");
await findings();
say(sessions === 0, `no session refetch across two more navigations (saw ${sessions})`);

// ── choosing a finding is a place, and not an arrival ──────────────────────
let works = 0;
const countWork = (request) => {
  if (new URL(request.url()).pathname === "/api/work") works += 1;
};
page.on("request", countWork);
await page.locator(ROWS).first().click();
await drawerShown(true);
say((await page.locator(DRAWER).count()) === 1, "choosing a finding opens its drawer");
say(
  new URL(page.url()).pathname === "/findings/01a0-d0",
  `and the finding is in the path (saw ${new URL(page.url()).pathname})`,
);
await page.keyboard.press("Escape");
await drawerShown(false);
await page.locator(ROWS).nth(1).click();
await page.waitForFunction(() => location.pathname === "/findings/01a0-d1", null, { timeout: 15_000 }).catch(() => {});
say(new URL(page.url()).pathname === "/findings/01a0-d1", "a second row is a second finding in the path");
say(works === 0, `and the counts are not re-read to get there (saw ${works})`);
page.off("request", countWork);

// ── a link to one finding restores it, which is D135 ───────────────────────
// A fresh load of a URL somebody could have been sent, for a finding that is
// not on the tab the screen opens on.
await page.goto(`${base}/findings/01a0-d7`);
await settle();
await drawerShown(true);
say((await page.locator(DRAWER).count()) === 1, "a link to one finding opens it, with nothing clicked");
say((await page.locator(DRAWER).innerText()).includes("carrier claim"), "and it is that finding, read by id");
say(
  ((await page.locator('[aria-label="Which findings"] [aria-selected="true"]').innerText()) ?? "").startsWith("Closed"),
  "and the tab moved to the one that can show a closed finding",
);

// ── Back is the way out, and a reload comes back ───────────────────────────
await page.goto(`${base}/findings`);
await settle();
await findings();
await page.locator(ROWS).first().click();
await drawerShown(true);
await page.goBack();
await drawerShown(false);
say(new URL(page.url()).pathname === "/findings", "Back leaves the finding");
say((await page.locator(DRAWER).count()) === 0, "and the drawer closes with it");
await page.goForward();
await drawerShown(true);
await page.reload();
await settle();
await drawerShown(true);
say((await page.locator(DRAWER).count()) === 1, "a reload comes back to the finding that was open");
say((await page.locator(DRAWER).innerText()).includes("NYL-440"), "and to the same one");

// ── on a phone the sidebar is a drawer ─────────────────────────────────────
await page.setViewportSize({ width: 390, height: 844 });
await page.goto(`${base}/findings`);
await settle();
await findings();
const menu = page.locator('button[aria-label="Menu"]');
say(await menu.isVisible(), "at 390px the header offers a Menu button");
say(!(await page.locator(SIDEBAR).isVisible()), "and the sidebar is not in the way of the work");
say(
  await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1),
  "and nothing scrolls sideways",
);
await menu.click();
await sidebarShown(true);
say(await page.locator(SIDEBAR).isVisible(), "Menu opens the sidebar");
await page.keyboard.press("Escape");
await sidebarShown(false);
say(!(await page.locator(SIDEBAR).isVisible()), "Escape closes it");
await menu.click();
await sidebarShown(true);
await page.click(`${SIDEBAR} a[href="/pack"]`);
await titled("Packing");
say(new URL(page.url()).pathname === "/pack", "choosing a destination navigates");
await sidebarShown(false);
say(!(await page.locator(SIDEBAR).isVisible()), "and arriving closes the sidebar");

// ── the passkey has a caller ───────────────────────────────────────────────
// A virtual authenticator makes the ceremony assertable without a person: the
// client decodes a challenge, asks for an assertion, and encodes what comes
// back.
await page.setViewportSize({ width: 1440, height: 900 });
const cdp = await page.context().newCDPSession(page);
await cdp.send("WebAuthn.enable");
const { authenticatorId } = await cdp.send("WebAuthn.addVirtualAuthenticator", {
  options: {
    protocol: "ctap2",
    transport: "internal",
    hasResidentKey: true,
    hasUserVerification: true,
    isUserVerified: true,
    automaticPresenceSimulation: true,
  },
});
const { privateKey } = generateKeyPairSync("ec", { namedCurve: "P-256" });
await cdp.send("WebAuthn.addCredential", {
  authenticatorId,
  // Plain padded base64: CDP's `binary`. The WebAuthn wire uses base64url.
  credential: {
    credentialId: Buffer.from("spork-frame-gate").toString("base64"),
    isResidentCredential: true,
    rpId: "localhost",
    userHandle: Buffer.from("01a0-p").toString("base64"),
    privateKey: privateKey.export({ type: "pkcs8", format: "der" }).toString("base64"),
    signCount: 0,
  },
});

const ceremony = [];
const onCeremony = (request) => {
  const { pathname } = new URL(request.url());
  if (pathname.startsWith("/api/passkeys/authentication/")) ceremony.push([pathname, request.postData() ?? ""]);
};
page.on("request", onCeremony);

await page.goto(`${base}/sign-in`);
const passkey = page.locator("button", { hasText: "Sign in with a passkey" }).first();
await passkey.waitFor({ timeout: 15_000 }).catch(() => {});
say(await passkey.isVisible(), "the sign-in screen offers a passkey");
await passkey.click();
await page.waitForFunction(() => location.pathname === "/", null, { timeout: 15_000 }).catch(() => {});
page.off("request", onCeremony);

const begun = ceremony.find(([p]) => p.endsWith("/begin"));
const finished = ceremony.find(([p]) => p.endsWith("/finish"));
say(begun !== undefined, "and pressing it begins a ceremony");
say(begun !== undefined && begun[1] === "{}", "with no email, so the authenticator says who this is");
say(finished !== undefined, "the assertion goes back to the server");
say(
  finished !== undefined && JSON.parse(finished[1]).credential?.response?.signature?.length > 0,
  "and it carries a signature, so the base64url edge is wired the right way round",
);
say(new URL(page.url()).pathname === "/", `and a passkey signs somebody in (saw ${new URL(page.url()).pathname})`);

await browser.close();
server.close();

for (const crash of crashes) console.error(`  threw  ${crash}`);
if (failures.length || crashes.length) {
  console.error(`\nframe failed — ${failures.length} assertions, ${crashes.length} exceptions`);
  process.exit(1);
}
console.log("\nframe ok — the frame is mounted once and the session read once");
