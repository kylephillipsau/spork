import type { BenchLine, PackUnit, Preset, StatedSize, Uuid } from "@domain/types";

/**
 * How what is left to pack would go into boxes (D195): which box, and what goes
 * where in it, layer by layer from the bottom.
 *
 * **A suggestion made from what is recorded.** An each with no size is listed,
 * not guessed at from its carton, so the packer can measure it at the bench and
 * the suggestion is made again. Whole cartons of a line ship as they are
 * (migration 98) and are left out. A thing with no size to measure (D138) goes
 * in round the rest.
 *
 * **Layers, because a packer builds one.** Each layer starts with the biggest
 * thing that still fits, laid on its biggest side, and is filled round it and on
 * top of the shorter things in it, largest first. It is a heuristic, not the
 * best packing there is: good enough to choose a box by, and simple enough that
 * the layers it makes can be followed with the goods in hand.
 *
 * No React and no three.js here, so a test runner can read it.
 */

/** Millimetres along the box's length, across its width, and up. */
export type Dims = [number, number, number];

/** Which of a thing's own length, width and height runs along each of the box's. */
export type Axes = [0 | 1 | 2, 0 | 1 | 2, 0 | 1 | 2];

/** One kind of thing to place: an each or an inner pack of a line's item. */
export interface Kind {
  line: Uuid;
  item_id: Uuid;
  item_code: string;
  level: PackUnit["level"];
  /** Eaches in one of it. */
  units: number;
  /** As measured: its length, width and height. */
  size: Dims;
  weight_g: number | null;
  /** Its sides, to draw it with. */
  faces: PackUnit["faces"];
  /** Its line's place on the bench, for telling kinds apart by colour. */
  index: number;
}

export interface Placement {
  kind: Kind;
  /** Its corner at the box's back left bottom, in millimetres from the box's. */
  x: number;
  y: number;
  z: number;
  /** Its extent along the box's length, width and height, as placed. */
  dims: Dims;
  axes: Axes;
}

export interface Layer {
  z: number;
  height: number;
  placements: Placement[];
}

/** A line's units that are not placed, and why, in units and in its terms. */
export interface Aside {
  line: Uuid;
  item_id: Uuid;
  item_code: string;
  units: number;
}

export interface BoxPlan {
  preset: Preset & { size: StatedSize };
  layers: Layer[];
  /** Things with no size to measure, put in round the rest of this box. */
  loose: Aside[];
  /** The share of the box's inside the placed goods take up. */
  fill: number;
  /** What the goods in it weigh by the record, and how many pieces have no weight. */
  weight_g: number;
  unweighed: number;
}

export interface Arrangement {
  boxes: BoxPlan[];
  /** Whole cartons of a line, which ship as they are. */
  own: (Aside & { cartons: number })[];
  /** No size recorded at the level they would go in at: measure, then it is placed. */
  unmeasured: Aside[];
  /** Bigger than every box, in any way up. */
  oversize: Aside[];
  /** Things with no size, when there is no box for them to go round. */
  loose: Aside[];
  /** More pieces than are worth arranging one by one. */
  tooMany: boolean;
}

/** Past this many pieces a layer plan is no help to anybody: it is a pallet job. */
export const MOST_PIECES = 1500;

/** The six ways up a thing can go, as which of its sides runs along the box's. */
const WAYS: readonly Axes[] = [
  [0, 1, 2],
  [1, 0, 2],
  [0, 2, 1],
  [2, 0, 1],
  [1, 2, 0],
  [2, 1, 0],
];

interface Way {
  axes: Axes;
  dims: Dims;
}

function turned(size: Dims, axes: Axes): Dims {
  return [size[axes[0]], size[axes[1]], size[axes[2]]];
}

/** Every distinct way up that fits in `room`. */
function fitting(size: Dims, room: Dims): Way[] {
  const seen = new Set<string>();
  const out: Way[] = [];
  for (const axes of WAYS) {
    const dims = turned(size, axes);
    const key = dims.join("x");
    if (seen.has(key)) continue;
    seen.add(key);
    if (dims[0] <= room[0] && dims[1] <= room[1] && dims[2] <= room[2]) out.push({ axes, dims });
  }
  return out;
}

/**
 * Laid on its biggest side: the way that fits with the most of it on the
 * floor. Of the two ways round it can lie, the one more of it fit across the
 * room: an apron 280 by 220 goes into a box 450 wide once lengthways and twice
 * turned.
 */
function flattest(size: Dims, room: Dims): Way | null {
  let best: Way | null = null;
  let bestTiles = 0;
  for (const w of fitting(size, room)) {
    const area = w.dims[0] * w.dims[1];
    const tiles = Math.floor(room[0] / w.dims[0]) * Math.floor(room[1] / w.dims[1]);
    const bestArea = best ? best.dims[0] * best.dims[1] : 0;
    if (!best || area > bestArea || (area === bestArea && tiles > bestTiles)) {
      best = w;
      bestTiles = tiles;
    }
  }
  return best;
}

function volume([l, w, h]: Dims): number {
  return l * w * h;
}

/** The area of its biggest side, which is what it takes of a floor laid flat. */
function footprint([a, b, c]: Dims): number {
  return Math.max(a * b, a * c, b * c);
}

/** What is still to place: each kind and how many of it, biggest first. */
interface Left {
  kind: Kind;
  count: number;
}

function sorted(left: Left[]): Left[] {
  return [...left].sort(
    (a, b) => footprint(b.kind.size) - footprint(a.kind.size) || volume(b.kind.size) - volume(a.kind.size) || a.kind.index - b.kind.index,
  );
}

interface Space {
  x: number;
  y: number;
  z: number;
  room: Dims;
}

/** Put the biggest thing that fits into the corner of `space`, then fill round and on top of it. */
function fill(space: Space, left: Left[], out: Placement[]): void {
  const [l, w, h] = space.room;
  if (l <= 0 || w <= 0 || h <= 0) return;
  for (const entry of left) {
    if (entry.count === 0) continue;
    const way = flattest(entry.kind.size, space.room);
    if (!way) continue;
    entry.count -= 1;
    out.push({ kind: entry.kind, x: space.x, y: space.y, z: space.z, dims: way.dims, axes: way.axes });
    round(space, way.dims, left, out);
    return;
  }
}

/**
 * Fill what is left of `space` once something of `placed` size is in its
 * corner: on top of it, up to the space's height, then the floor beside it,
 * cut so that the bigger of the two pieces of floor stays whole.
 */
function round(space: Space, placed: Dims, left: Left[], out: Placement[]): void {
  const [l, w, h] = space.room;
  const [pl, pw, ph] = placed;
  if (h > ph) fill({ x: space.x, y: space.y, z: space.z + ph, room: [pl, pw, h - ph] }, left, out);
  const beside = l - pl;
  const before = w - pw;
  if (beside * w >= before * l) {
    fill({ x: space.x + pl, y: space.y, z: space.z, room: [beside, w, h] }, left, out);
    fill({ x: space.x, y: space.y + pw, z: space.z, room: [pl, before, h] }, left, out);
  } else {
    fill({ x: space.x, y: space.y + pw, z: space.z, room: [l, before, h] }, left, out);
    fill({ x: space.x + pl, y: space.y, z: space.z, room: [beside, pw, h] }, left, out);
  }
}

/** Pack `left` into a box of `size`, layer on layer. What does not go in stays in `left`. */
export function pack(size: Dims, pieces: Left[]): { layers: Layer[]; left: Left[] } {
  const left = sorted(pieces.map((p) => ({ ...p })));
  const layers: Layer[] = [];
  let z = 0;
  for (;;) {
    // A layer starts with the biggest thing that still lies in the height left.
    let start: { entry: Left; way: Way } | null = null;
    for (const entry of left) {
      if (entry.count === 0) continue;
      const way = flattest(entry.kind.size, [size[0], size[1], size[2] - z]);
      if (way) {
        start = { entry, way };
        break;
      }
    }
    if (!start) break;
    const height = start.way.dims[2];
    const placements: Placement[] = [{ kind: start.entry.kind, x: 0, y: 0, z, dims: start.way.dims, axes: start.way.axes }];
    start.entry.count -= 1;
    round({ x: 0, y: 0, z, room: [size[0], size[1], height] }, start.way.dims, left, placements);
    layers.push({ z, height, placements });
    z += height;
  }
  return { layers, left: left.filter((e) => e.count > 0) };
}

function dims(s: StatedSize): Dims {
  return [s.length_mm, s.width_mm, s.height_mm];
}

function measured(p: PackUnit | undefined): Dims | null {
  if (!p?.size) return null;
  const d = dims(p.size);
  return d.every((n) => n > 0) ? d : null;
}

/**
 * What each line's units go in as: whole cartons that ship as they are, inner
 * packs while the count fills one, then eaches.
 */
export function piecesOf(lines: BenchLine[]): {
  pieces: Left[];
  own: Arrangement["own"];
  loose: Aside[];
  unmeasured: Aside[];
} {
  const pieces: Left[] = [];
  const own: Arrangement["own"] = [];
  const loose: Aside[] = [];
  const unmeasured: Aside[] = [];
  lines.forEach((line, index) => {
    let units = Math.max(0, line.remaining);
    if (units === 0) return;
    const aside = (n: number): Aside => ({ line: line.line_id, item_id: line.item_id, item_code: line.item_code, units: n });
    const kind = (unit: PackUnit, size: Dims): Kind => ({
      line: line.line_id,
      item_id: line.item_id,
      item_code: line.item_code,
      level: unit.level,
      units: unit.units,
      size,
      weight_g: unit.gross_weight_g,
      faces: unit.faces,
      index,
    });

    const carton = line.own_carton?.units ?? 0;
    if (carton > 0 && units >= carton) {
      const cartons = Math.floor(units / carton);
      own.push({ ...aside(cartons * carton), cartons });
      units -= cartons * carton;
    }
    const inner = line.packs.find((p) => p.level === "inner");
    const innerSize = measured(inner);
    if (inner && innerSize && inner.units > 1 && units >= inner.units) {
      const count = Math.floor(units / inner.units);
      pieces.push({ kind: kind(inner, innerSize), count });
      units -= count * inner.units;
    }
    if (units === 0) return;
    const each = line.packs.find((p) => p.level === "each");
    const eachSize = measured(each);
    if (each && eachSize) pieces.push({ kind: kind(each, eachSize), count: units });
    else if (each?.no_size) loose.push(aside(units));
    else unmeasured.push(aside(units));
  });
  return { pieces, own, loose, unmeasured };
}

function plan(preset: Preset & { size: StatedSize }, layers: Layer[]): BoxPlan {
  const placed = layers.flatMap((l) => l.placements);
  return {
    preset,
    layers,
    loose: [],
    fill: placed.reduce((t, p) => t + volume(p.dims), 0) / volume(dims(preset.size)),
    weight_g: placed.reduce((t, p) => t + (p.kind.weight_g ?? 0), 0),
    unweighed: placed.filter((p) => p.kind.weight_g === null).length,
  };
}

function bulk(left: Left[]): number {
  return left.reduce((t, e) => t + e.count * volume(e.kind.size), 0);
}

/**
 * The boxes for what is left on the bench: the smallest box that takes it
 * all, or, when none does, the box that takes the most (the smaller of two that
 * take as much), and again for the rest.
 */
export function arrange(lines: BenchLine[], presets: Preset[]): Arrangement {
  const { pieces, own, loose, unmeasured } = piecesOf(lines);
  const boxes = presets
    .filter((p): p is Preset & { size: StatedSize } => p.size !== null)
    .sort((a, b) => volume(dims(a.size)) - volume(dims(b.size)));
  const none: Arrangement = { boxes: [], own, unmeasured, oversize: [], loose, tooMany: false };
  if (pieces.reduce((t, p) => t + p.count, 0) > MOST_PIECES) return { ...none, tooMany: true };

  const plans: BoxPlan[] = [];
  let rest = pieces;
  while (rest.length > 0 && plans.length < 50) {
    const total = bulk(rest);
    let chosen: { box: Preset & { size: StatedSize }; layers: Layer[]; left: Left[] } | null = null;
    for (const box of boxes) {
      if (volume(dims(box.size)) < total) continue;
      const p = pack(dims(box.size), rest);
      if (p.left.length === 0) {
        chosen = { box, ...p };
        break;
      }
    }
    if (!chosen) {
      let most = 0;
      for (const box of boxes) {
        const p = pack(dims(box.size), rest);
        const took = total - bulk(p.left);
        if (took > most) {
          most = took;
          chosen = { box, ...p };
        }
      }
    }
    if (!chosen) break;
    plans.push(plan(chosen.box, chosen.layers));
    rest = chosen.left;
  }

  // Loose things go round the goods in the box with the most room to spare.
  const roomiest = plans.reduce<BoxPlan | null>((best, p) => (!best || p.fill < best.fill ? p : best), null);
  if (roomiest) roomiest.loose = loose;
  const oversize = rest.map((e) => ({
    line: e.kind.line,
    item_id: e.kind.item_id,
    item_code: e.kind.item_code,
    units: e.count * e.kind.units,
  }));
  return { ...none, boxes: plans, oversize, loose: roomiest ? [] : loose };
}

/** What a layer holds, by kind, in the order they first appear in it. */
export function contentsOf(layer: Layer): { kind: Kind; count: number; stacked: number }[] {
  const out = new Map<Kind, { kind: Kind; count: number; stacked: number }>();
  for (const p of layer.placements) {
    const row = out.get(p.kind) ?? { kind: p.kind, count: 0, stacked: 0 };
    row.count += 1;
    if (p.z > layer.z) row.stacked += 1;
    out.set(p.kind, row);
  }
  return [...out.values()];
}
