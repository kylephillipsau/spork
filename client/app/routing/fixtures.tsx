/**
 * Every state the screens can draw, reachable with no network.
 *
 * **Review build only.** These are behind `import.meta.env.MODE === "review"`
 * in `main.tsx`, so Rollup drops the branch and this whole module from the
 * production bundle. A deployment cannot reach invented data by typing a URL,
 * and the render gate still visits every one of them.
 *
 * D131 is unchanged by that: *"the states the screens draw are reachable from
 * a fixture"* stays true, because the gate builds in review mode. What changes
 * is only that a customer cannot.
 */
import type { ReactElement } from "react";

import { UiRoot } from "@ui/index";
import { AppShell } from "@app/shell/AppShell";
import { AuthLayout } from "@app/shell/frames";
import { Dock, DockHost } from "@app/shell/dock";
import { SessionProvider } from "@app/session/SessionContext";
import type { Badge } from "@app/shell/nav";
import { pattern } from "@domain/routing";
import type { Resolution, SiteRow } from "@domain/types";
import { SCREENS } from "./manifest";
import type { Landing } from "@app/scan/destination";
import { AMBIGUOUS, NO_SCREEN, UNKNOWN, UNRECOGNISED, fixtureScan } from "@app/scan/fixture";

import { witness } from "@app/measurement/baseline";

import { Dashboard } from "@app/home/Dashboard";
import { DASHBOARD, DASHBOARD_NO_SITE, DASHBOARD_QUIET } from "@app/home/dashboard-fixture";

import { PackBenchPage } from "@app/outbound/pack/PackBenchPage";
import { PACK_FIXTURE } from "@app/outbound/pack/fixture";
import { PackQueuePage } from "@app/outbound/pack/PackQueuePage";
import { CLEAR, QUEUE, fixtureQueue } from "@app/outbound/pack/queue-fixture";

import { DespatchPage } from "@app/outbound/despatch/DespatchPage";
import { BOOKED, DESPATCH_FIXTURE } from "@app/outbound/despatch/fixture";

import { CapturePage, CaptureDockPage } from "@app/measurement/capture/CapturePage";
import {
  BOUND_BARCODES,
  CAPTURE_CLEAR,
  CAPTURE_SUBJECT,
  CAPTURE_SUBJECT_EACH,
  SCAN_AMBIGUOUS,
  SCAN_RESOLVED,
  SCAN_UNKNOWN,
  fixtureBench,
} from "@app/measurement/capture/fixture";
import type { CaptureBench } from "@app/measurement/capture/useCapture";

import { PickingPage, PickingDock } from "@app/outbound/picking/PickingPage";
import { AT_THE_STATION, ON_A_PALLET, PICKING_CLEAR, PICKING_FIXTURE, fixturePicking } from "@app/outbound/picking/fixture";
import type { PickBench } from "@app/outbound/picking/usePicking";

import { ReceivingPage, ReceivingDockPage } from "@app/inbound/receiving/ReceivingPage";
import { RECEIVING_CLEAR, RECEIVING_FIXTURE, THE_DOCK, fixtureReceiving } from "@app/inbound/receiving/fixture";
import type { ReceivingBench } from "@app/inbound/receiving/useReceiving";

import { PutawayPage, PutawayDockPage } from "@app/inbound/putaway/PutawayPage";
import { A_BIN, PUTAWAY_CLEAR, PUTAWAY_FIXTURE, fixturePutaway } from "@app/inbound/putaway/fixture";
import type { PutawayBench } from "@app/inbound/putaway/usePutaway";

import { FindingsPage } from "@app/integrity/findings/FindingsPage";
import { FINDINGS_CLEAR, FINDINGS_FIXTURE, SHORT_PICK, fixtureDesk } from "@app/integrity/findings/fixture";

import { WeighPage } from "@app/measurement/weigh/WeighPage";
import { DISAGREED, WEIGH_CLEAR, fixtureBench as weighFixture } from "@app/measurement/weigh/fixture";

import { OrdersPage } from "@app/outbound/orders/OrdersPage";
import { FOUND, LATEST, NOTHING, SUPERSEDED, fixtureOrders } from "@app/outbound/orders/fixture";

import { SetupPage } from "@app/setup/SetupPage";
import { CLOSED, DONE, NEEDED, NO_TOKEN, fixtureSetup } from "@app/setup/fixture";

import { SignInPage } from "@app/session/SignInPage";
import { ASKING, CHOOSE_COMPANY, REFUSED, fixtureSignIn } from "@app/session/signin-fixture";
import { WherePage } from "@app/session/WherePage";
import { CHOOSE, NOWHERE, SETTLED, fixtureWhere } from "@app/session/fixture";

import { AccountPage } from "@app/account/AccountPage";
import { DONE_ALONE, DONE_ENDED, NO_SESSION, READY, fixturePassword } from "@app/account/fixture";
import { KeysPage } from "@app/account/KeysPage";
import { FAILED as KEYS_FAILED, NONE as KEYS_NONE, READY as KEYS_READY, fixtureKeys } from "@app/account/keys-fixture";

import { TokensPage } from "@app/admin/TokensPage";
import { FAILED as TOKENS_FAILED, MINTED, NONE as TOKENS_NONE, READY as TOKENS_READY, fixtureTokens } from "@app/admin/fixture";
import { ImportPage } from "@app/admin/ImportPage";
import { APPLIED as IMP_APPLIED, DRY as IMP_DRY, FAILED as IMP_FAILED, IDLE as IMP_IDLE, ITEMS as IMP_ITEMS, fixtureImport } from "@app/admin/import-fixture";
import { WorkspacePage } from "@app/admin/WorkspacePage";
import { EMPTY as WS_EMPTY, FAILED as WS_FAILED, READY as WS_READY, fixtureWorkspace } from "@app/admin/workspace-fixture";

import type { Screen, Surface } from "./Router";

/* ---- frames ------------------------------------------------------------ */

/**
 * A fixture's session: literal, so the provider never asks the server. The
 * frame the live screens share fetches a session and the badge counts, and the
 * render gate counts a failed request as a failure.
 */
const WHO = {
  kind: "signed-in" as const,
  who: {
    person_id: "p",
    display_name: "D. Stooke",
    tenant_id: "t",
    tenant_name: "Alpha Foods",
    site_id: "s",
    site_code: "MEL",
  },
};

/** The warehouses the header's site switcher lists: the session's, and one more. */
const SITES: SiteRow[] = [
  { id: "s", code: "MEL", name: "Melbourne", current: true },
  { id: "s2", code: "SYD", name: "Sydney", current: false },
];

/** The live path a fixture stands for, so the sidebar marks the right item. */
const pathOf = (screen: string) => SCREENS.find((x) => x.id === screen)?.path ?? "/";

/** The fixture's own badge counts. `home/quiet` has none, which is what proves
 *  zero hides rather than drawing a nought (D112). */
type Counts = Readonly<Partial<Record<Badge, number>>>;
const BUSY: Counts = { pack: 4, pick: 12, despatch: 1, findings: 7 };

interface Frame {
  /** The live screen this draws, so the sidebar marks it. */
  screen: string;
  counts?: Counts;
  landing?: Landing | null;
}

/** A desktop or handheld screen in the app shell. */
function AppFrame({ frame, title, touch, children }: { frame: Frame; title: string; touch: boolean; children: ReactElement }) {
  return (
    <UiRoot density={touch ? "touch" : "desktop"}>
      <SessionProvider session={WHO}>
        <AppShell
          screenId={frame.screen}
          title={title}
          counts={frame.counts ?? BUSY}
          scan={fixtureScan(frame.landing ?? null)}
          sites={SITES}
          at={pathOf(frame.screen)}
          // Handheld screens claim the scanner themselves (D117).
          showSearch={!touch}
        >
          {touch ? <DockHost>{children}</DockHost> : children}
        </AppShell>
      </SessionProvider>
    </UiRoot>
  );
}

/** Sign-in, setup and warehouse choice: the anodised ground, no session. */
function AuthFrame({ children }: { children: ReactElement }) {
  return (
    <UiRoot>
      <AuthLayout>{children}</AuthLayout>
    </UiRoot>
  );
}

/** A handheld page and its dock, from one bench. */
function Floor<B>({
  page: P,
  dock: D,
  bench,
}: {
  page: (p: { bench: B }) => ReactElement | null;
  dock: (p: { bench: B }) => ReactElement | null;
  bench: B;
}) {
  return (
    <>
      <P bench={bench} />
      <Dock>
        <D bench={bench} />
      </Dock>
    </>
  );
}

const capture = (bench: CaptureBench) => <Floor page={CapturePage} dock={CaptureDockPage} bench={bench} />;
const picking = (bench: PickBench) => <Floor page={PickingPage} dock={PickingDock} bench={bench} />;
const receiving = (bench: ReceivingBench) => <Floor page={ReceivingPage} dock={ReceivingDockPage} bench={bench} />;
const putaway = (bench: PutawayBench) => <Floor page={PutawayPage} dock={PutawayDockPage} bench={bench} />;

/** One fixture: where it lives, what it is called, and what it draws. */
function app(id: string, path: string, title: string, surface: Surface, frame: Frame, body: () => ReactElement): Screen {
  return {
    id,
    path,
    title,
    surface,
    pattern: pattern(path),
    own: true,
    render: () => (
      <AppFrame frame={frame} title={title} touch={surface === "floor"}>
        {body()}
      </AppFrame>
    ),
  };
}

function auth(id: string, path: string, title: string, body: () => ReactElement): Screen {
  return {
    id,
    path,
    title,
    surface: "plain",
    pattern: pattern(path),
    own: true,
    render: () => <AuthFrame>{body()}</AuthFrame>,
  };
}

const noop = async () => {};

/* ---- states that need more than one line --------------------------------- */

function packBench() {
  return (
    <PackBenchPage
      bench={{
        status: { kind: "ready", screen: PACK_FIXTURE },
        openCarton: PACK_FIXTURE.cartons.find((c) => !c.sealed)?.id ?? null,
        busy: false,
        problem: null,
        dismiss: () => {},
        startCarton: noop,
        addToCarton: noop,
        handOver: noop,
        measure: noop,
        takeOut: noop,
        seal: noop,
        discard: noop,
      }}
    />
  );
}

function despatch() {
  return (
    <DespatchPage
      bench={{
        status: { kind: "ready", screen: DESPATCH_FIXTURE },
        busy: false,
        problem: null,
        // **Booked, so the manifest is a state the gate visits.** Drawn with
        // `booked: null`, the carrier lines could be discarded without
        // anything noticing.
        booked: BOOKED,
        notice: true,
        dismiss: () => {},
        consign: noop,
        despatchCarton: noop,
        despatchAll: noop,
      }}
    />
  );
}

/** What the capture scan can answer, one route each. `identifier_unrecognised`
 *  draws the unknown shape with a different sentence and has no route. */
const scanned = (found: Resolution) =>
  capture(fixtureBench({ kind: "worklist" }, { scan: { typed: "", found, refocus: 0 } }));

/**
 * A carton scanned: the line chosen, the lot and date filled from the label,
 * and six cartons of twelve showing what they come to before the press.
 */
function receivingCounting() {
  const line = RECEIVING_FIXTURE.lines[0];
  return receiving(
    fixtureReceiving(RECEIVING_FIXTURE, {
      bay: THE_DOCK,
      delivery: "01a05bd6-4b60-7450-88cc-2516298acf3f",
      ...(line ? { counting: line } : {}),
      entered: "6",
      level: "carton",
      lotCode: "L2026-021",
      lotExpiry: "2027-01-01",
      owner: "9a247000-0000-0000-0000-000000000001",
      base: 72,
      // **The third witness, corroborating.** Six cartons at 400 g is 2.4 kg
      // and the scale says 2.4: the state a receiver meets most often.
      // Computed by `witness` so the fixture cannot drift from the sentence.
      weighed: "2.400",
      scale: witness(2_400, line?.levels[1]?.baseline ?? null, 6, "carton"),
    }),
  );
}

const pickLine = (i: number) => PICKING_FIXTURE.lines[i];
const putCell = (i: number) => PUTAWAY_FIXTURE.cells[i];
const recvLine = (i: number) => RECEIVING_FIXTURE.lines[i];

/* ---- the table ----------------------------------------------------------- */

/**
 * Every fixture, under `/fixtures`.
 *
 * The paths are the ones the render gate visits. They live outside the screen
 * namespace so that `/pack` means the pack screen with your work on it rather
 * than a picture of somebody else's.
 */
export const FIXTURES: readonly Screen[] = [
  app("f-home", "/fixtures/home", "Dashboard", "bench", { screen: "home" }, () => <Dashboard dash={DASHBOARD} site="MEL" />),
  app("f-home-quiet", "/fixtures/home/quiet", "Dashboard — nothing", "bench", { screen: "home", counts: {} }, () => (
    <Dashboard dash={DASHBOARD_QUIET} site="MEL" />
  )),
  app("f-home-no-site", "/fixtures/home/no-site", "Dashboard — no site", "bench", { screen: "home", counts: {} }, () => (
    <Dashboard dash={DASHBOARD_NO_SITE} site={null} />
  )),
  // The header search's D111 outcomes. A live screen shows one for a moment,
  // and three of the four need a warehouse with the wrong labels in it.
  app("f-scan-ambiguous", "/fixtures/scan/ambiguous", "Scan — ambiguous", "bench", { screen: "home", landing: AMBIGUOUS }, () => (
    <Dashboard dash={DASHBOARD} site="MEL" />
  )),
  app("f-scan-unknown", "/fixtures/scan/unknown", "Scan — unknown", "bench", { screen: "home", landing: UNKNOWN }, () => (
    <Dashboard dash={DASHBOARD} site="MEL" />
  )),
  app("f-scan-smudge", "/fixtures/scan/unrecognised", "Scan — unrecognised", "bench", { screen: "home", landing: UNRECOGNISED }, () => (
    <Dashboard dash={DASHBOARD} site="MEL" />
  )),
  app("f-scan-nowhere", "/fixtures/scan/nowhere", "Scan — no screen", "bench", { screen: "home", landing: NO_SCREEN }, () => (
    <Dashboard dash={DASHBOARD} site="MEL" />
  )),

  app("f-pack", "/fixtures/pack", "Pack order", "bench", { screen: "pack-one" }, packBench),
  app("f-queue", "/fixtures/queue", "Packing", "bench", { screen: "pack" }, () => <PackQueuePage bench={fixtureQueue(QUEUE)} />),
  app("f-queue-clear", "/fixtures/queue/clear", "Packing — nothing waiting", "bench", { screen: "pack" }, () => (
    <PackQueuePage bench={fixtureQueue(CLEAR)} />
  )),
  app("f-despatch", "/fixtures/despatch", "Despatch", "bench", { screen: "despatch" }, despatch),

  // The pick walk: what to pick, where it is, and what it looks like.
  app("f-picking", "/fixtures/picking", "Picking", "floor", { screen: "picking" }, () => picking(fixturePicking())),
  app("f-picking-clear", "/fixtures/picking/clear", "Picking — clear", "floor", { screen: "picking" }, () =>
    picking(fixturePicking(PICKING_CLEAR)),
  ),
  // D166: a forklift order going onto PALLET-A, one line scanned and in hand,
  // and the only primary button being the one that commits it.
  app("f-picking-pallet", "/fixtures/picking/pallet", "Picking — onto a pallet", "floor", { screen: "picking" }, () => {
    const line = pickLine(0);
    return picking(fixturePicking(PICKING_FIXTURE, { destination: ON_A_PALLET, ...(line ? { confirmed: line, quantity: "24" } : {}) }));
  }),
  // The trolley's half of D166: a quantity above what the bin holds, said
  // before the button is pressed rather than after the picker has walked away.
  app("f-picking-short", "/fixtures/picking/short", "Picking — short", "floor", { screen: "picking" }, () => {
    const line = pickLine(2);
    return picking(fixturePicking(PICKING_FIXTURE, { destination: AT_THE_STATION, ...(line ? { confirmed: line, quantity: "40" } : {}) }));
  }),
  app("f-picking-refused", "/fixtures/picking/refused", "Picking — refused", "floor", { screen: "picking" }, () =>
    picking(fixturePicking(PICKING_FIXTURE, { destination: AT_THE_STATION, problem: "GLOVE-L is not on this walk." })),
  ),
  app("f-picking-took", "/fixtures/picking/recorded", "Picking — recorded", "floor", { screen: "picking" }, () =>
    picking(
      fixturePicking(PICKING_FIXTURE, {
        destination: ON_A_PALLET,
        took: { code: "STY-7720-08", quantity: 24, warnings: ["24 taken from a cell holding 60"] },
      }),
    ),
  ),

  // A truck at the dock, with nowhere named yet. Nothing can be counted.
  app("f-receiving", "/fixtures/receiving", "Receiving", "floor", { screen: "receiving" }, () => receiving(fixtureReceiving())),
  app("f-receiving-counting", "/fixtures/receiving/counting", "Receiving — counting", "floor", { screen: "receiving" }, receivingCounting),
  // More arrived than was promised. The fact, not the judgement.
  app("f-receiving-over", "/fixtures/receiving/over", "Receiving — over", "floor", { screen: "receiving" }, () => {
    const line = recvLine(1);
    return receiving(
      fixtureReceiving(RECEIVING_FIXTURE, {
        bay: THE_DOCK,
        ...(line ? { counting: line } : {}),
        entered: "40",
        level: "each",
        owner: "9a247000-0000-0000-0000-000000000001",
        base: 40,
      }),
    );
  }),
  // The promise names no owner, so the line waits for somebody to say.
  app("f-receiving-no-owner", "/fixtures/receiving/no-owner", "Receiving — no owner", "floor", { screen: "receiving" }, () => {
    const line = recvLine(2);
    return receiving(
      fixtureReceiving(RECEIVING_FIXTURE, {
        bay: THE_DOCK,
        ...(line ? { counting: line } : {}),
        entered: "24",
        base: 24,
        missing: "Say whose the goods are.",
      }),
    );
  }),
  // Refused, and not an error: the policy wants a lot and the line carried
  // none. The goods are on the dock either way; the record did not happen.
  app("f-receiving-refused", "/fixtures/receiving/refused", "Receiving — refused", "floor", { screen: "receiving" }, () =>
    receiving(
      fixtureReceiving(RECEIVING_FIXTURE, {
        bay: THE_DOCK,
        delivery: "01a05bd6-4b60-7450-88cc-2516298acf3f",
        landed: {
          code: "GLOVE-M",
          quantity: 0,
          entered: "6 carton",
          accepted: false,
          finding: "d15c0000-0000-0000-0000-000000000009",
          warnings: [],
        },
      }),
    ),
  ),
  // Taken in, with the expiry conflict the lot rule reports rather than settles.
  app("f-receiving-landed", "/fixtures/receiving/taken-in", "Receiving — taken in", "floor", { screen: "receiving" }, () =>
    receiving(
      fixtureReceiving(RECEIVING_FIXTURE, {
        bay: THE_DOCK,
        delivery: "01a05bd6-4b60-7450-88cc-2516298acf3f",
        landed: {
          code: "GLOVE-M",
          quantity: 72,
          entered: "6 carton",
          accepted: true,
          finding: null,
          warnings: ["lot L2026-021 is on file with expiry 2027-01-01; this delivery says 2027-06-30. The held date stands."],
        },
      }),
    ),
  ),
  app("f-receiving-clear", "/fixtures/receiving/clear", "Receiving — clear", "floor", { screen: "receiving" }, () =>
    receiving(fixtureReceiving(RECEIVING_CLEAR, { bay: THE_DOCK })),
  ),

  // Three cells: one with a home to consolidate into, one never stored here,
  // one partly claimed. Nothing is held, so no bin can be chosen yet.
  app("f-putaway", "/fixtures/putaway", "Put away", "floor", { screen: "putaway" }, () => putaway(fixturePutaway())),
  app("f-putaway-holding", "/fixtures/putaway/in-hand", "Put away — in hand", "floor", { screen: "putaway" }, () => {
    const cell = putCell(0);
    return putaway(fixturePutaway(PUTAWAY_FIXTURE, { ...(cell ? { holding: cell, quantity: "120" } : {}), bin: A_BIN }));
  }),
  // Held, but no bin named yet: the button is drawn and refuses, because
  // saying which bin is the act.
  app("f-putaway-no-bin", "/fixtures/putaway/no-bin", "Put away — no bin yet", "floor", { screen: "putaway" }, () => {
    const cell = putCell(1);
    return putaway(fixturePutaway(PUTAWAY_FIXTURE, { ...(cell ? { holding: cell, quantity: "24" } : {}) }));
  }),
  app("f-putaway-refused", "/fixtures/putaway/refused", "Put away — refused", "floor", { screen: "putaway" }, () =>
    putaway(fixturePutaway(PUTAWAY_FIXTURE, { problem: "GLOVE-L is not on the dock." })),
  ),
  app("f-putaway-stowed", "/fixtures/putaway/recorded", "Put away — recorded", "floor", { screen: "putaway" }, () =>
    putaway(fixturePutaway(PUTAWAY_FIXTURE, { stowed: { code: "GLOVE-M", quantity: 120, bin: "A-01-1" } })),
  ),
  app("f-putaway-clear", "/fixtures/putaway/clear", "Put away — clear", "floor", { screen: "putaway" }, () =>
    putaway(fixturePutaway(PUTAWAY_CLEAR)),
  ),

  // The three capture stages, each reachable with no network. A stage no
  // fixture reaches is a stage the render gate cannot check.
  app("f-capture", "/fixtures/capture", "Capture", "floor", { screen: "capture" }, () => capture(fixtureBench({ kind: "worklist" }))),
  app("f-capture-clear", "/fixtures/capture/clear", "Capture — clear", "floor", { screen: "capture" }, () =>
    capture(fixtureBench({ kind: "worklist" }, {}, CAPTURE_CLEAR)),
  ),
  app("f-capture-scanned", "/fixtures/capture/scanned", "Capture — scanned", "floor", { screen: "capture" }, () => scanned(SCAN_RESOLVED)),
  app("f-capture-ambiguous", "/fixtures/capture/ambiguous", "Capture — ambiguous", "floor", { screen: "capture" }, () =>
    scanned(SCAN_AMBIGUOUS),
  ),
  app("f-capture-unknown", "/fixtures/capture/unknown", "Capture — unknown", "floor", { screen: "capture" }, () => scanned(SCAN_UNKNOWN)),
  app("f-capture-figures", "/fixtures/capture/figures", "Capture — figures", "floor", { screen: "capture" }, () =>
    capture(fixtureBench({ kind: "figures", subject: CAPTURE_SUBJECT })),
  ),
  // A single loose thing, which is where D138 and D139 land: a carton reaches
  // neither the arrangement tabs nor "no dimensions", so without this route
  // they would render on no screen the gate visits (D131).
  app("f-capture-each", "/fixtures/capture/each", "Capture — each", "floor", { screen: "capture" }, () =>
    capture(fixtureBench({ kind: "figures", subject: CAPTURE_SUBJECT_EACH })),
  ),
  app("f-capture-faces", "/fixtures/capture/faces", "Capture — faces", "floor", { screen: "capture" }, () =>
    capture(
      fixtureBench(
        { kind: "photographs", subject: CAPTURE_SUBJECT, event: "e0e00000-0000-0000-0000-000000000001" },
        { taken: ["front", "label"], recorded: { measurements: 4, warnings: [] } },
      ),
    ),
  ),
  // What a box already answers to (D164): one binding at the level being
  // captured, one at another, and one from a feed with nobody's name on it.
  app("f-capture-barcodes", "/fixtures/capture/barcodes", "Capture — barcodes", "floor", { screen: "capture" }, () =>
    capture(fixtureBench({ kind: "figures", subject: CAPTURE_SUBJECT }, { barcodes: BOUND_BARCODES, binding: "", count: "" })),
  ),

  app("f-orders-latest", "/fixtures/orders/latest", "Orders", "desk", { screen: "orders" }, () => <OrdersPage desk={fixtureOrders(LATEST)} />),
  app("f-orders", "/fixtures/orders", "Orders — found", "desk", { screen: "orders" }, () => <OrdersPage desk={fixtureOrders(FOUND)} />),
  app("f-orders-superseded", "/fixtures/orders/superseded", "Orders — replaced", "desk", { screen: "orders" }, () => (
    <OrdersPage desk={fixtureOrders(SUPERSEDED)} />
  )),
  app("f-orders-nothing", "/fixtures/orders/nothing", "Orders — nothing", "desk", { screen: "orders" }, () => (
    <OrdersPage desk={fixtureOrders(NOTHING)} />
  )),

  app("f-findings", "/fixtures/findings", "Findings", "desk", { screen: "findings" }, () => <FindingsPage desk={fixtureDesk()} />),
  // Open on a finding that has a pair and two acts available.
  app("f-findings-evidence", "/fixtures/findings/evidence", "Findings — evidence", "desk", { screen: "findings" }, () => (
    <FindingsPage desk={fixtureDesk({ selected: SHORT_PICK, reason: "" })} />
  )),
  // Open on a closed one: who closed it and why, and no acts.
  app("f-findings-closed", "/fixtures/findings/closed", "Findings — closed", "desk", { screen: "findings" }, () => (
    <FindingsPage desk={fixtureDesk({ selected: FINDINGS_FIXTURE.find((f) => f.state === "accepted") ?? null })} />
  )),
  // A deep link to a finding this deployment cannot answer for (D135). The
  // queue is still the queue, and the reason nothing is open is said.
  app("f-findings-missing", "/fixtures/findings/missing", "Findings — not here", "desk", { screen: "findings" }, () => (
    <FindingsPage desk={fixtureDesk({ selected: null, problem: "That finding is not here. It may belong to another company." })} />
  )),
  // Nothing to chase, which is the system working rather than an error.
  app("f-findings-clear", "/fixtures/findings/clear", "Findings — clear", "desk", { screen: "findings" }, () => (
    <FindingsPage desk={fixtureDesk({ status: { kind: "ready", findings: FINDINGS_CLEAR } })} />
  )),

  app("f-weigh", "/fixtures/weigh", "Weigh", "bench", { screen: "weigh" }, () => <WeighPage bench={weighFixture()} />),
  // The reading that disagrees, which is the state this screen exists for.
  app("f-weigh-disagreed", "/fixtures/weigh/disagreed", "Weigh — disagreed", "bench", { screen: "weigh" }, () => (
    <WeighPage bench={weighFixture({ at: 1, recorded: DISAGREED })} />
  )),
  app("f-weigh-clear", "/fixtures/weigh/clear", "Weigh — clear", "bench", { screen: "weigh" }, () => (
    <WeighPage bench={weighFixture({ status: { kind: "ready", queue: WEIGH_CLEAR } })} />
  )),

  auth("f-sign-in", "/fixtures/sign-in", "Sign in", () => <SignInPage bench={fixtureSignIn(ASKING)} />),
  auth("f-sign-in-company", "/fixtures/sign-in/company", "Sign in — which company", () => (
    <SignInPage
      bench={fixtureSignIn(CHOOSE_COMPANY, {
        tenant: "11111111-1111-1111-1111-111111111111",
        problem: "You work for more than one company. Choose which.",
      })}
    />
  )),
  auth("f-sign-in-refused", "/fixtures/sign-in/refused", "Sign in — refused", () => (
    <SignInPage bench={fixtureSignIn(ASKING, { problem: REFUSED })} />
  )),
  // The same question arrived at with a key: carrying on is a second
  // ceremony rather than a password, and says so.
  auth("f-sign-in-company-key", "/fixtures/sign-in/company-key", "Sign in — which company, by key", () => (
    <SignInPage
      bench={fixtureSignIn(CHOOSE_COMPANY, {
        tenant: "11111111-1111-1111-1111-111111111111",
        via: "key",
        credentials: { email: "", password: "" },
        problem: "You work for more than one company. Choose which, and your key will be asked for again.",
      })}
    />
  )),
  // A browser with no `PublicKeyCredential`: no passkey button.
  auth("f-sign-in-no-keys", "/fixtures/sign-in/no-keys", "Sign in — no passkeys here", () => (
    <SignInPage bench={fixtureSignIn(ASKING, { keys: false })} />
  )),

  // The four setup states, none of which a real deployment shows for long.
  auth("f-setup", "/fixtures/setup", "Setup", () => <SetupPage bench={fixtureSetup(NEEDED)} />),
  auth("f-setup-no-token", "/fixtures/setup/no-token", "Setup — no token", () => <SetupPage bench={fixtureSetup(NO_TOKEN)} />),
  auth("f-setup-closed", "/fixtures/setup/closed", "Setup — already set up", () => <SetupPage bench={fixtureSetup(CLOSED)} />),
  auth("f-setup-done", "/fixtures/setup/done", "Setup — done", () => <SetupPage bench={fixtureSetup(DONE)} />),

  // A ceremony needs an authenticator, so a headless render reaches none of
  // these any other way.
  app("f-keys", "/fixtures/keys", "Passkeys", "plain", { screen: "keys" }, () => <KeysPage bench={fixtureKeys(KEYS_READY)} />),
  app("f-keys-none", "/fixtures/keys/none", "Passkeys — none", "plain", { screen: "keys" }, () => <KeysPage bench={fixtureKeys(KEYS_NONE)} />),
  app("f-keys-unsupported", "/fixtures/keys/unsupported", "Passkeys — unsupported", "plain", { screen: "keys" }, () => (
    <KeysPage bench={fixtureKeys(KEYS_NONE, { supported: false })} />
  )),
  app("f-keys-failed", "/fixtures/keys/failed", "Passkeys — unreachable", "plain", { screen: "keys" }, () => (
    <KeysPage bench={fixtureKeys(KEYS_FAILED)} />
  )),

  // The dry-run report is the state worth drawing: it is what a person reads
  // before deciding, and on a live screen it lasts one click.
  app("f-import", "/fixtures/import", "Import", "desk", { screen: "import" }, () => <ImportPage bench={fixtureImport(IMP_IDLE)} />),
  app("f-import-dry", "/fixtures/import/dry", "Import — dry run", "desk", { screen: "import" }, () => <ImportPage bench={fixtureImport(IMP_DRY)} />),
  app("f-import-applied", "/fixtures/import/applied", "Import — applied", "desk", { screen: "import" }, () => (
    <ImportPage bench={fixtureImport(IMP_APPLIED)} />
  )),
  app("f-import-items", "/fixtures/import/items", "Import — item master", "desk", { screen: "import" }, () => (
    <ImportPage bench={fixtureImport(IMP_ITEMS)} />
  )),
  app("f-import-failed", "/fixtures/import/failed", "Import — refused", "desk", { screen: "import" }, () => (
    <ImportPage bench={fixtureImport(IMP_FAILED)} />
  )),

  // Four warehouses, one empty: what the bin import leaves when a site's bins
  // state no type.
  app("f-workspace", "/fixtures/workspace", "Workspace", "desk", { screen: "workspace" }, () => (
    <WorkspacePage bench={fixtureWorkspace(WS_READY)} />
  )),
  app("f-workspace-empty", "/fixtures/workspace/empty", "Workspace — no warehouses", "desk", { screen: "workspace" }, () => (
    <WorkspacePage bench={fixtureWorkspace(WS_EMPTY)} />
  )),
  app("f-workspace-failed", "/fixtures/workspace/failed", "Workspace — unreachable", "desk", { screen: "workspace" }, () => (
    <WorkspacePage bench={fixtureWorkspace(WS_FAILED)} />
  )),

  // Import tokens (D158). The secret shows for seconds on a live screen and
  // never again, so a fixture is the only way it is looked at twice.
  app("f-tokens", "/fixtures/tokens", "Import tokens", "desk", { screen: "tokens" }, () => <TokensPage bench={fixtureTokens(TOKENS_READY)} />),
  app("f-tokens-none", "/fixtures/tokens/none", "Import tokens — none", "desk", { screen: "tokens" }, () => (
    <TokensPage bench={fixtureTokens(TOKENS_NONE)} />
  )),
  app("f-tokens-minted", "/fixtures/tokens/minted", "Import tokens — just minted", "desk", { screen: "tokens" }, () => (
    <TokensPage bench={fixtureTokens(TOKENS_READY, { minted: MINTED })} />
  )),
  app("f-tokens-failed", "/fixtures/tokens/failed", "Import tokens — unreachable", "desk", { screen: "tokens" }, () => (
    <TokensPage bench={fixtureTokens(TOKENS_FAILED)} />
  )),

  app("f-password", "/fixtures/password", "Account", "plain", { screen: "account" }, () => <AccountPage bench={fixturePassword(READY)} />),
  // Two different new passwords: the one judgement the client makes itself,
  // because the server never sees the second copy.
  app("f-password-mismatch", "/fixtures/password/mismatch", "Account — mismatch", "plain", { screen: "account" }, () => (
    <AccountPage
      bench={fixturePassword(READY, {
        draft: { current: "a-current-password", next: "a-new-password", again: "a-nwe-password" },
        mismatched: true,
      })}
    />
  )),
  app("f-password-refused", "/fixtures/password/refused", "Account — refused", "plain", { screen: "account" }, () => (
    <AccountPage bench={fixturePassword(READY, { problem: "that is not the current password" })} />
  )),
  app("f-password-no-session", "/fixtures/password/no-session", "Account — not signed in", "plain", { screen: "account" }, () => (
    <AccountPage bench={fixturePassword(NO_SESSION)} />
  )),
  app("f-password-changed", "/fixtures/password/changed", "Account — changed", "plain", { screen: "account" }, () => (
    <AccountPage bench={fixturePassword(DONE_ENDED)} />
  )),
  app("f-password-changed-alone", "/fixtures/password/changed-alone", "Account — changed, alone", "plain", { screen: "account" }, () => (
    <AccountPage bench={fixturePassword(DONE_ALONE)} />
  )),

  auth("f-where", "/fixtures/where", "Select warehouse", () => <WherePage bench={fixtureWhere(CHOOSE)} />),
  auth("f-where-settled", "/fixtures/where/settled", "Select warehouse — settled", () => <WherePage bench={fixtureWhere(SETTLED)} />),
  auth("f-where-nowhere", "/fixtures/where/nowhere", "Select warehouse — none", () => <WherePage bench={fixtureWhere(NOWHERE)} />),
];
