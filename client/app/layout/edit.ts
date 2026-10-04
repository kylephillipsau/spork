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
  /** Bins in its cells, and places inside it: either keeps it. */
  bins: number;
  /** Drawn in this editor, not yet saved. */
  fresh: boolean;
}

export const SITE: Frame = { x: 0, y: 0, z: 0, turn: 0 };

/** The finest a drag or a nudge places a corner: half a cell. */
export const SNAP = 0.5;

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

/** To the nearest half cell, without a negative nought. */
export function snap(n: number, step = SNAP): number {
  return Math.round(n / step) * step + 0;
}

/**
 * The box after a drag of `dx`, `dy` across the site from where it started.
 * The drag is in the site's cells and the box is in its parent's, so a place
 * inside a turned building moves the way the pointer does.
 */
export function moved(start: PlaceBox, parent: Frame, dx: number, dy: number): PlaceBox {
  const [du, dv] = within({ ...parent, x: 0, y: 0 }, [dx, dy]);
  return { ...start, x: snap(start.x + du), y: snap(start.y + dv) };
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

/** Rounded past floating-point dust, so a quarter turn back lands where it was. */
function tidy(n: number): number {
  return Math.round(n * 1e6) / 1e6 + 0;
}
