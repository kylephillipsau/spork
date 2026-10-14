import { lazy, Suspense, useEffect, useState } from "react";
import { Check, ChevronLeft, ChevronRight, Copy } from "lucide-react";

import { Button, Card, Skeleton, Tabs, useToast } from "@ui/index";
import { imageUrl } from "@domain/api";
import type { Picture, Uuid } from "@domain/types";
import { kg } from "@app/common/format";
import { keep, recall } from "@app/common/remembered";
import { copyText } from "@app/common/copy";

import { contentsOf, type AsIs, type BoxPlan, type Dims, type Kind, type Layer } from "./arrange";
import type { Board, BoardParcel, Outcome } from "./board";
import { bookingCm, bookingText, gathered, type Freight } from "./freight";
import type { OrderView, Parcel } from "./order";
import type { ParcelShape } from "./pack3d";
import { tone } from "./tones";
import s from "./pack-bench.module.css";

const PackView = lazy(() => import("./PackView"));

/** The way this browser last showed a parcel: in 3D until somebody chooses its layers. */
const VIEW = "spork.pack.view";
const VIEWS = ["3d", "plan"] as const;
type View = (typeof VIEWS)[number];

/**
 * The right of the board (D245): the chosen parcel in 3D or layer by layer
 * from the bottom (D195), or the whole order side by side when nothing is
 * chosen (D202). While something is dragged over a parcel, that parcel as it
 * would be with it, what goes in drawn to go in, and the verdict. Under it,
 * the order as the carrier is handed it (D224).
 */
export function Chosen({
  board,
  parcel,
  outcome,
  pictures,
}: {
  board: Board;
  parcel: BoardParcel | null;
  /** The drop under the pointer, when something is dragged over somewhere. */
  outcome: Outcome | null;
  pictures: Map<Uuid, Picture | null>;
}) {
  const over = outcome?.shown ? (board.parcels.find((p) => p.id === outcome.shown!.parcel) ?? null) : null;
  const subject = over ?? parcel;
  const arranged: BoxPlan | null = (over ? outcome!.shown!.arranged : null) ?? subject?.arranged ?? null;
  const layers = arranged?.layers ?? [];
  const [view, setView] = useState<View>(() => recall(VIEW, VIEWS) ?? "3d");
  const [step, setStep] = useState(0);
  // A parcel chosen opens on its first layer with something still to put in.
  useEffect(() => setStep(nextLayer(layers)), [subject?.id]); // eslint-disable-line react-hooks/exhaustive-deps
  const at = over ? layers.length - 1 : Math.min(step, Math.max(0, layers.length - 1));
  const on = subject?.kind === "pallet" ? board.parcels.filter((p) => p.on === subject.id) : [];

  return (
    <Card
      title={subject ? subject.title : "The whole order"}
      description={subject ? describe(subject, arranged) : tally(board.view)}
      actions={
        arranged ? (
          <Tabs
            aria-label="Show it as"
            value={view}
            onValueChange={(v) => {
              setView(v as View);
              keep(VIEW, v);
            }}
            items={[
              { value: "3d", label: "3D" },
              { value: "plan", label: "Layers" },
            ]}
          />
        ) : undefined
      }
      padded={false}
    >
      <div className={s.suggestBody}>
        {outcome && (
          <p className={s.dropVerdict} data-ok={outcome.ok || undefined} role="status">
            {outcome.verdict}
          </p>
        )}
        {subject && arranged && subject.preset ? (
          <>
            {view === "plan" && layers[at] ? (
              <LayerPlan size={subject.preset.size} layer={layers[at]!} below={layers.slice(0, at)} pictures={pictures} />
            ) : (
              <Suspense fallback={<Skeleton height={280} />}>
                <PackView
                  groups={[{ size: dimsOf(subject.preset.size), layers, outline: true }]}
                  upTo={at + 1}
                  label={`${subject.title}, packed to layer ${at + 1} of ${layers.length}`}
                />
              </Suspense>
            )}
            {layers.length > 1 && !over && <Stepper layers={layers} at={at} setStep={setStep} />}
          </>
        ) : (
          <Suspense fallback={<Skeleton height={280} />}>
            <PackView
              groups={orderShapes(subject ? (subject.kind === "pallet" ? on : [subject]) : board.view.parcels)}
              upTo={Infinity}
              label={subject ? `${subject.title}, as it is` : `The order's ${board.view.parcels.length} parcels, side by side`}
            />
          </Suspense>
        )}
        <Booking freight={board.view.freight} />
      </div>
    </Card>
  );
}

/** Layer by layer, from the bottom: where it is up to, and each run of layers alike. */
function Stepper({ layers, at, setStep }: { layers: Layer[]; at: number; setStep: (n: number) => void }) {
  return (
    <>
      <div className={s.stepper}>
        <Button size="sm" variant="ghost" icon={<ChevronLeft />} disabled={at === 0} onClick={() => setStep(at - 1)} aria-label="Layer below" />
        <span className={s.stepName}>
          Layer {at + 1} of {layers.length}
          <span className={s.note}>{at === 0 ? "On the bottom" : `${layers[at]!.z}–${layers[at]!.z + layers[at]!.height} mm up`}</span>
        </span>
        <Button size="sm" variant="ghost" icon={<ChevronRight />} disabled={at >= layers.length - 1} onClick={() => setStep(at + 1)} aria-label="Layer above" />
      </div>
      <ol className={s.steps}>
        {runs(layers).map((run) => (
          <li key={run.from}>
            <button
              type="button"
              className={s.step}
              aria-current={at >= run.from && at <= run.to ? "step" : undefined}
              data-done={run.done || undefined}
              onClick={() => setStep(run.from)}
            >
              <span className={s.stepNumber}>{run.from === run.to ? run.from + 1 : `${run.from + 1}–${run.to + 1}`}</span>
              <span className={s.stepWhat}>
                {run.what}
                {run.from === run.to ? "" : ", in each"}
              </span>
            </button>
          </li>
        ))}
      </ol>
    </>
  );
}

const dimsOf = (size: { length_mm: number; width_mm: number; height_mm: number }): Dims => [size.length_mm, size.width_mm, size.height_mm];

/** "21 of 47 units packed · 26 planned · 4 to place · everything accounted for". */
export function tally(o: OrderView): string {
  const parts = [`${o.packed} of ${o.committed} units packed`];
  if (o.planned > 0) parts.push(`${o.planned} planned`);
  if (o.unplaced > 0) parts.push(`${o.unplaced} to place`);
  if (o.notBoxed > 0) parts.push(`${o.notBoxed} picked, not in a carton`);
  if (o.short > 0) parts.push(`${o.short} short`);
  if (o.missing > 0) parts.push(`${o.missing} not accounted for`);
  else if (o.unplaced === 0 && o.notBoxed === 0) parts.push(o.packed === o.committed ? "all packed" : "everything accounted for");
  return parts.join(" · ");
}

/** "Small Box, cut to 20 · 400 × 300 × 200 mm · 64% full · 1.080 kg by the record". */
function describe(p: BoardParcel, arranged: BoxPlan | null): string {
  const parts = [p.detail ?? (p.made ? null : p.byHand ? "Planned" : "Suggested")].filter((x): x is string => !!x);
  if (p.size) parts.push(`${p.size.join(" × ")} mm`);
  if (arranged) parts.push(`${Math.round(arranged.fill * 100)}% full`);
  if (p.weight_g !== null) parts.push(`${kg(p.weight_g)}${p.weighed ? " weighed" : " by the record"}`);
  return parts.join(" · ");
}

/** A closed thing to draw: one placement filling its own size. */
function closed(size: Dims, code: string, index: number, look: Pick<Parcel, "faces" | "round" | "keep">): ParcelShape {
  const kind: Kind = { line: "", item_id: "", item_code: code, level: "carton", units: 1, size, weight_g: null, ...look, upright: false, index };
  return { size, outline: false, layers: [{ z: 0, height: size[2], placements: [{ kind, x: 0, y: 0, z: 0, dims: size, axes: [0, 1, 2] }] }] };
}

/**
 * Parcels to draw side by side (D202): each box with what goes in it, each
 * sealed carton as a closed box with its number on it, and each thing that
 * ships as it is as itself, once for every parcel of it.
 */
function orderShapes(parcels: Parcel[]): ParcelShape[] {
  const out: ParcelShape[] = [];
  parcels.forEach((p, i) => {
    if (!p.size) return;
    if (p.layers && p.state !== "sealed") out.push({ size: p.size, layers: p.layers, outline: true });
    else if (p.asIs) for (let n = 0; n < p.count; n++) out.push(closed(p.size, p.asIs.item_code, p.index ?? i, p));
    else out.push(closed(p.size, p.state === "sealed" ? `${p.title} ✓` : p.title, i, p));
  });
  return out;
}

/**
 * The order's parcels as a booking takes them (D224): how many alike, their
 * size in whole centimetres and what one weighs, and Copy, which puts them on
 * the clipboard a parcel type to a line, in columns.
 */
function Booking({ freight }: { freight: Freight[] }) {
  const toast = useToast();
  const [copied, setCopied] = useState(false);
  const rows = gathered(freight);
  if (rows.length === 0) return null;
  const count = rows.reduce((t, r) => t + r.count, 0);
  const weighs = rows.every((r) => r.weight_g !== null) ? rows.reduce((t, r) => t + r.weight_g! * r.count, 0) : null;
  return (
    <div className={s.booking}>
      <span className={s.asidesTitle}>For the booking</span>
      <ul className={s.bookingRows} aria-label="Parcels for the booking">
        {rows.map((r) => (
          <li key={`${r.count}:${r.size?.join("x") ?? ""}:${r.weight_g ?? ""}`}>
            <span className={s.bookingCount}>{r.count} ×</span>
            <span>{r.size ? `${r.size.map(bookingCm).join(" × ")} cm` : "No size"}</span>
            <span className={s.bookingWeight}>{r.weight_g === null ? "not weighed" : kg(r.weight_g)}</span>
          </li>
        ))}
      </ul>
      <div className={s.bookingFoot}>
        <span className={s.note}>
          {count} {count === 1 ? "parcel" : "parcels"} · {weighs === null ? "some not weighed" : kg(weighs)}
        </span>
        <Button
          size="sm"
          icon={copied ? <Check /> : <Copy />}
          onClick={() =>
            void copyText(bookingText(freight)).then(
              () => setCopied(true),
              () => toast({ title: "Could not copy", description: "Select the rows and copy them by hand.", tone: "danger" }),
            )
          }
        >
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
    </div>
  );
}

/** The first layer with something still to put in, or the last when all of it is in. */
function nextLayer(layers: Layer[]): number {
  const i = layers.findIndex((l) => l.placements.some((p) => !p.packed));
  return i < 0 ? Math.max(0, layers.length - 1) : i;
}

/** "6 × APR-PE-CLR-L, 2 × HRH-2105W on top, 2 in": what to put in, in the order it goes. */
function saying(layer: Layer): string {
  return contentsOf(layer)
    .map(({ kind, count, stacked, packed }) => {
      const what = `${count} × ${kind.item_code}${kind.level === "inner" ? ` inner${count === 1 ? "" : "s"} of ${kind.units}` : ""}`;
      const placed = stacked === count ? `${what} on top` : stacked > 0 ? `${what} (${stacked} on top)` : what;
      // A layer all in is crossed out, which says it; a part-filled one says how much.
      return packed === 0 || packed === count ? placed : `${placed}, ${packed} in`;
    })
    .join("; ");
}

/** Layers that hold the same things the same height, said once: "1–6 · 1 × APR-PE-CLR-L, in each". */
function runs(layers: Layer[]): { from: number; to: number; what: string; done: boolean }[] {
  const out: { from: number; to: number; what: string; height: number; done: boolean }[] = [];
  layers.forEach((layer, i) => {
    const what = saying(layer);
    const done = layer.placements.every((p) => p.packed);
    const last = out[out.length - 1];
    if (last && last.what === what && last.height === layer.height && last.to === i - 1 && last.done === done) last.to = i;
    else out.push({ from: i, to: i, what, height: layer.height, done });
  });
  return out;
}

/**
 * One layer from above, to scale: each thing where it goes, with its picture,
 * and what is already in the box below it faint. The box's length runs across.
 */
function LayerPlan({
  size,
  layer,
  below,
  pictures,
}: {
  size: { length_mm: number; width_mm: number };
  layer: Layer;
  below: Layer[];
  pictures: Map<Uuid, Picture | null>;
}) {
  const { length_mm: L, width_mm: W } = size;
  const pad = Math.max(L, W) * 0.02;
  const text = Math.max(L, W) / 24;
  // Lower first, so what sits on top of something in the layer is drawn over it.
  const placed = [...layer.placements].sort((a, b) => a.z - b.z);
  return (
    <svg className={s.layerPlan} viewBox={`${-pad} ${-pad} ${L + 2 * pad} ${W + 2 * pad}`} role="img" aria-label={`Layer from above: ${saying(layer)}`}>
      <rect x={0} y={0} width={L} height={W} className={s.planBox} />
      {below.flatMap((l) => l.placements).map((p, i) => (
        <rect key={`b${i}`} x={p.x} y={p.y} width={p.dims[0]} height={p.dims[1]} className={s.planBelow} />
      ))}
      {placed.map((p, i) => {
        const picture = pictures.get(p.kind.item_id) ?? null;
        const t = tone(p.kind.index);
        const inset = Math.min(p.dims[0], p.dims[1]) * 0.08;
        const onTop = p.z > layer.z;
        return (
          <g key={i} className={s.planThing} data-tone={t} data-on-top={onTop || undefined} data-packed={p.packed || undefined}>
            <rect x={p.x} y={p.y} width={p.dims[0]} height={p.dims[1]} />
            {picture && (
              <image
                href={imageUrl(picture.digest)}
                x={p.x + inset}
                y={p.y + inset}
                width={p.dims[0] - 2 * inset}
                height={p.dims[1] - 2 * inset}
                preserveAspectRatio="xMidYMid meet"
              />
            )}
            {Math.min(p.dims[0], p.dims[1]) > text * 1.6 && (
              <text x={p.x + p.dims[0] / 2} y={p.y + p.dims[1] - text * 0.5} fontSize={text} textAnchor="middle">
                {p.packed ? "✓ " : ""}
                {p.kind.item_code}
                {onTop ? " ↑" : ""}
              </text>
            )}
          </g>
        );
      })}
    </svg>
  );
}

/** "2 own cartons of 6", "3 inner packs of 24", "3 as they are": what Ship will do, short enough for a button. */
export function shipLabel(a: AsIs): string {
  const s = a.count === 1 ? "" : "s";
  if (a.level === "carton") return `${a.count} own carton${s} of ${a.per}`;
  if (a.level === "inner") return `${a.count} inner pack${s} of ${a.per}`;
  return `${a.count} as ${a.count === 1 ? "it is" : "they are"}`;
}
