import { lazy, Suspense, useEffect, useMemo, useState, type ReactNode } from "react";
import { Boxes, Check, ChevronLeft, ChevronRight, Circle, CircleCheck, Copy, Package, PackageCheck, PackageOpen, Ruler, TriangleAlert } from "lucide-react";

import { Badge, Button, Card, Skeleton, Tabs, Toolbar, Spacer, useToast } from "@ui/index";
import { imageUrl } from "@domain/api";
import type { BenchScreen, Picture, Uuid } from "@domain/types";
import { Thumb } from "@app/common/Thumb";
import { kg } from "@app/common/format";
import { keep, recall } from "@app/common/remembered";
import { copyText } from "@app/common/copy";

import { arrange, contentsOf, type Aside, type AsIs, type BoxPlan, type Dims, type Kind, type Layer, type OpenCarton } from "./arrange";
import { bookingCm, bookingText, gathered, type Freight } from "./freight";
import { wholeOrder, type OrderLine, type OrderView, type Parcel, type ParcelState } from "./order";
import type { ParcelShape } from "./pack3d";
import { tone } from "./tones";
import type { PackBench } from "./usePackBench";
import s from "./pack-bench.module.css";

const PackView = lazy(() => import("./PackView"));

/** The plan as this browser last showed it: in 3D until somebody chooses the list. */
const VIEW = "spork.pack.view";
const VIEWS = ["3d", "plan"] as const;
type View = (typeof VIEWS)[number];

/**
 * The whole order as it will leave, checked off as it goes (D202), and the box
 * for what is left on the bench and how it goes in (D195), layer by
 * layer from the bottom, in 3D or as a plan from above. Made from what is
 * recorded: what has no size is listed with a way to measure it, and the
 * suggestion is made again when the drawer closes.
 *
 * **The carton being filled comes first** (D198): what is in it and what is
 * left, arranged together, with what is in already shown as done and the plan
 * on the next layer that has something to put in.
 */
export function Suggestion({
  screen,
  bench,
  look,
  ship,
  canShip,
}: {
  screen: BenchScreen;
  bench: PackBench;
  look: (itemId: Uuid) => void;
  /** Ship these as they are, as the line's own button would (D196). */
  ship: (a: AsIs) => void;
  canShip: (line: Uuid) => boolean;
}) {
  const open = useMemo(() => openOf(screen, bench.openCarton), [screen, bench.openCarton]);
  const plan = useMemo(() => arrange(screen.lines, screen.presets, open), [screen.lines, screen.presets, open]);
  const order = useMemo(() => wholeOrder(screen, plan), [screen, plan]);
  const shapes = useMemo(() => orderShapes(order), [order]);
  const pictures = useMemo(() => new Map(screen.lines.map((l) => [l.item_id, l.picture])), [screen.lines]);
  // The whole order first (D202); a box's own layers one tab along.
  const [which, setWhich] = useState<"order" | number>("order");
  const [step, setStep] = useState(0);
  // 3D first: what the goods will look like packed, at a glance.
  const [view, setView] = useState<View>(() => recall(VIEW, VIEWS) ?? "3d");

  const box: BoxPlan | undefined = which === "order" ? undefined : plan.boxes[Math.min(which, plan.boxes.length - 1)];
  const layers = box?.layers ?? [];
  const at = Math.min(step, Math.max(0, layers.length - 1));
  // Filling a carton, each press moves its plan on to what to put in next.
  const done = plan.boxes[0]?.carton ? plan.boxes[0].layers.flatMap((l) => l.placements).filter((p) => p.packed).length : -1;
  useEffect(() => {
    if (done < 0) return;
    setStep(nextLayer(plan.boxes[0]!.layers));
  }, [done]); // eslint-disable-line react-hooks/exhaustive-deps
  const choose = (n: "order" | number) => {
    setWhich(n);
    setStep(n === "order" ? 0 : nextLayer(plan.boxes[n]?.layers ?? []));
  };

  if (order.lines.length === 0) return null;

  return (
    <Card
      title="Packing plan"
      description={box ? describe(box) : tally(order)}
      actions={
        <Tabs
          aria-label="Show it as"
          value={view}
          onValueChange={(v) => {
            setView(v as View);
            keep(VIEW, v);
          }}
          items={[
            { value: "3d", label: "3D" },
            { value: "plan", label: box ? "Layers" : "List" },
          ]}
        />
      }
      padded={false}
    >
      {plan.boxes.length > 0 && (
        <div className={s.suggestTabs}>
          <Tabs
            aria-label="Parcel"
            value={String(which)}
            onValueChange={(v) => choose(v === "order" ? "order" : Number(v))}
            items={[
              { value: "order", label: "Whole order" },
              ...plan.boxes.map((b, i) => ({
                value: String(i),
                label: b.carton ? `Carton ${b.carton.sequence} · ${b.preset.name} (open)` : `${b.preset.name} (to start)`,
              })),
            ]}
          />
        </div>
      )}

      {!box && (
        <div className={s.suggestBody}>
          {view === "3d" && shapes.length > 0 ? (
            <Suspense fallback={<Skeleton height={280} />}>
              <PackView groups={shapes} upTo={Infinity} label={`The order's ${shapes.length} parcels, side by side`} />
            </Suspense>
          ) : (
            <WholeOrder
              order={order}
              pictures={pictures}
              ship={ship}
              canShip={canShip}
              show={(n) => choose(n)}
              start={(id) => void bench.startCarton(id)}
              canStart={!bench.busy && !!screen.dock_id}
              boxes={plan.boxes}
            />
          )}
          <Booking freight={order.freight} />
        </div>
      )}

      {box && (
        <div className={s.suggestBody}>
          {view === "plan" ? (
            <LayerPlan box={box} layer={layers[at]!} below={layers.slice(0, at)} pictures={pictures} />
          ) : (
            <Suspense fallback={<Skeleton height={280} />}>
              <PackView
                groups={[{ size: [box.preset.size.length_mm, box.preset.size.width_mm, box.preset.size.height_mm], layers, outline: true }]}
                upTo={at + 1}
                label={`${box.preset.name}, packed to layer ${at + 1} of ${layers.length}`}
              />
            </Suspense>
          )}

          <div className={s.stepper}>
            <Button size="sm" variant="ghost" icon={<ChevronLeft />} disabled={at === 0} onClick={() => setStep(at - 1)} aria-label="Layer below" />
            <span className={s.stepName}>
              Layer {at + 1} of {layers.length}
              <span className={s.note}>{at === 0 ? "On the bottom" : `${layers[at]!.z}–${layers[at]!.z + layers[at]!.height} mm up`}</span>
            </span>
            <Button
              size="sm"
              variant="ghost"
              icon={<ChevronRight />}
              disabled={at >= layers.length - 1}
              onClick={() => setStep(at + 1)}
              aria-label="Layer above"
            />
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
          {box.carton && (
            <p className={s.aside}>
              {layers.every((l) => l.placements.every((p) => p.packed))
                ? `Everything planned for carton ${box.carton.sequence} is in it.`
                : `Put in what isn't crossed out, layer by layer, into carton ${box.carton.sequence}.`}
            </p>
          )}
          {box.loose.length > 0 && (
            <p className={s.aside}>
              Round the rest: {listed(box.loose)}. {box.loose.length === 1 ? "It has" : "They have"} no size to measure.
            </p>
          )}
        </div>
      )}

      <Asides plan={plan} pictures={pictures} look={look} />

      {box && !box.carton && (
        <Toolbar placement="bottom">
          <span className={s.note}>Not started yet</span>
          <Spacer />
          {/* Secondary: Start carton below is the bench's one primary act. */}
          <Button
            variant="secondary"
            icon={<PackageOpen />}
            disabled={bench.busy || !screen.dock_id}
            onClick={() => void bench.startCarton(box.preset.id)}
          >
            Start {box.preset.name}
          </Button>
        </Toolbar>
      )}
    </Card>
  );
}

/** "21 of 47 units packed · 26 planned · everything accounted for". */
function tally(o: OrderView): string {
  const parts = [`${o.packed} of ${o.committed} units packed`];
  if (o.planned > 0) parts.push(`${o.planned} planned`);
  if (o.unplaced > 0) parts.push(`${o.unplaced} can't be placed yet`);
  if (o.notBoxed > 0) parts.push(`${o.notBoxed} picked, not in a carton`);
  if (o.missing > 0) parts.push(`${o.missing} not accounted for`);
  else if (o.unplaced === 0 && o.notBoxed === 0) parts.push(o.packed === o.committed ? "all packed" : "everything accounted for");
  return parts.join(" · ");
}

/** A closed thing to draw: one placement filling its own size. */
function closed(size: Dims, code: string, index: number, faces: Kind["faces"]): ParcelShape {
  const kind: Kind = { line: "", item_id: "", item_code: code, level: "carton", units: 1, size, weight_g: null, faces, upright: false, index };
  return { size, outline: false, layers: [{ z: 0, height: size[2], placements: [{ kind, x: 0, y: 0, z: 0, dims: size, axes: [0, 1, 2] }] }] };
}

/**
 * The whole order to draw side by side (D202): each box with what goes in it,
 * each carton nobody is filling as a closed box with its number on it, and each
 * thing that ships as it is as itself, once for every parcel of it.
 */
function orderShapes(o: OrderView): ParcelShape[] {
  const out: ParcelShape[] = [];
  o.parcels.forEach((p, i) => {
    if (!p.size) return;
    if (p.layers) out.push({ size: p.size, layers: p.layers, outline: true });
    else if (p.asIs) for (let n = 0; n < p.count; n++) out.push(closed(p.size, p.asIs.item_code, p.index ?? i, p.faces));
    else out.push(closed(p.size, p.state === "sealed" ? `${p.title} ✓` : p.title, i, p.faces));
  });
  return out;
}

const STATE_WORDS: Record<ParcelState, string> = { sealed: "Sealed", open: "Open", planned: "To start", "as-is": "Ships as it is" };

/**
 * The whole order, checked off (D202): every line with how much of it is in a
 * carton and where the rest is going, ticked when all of it is packed; then
 * every parcel the order leaves as, with what is in it and the press for it.
 */
function WholeOrder({
  order,
  pictures,
  ship,
  canShip,
  show,
  start,
  canStart,
  boxes,
}: {
  order: OrderView;
  pictures: Map<Uuid, Picture | null>;
  ship: (a: AsIs) => void;
  canShip: (line: Uuid) => boolean;
  show: (box: number) => void;
  start: (presetId: Uuid) => void;
  canStart: boolean;
  boxes: BoxPlan[];
}) {
  return (
    <div className={s.order}>
      <ul className={s.orderLines} aria-label="The order's lines">
        {order.lines.map((l) => {
          const trouble = l.missing > 0 || l.unplaced > 0 || l.notBoxed > 0;
          return (
            <li key={l.line} className={s.orderLine} data-done={l.done || undefined} data-trouble={(!l.done && trouble) || undefined}>
              <span className={s.orderMark} aria-label={l.done ? "Packed" : trouble ? "Needs attention" : "Not packed yet"} role="img">
                {l.done ? <CircleCheck /> : trouble ? <TriangleAlert /> : <Circle />}
              </span>
              <Thumb picture={pictures.get(l.item_id) ?? null} alt={l.item_code} />
              <span className={s.asideWhat}>
                <span className={s.code}>{l.item_code}</span>
                <span className={s.note}>{whereWords(l)}</span>
              </span>
              <span className={s.orderCount}>
                {l.packed} / {l.committed}
              </span>
            </li>
          );
        })}
      </ul>

      <span className={s.asidesTitle}>Parcels</span>
      <ul className={s.asideList} aria-label="Parcels">
        {order.parcels.map((p) => (
          <li key={p.key} className={s.parcelRow}>
            <span className={s.parcelIcon} aria-hidden>
              {p.state === "sealed" ? <PackageCheck /> : p.state === "open" ? <PackageOpen /> : p.state === "as-is" ? <Boxes /> : <Package />}
            </span>
            <span className={s.asideWhat}>
              <span className={s.code}>
                {p.count > 1 ? `${p.count} × ` : ""}
                {p.title}
              </span>
              <span className={s.note}>{[p.detail, contentsWords(p)].filter(Boolean).join(" · ")}</span>
              <span className={s.note}>{freightWords(p)}</span>
            </span>
            <Badge tone={p.state === "sealed" ? "success" : p.state === "open" ? "accent" : "neutral"}>{STATE_WORDS[p.state]}</Badge>
            {p.asIs ? (
              <span className={s.parcelAct}>
                <Button size="sm" icon={<Boxes />} disabled={!canShip(p.asIs.line)} onClick={() => ship(p.asIs!)}>
                  Ship {shipLabel(p.asIs)}
                </Button>
              </span>
            ) : p.box !== null ? (
              <span className={s.parcelAct}>
                {p.state === "planned" ? (
                  <Button size="sm" icon={<PackageOpen />} disabled={!canStart} onClick={() => start(boxes[p.box!]!.preset.id)}>
                    Start {p.title}
                  </Button>
                ) : (
                  <Button size="sm" variant="ghost" onClick={() => show(p.box!)}>
                    Its layers
                  </Button>
                )}
              </span>
            ) : null}
          </li>
        ))}
      </ul>
    </div>
  );
}

/** "400 × 300 × 190 mm · 4.200 kg weighed", or what the record's weight leaves out. */
function freightWords(p: Parcel): string {
  const size = p.size ? `${p.size.join(" × ")} mm` : "No size";
  const weight = p.weight_g === null ? "not weighed" : `${kg(p.weight_g)}${p.weighed ? " weighed" : ""}`;
  const note = p.weightNote && p.weight_g !== null ? ` (${p.weightNote})` : "";
  return `${size} · ${p.count > 1 ? `${weight} each` : weight}${note}`;
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

/** "Carton 1 · 8; small box, to go in · 4", and what is not anywhere. */
function whereWords(l: OrderLine): string {
  const parts = l.where.map((w) => `${w.parcel}${w.state === "planned" ? ", to go in" : ""} · ${w.units}`);
  if (l.unplaced > 0) parts.push(`${l.unplaced} can't be placed yet`);
  if (l.notBoxed > 0) parts.push(`${l.notBoxed} picked, not in a carton`);
  if (l.missing > 0) parts.push(`${l.missing} not accounted for`);
  return parts.length > 0 ? parts.join("; ") : "Nothing yet";
}

/** "8 × GLV-NIT-BLU-M in; 4 × APR-PE-CLR-L to go in", and what goes round the rest. */
function contentsWords(p: Parcel): string {
  const parts = p.lines.map((r) => `${r.units} × ${r.item_code}${r.state === "planned" && p.state !== "as-is" ? " to go in" : ""}`);
  for (const a of p.loose) parts.push(`${a.units} × ${a.item_code} round the rest`);
  return parts.length > 0 ? parts.join("; ") : "Empty";
}

/** The carton open on the bench, when its box has a size to fill (D198). A pallet or a product's own carton has none. */
function openOf(screen: BenchScreen, id: Uuid | null): OpenCarton | null {
  const c = screen.cartons.find((x) => x.id === id && !x.sealed);
  if (!c || c.own_carton_of || !c.stated_size || !c.package_type) return null;
  const preset = screen.presets.find((p) => p.name === c.package_type);
  if (!preset?.size) return null;
  return {
    id: c.id,
    sequence: c.sequence,
    name: c.package_type,
    size: c.stated_size,
    max_payload_g: preset.max_payload_g,
    tare_weight_g: preset.tare_weight_g,
    contents: c.contents.map((r) => ({ item_id: r.item_id, quantity: r.quantity })),
  };
}

/** The first layer with something still to put in, or the last when all of it is in. */
function nextLayer(layers: Layer[]): number {
  const i = layers.findIndex((l) => l.placements.some((p) => !p.packed));
  return i < 0 ? Math.max(0, layers.length - 1) : i;
}

/** "Carton 2, open · Small Box · 400 × 300 × 190 mm · 64% full · 1.08 kg by the record". */
function describe(box: BoxPlan): string {
  const { length_mm, width_mm, height_mm } = box.preset.size;
  const parts = [
    ...(box.carton ? [`Carton ${box.carton.sequence}, open`] : []),
    box.preset.name,
    `${length_mm} × ${width_mm} × ${height_mm} mm`,
    `${Math.round(box.fill * 100)}% full`,
  ];
  if (box.weight_g > 0) parts.push(`${kg(box.weight_g)} by the record${box.unweighed > 0 ? `, ${box.unweighed} not weighed` : ""}`);
  return parts.join(" · ");
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

function listed(asides: Aside[]): string {
  return asides.map((a) => `${a.units} × ${a.item_code}`).join(", ");
}

/**
 * One layer from above, to scale: each thing where it goes, with its picture,
 * and what is already in the box below it faint. The box's length runs across.
 */
function LayerPlan({ box, layer, below, pictures }: { box: BoxPlan; layer: Layer; below: Layer[]; pictures: Map<Uuid, Picture | null> }) {
  const { length_mm: L, width_mm: W } = box.preset.size;
  const pad = Math.max(L, W) * 0.02;
  const text = Math.max(L, W) / 24;
  // Lower first, so what sits on top of something in the layer is drawn over it.
  const placed = [...layer.placements].sort((a, b) => a.z - b.z);
  return (
    <svg
      className={s.layerPlan}
      viewBox={`${-pad} ${-pad} ${L + 2 * pad} ${W + 2 * pad}`}
      role="img"
      aria-label={`Layer from above: ${saying(layer)}`}
    >
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

/** "2 own cartons of 6", "3 inner packs of 24", "3 as they are": what the press will do, short enough for a column. */
export function shipLabel(a: AsIs): string {
  const s = a.count === 1 ? "" : "s";
  if (a.level === "carton") return `${a.count} own carton${s} of ${a.per}`;
  if (a.level === "inner") return `${a.count} inner pack${s} of ${a.per}`;
  return `${a.count} as ${a.count === 1 ? "it is" : "they are"}`;
}

/** What the suggestion leaves out, and why: each with what to do about it. */
function Asides({ plan, pictures, look }: { plan: ReturnType<typeof arrange>; pictures: Map<Uuid, Picture | null>; look: (itemId: Uuid) => void }) {
  if (plan.tooMany) {
    return <p className={s.aside}>More pieces than are worth arranging one by one: this is a pallet job.</p>;
  }
  const rows: { key: string; aside: Aside; why: ReactNode; act?: ReactNode }[] = [
    ...plan.unmeasured.map((a) => ({
      key: `m${a.line}`,
      aside: a,
      why: <Badge tone="warning">No size</Badge>,
      act: (
        <Button size="sm" variant="ghost" icon={<Ruler />} onClick={() => look(a.item_id)}>
          Measure
        </Button>
      ),
    })),
    ...plan.oversize.map((a) => ({ key: `o${a.line}`, aside: a, why: <Badge tone="danger">Too big for every box</Badge> })),
    ...plan.loose.map((a) => ({ key: `l${a.line}`, aside: a, why: <Badge>No size to measure</Badge> })),
  ];
  if (rows.length === 0) return null;
  // Loose goods to box by hand: what they weigh, so a box of them can be weighed against it (D224).
  const units = rows.reduce((t, r) => t + r.aside.units, 0);
  const weighed = rows.filter((r) => r.aside.weight_g !== null);
  const weight = weighed.reduce((t, r) => t + r.aside.weight_g!, 0);
  const unweighed = units - weighed.reduce((t, r) => t + r.aside.units, 0);
  return (
    <div className={s.asides}>
      <span className={s.asidesTitle}>Not in the suggestion</span>
      <span className={s.note}>
        {units} {units === 1 ? "unit" : "units"} to box by hand
        {weighed.length === 0 ? ", not weighed" : ` · ${kg(weight)}${unweighed > 0 ? `, and ${unweighed} not weighed` : ""}`}
      </span>
      <ul className={s.asideList}>
        {rows.map(({ key, aside, why, act }) => (
          <li key={key} className={s.asideRow}>
            <Thumb picture={pictures.get(aside.item_id) ?? null} alt={aside.item_code} />
            <span className={s.asideWhat}>
              <span className={s.code}>{aside.item_code}</span>
              <span className={s.note}>
                {aside.units} units · {aside.weight_g === null ? "not weighed" : kg(aside.weight_g)}
              </span>
            </span>
            {why}
            {act}
          </li>
        ))}
      </ul>
    </div>
  );
}
