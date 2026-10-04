import { Suspense, lazy, useEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { Maximize, Minus, Plus, Redo2, RotateCcw, RotateCw, Rotate3d, Save, Trash2, Undo2 } from "lucide-react";

import { Alert, Button, Card, IconButton, Page, PageHeader, Select, Skeleton, TextField } from "@ui/index";
import type { Frame, LayoutPlace, PlanShape, Uuid } from "@domain/types";

import { moved, PRESETS, SITE, SNAP, turned, type Draft, type PlaceBox, type Point } from "./edit";
import type { PlanDesk } from "./usePlanEditor";
import s from "./plan-editor.module.css";

const Site3D = lazy(() => import("./Site3D"));

/**
 * The plan editor (D209): the site from above, to move, turn, resize, draw
 * and take away places, with the exact numbers beside it. Bins move with
 * their racks, because a bin's cell is a bay and a level of its place.
 *
 * Drag a place to move it, a half cell at a time. Drag the floor to look
 * around, and scroll to zoom. With a place chosen: the arrow keys nudge it, R
 * turns it a quarter, and Delete takes it away. Nothing is saved until Save.
 */
export function PlanEditorPage({ desk }: { desk: PlanDesk }) {
  const [shown3d, setShown3d] = useState(false);
  const read = desk.read;
  if (read.kind !== "ready") {
    return (
      <Page>
        <PageHeader title="Edit layout" />
        {read.kind === "failed" ? <Alert tone="danger">{read.message}</Alert> : <Skeleton width="50%" />}
      </Page>
    );
  }

  return (
    <Page>
      <PageHeader
        title="Edit layout"
        description="Move, turn and add places. Bins move with their racks."
        actions={
          <>
            <IconButton label="Undo" icon={<Undo2 />} disabled={!desk.canUndo || desk.busy} onClick={desk.undo} />
            <IconButton label="Redo" icon={<Redo2 />} disabled={!desk.canRedo || desk.busy} onClick={desk.redo} />
            <Button disabled={desk.count === 0 || desk.busy} onClick={desk.discard}>
              Discard
            </Button>
            <Button variant="primary" icon={<Save />} loading={desk.busy} disabled={desk.count === 0} onClick={() => void desk.save()}>
              {desk.count === 0 ? "Saved" : desk.count === 1 ? "Save 1 change" : `Save ${desk.count} changes`}
            </Button>
          </>
        }
      />
      {desk.problem && (
        <Alert tone="danger" onDismiss={desk.dismiss}>
          {desk.problem}
        </Alert>
      )}
      {desk.said && !desk.problem && (
        <Alert tone="success" onDismiss={desk.dismiss}>
          {desk.said}
        </Alert>
      )}

      <div className={s.editor}>
        <Card
          title="Plan"
          padded={false}
          actions={
            <Button size="sm" icon={<Rotate3d />} onClick={() => setShown3d((v) => !v)}>
              {shown3d ? "Hide 3D" : "Show 3D"}
            </Button>
          }
        >
          <div className={shown3d ? s.split : undefined}>
            <PlanCanvas desk={desk} />
            {shown3d && (
              <Suspense fallback={<div />}>
                <Site3D plan={desk.plan} places={gridsOf(read.value.places, desk.drafts)} chosen={desk.selected?.place_id ?? null} choose={desk.select} />
              </Suspense>
            )}
          </div>
        </Card>
        <aside>
          <Inspector desk={desk} />
        </aside>
      </div>
    </Page>
  );
}

/** The places' grids for the 3D view, under the names and sizes being edited. */
function gridsOf(places: LayoutPlace[], drafts: Draft[]): LayoutPlace[] {
  const now = new Map(drafts.map((d) => [d.place_id, d]));
  return places.filter((p) => now.has(p.place_id)).map((p) => ({ ...p, name: now.get(p.place_id)!.name }));
}

// ── the plan ────────────────────────────────────────────────────────────

type Gesture =
  | { kind: "drag"; id: Uuid; from: Point; box: PlaceBox; parent: Frame; begun: boolean }
  | { kind: "pan"; from: { x: number; y: number }; view: View; moved: boolean; on: Uuid | null };

/** What the plan shows, in its own units: the site's, with y turned up. */
type View = { x: number; y: number; w: number; h: number };

function fit(plan: PlanShape[]): View {
  const xs = plan.flatMap((sh) => sh.corners.map((c) => c[0]));
  const ys = plan.flatMap((sh) => sh.corners.map((c) => c[1]));
  if (xs.length === 0) return { x: -1, y: -11, w: 12, h: 12 };
  const [minX, maxX, minY, maxY] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  const pad = Math.max(maxX - minX, maxY - minY) * 0.04 + 1;
  return { x: minX - pad, y: -maxY - pad, w: maxX - minX + 2 * pad, h: maxY - minY + 2 * pad };
}

function PlanCanvas({ desk }: { desk: PlanDesk }) {
  const svg = useRef<SVGSVGElement>(null);
  const gesture = useRef<Gesture | null>(null);
  const [view, setView] = useState<View>(() => fit(desk.plan));
  const frames = useMemo(() => new Map(desk.plan.map((sh) => [sh.place_id, sh.frame])), [desk.plan]);
  const chosen = desk.selected?.place_id ?? null;
  const outermost = desk.plan.find((sh) => sh.nesting === 0)?.place_id ?? null;

  /** Where a pointer is, in site cells. */
  const site = (e: { clientX: number; clientY: number }): Point => {
    const el = svg.current;
    const m = el?.getScreenCTM();
    if (!el || !m) return [0, 0];
    const p = new DOMPoint(e.clientX, e.clientY).matrixTransform(m.inverse());
    return [p.x, -p.y];
  };
  const parentOf = (d: Draft): Frame => (d.parent_id ? (frames.get(d.parent_id) ?? SITE) : SITE);

  // Scrolling zooms about the pointer. Not React's handler, which is passive
  // and so can't stop the page scrolling instead.
  useEffect(() => {
    const el = svg.current;
    if (!el) return;
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      const [px, py] = site(e);
      const k = Math.exp(Math.max(-0.5, Math.min(0.5, e.deltaY * 0.0015)));
      setView((v) => zoomed(v, k, px, -py));
    };
    el.addEventListener("wheel", wheel, { passive: false });
    return () => el.removeEventListener("wheel", wheel);
  }, []);

  const down = (e: PointerEvent<SVGSVGElement>) => {
    if (e.button !== 0) return;
    const target = (e.target as Element).closest("[data-place]")?.getAttribute("data-place") ?? null;
    const d = target && target !== outermost ? desk.drafts.find((x) => x.place_id === target) : undefined;
    svg.current?.setPointerCapture(e.pointerId);
    if (d) {
      desk.select(d.place_id);
      gesture.current = { kind: "drag", id: d.place_id, from: site(e), box: d.box, parent: parentOf(d), begun: false };
    } else {
      gesture.current = { kind: "pan", from: { x: e.clientX, y: e.clientY }, view, moved: false, on: target };
    }
  };

  const move = (e: PointerEvent<SVGSVGElement>) => {
    const g = gesture.current;
    if (!g) return;
    if (g.kind === "drag") {
      const [x, y] = site(e);
      const box = moved(g.box, g.parent, x - g.from[0], y - g.from[1]);
      if (!g.begun && box.x === g.box.x && box.y === g.box.y) return;
      if (!g.begun) {
        desk.begin();
        g.begun = true;
      }
      desk.drag(g.id, box);
    } else {
      const el = svg.current;
      if (!el) return;
      const scale = g.view.w / el.clientWidth;
      const [dx, dy] = [(e.clientX - g.from.x) * scale, (e.clientY - g.from.y) * scale];
      if (Math.hypot(dx, dy) > 0) g.moved = true;
      setView({ ...g.view, x: g.view.x - dx, y: g.view.y - dy });
    }
  };

  const up = () => {
    const g = gesture.current;
    gesture.current = null;
    // A click on the floor chooses it, or nothing.
    if (g?.kind === "pan" && !g.moved) desk.select(g.on);
  };

  const key = (e: KeyboardEvent<SVGSVGElement>) => {
    const d = desk.selected;
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "z") {
      e.preventDefault();
      if (e.shiftKey) desk.redo();
      else desk.undo();
      return;
    }
    if (!d) return;
    const step = e.shiftKey ? SNAP * 10 : SNAP;
    const nudge: Record<string, Point> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] };
    const by = nudge[e.key];
    if (by) {
      e.preventDefault();
      desk.change(d.place_id, (x) => ({ ...x, box: moved(x.box, parentOf(x), by[0], by[1]) }));
    } else if (e.key === "r" || e.key === "R") {
      e.preventDefault();
      desk.change(d.place_id, (x) => ({ ...x, box: turned(x.box, e.shiftKey ? 90 : -90) }));
    } else if ((e.key === "Delete" || e.key === "Backspace") && d.bins === 0) {
      e.preventDefault();
      desk.remove(d.place_id);
    } else if (e.key === "Escape") {
      desk.select(null);
    }
  };

  // Outermost first, so what is inside is drawn over it, and the chosen last.
  const drawn = [...desk.plan].sort((a, b) => Number(a.place_id === chosen) - Number(b.place_id === chosen) || a.nesting - b.nesting);
  const picked = desk.plan.find((sh) => sh.place_id === chosen && sh.nesting > 0);
  const grid = fit(desk.plan);

  return (
    <div className={s.canvas}>
      <svg
        ref={svg}
        className={s.plan}
        viewBox={`${view.x} ${view.y} ${view.w} ${view.h}`}
        tabIndex={0}
        role="application"
        aria-label="The site's plan. Drag a place to move it; with one chosen, the arrow keys nudge it and R turns it."
        onPointerDown={down}
        onPointerMove={move}
        onPointerUp={up}
        onPointerCancel={up}
        onKeyDown={key}
      >
        <defs>
          <pattern id="plan-cells" width={1} height={1} patternUnits="userSpaceOnUse">
            <path d="M 1 0 L 0 0 0 1" className={s.cell} />
          </pattern>
        </defs>
        <rect x={grid.x} y={grid.y} width={grid.w} height={grid.h} fill="url(#plan-cells)" className={s.cells} />
        {drawn.map((sh) => (
          <polygon
            key={sh.place_id}
            data-place={sh.place_id}
            points={sh.corners.map(([x, y]) => `${x},${-y}`).join(" ")}
            vectorEffect="non-scaling-stroke"
            className={[
              s.shape,
              sh.nesting === 0 ? s.outer : sh.solid ? s.solid : s.floor,
              sh.nesting > 0 && s.movable,
              sh.place_id === chosen && s.chosen,
            ]
              .filter(Boolean)
              .join(" ")}
          >
            <title>{sh.name}</title>
          </polygon>
        ))}
        {drawn
          .filter((sh) => sh.nesting > 0)
          .map((sh) => {
            const [cx, cy] = middle(sh.corners);
            const size = Math.min(1, Math.max(0.35, shortSide(sh.corners) * 0.6));
            return (
              <text
                key={`n-${sh.place_id}`}
                x={cx}
                y={-cy}
                fontSize={size}
                transform={`rotate(${upright(sh.frame.turn)} ${cx} ${-cy})`}
                className={s.name}
                textAnchor="middle"
                dominantBaseline="central"
              >
                {sh.name}
              </text>
            );
          })}
        {picked && picked.corners.length === 4 && <Front shape={picked} sides={desk.selected?.sides ?? 1} />}
      </svg>
      <div className={s.tools}>
        <IconButton size="sm" label="Zoom in" icon={<Plus />} onClick={() => setView((v) => zoomed(v, 1 / 1.4))} />
        <IconButton size="sm" label="Zoom out" icon={<Minus />} onClick={() => setView((v) => zoomed(v, 1.4))} />
        <IconButton size="sm" label="Show the whole site" icon={<Maximize />} onClick={() => setView(fit(desk.plan))} />
      </div>
    </div>
  );
}

/**
 * Which side of the chosen place is its front: the face its first bins open
 * onto, and what turning it changes. A rack with two sides says so of its back.
 */
function Front({ shape, sides }: { shape: PlanShape; sides: number }) {
  const [a, b, c, d] = shape.corners as [Point, Point, Point, Point];
  const out = unit([a[0] - d[0], a[1] - d[1]]);
  const label = (p: Point, q: Point, normal: Point, text: string) => {
    const m: Point = [(p[0] + q[0]) / 2 + normal[0] * 1.1, (p[1] + q[1]) / 2 + normal[1] * 1.1];
    return (
      <text x={m[0]} y={-m[1]} fontSize={0.8} className={s.side} textAnchor="middle" dominantBaseline="central">
        {text}
      </text>
    );
  };
  return (
    <g pointerEvents="none">
      <line x1={a[0]} y1={-a[1]} x2={b[0]} y2={-b[1]} className={s.front} vectorEffect="non-scaling-stroke" />
      {label(a, b, out, "Front")}
      {sides === 2 && label(c, d, [-out[0], -out[1]], "Back")}
    </g>
  );
}

/** A name along its place, never upside down: the plan's y runs down, so the turn is negated. */
function upright(turn: number): number {
  const t = ((turn % 180) + 180) % 180;
  return t > 90 ? 180 - t : -t;
}

function zoomed(v: View, k: number, px = v.x + v.w / 2, py = v.y + v.h / 2): View {
  const w = Math.min(4000, Math.max(2, v.w * k));
  const f = w / v.w;
  return { x: px - (px - v.x) * f, y: py - (py - v.y) * f, w, h: v.h * f };
}

function middle(corners: [number, number][]): Point {
  const n = corners.length || 1;
  return [corners.reduce((t, c) => t + c[0], 0) / n, corners.reduce((t, c) => t + c[1], 0) / n];
}

function shortSide(corners: [number, number][]): number {
  if (corners.length < 3) return 1;
  const [a, b, c] = corners as [Point, Point, Point];
  return Math.min(Math.hypot(b[0] - a[0], b[1] - a[1]), Math.hypot(c[0] - b[0], c[1] - b[1]));
}

function unit([x, y]: Point): Point {
  const l = Math.hypot(x, y) || 1;
  return [x / l, y / l];
}

// ── the numbers ─────────────────────────────────────────────────────────

/** The chosen place's name, kind and box, or what can be added. */
function Inspector({ desk }: { desk: PlanDesk }) {
  const d = desk.selected;
  const outer = desk.drafts.find((x) => x.parent_id === null) ?? null;
  if (!d) {
    // A new place goes in the middle of the outermost place.
    const where: Point = outer ? [Math.round(outer.box.length / 2), Math.round(outer.box.depth / 2)] : [0, 0];
    return (
      <Card title="Add to the plan" description="Choose a place on the plan to move, turn or resize it, or add one here.">
        <div className={s.presets}>
          {PRESETS.map((p) => (
            <Button key={p.id} icon={<Plus />} onClick={() => desk.add(p, where)}>
              {p.name}
            </Button>
          ))}
        </div>
      </Card>
    );
  }

  const parent = desk.drafts.find((x) => x.place_id === d.parent_id);
  const set = (field: keyof PlaceBox) => (value: number) =>
    desk.change(d.place_id, (x) => ({ ...x, box: { ...x.box, [field]: value } }));
  const rack = d.sides === 2 || d.bins > 0;

  return (
    <Card title={d.name || "A place"} description={parent ? `Inside ${parent.name}, in its cells.` : "Standing on the site, in cells."}>
      <div className={s.fields}>
        <TextField label="Name" value={d.name} onChange={(e) => desk.change(d.place_id, (x) => ({ ...x, name: e.target.value }))} />
        <Select
          label="What it is"
          value={d.solid ? "solid" : "floor"}
          disabled={d.sides === 2}
          hint={d.sides === 2 ? "It has bins on two sides, so it stays solid." : undefined}
          options={[
            { value: "solid", label: "Solid: racking, a wall, a column" },
            { value: "floor", label: "Walk-through: floor, an area, a dock" },
          ]}
          onValueChange={(v) => desk.change(d.place_id, (x) => ({ ...x, solid: v === "solid" }))}
        />
        <div className={s.pair}>
          <NumberField label="Across" value={d.box.x} step={SNAP} onCommit={set("x")} />
          <NumberField label="Up the plan" value={d.box.y} step={SNAP} onCommit={set("y")} />
        </div>
        <div className={s.trio}>
          <NumberField label="Length" value={d.box.length} step={SNAP} min={0.1} disabled={!!d.outline} onCommit={set("length")} />
          <NumberField label="Depth" value={d.box.depth} step={SNAP} min={0.1} disabled={!!d.outline} onCommit={set("depth")} />
          <NumberField label="Height" value={d.box.height} step={SNAP} min={0.1} onCommit={set("height")} />
        </div>
        <div className={s.turn}>
          <span className={s.turnLabel}>Turned {d.box.turn}°</span>
          <IconButton label="Turn a quarter anticlockwise" icon={<RotateCcw />} onClick={() => desk.change(d.place_id, (x) => ({ ...x, box: turned(x.box, 90) }))} />
          <IconButton label="Turn a quarter clockwise" icon={<RotateCw />} onClick={() => desk.change(d.place_id, (x) => ({ ...x, box: turned(x.box, -90) }))} />
          <Button size="sm" onClick={() => desk.change(d.place_id, (x) => ({ ...x, box: turned(x.box, 180) }))}>
            Face the other way
          </Button>
        </div>
        {rack && <p className={s.note}>Its bins move with it: each is in a bay and a level of it, wherever it stands.</p>}
        {d.outline && <p className={s.note}>It has an outline of its own, so it is moved and turned, not resized.</p>}
        <div className={s.remove}>
          <Button
            variant="ghost"
            icon={<Trash2 />}
            disabled={d.bins > 0 || d.parent_id === null}
            onClick={() => desk.remove(d.place_id)}
          >
            Take it away
          </Button>
          {d.bins > 0 && <span className={s.note}>It holds bins or places, so it stays.</span>}
        </div>
      </div>
    </Card>
  );
}

/**
 * A number typed in full before it counts: "1." on the way to "1.5" isn't
 * read as 1. It counts on Enter, or on leaving the field.
 */
function NumberField({
  label,
  value,
  step,
  min,
  disabled,
  onCommit,
}: {
  label: string;
  value: number;
  step: number;
  min?: number | undefined;
  disabled?: boolean | undefined;
  onCommit: (value: number) => void;
}) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  const commit = () => {
    const n = Number.parseFloat(text);
    if (Number.isFinite(n) && (min === undefined || n >= min) && n !== value) onCommit(n);
    else setText(String(value));
  };
  return (
    <TextField
      label={label}
      type="number"
      inputMode="decimal"
      step={step}
      min={min}
      value={text}
      disabled={disabled}
      trailing="cells"
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
      }}
    />
  );
}
