/**
 * Every state the design system can draw, reachable with no network.
 *
 * **Review build only.** These are behind `import.meta.env.MODE === "review"`
 * in `main.tsx`, so Rollup drops the branch and this whole module from the
 * production bundle. A deployment cannot reach invented data by typing a URL,
 * and the render gate still visits every one of them.
 *
 * D131 is unchanged by that: *"the states the design system draws are reachable
 * from a fixture"* stays true, because the gate builds in review mode. What
 * changes is only that a customer cannot.
 */

import "@design/tokens.css";
import "@design/layers.css";

import { witness } from "@app/measurement/baseline";
import { BenchShell } from "@app/shells/BenchShell";
import { PackBench } from "@app/outbound/pack/PackBench";
import { PACK_FIXTURE } from "@app/outbound/pack/fixture";

import { Despatch } from "@app/outbound/despatch/Despatch";
import { BOOKED, DESPATCH_FIXTURE } from "@app/outbound/despatch/fixture";

import { FloorShell } from "@app/shells/FloorShell";
import { Capture, CaptureDock } from "@app/measurement/capture/Capture";
import { BOUND_BARCODES, CAPTURE_CLEAR, CAPTURE_SUBJECT, CAPTURE_SUBJECT_EACH, SCAN_AMBIGUOUS, SCAN_RESOLVED, SCAN_UNKNOWN, fixtureBench } from "@app/measurement/capture/fixture";

import type { Resolution } from "@domain/types";
import { PickList, PickDock } from "@app/outbound/picking/PickList";
import { Putaway, PutawayDock } from "@app/inbound/putaway/Putaway";
import { Receiving, ReceivingDock } from "@app/inbound/receiving/Receiving";
import {
  RECEIVING_CLEAR,
  RECEIVING_FIXTURE,
  THE_DOCK,
  fixtureReceiving,
} from "@app/inbound/receiving/fixture";
import {
  A_BIN,
  PUTAWAY_CLEAR,
  PUTAWAY_FIXTURE,
  fixturePutaway,
} from "@app/inbound/putaway/fixture";
import {
  AT_THE_STATION,
  ON_A_PALLET,
  PICKING_CLEAR,
  PICKING_FIXTURE,
  fixturePicking,
} from "@app/outbound/picking/fixture";

import { DeskShell } from "@app/shells/DeskShell";
import { Findings, FindingsRail } from "@app/integrity/findings/Findings";
import { FINDINGS_CLEAR, FINDINGS_FIXTURE, SHORT_PICK, fixtureDesk } from "@app/integrity/findings/fixture";

import { Weigh } from "@app/measurement/weigh/Weigh";
import { DISAGREED, WEIGH_CLEAR, fixtureBench as weighFixture } from "@app/measurement/weigh/fixture";

import { PlainShell } from "@app/shells/PlainShell";
import { Setup } from "@app/setup/Setup";
import { CLOSED, DONE, NEEDED, NO_TOKEN, fixtureSetup } from "@app/setup/fixture";

import { Where } from "@app/session/Where";

import { CHOOSE, NOWHERE, SETTLED, fixtureWhere } from "@app/session/fixture";
import { Password } from "@app/account/Password";
import { Keys } from "@app/account/Keys";
import { FAILED as KEYS_FAILED, NONE as KEYS_NONE, READY as KEYS_READY, fixtureKeys } from "@app/account/keys-fixture";
import { Tokens } from "@app/admin/Tokens";
import { Import } from "@app/admin/Import";
import { APPLIED as IMP_APPLIED, DRY as IMP_DRY, FAILED as IMP_FAILED, IDLE as IMP_IDLE, ITEMS as IMP_ITEMS, fixtureImport } from "@app/admin/import-fixture";
import { Workspace } from "@app/admin/Workspace";
import { EMPTY as WS_EMPTY, FAILED as WS_FAILED, READY as WS_READY, fixtureWorkspace } from "@app/admin/workspace-fixture";
import { FAILED as TOKENS_FAILED, MINTED, NONE as TOKENS_NONE, READY as TOKENS_READY, fixtureTokens } from "@app/admin/fixture";
import { DONE_ALONE, DONE_ENDED, NO_SESSION, READY, fixturePassword } from "@app/account/fixture";

import { pattern } from "@domain/routing";
import { Home } from "@app/home/Home";
import { BUSY, NO_SITE, QUIET, fixtureHome } from "@app/home/fixture";
import { WorkRail } from "@app/nav/WorkRail";
import { Locator } from "@app/scan/Locator";
import { SignIn } from "@app/session/SignIn";
import { ASKING, CHOOSE_COMPANY, REFUSED, fixtureSignIn } from "@app/session/signin-fixture";
import { Orders } from "@app/outbound/orders/Orders";
import { PackQueue } from "@app/outbound/pack/PackQueue";
import { CLEAR, QUEUE, fixtureQueue } from "@app/outbound/pack/queue-fixture";
import { FOUND, LATEST, NOTHING, SUPERSEDED, fixtureOrders } from "@app/outbound/orders/fixture";
import { AMBIGUOUS, NO_SCREEN, UNKNOWN, UNRECOGNISED, fixtureScan } from "@app/scan/fixture";
import type { Screen } from "./Router";

function whereRoute(title: string, state: Parameters<typeof fixtureWhere>[0]) {
  return function WhereFixture() {
    return (
      <PlainShell title={title}>
        <Where bench={fixtureWhere(state)} />
      </PlainShell>
    );
  };
}

const FixtureWhere = whereRoute("Where — choose", CHOOSE);
const FixtureWhereSettled = whereRoute("Where — settled", SETTLED);
const FixtureWhereNowhere = whereRoute("Where — no site", NOWHERE);

const noop = async () => {};

function FixturePack() {
  return (
    <BenchShell title="Pack — fixture" site="MEL" who="d.stooke">
      <PackBench
        bench={{
          status: { kind: "ready", screen: PACK_FIXTURE },
          openCarton: PACK_FIXTURE.cartons.find((c) => !c.sealed)?.id ?? null,
          busy: false,
          problem: null,
          dismiss: () => {},
          startCarton: noop,
          addToCarton: noop,
          measure: noop,
          takeOut: noop,
          seal: noop,
          discard: noop,
        }}
      />
    </BenchShell>
  );
}

function FixtureDespatch() {
  return (
    <BenchShell title="Despatch — fixture" site="MEL" who="d.stooke">
      <Despatch
        bench={{
          status: { kind: "ready", screen: DESPATCH_FIXTURE },
          busy: false,
          problem: null,
          // **Booked, so the manifest is a state the gate visits.** The screen
          // spent its whole life drawing `booked: null` here, which is why the
          // carrier lines could be discarded without anything noticing.
          booked: BOOKED,
          notice: true,
          dismiss: () => {},
          consign: noop,
          despatchCarton: noop,
          despatchAll: noop,
        }}
      />
    </BenchShell>
  );
}

/** The three stages, each reachable with no network. A stage no fixture
 *  reaches is a stage the render gate cannot check. */
function FixtureCapture() {
  const bench = fixtureBench({ kind: "worklist" });
  return (
    <FloorShell
      title="Capture — fixture"
      site="MEL"
      who="d.stooke"
      dock={<CaptureDock bench={bench} />}
    >
      <Capture bench={bench} />
    </FloorShell>
  );
}

/** Nothing to capture: three drawn absences and the pill that says so. */
function FixtureCaptureClear() {
  const bench = fixtureBench({ kind: "worklist" }, {}, CAPTURE_CLEAR);
  return (
    <FloorShell
      title="Capture — clear"
      site="MEL"
      who="d.stooke"
      dock={<CaptureDock bench={bench} />}
    >
      <Capture bench={bench} />
    </FloorShell>
  );
}

/** What the locator can answer, one route each. Three of the four outcomes:
 *  `identifier_unrecognised` draws the unknown shape with a different sentence
 *  and does not earn a route of its own. */
function scanRoute(title: string, found: Resolution) {
  return function ScanFixture() {
    const bench = fixtureBench(
      { kind: "worklist" },
      { scan: { typed: "", found, refocus: 0 } },
    );
    return (
      <FloorShell title={title} site="MEL" who="d.stooke" dock={<CaptureDock bench={bench} />}>
        <Capture bench={bench} />
      </FloorShell>
    );
  };
}

const FixtureScanResolved = scanRoute("Capture — scanned", SCAN_RESOLVED);
const FixtureScanAmbiguous = scanRoute("Capture — ambiguous", SCAN_AMBIGUOUS);
const FixtureScanUnknown = scanRoute("Capture — unknown", SCAN_UNKNOWN);

function FixtureCaptureFigures() {
  const bench = fixtureBench({ kind: "figures", subject: CAPTURE_SUBJECT });
  return (
    <FloorShell
      title="Capture — figures"
      site="MEL"
      who="d.stooke"
      dock={<CaptureDock bench={bench} />}
    >
      <Capture bench={bench} />
    </FloorShell>
  );
}

/**
 * The figures stage against a single loose thing, which is where D138 and D139
 * both land.
 *
 * A carton reaches neither new control — it is rigid, so there is one
 * arrangement and no question, and it has a box by definition — so without a
 * second route the presentation tabs and the *it has no dimensions* answer
 * would render on no screen the gate visits. That is the rule D131 exists for.
 */
function FixtureCaptureEach() {
  const bench = fixtureBench({ kind: "figures", subject: CAPTURE_SUBJECT_EACH });
  return (
    <FloorShell
      title="Capture — each"
      site="MEL"
      who="d.stooke"
      dock={<CaptureDock bench={bench} />}
    >
      <Capture bench={bench} />
    </FloorShell>
  );
}

/**
 * What a box already answers to, and the field that binds another (D164).
 *
 * The state worth a route of its own: three bindings, one at the level being
 * captured and one at another, and one written before D164 that does not say
 * which — with nobody's name on it, because it came from a feed. That last row
 * is what the column exists to stop being created and what a real catalogue is
 * full of.
 */
function FixtureCaptureBarcodes() {
  return (
    <FloorShell title="Capture — barcodes" site="MEL" who="d.stooke" locator={undefined}>
      <Capture
        bench={fixtureBench(
          { kind: "figures", subject: CAPTURE_SUBJECT },
          { barcodes: BOUND_BARCODES, binding: "", count: "" },
        )}
      />
    </FloorShell>
  );
}

function FixtureCaptureFaces() {
  const bench = fixtureBench(
    {
      kind: "photographs",
      subject: CAPTURE_SUBJECT,
      event: "e0e00000-0000-0000-0000-000000000001",
    },
    {
      taken: ["front", "label"],
      recorded: { measurements: 4, warnings: [] },
    },
  );
  return (
    <FloorShell
      title="Capture — faces"
      site="MEL"
      who="d.stooke"
      dock={<CaptureDock bench={bench} />}
    >
      <Capture bench={bench} />
    </FloorShell>
  );
}

/** The queue, with nothing selected. */
function FixtureFindings() {
  const desk = fixtureDesk();
  return (
    <DeskShell title="Findings — fixture" site="MEL" who="d.stooke">
      <Findings desk={desk} />
    </DeskShell>
  );
}

/** The rail open on a finding that has a pair and two acts available. */
function FixtureFindingsPanel() {
  const desk = fixtureDesk({ selected: SHORT_PICK, reason: "" });
  return (
    <DeskShell
      title="Findings — evidence"
      site="MEL"
      who="d.stooke"
      badge={{ label: "weigh", count: 12 }}
      evidence={<FindingsRail desk={desk} />}
    >
      <Findings desk={desk} />
    </DeskShell>
  );
}

/** The rail open on a closed one: who closed it and why, and no acts. */
function FixtureFindingsClosed() {
  const closed = FINDINGS_FIXTURE.find((f) => f.state === "accepted");
  const desk = fixtureDesk({ selected: closed ?? null });
  return (
    <DeskShell
      title="Findings — closed"
      site="MEL"
      who="d.stooke"
      evidence={<FindingsRail desk={desk} />}
    >
      <Findings desk={desk} />
    </DeskShell>
  );
}

/**
 * A link to a finding this deployment cannot answer for (D135).
 *
 * The state a deep link adds: somebody was sent `/findings/{id}` for a finding
 * that belongs to another company, or that came from a different deployment.
 * The queue is still the queue — there is no reason to lose it — and the reason
 * the panel is not open is said rather than left to be guessed at.
 */
function FixtureFindingsMissing() {
  const desk = fixtureDesk({
    selected: null,
    problem: "That finding is not here. It may belong to another company.",
  });
  return (
    <DeskShell title="Findings — not here" site="MEL" who="d.stooke">
      <Findings desk={desk} />
    </DeskShell>
  );
}

/** Nothing to chase, which is the system working rather than an error. */
function FixtureFindingsClear() {
  const desk = fixtureDesk({ status: { kind: "ready", findings: FINDINGS_CLEAR } });
  return (
    <DeskShell title="Findings — clear" site="MEL" who="d.stooke">
      <Findings desk={desk} />
    </DeskShell>
  );
}

function FixtureWeigh() {
  return (
    <BenchShell title="Weigh — fixture" site="MEL" who="d.stooke">
      <Weigh bench={weighFixture()} />
    </BenchShell>
  );
}

/** The reading that disagrees, which is the state this screen exists for. */
function FixtureWeighDisagreed() {
  return (
    <BenchShell title="Weigh — disagreed" site="MEL" who="d.stooke">
      <Weigh bench={weighFixture({ at: 1, recorded: DISAGREED })} />
    </BenchShell>
  );
}

/** Nothing waiting: every weight measured and in date. */
function FixtureWeighClear() {
  return (
    <BenchShell title="Weigh — clear" site="MEL" who="d.stooke">
      <Weigh bench={weighFixture({ status: { kind: "ready", queue: WEIGH_CLEAR } })} />
    </BenchShell>
  );
}

/**
 * The pick walk: what to pick, where it is, and what it looks like.
 *
 * A Floor screen, so the handheld's density and viewport. It is the first
 * screen here with no lit key, because the one thing it does is be read while
 * somebody walks — recording the pick waits on the carton question
 * `crate::picking_list` declines to answer.
 */
function FixturePicking() {
  const bench = fixturePicking();
  return (
    <FloorShell title="Picking" site="MEL" who="d.stooke" dock={<PickDock bench={bench} />}>
      <PickList bench={bench} />
    </FloorShell>
  );
}

/**
 * Picking onto a pallet, with a row confirmed and the dock holding the act.
 *
 * The state D166 bought: a forklift order going straight onto `PALLET-A`, one
 * line scanned and in the picker's hand, and the only lit key on the screen
 * being the one that commits it.
 */
function FixturePickingOnPallet() {
  const line = PICKING_FIXTURE.lines[0];
  const bench = fixturePicking(PICKING_FIXTURE, {
    destination: ON_A_PALLET,
    ...(line ? { confirmed: line, quantity: "24" } : {}),
  });
  return (
    <FloorShell title="Picking — onto a pallet" site="MEL" who="d.stooke" dock={<PickDock bench={bench} />}>
      <PickList bench={bench} />
    </FloorShell>
  );
}

/**
 * The trolley's half of D166: goods put down at the packing station, and a
 * quantity above what the bin holds — said before the key is pressed rather
 * than after the picker has walked away from the shelf.
 */
function FixturePickingShort() {
  const line = PICKING_FIXTURE.lines[2];
  const bench = fixturePicking(PICKING_FIXTURE, {
    destination: AT_THE_STATION,
    ...(line ? { confirmed: line, quantity: "40" } : {}),
  });
  return (
    <FloorShell title="Picking — short" site="MEL" who="d.stooke" dock={<PickDock bench={bench} />}>
      <PickList bench={bench} />
    </FloorShell>
  );
}

/** A scan that named nothing on the walk. The refusal, in one notice. */
function FixturePickingRefused() {
  const bench = fixturePicking(PICKING_FIXTURE, {
    destination: AT_THE_STATION,
    problem: "GLOVE-L is not on this walk.",
  });
  return (
    <FloorShell title="Picking — refused" site="MEL" who="d.stooke" dock={<PickDock bench={bench} />}>
      <PickList bench={bench} />
    </FloorShell>
  );
}

/** What a pick says once it has landed, warnings and all. */
function FixturePickingTook() {
  const bench = fixturePicking(PICKING_FIXTURE, {
    destination: ON_A_PALLET,
    took: { code: "STY-7720-08", quantity: 24, warnings: ["24 taken from a cell holding 60"] },
  });
  return (
    <FloorShell title="Picking — recorded" site="MEL" who="d.stooke" dock={<PickDock bench={bench} />}>
      <PickList bench={bench} />
    </FloorShell>
  );
}

/** A truck at the dock, with nowhere named yet. Nothing can be counted. */
function FixtureReceiving() {
  const bench = fixtureReceiving();
  return (
    <FloorShell title="Receiving" site="MEL" who="d.stooke" dock={<ReceivingDock bench={bench} />}>
      <Receiving bench={bench} />
    </FloorShell>
  );
}

/**
 * A carton scanned: the line chosen, the lot and the date filled from the label,
 * and six cartons of twelve showing what they come to before the press.
 */
function FixtureReceivingCounting() {
  const line = RECEIVING_FIXTURE.lines[0];
  const bench = fixtureReceiving(RECEIVING_FIXTURE, {
    bay: THE_DOCK,
    delivery: "01a05bd6-4b60-7450-88cc-2516298acf3f",
    ...(line ? { counting: line } : {}),
    entered: "6",
    level: "carton",
    lotCode: "L2026-021",
    lotExpiry: "2027-01-01",
    owner: "9a247000-0000-0000-0000-000000000001",
    base: 72,
    // **The third witness, corroborating.** Six cartons at 400 g apiece is
    // 2.4 kg and the scale says 2.4, so the count and the scale agree — which
    // is the state a receiver meets most often and the one the wording has to
    // read well in. Computed by `witness` rather than typed out, so the
    // fixture cannot drift from the function that produces the sentence.
    weighed: "2.400",
    scale: witness(2_400, line?.levels[1]?.baseline ?? null, 6, "carton"),
  });
  return (
    <FloorShell title="Receiving — counting" site="MEL" who="d.stooke" dock={<ReceivingDock bench={bench} />}>
      <Receiving bench={bench} />
    </FloorShell>
  );
}

/** More arrived than was promised. The fact, not the judgement. */
function FixtureReceivingOver() {
  const line = RECEIVING_FIXTURE.lines[1];
  const bench = fixtureReceiving(RECEIVING_FIXTURE, {
    bay: THE_DOCK,
    ...(line ? { counting: line } : {}),
    entered: "40",
    level: "each",
    owner: "9a247000-0000-0000-0000-000000000001",
    base: 40,
  });
  return (
    <FloorShell title="Receiving — over" site="MEL" who="d.stooke" dock={<ReceivingDock bench={bench} />}>
      <Receiving bench={bench} />
    </FloorShell>
  );
}

/** The promise names no owner, so the line waits for somebody to say. */
function FixtureReceivingNoOwner() {
  const line = RECEIVING_FIXTURE.lines[2];
  const bench = fixtureReceiving(RECEIVING_FIXTURE, {
    bay: THE_DOCK,
    ...(line ? { counting: line } : {}),
    entered: "24",
    base: 24,
    missing: "Say whose the goods are.",
  });
  return (
    <FloorShell title="Receiving — no owner" site="MEL" who="d.stooke" dock={<ReceivingDock bench={bench} />}>
      <Receiving bench={bench} />
    </FloorShell>
  );
}

/**
 * **Refused, and not an error.** The policy wants a lot for this item and the
 * line carried none. The goods are on the dock either way; what did not happen
 * is the record.
 */
function FixtureReceivingRefused() {
  const bench = fixtureReceiving(RECEIVING_FIXTURE, {
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
  });
  return (
    <FloorShell title="Receiving — refused" site="MEL" who="d.stooke" dock={<ReceivingDock bench={bench} />}>
      <Receiving bench={bench} />
    </FloorShell>
  );
}

/** Taken in, with the expiry conflict the lot rule reports rather than settles. */
function FixtureReceivingLanded() {
  const bench = fixtureReceiving(RECEIVING_FIXTURE, {
    bay: THE_DOCK,
    delivery: "01a05bd6-4b60-7450-88cc-2516298acf3f",
    landed: {
      code: "GLOVE-M",
      quantity: 72,
      entered: "6 carton",
      accepted: true,
      finding: null,
      warnings: [
        "lot L2026-021 is on file with expiry 2027-01-01; this delivery says 2027-06-30. The held date stands.",
      ],
    },
  });
  return (
    <FloorShell title="Receiving — taken in" site="MEL" who="d.stooke" dock={<ReceivingDock bench={bench} />}>
      <Receiving bench={bench} />
    </FloorShell>
  );
}

/** Nothing expected: everything promised to this site has arrived. */
function FixtureReceivingClear() {
  const bench = fixtureReceiving(RECEIVING_CLEAR, { bay: THE_DOCK });
  return (
    <FloorShell title="Receiving — clear" site="MEL" who="d.stooke" dock={<ReceivingDock bench={bench} />}>
      <Receiving bench={bench} />
    </FloorShell>
  );
}

/**
 * The dock, with nothing in anybody's hands yet.
 *
 * Three cells: one with a home to consolidate into, one never stored here, one
 * partly claimed. The `homes` keys are dead until something is held, because
 * naming a bin before there are goods to put in it names nothing.
 */
function FixturePutaway() {
  const bench = fixturePutaway();
  return (
    <FloorShell title="Put away" site="MEL" who="d.stooke" dock={<PutawayDock bench={bench} />}>
      <Putaway bench={bench} />
    </FloorShell>
  );
}

/** Goods in hand, a bin scanned, and the one lit key that commits it. */
function FixturePutawayHolding() {
  const cell = PUTAWAY_FIXTURE.cells[0];
  const bench = fixturePutaway(PUTAWAY_FIXTURE, {
    ...(cell ? { holding: cell, quantity: "120" } : {}),
    bin: A_BIN,
  });
  return (
    <FloorShell title="Put away — in hand" site="MEL" who="d.stooke" dock={<PutawayDock bench={bench} />}>
      <Putaway bench={bench} />
    </FloorShell>
  );
}

/**
 * Held, but no bin named yet. The state the whole screen is built around: the
 * key is drawn and refuses to be pressed, because saying which bin is the act.
 */
function FixturePutawayNoBin() {
  const cell = PUTAWAY_FIXTURE.cells[1];
  const bench = fixturePutaway(PUTAWAY_FIXTURE, {
    ...(cell ? { holding: cell, quantity: "24" } : {}),
  });
  return (
    <FloorShell title="Put away — no bin yet" site="MEL" who="d.stooke" dock={<PutawayDock bench={bench} />}>
      <Putaway bench={bench} />
    </FloorShell>
  );
}

/** A scan that named nothing on the dock. */
function FixturePutawayRefused() {
  const bench = fixturePutaway(PUTAWAY_FIXTURE, {
    problem: "GLOVE-L is not on the dock.",
  });
  return (
    <FloorShell title="Put away — refused" site="MEL" who="d.stooke" dock={<PutawayDock bench={bench} />}>
      <Putaway bench={bench} />
    </FloorShell>
  );
}

/** What it says once the goods have a home. */
function FixturePutawayStowed() {
  const bench = fixturePutaway(PUTAWAY_FIXTURE, {
    stowed: { code: "GLOVE-M", quantity: 120, bin: "A-01-1" },
  });
  return (
    <FloorShell title="Put away — recorded" site="MEL" who="d.stooke" dock={<PutawayDock bench={bench} />}>
      <Putaway bench={bench} />
    </FloorShell>
  );
}

/** A clear dock. Everything that arrived has a home, which is the good state. */
function FixturePutawayClear() {
  const bench = fixturePutaway(PUTAWAY_CLEAR);
  return (
    <FloorShell title="Put away — clear" site="MEL" who="d.stooke" dock={<PutawayDock bench={bench} />}>
      <Putaway bench={bench} />
    </FloorShell>
  );
}

/** Nothing to pick: the drawn absence, which no other fixture reaches. */
function FixturePickingClear() {
  const bench = fixturePicking(PICKING_CLEAR);
  return (
    <FloorShell
      title="Picking — clear"
      site="MEL"
      who="d.stooke"
      dock={<PickDock bench={bench} />}
    >
      <PickList bench={bench} />
    </FloorShell>
  );
}

/** The four states, none of which a real deployment shows for long. */
function setupRoute(title: string, state: Parameters<typeof fixtureSetup>[0]) {
  return function SetupFixture() {
    return (
      <PlainShell title={title}>
        <Setup bench={fixtureSetup(state)} />
      </PlainShell>
    );
  };
}

const FixtureSetup = setupRoute("Setup — fixture", NEEDED);
const FixtureSetupNoToken = setupRoute("Setup — no token", NO_TOKEN);
const FixtureSetupClosed = setupRoute("Setup — already set up", CLOSED);
const FixtureSetupDone = setupRoute("Setup — done", DONE);

/** Passkeys. A ceremony needs an authenticator, so a headless render
 *  reaches none of these — least of all a browser that cannot do passkeys. */
function keysRoute(title: string, bench: Parameters<typeof Keys>[0]["bench"]) {
  return function KeysFixture() {
    return (
      <PlainShell title={title}>
        <Keys bench={bench} />
      </PlainShell>
    );
  };
}

const FixtureKeys = keysRoute("Passkeys", fixtureKeys(KEYS_READY));
const FixtureKeysNone = keysRoute("Passkeys — none", fixtureKeys(KEYS_NONE));
const FixtureKeysUnsupported = keysRoute(
  "Passkeys — unsupported",
  fixtureKeys(KEYS_NONE, { supported: false }),
);
const FixtureKeysFailed = keysRoute("Passkeys — unreachable", fixtureKeys(KEYS_FAILED));

/** The bin import. The dry-run report is the state worth drawing: it is what a
 *  person reads before deciding, and on a live screen it lasts one click. */
function importRoute(title: string, state: Parameters<typeof fixtureImport>[0]) {
  return function ImportFixture() {
    return (
      <DeskShell title={title} site="MEL" who="d.stooke">
        <Import bench={fixtureImport(state)} />
      </DeskShell>
    );
  };
}

const FixtureImport = importRoute("Import", IMP_IDLE);
const FixtureImportDry = importRoute("Import — dry run", IMP_DRY);
const FixtureImportApplied = importRoute("Import — applied", IMP_APPLIED);
const FixtureImportFailed = importRoute("Import — refused", IMP_FAILED);
const FixtureImportItems = importRoute("Import — item master", IMP_ITEMS);

/** The workspace. Four warehouses, one of them empty, which is the state the
 *  bin import leaves behind when a site's bins state no type. */
function workspaceRoute(title: string, state: Parameters<typeof fixtureWorkspace>[0]) {
  return function WorkspaceFixture() {
    return (
      <DeskShell title={title} site="MEL" who="d.stooke">
        <Workspace bench={fixtureWorkspace(state)} />
      </DeskShell>
    );
  };
}

const FixtureWorkspace = workspaceRoute("Workspace", WS_READY);
const FixtureWorkspaceEmpty = workspaceRoute("Workspace — no warehouses", WS_EMPTY);
const FixtureWorkspaceFailed = workspaceRoute("Workspace — unreachable", WS_FAILED);

/** Import tokens (D158). The secret shows for seconds on a live screen and
 *  never again, so a fixture is the only way it is ever looked at twice. */
function tokensRoute(title: string, bench: Parameters<typeof Tokens>[0]["bench"]) {
  return function TokensFixture() {
    return (
      <DeskShell title={title} site="MEL" who="d.stooke">
        <Tokens bench={bench} />
      </DeskShell>
    );
  };
}

const FixtureTokens = tokensRoute("Import tokens", fixtureTokens(TOKENS_READY));
const FixtureTokensNone = tokensRoute("Import tokens — none", fixtureTokens(TOKENS_NONE));
const FixtureTokensMinted = tokensRoute(
  "Import tokens — just minted",
  fixtureTokens(TOKENS_READY, { minted: MINTED }),
);
const FixtureTokensFailed = tokensRoute("Import tokens — unreachable", fixtureTokens(TOKENS_FAILED));

/** The states a live screen shows one of, briefly, and only after something. */
function passwordRoute(title: string, state: Parameters<typeof fixturePassword>[0]) {
  return function PasswordFixture() {
    return (
      <PlainShell title={title}>
        <Password bench={fixturePassword(state)} />
      </PlainShell>
    );
  };
}

const FixturePassword = passwordRoute("Password — fixture", READY);
const FixturePasswordNoSession = passwordRoute("Password — not signed in", NO_SESSION);
const FixturePasswordEnded = passwordRoute("Password — changed", DONE_ENDED);
const FixturePasswordAlone = passwordRoute("Password — changed, alone", DONE_ALONE);

/** Typed two different new passwords. The one judgement the client makes for
 *  itself, because the server never sees the second copy. */
function FixturePasswordMismatch() {
  return (
    <PlainShell title="Password — mismatch">
      <Password
        bench={fixturePassword(READY, {
          draft: { current: "a-current-password", next: "a-new-password", again: "a-nwe-password" },
          mismatched: true,
        })}
      />
    </PlainShell>
  );
}

/** The server refused it. The message is the server's, drawn where every other
 *  refusal on this screen is drawn. */
function FixturePasswordRefused() {
  return (
    <PlainShell title="Password — refused">
      <Password
        bench={fixturePassword(READY, {
          problem: "that is not the current password",
        })}
      />
    </PlainShell>
  );
}

/**
 * Every fixture, under `/fixtures`.
 *
 * The paths are the ones the render gate visits. They moved out of the screen
 * namespace so that `/pack` means the pack screen with your work on it rather
 * than a picture of somebody else's.
 */
/**
 * **The rail is drawn here**, because it is drawn on every live Bench and Desk
 * screen and the gate had no way to see it otherwise. The counts are the
 * fixture's own, so the badge rule is checkable: `home` has work waiting and
 * `home/quiet` has none, which is what proves zero hides rather than drawing a
 * nought (D112).
 */
function homeRoute(
  title: string,
  state: Parameters<typeof fixtureHome>[0],
  counts: Parameters<typeof WorkRail>[0]["counts"],
  landing: Parameters<typeof fixtureScan>[0] = null,
) {
  return function HomeFixture() {
    return (
      <BenchShell
        title={title}
        site="MEL"
        who="d.stooke"
        rail={<WorkRail here="home" counts={counts} />}
        locator={<Locator scan={fixtureScan(landing)} />}
      >
        <Home bench={fixtureHome(state)} />
      </BenchShell>
    );
  };
}
const FixtureHome = homeRoute("Waiting", BUSY, { pack: 4, pick: 12, despatch: 1, findings: 7 });
const FixtureHomeQuiet = homeRoute("Waiting — nothing", QUIET, {});
const FixtureHomeNoSite = homeRoute("Waiting — no site", NO_SITE, {});
const busy = { pack: 4, pick: 12, despatch: 1, findings: 7 };

function signInRoute(
  title: string,
  state: Parameters<typeof fixtureSignIn>[0],
  over: Parameters<typeof fixtureSignIn>[1] = {},
) {
  return function SignInFixture() {
    return (
      <PlainShell title={title} align="centre">
        <SignIn bench={fixtureSignIn(state, over)} />
      </PlainShell>
    );
  };
}
const FixtureSignIn = signInRoute("Sign in", ASKING);
const FixtureSignInCompany = signInRoute("Sign in — which company", CHOOSE_COMPANY, {
  tenant: "11111111-1111-1111-1111-111111111111",
  problem: "You work for more than one company. Choose which.",
});
const FixtureSignInRefused = signInRoute("Sign in — refused", ASKING, { problem: REFUSED });
/** The same question, arrived at with a key: the control that carries on is a
 *  second ceremony rather than a password, and says so. */
const FixtureSignInCompanyKey = signInRoute("Sign in — which company, by key", CHOOSE_COMPANY, {
  tenant: "11111111-1111-1111-1111-111111111111",
  via: "key",
  credentials: { email: "", password: "" },
  problem:
    "You work for more than one company. Choose which, and your key will be asked for again.",
});
/** A browser with no `PublicKeyCredential`: a sentence where the control was. */
const FixtureSignInNoKeys = signInRoute("Sign in — no passkeys here", ASKING, { keys: false });

function queueRoute(title: string, state: Parameters<typeof fixtureQueue>[0]) {
  return function QueueFixture() {
    return (
      <BenchShell
        title={title}
        site="MEL"
        who="d.stooke"
        rail={<WorkRail here="pack" counts={busy} />}
        locator={<Locator scan={fixtureScan()} />}
      >
        <PackQueue bench={fixtureQueue(state)} />
      </BenchShell>
    );
  };
}
const FixtureQueue = queueRoute("Pack — the queue", QUEUE);
const FixtureQueueClear = queueRoute("Pack — nothing waiting", CLEAR);

function ordersRoute(title: string, state: Parameters<typeof fixtureOrders>[0]) {
  return function OrdersFixture() {
    return (
      <DeskShell
        title={title}
        site="MEL"
        who="d.stooke"
        rail={<WorkRail here="orders" counts={busy} />}
        locator={<Locator scan={fixtureScan()} />}
      >
        <Orders desk={fixtureOrders(state)} />
      </DeskShell>
    );
  };
}
const FixtureOrders = ordersRoute("Find an order", FOUND);
const FixtureOrdersLatest = ordersRoute("Find an order — the latest", LATEST);
const FixtureOrdersSuperseded = ordersRoute("Find an order — replaced", SUPERSEDED);
const FixtureOrdersNothing = ordersRoute("Find an order — nothing", NOTHING);
const FixtureScanAmbig = homeRoute("Scan — ambiguous", BUSY, busy, AMBIGUOUS);
const FixtureScanUnknownId = homeRoute("Scan — unknown", BUSY, busy, UNKNOWN);
const FixtureScanSmudge = homeRoute("Scan — unrecognised", BUSY, busy, UNRECOGNISED);
const FixtureScanNowhere = homeRoute("Scan — no screen", BUSY, busy, NO_SCREEN);

const DRAWN: readonly Screen[] = [
  { id: "f-home", path: "/fixtures/home", title: "Waiting", surface: "bench", pattern: pattern("/fixtures/home"), render: () => <FixtureHome /> },
  { id: "f-home-quiet", path: "/fixtures/home/quiet", title: "Waiting — nothing", surface: "bench", pattern: pattern("/fixtures/home/quiet"), render: () => <FixtureHomeQuiet /> },
  { id: "f-scan-ambiguous", path: "/fixtures/scan/ambiguous", title: "Scan — ambiguous", surface: "bench", pattern: pattern("/fixtures/scan/ambiguous"), render: () => <FixtureScanAmbig /> },
  { id: "f-scan-unknown", path: "/fixtures/scan/unknown", title: "Scan — unknown", surface: "bench", pattern: pattern("/fixtures/scan/unknown"), render: () => <FixtureScanUnknownId /> },
  { id: "f-scan-smudge", path: "/fixtures/scan/unrecognised", title: "Scan — unrecognised", surface: "bench", pattern: pattern("/fixtures/scan/unrecognised"), render: () => <FixtureScanSmudge /> },
  { id: "f-scan-nowhere", path: "/fixtures/scan/nowhere", title: "Scan — no screen", surface: "bench", pattern: pattern("/fixtures/scan/nowhere"), render: () => <FixtureScanNowhere /> },
  { id: "f-home-no-site", path: "/fixtures/home/no-site", title: "Waiting — no site", surface: "bench", pattern: pattern("/fixtures/home/no-site"), render: () => <FixtureHomeNoSite /> },
  { id: "f-pack", path: "/fixtures/pack", title: "Pack — fixture", surface: "bench", pattern: pattern("/fixtures/pack"), render: () => <FixturePack /> },
  { id: "f-queue", path: "/fixtures/queue", title: "Pack — the queue", surface: "bench", pattern: pattern("/fixtures/queue"), render: () => <FixtureQueue /> },
  { id: "f-queue-clear", path: "/fixtures/queue/clear", title: "Pack — nothing waiting", surface: "bench", pattern: pattern("/fixtures/queue/clear"), render: () => <FixtureQueueClear /> },
  { id: "f-despatch", path: "/fixtures/despatch", title: "Despatch — fixture", surface: "bench", pattern: pattern("/fixtures/despatch"), render: () => <FixtureDespatch /> },
  { id: "f-picking", path: "/fixtures/picking", title: "Picking", surface: "floor", pattern: pattern("/fixtures/picking"), render: () => <FixturePicking /> },
  { id: "f-picking-clear", path: "/fixtures/picking/clear", title: "Picking — clear", surface: "floor", pattern: pattern("/fixtures/picking/clear"), render: () => <FixturePickingClear /> },
  { id: "f-picking-pallet", path: "/fixtures/picking/pallet", title: "Picking — onto a pallet", surface: "floor", pattern: pattern("/fixtures/picking/pallet"), render: () => <FixturePickingOnPallet /> },
  { id: "f-picking-short", path: "/fixtures/picking/short", title: "Picking — short", surface: "floor", pattern: pattern("/fixtures/picking/short"), render: () => <FixturePickingShort /> },
  { id: "f-picking-refused", path: "/fixtures/picking/refused", title: "Picking — refused", surface: "floor", pattern: pattern("/fixtures/picking/refused"), render: () => <FixturePickingRefused /> },
  { id: "f-picking-took", path: "/fixtures/picking/recorded", title: "Picking — recorded", surface: "floor", pattern: pattern("/fixtures/picking/recorded"), render: () => <FixturePickingTook /> },
  { id: "f-receiving", path: "/fixtures/receiving", title: "Receiving", surface: "floor", pattern: pattern("/fixtures/receiving"), render: () => <FixtureReceiving /> },
  { id: "f-receiving-counting", path: "/fixtures/receiving/counting", title: "Receiving — counting", surface: "floor", pattern: pattern("/fixtures/receiving/counting"), render: () => <FixtureReceivingCounting /> },
  { id: "f-receiving-over", path: "/fixtures/receiving/over", title: "Receiving — over", surface: "floor", pattern: pattern("/fixtures/receiving/over"), render: () => <FixtureReceivingOver /> },
  { id: "f-receiving-no-owner", path: "/fixtures/receiving/no-owner", title: "Receiving — no owner", surface: "floor", pattern: pattern("/fixtures/receiving/no-owner"), render: () => <FixtureReceivingNoOwner /> },
  { id: "f-receiving-refused", path: "/fixtures/receiving/refused", title: "Receiving — refused", surface: "floor", pattern: pattern("/fixtures/receiving/refused"), render: () => <FixtureReceivingRefused /> },
  { id: "f-receiving-landed", path: "/fixtures/receiving/taken-in", title: "Receiving — taken in", surface: "floor", pattern: pattern("/fixtures/receiving/taken-in"), render: () => <FixtureReceivingLanded /> },
  { id: "f-receiving-clear", path: "/fixtures/receiving/clear", title: "Receiving — clear", surface: "floor", pattern: pattern("/fixtures/receiving/clear"), render: () => <FixtureReceivingClear /> },
  { id: "f-putaway", path: "/fixtures/putaway", title: "Put away", surface: "floor", pattern: pattern("/fixtures/putaway"), render: () => <FixturePutaway /> },
  { id: "f-putaway-holding", path: "/fixtures/putaway/in-hand", title: "Put away — in hand", surface: "floor", pattern: pattern("/fixtures/putaway/in-hand"), render: () => <FixturePutawayHolding /> },
  { id: "f-putaway-no-bin", path: "/fixtures/putaway/no-bin", title: "Put away — no bin yet", surface: "floor", pattern: pattern("/fixtures/putaway/no-bin"), render: () => <FixturePutawayNoBin /> },
  { id: "f-putaway-refused", path: "/fixtures/putaway/refused", title: "Put away — refused", surface: "floor", pattern: pattern("/fixtures/putaway/refused"), render: () => <FixturePutawayRefused /> },
  { id: "f-putaway-stowed", path: "/fixtures/putaway/recorded", title: "Put away — recorded", surface: "floor", pattern: pattern("/fixtures/putaway/recorded"), render: () => <FixturePutawayStowed /> },
  { id: "f-putaway-clear", path: "/fixtures/putaway/clear", title: "Put away — clear", surface: "floor", pattern: pattern("/fixtures/putaway/clear"), render: () => <FixturePutawayClear /> },
  { id: "f-capture", path: "/fixtures/capture", title: "Capture", surface: "floor", pattern: pattern("/fixtures/capture"), render: () => <FixtureCapture /> },
  { id: "f-capture-clear", path: "/fixtures/capture/clear", title: "Capture — clear", surface: "floor", pattern: pattern("/fixtures/capture/clear"), render: () => <FixtureCaptureClear /> },
  { id: "f-capture-scanned", path: "/fixtures/capture/scanned", title: "Capture — scanned", surface: "floor", pattern: pattern("/fixtures/capture/scanned"), render: () => <FixtureScanResolved /> },
  { id: "f-capture-ambiguous", path: "/fixtures/capture/ambiguous", title: "Capture — ambiguous", surface: "floor", pattern: pattern("/fixtures/capture/ambiguous"), render: () => <FixtureScanAmbiguous /> },
  { id: "f-capture-unknown", path: "/fixtures/capture/unknown", title: "Capture — unknown", surface: "floor", pattern: pattern("/fixtures/capture/unknown"), render: () => <FixtureScanUnknown /> },
  { id: "f-capture-figures", path: "/fixtures/capture/figures", title: "Capture — figures", surface: "floor", pattern: pattern("/fixtures/capture/figures"), render: () => <FixtureCaptureFigures /> },
  { id: "f-capture-each", path: "/fixtures/capture/each", title: "Capture — each", surface: "floor", pattern: pattern("/fixtures/capture/each"), render: () => <FixtureCaptureEach /> },
  { id: "f-capture-faces", path: "/fixtures/capture/faces", title: "Capture — faces", surface: "floor", pattern: pattern("/fixtures/capture/faces"), render: () => <FixtureCaptureFaces /> },
  { id: "f-capture-barcodes", path: "/fixtures/capture/barcodes", title: "Capture — barcodes", surface: "floor", pattern: pattern("/fixtures/capture/barcodes"), render: () => <FixtureCaptureBarcodes /> },
  { id: "f-orders-latest", path: "/fixtures/orders/latest", title: "Orders — the latest", surface: "desk", pattern: pattern("/fixtures/orders/latest"), render: () => <FixtureOrdersLatest /> },
  { id: "f-orders", path: "/fixtures/orders", title: "Find an order", surface: "desk", pattern: pattern("/fixtures/orders"), render: () => <FixtureOrders /> },
  { id: "f-orders-superseded", path: "/fixtures/orders/superseded", title: "Find an order — replaced", surface: "desk", pattern: pattern("/fixtures/orders/superseded"), render: () => <FixtureOrdersSuperseded /> },
  { id: "f-orders-nothing", path: "/fixtures/orders/nothing", title: "Find an order — nothing", surface: "desk", pattern: pattern("/fixtures/orders/nothing"), render: () => <FixtureOrdersNothing /> },
  { id: "f-findings", path: "/fixtures/findings", title: "Findings — fixture", surface: "desk", pattern: pattern("/fixtures/findings"), render: () => <FixtureFindings /> },
  { id: "f-findings-evidence", path: "/fixtures/findings/evidence", title: "Findings — evidence", surface: "desk", pattern: pattern("/fixtures/findings/evidence"), render: () => <FixtureFindingsPanel /> },
  { id: "f-findings-closed", path: "/fixtures/findings/closed", title: "Findings — closed", surface: "desk", pattern: pattern("/fixtures/findings/closed"), render: () => <FixtureFindingsClosed /> },
  { id: "f-findings-missing", path: "/fixtures/findings/missing", title: "Findings — not here", surface: "desk", pattern: pattern("/fixtures/findings/missing"), render: () => <FixtureFindingsMissing /> },
  { id: "f-findings-clear", path: "/fixtures/findings/clear", title: "Findings — clear", surface: "desk", pattern: pattern("/fixtures/findings/clear"), render: () => <FixtureFindingsClear /> },
  { id: "f-weigh", path: "/fixtures/weigh", title: "Weigh — fixture", surface: "bench", pattern: pattern("/fixtures/weigh"), render: () => <FixtureWeigh /> },
  { id: "f-weigh-disagreed", path: "/fixtures/weigh/disagreed", title: "Weigh — disagreed", surface: "bench", pattern: pattern("/fixtures/weigh/disagreed"), render: () => <FixtureWeighDisagreed /> },
  { id: "f-weigh-clear", path: "/fixtures/weigh/clear", title: "Weigh — clear", surface: "bench", pattern: pattern("/fixtures/weigh/clear"), render: () => <FixtureWeighClear /> },
  { id: "f-sign-in", path: "/fixtures/sign-in", title: "Sign in", surface: "plain", pattern: pattern("/fixtures/sign-in"), render: () => <FixtureSignIn /> },
  { id: "f-sign-in-company", path: "/fixtures/sign-in/company", title: "Sign in — which company", surface: "plain", pattern: pattern("/fixtures/sign-in/company"), render: () => <FixtureSignInCompany /> },
  { id: "f-sign-in-refused", path: "/fixtures/sign-in/refused", title: "Sign in — refused", surface: "plain", pattern: pattern("/fixtures/sign-in/refused"), render: () => <FixtureSignInRefused /> },
  { id: "f-sign-in-company-key", path: "/fixtures/sign-in/company-key", title: "Sign in — which company, by key", surface: "plain", pattern: pattern("/fixtures/sign-in/company-key"), render: () => <FixtureSignInCompanyKey /> },
  { id: "f-sign-in-no-keys", path: "/fixtures/sign-in/no-keys", title: "Sign in — no passkeys here", surface: "plain", pattern: pattern("/fixtures/sign-in/no-keys"), render: () => <FixtureSignInNoKeys /> },
  { id: "f-setup", path: "/fixtures/setup", title: "Setup — fixture", surface: "plain", pattern: pattern("/fixtures/setup"), render: () => <FixtureSetup /> },
  { id: "f-setup-no-token", path: "/fixtures/setup/no-token", title: "Setup — no token", surface: "plain", pattern: pattern("/fixtures/setup/no-token"), render: () => <FixtureSetupNoToken /> },
  { id: "f-setup-closed", path: "/fixtures/setup/closed", title: "Setup — already set up", surface: "plain", pattern: pattern("/fixtures/setup/closed"), render: () => <FixtureSetupClosed /> },
  { id: "f-setup-done", path: "/fixtures/setup/done", title: "Setup — done", surface: "plain", pattern: pattern("/fixtures/setup/done"), render: () => <FixtureSetupDone /> },
  { id: "f-keys", path: "/fixtures/keys", title: "Passkeys", surface: "plain", pattern: pattern("/fixtures/keys"), render: () => <FixtureKeys /> },
  { id: "f-keys-none", path: "/fixtures/keys/none", title: "Passkeys — none", surface: "plain", pattern: pattern("/fixtures/keys/none"), render: () => <FixtureKeysNone /> },
  { id: "f-keys-unsupported", path: "/fixtures/keys/unsupported", title: "Passkeys — unsupported", surface: "plain", pattern: pattern("/fixtures/keys/unsupported"), render: () => <FixtureKeysUnsupported /> },
  { id: "f-keys-failed", path: "/fixtures/keys/failed", title: "Passkeys — unreachable", surface: "plain", pattern: pattern("/fixtures/keys/failed"), render: () => <FixtureKeysFailed /> },
  { id: "f-import", path: "/fixtures/import", title: "Import", surface: "desk", pattern: pattern("/fixtures/import"), render: () => <FixtureImport /> },
  { id: "f-import-dry", path: "/fixtures/import/dry", title: "Import — dry run", surface: "desk", pattern: pattern("/fixtures/import/dry"), render: () => <FixtureImportDry /> },
  { id: "f-import-applied", path: "/fixtures/import/applied", title: "Import — applied", surface: "desk", pattern: pattern("/fixtures/import/applied"), render: () => <FixtureImportApplied /> },
  { id: "f-import-items", path: "/fixtures/import/items", title: "Import — item master", surface: "desk", pattern: pattern("/fixtures/import/items"), render: () => <FixtureImportItems /> },
  { id: "f-import-failed", path: "/fixtures/import/failed", title: "Import — refused", surface: "desk", pattern: pattern("/fixtures/import/failed"), render: () => <FixtureImportFailed /> },
  { id: "f-workspace", path: "/fixtures/workspace", title: "Workspace", surface: "desk", pattern: pattern("/fixtures/workspace"), render: () => <FixtureWorkspace /> },
  { id: "f-workspace-empty", path: "/fixtures/workspace/empty", title: "Workspace — no warehouses", surface: "desk", pattern: pattern("/fixtures/workspace/empty"), render: () => <FixtureWorkspaceEmpty /> },
  { id: "f-workspace-failed", path: "/fixtures/workspace/failed", title: "Workspace — unreachable", surface: "desk", pattern: pattern("/fixtures/workspace/failed"), render: () => <FixtureWorkspaceFailed /> },
  { id: "f-tokens", path: "/fixtures/tokens", title: "Import tokens", surface: "desk", pattern: pattern("/fixtures/tokens"), render: () => <FixtureTokens /> },
  { id: "f-tokens-none", path: "/fixtures/tokens/none", title: "Import tokens — none", surface: "desk", pattern: pattern("/fixtures/tokens/none"), render: () => <FixtureTokensNone /> },
  { id: "f-tokens-minted", path: "/fixtures/tokens/minted", title: "Import tokens — just minted", surface: "desk", pattern: pattern("/fixtures/tokens/minted"), render: () => <FixtureTokensMinted /> },
  { id: "f-tokens-failed", path: "/fixtures/tokens/failed", title: "Import tokens — unreachable", surface: "desk", pattern: pattern("/fixtures/tokens/failed"), render: () => <FixtureTokensFailed /> },
  { id: "f-password", path: "/fixtures/password", title: "Password — fixture", surface: "plain", pattern: pattern("/fixtures/password"), render: () => <FixturePassword /> },
  { id: "f-password-mismatch", path: "/fixtures/password/mismatch", title: "Password — mismatch", surface: "plain", pattern: pattern("/fixtures/password/mismatch"), render: () => <FixturePasswordMismatch /> },
  { id: "f-password-refused", path: "/fixtures/password/refused", title: "Password — refused", surface: "plain", pattern: pattern("/fixtures/password/refused"), render: () => <FixturePasswordRefused /> },
  { id: "f-password-no-session", path: "/fixtures/password/no-session", title: "Password — not signed in", surface: "plain", pattern: pattern("/fixtures/password/no-session"), render: () => <FixturePasswordNoSession /> },
  { id: "f-password-changed", path: "/fixtures/password/changed", title: "Password — changed", surface: "plain", pattern: pattern("/fixtures/password/changed"), render: () => <FixturePasswordEnded /> },
  { id: "f-password-changed-alone", path: "/fixtures/password/changed-alone", title: "Password — changed, alone", surface: "plain", pattern: pattern("/fixtures/password/changed-alone"), render: () => <FixturePasswordAlone /> },
  { id: "f-where", path: "/fixtures/where", title: "Where — choose", surface: "plain", pattern: pattern("/fixtures/where"), render: () => <FixtureWhere /> },
  { id: "f-where-settled", path: "/fixtures/where/settled", title: "Where — settled", surface: "plain", pattern: pattern("/fixtures/where/settled"), render: () => <FixtureWhereSettled /> },
  { id: "f-where-nowhere", path: "/fixtures/where/nowhere", title: "Where — no site", surface: "plain", pattern: pattern("/fixtures/where/nowhere"), render: () => <FixtureWhereNowhere /> },
];

/**
 * Every fixture draws its own shell, so the router hands it straight through.
 *
 * That is what a fixture *is*: a component in one state, with literal chrome and
 * no network, so it carries its own `site` and `who` and its own `dock` or
 * evidence panel. The frame the live screens share fetches a session and the
 * badge counts, and the render gate counts a failed request as a failure — so
 * putting these inside it would break the no-network property that makes sixty
 * screens reviewable without a server.
 *
 * Marked here rather than repeated on every entry: it is true of all of them,
 * and a flag that has to be remembered per line is a flag that gets forgotten.
 */
export const FIXTURES: readonly Screen[] = DRAWN.map((s) => ({ ...s, own: true }));
