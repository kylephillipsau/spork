#!/usr/bin/env node
/**
 * THE FRAME, AS THE PRODUCTION BUNDLE ACTUALLY BUILDS IT.
 *
 * The render gate measures `dist-review` and every route it visits is a fixture
 * — and every fixture draws its own shell, its own chrome and its own literal
 * site and operator. So the frame the *deployment* uses, the one with the
 * session in it, was checked by nothing at all. That is the same shape of gap
 * this repository has recorded twice: a green gate measuring the wrong artefact.
 *
 * This one serves `dist` — what the Dockerfile copies — answers the API with
 * canned JSON, and asks the questions the persistent layout is for:
 *
 *  - a screen's own regions reach the shell that outlives them (the dock, the
 *    evidence panel), and take no space while they hold nothing;
 *  - the chrome, the work rail and the room's canvas are the *same DOM nodes*
 *    after a navigation, which is the whole claim;
 *  - `/sessions/current` is fetched once, not once per screen.
 *
 * Those are all properties of a running tree. None of them is visible to a
 * grep, and every one of them was false a commit ago.
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

/** 60% of the viewport, which one column of two never is. */
const window_ratio = (p) => p.viewportSize().width * 0.6;

const failures = [];
const crashes = [];
// A screen that threw renders nothing, and every assertion after it fails for
// the wrong reason. Say which it was.
page.on("pageerror", (error) => crashes.push(String(error).split("\n")[0]));

function say(ok, what) {
  console.log(`  ${ok ? "ok  " : "FAIL"}  ${what}`);
  if (!ok) failures.push(what);
}
/**
 * Waits for the frame, not for a number of milliseconds.
 *
 * A fixed delay is the classic way to write a gate that is green on a laptop
 * and red on a cold CI runner, and this one would fail a build that is fine.
 * The chrome is the first thing the frame draws, so it is what "the screen is
 * up" means here.
 */
const settle = () => page.waitForSelector('[data-region="chrome"]', { timeout: 15_000 });

/**
 * The findings have arrived.
 *
 * The title changes when the route matches, which is before the screen's read
 * comes back — so anything that depends on there being rows, including the page
 * being tall enough to scroll, has to wait for the rows and not for the route.
 * This gate went green and red on the same bundle until it did.
 */
const findings = () =>
  page.waitForSelector('[data-region="work"] button:has-text("Evidence")', { timeout: 15_000 });

// ── a screen's own region reaches the shell above it ───────────────────────
await page.goto(`${base}/picking`);
await settle();
say((await page.locator('[data-region="dock"]').count()) === 1, "picking draws one dock");
// The dock's contents arrive with the screen's read, so wait for the portal
// rather than for the clock.
await page
  .waitForFunction(
    () => (document.querySelector('[data-region="dock"]')?.textContent ?? "").trim().length > 0,
    null,
    { timeout: 15_000 },
  )
  .catch(() => {});
say(
  (await page.locator('[data-region="dock"]').innerText()).trim().length > 0,
  "the screen's own state is in the shell's dock",
);
const dock = await page.locator('[data-region="dock"]').boundingBox();
const view = page.viewportSize();
say(
  dock !== null && dock.y + dock.height > view.height - 60,
  "and the dock is in the reachable third (D134)",
);

// ── and takes nothing while it holds nothing ───────────────────────────────
await page.goto(`${base}/capture`);
await settle();
say(
  (await page.locator('[data-region="dock"]').count()) === 1,
  "capture's dock container is in the document with nothing in it",
);
say(
  !(await page.locator('[data-region="dock"]').isVisible()),
  "and out of the layout, so an empty dock costs a handheld nothing",
);

// ── the frame is the same frame afterwards ─────────────────────────────────
await page.goto(`${base}/pack`);
await settle();
const stamped = await page.evaluate(() => {
  const regions = document.querySelectorAll('[data-region="chrome"]');
  regions.forEach((node, i) => (node.dataset["stamp"] = `chrome-${i}`));
  const canvas = document.querySelector("canvas");
  if (canvas) canvas.dataset["stamp"] = "canvas";
  return regions.length;
});
say(stamped === 2, "the chrome and the work rail are both there to begin with");

await page.click('a[href="/despatch"]');
await page.waitForFunction(() => document.title.startsWith("Despatch"), null, { timeout: 15_000 });
say(new URL(page.url()).pathname === "/despatch", "a rail link navigates without a page load");
say(
  await page.evaluate(() =>
    [...document.querySelectorAll('[data-region="chrome"]')].every(
      (node, i) => node.dataset["stamp"] === `chrome-${i}`,
    ),
  ),
  "the chrome and the rail are the same DOM nodes after it",
);
say(await page.evaluate(() => document.title.startsWith("Despatch")), "and the tab title followed");

// ── one session for the application ────────────────────────────────────────
let sessions = 0;
page.on("request", (request) => {
  if (new URL(request.url()).pathname === "/api/sessions/current") sessions += 1;
});
await page.click('a[href="/weigh"]');
await page.waitForFunction(() => document.title.startsWith("Weigh"), null, { timeout: 15_000 });
await page.click('a[href="/findings"]');
await page.waitForFunction(() => document.title.startsWith("Findings"), null, { timeout: 15_000 });
await findings();
say(sessions === 0, `no session refetch across two more navigations (saw ${sessions})`);

// ── the room outlives all of it ────────────────────────────────────────────
say(
  await page.evaluate(() => document.querySelector("canvas")?.dataset["stamp"] === "canvas"),
  "the light room's canvas survived bench → bench → desk",
);
say(
  await page.evaluate(() => document.querySelector("aside") !== null),
  "the evidence panel's container is in the document",
);
say(
  await page.evaluate(() => !document.querySelector("aside").checkVisibility()),
  "and out of the layout with nothing selected",
);
const wide = await page.evaluate(
  () => document.querySelector('[data-region="work"]').getBoundingClientRect().width,
);
say(wide > window_ratio(page), "so the work column is not paying for a panel nobody opened");

// ── and opens beside the work when a row is chosen ─────────────────────────
const row = page.locator('[data-region="work"] button', { hasText: "Evidence" }).first();
if ((await row.count()) > 0) {
  await row.click();
  await page.waitForFunction(() => document.querySelector("aside")?.checkVisibility() === true, null, {
    timeout: 15_000,
  });
  say(true, "choosing a finding opens the evidence panel through the portal");
  const narrowed = await page.evaluate(
    () => document.querySelector('[data-region="work"]').getBoundingClientRect().width,
  );
  say(narrowed < wide, "and the work column gives it the room, rather than overlaying (D119)");
} else {
  say(false, "a finding to choose");
}

say(
  new URL(page.url()).pathname === "/findings/01a0-d0",
  `and the finding is in the path afterwards (saw ${new URL(page.url()).pathname})`,
);

// ── and moving between rows is not arriving anywhere ───────────────────────
// The badge counts are refreshed on arrival at a screen. Choosing a row is a
// navigation now, so keyed on the path that read fired once per finding an
// operator looked at — twenty reads of `/work` to triage twenty findings.
let works = 0;
const countWork = (request) => {
  if (new URL(request.url()).pathname === "/api/work") works += 1;
};
page.on("request", countWork);
await page.locator('[data-region="work"] button', { hasText: "Evidence" }).first().click();
await page
  .waitForFunction(() => location.pathname === "/findings/01a0-d1", null, { timeout: 15_000 })
  .catch(() => {});
say(
  new URL(page.url()).pathname === "/findings/01a0-d1",
  "a second row is a second finding in the path",
);
say(works === 0, `and the badges are not re-read to get there (saw ${works})`);
page.off("request", countWork);

// ── a link to one finding restores the panel, which is D135 ────────────────
// The whole reason `/findings/:finding` exists: this is a fresh load of a URL
// somebody could have been sent, for a finding that is *not* on the tab the
// screen opens on. The panel has to be open before anything is clicked, and the
// tab has to have moved to the one that can show it.
await page.goto(`${base}/findings/01a0-d7`);
await settle();
await page
  .waitForFunction(() => document.querySelector("aside")?.checkVisibility() === true, null, {
    timeout: 15_000,
  })
  .catch(() => {});
say(
  await page.evaluate(() => document.querySelector("aside")?.checkVisibility() === true),
  "a link to one finding opens the panel on it, with nothing clicked",
);
say(
  // `.first()`, because the panel that portals in is an `aside` too — the
  // shell's evidence column is the one that is always there.
  (await page.locator("aside").first().innerText()).includes("carrier claim"),
  "and it is that finding, read by id rather than found in the queue",
);
say(
  await page.evaluate(
    () =>
      document.querySelector('[aria-label="Which findings"] [aria-checked="true"]')?.textContent ===
      "Closed",
  ),
  "and the tab moved to the one that can show a closed finding",
);

// ── and Back is the way out of it ──────────────────────────────────────────
// Back was announced to React and the path it then read was the one from
// before the navigation, so the address bar moved and the screen did not.
// `/findings/:finding` is the first route where anybody would see that.
const chosen = page.locator('[data-region="work"] button', { hasText: "Evidence" }).first();
const panelShown = (shown) =>
  page
    .waitForFunction(
      (want) => document.querySelector("aside")?.checkVisibility() === want,
      shown,
      { timeout: 15_000 },
    )
    .catch(() => {});

await page.goto(`${base}/findings`);
await settle();
await findings();
await chosen.click();
await panelShown(true);
await page.goBack();
await panelShown(false);
say(new URL(page.url()).pathname === "/findings", "Back leaves the finding");
say(
  await page.evaluate(() => document.querySelector("aside")?.checkVisibility() === false),
  "and the panel closes with it, rather than the URL moving on its own",
);

// ── and a refresh comes back to the same evidence ──────────────────────────
// The other half of what D135 asked for: selection was in memory, so reloading
// the page a finding was open on dropped the operator on the bare queue.
await chosen.click();
await panelShown(true);
await page.reload();
await settle();
await panelShown(true);
say(
  await page.evaluate(() => document.querySelector("aside")?.checkVisibility() === true),
  "a refresh comes back to the finding that was open",
);
say(
  (await page.locator("aside").first().innerText()).includes("NYL-440"),
  "and to the same one, rather than to whichever row is first",
);

await page.goto(`${base}/findings`);
await settle();
await findings();

// ── wide: there is no dialog, and nothing to open ──────────────────────────
say(
  (await page.locator('[role="dialog"]').count()) === 0,
  "at 1440px the rail is a column and claims to be nothing else",
);
say(
  !(await page.locator("button", { hasText: "Menu" }).first().isVisible()),
  "and the Menu key is not drawn where the rail is already on the screen",
);

// ── narrow: the rail is a sheet, and the way to it is pinned ───────────────
await page.setViewportSize({ width: 390, height: 844 });
await page.goto(`${base}/findings`);
await settle();
await findings();

const menu = page.locator("button", { hasText: "Menu" }).first();
const rail = page.locator('nav[aria-label="Work"]');
say(await menu.isVisible(), "at 390px the chrome offers a Menu key");
say(!(await rail.isVisible()), "and the rail is not in the way of the work");
say(
  (await page.locator('[role="dialog"]').count()) === 0,
  "nothing claims to be a dialog while it is closed",
);

await page.evaluate(() => window.scrollTo(0, 600));
await page.waitForFunction(() => window.scrollY > 0, null, { timeout: 5_000 }).catch(() => {});
say(await page.evaluate(() => window.scrollY > 100), "a pocket-width worklist scrolls");
say(
  await page.evaluate(() => {
    const chrome = document.querySelector('[data-region="chrome"]');
    return Math.abs(chrome.getBoundingClientRect().top) < 1;
  }),
  "the chrome is pinned, so the way out is not at the top of a list you scrolled",
);

await menu.click();
await page.waitForSelector('[role="dialog"]', { timeout: 5_000 });
say(await rail.isVisible(), "the Menu key opens the sheet, with the same rail in it");
say(
  (await page.locator('nav[aria-label="Work"]').count()) === 1,
  "one rail in the document, not a second copy shown at this width",
);
const sheet = await page.locator('[role="dialog"]').boundingBox();
say(
  sheet.x === 0 && sheet.y === 0 && sheet.width === 390 && sheet.height === 844,
  "it covers the screen rather than overlaying the work (D119)",
);
say(
  await page.evaluate(() => document.querySelector('[role="dialog"]').contains(document.activeElement)),
  "and focus is inside it",
);

await page.keyboard.press("Escape");
await page.waitForSelector('[role="dialog"]', { state: "detached", timeout: 5_000 });
say(true, "Escape closes it");
say(
  await page.evaluate(() => document.activeElement?.textContent?.trim() === "Menu"),
  "and focus goes back to the key that opened it",
);

await menu.click();
await page.waitForSelector('[role="dialog"]', { timeout: 5_000 });
await page.click('nav[aria-label="Work"] a[href="/pack"]');
await page.waitForFunction(() => document.title.startsWith("Pack"), null, { timeout: 15_000 });
say(new URL(page.url()).pathname === "/pack", "choosing a destination navigates");
say(
  (await page.locator('[role="dialog"]').count()) === 0,
  "and arriving is what closes the sheet — nothing is left over the screen",
);
say(
  await page.evaluate(() => document.body.style.overflow !== "hidden"),
  "and the page scrolls again",
);
// The frame outlives the navigation now, and the scroll position is the
// frame's. Arriving 600px down a screen you have never seen would be a
// regression the old remount was hiding.
say(await page.evaluate(() => window.scrollY === 0), "and the new screen starts at the top");

// ── the band between the two, which is neither ─────────────────────────────
// Below 62rem the rail is a sheet; below 48rem the chrome stacks into three
// rows (D159). Between them is a one-row chrome with a Menu key in it, and it
// is the width nothing else here measures.
await page.setViewportSize({ width: 800, height: 900 });
await page.goto(`${base}/findings`);
await settle();
await findings();
say(await menu.isVisible(), "at 800px the Menu key is there too");
say(!(await rail.isVisible()), "and the rail is still a sheet rather than a column");
await menu.click();
await page.waitForSelector('[role="dialog"]', { timeout: 5_000 });
const tablet = await page.locator('[role="dialog"]').boundingBox();
say(
  tablet.x === 0 && tablet.width === 800 && tablet.height === 900,
  "and it covers a tablet the same way it covers a phone",
);

// ── the passkey has a caller ───────────────────────────────────────────────
// The debt this closes was not a bug in anything: it was an endpoint with
// nobody calling it, which no gate here could see. A virtual authenticator is
// what makes the ceremony assertable without a person and a fingerprint — the
// client decodes a challenge, asks for an assertion, and encodes what comes
// back, and every one of those three was written from scratch for this screen.
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
  // **Plain base64, padded.** CDP's `binary` is base64; base64url is what the
  // WebAuthn *wire* uses, and mixing the two is the exact confusion this
  // client's `webauthn.ts` exists to keep in one place.
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
  if (pathname.startsWith("/api/passkeys/authentication/")) {
    ceremony.push([pathname, request.postData() ?? ""]);
  }
};
page.on("request", onCeremony);

await page.goto(`${base}/sign-in`);
await page.waitForSelector('[data-region="work"]', { timeout: 15_000 });
const passkey = page.locator("button", { hasText: "Use a passkey" }).first();
say(await passkey.isVisible(), "the sign-in screen offers a key");
await passkey.click();
await page
  .waitForFunction(() => location.pathname === "/", null, { timeout: 15_000 })
  .catch(() => {});
page.off("request", onCeremony);

const begun = ceremony.find(([p]) => p.endsWith("/begin"));
const finished = ceremony.find(([p]) => p.endsWith("/finish"));
say(begun !== undefined, "and pressing it begins a ceremony, which nothing called before today");
say(
  begun !== undefined && begun[1] === "{}",
  "with no email, so the authenticator says who this is",
);
say(finished !== undefined, "the assertion goes back to the server");
say(
  finished !== undefined && JSON.parse(finished[1]).credential?.response?.signature?.length > 0,
  "and it carries a signature, so the base64url edge is wired the right way round",
);
say(
  new URL(page.url()).pathname === "/",
  `and a key signs somebody in (saw ${new URL(page.url()).pathname})`,
);

await browser.close();
server.close();

for (const crash of crashes) console.error(`  threw  ${crash}`);
if (failures.length || crashes.length) {
  console.error(`\nframe failed — ${failures.length} assertions, ${crashes.length} exceptions`);
  process.exit(1);
}
console.log("\nframe ok — the shell, the chrome and the room are mounted once");
