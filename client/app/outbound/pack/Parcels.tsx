import { useState } from "react";
import { Boxes, FileText, Package, PackageCheck, PackageOpen, Trash2 } from "lucide-react";

import { Badge, Button, Fact, Facts, Link, TextField, useDraggable, useDropTarget } from "@ui/index";
import type { BenchLine, CartonSummary, ExpectedWeight, OnParent, PackedRow, StatedSize, Uuid } from "@domain/types";
import { agreement, provenance } from "@app/measurement/baseline";
import { href } from "@app/routing/location";
import { kg } from "@app/common/format";
import { cutSaying } from "@app/items/box";

import { goodsKey, parcelKey, type Board, type BoardParcel, type Outcome } from "./board";
import { explain, needsCheck, offBy, uncheckedIn } from "./check";
import { shipLabel } from "./Chosen";
import type { PackBench } from "./usePackBench";
import s from "./pack-bench.module.css";

/** What a parcel card can ask of the page. */
export interface ParcelActs {
  bench: PackBench;
  /** Take a line's goods in a carton out onto the bench. */
  takeOut: (line: Uuid, carton: Uuid) => void;
  /** Ship something as it is (D196). */
  ship: (p: BoardParcel) => void;
  canShip: (p: BoardParcel) => boolean;
}

const STATE: Record<BoardParcel["state"], { word: string; tone: "success" | "accent" | "neutral" }> = {
  sealed: { word: "Sealed", tone: "success" },
  open: { word: "Open", tone: "accent" },
  planned: { word: "Planned", tone: "neutral" },
  "as-is": { word: "Ships as it is", tone: "neutral" },
};

/**
 * A parcel on the board (D245): a carton, made or planned, with what is in it
 * or to go in; a pallet with the cartons on it. Something to drag (by its
 * head) and to drop on; a planned one outlined dashed, a made one solid, a
 * sealed one shut. While something is held over it, what the drop would do.
 */
export function ParcelCard({
  parcel,
  board,
  lines,
  acts,
  live,
}: {
  parcel: BoardParcel;
  board: Board;
  lines: BenchLine[];
  acts: ParcelActs;
  /** The drop under the pointer, and where: this card says it when it is here. */
  live: { target: string; outcome: Outcome } | null;
}) {
  const drag = useDraggable(parcelKey(parcel.id));
  const drop = useDropTarget(parcelKey(parcel.id));
  const on = board.parcels.filter((p) => p.on === parcel.id);
  const state = STATE[parcel.state];
  // How full, when anything in it has a size to fit by.
  const fill = parcel.arranged?.layers.length ? Math.round(parcel.arranged.fill * 100) : null;
  const limit = parcel.limit_g;
  const busy = acts.bench.busy;
  const over = live?.target === parcelKey(parcel.id) ? live.outcome : null;

  return (
    <section
      {...drop.props}
      className={s.parcel}
      data-made={parcel.made || undefined}
      data-state={parcel.state}
      data-kind={parcel.kind}
      aria-label={parcel.title}
    >
      <header {...drag.props} className={s.parcelHead}>
        <span className={s.parcelIcon} aria-hidden>
          {parcel.kind === "as_is" ? <Boxes /> : parcel.state === "sealed" ? <PackageCheck /> : parcel.made ? <PackageOpen /> : <Package />}
        </span>
        <span className={s.parcelName}>
          <span className={s.parcelTitle}>
            {parcel.count > 1 ? `${parcel.count} × ` : ""}
            {parcel.title}
          </span>
          <span className={s.note}>{parcelWords(parcel, fill, limit)}</span>
        </span>
        <span className={s.parcelBadges}>
          {parcel.carton?.breaks_down && <Badge>Breakdown pallet</Badge>}
          {!parcel.made && (parcel.kind !== "as_is" || parcel.planned) && <Badge>{parcel.byHand ? "Placed" : "Suggested"}</Badge>}
          <Badge tone={state.tone} dot={parcel.made}>
            {state.word}
          </Badge>
        </span>
      </header>

      {over && (
        <p className={s.dropVerdict} data-ok={over.ok || undefined} role="status">
          {over.verdict}
        </p>
      )}

      {fill !== null && parcel.kind === "box" && (
        <div className={s.fillBar} role="img" aria-label={`${fill}% full`}>
          <span style={{ width: `${Math.min(100, fill)}%` }} />
        </div>
      )}

      {parcel.kind !== "pallet" && parcel.goods.length > 0 && (
        <ul className={s.goods} aria-label={`What is ${parcel.made ? "in" : "to go in"} ${parcel.title}`}>
          {parcel.goods.map((g) => (
            <Goods
              key={g.line.line_id}
              line={g.line.line_id}
              code={g.line.item_code}
              units={g.units}
              parcel={parcel}
              takeOut={parcel.made && parcel.state === "open" ? () => acts.takeOut(g.line.line_id, parcel.id) : null}
              busy={busy}
            />
          ))}
        </ul>
      )}
      {parcel.kind === "box" && parcel.goods.length === 0 && <p className={s.dropHere}>Empty. Drop goods here.</p>}

      {parcel.kind === "pallet" &&
        (on.length > 0 ? (
          <div className={s.onPallet}>
            {on.map((p) => (
              <ParcelCard key={p.id} parcel={p} board={board} lines={lines} acts={acts} live={live} />
            ))}
          </div>
        ) : (
          <p className={s.dropHere}>Drop a carton here</p>
        ))}

      {parcel.made && parcel.carton && parcel.kind === "box" && <CartonFoot carton={parcel.carton} lines={lines} bench={acts.bench} />}

      {!parcel.made && parcel.kind === "box" && (
        <footer className={s.parcelFoot}>
          <span className={s.note}>{cutSaying(parcel.preset?.cut ?? null, parcel.preset?.size ?? null) ?? "Not made yet"}</span>
          <Button size="sm" icon={<PackageOpen />} disabled={busy || parcel.goods.length === 0} onClick={() => void acts.bench.make(parcel.id)}>
            Make it
          </Button>
        </footer>
      )}
      {parcel.kind === "as_is" && !parcel.made && parcel.asIs && (
        <footer className={s.parcelFoot}>
          <span className={s.note}>{needsCheck(parcel.goods[0]!.line) ? "Check it first: it is sealed as it is made" : "Made, filled and sealed as it is"}</span>
          {/* Planned onto a pallet, it is made from the plan, and goes on as made. */}
          <Button
            size="sm"
            icon={<Boxes />}
            disabled={busy || !acts.canShip(parcel)}
            onClick={() => (parcel.planned ? void acts.bench.make(parcel.id) : acts.ship(parcel))}
          >
            Ship {shipLabel(parcel.asIs)}
          </Button>
        </footer>
      )}
      {parcel.made && parcel.state === "sealed" && (
        <footer className={s.parcelFoot}>
          <Link variant="plain" href={href(`/print/packing-list/${parcel.id}`)} className={s.print}>
            <FileText aria-hidden /> Packing list
          </Link>
          {parcel.kind === "box" && (
            <Button size="sm" variant="ghost" disabled={busy} onClick={() => void acts.bench.open(parcel.id)}>
              Open it
            </Button>
          )}
        </footer>
      )}
    </section>
  );
}

/** A line's goods in a parcel: dragged out to another, or taken out onto the bench. */
function Goods({
  line,
  code,
  units,
  parcel,
  takeOut,
  busy,
}: {
  line: Uuid;
  code: string;
  units: number;
  parcel: BoardParcel;
  takeOut: (() => void) | null;
  busy: boolean;
}) {
  const drag = useDraggable(goodsKey(line, parcel.id));
  const movable = parcel.state !== "sealed" && parcel.kind !== "as_is";
  return (
    <li className={s.goodsRow} {...(movable ? drag.props : {})}>
      <span className={s.code}>{code}</span>
      <span className={s.goodsUnits}>{units}</span>
      {takeOut && (
        <Button size="sm" variant="ghost" disabled={busy} onClick={takeOut} aria-label={`Take ${code} out onto the bench`}>
          Take out
        </Button>
      )}
    </li>
  );
}

/** "Medium Box, cut to 28 · 58% full · 6.8 of 25 kg · On Carton 2 (PALLET)". */
function parcelWords(p: BoardParcel, fill: number | null, limit: number | null): string {
  const parts: string[] = [];
  if (p.detail && p.made) parts.push(p.detail);
  if (fill !== null && p.kind === "box") parts.push(`${fill}% full`);
  if (p.weight_g !== null) parts.push(limit ? `${kg(p.weight_g)} of ${kg(limit)}` : `${kg(p.weight_g)}${p.weighed ? " weighed" : ""}`);
  if (p.carton?.on) parts.push(`On ${onWhat(p.carton.on)}`);
  if (p.size && p.kind !== "box") parts.push(`${p.size.join(" × ")} mm`);
  return parts.join(" · ");
}

/** Where to drop for a new parcel: the smallest box that takes it, or a pallet (D245). */
export function DropSlot({ target, label }: { target: "new-box" | "new-pallet"; label: string }) {
  const drop = useDropTarget(target);
  return (
    <div {...drop.props} className={s.dropSlot}>
      {label}
    </div>
  );
}

/**
 * A made carton's weighing and sealing (D224, D244): its weight and height
 * typed, what it should weigh and whether the scale agrees, and Seal once what
 * is in it is checked. A sealed one says what it weighed.
 */
function CartonFoot({ carton, lines, bench }: { carton: CartonSummary; lines: BenchLine[]; bench: PackBench }) {
  const unchecked = uncheckedIn(carton, lines);
  const stated = carton.stated_size?.height_mm ?? null;
  const [weight, setWeight] = useState(carton.gross_weight_g === null ? "" : (carton.gross_weight_g / 1000).toFixed(3));
  const [height, setHeight] = useState(String(carton.height_mm ?? stated ?? ""));
  const cut = height.trim() !== "" && height.trim() !== String(stated ?? "");
  const footprint = carton.stated_size ? `${carton.stated_size.length_mm} × ${carton.stated_size.width_mm} mm` : null;

  if (carton.sealed) {
    return (
      <div className={s.body}>
        <Facts columns={4}>
          <Fact label="Gross">
            <span className={s.big}>{kg(carton.gross_weight_g)}</span>
          </Fact>
          {carton.expected && <ExpectedFact expected={carton.expected} contents={carton.contents} />}
          <Fact label="Height">{carton.height_mm !== null ? `${carton.height_mm} mm` : null}</Fact>
          <Fact label="Footprint">{footprint}</Fact>
        </Facts>
      </div>
    );
  }
  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        void bench.measure({
          carton: carton.id,
          ...(weight.trim() ? { weightKg: weight.trim() } : {}),
          ...(cut ? { heightMm: height.trim() } : {}),
        });
      }}
    >
      <div className={s.body}>
        {/* Until it is sealed, what to cut its box down to (D252). */}
        {cutSaying(carton.cut, carton.stated_size) && <p className={s.note}>{cutSaying(carton.cut, carton.stated_size)}</p>}
        <div className={s.measure}>
          <div className={s.measureField}>
            <TextField label="Weight" inputMode="decimal" trailing="kg" value={weight} onChange={(e) => setWeight(e.target.value)} />
          </div>
          <div className={s.measureField}>
            <TextField label="Height" inputMode="numeric" trailing="mm" value={height} onChange={(e) => setHeight(e.target.value)} />
          </div>
        </div>
        {carton.expected && (
          <Facts columns={1}>
            <ExpectedFact expected={carton.expected} contents={carton.contents} />
          </Facts>
        )}
      </div>
      <footer className={s.parcelFoot}>
        {carton.contents.length === 0 && (
          <Button size="sm" variant="ghost" icon={<Trash2 />} disabled={bench.busy} onClick={() => void bench.discard(carton.id)}>
            Discard
          </Button>
        )}
        {unchecked.length > 0 && <span className={s.note}>Check {unchecked.join(", ")} to seal it</span>}
        <Button size="sm" type="submit" disabled={bench.busy}>
          Record
        </Button>
        <Button size="sm" disabled={bench.busy || unchecked.length > 0 || carton.contents.length === 0} onClick={() => void bench.seal(carton.id)}>
          Seal
        </Button>
      </footer>
    </form>
  );
}

/** "the breakdown pallet", "Pallet 2": what a carton is on (D246, D247). */
function onWhat(on: OnParent): string {
  if (on.breaks_down) return "the breakdown pallet";
  return on.sequence ? `Pallet ${on.sequence}` : "a pallet";
}

export function sized(size: StatedSize): string {
  return `${size.length_mm} × ${size.width_mm} × ${size.height_mm} mm`;
}

/**
 * What this carton should weigh, and on what basis; once weighed, whether the
 * scale agrees within the site's tolerance, and if not what would explain it
 * (D244).
 */
function ExpectedFact({ expected, contents }: { expected: ExpectedWeight; contents: PackedRow[] }) {
  // Said once: by the verdict when it is off, else beside where it came from.
  const gap = expected.agrees === false ? null : agreement(expected);
  const why = explain(expected, contents);
  return (
    <Fact label="Expected">
      <span>
        <span className={s.verdict}>
          {kg(expected.grams)}
          {expected.agrees !== null && <Badge tone={expected.agrees ? "success" : "warning"}>{expected.agrees ? "Agrees" : offBy(expected)}</Badge>}
        </span>
        <span className={s.note}>
          {provenance(expected)}
          {gap ? ` · ${gap}` : ""}
        </span>
        {why.map((m) => (
          <span key={m.code} className={s.why}>
            {Math.abs(m.by)} {m.by > 0 ? "more" : "fewer"} of <span className={s.whole}>{m.code}</span> at {kg(m.each_g)}?
          </span>
        ))}
      </span>
    </Fact>
  );
}
