import { Suspense, lazy, useEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { Maximize, Minus, Plus, Redo2, RotateCcw, RotateCw, Rotate3d, Save, Trash2, Undo2 } from "lucide-react";

import { Alert, Button, Card, Checkbox, IconButton, Page, PageHeader, Select, Skeleton, TextField } from "@ui/index";
import type { Frame, LayoutPlace, PlanShape, Uuid } from "@domain/types";

import {
  at,
  boundsOf,
  byName,
  cleared,
  clearances,
  inRow,
  makeOf,
  moved,
  PRESETS,
  sameMake,
  shown,
  SITE,
  sizedFrom,
  stepOf,
  stored,
  turned,
  unitOf,
  type Clearance,
  type Draft,
  type PlaceBox,
  type Point,
  type RowWay,
} from "./edit";
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

      {desk.cellMm === null && (
        <Card
          title="Measure in metres"
          description="The plan isn't to scale yet: it was drafted from the bin codes. In metres, each cell on the plan is a metre, and everything you type or measure is in metres. The racks keep their drawn sizes until you set them. The scale stays once it is set."
          actions={
            <Button variant="secondary" loading={desk.busy} onClick={() => void desk.measureInMetres()}>
              Measure in metres
            </Button>
          }
        >
          {null}
        </Card>
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
  | { kind: "drag"; on: Uuid; from: Point; starts: { id: Uuid; box: PlaceBox; parent: Frame }[]; begun: boolean }
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
  const chosenIds = new Set(desk.chosen.map((c) => c.place_id));
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
    if (d && e.shiftKey) {
      // Shift adds to the choice, or takes away from it, and moves nothing.
      desk.select(d.place_id, true);
      gesture.current = null;
    } else if (d) {
      // A place already among several chosen moves them all.
      const group = desk.chosen.length > 1 && desk.chosen.some((c) => c.place_id === d.place_id) ? desk.chosen : [d];
      if (group.length === 1) desk.select(d.place_id);
      gesture.current = {
        kind: "drag",
        on: d.place_id,
        from: site(e),
        starts: group.map((x) => ({ id: x.place_id, box: x.box, parent: parentOf(x) })),
        begun: false,
      };
    } else {
      gesture.current = { kind: "pan", from: { x: e.clientX, y: e.clientY }, view, moved: false, on: target };
    }
  };

  const move = (e: PointerEvent<SVGSVGElement>) => {
    const g = gesture.current;
    if (!g) return;
    if (g.kind === "drag") {
      const [x, y] = site(e);
      const step = stepOf(desk.cellMm, e.shiftKey);
      const boxes = new Map(g.starts.map((s0) => [s0.id, moved(s0.box, s0.parent, x - g.from[0], y - g.from[1], step)]));
      const first = g.starts[0];
      const now = first && boxes.get(first.id);
      if (!g.begun && first && now && now.x === first.box.x && now.y === first.box.y) return;
      if (!g.begun) {
        desk.begin();
        g.begun = true;
      }
      desk.drag(boxes);
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
    // A click on the floor chooses it, or nothing; a click on one of several
    // chosen, without a drag, chooses just that one.
    if (g?.kind === "pan" && !g.moved) desk.select(g.on);
    if (g?.kind === "drag" && !g.begun && g.starts.length > 1) desk.select(g.on);
  };

  const key = (e: KeyboardEvent<SVGSVGElement>) => {
    const d = desk.selected;
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "z") {
      e.preventDefault();
      if (e.shiftKey) desk.redo();
      else desk.undo();
      return;
    }
    const all = desk.chosen.filter((x) => x.parent_id !== null);
    if (all.length === 0) return;
    const step = stepOf(desk.cellMm, e.shiftKey);
    const nudge: Record<string, Point> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] };
    const by = nudge[e.key];
    if (by) {
      e.preventDefault();
      desk.place(new Map(all.map((x) => [x.place_id, moved(x.box, parentOf(x), by[0], by[1], step)])));
    } else if (e.key === "r" || e.key === "R") {
      e.preventDefault();
      desk.place(new Map(all.map((x) => [x.place_id, turned(x.box, e.shiftKey ? 90 : -90)])));
    } else if (!d) {
      if (e.key === "Escape") desk.select(null);
    } else if ((e.key === "Delete" || e.key === "Backspace") && d.bins === 0 && d.parent_id !== null) {
      e.preventDefault();
      desk.remove(d.place_id);
    } else if (e.key === "Escape") {
      desk.select(null);
    }
  };

  // Outermost first, so what is inside is drawn over it, and the chosen last.
  const drawn = [...desk.plan].sort((a, b) => Number(a.place_id === chosen) - Number(b.place_id === chosen) || a.nesting - b.nesting);
  const picked = desk.plan.find((sh) => sh.place_id === chosen && sh.nesting > 0);
  // How much of the site a screen pixel is, so marks keep their size on screen.
  // The plan keeps its shape, so whichever way it is tighter sets the scale.
  const px = Math.max(view.w / (svg.current?.clientWidth || 800), view.h / (svg.current?.clientHeight || 600));
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
              chosenIds.has(sh.place_id) && s.chosen,
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
        {picked && picked.corners.length === 4 && desk.selected && <Front shape={picked} d={desk.selected} px={px} />}
        {desk.selected && desk.selected.parent_id && (
          <Gaps d={desk.selected} gaps={clearances(desk.drafts, desk.selected)} parent={parentOf(desk.selected)} cellMm={desk.cellMm} px={px} />
        )}
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
 * onto, and what turning it changes. A rack with two sides says so of its back,
 * and a dot on the front marks the end its numbering starts at (D220).
 */
function Front({ shape, d: place, px }: { shape: PlanShape; d: Draft; px: number }) {
  const [a, b, c, d] = shape.corners as [Point, Point, Point, Point];
  const out = unit([a[0] - d[0], a[1] - d[1]]);
  const [start, end] = place.from_right ? [b, a] : [a, b];
  const along = unit([end[0] - start[0], end[1] - start[1]]);
  const first: Point = [start[0] + along[0] * 7 * px, start[1] + along[1] * 7 * px];
  const label = (p: Point, q: Point, normal: Point, text: string) => {
    const m: Point = [(p[0] + q[0]) / 2 + normal[0] * 14 * px, (p[1] + q[1]) / 2 + normal[1] * 14 * px];
    return (
      <text x={m[0]} y={-m[1]} fontSize={11 * px} className={s.side} textAnchor="middle" dominantBaseline="central">
        {text}
      </text>
    );
  };
  return (
    <g pointerEvents="none">
      <line x1={a[0]} y1={-a[1]} x2={b[0]} y2={-b[1]} className={s.front} vectorEffect="non-scaling-stroke" />
      {label(a, b, out, "Front")}
      {place.sides === 2 && label(c, d, [-out[0], -out[1]], "Back")}
      {place.bays > 1 && <circle cx={first[0]} cy={-first[1]} r={4 * px} className={s.first} />}
    </g>
  );
}

/** A name along its place, never upside down: the plan's y runs down, so the turn is negated. */
function upright(turn: number): number {
  const t = ((turn % 180) + 180) % 180;
  return t > 90 ? 180 - t : -t;
}

/**
 * The room round the chosen place, drawn the way a tape would be held: to the
 * nearest neighbour across each gap, or to the wall. The same numbers are in
 * the panel beside the plan, to type over.
 */
function Gaps({ d, gaps, parent, cellMm, px }: { d: Draft; gaps: Clearance[]; parent: Frame; cellMm: number | null; px: number }) {
  const b = boundsOf(d);
  const [mx, my] = [(b.left + b.right) / 2, (b.front + b.back) / 2];
  return (
    <g pointerEvents="none">
      {gaps
        .filter((g) => g.gap > 1e-6)
        .map((g) => {
          const [u1, v1, u2, v2] =
            g.side === "left" ? [b.left, my, b.left - g.gap, my]
            : g.side === "right" ? [b.right, my, b.right + g.gap, my]
            : g.side === "front" ? [mx, b.front, mx, b.front - g.gap]
            : [mx, b.back, mx, b.back + g.gap];
          const [x1, y1] = at(parent, u1, v1);
          const [x2, y2] = at(parent, u2, v2);
          const [lx, ly] = [(x1 + x2) / 2, (y1 + y2) / 2];
          return (
            <g key={g.side}>
              <line x1={x1} y1={-y1} x2={x2} y2={-y2} className={s.gap} vectorEffect="non-scaling-stroke" />
              <text x={lx} y={-ly} fontSize={12 * px} strokeWidth={3 * px} className={s.gapLabel} textAnchor="middle" dominantBaseline="central">
                {`${shown(g.gap, cellMm)} ${unitOf(cellMm) === "m" ? "m" : ""}`.trim()}
              </text>
            </g>
          );
        })}
    </g>
  );
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

/** The chosen place, several chosen together, or what can be added. */
function Inspector({ desk }: { desk: PlanDesk }) {
  if (desk.chosen.length > 1) return <Several desk={desk} />;
  if (desk.selected) return <One desk={desk} d={desk.selected} />;
  const outer = desk.drafts.find((x) => x.parent_id === null) ?? null;
  // A new place goes in the middle of the outermost place.
  const where: Point = outer ? [Math.round(outer.box.length / 2), Math.round(outer.box.depth / 2)] : [0, 0];
  const racks = desk.drafts.filter((d) => d.solid && d.bays > 1);
  return (
    <Card title="Add to the plan" description="Choose a place on the plan to move, turn or measure it; Shift-click to choose more than one. Or add one here.">
      <div className={s.fields}>
        <div className={s.presets}>
          {PRESETS.map((p) => (
            <Button key={p.id} icon={<Plus />} onClick={() => desk.add(p, where)}>
              {p.name}
            </Button>
          ))}
        </div>
        {racks.length > 1 && (
          <div className={s.remove}>
            <Button onClick={() => desk.selectAll(racks.sort(byName).map((r) => r.place_id))}>Choose every rack ({racks.length})</Button>
          </div>
        )}
        <Tray desk={desk} where={where} />
      </div>
    </Card>
  );
}

/**
 * The bins on no layout yet (D211): the packing bench, a dock door, a floor
 * bay. Each can go on the plan as a spot of its own, and the packing bench
 * there is where the picking walk starts and ends.
 */
function Tray({ desk, where }: { desk: PlanDesk; where: Point }) {
  if (desk.tray.kind !== "ready") return null;
  const placing = new Set(desk.drafts.map((d) => d.holds?.location_id).filter(Boolean));
  const left = desk.tray.value.filter((b) => !placing.has(b.location_id));
  if (left.length === 0) return null;
  return (
    <fieldset className={s.group}>
      <legend className={s.legend}>On no layout ({left.length})</legend>
      <p className={s.note}>Put one on the plan as a spot of its own. The packing bench there is where the picking walk starts and ends.</p>
      <ul className={s.tray}>
        {left.map((b) => (
          <li key={b.location_id}>
            <span className={s.code}>{b.code}</span>
            <Button size="sm" icon={<Plus />} onClick={() => desk.placeBin(b, where)}>
              Place
            </Button>
          </li>
        ))}
      </ul>
    </fieldset>
  );
}

/** One place: its name, what it is, where it stands, the room round it, and its size. */
function One({ desk, d }: { desk: PlanDesk; d: Draft }) {
  const parent = desk.drafts.find((x) => x.place_id === d.parent_id);
  const mm = desk.cellMm;
  const unit = unitOf(mm);
  const set = (field: keyof PlaceBox) => (value: number) =>
    desk.change(d.place_id, (x) => ({ ...x, box: { ...x.box, [field]: stored(value, mm) } }));
  const step = mm ? 0.01 : 0.5;
  const rack = d.solid && d.bays > 1;
  const gaps = d.parent_id ? clearances(desk.drafts, d) : [];

  return (
    <Card
      title={d.name || "A place"}
      description={
        parent
          ? `Inside ${parent.name}, measured from its front left corner${mm ? ", in metres" : ""}.`
          : `Standing on the site${mm ? ", in metres" : ", in cells"}.`
      }
    >
      <div className={s.fields}>
        <TextField
          label="Name"
          value={d.name}
          disabled={!!d.holds}
          hint={d.holds ? `The spot for ${d.holds.code}, from the bins on no layout. It takes the bin's name.` : undefined}
          onChange={(e) => desk.change(d.place_id, (x) => ({ ...x, name: e.target.value }))}
        />
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
        {d.bays > 1 && <Numbering desk={desk} places={[d]} />}
        <div className={s.pair}>
          <NumberField label={parent ? "From its left" : "Across"} unit={unit} value={shown(d.box.x, mm)} step={step} onCommit={set("x")} />
          <NumberField label={parent ? "From its front" : "Up the plan"} unit={unit} value={shown(d.box.y, mm)} step={step} onCommit={set("y")} />
        </div>
        {gaps.length > 0 && (
          <fieldset className={s.group}>
            <legend className={s.legend}>Room round it</legend>
            <div className={s.pair}>
              {gaps.map((g) => (
                <NumberField
                  key={g.side}
                  label={`${SIDE_WORD[g.side]}, to ${g.to ?? "the wall"}`}
                  unit={unit}
                  value={shown(g.gap, mm)}
                  step={step}
                  onCommit={(v) => desk.change(d.place_id, (x) => ({ ...x, box: cleared(x.box, g.side, g.gap, stored(v, mm)) }))}
                />
              ))}
            </div>
          </fieldset>
        )}
        <div className={s.trio}>
          <NumberField label="Length" unit={unit} value={shown(d.box.length, mm)} step={step} min={0.01} disabled={!!d.outline} onCommit={set("length")} />
          <NumberField label="Depth" unit={unit} value={shown(d.box.depth, mm)} step={step} min={0.01} disabled={!!d.outline} onCommit={set("depth")} />
          <NumberField label="Height" unit={unit} value={shown(d.box.height, mm)} step={step} min={0.01} onCommit={set("height")} />
        </div>
        {rack && <MakeForm desk={desk} racks={[d]} more={sameMake(desk.drafts, d)} />}
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
          <Button variant="ghost" icon={<Trash2 />} disabled={d.bins > 0 || d.parent_id === null} onClick={() => desk.remove(d.place_id)}>
            Take it away
          </Button>
          {d.bins > 0 && <span className={s.note}>It holds bins or places, so it stays.</span>}
        </div>
      </div>
    </Card>
  );
}

const SIDE_WORD = { left: "Left", right: "Right", front: "In front", back: "Behind" } as const;

/** Several places chosen: set out in a row, or sized from their bays together. */
function Several({ desk }: { desk: PlanDesk }) {
  const mm = desk.cellMm;
  const unit = unitOf(mm);
  const racks = desk.chosen.filter((d) => d.parent_id !== null);
  const parents = new Set(racks.map((d) => d.parent_id));
  const bounds = racks.map(boundsOf);
  const [left, setLeft] = useState(() => String(shown(Math.min(...bounds.map((b) => b.left)), mm)));
  const [front, setFront] = useState(() => String(shown(Math.min(...bounds.map((b) => b.front)), mm)));
  const [aisle, setAisle] = useState(() => String(mm ? 2.4 : 2));
  const [way, setWay] = useState<RowWay>("up");
  const [order, setOrder] = useState<"az" | "za">("az");
  const numbers = [left, front, aisle].map((t) => Number.parseFloat(t));
  const ready = parents.size === 1 && numbers.every((n) => Number.isFinite(n)) && numbers[2]! >= 0;
  const sized = racks.filter((d) => d.solid && d.bays > 1);
  const numbered = desk.chosen.filter((d) => d.bays > 1);

  return (
    <Card title={`${desk.chosen.length} places chosen`} description="Shift-click a place to add it or take it out. Drag any of them to move them all.">
      <div className={s.fields}>
        <fieldset className={s.group}>
          <legend className={s.legend}>Set out in a row</legend>
          <div className={s.pair}>
            <TextField label="First from the left" type="number" inputMode="decimal" trailing={unit} value={left} onChange={(e) => setLeft(e.target.value)} />
            <TextField label="First from the front" type="number" inputMode="decimal" trailing={unit} value={front} onChange={(e) => setFront(e.target.value)} />
          </div>
          <div className={s.pair}>
            <TextField label="Aisles" type="number" inputMode="decimal" trailing={unit} value={aisle} onChange={(e) => setAisle(e.target.value)} />
            <Select
              label="Each next one"
              value={way}
              options={[
                { value: "up", label: "Behind" },
                { value: "across", label: "To the right" },
              ]}
              onValueChange={(v) => setWay(v as RowWay)}
            />
          </div>
          <Select
            label="In order"
            value={order}
            options={[
              { value: "az", label: "By name, A first" },
              { value: "za", label: "By name, A last" },
            ]}
            onValueChange={(v) => setOrder(v as "az" | "za")}
          />
          {parents.size > 1 && <p className={s.note}>They are inside different places, so they can't share a row.</p>}
          <Button
            disabled={!ready}
            onClick={() => {
              const sorted = [...racks].sort(byName);
              if (order === "za") sorted.reverse();
              const [l, f, a] = numbers as [number, number, number];
              desk.place(inRow(sorted, { left: stored(l, mm), front: stored(f, mm) }, stored(a, mm), way));
            }}
          >
            Set out {racks.length} in a row
          </Button>
        </fieldset>
        {numbered.length > 0 && <Numbering desk={desk} places={numbered} />}
        {sized.length > 0 && <MakeForm desk={desk} racks={sized} more={[]} />}
      </div>
    </Card>
  );
}

/**
 * Which end of its front a place's labels start at, as you face it (D220).
 * Its bins keep their names: on Save each goes to the bay its name is on, so a
 * rack drafted the wrong way round is put right here.
 */
function Numbering({ desk, places }: { desk: PlanDesk; places: Draft[] }) {
  const right = places.filter((d) => d.from_right).length;
  return (
    <Select
      label="Numbered from"
      value={right === places.length ? "right" : right === 0 ? "left" : ""}
      placeholder="Some from each end"
      hint={
        places.length === 1
          ? "Its first bay, as you face the front. The dot on the plan marks it."
          : "Each one's first bay, as you face its front."
      }
      options={[
        { value: "left", label: "The left end" },
        { value: "right", label: "The right end" },
      ]}
      onValueChange={(v) => desk.change(places.map((d) => d.place_id), (x) => ({ ...x, from_right: v === "right" }))}
    />
  );
}

/**
 * A rack's make, measured once: a bay's width, one side's depth, a level's
 * height. Every rack given it is sized from its own bays and levels.
 */
function MakeForm({ desk, racks, more }: { desk: PlanDesk; racks: Draft[]; more: Draft[] }) {
  const mm = desk.cellMm;
  const unit = unitOf(mm);
  const first = racks[0]!;
  const now = makeOf(first);
  const [bay, setBay] = useState(() => String(shown(now.bay, mm)));
  const [side, setSide] = useState(() => String(shown(now.side, mm)));
  const [level, setLevel] = useState(() => String(shown(now.level, mm)));
  const [all, setAll] = useState(false);
  useEffect(() => {
    const m = makeOf(first);
    setBay(String(shown(m.bay, mm)));
    setSide(String(shown(m.side, mm)));
    setLevel(String(shown(m.level, mm)));
  }, [first.place_id, mm]); // eslint-disable-line react-hooks/exhaustive-deps
  const values = [bay, side, level].map((t) => Number.parseFloat(t));
  const ready = values.every((n) => Number.isFinite(n) && n > 0);
  const others = more.filter((d) => !racks.includes(d));
  const targets = all ? [...racks, ...others] : racks;

  return (
    <fieldset className={s.group}>
      <legend className={s.legend}>Size from its bays</legend>
      <div className={s.trio}>
        <TextField label="Bay width" type="number" inputMode="decimal" trailing={unit} value={bay} onChange={(e) => setBay(e.target.value)} />
        <TextField label="Side depth" type="number" inputMode="decimal" trailing={unit} value={side} onChange={(e) => setSide(e.target.value)} />
        <TextField label="Level height" type="number" inputMode="decimal" trailing={unit} value={level} onChange={(e) => setLevel(e.target.value)} />
      </div>
      {others.length > 0 && (
        <Checkbox
          checked={all}
          onCheckedChange={(v) => setAll(v === true)}
          label={`And the ${others.length} other rack${others.length === 1 ? "" : "s"} with ${first.levels} levels and ${first.sides === 2 ? "two sides" : "one side"}`}
        />
      )}
      <Button
        disabled={!ready}
        onClick={() => {
          const [b, sd, lv] = values.map((v) => stored(v, mm)) as [number, number, number];
          desk.place(new Map(targets.map((d) => [d.place_id, sizedFrom(d, { bay: b, side: sd, level: lv })])));
        }}
      >
        {targets.length === 1 ? "Size it" : `Size ${targets.length} racks`}
      </Button>
      <p className={s.note}>
        {racks.length === 1
          ? `${first.bays} bays long, ${first.sides === 2 ? "two sides" : "one side"} deep and ${first.levels} levels high. It grows from its front left corner.`
          : "Each is sized from its own bays and levels, and grows from its front left corner."}
      </p>
    </fieldset>
  );
}

/**
 * A number typed in full before it counts: "1." on the way to "1.5" isn't
 * read as 1. It counts on Enter, or on leaving the field.
 */
function NumberField({
  label,
  unit,
  value,
  step,
  min,
  disabled,
  onCommit,
}: {
  label: string;
  unit: string;
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
      trailing={unit}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
      }}
    />
  );
}
