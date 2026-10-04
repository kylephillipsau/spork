import type { Frame, LayoutView, PlanShape, Uuid } from "@domain/types";

/**
 * The plan editor's arithmetic (D209), with no React and no network in it.
 *
 * A place's box is in its parent's cells: its front-left corner, its length
 * along its front, its depth back from it, its height, and its turn in degrees
 * anticlockwise. Where it stands on the site is worked out by composing the
 * boxes down from the site, as the server's `layout::compose` does; this is
 * the same arithmetic, so the plan can follow a drag without asking.
 *
 * **Bins never move.** A bin's cell is a bay and a level of its place, so a
 * rack moved, turned or stretched carries its bins with it. Nothing here
 * touches a grid.
 */

export type Point = [number, number];

export interface PlaceBox {
  x: number;
  y: number;
  z: number;
  length: number;
  depth: number;
  height: number;
  turn: number;
}

/** A place as the editor holds it. */
export interface Draft {
  place_id: Uuid;
  parent_id: Uuid | null;
  name: string;
  solid: boolean;
  box: PlaceBox;
  /** Its own outline, in its own cells: moved and turned, never resized. */
  outline: Point[] | null;
  /** Bins on two faces, so it stays solid. */
  sides: number;
  /** Its grid, which sizing from bays reads and nothing here changes. */
  bays: number;
  levels: number;
  /** Bins in its cells, and places inside it: either keeps it. */
  bins: number;
  /** Drawn in this editor, not yet saved. */
  fresh: boolean;
}

export const SITE: Frame = { x: 0, y: 0, z: 0, turn: 0 };

/** The finest a drag or a nudge places a corner when the plan isn't to scale: half a cell. */
export const SNAP = 0.5;

/**
 * How the layout's numbers read (D210): metres once the site says how long a
 * cell is, cells until then. Stored positions are always cells.
 */
export function shown(cells: number, cellMm: number | null): number {
  return cellMm ? round((cells * cellMm) / 1000) : round(cells);
}

/** A number as read on screen, back into cells. */
export function stored(value: number, cellMm: number | null): number {
  return cellMm ? (value * 1000) / cellMm : value;
}

/** The unit the numbers are shown in. */
export function unitOf(cellMm: number | null): "m" | "cells" {
  return cellMm ? "m" : "cells";
}

/**
 * The step a drag or a nudge moves by, in cells: ten centimetres once the
 * plan is to scale, which a tape can tell apart, or a metre with Shift.
 * Half a cell, or five, before then.
 */
export function stepOf(cellMm: number | null, big = false): number {
  if (!cellMm) return big ? SNAP * 10 : SNAP;
  return (big ? 1000 : 100) / cellMm;
}

/** A point in a frame's cells, on the site: `Frame::point`. */
export function at(frame: Frame, u: number, v: number): Point {
  const r = (frame.turn * Math.PI) / 180;
  const [s, c] = [Math.sin(r), Math.cos(r)];
  return [frame.x + u * c - v * s, frame.y + u * s + v * c];
}

/** A point on the site, in a frame's cells: `at`, the other way. */
export function within(frame: Frame, [x, y]: Point): Point {
  const r = (frame.turn * Math.PI) / 180;
  const [s, c] = [Math.sin(r), Math.cos(r)];
  const [dx, dy] = [x - frame.x, y - frame.y];
  return [dx * c + dy * s, -dx * s + dy * c];
}

/** A child's frame on the site, from its parent's and its box: `layout::compose`. */
export function compose(parent: Frame, box: Pick<PlaceBox, "x" | "y" | "z" | "turn">): Frame {
  const [x, y] = at(parent, box.x, box.y);
  return { x, y, z: parent.z + box.z, turn: mod360(parent.turn + box.turn) };
}

/** The editor's places, from the layout as read. */
export function draftsOf(view: LayoutView): Draft[] {
  const shapes = new Map(view.plan.map((s) => [s.place_id, s]));
  const children = new Set(view.places.map((p) => p.parent_id).filter((id): id is Uuid => id !== null));
  return view.places.map((p) => {
    const shape = shapes.get(p.place_id);
    return {
      place_id: p.place_id,
      parent_id: p.parent_id,
      name: p.name,
      solid: p.solid,
      box: { x: p.x, y: p.y, z: p.z, length: p.length, depth: p.depth, height: p.height, turn: p.turn },
      outline: p.outlined && shape ? shape.corners.map((c) => within(shape.frame, c)) : null,
      sides: p.sides,
      bays: p.bays,
      levels: p.levels,
      bins: p.bins + (children.has(p.place_id) ? 1 : 0),
      fresh: false,
    };
  });
}

/** Every place's frame on the site. A place whose parents loop gets none. */
export function framesOf(drafts: readonly Draft[]): Map<Uuid, Frame> {
  const byId = new Map(drafts.map((d) => [d.place_id, d]));
  const out = new Map<Uuid, Frame>();
  const frameOf = (d: Draft, seen: Set<Uuid>): Frame | null => {
    const known = out.get(d.place_id);
    if (known) return known;
    if (seen.has(d.place_id)) return null;
    seen.add(d.place_id);
    const parent = d.parent_id ? byId.get(d.parent_id) : undefined;
    const base = parent ? frameOf(parent, seen) : SITE;
    if (!base) return null;
    const f = compose(base, d.box);
    out.set(d.place_id, f);
    return f;
  };
  for (const d of drafts) frameOf(d, new Set());
  return out;
}

/** The site's plan from the editor's places, as `GET /layout` draws it. */
export function planOf(drafts: readonly Draft[]): PlanShape[] {
  const frames = framesOf(drafts);
  const ids = new Set(drafts.map((d) => d.place_id));
  const kids = new Map<Uuid | null, Draft[]>();
  for (const d of drafts) {
    const parent = d.parent_id && ids.has(d.parent_id) ? d.parent_id : null;
    kids.set(parent, [...(kids.get(parent) ?? []), d]);
  }
  const out: PlanShape[] = [];
  const walk = (d: Draft, nesting: number, seen: Set<Uuid>) => {
    if (seen.has(d.place_id)) return;
    seen.add(d.place_id);
    const f = frames.get(d.place_id);
    if (!f) return;
    out.push({ place_id: d.place_id, name: d.name, solid: d.solid, nesting, corners: footprint(f, d), z: f.z, height: d.box.height, frame: f });
    for (const k of kids.get(d.place_id) ?? []) walk(k, nesting + 1, seen);
  };
  const seen = new Set<Uuid>();
  for (const root of kids.get(null) ?? []) walk(root, 0, seen);
  return out;
}

/** A place's corners on the site: its outline, or its rectangle. */
export function footprint(f: Frame, d: Pick<Draft, "box" | "outline">): Point[] {
  if (d.outline) return d.outline.map(([u, v]) => at(f, u, v));
  const { length: l, depth: w } = d.box;
  return [at(f, 0, 0), at(f, l, 0), at(f, l, w), at(f, 0, w)];
}

/** To the nearest step, without a negative nought or floating-point dust. */
export function snap(n: number, step = SNAP): number {
  return tidy(Math.round(n / step) * step);
}

/**
 * The box after a drag of `dx`, `dy` across the site from where it started.
 * The drag is in the site's cells and the box is in its parent's, so a place
 * inside a turned building moves the way the pointer does.
 */
export function moved(start: PlaceBox, parent: Frame, dx: number, dy: number, step = SNAP): PlaceBox {
  const [du, dv] = within({ ...parent, x: 0, y: 0 }, [dx, dy]);
  return { ...start, x: snap(start.x + du, step), y: snap(start.y + dv, step) };
}

/**
 * A quarter turn, or any turn, about the place's middle: a long rack turned
 * swings about its centre, not its end.
 */
export function turned(box: PlaceBox, by: number): PlaceBox {
  const centre = (turn: number): Point => at({ x: 0, y: 0, z: 0, turn }, box.length / 2, box.depth / 2);
  const [cx, cy] = centre(box.turn);
  const turn = mod360(box.turn + by);
  const [nx, ny] = centre(turn);
  return { ...box, turn, x: tidy(box.x + cx - nx), y: tidy(box.y + cy - ny) };
}

/** What a place can be added as. */
export const PRESETS = [
  { id: "wall", name: "Wall", solid: true, length: 10, depth: 0.5, height: 4 },
  { id: "column", name: "Column", solid: true, length: 1, depth: 1, height: 4 },
  { id: "dock", name: "Dock", solid: false, length: 6, depth: 4, height: 1 },
  { id: "packing", name: "Packing station", solid: false, length: 3, depth: 2, height: 1 },
  { id: "area", name: "Area", solid: false, length: 4, depth: 4, height: 1 },
] as const;

export type Preset = (typeof PRESETS)[number];

/** A name not yet used inside the same parent: "Wall", then "Wall 2". */
export function freeName(drafts: readonly Draft[], parent: Uuid | null, base: string): string {
  const taken = new Set(drafts.filter((d) => d.parent_id === parent).map((d) => d.name.trim().toLowerCase()));
  if (!taken.has(base.toLowerCase())) return base;
  for (let n = 2; ; n++) if (!taken.has(`${base} ${n}`.toLowerCase())) return `${base} ${n}`;
}

/** A place as a save sends it: `POST /layout/edit`'s `changed`. */
export type PlaceChanged = { place_id: Uuid; name: string; solid: boolean } & PlaceBox;
/** And its `added`, which also says what the place is inside. */
export type PlaceAdded = PlaceChanged & { parent_id: Uuid | null };

export interface Changes {
  changed: PlaceChanged[];
  added: PlaceAdded[];
  removed: Uuid[];
}

/** What a save sends: each place changed, added or removed since the read. */
export function changesOf(view: LayoutView, drafts: readonly Draft[]): Changes {
  const was = new Map(draftsOf(view).map((d) => [d.place_id, d]));
  const now = new Set(drafts.map((d) => d.place_id));
  const changed: PlaceChanged[] = [];
  const added: PlaceAdded[] = [];
  for (const d of drafts) {
    const before = was.get(d.place_id);
    const row = { place_id: d.place_id, name: d.name.trim(), solid: d.solid, ...d.box };
    if (!before) added.push({ ...row, parent_id: d.parent_id });
    else if (before.name !== row.name || before.solid !== d.solid || !sameBox(before.box, d.box)) changed.push(row);
  }
  const removed = [...was.keys()].filter((id) => !now.has(id));
  return { changed, added, removed };
}

/** How many changes a save would make. */
export function changeCount(view: LayoutView, drafts: readonly Draft[]): number {
  const c = changesOf(view, drafts);
  return c.changed.length + c.added.length + c.removed.length;
}

function sameBox(a: PlaceBox, b: PlaceBox): boolean {
  return (["x", "y", "z", "length", "depth", "height", "turn"] as const).every((k) => Math.abs(a[k] - b[k]) < 1e-9);
}

function mod360(n: number): number {
  return ((n % 360) + 360) % 360;
}

/** To the millimetre of a metre, or the thousandth of a cell: what is shown. */
function round(n: number): number {
  return Math.round(n * 1000) / 1000 + 0;
}

/** Rounded past floating-point dust, so a quarter turn back lands where it was. */
function tidy(n: number): number {
  return Math.round(n * 1e6) / 1e6 + 0;
}

// ── building the floor from measurements (D210) ─────────────────────────

/** A rack's make: how wide a bay is, how deep one side, how high a level. In cells. */
export interface RackMake {
  bay: number;
  side: number;
  level: number;
}

/** What a rack's make is, as it is drawn now. */
export function makeOf(d: Draft): RackMake {
  return {
    bay: tidy(d.box.length / Math.max(1, d.bays)),
    side: tidy(d.box.depth / Math.max(1, d.sides)),
    level: tidy(d.box.height / Math.max(1, d.levels)),
  };
}

/**
 * A rack sized from its bays: as long as its bays, as deep as its sides, as
 * tall as its levels. It grows from its front left corner, so where it stands
 * doesn't change, and its bins, a bay and a level each, go with it.
 */
export function sizedFrom(d: Draft, make: RackMake): PlaceBox {
  return {
    ...d.box,
    length: tidy(d.bays * make.bay),
    depth: tidy(d.sides * make.side),
    height: tidy(d.levels * make.level),
  };
}

/** Racks of the same make as this one, by their grid: the same levels and sides. */
export function sameMake(drafts: readonly Draft[], d: Draft): Draft[] {
  return drafts.filter((x) => x.solid && x.bays > 1 && x.levels === d.levels && x.sides === d.sides);
}

/** A rectangle on the plan, in its parent's cells: left, front, right, back. */
export interface Bounds {
  left: number;
  front: number;
  right: number;
  back: number;
}

/** Where a place lies in its parent, whichever way it is turned. */
export function boundsOf(d: Pick<Draft, "box" | "outline">): Bounds {
  const corners = footprint({ x: d.box.x, y: d.box.y, z: 0, turn: d.box.turn }, d);
  const xs = corners.map((c) => c[0]);
  const ys = corners.map((c) => c[1]);
  return { left: tidy(Math.min(...xs)), front: tidy(Math.min(...ys)), right: tidy(Math.max(...xs)), back: tidy(Math.max(...ys)) };
}

/** What a place's parent is, as the walls a place inside it stands between. */
export function wallsOf(parent: Draft | undefined): Bounds | null {
  if (!parent) return null;
  if (parent.outline) {
    const xs = parent.outline.map((c) => c[0]);
    const ys = parent.outline.map((c) => c[1]);
    return { left: Math.min(...xs), front: Math.min(...ys), right: Math.max(...xs), back: Math.max(...ys) };
  }
  return { left: 0, front: 0, right: parent.box.length, back: parent.box.depth };
}

export type Side = "left" | "right" | "front" | "back";

/** How far a place is from whatever is next to it on one side, and what that is. */
export interface Clearance {
  side: Side;
  gap: number;
  /** A neighbour's name, or null for its parent's wall. */
  to: string | null;
}

/**
 * The room round a place on each side: to the nearest neighbour that faces it
 * across the gap, or else to the wall. What a tape measures from a rack.
 */
export function clearances(drafts: readonly Draft[], d: Draft): Clearance[] {
  const parent = drafts.find((x) => x.place_id === d.parent_id);
  const walls = wallsOf(parent);
  const me = boundsOf(d);
  const others = drafts.filter((x) => x.parent_id === d.parent_id && x.place_id !== d.place_id).map((x) => ({ name: x.name, b: boundsOf(x) }));
  const across = (b: Bounds) => b.left < me.right && b.right > me.left;
  const along = (b: Bounds) => b.front < me.back && b.back > me.front;
  const nearest = (side: Side): Clearance | null => {
    const wall = walls ? { left: me.left - walls.left, right: walls.right - me.right, front: me.front - walls.front, back: walls.back - me.back }[side] : null;
    let best: Clearance | null = wall === null ? null : { side, gap: wall, to: null };
    for (const o of others) {
      const gap =
        side === "left" && along(o.b) && o.b.right <= me.left + 1e-9 ? me.left - o.b.right
        : side === "right" && along(o.b) && o.b.left >= me.right - 1e-9 ? o.b.left - me.right
        : side === "front" && across(o.b) && o.b.back <= me.front + 1e-9 ? me.front - o.b.back
        : side === "back" && across(o.b) && o.b.front >= me.back - 1e-9 ? o.b.front - me.back
        : null;
      if (gap !== null && (!best || gap < best.gap)) best = { side, gap: tidy(gap), to: o.name };
    }
    return best && { ...best, gap: tidy(best.gap) };
  };
  return (["left", "right", "front", "back"] as const).map(nearest).filter((c): c is Clearance => c !== null);
}

/** The box moved so that its clearance on one side is `wanted` rather than `now`. */
export function cleared(box: PlaceBox, side: Side, now: number, wanted: number): PlaceBox {
  const by = wanted - now;
  const [dx, dy] = side === "left" ? [by, 0] : side === "right" ? [-by, 0] : side === "front" ? [0, by] : [0, -by];
  return { ...box, x: tidy(box.x + dx), y: tidy(box.y + dy) };
}

/** Which way a row of racks steps: up the plan, each behind the last, or across it. */
export type RowWay = "up" | "across";

/**
 * Racks set out in a row, as they are on most floors: the first `from` its
 * parent's left and front, each next one an aisle beyond the last, their ends
 * lined up. Each keeps its turn, so a rack that faces the other way still
 * does. In the order given.
 */
export function inRow(racks: readonly Draft[], from: { left: number; front: number }, aisle: number, way: RowWay): Map<string, PlaceBox> {
  const out = new Map<string, PlaceBox>();
  let at = way === "up" ? from.front : from.left;
  for (const d of racks) {
    const b = boundsOf(d);
    const [dx, dy] = way === "up" ? [from.left - b.left, at - b.front] : [at - b.left, from.front - b.front];
    out.set(d.place_id, { ...d.box, x: tidy(d.box.x + dx), y: tidy(d.box.y + dy) });
    at += (way === "up" ? b.back - b.front : b.right - b.left) + aisle;
  }
  return out;
}

/** Names in the order people say them: Rack 2 before Rack 10. */
export function byName(a: Draft, b: Draft): number {
  return a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: "base" });
}
