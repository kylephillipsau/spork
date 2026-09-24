/**
 * The screens, and what they are called.
 *
 * **This module imports no fixture**, and a gate asserts it. That is what makes
 * "the deployment has no invented data in it" a fact rather than an intention:
 * the fixtures live next door in `fixtures.tsx`, which the production build
 * never reaches.
 *
 * Every entry names a surface. That is not decoration — it says which shell
 * mounts, and it gives the render gate somewhere to check density against
 * instead of a second list kept by hand.
 */
import { useCallback, type ReactElement } from "react";

import { Dock } from "@app/shells/slots";

import "@design/tokens.css";
import "@design/layers.css";

import { PackBenchPage } from "@app/outbound/pack/PackBenchPage";

import { usePackBench } from "@app/outbound/pack/usePackBench";
import { DespatchPage } from "@app/outbound/despatch/DespatchPage";

import { useDespatch } from "@app/outbound/despatch/useDespatch";
import { Capture, CaptureDock } from "@app/measurement/capture/Capture";
import { useCapture } from "@app/measurement/capture/useCapture";
import { PickList, PickDock } from "@app/outbound/picking/PickList";
import { usePicking } from "@app/outbound/picking/usePicking";
import { Putaway, PutawayDock } from "@app/inbound/putaway/Putaway";
import { usePutaway } from "@app/inbound/putaway/usePutaway";
import { Receiving, ReceivingDock } from "@app/inbound/receiving/Receiving";
import { useReceiving } from "@app/inbound/receiving/useReceiving";
import { FindingsPage } from "@app/integrity/findings/FindingsPage";
import { useFindings } from "@app/integrity/findings/useFindings";
import { WeighPage } from "@app/measurement/weigh/WeighPage";
import { useWeigh } from "@app/measurement/weigh/useWeigh";
import { Setup } from "@app/setup/Setup";
import { useSetup } from "@app/setup/useSetup";
import { WherePage } from "@app/session/WherePage";
import { useWhere } from "@app/session/useWhere";
import { AccountPage } from "@app/account/AccountPage";
import { KeysPage } from "@app/account/KeysPage";
import { useKeys } from "@app/account/useKeys";
import { TokensPage } from "@app/admin/TokensPage";
import { ImportPage } from "@app/admin/ImportPage";
import { useImport } from "@app/admin/useImport";
import { WorkspacePage } from "@app/admin/WorkspacePage";
import { useWorkspace } from "@app/admin/useWorkspace";
import { useTokens } from "@app/admin/useTokens";
import { usePassword } from "@app/account/usePassword";

import { SCREENS } from "./manifest";
import type { Params } from "@domain/routing";

import { href } from "./location";
import { nextAfterSignIn } from "@app/session/Gate";
import { useSessionBench } from "@app/session/SessionContext";
import { useNavigate } from "@app/routing/Router";
import { SignInPage } from "@app/session/SignInPage";
import { useSignIn } from "@app/session/useSignIn";
import { OrdersPage } from "@app/outbound/orders/OrdersPage";
import { LivePackQueue } from "@app/outbound/pack/PackQueuePage";
import { useQueue } from "@app/outbound/pack/useQueue";
import { useOrders } from "@app/outbound/orders/useOrders";
import { Dashboard } from "@app/home/Dashboard";
import { useDashboard } from "@app/home/useDashboard";
import type { Screen } from "./Router";

/**
 * Every screen here draws its work and nothing else.
 *
 * The shell, the chrome, the rail, the session and the gate are all in
 * [`Framed`], mounted once above the router. A screen that drew its own would
 * be the top of the tree, and swapping it on a navigation would take all of
 * those down with it — which is exactly what these functions used to do.
 *
 * The two exceptions are regions the shell owns but the *screen's* state fills:
 * Floor's dock and Desk's evidence panel. Those are rendered here, beside the
 * hook that feeds them, and portal into the shell's container.
 */
/**
 * The queue at `/pack`, and one commitment at `/pack/:fulfilment`.
 *
 * The last of the outbound worklist to move off the server-rendered pages. It
 * opens on the work rather than on a search box, which is one of the few things
 * those pages got right.
 */
function LiveQueue() {
  return <LivePackQueue bench={useQueue()} />;
}

function LivePack({ fulfilment }: { fulfilment: string }) {
  return <PackBenchPage bench={usePackBench(fulfilment)} />;
}

function LiveDespatch() {
  return <DespatchPage bench={useDespatch()} />;
}

/**
 * Where are you working — the question no browser session has ever been asked.
 *
 * Ordinary deployments never see it: one site per tenant settles silently.
 */
function LiveWhere() {
  const { refresh } = useSessionBench();
  const navigate = useNavigate();
  // Settling mints a different session, so the chrome and every badge are stale
  // until the session is re-asked. Then carry on to whatever was being asked
  // for — replace, so Back does not return to a question already answered.
  const onSettled = useCallback(() => {
    void refresh().then(() => navigate(nextAfterSignIn(), { replace: true }));
  }, [refresh, navigate]);
  return <WherePage bench={useWhere(onSettled)} />;
}

/**
 * The Floor surface, and the first screen on it.
 *
 * **Capture was named as the screen that would force a real router, and it is
 * the screen that argues against one.** The subject being captured is held in
 * `useCapture` rather than in the URL, because D133 makes the session one act:
 * nothing typed is durable until Record, so a link that reopened
 * `/capture/17e1…` would restore the subject and silently drop the figures
 * beside it. A URL that promises a resumability the model refuses is worse
 * than no URL. The router arrives with a screen whose state is on the server.
 */
function LiveCapture() {
  const bench = useCapture();
  return (
    <>
      <Capture bench={bench} />
      <Dock>
        <CaptureDock bench={bench} />
      </Dock>
    </>
  );
}

/**
 * The Desk surface, and the screen the design document calls the premise.
 *
 * The evidence panel goes in the shell's own column rather than inside the work,
 * because "inspect without leaving" is a property of the *layout* — the queue
 * keeps its place beside the evidence — and a panel rendered inside the work
 * column would push the queue around every time a row was chosen. It is drawn
 * here because what it shows is `desk.selected`, which is this screen's state,
 * and it lands in the shell through the portal.
 *
 * **Two routes, one component.** `/findings` and `/findings/:finding` are the
 * same screen with a row open on it, so they render the same element type and
 * React reconciles rather than remounts — choosing a row pushes a history entry
 * without the queue, the shell or the room going anywhere. That is what D135
 * held the router back for: a finding is on the server, so a link to one
 * restores everything it names.
 */
function LiveFindings({ at }: { at: string | null }) {
  const navigate = useNavigate();
  // **Push, not replace.** Choosing a row is a move, and Back out of the
  // evidence is the same gesture as Back out of anything else.
  const place = useCallback(
    (id: string | null) => navigate(id === null ? "/findings" : `/findings/${id}`),
    [navigate],
  );
  return <FindingsPage desk={useFindings(at, place)} />;
}

/** Weigh is a Bench surface: standing at a scale, several hundred a day. */
function LiveWeigh() {
  return <WeighPage bench={useWeigh()} />;
}

function LiveReceiving() {
  const bench = useReceiving();
  return (
    <>
      <Receiving bench={bench} />
      <Dock>
        <ReceivingDock bench={bench} />
      </Dock>
    </>
  );
}

function LivePutaway() {
  const bench = usePutaway();
  return (
    <>
      <Putaway bench={bench} />
      <Dock>
        <PutawayDock bench={bench} />
      </Dock>
    </>
  );
}

function LivePicking() {
  const bench = usePicking();
  return (
    <>
      <PickList bench={bench} />
      <Dock>
        <PickDock bench={bench} />
      </Dock>
    </>
  );
}

/**
 * The way into a deployment that has nobody in it (D142).
 *
 * Live rather than fixture-first, unlike every other screen here, because the
 * state it reads is a property of the deployment rather than of a session — and
 * because a deployment that needs it cannot show anything else.
 */
/**
 * Signing in — the gate's destination.
 *
 * **No `SessionProvider` and no `Gate`**, like Setup beside it: both run when
 * there is no session, and wrapping this one would ask the server who you are on
 * the screen whose whole job is to establish that. The frame reads that from
 * `session: "none"` in the manifest rather than from anything here.
 */
function LiveSignIn() {
  // A full load rather than a client navigation once it succeeds. The cookie is
  // new, and every hook behind the gate should start from a provider that has
  // seen it rather than one that resolved `anonymous` a moment ago.
  return <SignInPage bench={useSignIn(() => window.location.assign(href(nextAfterSignIn())))} />;
}

function LiveSetup() {
  return <Setup bench={useSetup()} />;
}

/**
 * Changing your own password (D143).
 *
 * The same shell as setup and for the same reason: it is not a bench, a floor
 * or a desk, and what it is about is the account rather than the work.
 */
function LivePassword() {
  return <AccountPage bench={usePassword()} />;
}

/** Passkeys. The same shell as the password screen: an account, not work. */
function LiveKeys() {
  return <KeysPage bench={useKeys()} />;
}

/** Loading reference data from a file (D158). */
function LiveImport() {
  return <ImportPage bench={useImport()} />;
}

/** The organisation and its warehouses. */
function LiveWorkspace() {
  return <WorkspacePage bench={useWorkspace()} />;
}

/** Import tokens (D158). A desk task: done sitting down, rarely. */
function LiveTokens() {
  return <TokensPage bench={useTokens()} />;
}

/**
 * Finding an order (D39, D44). The reference lives in the query string rather
 * than the path: a filter is not a place, and `?reference=` keeps `/orders`
 * meaning the screen while the same URL still finds the same order.
 */
function LiveOrders() {
  const reference = new URLSearchParams(window.location.search).get("reference") ?? "";
  return <OrdersPage desk={useOrders(reference)} />;
}

/** The dashboard (D171): counts, the packing queue, findings and orders. */
function LiveHome() {
  const { session } = useSessionBench();
  const site = session.kind === "signed-in" ? session.who.site_code : null;
  return <Dashboard dash={useDashboard()} site={site} />;
}

/**
 * The live table: the manifest, with a component attached to each entry.
 *
 * **The pairing is checked rather than assumed.** A spec with no renderer is a
 * screen that resolves to nothing, and a renderer with no spec is a component
 * that renders nowhere — which is precisely the defect the render gate was
 * built after finding once already.
 */
const RENDER: Record<string, (params: Params) => ReactElement> = {
  home: () => <LiveHome />,
  pack: () => <LiveQueue />,
  "pack-one": (params) => <LivePack fulfilment={params["fulfilment"] ?? ""} />,
  despatch: () => <LiveDespatch />,
  weigh: () => <LiveWeigh />,
  capture: () => <LiveCapture />,
  picking: () => <LivePicking />,
  receiving: () => <LiveReceiving />,
  putaway: () => <LivePutaway />,
  orders: () => <LiveOrders />,
  findings: () => <LiveFindings at={null} />,
  finding: (params) => <LiveFindings at={params["finding"] ?? null} />,
  where: () => <LiveWhere />,
  account: () => <LivePassword />,
  tokens: () => <LiveTokens />,
  workspace: () => <LiveWorkspace />,
  import: () => <LiveImport />,
  keys: () => <LiveKeys />,
  "sign-in": () => <LiveSignIn />,
  setup: () => <LiveSetup />,
};

export const LIVE: readonly Screen[] = SCREENS.map((s) => {
  const render = RENDER[s.id];
  if (!render) throw new Error(`${s.id} is in the manifest with nothing to draw it`);
  return { ...s, render };
});
