#!/usr/bin/env node
/**
 * THE SCREEN, RENDERED AND MEASURED.
 *
 * The other three gates read source. This one runs the bundle in a browser and
 * measures what came out, which is a different question and the one that had
 * been going unasked — in a single pass it found three things that six rounds
 * of reading CSS had not:
 *
 *  1. A component built, styled and documented that rendered **nowhere**,
 *     because no fixture reached the state it draws.
 *  2. A decorative layer escaping the element it decorates, painting across
 *     the whole chassis. Its symptom had been read as a design choice and
 *     tuned four times.
 *  3. A rule whose edit had silently failed to apply, so the stylesheet still
 *     shipped what its own comment said it had removed.
 *
 * None of those is visible to a grep, and all three are obvious to a
 * `getBoundingClientRect`.
 *
 * It serves `dist/` itself rather than shelling out to a preview server, so
 * `npm run render` is one command with nothing to leave running.
 *
 *   npm run render            assert, and write screenshots
 *   npm run render -- --open  also print where they went
 */

import { createServer } from "node:http";
import { readFile, mkdir } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join, extname, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";

const here = dirname(fileURLToPath(import.meta.url));
const ROOT = join(here, "..");
// **The review build, not the shipped one.** Fixtures are excluded from
// production so a deployment cannot reach invented data by typing a URL, and
// the two bundles differ in exactly one thing: the route table. Gating the
// review build is what keeps D131 true — "the states the design system draws
// are reachable from a fixture" — while the deployment has none of them.
//
// The compensating check is in `check-laws.mjs`: the production entry imports
// no fixture. Without that, this gate measures an artefact nobody ships.
const DIST = join(ROOT, "dist-review");
const SHOTS = join(ROOT, ".render");
const MOUNT = "/";

if (!existsSync(join(DIST, "index.html"))) {
  console.error("No dist/ to render. Run `npm run build` first.");
  process.exit(1);
}

const TYPES = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".svg": "image/svg+xml",
  ".woff2": "font/woff2",
};

/** The same shape the server serves: assets under the mount, index for the
 *  rest of it, so client routing is exercised rather than bypassed. */
/**
 * A stand-in photograph, so the fixture's pictures render.
 *
 * **Without this the gate can only ever see the absent state.** `Photo` swaps
 * to `NoPhoto` when the bytes 404 — which is right, because a row addressing a
 * file that is not there is a real state D132 names — and this server has no
 * API behind it, so every fixture picture took that path and the picture
 * itself, and the label that makes a borrowed one honest, rendered on no
 * screen at all. That is precisely the rule this gate exists to enforce,
 * turned on the gate.
 *
 * Sixteen pixels of gradient. It is not a photograph of anything and does not
 * need to be: what is being checked is that the frame, the crop and the source
 * label draw, not what a carton looks like.
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
  // **`/api/images/`, because that is where the bytes are now.** `Photo` takes
  // a URL rather than a digest, and the URL is built in `domain/api.ts` under
  // the API prefix — so a stand-in served from the old path would leave every
  // photograph drawing its broken-image state and the gate would still pass.
  if (url.pathname.startsWith("/api/images/")) {
    res.writeHead(200, { "content-type": "image/png" });
    res.end(STAND_IN);
    return;
  }
  // The bundle is at the root now, so this is a plain static server with an
  // index fallback — the same shape `assets.rs` serves.
  let file = join(DIST, url.pathname);
  if (url.pathname.endsWith("/")) file = join(file, "index.html");

  try {
    const body = await readFile(file);
    res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" });
    res.end(body);
  } catch {
    // Anything the bundle owns but the filesystem does not is a client route.
    const body = await readFile(join(DIST, "index.html"));
    res.writeHead(200, { "content-type": "text/html" });
    res.end(body);
  }
});

await new Promise((done) => server.listen(0, "127.0.0.1", done));
const { port } = server.address();
const base = `http://127.0.0.1:${port}${MOUNT}/`;

await mkdir(SHOTS, { recursive: true });

const failures = [];
const note = (where, what) => failures.push(`${where}: ${what}`);

/**
 * Every screen, in both faces.
 *
 * A screen the gate does not visit is a screen the gate does not check, and
 * the first thing this found was a component rendering nowhere — so a new
 * screen belongs here in the same commit that adds it, not later.
 */
/**
 * The Bench monitor, and the handheld.
 *
 * **A Floor screen reviewed at 1600px is a screen nobody will see.** D109 says
 * the surfaces differ posturally rather than dimensionally, which is the
 * argument for one token set at two densities — it is not an argument for
 * rendering the handheld at desk width and calling it checked. A dock that
 * fits on a monitor and pushes the last row off a phone is invisible here
 * unless the viewport is the phone's.
 */
const BENCH = { width: 1600, height: 1500 };
const HANDHELD = { width: 430, height: 932 };

/**
 * **A Bench screen on a phone**, which is a real way this gets used and was
 * checked by nobody. The shells put the rail beside the work above a
 * breakpoint and stacked it *above* the work below one — so a phone scrolled
 * through eleven navigation rows to reach the queue, and every gate passed
 * because every gate looked at 1600px.
 *
 * Routes carrying `alsoNarrow` are rendered a second time at this width. What
 * it asserts is the same thing every route asserts plus one: nothing overflows
 * the viewport horizontally, which is the failure a screenshot at desk width
 * cannot show.
 */
const POCKET = { width: 390, height: 844 };

const ROUTES = [
  // The front door (D147). Three routes, and two of them are the point: a site
  // with nothing waiting and a session naming no site are opposite instructions
  // that must never be drawn the same way.
  // Two faces: the outbound group and the integrity group, which is the whole
  // screen. It had two more — a paragraph on why these are counts rather than
  // totals, and a heading reading "What is waiting / at this site, now" above a
  // chrome already saying WAITING beside a tag already saying MEL.
  { path: "fixtures/home", expect: { emptySlots: 0, minFaces: 2, litKeys: 0 } },
  // D111's four outcomes. A live screen shows one of them for a moment and
  // three of the four need a warehouse with the wrong labels in it.
  //
  // Three faces because these draw the *landing screen* under an open locator,
  // so they follow `fixtures/home` — which lost the standing paragraph about
  // counts against totals.
  { path: "fixtures/scan/ambiguous", expect: { emptySlots: 0, minFaces: 2, litKeys: 0 } },
  { path: "fixtures/scan/unknown", expect: { emptySlots: 0, minFaces: 2, litKeys: 0 } },
  { path: "fixtures/scan/unrecognised", expect: { emptySlots: 0, minFaces: 2, litKeys: 0 } },
  { path: "fixtures/scan/nowhere", expect: { emptySlots: 0, minFaces: 2, litKeys: 0 } },
  { path: "fixtures/home/quiet", expect: { emptySlots: 0, minFaces: 1, litKeys: 0 } },
  { path: "fixtures/home/no-site", expect: { emptySlots: 0, minFaces: 2, litKeys: 0 } },
  { path: "fixtures/pack", expect: { emptySlots: 1, ghostPaths: 2, minAdd: 1, minFaces: 4 } },
  // The pack queue (D151). Four groups need four kinds of job, and a real site
  // rarely has all four at once — so the fixture is the only place the grouping
  // can be looked at whole.
  { path: "fixtures/queue", expect: { emptySlots: 0, minFaces: 5, litKeys: 0 } },
  { path: "fixtures/queue/clear", expect: { emptySlots: 1, minFaces: 2, litKeys: 0 } },
  { path: "fixtures/despatch", expect: { minChoosers: 2, minFaces: 4 } },
  // Setting a deployment up. Four routes because the gate makes the live screen
  // almost unreviewable — a deployment stops needing setup the moment somebody
  // does it, so these fixtures are the only way three of these four are ever
  // seen again.
  // Signing in. The company question is the state the page this replaces could
  // not draw at all, so a person with two employers could not sign in.
  // One face fewer: the screen opened with a panel containing the word "Sign
  // in", under a chrome that says Sign in, above a key that says Sign in.
  { path: "fixtures/sign-in", expect: { emptySlots: 0, minFaces: 2, litKeys: 1, density: "desk" } },
  { path: "fixtures/sign-in/company", expect: { emptySlots: 0, minFaces: 4, density: "desk" } },
  { path: "fixtures/sign-in/refused", expect: { emptySlots: 0, minFaces: 3, litKeys: 1, density: "desk" } },
  // The key was dropped in the port and is back. Two states it adds: the
  // company question reached by a key, where carrying on is a second ceremony
  // rather than a password, and a browser that has no `PublicKeyCredential` at
  // all, where a sentence stands in place of a control that could only throw.
  { path: "fixtures/sign-in/company-key", expect: { emptySlots: 0, minFaces: 4, density: "desk" } },
  { path: "fixtures/sign-in/no-keys", expect: { emptySlots: 0, minFaces: 2, litKeys: 1, density: "desk" } },
  { path: "fixtures/setup", expect: { emptySlots: 0, minFaces: 5, density: "desk" } },
  { path: "fixtures/setup/no-token", expect: { emptySlots: 0, minFaces: 5, density: "desk" } },
  { path: "fixtures/setup/closed", expect: { emptySlots: 0, minFaces: 2, density: "desk" } },
  { path: "fixtures/setup/done", expect: { emptySlots: 0, minFaces: 3, density: "desk" } },
  // Changing your own password (D143). Six routes, for the same reason setup
  // has four: a live screen shows exactly one of these and only after
  // something has already happened, so a fixture is the only way the other
  // five are ever looked at.
  // Passkeys. Four routes because a live screen reaches none of them
  // without an authenticator, and the unsupported one not even with.
  { path: "fixtures/keys", expect: { emptySlots: 0, minFaces: 2, litKeys: 1, density: "desk" } },
  { path: "fixtures/keys/none", expect: { emptySlots: 0, minFaces: 2, litKeys: 1, density: "desk" } },
  { path: "fixtures/keys/unsupported", expect: { emptySlots: 0, minFaces: 2, litKeys: 0, density: "desk" } },
  { path: "fixtures/keys/failed", expect: { emptySlots: 0, minFaces: 1, litKeys: 0, density: "desk" } },
  // The bin import (D158). Dry run and applied are the two a person reads;
  // neither lasts on a live screen, and the numbers are the real August load.
  { path: "fixtures/import", expect: { emptySlots: 0, minFaces: 1, litKeys: 0, density: "desk" } },
  { path: "fixtures/import/dry", expect: { emptySlots: 0, minFaces: 2, litKeys: 1, density: "desk" } },
  { path: "fixtures/import/applied", expect: { emptySlots: 0, minFaces: 2, litKeys: 0, density: "desk" } },
  { path: "fixtures/import/items", expect: { emptySlots: 0, minFaces: 2, litKeys: 1, density: "desk" } },
  { path: "fixtures/import/failed", expect: { emptySlots: 0, minFaces: 2, litKeys: 0, density: "desk" } },
  // The workspace. The empty-warehouse row is the one worth drawing: the bin
  // import creates a site the moment a warehouse appears in the export.
  { path: "fixtures/workspace", expect: { emptySlots: 0, minFaces: 2, litKeys: 0, density: "desk" } },
  { path: "fixtures/workspace/empty", expect: { emptySlots: 0, minFaces: 2, litKeys: 0, density: "desk" } },
  { path: "fixtures/workspace/failed", expect: { emptySlots: 0, minFaces: 1, litKeys: 0, density: "desk" } },
  // Import tokens (D158). The minted route is the reason a fixture exists: the
  // secret is in one HTTP response and a live screen shows it for seconds.
  { path: "fixtures/tokens", expect: { emptySlots: 0, minFaces: 2, litKeys: 1, density: "desk" } },
  { path: "fixtures/tokens/none", expect: { emptySlots: 0, minFaces: 2, litKeys: 1, density: "desk" } },
  { path: "fixtures/tokens/minted", expect: { emptySlots: 0, minFaces: 3, litKeys: 1, density: "desk" } },
  { path: "fixtures/tokens/failed", expect: { emptySlots: 0, minFaces: 1, litKeys: 0, density: "desk" } },
  { path: "fixtures/password", expect: { emptySlots: 0, minFaces: 4, litKeys: 1, density: "desk" } },
  { path: "fixtures/password/mismatch", expect: { emptySlots: 0, minFaces: 4, density: "desk" } },
  { path: "fixtures/password/refused", expect: { emptySlots: 0, minFaces: 5, density: "desk" } },
  { path: "fixtures/password/no-session", expect: { emptySlots: 0, minFaces: 1, litKeys: 0, density: "desk" } },
  { path: "fixtures/password/changed", expect: { emptySlots: 0, minFaces: 2, litKeys: 0, density: "desk" } },
  { path: "fixtures/password/changed-alone", expect: { emptySlots: 0, minFaces: 2, litKeys: 0, density: "desk" } },
  // Where you are working (D145). The question no browser session was ever
  // asked. Three routes because a live deployment shows none of them: one site
  // per tenant settles silently, and "attached to nowhere" is a database state
  // nobody can reproduce on demand.
  { path: "fixtures/where", expect: { emptySlots: 0, minFaces: 3, litKeys: 0, density: "desk" } },
  { path: "fixtures/where/settled", expect: { emptySlots: 0, minFaces: 2, litKeys: 0, density: "desk" } },
  { path: "fixtures/where/nowhere", expect: { emptySlots: 0, minFaces: 2, litKeys: 0, density: "desk" } },
  // Weigh: the queue with something on the scale, the reading that disagrees
  // (the state the screen exists for), and nothing waiting.
  { path: "fixtures/weigh", expect: { emptySlots: 0, minFaces: 3, minChoosers: 1, density: "desk" } },
  { path: "fixtures/weigh/disagreed", expect: { emptySlots: 0, minFaces: 4, density: "desk" } },
  { path: "fixtures/weigh/clear", expect: { emptySlots: 1, minFaces: 2, density: "desk" } },
  // The Desk surface. Five routes: the queue, the rail open on a finding with
  // both acts, the rail open on a closed one (who and why, no acts), a link to
  // a finding that is not here (D135, and the only state a deep link adds), and
  // the empty queue — which is the system working rather than an error, and the
  // only fixture that draws it.
  // Counted, not guessed: the queue is a header face and the queue face, and
  // the rail adds its own two. Guessed numbers here were wrong in both
  // directions on the first run.
  // Finding an order (D150). The superseded pair is the fixture worth having:
  // D44's cancel-and-reraise is the case a lookup would hide.
  { path: "fixtures/orders/latest", expect: { emptySlots: 0, minFaces: 3, litKeys: 1, density: "desk" } },
  { path: "fixtures/orders", expect: { emptySlots: 0, minFaces: 3, litKeys: 1, density: "desk" } },
  { path: "fixtures/orders/superseded", expect: { emptySlots: 0, minFaces: 5, litKeys: 1, density: "desk" } },
  { path: "fixtures/orders/nothing", expect: { emptySlots: 0, minFaces: 2, litKeys: 1, density: "desk" } },
  { path: "fixtures/findings", expect: { emptySlots: 0, minFaces: 2, density: "desk" } },
  { path: "fixtures/findings/evidence", expect: { emptySlots: 0, minFaces: 4, density: "desk" } },
  { path: "fixtures/findings/closed", expect: { emptySlots: 0, minFaces: 4, density: "desk" } },
  { path: "fixtures/findings/missing", expect: { emptySlots: 0, minFaces: 3, density: "desk" } },
  { path: "fixtures/findings/clear", expect: { emptySlots: 1, minFaces: 2, density: "desk" } },
  // The Floor surface, at the size of the device. Seven routes, because a
  // stage no fixture reaches is a stage this gate cannot check — the rule that
  // found a component rendering nowhere. The worklist with every list carrying
  // a row it could actually be on, then the same screen with nothing to do
  // (the only fixture reaching the "Nothing waiting" pill), then what the
  // locator can answer, then the two session stages.
  // The pick walk. It answers the carton question now (D166) and the picker
  // answers it: the first scan of a walk names the pallet or the station.
  //
  // **No lit key until a row is confirmed**, which is the state below. Nothing
  // to commit is nothing to light, and a Pick key offered before there is
  // anywhere to put the goods is a key that can only refuse.
  {
    path: "fixtures/picking",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 2, litKeys: 0, density: "floor", minScans: 1 },
  },
  {
    path: "fixtures/picking/clear",
    viewport: HANDHELD,
    // A drawn absence, and the scan bar still there: an empty walk is the
    // moment to scan the pallet for the next one.
    expect: { emptySlots: 1, minFaces: 1, litKeys: 0, density: "floor", minScans: 1 },
  },
  {
    // A row in the picker's hand, going onto a pallet. **One lit key** — the
    // one that commits it, in the dock, D116.
    path: "fixtures/picking/pallet",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 2, litKeys: 1, density: "floor", minScans: 1 },
  },
  {
    // The trolley's half of D166, and a quantity above what the bin holds —
    // said before the key is pressed rather than after the picker has walked
    // away from the shelf.
    path: "fixtures/picking/short",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 2, litKeys: 1, density: "floor", minScans: 1 },
  },
  {
    // A refusal. Not lit: the key on a notice clears it, and clearing is not
    // the work.
    path: "fixtures/picking/refused",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 3, litKeys: 0, density: "floor", minScans: 1 },
  },
  {
    path: "fixtures/picking/recorded",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 3, litKeys: 0, density: "floor", minScans: 1 },
  },
  // Receiving. The step before put-away and the same shape a third time: scan
  // the bay, then scan each carton. A GS1 label answers item, lot and date at
  // once, which is the first write path to use what the locator has always
  // parsed.
  {
    path: "fixtures/receiving",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 2, litKeys: 0, density: "floor", minScans: 1 },
  },
  {
    // A carton counted, with the lot and the date off the label. One lit key —
    // "Take it in", which is the only thing on the screen that commits.
    path: "fixtures/receiving/counting",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 3, litKeys: 1, density: "floor", minScans: 1 },
  },
  {
    path: "fixtures/receiving/over",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 3, litKeys: 1, density: "floor", minScans: 1 },
  },
  {
    // **Two lit keys here, and they are not two primary actions.** The owner
    // choice is drawn `live` to show which is chosen, the way a tab does; the
    // commit is disabled because the line is not ready. D116 is about competing
    // primaries and this is a selection.
    path: "fixtures/receiving/no-owner",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 3, litKeys: 1, density: "floor", minScans: 1 },
  },
  {
    // Refused for a missing lot. Not lit: nothing on this screen is offered as
    // the thing to do next until a line is chosen again.
    path: "fixtures/receiving/refused",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 3, litKeys: 0, density: "floor", minScans: 1 },
  },
  {
    path: "fixtures/receiving/taken-in",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 3, litKeys: 0, density: "floor", minScans: 1 },
  },
  {
    path: "fixtures/receiving/clear",
    viewport: HANDHELD,
    expect: { emptySlots: 1, minFaces: 1, litKeys: 0, density: "floor", minScans: 1 },
  },
  // Put-away: the pick walk's mirror. One source, many destinations, so the
  // goods are selected and the bin is scanned per trip.
  {
    path: "fixtures/putaway",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 2, litKeys: 0, density: "floor", minScans: 1 },
  },
  {
    // Goods in hand and a bin named: the one lit key, in the dock.
    path: "fixtures/putaway/in-hand",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 2, litKeys: 1, density: "floor", minScans: 1 },
  },
  {
    // **Held with no bin, and the key is still lit.** D116 counts what the
    // screen offers as primary, not what happens to be pressable — a key that
    // vanished until the bin was scanned would leave the operator wondering
    // what the screen wanted, which is the state this fixture exists to show.
    path: "fixtures/putaway/no-bin",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 2, litKeys: 1, density: "floor", minScans: 1 },
  },
  {
    path: "fixtures/putaway/refused",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 3, litKeys: 0, density: "floor", minScans: 1 },
  },
  {
    path: "fixtures/putaway/recorded",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 3, litKeys: 0, density: "floor", minScans: 1 },
  },
  {
    // A clear dock is the good state, drawn as one: everything that arrived
    // has a home.
    path: "fixtures/putaway/clear",
    viewport: HANDHELD,
    expect: { emptySlots: 1, minFaces: 1, litKeys: 0, density: "floor", minScans: 1 },
  },
  // **Two faces where there were four, and one drawn absence where there were
  // three.** The worklist was three lists — never recorded, part recorded,
  // never measured or overdue — and is one walk in bin order, because an
  // operator working three lists in bin order walks past every shelf three
  // times. Counted from the gate rather than reasoned about: the header face
  // with the scan bar, and the walk.
  {
    path: "fixtures/capture",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 2, minScans: 1, density: "floor" },
  },
  {
    path: "fixtures/capture/clear",
    viewport: HANDHELD,
    expect: { emptySlots: 1, minFaces: 2, density: "floor" },
  },
  // The locator, and the three things it can say. `identifier_unrecognised`
  // draws the unknown shape with a different sentence, so it does not earn a
  // route — which is a coverage decision worth stating rather than a gap.
  {
    path: "fixtures/capture/scanned",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 3, minScans: 1, density: "floor" },
  },
  {
    path: "fixtures/capture/ambiguous",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 3, minScans: 1, density: "floor" },
  },
  {
    path: "fixtures/capture/unknown",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 3, minScans: 1, density: "floor" },
  },
  {
    path: "fixtures/capture/figures",
    viewport: HANDHELD,
    expect: { emptySlots: 1, minFaces: 3, density: "floor" },
  },
  // The other figures stage: a single loose thing, which has to say how it was
  // arranged, and a set with parts, which can answer that it has no box at all.
  // Neither control is reachable from a carton.
  {
    path: "fixtures/capture/each",
    viewport: HANDHELD,
    expect: { emptySlots: 0, minFaces: 4, density: "floor" },
  },
  // What a box already answers to, and the field that binds another (D164).
  // Three bindings, one of them from before the level was recordable.
  {
    path: "fixtures/capture/barcodes",
    viewport: HANDHELD,
    // One drawn absence, and it is the subject's own: the glove carton has no
    // figures yet, so `Held` says so. Counted from the gate.
    expect: { emptySlots: 1, minFaces: 3, minScans: 1, density: "floor" },
  },
  {
    path: "fixtures/capture/faces",
    viewport: HANDHELD,
    expect: { minFaces: 3, minCameras: 7, density: "floor" },
  },
];

/**
 * **Every fixture route is rendered, or this list has quietly fallen behind.**
 *
 * `ROUTES` is an allow-list, and this project has now been bitten by three of
 * them: the contract gate's module list, its `PAIRS`, and this. Four picking
 * fixtures were added and the gate went on reporting "68 screens" — the same
 * number, cheerfully, about a client that had grown four states nothing looked
 * at. A gate that fails by passing is the failure mode here, so the count is
 * checked against the source rather than trusted.
 *
 * A fixture deliberately left out belongs in `NOT_RENDERED`, named, with a
 * reason. There are none today and that is worth keeping true.
 */
const NOT_RENDERED = new Set([]);
{
  const src = await readFile(join(ROOT, "app/routing/fixtures.tsx"), "utf8");
  const declared = [...src.matchAll(/path:\s*"\/(fixtures\/[a-z0-9/-]*)"/g)].map((m) => m[1]);
  const listed = new Set(ROUTES.map((r) => r.path));
  const missing = declared.filter((p) => !listed.has(p) && !NOT_RENDERED.has(p));
  if (missing.length > 0) {
    console.error(
      `${missing.length} fixture route(s) exist and are never rendered:\n` +
        missing.map((p) => `  /${p}`).join("\n") +
        `\n\nAdd them to ROUTES in scripts/render.mjs, or to NOT_RENDERED with a reason.`,
    );
    process.exit(1);
  }
}

const browser = await chromium.launch();

// Each route at its own width, and the desk-shaped ones again in a pocket.
const VISITS = ROUTES.flatMap((route) =>
  ["light", "dark"]
    .map((scheme) => ({ route, scheme, viewport: route.viewport ?? BENCH, narrow: false }))
    .concat(
      route.viewport
        ? []
        : [{ route, scheme: "light", viewport: POCKET, narrow: true }],
    ),
);

for (const { route, scheme, viewport, narrow } of VISITS) {
  const where = `${route.path}/${narrow ? "pocket" : scheme}`;
  const page = await browser.newPage({ viewport, colorScheme: scheme });

  const problems = [];
  page.on("console", (m) => m.type() === "error" && problems.push(m.text()));
  page.on("pageerror", (e) => problems.push(`uncaught: ${e.message}`));
  page.on("requestfailed", (r) => problems.push(`request failed: ${r.url()}`));

  await page.goto(`${base}${route.path}`, { waitUntil: "networkidle" });
  await page.waitForTimeout(400);

  const seen = await page.evaluate(() => {
    const faces = [...document.querySelectorAll('[data-layer="face"]')];

    /**
     * **A decorative layer must not escape the element it decorates.**
     *
     * This is the invariant the grain broke: `position: absolute; inset: 0`
     * resolves against the nearest *positioned* ancestor, and a parent that
     * forgot `position: relative` silently hands the layer the whole panel.
     * The stylesheet is valid, nothing warns, and the result is a texture
     * painted across everything the parent does not cover.
     */
    const escapees = [];
    for (const face of faces) {
      const box = face.getBoundingClientRect();
      for (const child of face.querySelectorAll("*")) {
        if (getComputedStyle(child).position !== "absolute") continue;
        const c = child.getBoundingClientRect();
        if (c.width > box.width + 1 || c.height > box.height + 1) {
          escapees.push(
            `${child.tagName}.${String(child.className).slice(0, 24)} ` +
              `${Math.round(c.width)}x${Math.round(c.height)} inside ` +
              `${Math.round(box.width)}x${Math.round(box.height)}`,
          );
        }
      }
    }

    const ghost = (document.querySelector('[data-region="work"]') ?? document).querySelector(
      'svg[aria-label="An empty container"]',
    );

    /**
     * **The screen, not the chrome.** Every Bench and Desk screen now carries a
     * navigation rail, and every shell a wordmark link. Counted at document
     * level, those would inflate the numbers this gate holds for twenty-nine
     * routes — and worse, a chrome scan bar would make `minScans` trivially
     * true everywhere and silently destroy the check that says *capture has a
     * locator*. So the screen's own counters look inside `[data-region="work"]`
     * and nowhere else.
     *
     * The chrome gets its own assertions below rather than being ignored.
     */
    const work = document.querySelector('[data-region="work"]') ?? document;
    const chrome = [...document.querySelectorAll('[data-region="chrome"]')];

    return {
      faces: faces.length,
      panels: document.querySelectorAll('[data-material]').length,
      unlayered: [...document.querySelectorAll("[data-material]")].filter(
        (n) => !n.hasAttribute("data-layer"),
      ).length,
      emptySlots: work.querySelectorAll('svg[aria-label="An empty container"]').length,
      ghostPaths: ghost ? ghost.querySelectorAll("path").length : 0,
      addKeys: [...work.querySelectorAll("button")].filter(
        (b) => b.textContent?.trim() === "Add",
      ).length,
      choosers: work.querySelectorAll("select").length,
      cameras: work.querySelectorAll('input[type="file"]').length,
      /**
       * **The one lit key.** `Key` says of itself that `live` marks "the one
       * key on a screen that is lit from within: the thing the operator is
       * about to do. Two lit keys on one screen means neither is." That was a
       * comment and nothing enforced it — the pack screen broke it once with
       * three violet keys, and the findings screen broke it again with four
       * tabs and an Accept.
       *
       * Counted by resolved colour rather than by class name, because CSS
       * modules hash the class and the colour is the thing the rule is
       * actually about.
       */
      /**
       * The chrome, asserted rather than ignored.
       *
       * `chromeFaces` must be nought: D118 splits the system into metal chassis
       * and paper data, and navigation is chassis. It is also what protects
       * every `minFaces` number this file holds — a face in the chrome would
       * inflate all of them at once.
       */
      chromeFaces: chrome.reduce((n, c) => n + c.querySelectorAll('[data-layer="face"]').length, 0),
      /** Exactly one way home, on every surface. */
      homeLinks: chrome.reduce(
        (n, c) => n + [...c.querySelectorAll("a")].filter((a) => new URL(a.href).pathname === "/").length,
        0,
      ),
      /** At most one place in the rail is "here". */
      currentLinks: chrome.reduce((n, c) => n + c.querySelectorAll('a[aria-current="page"]').length, 0),
      /**
       * How far the page runs past the window.
       *
       * The one thing a desk-width screenshot cannot show: a table, a row of
       * readouts or a fixed-width field that fits at 1600px and pushes the body
       * sideways at 390px. Sideways scrolling on a phone is the difference
       * between an application and a website somebody forgot to finish.
       */
      overflow: Math.max(
        0,
        document.documentElement.scrollWidth - document.documentElement.clientWidth,
      ),
      /**
       * What is out there, named.
       *
       * **A number on its own sends you looking in the wrong place.** The pack
       * bench once reported 73px and every visible thing on it was correctly
       * clipped: the culprit was a 1px screen-reader label, `position:
       * absolute` inside a scroller that was `position: static`, so it escaped
       * the clip and dragged the document with it. Two attempts at the obvious
       * fix moved the number not at all, which is the tell.
       *
       * The element whose right edge lands on `scrollWidth` is the one that
       * decided it, so that is what this reports.
       */
      widest: (() => {
        const edge = document.documentElement.scrollWidth;
        for (const el of document.querySelectorAll("*")) {
          const r = el.getBoundingClientRect();
          if (Math.abs(r.right - edge) < 1.5 && r.width > 0) {
            const cls = (el.className ?? "").toString().split(" ")[0] ?? "";
            const text = (el.textContent ?? "").trim().slice(0, 30);
            return `${el.tagName.toLowerCase()}.${cls} (${Math.round(r.width)}px wide)` +
              (text ? ` — ${JSON.stringify(text)}` : "");
          }
        }
        return "nothing whose right edge matches; look for a margin";
      })(),
      /** The chrome's locator (D111), counted apart from the screen's. */
      chromeScans: chrome.reduce(
        (n, c) => n + c.querySelectorAll('label[data-layer="instrument"] input').length,
        0,
      ),
      /** A badge is never drawn at nought (D112) — the rule made checkable. */
      zeroBadges: [...document.querySelectorAll('[data-layer="nylon"]')].filter(
        (b) => /^0$/.test(b.textContent?.trim() ?? ""),
      ).length,
      litKeys: (() => {
        const room = document.querySelector("[data-density]");
        if (!room) return 0;
        const lit = getComputedStyle(room).getPropertyValue("--key-live").trim();
        if (!lit) return 0;
        // Resolve the token to whatever the browser computes, so a hex and an
        // rgb() of the same colour compare equal.
        const probe = document.createElement("span");
        probe.style.color = lit;
        document.body.appendChild(probe);
        const want = getComputedStyle(probe).color;
        probe.remove();
        return [...document.querySelectorAll('[data-layer="key"]')].filter((k) => {
          const bg = getComputedStyle(k).backgroundColor;
          return bg === want;
        }).length;
      })(),
      // The locator is the primary input on this surface (D111), so its
      // presence is asserted rather than assumed: a scan bar that stopped
      // rendering would leave a worklist nobody can shortcut.
      // **A scanner's field, not every field that takes words.** The selector
      // was `input[inputmode="text"]`, which is also every non-numeric `Field`
      // — so the password screen reported three scan inputs and had none. Only
      // `ScanInput` marks itself an instrument.
      scans: work.querySelectorAll('label[data-layer="instrument"] input').length,
      /**
       * **The surface, read off the DOM rather than assumed.**
       *
       * D109 selects density by surface and not by viewport, and the whole
       * argument for a second shell is that a handheld wants 48px targets. A
       * Floor screen that mounted inside `density="desk"` would look almost
       * right at this viewport and be wrong on the device — which is exactly
       * the class of defect this gate exists for, since nothing about it is
       * visible to a grep.
       */
      density: document.querySelector("[data-density]")?.getAttribute("data-density") ?? "",
      // **Read off the density element, not the root.** `--target` is declared
      // inside `[data-density="floor"]`, so asking the document element for it
      // returns an empty string — which is what this did first, and an empty
      // string compared against nothing is a check that always passes.
      target: (() => {
        const room = document.querySelector("[data-density]");
        return room ? getComputedStyle(room).getPropertyValue("--target").trim() : "";
      })(),
      steel: getComputedStyle(document.documentElement).getPropertyValue("--face-steel").trim(),
      escapees,

      /**
       * **Every action in one list trails at the same margin (D167).**
       *
       * The measurement this gate was missing. Two separate bugs shipped past
       * it: an action that wrapped to the left of its own second line, and
       * before that a `Spacer` that pushed to the end of the *line* rather
       * than the end of the record — so Capture drew its key at the right
       * margin on three rows and the left on four. Both are invisible in the
       * source, invisible to a type checker, and obvious in a screenshot
       * nobody was looking at.
       *
       * Reported as the set of distinct right-hand gaps per list. One value
       * means one margin; more than one means the rows disagree, and the
       * numbers say by how much.
       */
      ragged: (() => {
        const bad = [];
        for (const list of document.querySelectorAll('[class*="records"]')) {
          const gaps = new Set();
          for (const row of list.children) {
            const action = row.querySelector('[class*="action"]');
            if (!action) continue;
            gaps.add(Math.round(row.getBoundingClientRect().right - action.getBoundingClientRect().right));
          }
          // A pixel of tolerance, because a fractional width rounds two ways
          // and that is not raggedness. A wrapped action is off by hundreds.
          const seen = [...gaps].sort((a, b) => a - b);
          if (seen.length > 1 && seen[seen.length - 1] - seen[0] > 1) bad.push(seen.join("/"));
        }
        return bad;
      })(),

      /**
       * **A notice's key sits on the right margin, not the left of line two.**
       *
       * The same failure as `ragged`, in the other pattern, found the same way
       * — by opening a screenshot. Fifteen screens wrote `Row` + `Spacer` for
       * this, and at 430px the sentence fills the line, the key wraps, and
       * `margin-left: auto` puts it at the start of the next one.
       *
       * A single notice has no siblings to disagree with, so the invariant is
       * absolute rather than relative: flush right, within the pixel a
       * fractional width rounds by.
       */
      strandedKeys: (() => {
        const bad = [];
        /*
         * **Regression cover for the two patterns that encode the fix, and it
         * is honest about being only that.**
         *
         * `Record`, `Notice` and `Trailing` each put the trailing key in a
         * growing, end-aligned box so it lands on the right margin whether it
         * shares a line or takes one. This measures that it still does.
         *
         * It would **not** have caught the bug that produced `Trailing`: a dock
         * with a `Spacer` and two loose keys in a wrapping row has no box to
         * measure. Two attempts at a general geometric rule failed — the second
         * reported 114 findings across screens with nothing wrong, because "the
         * last key sits left of something wider above it" is true of any row
         * where a key is not the widest thing in it. The source shape is
         * checkable and the geometry is not, so `check-laws.mjs` has that half.
         */
        for (const box of document.querySelectorAll('[class*="notice"], [class*="trailing"]')) {
          const keys = box.querySelectorAll("button");
          const key = keys[keys.length - 1];
          if (!key) continue;
          const gap = box.getBoundingClientRect().right - key.getBoundingClientRect().right;
          if (gap > 1) bad.push(Math.round(gap));
        }
        return bad;
      })(),
    };
  });

  const want = route.expect;

  // True of every screen.
  if (seen.faces < (want.minFaces ?? 1)) note(where, `only ${seen.faces} faces rendered`);
  // The rule `Key` states about itself, enforced rather than commented.
  if (seen.litKeys > 1) {
    note(where, `${seen.litKeys} lit keys — two lit keys on one screen means neither is`);
  }
  if (seen.panels < 1) note(where, "no material rendered");
  if (seen.unlayered > 0) note(where, `${seen.unlayered} material(s) with no data-layer`);
  if (!seen.steel) note(where, "--face-steel does not resolve");
  for (const e of seen.escapees) note(where, `layer escaped its parent: ${e}`);
  for (const r of seen.ragged) {
    note(where, `a list's actions trail at different margins (${r}px) — see D167`);
  }
  for (const g of seen.strandedKeys) {
    note(where, `a trailing key is ${g}px off the right margin — it wrapped, see D167`);
  }
  for (const p of problems) note(where, p);

  // And what this screen in particular is supposed to draw. A state no fixture
  // reaches is a state nobody has seen, however carefully it was built.
  // **Exact, not a floor.** This was `<`, which makes `emptySlots: 0` a check
  // that can never fail — the shape this gate caught in `--target` and the one
  // worth refusing on sight. An exact count also says something the floor
  // could not: the capture worklist draws *no* absence because every list has
  // a row, and `capture/clear` draws three because none does.
  if (want.emptySlots !== undefined && seen.emptySlots !== want.emptySlots) {
    note(
      where,
      `${seen.emptySlots} empty-container state(s), expected exactly ${want.emptySlots}`,
    );
  }
  if (want.ghostPaths !== undefined && seen.ghostPaths !== want.ghostPaths) {
    note(where, `ghost drew ${seen.ghostPaths} paths, expected ${want.ghostPaths}`);
  }
  if (want.minAdd !== undefined && seen.addKeys < want.minAdd) {
    note(where, "no Add control rendered");
  }
  if (want.minChoosers !== undefined && seen.choosers < want.minChoosers) {
    note(where, `${seen.choosers} chooser(s), expected at least ${want.minChoosers}`);
  }
  // Universal, on every route: the chrome is chassis and carries no data, there
  // is exactly one way home, at most one destination is current, and no badge
  // shows a nought.
  if (seen.chromeFaces > 0) {
    note(where, `${seen.chromeFaces} instrument face(s) in the chrome — the chassis carries no data`);
  }
  if (seen.homeLinks !== 1) {
    note(where, `${seen.homeLinks} link(s) home in the chrome, expected exactly 1`);
  }
  if (seen.currentLinks > 1) {
    note(where, `${seen.currentLinks} destinations marked current, expected at most 1`);
  }
  // **Never two.** D111 wants a locator on every surface and D117 says a screen
  // that claims the scanner keeps it; the failure to guard against is both at
  // once, fighting over the caret. Which screens draw one is decided by the
  // manifest and asserted in `app/routing/manifest.test.ts` — a fixture that
  // does not model the chrome cannot answer that question, and a floor here
  // would fail every such route for the wrong reason.
  if (seen.chromeScans > 1) {
    note(where, `${seen.chromeScans} locators in the chrome, expected at most 1`);
  }
  if (seen.chromeScans > 0 && seen.scans > 0) {
    note(where, "two locators: the screen claimed the scanner and the chrome drew one anyway");
  }
  // Two pixels of slack for a sub-pixel layout rounding; anything more is a
  // real overhang.
  if (seen.overflow > 2) {
    note(
      where,
      `${seen.overflow}px wider than the window — the page scrolls sideways. ` +
        `Widest: ${seen.widest}`,
    );
  }
  if (seen.zeroBadges > 0) {
    note(where, `${seen.zeroBadges} badge(s) showing nought — D112 says zero hides`);
  }

  if (want.minScans !== undefined && seen.scans < want.minScans) {
    note(where, `${seen.scans} scan input(s), expected at least ${want.minScans}`);
  }
  if (want.minCameras !== undefined && seen.cameras < want.minCameras) {
    note(where, `${seen.cameras} camera control(s), expected ${want.minCameras}`);
  }
  if (want.density !== undefined) {
    if (seen.density !== want.density) {
      note(where, `mounted at density ${seen.density || "(none)"}, expected ${want.density}`);
    }
    // The density is only worth asserting because it changes the touch target.
    // 48px on the floor is the number D109's whole argument rests on.
    const wantTarget = want.density === "floor" ? "48px" : "34px";
    if (seen.target !== wantTarget) {
      note(where, `--target is ${seen.target || "(unset)"}, expected ${wantTarget}`);
    }
  }

  // Named for the visit rather than the scheme, or the pocket render
  // overwrites the light one and the artefact quietly loses half of what it
  // measured.
  const shot = `${route.path.replace(/\//g, "-")}-${narrow ? "pocket" : scheme}.png`;
  await page.screenshot({ path: join(SHOTS, shot), fullPage: true });
  await page.close();

  console.log(
    `  ${where.padEnd(24)} faces ${seen.faces} · empty ${seen.emptySlots} · ` +
      `add ${seen.addKeys} · lit ${seen.litKeys} · choosers ${seen.choosers} · ` +
      `scans ${seen.scans} · ` +
      `cameras ${seen.cameras} · ` +
      `${seen.density}/${seen.target} · steel ${seen.steel}`,
  );
}

await browser.close();
server.close();

if (failures.length) {
  console.error(`\n${failures.length} render failure${failures.length === 1 ? "" : "s"}:\n`);
  for (const f of failures) console.error(`  ✗ ${f}`);
  process.exit(1);
}

console.log(
  `render ok — ${ROUTES.length} screens, ${VISITS.length} renders ` +
    `(both faces, and a 390px pocket for every desk-shaped one), ` +
    `screenshots in ${SHOTS.replace(ROOT, ".")}`,
);
