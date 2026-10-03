import { lazy, Suspense, useMemo, useState, type ReactNode } from "react";
import { ChevronLeft, ChevronRight, PackageOpen, Ruler } from "lucide-react";

import { Badge, Button, Card, Skeleton, Tabs, Toolbar, Spacer } from "@ui/index";
import { imageUrl } from "@domain/api";
import type { BenchScreen, Picture, Uuid } from "@domain/types";
import { Thumb } from "@app/common/Thumb";
import { kg } from "@app/common/format";

import { arrange, contentsOf, type Aside, type BoxPlan, type Layer } from "./arrange";
import { tone } from "./tones";
import type { PackBench } from "./usePackBench";
import s from "./pack-bench.module.css";

const PackView = lazy(() => import("./PackView"));

/**
 * The box for what is left on the bench and how it goes in (D195), layer by
 * layer from the bottom, as a plan from above or in 3D. Made from what is
 * recorded: what has no size is listed with a way to measure it, and the
 * suggestion is made again when the drawer closes.
 */
export function Suggestion({ screen, bench, look }: { screen: BenchScreen; bench: PackBench; look: (itemId: Uuid) => void }) {
  const plan = useMemo(() => arrange(screen.lines, screen.presets), [screen.lines, screen.presets]);
  const pictures = useMemo(() => new Map(screen.lines.map((l) => [l.item_id, l.picture])), [screen.lines]);
  const [which, setWhich] = useState(0);
  const [step, setStep] = useState(0);
  const [view, setView] = useState<"plan" | "3d">("plan");

  const anything = plan.boxes.length + plan.unmeasured.length + plan.oversize.length + plan.loose.length > 0 || plan.tooMany;
  if (!anything) return null;

  const box: BoxPlan | undefined = plan.boxes[Math.min(which, plan.boxes.length - 1)];
  const layers = box?.layers ?? [];
  const at = Math.min(step, Math.max(0, layers.length - 1));
  const choose = (n: number) => {
    setWhich(n);
    setStep(0);
  };

  return (
    <Card
      title="Suggested packing"
      description={box ? describe(box) : "Nothing here can be arranged yet."}
      actions={
        box && (
          <Tabs
            aria-label="Show the arrangement as"
            value={view}
            onValueChange={(v) => setView(v as "plan" | "3d")}
            items={[
              { value: "plan", label: "Layers" },
              { value: "3d", label: "3D" },
            ]}
          />
        )
      }
      padded={false}
    >
      {plan.boxes.length > 1 && (
        <div className={s.suggestTabs}>
          <Tabs
            aria-label="Box"
            value={String(which)}
            onValueChange={(v) => choose(Number(v))}
            items={plan.boxes.map((b, i) => ({ value: String(i), label: `${i + 1} · ${b.preset.name}` }))}
          />
        </div>
      )}

      {box && (
        <div className={s.suggestBody}>
          {view === "plan" ? (
            <LayerPlan box={box} layer={layers[at]!} below={layers.slice(0, at)} pictures={pictures} />
          ) : (
            <Suspense fallback={<Skeleton height={280} />}>
              <PackView
                size={[box.preset.size.length_mm, box.preset.size.width_mm, box.preset.size.height_mm]}
                layers={layers}
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
          {box.loose.length > 0 && (
            <p className={s.aside}>
              Round the rest: {listed(box.loose)}. {box.loose.length === 1 ? "It has" : "They have"} no size to measure.
            </p>
          )}
        </div>
      )}

      <Asides plan={plan} pictures={pictures} look={look} />

      {box && (
        <Toolbar placement="bottom">
          <span className={s.note}>{plan.boxes.length > 1 ? `${plan.boxes.length} boxes for what is left` : "One box for what is left"}</span>
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

/** "Small Box · 400 × 300 × 190 mm · 64% full · 1.08 kg by the record". */
function describe(box: BoxPlan): string {
  const { length_mm, width_mm, height_mm } = box.preset.size;
  const parts = [box.preset.name, `${length_mm} × ${width_mm} × ${height_mm} mm`, `${Math.round(box.fill * 100)}% full`];
  if (box.weight_g > 0) parts.push(`${kg(box.weight_g)} by the record${box.unweighed > 0 ? `, ${box.unweighed} not weighed` : ""}`);
  return parts.join(" · ");
}

/** "6 × APR-PE-CLR-L, 2 × HRH-2105W on top": what to put in, in the order it goes. */
function saying(layer: Layer): string {
  return contentsOf(layer)
    .map(({ kind, count, stacked }) => {
      const what = `${count} × ${kind.item_code}${kind.level === "inner" ? ` inner${count === 1 ? "" : "s"} of ${kind.units}` : ""}`;
      return stacked === count ? `${what} on top` : stacked > 0 ? `${what} (${stacked} on top)` : what;
    })
    .join(", ");
}

/** Layers that hold the same things the same height, said once: "1–6 · 1 × APR-PE-CLR-L, in each". */
function runs(layers: Layer[]): { from: number; to: number; what: string }[] {
  const out: { from: number; to: number; what: string; height: number }[] = [];
  layers.forEach((layer, i) => {
    const what = saying(layer);
    const last = out[out.length - 1];
    if (last && last.what === what && last.height === layer.height && last.to === i - 1) last.to = i;
    else out.push({ from: i, to: i, what, height: layer.height });
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
          <g key={i} className={s.planThing} data-tone={t} data-on-top={onTop || undefined}>
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
    ...plan.own.map((a) => ({
      key: `c${a.line}`,
      aside: a,
      why: <Badge tone="info">{`${a.cartons} own carton${a.cartons === 1 ? "" : "s"}`}</Badge>,
    })),
  ];
  if (rows.length === 0) return null;
  return (
    <div className={s.asides}>
      <span className={s.asidesTitle}>Not in the suggestion</span>
      <ul className={s.asideList}>
        {rows.map(({ key, aside, why, act }) => (
          <li key={key} className={s.asideRow}>
            <Thumb picture={pictures.get(aside.item_id) ?? null} alt={aside.item_code} />
            <span className={s.asideWhat}>
              <span className={s.code}>{aside.item_code}</span>
              <span className={s.note}>{aside.units} units</span>
            </span>
            {why}
            {act}
          </li>
        ))}
      </ul>
    </div>
  );
}
