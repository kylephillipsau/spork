import type { BenchLine, PackUnit, Preset, StatedSize, Uuid } from "@domain/types";

import { roundLook, type RoundLook } from "../../items/round.ts";
import { DEFAULT_OBJECTIVE, type Freight, type Objective } from "./freight.ts";

/**
 * How what is left to pack would go into boxes (D195): which box, and what goes
 * where in it, layer by layer from the bottom.
 *
 * **A suggestion made from what is recorded.** An each with no size is listed,
 * not guessed at from its carton, so the packer can measure it at the bench and
 * the suggestion is made again. What ships as it is (D196), a carton unless
 * somebody said otherwise, or a roll in its own box once somebody says so,
 * goes to the carrier on its own and is left out of the boxes. A thing with no
 * size to measure (D138) goes in round the rest.
 *
 * **Layers, because a packer builds one.** Each layer starts with the biggest
 * thing that still fits, laid on its biggest side, and is filled round it and on
 * top of the shorter things in it, largest first. It is a heuristic, not the
 * best packing there is: good enough to choose a box by, and simple enough that
 * the layers it makes can be followed with the goods in hand.
 *
 * **Which boxes is a setting** (D224): the ways tried are scored by an
 * objective, fewest parcels unless the workspace says otherwise.
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
  /** A round thing's shape and wrapping, to draw it as the tub it is (D240); null for a box. */
  round: RoundLook | null;
  /** It stays the way up it stands (D200): turned round, never onto its side. */
  upright: boolean;
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
  /** Filling the open carton (D198): whether it is in already, the first of each
   *  kind in the order the layers go in. Absent in a box not yet started. */
  packed?: boolean;
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
  /** What they weigh together by the record, or null when any has no weight. */
  weight_g: number | null;
}

/** Things that go to the carrier as they are (D196): so many of one level of a line's item. */
export interface AsIs extends Aside {
  level: PackUnit["level"];
  /** How many parcels: one each. */
  count: number;
  /** Eaches in one of them. */
  per: number;
  size: Dims | null;
  /** Its sides, or its shape when round (D240), to draw it with, and its line's place, for its colour. */
  faces: PackUnit["faces"];
  round: RoundLook | null;
  index: number;
}

export interface BoxPlan {
  preset: Preset & { size: StatedSize };
  /** The carton open on the bench, when this is the plan for filling it. */
  carton?: { id: Uuid; sequence: string };
  layers: Layer[];
  /** Things with no size to measure, put in round the rest of this box. */
  loose: Aside[];
  /** The share of the box's inside the placed goods take up. */
  fill: number;
  /** What the goods in it weigh by the record, loose things included, and how
   *  many pieces or loose units have no weight. */
  weight_g: number;
  unweighed: number;
  /** What the box weighs empty, when the workspace says (D224). */
  tare_g: number | null;
}

export interface Arrangement {
  boxes: BoxPlan[];
  /** What goes to the carrier as it is, parcel by parcel (D196). */
  asIs: AsIs[];
  /** No size recorded at the level they would go in at: measure, then it is placed. */
  unmeasured: Aside[];
  /** Bigger than every box, in any way up. */
  oversize: Aside[];
  /** Things with no size, when there is no box for them to go round. */
  loose: Aside[];
  /** The ones still to pack that went round a box, and which box (D202):
   *  a box's `loose` also holds what is in the open carton already. */
  placedLoose: Aside[];
  looseBox: number | null;
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

/** Every distinct way up that fits in `room`; for a thing kept upright, only turned round (D200). */
function fitting(size: Dims, room: Dims, upright = false): Way[] {
  const seen = new Set<string>();
  const out: Way[] = [];
  for (const axes of WAYS) {
    if (upright && axes[2] !== 2) continue;
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
function flattest(size: Dims, room: Dims, upright = false): Way | null {
  let best: Way | null = null;
  let bestTiles = 0;
  for (const w of fitting(size, room, upright)) {
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

/**
 * What a box's goods may weigh, and what they weigh so far (D199). A thing with
 * no recorded weight weighs nothing here: the limit is the record's, and the
 * box says how many pieces it could not count.
 */
interface Budget {
  limit: number | null;
  used: number;
}

function affords(budget: Budget, kind: Kind): boolean {
  return budget.limit === null || budget.used + (kind.weight_g ?? 0) <= budget.limit;
}

/** Put the biggest thing that fits into the corner of `space`, then fill round and on top of it. */
function fill(space: Space, left: Left[], out: Placement[], budget: Budget): void {
  const [l, w, h] = space.room;
  if (l <= 0 || w <= 0 || h <= 0) return;
  for (const entry of left) {
    if (entry.count === 0 || !affords(budget, entry.kind)) continue;
    const way = flattest(entry.kind.size, space.room, entry.kind.upright);
    if (!way) continue;
    entry.count -= 1;
    budget.used += entry.kind.weight_g ?? 0;
    out.push({ kind: entry.kind, x: space.x, y: space.y, z: space.z, dims: way.dims, axes: way.axes });
    round(space, way.dims, left, out, budget);
    return;
  }
}

/**
 * Fill what is left of `space` once something of `placed` size is in its
 * corner: on top of it, up to the space's height, then the floor beside it,
 * cut so that the bigger of the two pieces of floor stays whole.
 */
function round(space: Space, placed: Dims, left: Left[], out: Placement[], budget: Budget): void {
  const [l, w, h] = space.room;
  const [pl, pw, ph] = placed;
  if (h > ph) fill({ x: space.x, y: space.y, z: space.z + ph, room: [pl, pw, h - ph] }, left, out, budget);
  const beside = l - pl;
  const before = w - pw;
  if (beside * w >= before * l) {
    fill({ x: space.x + pl, y: space.y, z: space.z, room: [beside, w, h] }, left, out, budget);
    fill({ x: space.x, y: space.y + pw, z: space.z, room: [pl, before, h] }, left, out, budget);
  } else {
    fill({ x: space.x, y: space.y + pw, z: space.z, room: [l, before, h] }, left, out, budget);
    fill({ x: space.x + pl, y: space.y, z: space.z, room: [beside, pw, h] }, left, out, budget);
  }
}

/**
 * Pack `left` into a box of `size`, layer on layer, its goods weighing no more
 * than `maxWeight` when the box has one (D199). What does not go in stays in
 * `left`.
 */
export function pack(size: Dims, pieces: Left[], maxWeight: number | null = null): { layers: Layer[]; left: Left[] } {
  const left = sorted(pieces.map((p) => ({ ...p })));
  const layers: Layer[] = [];
  const budget: Budget = { limit: maxWeight, used: 0 };
  let z = 0;
  for (;;) {
    // A layer starts with the biggest thing that still lies in the height left.
    let start: { entry: Left; way: Way } | null = null;
    for (const entry of left) {
      if (entry.count === 0 || !affords(budget, entry.kind)) continue;
      const way = flattest(entry.kind.size, [size[0], size[1], size[2] - z], entry.kind.upright);
      if (way) {
        start = { entry, way };
        break;
      }
    }
    if (!start) break;
    const height = start.way.dims[2];
    const placements: Placement[] = [{ kind: start.entry.kind, x: 0, y: 0, z, dims: start.way.dims, axes: start.way.axes }];
    start.entry.count -= 1;
    budget.used += start.entry.kind.weight_g ?? 0;
    round({ x: 0, y: 0, z, room: [size[0], size[1], height] }, start.way.dims, left, placements, budget);
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

/** One kind per line and level, so the same thing in the open carton and still to pack is one kind. */
type KindOf = (line: BenchLine, index: number, unit: PackUnit, size: Dims) => Kind;

function kinds(): KindOf {
  const made = new Map<string, Kind>();
  return (line, index, unit, size) => {
    const key = `${line.line_id}:${unit.level}`;
    let kind = made.get(key);
    if (!kind) {
      kind = {
        line: line.line_id,
        item_id: line.item_id,
        item_code: line.item_code,
        level: unit.level,
        units: unit.units,
        size,
        weight_g: unit.gross_weight_g,
        faces: unit.faces,
        round: roundLook(unit),
        upright: unit.upright,
        index,
      };
      made.set(key, kind);
    }
    return kind;
  };
}

/**
 * Units of each line, as what they go as: cartons while the count fills one,
 * then inner packs, then eaches. Outside a box, a level that ships as it is
 * (D196) goes on its own; in one, nothing does.
 */
function split(
  lines: BenchLine[],
  unitsOf: (line: BenchLine) => number,
  inBox: boolean,
  kindOf: KindOf,
): { pieces: Left[]; asIs: AsIs[]; loose: Aside[]; unmeasured: Aside[] } {
  const pieces: Left[] = [];
  const asIs: AsIs[] = [];
  const loose: Aside[] = [];
  const unmeasured: Aside[] = [];
  lines.forEach((line, index) => {
    let units = Math.max(0, unitsOf(line));
    if (units === 0) return;
    const aside = (n: number, unit?: PackUnit): Aside => ({
      line: line.line_id,
      item_id: line.item_id,
      item_code: line.item_code,
      units: n,
      weight_g: unit?.gross_weight_g != null ? (unit.gross_weight_g * n) / unit.units : null,
    });
    for (const level of ["carton", "inner", "each"] as const) {
      const unit = line.packs.find((p) => p.level === level);
      if (!unit || unit.units <= 0 || units < unit.units) continue;
      const size = measured(unit);
      const alone = unit.ships_as_is && !inBox;
      if (level !== "each" && !alone && !size) continue;
      const count = Math.floor(units / unit.units);
      if (alone) asIs.push({ ...aside(count * unit.units, unit), level, count, per: unit.units, size, faces: unit.faces, round: roundLook(unit), index });
      else if (size) pieces.push({ kind: kindOf(line, index, unit, size), count });
      else if (unit.no_size) loose.push(aside(count, unit));
      // Not measured, but what it weighs is what the record says all the same (D224).
      else unmeasured.push(aside(count, unit));
      units -= count * unit.units;
    }
    // Nothing recorded of its each at all: it is not measured.
    if (units > 0) unmeasured.push(aside(units));
  });
  return { pieces, asIs, loose, unmeasured };
}

/**
 * What each line's units go as, biggest first: cartons while the count fills
 * one, then inner packs, then eaches. A level that ships as it is (D196) goes
 * on its own; one that does not goes into a box when its size is recorded, and
 * otherwise its units are taken a level down. Eaches with no size are listed
 * as not measured, or go in loose when they have none to measure.
 */
export function piecesOf(lines: BenchLine[]): {
  pieces: Left[];
  asIs: AsIs[];
  loose: Aside[];
  unmeasured: Aside[];
} {
  return split(lines, (l) => l.remaining, false, kinds());
}

/** The carton open on the bench: its box, and what is in it already. */
export interface OpenCarton {
  id: Uuid;
  sequence: string;
  /** Its box's name. */
  name: string;
  size: StatedSize;
  /** The most its goods may weigh, when its box says (D199). */
  max_payload_g: number | null;
  /** What its box weighs empty, when said (D224). */
  tare_weight_g: number | null;
  contents: { item_id: Uuid; quantity: number }[];
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
    tare_g: preset.tare_weight_g,
  };
}

/** A box as a carrier sees it (D224): its size, and its goods and itself by the record. */
export function boxFreight(b: BoxPlan): Freight {
  return { count: 1, size: dims(b.preset.size), weight_g: b.weight_g + (b.tare_g ?? 0) };
}

/** One way of boxing what is left: its boxes, and what fitted none of them. */
interface Candidate {
  plans: BoxPlan[];
  left: Left[];
}

type Box = Preset & { size: StatedSize };

/**
 * The smallest box that takes it all, or, when none does, the box that takes
 * the most (the smaller of two that take as much), and again for the rest.
 */
function greedy(pieces: Left[], boxes: Box[]): Candidate {
  const plans: BoxPlan[] = [];
  let rest = pieces;
  while (rest.length > 0 && plans.length < 50) {
    const total = bulk(rest);
    let chosen: { box: Box; layers: Layer[]; left: Left[] } | null = null;
    for (const box of boxes) {
      if (volume(dims(box.size)) < total) continue;
      const p = pack(dims(box.size), rest, box.max_payload_g);
      if (p.left.length === 0) {
        chosen = { box, ...p };
        break;
      }
    }
    if (!chosen) {
      let most = 0;
      for (const box of boxes) {
        const p = pack(dims(box.size), rest, box.max_payload_g);
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
  return { plans, left: rest };
}

/** Boxes of one kind only, filled one after another until nothing more goes in. */
function uniform(pieces: Left[], box: Box): Candidate {
  const plans: BoxPlan[] = [];
  let rest = pieces;
  while (rest.length > 0 && plans.length < 50) {
    const p = pack(dims(box.size), rest, box.max_payload_g);
    if (bulk(p.left) === bulk(rest)) break;
    plans.push(plan(box, p.layers));
    rest = p.left;
  }
  return { plans, left: rest };
}

/** Past this many pieces, only the first way is tried: the others cost more than they save. */
const MOST_TRIED = 300;

/**
 * The best way to box `pieces` by `objective` (D224): of the ways tried, the
 * one that leaves least unboxed, then costs least.
 */
function best(pieces: Left[], boxes: Box[], objective: Objective): Candidate {
  const tried = [greedy(pieces, boxes)];
  if (pieces.reduce((t, p) => t + p.count, 0) <= MOST_TRIED) for (const box of boxes) tried.push(uniform(pieces, box));
  const score = (c: Candidate) => [bulk(c.left), objective.cost(c.plans.map(boxFreight))] as const;
  return tried.reduce((a, b) => {
    const [la, ca] = score(a);
    const [lb, cb] = score(b);
    return lb < la || (lb === la && cb < ca) ? b : a;
  });
}

function bulk(left: Left[]): number {
  return left.reduce((t, e) => t + e.count * volume(e.kind.size), 0);
}

/**
 * The boxes for what is left on the bench, the open carton first (D198), the
 * rest boxed the way `objective` prices lowest (D224).
 */
export function arrange(
  lines: BenchLine[],
  presets: Preset[],
  open: OpenCarton | null = null,
  objective: Objective = DEFAULT_OBJECTIVE,
): Arrangement {
  const kindOf = kinds();
  const { pieces, asIs, loose, unmeasured } = split(lines, (l) => l.remaining, false, kindOf);
  // Only the boxes the workspace lets it choose (D196).
  const boxes = presets
    .filter((p): p is Box => p.size !== null && p.suggested)
    .sort((a, b) => volume(dims(a.size)) - volume(dims(b.size)));
  const none: Arrangement = { boxes: [], asIs, unmeasured, oversize: [], loose, placedLoose: [], looseBox: null, tooMany: false };
  if (pieces.reduce((t, p) => t + p.count, 0) > MOST_PIECES) return { ...none, tooMany: true };

  const plans: BoxPlan[] = [];
  let rest = pieces;
  const extraLoose: Aside[] = [];
  if (open) {
    const filled = fillOpen(open, lines, pieces, kindOf);
    plans.push(filled.plan);
    extraLoose.push(...filled.loose);
    rest = filled.rest;
  }
  if (rest.length > 0) {
    const chosen = best(rest, boxes, objective);
    plans.push(...chosen.plans);
    rest = chosen.left;
  }

  // Loose things go round the goods in the box with the most room to spare;
  // what is loose in the open carton already is in it.
  const roomiest = plans.reduce<BoxPlan | null>((best, p) => (!best || p.fill < best.fill ? p : best), null);
  const first = plans[0];
  if (first?.carton) {
    first.loose = extraLoose;
    for (const a of extraLoose) {
      if (a.weight_g === null) first.unweighed += a.units;
      else first.weight_g += a.weight_g;
    }
  }
  if (roomiest) {
    roomiest.loose = [...roomiest.loose, ...loose];
    for (const a of loose) {
      if (a.weight_g === null) roomiest.unweighed += a.units;
      else roomiest.weight_g += a.weight_g;
    }
  }
  const oversize = rest.map((e) => ({
    line: e.kind.line,
    item_id: e.kind.item_id,
    item_code: e.kind.item_code,
    units: e.count * e.kind.units,
    weight_g: e.kind.weight_g === null ? null : e.kind.weight_g * e.count,
  }));
  return {
    ...none,
    boxes: plans,
    oversize,
    loose: roomiest ? [] : loose,
    placedLoose: roomiest ? loose : [],
    looseBox: roomiest ? plans.indexOf(roomiest) : null,
  };
}

/**
 * The open carton, filled (D198): what is in it and what is still to pack,
 * arranged together, the first of each kind in the order the layers go in
 * marked as in already. What is in it plus what is left is what there was, so
 * a packer following the layers sees the same plan after every press. What
 * does not fit is left for the next box; what is in it is never.
 */
function fillOpen(
  open: OpenCarton,
  lines: BenchLine[],
  remaining: Left[],
  kindOf: KindOf,
): { plan: BoxPlan; rest: Left[]; loose: Aside[] } {
  const inCarton = new Map<Uuid, number>();
  for (const c of open.contents) inCarton.set(c.item_id, (inCarton.get(c.item_id) ?? 0) + c.quantity);
  // A product on two lines is counted against the first.
  const seen = new Set<Uuid>();
  const unitsIn = (l: BenchLine) => {
    if (seen.has(l.item_id)) return 0;
    seen.add(l.item_id);
    return inCarton.get(l.item_id) ?? 0;
  };
  const already = split(lines, unitsIn, true, kindOf);
  const together = new Map<Kind, number>();
  for (const p of [...already.pieces, ...remaining]) together.set(p.kind, (together.get(p.kind) ?? 0) + p.count);
  const packed = pack(
    [open.size.length_mm, open.size.width_mm, open.size.height_mm],
    [...together].map(([kind, count]) => ({ kind, count })),
    open.max_payload_g,
  );
  const toMark = new Map(already.pieces.map((p) => [p.kind, p.count]));
  for (const layer of packed.layers) {
    for (const p of layer.placements) {
      const n = toMark.get(p.kind) ?? 0;
      p.packed = n > 0;
      if (n > 0) toMark.set(p.kind, n - 1);
    }
  }
  const stillToPack = new Map(remaining.map((p) => [p.kind, p.count]));
  const rest = packed.left
    .map((e) => ({ kind: e.kind, count: Math.min(e.count, stillToPack.get(e.kind) ?? 0) }))
    .filter((e) => e.count > 0);
  const preset = { id: open.id, name: open.name, size: open.size, suggested: true, max_payload_g: open.max_payload_g, tare_weight_g: open.tare_weight_g };
  return {
    plan: { ...plan(preset, packed.layers), carton: { id: open.id, sequence: open.sequence } },
    rest,
    loose: [...already.loose, ...already.unmeasured],
  };
}

/** What a layer holds, by kind, in the order they first appear in it. */
export function contentsOf(layer: Layer): { kind: Kind; count: number; stacked: number; packed: number }[] {
  const out = new Map<Kind, { kind: Kind; count: number; stacked: number; packed: number }>();
  for (const p of layer.placements) {
    const row = out.get(p.kind) ?? { kind: p.kind, count: 0, stacked: 0, packed: 0 };
    row.count += 1;
    if (p.z > layer.z) row.stacked += 1;
    if (p.packed) row.packed += 1;
    out.set(p.kind, row);
  }
  return [...out.values()];
}
