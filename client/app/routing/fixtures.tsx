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
import type { BenchScreen, Found, SiteRow } from "@domain/types";
import { SCREENS } from "./manifest";
import type { Landing } from "@app/scan/destination";
import { AMBIGUOUS, SEARCHED, NO_SCREEN, UNKNOWN, UNRECOGNISED, fixtureScan } from "@app/scan/fixture";

import { witness } from "@app/measurement/baseline";

import { Dashboard } from "@app/home/Dashboard";
import { DASHBOARD, DASHBOARD_NO_SITE, DASHBOARD_QUIET } from "@app/home/dashboard-fixture";

import { PackBenchPage } from "@app/outbound/pack/PackBenchPage";
import { PACK_FILLING, PACK_FIXTURE, PACK_LOOSE } from "@app/outbound/pack/fixture";
import { SearchPage } from "@app/scan/SearchPage";
import { SEARCH_FAMILY } from "@app/scan/fixture";
import { PackQueuePage } from "@app/outbound/pack/PackQueuePage";
import { CLEAR, QUEUE, fixtureQueue } from "@app/outbound/pack/queue-fixture";

import { DespatchPage } from "@app/outbound/despatch/DespatchPage";
import { BOOKED, DESPATCH_FIXTURE } from "@app/outbound/despatch/fixture";

import { PickingPage, PickingDock } from "@app/outbound/picking/PickingPage";
import { ToPickPage } from "@app/outbound/picking/ToPickPage";
import { fixtureToPick } from "@app/outbound/picking/to-pick-fixture";
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

import { OrdersPage } from "@app/outbound/orders/OrdersPage";
import { OrderPage } from "@app/outbound/orders/OrderPage";
import { FOUND, LATEST, NOTHING, ORDER, ORDER_MISSING, SUPERSEDED, fixtureOrder, fixtureOrders } from "@app/outbound/orders/fixture";
import { BinPage, PlacePage } from "@app/layout/PlacePage";
import { WarehousePage } from "@app/layout/WarehousePage";
import {
  BIN,
  BIN_BACK,
  BIN_UNPLACED,
  BUILDING,
  DRAFTED,
  DRAFTED_BINS,
  DRAFTED_CHOSEN,
  DRAFTED_SITE,
  FOUND_BINS,
  LAID_OUT,
  NO_LAYOUT,
  RACK_C_BINS,
  SHELF,
  UNPLACED_BINS,
  fixtureBin,
  fixturePlace,
  fixtureWarehouse,
} from "@app/layout/fixture";

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
import { PlanEditorPage } from "@app/layout/PlanEditorPage";
import { fixturePlan, fixturePlanChosen, fixturePlanEdited } from "@app/layout/plan-fixture";
import { MapPage } from "@app/layout/MapPage";
import { fixtureMap, fixtureMapChosen, MAP_WALK } from "@app/layout/map-fixture";
import { BackupPage } from "@app/admin/BackupPage";
import { BACKUP_READY, fixtureBackup } from "@app/admin/backup-fixture";
import { PeoplePage } from "@app/admin/PeoplePage";
import { PEOPLE_READY, fixturePeople } from "@app/admin/people-fixture";
import { ItemPage } from "@app/items/ItemPage";
import { MatchDialog, MoveDialog } from "@app/items/ItemProperties";
import {
  BRUSH,
  ITEM,
  ITEM_FLAGGED,
  MEASURED_AS_KIT,
  MISFILED,
  SOLD_BY_BOX,
  CORRECTING_HOLDS,
  SOLD_BY_CARTON,
  SOLD_SINGLY,
  ITEM_UNKNOWN,
  ITEMS_LISTED,
  ITEMS_NONE,
  ITEMS_PAGE,
  LISTS,
  CARTON_MEASURING,
  fixturePhotoQueue,
  TURNED,
  CROPPING,
  MEASURING,
  MEASURING_BUCKET,
  PHOTOGRAPHING,
  PHOTOGRAPHING_NO_BOX,
  RECROPPING,
  WEIGHED_APART,
  fixtureItems,
  fixtureProperties,
} from "@app/items/fixture";
import { ItemsPage, NewList } from "@app/items/ItemsPage";
import { PhotosPage } from "@app/items/PhotosPage";
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
    administrator: true,
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
  /** The search in use: what is typed and what it found (D189). */
  search?: { value: string; results: Found[] };
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
          scan={fixtureScan(frame.landing ?? null, frame.search?.value ?? "", frame.search?.results ?? null)}
          searchOpen={frame.search !== undefined}
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

function packBench(screen: BenchScreen = PACK_FIXTURE) {
  return (
    <PackBenchPage
      bench={{
        status: { kind: "ready", screen },
        openCarton: screen.cartons.find((c) => !c.sealed)?.id ?? null,
        busy: false,
        problem: null,
        dismiss: () => {},
        refresh: () => {},
        startCarton: noop,
        addToCarton: noop,
        handOver: noop,
        shipAsIs: noop,
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
  // D189: the search as it is typed, at a desk and (as "pocket") on a phone.
  app("f-search", "/fixtures/search", "Search — typing", "bench", { screen: "home", search: { value: "30", results: SEARCHED } }, () => (
    <Dashboard dash={DASHBOARD} site="MEL" />
  )),
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

  // Enter in the search on a family's code: its members, not the first of them (D227).
  app("f-search-results", "/fixtures/search-results", "Search — a family", "desk", { screen: "search" }, () => <SearchPage desk={SEARCH_FAMILY} />),
  app("f-pack", "/fixtures/pack", "Pack order", "bench", { screen: "pack-one" }, () => packBench()),
  // A site that has not said where it packs or whose stock it holds (migration
  // 97): the bench says so before anybody tries to start a carton.
  // A small box open with two aprons in it: the suggestion fills it first (D198).
  app("f-pack-filling", "/fixtures/pack/filling", "Pack order — filling a carton", "bench", { screen: "pack-one" }, () =>
    packBench(PACK_FILLING),
  ),
  // Cartons as they came and loose things not measured: what the loose things weigh (D224).
  app("f-pack-loose", "/fixtures/pack/loose", "Pack order — loose things to box by hand", "bench", { screen: "pack-one" }, () =>
    packBench(PACK_LOOSE),
  ),
  app("f-pack-unready", "/fixtures/pack/unready", "Pack order — site not set up", "bench", { screen: "pack-one" }, () =>
    packBench({
      ...PACK_FIXTURE,
      dock_id: null,
      staging_id: null,
      unready: "MEL doesn't say where it packs or who owns its stock yet. Set both in Workspace.",
      cartons: [],
    }),
  ),
  app("f-queue", "/fixtures/queue", "Packing", "bench", { screen: "pack" }, () => <PackQueuePage bench={fixtureQueue(QUEUE, "", 12)} />),
  app("f-queue-clear", "/fixtures/queue/clear", "Packing — nothing waiting", "bench", { screen: "pack" }, () => (
    <PackQueuePage bench={fixtureQueue(CLEAR)} />
  )),
  app("f-despatch", "/fixtures/despatch", "Despatch", "bench", { screen: "despatch" }, despatch),

  // The pick walk: what to pick, where it is, and what it looks like.
  app("f-picking", "/fixtures/picking", "Picking", "floor", { screen: "picking" }, () => picking(fixturePicking())),
  app("f-to-pick", "/fixtures/to-pick", "To pick", "desk", { screen: "to-pick" }, () => <ToPickPage desk={fixtureToPick()} />),
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
  // A dead spot (D207): the last pick is kept on the device, two are waiting,
  // one the server refused when it went, and a colleague's are left behind.
  app("f-picking-offline", "/fixtures/picking/offline", "Picking — no connection", "floor", { screen: "picking" }, () =>
    picking(
      fixturePicking(PICKING_FIXTURE, {
        destination: AT_THE_STATION,
        took: { code: "STY-7720-12", quantity: 2, warnings: [], held: true },
        outbox: {
          waiting: 2,
          refused: [{ key: "r1", code: "GLOVE-M", quantity: 3, where: "PACK-1", reason: "That order was cancelled." }],
          others: [{ name: "Sam Rivera", count: 1 }],
          send: () => {},
          dismiss: () => {},
        },
      }),
    ),
  ),
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

  app("f-orders-latest", "/fixtures/orders/latest", "Orders", "desk", { screen: "orders" }, () => <OrdersPage desk={fixtureOrders(LATEST)} />),
  app("f-orders", "/fixtures/orders", "Orders — found", "desk", { screen: "orders" }, () => <OrdersPage desk={fixtureOrders(FOUND)} />),
  app("f-orders-superseded", "/fixtures/orders/superseded", "Orders — replaced", "desk", { screen: "orders" }, () => (
    <OrdersPage desk={fixtureOrders(SUPERSEDED)} />
  )),
  app("f-orders-nothing", "/fixtures/orders/nothing", "Orders — nothing", "desk", { screen: "orders" }, () => (
    <OrdersPage desk={fixtureOrders(NOTHING)} />
  )),
  app("f-order", "/fixtures/order", "Order", "desk", { screen: "order" }, () => <OrderPage desk={fixtureOrder(ORDER)} />),

  // D173: a scanned bin lands on the face of its rack with its spot lit; a bin
  // not on the layout says so; a building lists what is inside it; a shelf
  // splits its bottom level three ways.
  app("f-bin", "/fixtures/bin", "Bin", "floor", { screen: "bin" }, () => <BinPage desk={fixtureBin(BIN)} />),
  app("f-bin-back", "/fixtures/bin/back", "Bin — back of a rack", "floor", { screen: "bin" }, () => (
    <BinPage desk={fixtureBin(BIN_BACK)} />
  )),
  app("f-bin-unplaced", "/fixtures/bin/unplaced", "Bin — not on the layout", "floor", { screen: "bin" }, () => (
    <BinPage desk={fixtureBin(BIN_UNPLACED)} />
  )),
  app("f-place", "/fixtures/place", "Place", "floor", { screen: "place" }, () => <PlacePage desk={fixturePlace(BUILDING)} />),
  app("f-place-shelf", "/fixtures/place/shelf", "Place — shelf", "floor", { screen: "place" }, () => (
    <PlacePage desk={fixturePlace(SHELF)} />
  )),
  // An item's page: where NetSuite says it is beside Spork's own record, its
  // carton from the family's prepack row; and one nobody has recorded at all.
  // The item list: a page of a long list, and a search that found nothing.
  app("f-items", "/fixtures/items", "Items", "desk", { screen: "items" }, () => <ItemsPage desk={fixtureItems(ITEMS_PAGE)} />),
  // A row open beside the list: its properties, with Previous and Next (D174).
  app("f-items-open", "/fixtures/items/open", "Items — an item open", "desk", { screen: "items" }, () => (
    <ItemsPage
      desk={fixtureItems(ITEMS_PAGE, { stock: "here", needs: "measuring", order: "walk" }, BRUSH)}
      panel={fixtureProperties(ITEM)}
    />
  )),
  app("f-items-none", "/fixtures/items/none", "Items — nothing matches", "desk", { screen: "items" }, () => (
    <ItemsPage desk={fixtureItems(ITEMS_NONE, { q: "SKU-0000", stock: "here", needs: "photo" })} />
  )),
  // D179: a sheet made into a list and worked in its order, on a handheld; and
  // making one, refused for a code nobody knows.
  app("f-items-list", "/fixtures/items/list", "Items — a list", "floor", { screen: "items" }, () => (
    <ItemsPage desk={fixtureItems(ITEMS_LISTED, { list: LISTS[0]!.item_list_id, order: "list" })} />
  )),
  app("f-items-new-list", "/fixtures/items/new-list", "Items — making a list", "desk", { screen: "items" }, () => {
    const desk = fixtureItems(ITEMS_PAGE, {}, null, {
      making: { busy: false, problem: "no item has the code SKU-0000", dismiss: () => {} },
    });
    return (
      <>
        <ItemsPage desk={desk} />
        <NewList desk={desk} onClose={() => {}} />
      </>
    );
  }),
  app("f-item", "/fixtures/item", "Item", "floor", { screen: "item" }, () => <ItemPage desk={fixtureProperties(ITEM)} />),
  // Measuring the each, which says how it was arranged and may say it has no box (D138).
  app("f-item-measuring", "/fixtures/item/measuring", "Item — measuring", "floor", { screen: "item" }, () => (
    <ItemPage desk={MEASURING} />
  )),
  // A reading far from the figure on record raises a finding, and says so.
  app("f-item-weighed", "/fixtures/item/weighed", "Item — weighed apart", "floor", { screen: "item" }, () => (
    <ItemPage desk={WEIGHED_APART} />
  )),
  app("f-item-photographing", "/fixtures/item/photographing", "Item — photographing", "floor", { screen: "item" }, () => (
    <ItemPage desk={PHOTOGRAPHING} />
  )),
  // D176: a photo cut to its face. Straight after taking it on a handheld, and
  // again at a desk, where it was cut before.
  app("f-item-cropping", "/fixtures/item/cropping", "Item — cropping a photo", "floor", { screen: "item" }, () => (
    <ItemPage desk={CROPPING} />
  )),
  app("f-item-recropping", "/fixtures/item/recropping", "Item — cropping again", "desk", { screen: "item" }, () => (
    <ItemPage desk={RECROPPING} />
  )),
  // A thing in a bag is asked for its photo and what else helps, not six sides (D191).
  app("f-item-no-box", "/fixtures/item/no-box", "Item — in a bag", "floor", { screen: "item" }, () => (
    <ItemPage desk={PHOTOGRAPHING_NO_BOX} />
  )),
  // A bucket is measured across, top and base, and by the straight part under its rim (D213).
  app("f-item-bucket", "/fixtures/item/bucket", "Item — measuring a bucket", "floor", { screen: "item" }, () => (
    <ItemPage desk={MEASURING_BUCKET} />
  )),
  // D218: an item leads with what NetSuite counts one of; what is only
  // offered (a carton nobody said, a single product inside) is a quiet line.
  app("f-item-sold-singly", "/fixtures/item/sold-singly", "Item — sold singly", "floor", { screen: "item" }, () => (
    <ItemPage desk={fixtureProperties(SOLD_SINGLY)} />
  )),
  app("f-item-sold-by-carton", "/fixtures/item/sold-by-carton", "Item — sold by the carton", "floor", { screen: "item" }, () => (
    <ItemPage desk={fixtureProperties(SOLD_BY_CARTON)} />
  )),
  app("f-item-sold-by-box", "/fixtures/item/sold-by-box", "Item — sold by the box", "desk", { screen: "item" }, () => (
    <ItemPage desk={fixtureProperties(SOLD_BY_BOX)} />
  )),
  // D219: a box recorded on its carton's card, and moving it to the box.
  app("f-item-misfiled", "/fixtures/item/misfiled", "Item — recorded on the wrong card", "floor", { screen: "item" }, () => (
    <ItemPage desk={fixtureProperties(MISFILED)} />
  )),
  app("f-item-moving", "/fixtures/item/moving", "Item — moving to the right card", "floor", { screen: "item" }, () => {
    const desk = fixtureProperties(MISFILED);
    const carton = MISFILED.subjects.find((x) => x.packaging_level === "carton")!;
    return (
      <>
        <ItemPage desk={desk} />
        <MoveDialog item={MISFILED} subject={carton} desk={desk} onClose={() => {}} />
      </>
    );
  }),
  // D222: a kit measured as though it were its part, moving to the part's own item.
  app("f-item-moving-elsewhere", "/fixtures/item/moving-elsewhere", "Item — moving to another item", "floor", { screen: "item" }, () => {
    const desk = fixtureProperties(MEASURED_AS_KIT);
    return (
      <>
        <ItemPage desk={desk} />
        <MoveDialog item={MEASURED_AS_KIT} subject={MEASURED_AS_KIT.subjects[0]!} desk={desk} onClose={() => {}} />
      </>
    );
  }),
  // D229: a carton said wrongly, put right from its Holds.
  app("f-item-holds", "/fixtures/item/holds", "Item — a carton said wrongly", "floor", { screen: "item" }, () => (
    <ItemPage desk={CORRECTING_HOLDS} />
  )),
  // D228: its family, and matching it from the green one.
  app("f-item-matching", "/fixtures/item/matching", "Item — matching from its family", "floor", { screen: "item" }, () => {
    const desk = fixtureProperties(ITEM);
    return (
      <>
        <ItemPage desk={desk} />
        <MatchDialog item={ITEM} from={ITEM.family[0]!} desk={desk} onClose={() => {}} />
      </>
    );
  }),
  // D215: what the floor says against NetSuite's bins, and the two questions.
  app("f-item-flagged", "/fixtures/item/flagged", "Item — not where NetSuite lists it", "floor", { screen: "item" }, () => (
    <ItemPage desk={fixtureProperties(ITEM_FLAGGED)} />
  )),
  app("f-item-not-here", "/fixtures/item/not-here", "Item — not in a listed bin", "floor", { screen: "item" }, () => (
    <ItemPage desk={fixtureProperties(ITEM)} asking={{ said: "not_here", row: ITEM.reported[0]! }} />
  )),
  app("f-item-found-here", "/fixtures/item/found-here", "Item — found in a bin", "floor", { screen: "item" }, () => (
    <ItemPage desk={fixtureProperties(ITEM_UNKNOWN)} asking={{ said: "found_here" }} />
  )),
  app("f-item-unknown", "/fixtures/item/unknown", "Item — nothing recorded", "floor", { screen: "item" }, () => (
    <ItemPage desk={fixtureProperties(ITEM_UNKNOWN)} />
  )),
  // D181: a phone's photographs, their faces found at a computer and checked;
  // and the queue when every photo has been cut.
  app("f-photos", "/fixtures/photos", "Photos to crop", "desk", { screen: "photos" }, () => <PhotosPage desk={fixturePhotoQueue()} />),
  // D214: a face found a quarter turn out, open to be turned before it is kept.
  app("f-photos-turned", "/fixtures/photos/turned", "Photos to crop — a quarter turn out", "desk", { screen: "photos" }, () => (
    <PhotosPage desk={fixturePhotoQueue({ adjusting: TURNED })} />
  )),
  app("f-photos-none", "/fixtures/photos/none", "Photos to crop — none", "desk", { screen: "photos" }, () => (
    <PhotosPage desk={fixturePhotoQueue({ queued: [] })} />
  )),
  // D178: its own carton measured, and how many it holds said with it.
  app("f-item-carton", "/fixtures/item/carton", "Item — measuring its carton", "floor", { screen: "item" }, () => (
    <ItemPage desk={CARTON_MEASURING} />
  )),
  // The warehouse: none yet, its draft previewed (and adjusted: a lone code's
  // "rack" left out, three racks with two sides), a rack chosen with its
  // bins, the tray of bins no pattern fits, a search across the site, and a
  // big site just after its first draft was applied.
  // The plan editor (D209): as read, and with a rack moved and turned and a wall drawn.
  app("f-plan", "/fixtures/warehouse/edit", "Edit layout", "desk", { screen: "plan" }, () => <PlanEditorPage desk={fixturePlan()} />),
  app("f-plan-edited", "/fixtures/warehouse/edit/changed", "Edit layout — changed", "desk", { screen: "plan" }, () => (
    <PlanEditorPage desk={fixturePlanEdited()} />
  )),
  app("f-plan-several", "/fixtures/warehouse/edit/several", "Edit layout — several chosen", "desk", { screen: "plan" }, () => (
    <PlanEditorPage desk={fixturePlanChosen()} />
  )),
  // The bin map (D208): the site, a bin chosen with its card, and the reach layer.
  app("f-map", "/fixtures/map", "Bin map", "desk", { screen: "map" }, () => <MapPage desk={fixtureMap()} />),
  app("f-map-chosen", "/fixtures/map/chosen", "Bin map — a bin chosen", "desk", { screen: "map" }, () => <MapPage desk={fixtureMapChosen()} />),
  app("f-map-reach", "/fixtures/map/reach", "Bin map — reach", "desk", { screen: "map" }, () => (
    <MapPage desk={fixtureMapChosen({ layer: "reach" })} />
  )),
  app("f-map-walk", "/fixtures/map/walk", "Bin map — today's walk", "desk", { screen: "map" }, () => (
    <MapPage desk={fixtureMap({ showWalk: true, walk: { kind: "ready", value: MAP_WALK } })} />
  )),
  app("f-warehouse-none", "/fixtures/warehouse/none", "Warehouse — none yet", "desk", { screen: "warehouse" }, () => (
    <WarehousePage desk={fixtureWarehouse(NO_LAYOUT)} />
  )),
  app("f-warehouse-draft", "/fixtures/warehouse/draft", "Warehouse — draft", "desk", { screen: "warehouse" }, () => (
    <WarehousePage desk={fixtureWarehouse(NO_LAYOUT, { draft: { kind: "previewed", report: DRAFTED } })} />
  )),
  app("f-warehouse-draft-adjusted", "/fixtures/warehouse/draft/adjusted", "Warehouse — draft, adjusted", "desk", { screen: "warehouse" }, () => (
    <WarehousePage
      desk={fixtureWarehouse(NO_LAYOUT, {
        draft: { kind: "previewed", report: DRAFTED },
        leftOut: ["Rack X"],
        twoSided: ["Rack A", "Rack B", "Rack C"],
        fromRight: ["Rack A", "Rack B", "Rack C"],
      })}
    />
  )),
  app("f-warehouse", "/fixtures/warehouse", "Warehouse", "desk", { screen: "warehouse" }, () => (
    <WarehousePage desk={fixtureWarehouse(LAID_OUT, { chosen: LAID_OUT.places[1]!.place_id, bins: RACK_C_BINS })} />
  )),
  app("f-warehouse-unplaced", "/fixtures/warehouse/unplaced", "Warehouse — not on the layout", "desk", { screen: "warehouse" }, () => (
    <WarehousePage desk={fixtureWarehouse(LAID_OUT, { chosen: "unplaced", bins: UNPLACED_BINS })} />
  )),
  app("f-warehouse-found", "/fixtures/warehouse/found", "Warehouse — search", "desk", { screen: "warehouse" }, () => (
    <WarehousePage desk={fixtureWarehouse(LAID_OUT, { chosen: LAID_OUT.places[1]!.place_id, bins: FOUND_BINS, asked: "01" })} />
  )),
  app("f-warehouse-drafted", "/fixtures/warehouse/drafted", "Warehouse — first draft", "desk", { screen: "warehouse" }, () => (
    <WarehousePage desk={fixtureWarehouse(DRAFTED_SITE, { chosen: DRAFTED_CHOSEN, bins: DRAFTED_BINS })} />
  )),
  app("f-order-missing", "/fixtures/order/missing", "Order — missing", "desk", { screen: "order" }, () => (
    <OrderPage desk={fixtureOrder(ORDER_MISSING)} />
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
  // The workspace's people, one of whom has left, and the line after an add (D205).
  app("f-people", "/fixtures/people", "People", "desk", { screen: "people" }, () => <PeoplePage bench={fixturePeople(PEOPLE_READY)} />),
  app("f-people-added", "/fixtures/people/added", "People — added", "desk", { screen: "people" }, () => (
    <PeoplePage bench={fixturePeople(PEOPLE_READY, { said: "Sam Rivera can sign in now with that password, and change it under Account." })} />
  )),
  // A backup of the whole workspace, and the moment after it was asked for (D193).
  app("f-backup", "/fixtures/backup", "Backup", "desk", { screen: "backup" }, () => <BackupPage bench={fixtureBackup(BACKUP_READY)} />),
  app("f-backup-asked", "/fixtures/backup/asked", "Backup — downloading", "desk", { screen: "backup" }, () => (
    <BackupPage bench={fixtureBackup(BACKUP_READY, { asked: true })} />
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
