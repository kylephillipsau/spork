import type { LayoutPlace, MapBin, PlanShape } from "@domain/types";

/**
 * The site in three dimensions, as plain numbers (D173): what the 3D pane
 * draws, worked out from the same plan the 2D one draws, with no renderer in
 * it.
 *
 * Coordinates are the site's own: x and y across the floor in cells, z up.
 * Turning them into a renderer's axes is the scene's job.
 *
 * **Solid places stand up as blocks; walk-through places lie flat as floor**,
 * whatever their height, because what is walked through is seen through. A
 * place with a grid shows it: a rack's bays and levels on its faces, a dock's
 * doors on its floor. That is all a glance at it has to confirm.
 */

export type Point = [number, number];
export type Point3 = [number, number, number];
export type Segment = [Point3, Point3];

export interface Block {
  place_id: string;
  name: string;
  solid: boolean;
  nesting: number;
  /** Whether choosing it means anything: everything but the outermost. */
  target: boolean;
  /** The footprint, counter-clockwise seen from above. */
  ring: Point[];
  /** Where its foot is, or where its floor lies. */
  z: number;
  /** How tall it stands: its own height when solid, nothing when it is floor. */
  height: number;
  /** The lines between its bays, levels and rows, on its faces or its floor. */
  grid: Segment[];
  /** Where a label about it goes: over the middle of its top. */
  top: Point3;
}

export interface Scene3D {
  blocks: Block[];
  /** The corners of the box everything fits in. */
  min: Point3;
  max: Point3;
}

/**
 * How far a floor is lifted for each place it is inside, so a floor drawn on
 * another floor is seen on top of it rather than flickering through it.
 */
export const FLOOR_LIFT = 0.02;

/** The scene for a site's plan, with each place's grid from the site's list. */
export function sceneOf(plan: PlanShape[], places: LayoutPlace[]): Scene3D {
  const grids = new Map(places.map((p) => [p.place_id, p]));
  const blocks: Block[] = [];
  for (const shape of plan) {
    const ring = counterClockwise(shape.corners);
    if (!ring || !Number.isFinite(shape.z)) continue;
    const solid = shape.solid;
    const height = solid ? Math.max(0, finite(shape.height)) : 0;
    const z = solid ? shape.z : shape.z + FLOOR_LIFT * shape.nesting;
    const place = grids.get(shape.place_id);
    const [cx, cy] = centroid(ring);
    blocks.push({
      place_id: shape.place_id,
      name: shape.name,
      solid,
      nesting: shape.nesting,
      target: shape.nesting > 0,
      ring,
      z,
      height,
      grid: place && shape.corners.length === 4 ? gridLines(shape.corners as Point[], z, height, place) : [],
      top: [cx, cy, z + height],
    });
  }

  if (blocks.length === 0) return { blocks, min: [0, 0, 0], max: [0, 0, 0] };
  const xs = blocks.flatMap((b) => b.ring.map((p) => p[0]));
  const ys = blocks.flatMap((b) => b.ring.map((p) => p[1]));
  const zs = blocks.flatMap((b) => [b.z, b.z + b.height]);
  return {
    blocks,
    min: [Math.min(...xs), Math.min(...ys), Math.min(...zs)],
    max: [Math.max(...xs), Math.max(...ys), Math.max(...zs)],
  };
}

/**
 * The lines of a rectangle's grid. Its corners come as the plan gives them:
 * the front left, the front right, the back right, the back left, so the bays
 * run along the front and the rows run back from it.
 *
 * - A solid place: bays down its front and back and across its top, levels
 *   around all four faces, rows across its top. A rack with a face on each
 *   side has each side's rows, and the spine between the two.
 * - A floor: bays and rows across it.
 */
export function gridLines(
  corners: Point[],
  z: number,
  height: number,
  grid: Pick<LayoutPlace, "bays" | "levels" | "rows"> & { sides?: number },
): Segment[] {
  const [a, b, c, d] = corners as [Point, Point, Point, Point];
  const out: Segment[] = [];
  const at = (p: Point, h: number): Point3 => [p[0], p[1], h];
  const top = z + height;

  for (let i = 1; i < grid.bays; i++) {
    const t = i / grid.bays;
    const front = lerp(a, b, t);
    const back = lerp(d, c, t);
    if (height > 0) {
      out.push([at(front, z), at(front, top)], [at(back, z), at(back, top)]);
    }
    out.push([at(front, top), at(back, top)]);
  }
  if (height > 0) {
    for (let j = 1; j < grid.levels; j++) {
      const h = z + (height * j) / grid.levels;
      out.push([at(a, h), at(b, h)], [at(b, h), at(c, h)], [at(c, h), at(d, h)], [at(d, h), at(a, h)]);
    }
  }
  const deep = grid.rows * (grid.sides ?? 1);
  for (let k = 1; k < deep; k++) {
    const s = k / deep;
    out.push([at(lerp(a, d, s), top), at(lerp(b, c, s), top)]);
  }
  return out;
}

/** The corners counter-clockwise from above, or nothing if they enclose no area. */
function counterClockwise(corners: [number, number][]): Point[] | null {
  if (corners.length < 3 || corners.some(([x, y]) => !Number.isFinite(x) || !Number.isFinite(y))) return null;
  const area = signedArea(corners);
  if (Math.abs(area) < 1e-9) return null;
  const ring = corners.map(([x, y]) => [x, y] as Point);
  return area > 0 ? ring : ring.reverse();
}

function signedArea(ring: [number, number][]): number {
  let sum = 0;
  for (let i = 0; i < ring.length; i++) {
    const [x1, y1] = ring[i]!;
    const [x2, y2] = ring[(i + 1) % ring.length]!;
    sum += x1 * y2 - x2 * y1;
  }
  return sum / 2;
}

/** The middle of its area. */
function centroid(ring: Point[]): Point {
  const area = signedArea(ring);
  let x = 0;
  let y = 0;
  for (let i = 0; i < ring.length; i++) {
    const [x1, y1] = ring[i]!;
    const [x2, y2] = ring[(i + 1) % ring.length]!;
    const cross = x1 * y2 - x2 * y1;
    x += (x1 + x2) * cross;
    y += (y1 + y2) * cross;
  }
  return [x / (6 * area), y / (6 * area)];
}

function lerp(p: Point, q: Point, t: number): Point {
  return [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
}

function finite(n: number): number {
  return Number.isFinite(n) ? n : 0;
}

/**
 * A bin as a box in its cell on its rack (D208): the map's unit.
 *
 * The cell is cut from its place's box the way the rack is built: its bay along
 * the front, its level up, its row back from the face it is on, and its place
 * along the bay when several bins share one. A rack with two sides is two
 * faces back to back, so the back's rows are counted in from the back.
 */
export interface BinCell {
  bin: MapBin;
  /** The middle of the cell, on the site. */
  centre: Point3;
  /** How long it runs along the rack, how deep, and how tall. */
  size: Point3;
  /** Which way the rack runs, in radians from the site's x axis. */
  angle: number;
  /** Which way the face it is on looks, out into the aisle, as a unit vector. */
  facing: Point;
}

/** How much of a cell its box fills, so neighbours read as separate bins. */
export const CELL_FILL = 0.86;

/**
 * Every bin's cell, from the plan's footprints and each place's grid. A bin
 * whose place has no rectangle on the plan, or whose cell is outside its grid,
 * is left out rather than drawn somewhere wrong.
 */
export function cellsOf(plan: PlanShape[], places: LayoutPlace[], bins: MapBin[]): BinCell[] {
  const shapes = new Map(plan.map((s) => [s.place_id, s]));
  const grids = new Map(places.map((p) => [p.place_id, p]));
  const out: BinCell[] = [];
  for (const bin of bins) {
    const shape = shapes.get(bin.place_id);
    const grid = grids.get(bin.place_id);
    if (!shape || !grid || shape.corners.length !== 4) continue;
    const cell = cellBox(shape, grid, bin);
    if (cell) out.push(cell);
  }
  return out;
}

function cellBox(shape: PlanShape, grid: LayoutPlace, bin: MapBin): BinCell | null {
  const [a, b, , d] = shape.corners as [Point, Point, Point, Point];
  const sides = Math.max(1, grid.sides);
  const across = grid.positions[bin.level - 1] ?? 1;
  const inside =
    bin.side >= 1 && bin.side <= sides &&
    bin.bay >= 1 && bin.bay <= grid.bays &&
    bin.level >= 1 && bin.level <= grid.levels &&
    bin.row >= 1 && bin.row <= grid.rows &&
    bin.position >= 1 && bin.position <= across;
  if (!inside) return null;

  const length = Math.hypot(b[0] - a[0], b[1] - a[1]);
  const depth = Math.hypot(d[0] - a[0], d[1] - a[1]);
  if (length === 0 || depth === 0) return null;
  const u: Point = [(b[0] - a[0]) / length, (b[1] - a[1]) / length];
  const v: Point = [(d[0] - a[0]) / depth, (d[1] - a[1]) / depth];

  // Along: the bay, then its share of it. On the back, the first position is
  // the left one as the back is faced, which is the far end of the bay.
  const bay = length / grid.bays;
  const share = bay / across;
  const nth = bin.side === 2 ? across - bin.position : bin.position - 1;
  const along = (bin.bay - 1) * bay + nth * share + share / 2;

  // Back: the front's rows from the front, the back's from the back.
  const slabs = sides * grid.rows;
  const slab = depth / slabs;
  const index = bin.side === 2 ? slabs - bin.row : bin.row - 1;
  const back = index * slab + slab / 2;

  // Up: a solid place's levels share its height; a floor's cells lie on it.
  const solid = shape.solid && shape.height > 0;
  const level = solid ? shape.height / grid.levels : Math.min(share, slab) * 0.3;
  const up = solid ? shape.z + (bin.level - 1) * level + level / 2 : shape.z + level / 2;

  const facing: Point = bin.side === 2 ? v : [-v[0], -v[1]];
  return {
    bin,
    centre: [a[0] + u[0] * along + v[0] * back, a[1] + u[1] * along + v[1] * back, up],
    size: [share * CELL_FILL, slab * CELL_FILL, level * CELL_FILL],
    angle: Math.atan2(u[1], u[0]),
    facing,
  };
}
