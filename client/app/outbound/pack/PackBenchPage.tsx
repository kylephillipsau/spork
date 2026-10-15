import { useEffect, useMemo, useRef, useState } from "react";
import { Sparkles, Undo2 } from "lucide-react";

import {
  Alert,
  Badge,
  Button,
  Card,
  Dialog,
  DragZone,
  Link,
  Page,
  PageHeader,
  Select,
  Skeleton,
  TextField,
  useChosen,
  useDragNow,
  useToast,
} from "@ui/index";
import { uuid } from "@domain/acts";
import type { BenchScreen } from "@domain/types";
import { usePageTitle } from "@app/routing/title";
import { ItemDrawer } from "@app/items/ItemProperties";
import { Kept, othersSaying } from "@app/common/Kept";

import { boardOf, buildFor, dropOn, labelOf, parcelKey, thingOf, type Board, type BoardParcel, type Outcome } from "./board";
import { proposedHollow } from "./carrying";
import { needsCheck, uncheckedIn } from "./check";
import { Checks } from "./Checks";
import { Chosen, tally } from "./Chosen";
import { DropSlot, ParcelCard, type ParcelActs } from "./Parcels";
import { ToPack } from "./ToPack";
import type { KeptPress, PackBench } from "./usePackBench";
import s from "./pack-bench.module.css";

/** A drop's id for a parcel it would make, while only saying what it would do. */
const SAYING = () => "preview";

/**
 * The pack bench for one fulfilment: the order on one board (D245), laid out
 * for a 1080p screen, a mouse and a keyboard. To pack and its check on the
 * left, the parcels in the middle, pallets holding their cartons, and the
 * chosen parcel in 3D on the right. Above, the tally, Undo, Suggest and the
 * next thing to do, the one primary press.
 *
 * `drag` holds a drag still, for a screen drawn from a fixture.
 */
export function PackBenchPage({ bench, drag = null }: { bench: PackBench; drag?: { keys: string[]; over: string | null } | null }) {
  // The item whose size, weight and photos are open beside the bench (D174).
  const [looking, setLooking] = useState<string | null>(null);
  const st = bench.status;
  const loaded = st.kind === "ready" ? st.screen : null;
  usePageTitle([loaded?.order_reference, loaded?.reference, loaded?.customer]);

  if (st.kind !== "ready") {
    return (
      <Page>
        <PageHeader title="Pack order" />
        {st.kind === "failed" ? (
          <Alert tone="danger">{st.message}</Alert>
        ) : (
          <Card>
            <Skeleton width="50%" />
          </Card>
        )}
      </Page>
    );
  }
  return <Bench screen={st.screen} bench={bench} drag={drag} looking={looking} look={setLooking} />;
}

function Bench({
  screen,
  bench,
  drag,
  looking,
  look,
}: {
  screen: BenchScreen;
  bench: PackBench;
  drag: { keys: string[]; over: string | null } | null;
  looking: string | null;
  look: (item: string | null) => void;
}) {
  const board = useMemo(() => boardOf(screen), [screen]);
  const toast = useToast();
  // A drop with Alt, or M: where to, and how many.
  const [moving, setMoving] = useState<{ keys: string[]; target: string | null } | null>(null);
  // Solid or hollow, as the packer said it before sealing; else as proposed (D249).
  const [hollowSaid, setHollowSaid] = useState<Record<string, boolean>>({});
  const hollowOf = (p: BoardParcel) => hollowSaid[p.id] ?? proposedHollow(p);

  // An order opened with nothing planned is planned at once, as the bench
  // always suggested; after that, only Suggest plans again.
  const suggested = useRef(false);
  const left = board.lines.reduce((t, b) => t + b.left, 0);
  useEffect(() => {
    if (suggested.current || screen.plan.length > 0 || left === 0) return;
    suggested.current = true;
    void bench.suggest();
  }, [screen.plan.length, left]); // eslint-disable-line react-hooks/exhaustive-deps

  const drop = (keys: string[], target: string, how: { one: boolean; ask: boolean }) => {
    if (how.ask) return setMoving({ keys, target });
    const out = dropOn(screen, board, keys, target, { one: how.one }, uuid);
    // Nothing to choose a box by: ask which, in Move to….
    const box = screen.presets.find((p) => p.size && !p.deck && p.suggested);
    if (out.chooseBox && box) return setMoving({ keys, target: `new-box:${box.id}` });
    if (!out.ok) return toast({ title: out.verdict, tone: "danger" });
    void bench.apply(out, out.verdict);
  };

  // Picked elsewhere and not yet here: where the picking happened is said once.
  const elsewhere = screen.lines.find((l) => l.elsewhere)?.elsewhere?.provenance ?? null;
  const acts: ParcelActs = {
    bench,
    screen,
    hollowOf,
    sayHollow: (id, hollow) => setHollowSaid((said) => ({ ...said, [id]: hollow })),
    buildFor: (p, carrier) => {
      const out = buildFor(board, p.id, carrier);
      void bench.apply(out, out.verdict);
    },
    takeOut: (line, carton) => drop([`goods:${line}@${carton}`], "bench", { one: false, ask: false }),
    canShip: (p) => {
      const l = p.goods[0]?.line;
      return !!l && !!screen.dock_id && !bench.busy && (l.cells.length > 0 || !!l.elsewhere) && !needsCheck(l);
    },
    ship: (p) => {
      const a = p.asIs;
      const l = p.goods[0]?.line;
      if (!a || !l) return;
      void bench.shipAsIs({
        line: l.line_id,
        item: l.item_id,
        level: a.level,
        config: l.own_carton?.item_packing_config_id ?? null,
        units: a.per,
        count: a.count,
      });
    },
  };
  const next = nextStep(board, screen, bench, hollowOf);
  const waiting = bench.kept.waiting.length > 0;

  return (
    <Page>
      <PageHeader
        title={
          <>
            Pack <span className={s.ref}>{screen.reference}</span>
          </>
        }
        description={`${screen.customer} · Order ${screen.order_reference}`}
        actions={<Badge tone={left ? "accent" : "success"}>{left ? `${left} ${left === 1 ? "unit" : "units"} to place` : "Everything placed"}</Badge>}
      />

      {bench.problem && (
        <Alert tone="danger" onDismiss={bench.dismiss}>
          {bench.problem}
        </Alert>
      )}
      <KeptPresses kept={bench.kept} order={screen.order_reference} />
      {/* Said before somebody tries, rather than as the refusal after. */}
      {screen.unready && (
        <Alert tone="warning">
          {screen.unready} <Link href="/workspace">Open Workspace</Link>
        </Alert>
      )}

      <div className={s.boardTop}>
        <span className={s.boardTally}>{tally(board.view)}</span>
        <Button icon={<Undo2 />} disabled={bench.busy || !bench.undoable} onClick={() => void bench.undo()} aria-keyshortcuts="Z">
          {bench.undoable ? `Undo: ${bench.undoable}` : "Undo"}
        </Button>
        <Button icon={<Sparkles />} disabled={bench.busy} onClick={() => void bench.suggest()}>
          Suggest
        </Button>
        {next && (
          <Button variant="primary" disabled={bench.busy || !next.can} onClick={next.press}>
            {next.label}
          </Button>
        )}
      </div>

      {/* While a kept press waits, the board waits for it: nothing is worked out from a board the server hasn't caught up with. */}
      <div className={waiting ? s.waits : undefined} inert={waiting}>
        <DragZone
          label={(keys) => labelOf(board, keys)}
          onDrop={drop}
          onMove={(keys) => setMoving({ keys, target: null })}
          onRemove={(keys) => drop(keys, "bench", { one: false, ask: false })}
          onUndo={() => void bench.undo()}
          frozen={drag}
        >
          <BoardBody screen={screen} board={board} acts={acts} look={look} elsewhere={elsewhere} />
        </DragZone>
      </div>

      {moving && <MoveTo screen={screen} board={board} keys={moving.keys} target={moving.target} close={() => setMoving(null)} bench={bench} />}

      {/* Read again on closing: a size recorded in it changes what fits. */}
      <ItemDrawer
        itemId={looking}
        onClose={() => {
          look(null);
          bench.refresh();
        }}
      />
    </Page>
  );
}

/**
 * The bench's presses kept on this device (D207): what waits, and the board
 * with it; what the server refused when it went; and anybody else's.
 */
function KeptPresses({ kept, order }: { kept: PackBench["kept"]; order: string }) {
  const named = (k: KeptPress) => (k.order && k.order !== order ? `${k.label}, on order ${k.order}` : k.label);
  const n = kept.waiting.length;
  return (
    <Kept
      waiting={
        n > 0
          ? `Saved on this device: ${kept.waiting.map((k) => `${named(k)}.`).join(" ")} ${n === 1 ? "It" : "They"} will be sent when there's a connection, and the board waits until then.`
          : null
      }
      send={kept.send}
      refused={kept.refused.map((k) => ({ key: k.key, text: `Not recorded: ${named(k)}. ${k.reason}` }))}
      dismiss={kept.dismiss}
      others={othersSaying(kept.others, "press", "presses")}
    />
  );
}

/** The three columns, inside the drag so they can see it. */
function BoardBody({
  screen,
  board,
  acts,
  look,
  elsewhere,
}: {
  screen: BenchScreen;
  board: Board;
  acts: ParcelActs;
  look: (item: string) => void;
  elsewhere: string | null;
}) {
  const now = useDragNow();
  const chosen = useChosen();
  // What the drop under the pointer would do, said as it moves.
  const live = useMemo(() => {
    if (!now?.over) return null;
    return { target: now.over, outcome: dropOn(screen, board, now.keys, now.over, { one: false }, SAYING) };
  }, [screen, board, now?.keys, now?.over]); // eslint-disable-line react-hooks/exhaustive-deps
  const pictures = useMemo(() => new Map(screen.lines.map((l) => [l.item_id, l.picture])), [screen.lines]);
  const shown = chosenParcel(board, chosen);
  const top = board.parcels.filter((p) => p.on === null || !board.parcels.some((q) => q.id === p.on));

  return (
    <div className={s.board}>
      <div className={s.boardColumn}>
        <Checks lines={screen.lines} bench={acts.bench} look={look} />
        <ToPack board={board} look={look} elsewhere={elsewhere} />
      </div>
      <div className={s.boardColumn} aria-label="Parcels">
        {top.map((p) => (
          <ParcelCard key={p.id} parcel={p} board={board} lines={screen.lines} acts={acts} live={live} />
        ))}
        <div className={s.dropSlots}>
          <DropSlot target="new-box" label="Drop here for a new box" />
          <DropSlot target="new-pallet" label="Drop a carton here for a new pallet" />
        </div>
      </div>
      <div className={s.boardColumn}>
        <Chosen board={board} parcel={shown} outcome={live?.outcome ?? null} pictures={pictures} />
      </div>
    </div>
  );
}

/** The parcel chosen last, or the one whose goods were: what the right shows. */
function chosenParcel(board: Board, chosen: string[]): BoardParcel | null {
  for (const key of [...chosen].reverse()) {
    const t = thingOf(key);
    const id = t?.kind === "parcel" ? t.id : t?.parcel;
    const p = id ? board.parcels.find((x) => x.id === id) : undefined;
    if (p) return p;
  }
  return null;
}

/**
 * The one primary press (D171): seal the carton that is ready, else make the
 * next planned box. Each card has its own as well, quieter.
 */
function nextStep(
  board: Board,
  screen: BenchScreen,
  bench: PackBench,
  hollowOf: (p: BoardParcel) => boolean,
): { label: string; can: boolean; press: () => void } | null {
  const ready = board.parcels.find(
    (p) => p.made && p.state === "open" && p.kind === "box" && p.goods.length > 0 && !!p.carton && uncheckedIn(p.carton, screen.lines).length === 0,
  );
  if (ready) return { label: `Seal ${ready.title}`, can: true, press: () => void bench.seal(ready.id, hollowOf(ready)) };
  const planned = board.parcels.find((p) => !p.made && p.kind === "box" && p.goods.length > 0);
  if (planned) return { label: `Make ${planned.title}`, can: !!screen.dock_id, press: () => void bench.make(planned.id) };
  return null;
}

/** Move to… (settled 5): where, and how many, said before it is done. */
function MoveTo({
  screen,
  board,
  keys,
  target: first,
  close,
  bench,
}: {
  screen: BenchScreen;
  board: Board;
  keys: string[];
  target: string | null;
  close: () => void;
  bench: PackBench;
}) {
  const goods = keys.some((k) => thingOf(k)?.kind === "goods");
  const options = [
    ...board.parcels
      .filter((p) => p.kind !== "as_is" && p.state !== "sealed" && (goods ? p.kind === "box" : p.kind === "pallet"))
      .map((p) => ({ value: parcelKey(p.id), label: `${p.title}${p.made ? "" : " (planned)"}` })),
    ...(goods
      ? [
          { value: "new-box", label: "A new box, the smallest that takes it" },
          ...screen.presets.filter((p) => p.size && !p.deck).map((p) => ({ value: `new-box:${p.id}`, label: `A new ${p.name}` })),
        ]
      : [{ value: "new-pallet", label: "A new pallet" }]),
    { value: "bench", label: goods ? "Out onto the bench" : "Off its pallet" },
  ];
  const [target, setTarget] = useState(first ?? options[0]?.value ?? "bench");
  const [count, setCount] = useState("");
  const n = Number.parseInt(count, 10);
  const how = { one: false, count: n > 0 ? n : undefined };
  const preview: Outcome = useMemo(() => dropOn(screen, board, keys, target, how, SAYING), [screen, board, keys, target, count]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && close()}
      title={`Move ${labelOf(board, keys)}`}
      description="Where to, and how many: as many as fit when it says nothing."
      footer={
        <>
          <Button onClick={close}>Cancel</Button>
          <Button
            variant="primary"
            disabled={!preview.ok || bench.busy}
            onClick={() => {
              const out = dropOn(screen, board, keys, target, how, uuid);
              close();
              if (out.ok) void bench.apply(out, out.verdict);
            }}
          >
            Move
          </Button>
        </>
      }
    >
      <div className={s.moveTo}>
        <Select label="To" value={target} onValueChange={setTarget} options={options} />
        {goods && <TextField label="How many" inputMode="numeric" placeholder="All that fit" value={count} onChange={(e) => setCount(e.target.value)} />}
        <p className={s.dropVerdict} data-ok={preview.ok || undefined} role="status">
          {preview.verdict}
        </p>
      </div>
    </Dialog>
  );
}

