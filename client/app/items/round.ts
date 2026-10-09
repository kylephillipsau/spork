import type { Wrap } from "@domain/types";

import type { Pixels } from "./cut.ts";

/**
 * A round thing as the shape it was measured to be, and its photographs
 * wrapped round it (D240). No DOM here, so a test runner can read it.
 *
 * **Geometry, not a model's guess.** A bucket is a surface of revolution
 * whose widths and heights are on file (D213). For each photograph of its
 * side, the camera that took it is found by fitting the shape's outline to
 * the outline the face-finder sees; every point of the surface then has one
 * place in that photograph, and the label is read off it where it faces the
 * camera. Four quarter turns, laid side by side and blended where they
 * overlap, are the whole label: the photograph's own pixels, never redrawn.
 *
 * Millimetres throughout; angles in radians; `θ` round from the front
 * (the side facing the camera in the photograph of `front`) toward its right.
 */

/** A round thing's widths and heights, in millimetres, as D213 measures them. */
export interface RoundSize {
  /** Across the top, at the rim. */
  top: number;
  /** Across the base; the top's when it does not taper. */
  base: number;
  /** The whole height, lid on. */
  height: number;
  /** How far down from the rim it stays straight; 0 when it tapers all the way. */
  band: number;
}

/** Its size from what is on file, or nothing until the top and the height are. */
export function roundSize(s: {
  diameter_mm: number | null;
  base_diameter_mm: number | null;
  height_mm: number | null;
  top_height_mm: number | null;
}): RoundSize | null {
  if (!s.diameter_mm || !s.height_mm) return null;
  const band = Math.min(Math.max(s.top_height_mm ?? 0, 0), s.height_mm);
  return { top: s.diameter_mm, base: s.base_diameter_mm || s.diameter_mm, height: s.height_mm, band };
}

/** A point of its outline from the base up: how far out, and how high. */
export interface Ring {
  r: number;
  y: number;
}

/** Its outline from the base to the rim: tapering, then straight under the rim. */
export function profile(s: RoundSize): Ring[] {
  const rings: Ring[] = [{ r: s.base / 2, y: 0 }];
  if (s.band > 0 && s.band < s.height) rings.push({ r: s.top / 2, y: s.height - s.band });
  rings.push({ r: s.top / 2, y: s.height });
  return rings;
}

/** The length of its side from base to rim, measured along it. */
export function slant(rings: readonly Ring[]): number {
  let s = 0;
  for (let i = 1; i < rings.length; i++) s += Math.hypot(rings[i]!.r - rings[i - 1]!.r, rings[i]!.y - rings[i - 1]!.y);
  return s;
}

/**
 * The point of its side so far up it, measured along it from the base, with
 * the side's outward slope there: `nr` out, `ny` up, of length 1.
 */
export function along(rings: readonly Ring[], s: number): { r: number; y: number; nr: number; ny: number } {
  for (let i = 1; i < rings.length; i++) {
    const a = rings[i - 1]!, b = rings[i]!;
    const len = Math.hypot(b.r - a.r, b.y - a.y);
    if (s <= len || i === rings.length - 1) {
      const t = len > 0 ? Math.min(Math.max(s / len, 0), 1) : 0;
      // Outward, perpendicular to the side: a side leaning in faces up a little.
      return { r: a.r + (b.r - a.r) * t, y: a.y + (b.y - a.y) * t, nr: (b.y - a.y) / len, ny: -(b.r - a.r) / len };
    }
    s -= len;
  }
  const last = rings[rings.length - 1]!;
  return { r: last.r, y: last.y, nr: 1, ny: 0 };
}

/**
 * The camera, as a pinhole: its focal length and the middle of the
 * photograph, in the photograph's pixels. A phone's photograph keeps no lens
 * once it is a WebP, so a phone's ordinary lens is assumed: 26 mm as a 35 mm
 * camera has it, three quarters of the longest side.
 */
export interface Lens {
  f: number;
  cx: number;
  cy: number;
}

export function phoneLens(width: number, height: number): Lens {
  return { f: 0.75 * Math.max(width, height), cx: width / 2, cy: height / 2 };
}

/** The lens, for the same photograph at another size. */
export function scaled(lens: Lens, by: number): Lens {
  return { f: lens.f * by, cx: lens.cx * by, cy: lens.cy * by };
}

/**
 * Where the camera stood and how it was held: so far out from the thing's
 * middle and so high, looking at it, turned so far left or right (`yaw`),
 * tipped down or up (`pitch`) and rolled. Five numbers: a round thing looks
 * the same from every side, so which side is in front is the photograph's
 * face, not the camera's.
 */
export interface Pose {
  d: number;
  h: number;
  yaw: number;
  pitch: number;
  roll: number;
}

/** A point of the surface in the camera's frame: x right, y down, z ahead. */
function toCamera(p: Pose, x: number, y: number, z: number): [number, number, number] {
  // Looking from (0, h, d) toward the axis, level.
  let cx = x, cy = p.h - y, cz = p.d - z;
  // Turned, then tipped, then rolled.
  const ca = Math.cos(p.yaw), sa = Math.sin(p.yaw);
  [cx, cz] = [ca * cx + sa * cz, -sa * cx + ca * cz];
  const cb = Math.cos(p.pitch), sb = Math.sin(p.pitch);
  [cy, cz] = [cb * cy - sb * cz, sb * cy + cb * cz];
  const cc = Math.cos(p.roll), sc = Math.sin(p.roll);
  [cx, cy] = [cc * cx - sc * cy, sc * cx + cc * cy];
  return [cx, cy, cz];
}

/** Where a point of the surface falls in the photograph, `θ` round from the side facing the camera; null behind it. */
export function project(p: Pose, lens: Lens, theta: number, r: number, y: number): [number, number] | null {
  const [x, yc, z] = toCamera(p, r * Math.sin(theta), y, r * Math.cos(theta));
  if (z <= 1) return null;
  return [lens.f * (x / z) + lens.cx, lens.f * (yc / z) + lens.cy];
}

/**
 * How squarely the surface faces the camera there: the cosine of the angle
 * between its outward normal and the way to the camera. At or below 0 it is
 * round the back.
 */
export function facing(p: Pose, theta: number, at: { r: number; y: number; nr: number; ny: number }): number {
  const sx = at.r * Math.sin(theta), sz = at.r * Math.cos(theta);
  const vx = -sx, vy = p.h - at.y, vz = p.d - sz;
  const len = Math.hypot(vx, vy, vz);
  return (at.nr * Math.sin(theta) * vx + at.ny * vy + at.nr * Math.cos(theta) * vz) / len;
}

type P2 = [number, number];
const cross = (o: P2, a: P2, b: P2) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);

/** Its outline in the photograph: the convex hull of its rings, rim and base, as seen. */
export function outlineOf(p: Pose, lens: Lens, rings: readonly Ring[], steps = 96): P2[] {
  const pts: P2[] = [];
  for (const ring of rings) {
    for (let k = 0; k < steps; k++) {
      const at = project(p, lens, (2 * Math.PI * k) / steps, ring.r, ring.y);
      if (!at) return [];
      pts.push(at);
    }
  }
  pts.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const lower: P2[] = [], upper: P2[] = [];
  for (const q of pts) {
    while (lower.length >= 2 && cross(lower.at(-2)!, lower.at(-1)!, q) <= 0) lower.pop();
    lower.push(q);
  }
  for (const q of pts.reverse()) {
    while (upper.length >= 2 && cross(upper.at(-2)!, upper.at(-1)!, q) <= 0) upper.pop();
    upper.push(q);
  }
  return lower.slice(0, -1).concat(upper.slice(0, -1));
}

/** An outline the face-finder found: yes or no per pixel, row by row. */
export interface Mask {
  data: Uint8Array;
  width: number;
  height: number;
}

/** Each row's running count of the mask, so any stretch of a row is counted at once. */
function runningCounts(m: Mask): { rows: Uint32Array; area: number } {
  const rows = new Uint32Array(m.height * (m.width + 1));
  let area = 0;
  for (let y = 0; y < m.height; y++) {
    let n = 0;
    const at = y * (m.width + 1);
    for (let x = 0; x < m.width; x++) {
      n += m.data[y * m.width + x]!;
      rows[at + x + 1] = n;
    }
    area += n;
  }
  return { rows, area };
}

/** How well an outline covers a mask: what both cover over what either does. */
function overlap(poly: P2[], m: Mask, counts: { rows: Uint32Array; area: number }): number {
  if (poly.length < 3) return 0;
  let both = 0, shape = 0;
  for (let y = 0; y < m.height; y++) {
    const yy = y + 0.5;
    let lo = Infinity, hi = -Infinity;
    for (let i = 0; i < poly.length; i++) {
      const a = poly[i]!, b = poly[(i + 1) % poly.length]!;
      if ((a[1] <= yy && b[1] > yy) || (b[1] <= yy && a[1] > yy)) {
        const x = a[0] + ((yy - a[1]) / (b[1] - a[1])) * (b[0] - a[0]);
        lo = Math.min(lo, x);
        hi = Math.max(hi, x);
      }
    }
    if (hi <= lo) continue;
    const x0 = Math.min(Math.max(Math.round(lo), 0), m.width), x1 = Math.min(Math.max(Math.round(hi), 0), m.width);
    shape += hi - lo;
    const at = y * (m.width + 1);
    both += counts.rows[at + x1]! - counts.rows[at + x0]!;
  }
  return both / (counts.area + shape - both);
}

/** Nelder and Mead's simplex, minimising `f` from `start`, each number first stepped by its `step`. */
export function minimise(f: (x: number[]) => number, start: number[], step: number[], rounds = 400): { x: number[]; value: number } {
  const n = start.length;
  let simplex = [start, ...start.map((_, i) => start.map((v, j) => (i === j ? v + step[j]! : v)))].map((x) => ({ x, value: f(x) }));
  for (let k = 0; k < rounds; k++) {
    simplex.sort((a, b) => a.value - b.value);
    const best = simplex[0]!, worst = simplex[n]!, next = simplex[n - 1]!;
    const middle = start.map((_, j) => simplex.slice(0, n).reduce((s, p) => s + p.x[j]!, 0) / n);
    const toward = (t: number) => middle.map((m, j) => m + t * (worst.x[j]! - m));
    const reflected = toward(-1);
    const r = f(reflected);
    if (r < best.value) {
      const expanded = toward(-2);
      const e = f(expanded);
      simplex[n] = e < r ? { x: expanded, value: e } : { x: reflected, value: r };
    } else if (r < next.value) {
      simplex[n] = { x: reflected, value: r };
    } else {
      const contracted = toward(r < worst.value ? -0.5 : 0.5);
      const c = f(contracted);
      if (c < Math.min(r, worst.value)) simplex[n] = { x: contracted, value: c };
      else simplex = simplex.map((p, i) => (i === 0 ? p : { x: p.x.map((v, j) => best.x[j]! + 0.5 * (v - best.x[j]!)), value: NaN })).map((p) => (Number.isNaN(p.value) ? { x: p.x, value: f(p.x) } : p));
    }
  }
  simplex.sort((a, b) => a.value - b.value);
  return simplex[0]!;
}

const poseOf = (x: number[]): Pose => ({ d: x[0]!, h: x[1]!, yaw: x[2]!, pitch: x[3]!, roll: x[4]! });
const numbersOf = (p: Pose) => [p.d, p.h, p.yaw, p.pitch, p.roll];

/**
 * What an outline of it could be: the whole thing, or its body without the
 * straight band under the rim, which the face-finder can take for a part of
 * its own, a lid. The body stands where the whole does, so a pose fitted to
 * either is the whole's.
 */
export function readings(size: RoundSize): RoundSize[] {
  const whole = [size];
  if (size.band > 0 && size.band < size.height) whole.push({ ...size, height: size.height - size.band, band: 0 });
  return whole;
}

/** A mask at half its size, a pixel kept where any of the four it stands for was. */
function halved(m: Mask): Mask {
  const width = Math.floor(m.width / 2), height = Math.floor(m.height / 2);
  const data = new Uint8Array(width * height);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const at = 2 * y * m.width + 2 * x;
      data[y * width + x] = (m.data[at]! + m.data[at + 1]! + m.data[at + m.width]! + m.data[at + m.width + 1]!) >= 2 ? 1 : 0;
    }
  }
  return { data, width, height };
}

/** Where the outline is, to start from: its bounding box. */
function bounds(m: Mask): { x0: number; x1: number; y0: number; y1: number } | null {
  let x0 = m.width, x1 = -1, y0 = m.height, y1 = -1;
  for (let y = 0; y < m.height; y++) {
    for (let x = 0; x < m.width; x++) {
      if (!m.data[y * m.width + x]) continue;
      x0 = Math.min(x0, x);
      x1 = Math.max(x1, x);
      y0 = Math.min(y0, y);
      y1 = Math.max(y1, y);
    }
  }
  return x1 < 0 ? null : { x0, x1, y0, y1 };
}

/** The pose that best fits one outline read as one shape, started from a few heights. */
/** Where a photograph was taken from: round its side, or looking down on its top. */
export type Seen = "side" | "above";

/** A camera that would be inside the thing or under it: no photograph was taken from there. */
const impossible = (p: Pose, size: RoundSize) => (p.d < size.top / 2 && p.h < size.height + 10) || p.h < -size.height;

function fitOne(mask: Mask, lens: Lens, size: RoundSize, seen: Seen, from?: Pose): { pose: Pose; fit: number } | null {
  const rings = profile(size);
  const counts = runningCounts(mask);
  const cost = (x: number[]) => {
    const p = poseOf(x);
    if (impossible(p, size)) return 2;
    return 1 - overlap(outlineOf(p, lens, rings, 48), mask, counts);
  };
  if (from) {
    const done = minimise(cost, numbersOf(from), [from.d * 0.02, size.height * 0.05, 0.005, 0.01, 0.005], 300);
    return { pose: poseOf(done.x), fit: 1 - done.value };
  }
  const box = bounds(mask);
  if (!box) return null;
  const away = (lens.f * size.top) / Math.max(box.x1 - box.x0, 1);
  const yaw = Math.atan(((box.x0 + box.x1) / 2 - lens.cx) / lens.f);
  const middle = Math.atan(((box.y0 + box.y1) / 2 - lens.cy) / lens.f);
  let best: { x: number[]; value: number } | null = null;
  // From its side, a bucket from above and one from level look alike at
  // first; from above, the camera stands over it, a little toward its front.
  const starts =
    seen === "side"
      ? [0.5, 1, 1.6, 2.4].map((above) => ({ d: away, h: size.height * above, at: size.height / 2 }))
      : [0.05, 0.25, 0.5].map((off) => ({ d: away * off + size.top / 2, h: size.height + away, at: size.height }));
  for (const s of starts) {
    // Tipped so what it looks at falls where the outline's middle is.
    const pitch = Math.atan2(s.h - s.at, s.d) - middle;
    const tried = minimise(cost, [s.d, s.h, yaw, pitch, 0], [away * 0.1, size.height * 0.3, 0.03, 0.05, 0.02], 250);
    if (!best || tried.value < best.value) best = tried;
  }
  return { pose: poseOf(best!.x), fit: 1 - best!.value };
}

/**
 * Where the camera stood for a photograph of its side, or of its top looking
 * down: of the face-finder's outlines, the one the measured shape fits best,
 * whole or without its band, and the pose that fits it. `lens` is at the
 * outlines' size. Searched at half size, then put right at full size.
 */
export function fitPose(masks: Mask[], lens: Lens, size: RoundSize, seen: Seen = "side"): { pose: Pose; fit: number; mask: number } | null {
  let best: { pose: Pose; fit: number; mask: number; reading: RoundSize; score: number } | null = null;
  const half = scaled(lens, 0.5);
  masks.forEach((mask, k) => {
    const small = halved(mask);
    readings(size).forEach((reading, r) => {
      const found = fitOne(small, half, reading, seen);
      // As good either way, the whole says more: its rim and its lid.
      const score = found ? found.fit + (r === 0 ? 0.01 : 0) : -1;
      if (found && (!best || score > best.score)) best = { ...found, mask: k, reading, score };
    });
  });
  if (!best) return null;
  const { mask, reading, pose } = best as { pose: Pose; mask: number; reading: RoundSize };
  const done = fitOne(masks[mask]!, lens, reading, seen, pose)!;
  return { pose: done.pose, fit: done.fit, mask };
}

/**
 * One photograph unwrapped: for every point of the side, its colour as the
 * photograph shows it and how much to trust it (how squarely it faced the
 * camera, and not at the photograph's edge). `width` runs once round from the
 * back, the front in the middle; `height` from the rim down.
 */
export interface Layer {
  width: number;
  height: number;
  rgb: Float32Array;
  weight: Float32Array;
}

/** How a photograph sits round the thing: its camera, and which way round it was turned. */
export interface View {
  photo: Pixels;
  lens: Lens;
  pose: Pose;
  /** Round from the front to the side this photograph faces: 0, a quarter turn, … */
  turn: number;
  /** The outline the face-finder saw it in, at any size: nothing outside it is the thing. */
  within?: Mask | undefined;
}

/** How sharply trust falls away from facing the camera: a side seen at an angle is stretched and soft. */
const SQUARELY = 4;

/**
 * Whether the camera sees a point inside an open thing (D241): the line from
 * it to the camera leaves through the opening, under the rim, not through the
 * wall. Inside, the thing is convex, so the line leaves it once.
 */
export function throughTheTop(p: Pose, size: RoundSize, x: number, y: number, z: number): boolean {
  if (p.h <= size.height) return false;
  const t = (size.height - y) / (p.h - y);
  const qx = x * (1 - t), qz = z + (p.d - z) * t;
  return Math.hypot(qx, qz) < size.top / 2;
}

/** A photograph's colour at a point of it, read between its pixels. */
function sample(photo: Pixels, x: number, y: number, into: Float32Array, at: number): void {
  const { data, width: sw, height: sh } = photo;
  const x0 = Math.floor(x), y0 = Math.floor(y);
  const x1 = Math.min(x0 + 1, sw - 1), y1 = Math.min(y0 + 1, sh - 1);
  const tx = x - x0, ty = y - y0;
  for (let k = 0; k < 3; k++) {
    const top = data[(y0 * sw + x0) * 4 + k]! * (1 - tx) + data[(y0 * sw + x1) * 4 + k]! * tx;
    const bottom = data[(y1 * sw + x0) * 4 + k]! * (1 - tx) + data[(y1 * sw + x1) * 4 + k]! * tx;
    into[at + k] = top * (1 - ty) + bottom * ty;
  }
}

/**
 * One photograph unwrapped onto its side, or `inner`, onto its inside wall
 * seen through the opening (D241): the same rows and columns either way, so
 * the inside lines up with the outside round the rim.
 */
export function unwrap(view: View, size: RoundSize, width: number, height: number, inner = false): Layer {
  const rings = profile(size);
  const total = slant(rings);
  const rgb = new Float32Array(width * height * 3);
  const weight = new Float32Array(width * height);
  const { width: sw, height: sh } = view.photo;
  const margin = 0.02 * Math.max(sw, sh);
  const sines = new Float64Array(width), cosines = new Float64Array(width);
  for (let i = 0; i < width; i++) {
    const theta = (2 * Math.PI * (i + 0.5)) / width - Math.PI - view.turn;
    sines[i] = Math.sin(theta);
    cosines[i] = Math.cos(theta);
  }
  for (let j = 0; j < height; j++) {
    const out = along(rings, total * (1 - (j + 0.5) / height));
    // Inside, the wall faces the other way.
    const at = inner ? { ...out, nr: -out.nr, ny: -out.ny } : out;
    for (let i = 0; i < width; i++) {
      const theta = Math.atan2(sines[i]!, cosines[i]!);
      const face = facing(view.pose, theta, at);
      if (face <= 0.05) continue;
      if (inner && !throughTheTop(view.pose, size, at.r * Math.sin(theta), at.y, at.r * Math.cos(theta))) continue;
      const pt = project(view.pose, view.lens, theta, at.r, at.y);
      if (!pt) continue;
      const [x, y] = pt;
      if (x < 0 || y < 0 || x > sw - 1 || y > sh - 1) continue;
      // Seen at a slant near its outline, a pose a little out reads the wall
      // behind it: outside the outline is trusted only where nothing else saw.
      const m = view.within;
      const outside = m && !m.data[Math.min(Math.floor((y / sh) * m.height), m.height - 1) * m.width + Math.min(Math.floor((x / sw) * m.width), m.width - 1)];
      // The inside is seen in one photograph: past its outline is the table, never it.
      if (outside && inner) continue;
      // Less and less trusted toward the photograph's edge.
      const edge = Math.min(1, Math.min(x, y, sw - 1 - x, sh - 1 - y) / margin);
      const o = j * width + i;
      sample(view.photo, x, y, rgb, o * 3);
      weight[o] = Math.pow(face, SQUARELY) * edge * (outside ? 0.001 : 1);
    }
  }
  return { width, height, rgb, weight };
}

/** Brightness of a layer's point. */
const luma = (l: Layer, o: number) => 0.299 * l.rgb[o * 3]! + 0.587 * l.rgb[o * 3 + 1]! + 0.114 * l.rgb[o * 3 + 2]!;

/**
 * A layer's detail: its brightness less the brightness round it, so the
 * light falling differently in two photographs does not count as a
 * difference, and only marks on the side do.
 */
function detail(l: Layer, radius = 6): Float32Array {
  const { width, height } = l;
  const grey = new Float32Array(width * height);
  for (let o = 0; o < width * height; o++) grey[o] = luma(l, o);
  const out = new Float32Array(width * height);
  for (let j = 0; j < height; j++) {
    for (let i = 0; i < width; i++) {
      if (l.weight[j * width + i]! <= 0) continue;
      // Round it, only where the photograph saw: what it did not is not dark.
      let sum = 0, n = 0;
      for (let dj = -radius; dj <= radius; dj += 2) {
        const jj = j + dj;
        if (jj < 0 || jj >= height) continue;
        for (let di = -radius; di <= radius; di += 2) {
          const o = jj * width + ((i + di + width) % width);
          if (l.weight[o]! <= 0) continue;
          sum += grey[o]!;
          n++;
        }
      }
      out[j * width + i] = grey[j * width + i]! - sum / n;
    }
  }
  return out;
}

/**
 * How well `b` moved round by each shift from `-reach` to `reach` columns
 * matches `a` where both saw the same part of the side: the normalised
 * cross-correlation of their detail, over the rows from `rows[0]` to
 * `rows[1]`. −1 where they share too little to say.
 */
export function matches(a: Layer, b: Layer, reach: number, rows: [number, number] = [0, a.height]): Float64Array {
  const da = detail(a), db = detail(b);
  const out = new Float64Array(2 * reach + 1).fill(-1);
  for (let s = -reach; s <= reach; s++) {
    let n = 0, sa = 0, sb = 0, saa = 0, sbb = 0, sab = 0;
    for (let j = rows[0]; j < rows[1]; j++) {
      for (let i = 0; i < a.width; i++) {
        const oa = j * a.width + i;
        const ob = j * b.width + ((i - s + b.width) % b.width);
        const w = Math.min(a.weight[oa]!, b.weight[ob]!);
        if (w < 0.05) continue;
        const va = da[oa]!, vb = db[ob]!;
        n += w;
        sa += w * va;
        sb += w * vb;
        saa += w * va * va;
        sbb += w * vb * vb;
        sab += w * va * vb;
      }
    }
    if (n < 50) continue;
    const cov = sab / n - (sa / n) * (sb / n);
    const varA = saa / n - (sa / n) ** 2, varB = sbb / n - (sb / n) ** 2;
    out[s + reach] = cov / Math.sqrt(Math.max(varA * varB, 1e-6));
  }
  return out;
}

/**
 * How far round to move each layer after the first so all of them line up:
 * the shifts between neighbours that match best together. Taken all the way
 * round (`ring`), they must **come back to where they started**, since turning
 * a thing four times round brings it back, so a pair that matches as well at
 * the wrong place (the ribs under a rim repeat; a handle moves between
 * photographs) is outvoted. Not all the way round, each pair says alone.
 * In columns, the first layer's 0.
 */
export function lineUp(layers: Layer[], reach: number, rows?: [number, number], ring = true): { at: number[]; match: number } {
  const n = layers.length;
  if (!ring) {
    const out = [0];
    let total = 0;
    for (let k = 0; k < n - 1; k++) {
      const c = matches(layers[k]!, layers[k + 1]!, reach, rows);
      const best = c.reduce((b, v, i) => (v > c[b]! ? i : b), reach);
      out.push(out[k]! + best - reach);
      total += c[best]!;
    }
    return { at: out, match: n > 1 ? total / (n - 1) : 0 };
  }
  // Each pair's say is how far a shift stands out from its usual match: a
  // plain white back matches every shift alike, and so says nothing.
  const curves = layers.map((l, k) => {
    const c = matches(l, layers[(k + 1) % n]!, reach, rows);
    const usual = [...c].sort((a, b) => a - b)[Math.floor(c.length / 2)]!;
    return c.map((v) => v - usual);
  });
  const at = (k: number, s: number) => (Math.abs(s) <= reach ? curves[k]![s + reach]! : -Infinity);
  let best = { shifts: new Array<number>(n).fill(0), match: -Infinity };
  // Every way round for all but the last, which closes the ring.
  const tryFrom = (k: number, shifts: number[], sum: number, total: number) => {
    if (k === n - 1) {
      const last = -sum;
      const m = total + at(k, last);
      if (Math.abs(last) <= reach && m > best.match) best = { shifts: [...shifts, last], match: m };
      return;
    }
    for (let s = -reach; s <= reach; s++) {
      shifts.push(s);
      tryFrom(k + 1, shifts, sum + s, total + at(k, s));
      shifts.pop();
    }
  };
  tryFrom(0, [], 0, 0);
  const out = [0];
  for (let k = 0; k < n - 1; k++) out.push(out[k]! + best.shifts[k]!);
  return { at: out, match: best.match / n };
}

/** A layer moved round by so many columns. */
export function shifted(l: Layer, by: number): Layer {
  const rgb = new Float32Array(l.rgb.length), weight = new Float32Array(l.weight.length);
  for (let j = 0; j < l.height; j++) {
    for (let i = 0; i < l.width; i++) {
      const from = j * l.width + ((i - by + l.width * 8) % l.width), to = j * l.width + i;
      weight[to] = l.weight[from]!;
      rgb.set(l.rgb.subarray(from * 3, from * 3 + 3), to * 3);
    }
  }
  return { ...l, rgb, weight };
}

/**
 * Each layer's brightness against its neighbours' where they overlap, as a
 * gain to even them out: one photograph a little darker than the next would
 * otherwise show as a band. Their product is 1, so the whole stays as bright.
 */
export function gains(layers: Layer[]): number[] {
  const n = layers.length;
  const logs = new Array<number>(n).fill(0);
  if (n < 2) return [1];
  // A round of neighbours; each pair's ratio of brightness where both saw.
  const ratios: number[] = [];
  for (let k = 0; k < n; k++) {
    const a = layers[k]!, b = layers[(k + 1) % n]!;
    let wa = 0, wb = 0, w = 0;
    for (let o = 0; o < a.weight.length; o++) {
      const both = Math.min(a.weight[o]!, b.weight[o]!);
      if (both < 0.05) continue;
      wa += both * luma(a, o);
      wb += both * luma(b, o);
      w += both;
    }
    ratios.push(w > 0 && wa > 0 && wb > 0 ? Math.log(wb / wa) : 0);
  }
  // Each log-gain is its neighbours' differences, accumulated, then centred.
  for (let k = 1; k < n; k++) logs[k] = logs[k - 1]! - ratios[k - 1]!;
  const mean = logs.reduce((s, v) => s + v, 0) / n;
  return logs.map((v) => Math.exp(v - mean));
}

/**
 * What no photograph saw, filled from round it: averaged at half the size,
 * and half again, until every gap has something, then brought back up.
 * `seen` marks the pixels that keep their own colour.
 */
export function fillGaps(px: Pixels, seen: Uint8Array): void {
  const { width, height, data } = px;
  if (seen.every((v) => v) || !seen.some((v) => v)) return;
  // Half the size, each pixel the mean of the seen ones it stands for.
  const half = Math.max(1, Math.ceil(width / 2)), tall = Math.max(1, Math.ceil(height / 2));
  const small = new Uint8ClampedArray(half * tall * 4);
  const smallSeen = new Uint8Array(half * tall);
  for (let j = 0; j < tall; j++) {
    for (let i = 0; i < half; i++) {
      let n = 0, r = 0, g = 0, b = 0;
      for (const [di, dj] of [[0, 0], [1, 0], [0, 1], [1, 1]] as const) {
        const x = 2 * i + di, y = 2 * j + dj;
        if (x >= width || y >= height || !seen[y * width + x]) continue;
        const o = (y * width + x) * 4;
        r += data[o]!;
        g += data[o + 1]!;
        b += data[o + 2]!;
        n++;
      }
      if (!n) continue;
      const o = (j * half + i) * 4;
      small[o] = r / n;
      small[o + 1] = g / n;
      small[o + 2] = b / n;
      small[o + 3] = 255;
      smallSeen[j * half + i] = 1;
    }
  }
  if (half < width || tall < height) fillGaps({ data: small, width: half, height: tall }, smallSeen);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (seen[y * width + x]) continue;
      const from = (Math.min(y >> 1, tall - 1) * half + Math.min(x >> 1, half - 1)) * 4, to = (y * width + x) * 4;
      data[to] = small[from]!;
      data[to + 1] = small[from + 1]!;
      data[to + 2] = small[from + 2]!;
      data[to + 3] = 255;
    }
  }
}

/** The layers laid over each other, each by its trust and gain, with what none saw filled: the side's picture, as RGBA. */
export function blend(layers: Layer[], gain: number[] = layers.map(() => 1)): Pixels {
  const { width, height } = layers[0]!;
  const data = new Uint8ClampedArray(width * height * 4);
  const seen = new Uint8Array(width * height);
  for (let o = 0; o < width * height; o++) {
    let w = 0, r = 0, g = 0, b = 0;
    layers.forEach((l, k) => {
      const lw = l.weight[o]!;
      if (lw <= 0) return;
      w += lw;
      r += lw * gain[k]! * l.rgb[o * 3]!;
      g += lw * gain[k]! * l.rgb[o * 3 + 1]!;
      b += lw * gain[k]! * l.rgb[o * 3 + 2]!;
    });
    data[o * 4 + 3] = 255;
    if (w > 0) {
      data[o * 4] = r / w;
      data[o * 4 + 1] = g / w;
      data[o * 4 + 2] = b / w;
      seen[o] = 1;
    }
  }
  const px = { data, width, height };
  fillGaps(px, seen);
  return px;
}

/** The ellipse an outline is, from its spread: middle, half-widths and turn. */
export function ellipseOf(mask: Mask): { cx: number; cy: number; a: number; b: number; angle: number } | null {
  let n = 0, sx = 0, sy = 0;
  for (let y = 0; y < mask.height; y++) {
    for (let x = 0; x < mask.width; x++) {
      if (!mask.data[y * mask.width + x]) continue;
      n++;
      sx += x + 0.5;
      sy += y + 0.5;
    }
  }
  if (n < 10) return null;
  const cx = sx / n, cy = sy / n;
  let xx = 0, yy = 0, xy = 0;
  for (let y = 0; y < mask.height; y++) {
    for (let x = 0; x < mask.width; x++) {
      if (!mask.data[y * mask.width + x]) continue;
      const dx = x + 0.5 - cx, dy = y + 0.5 - cy;
      xx += dx * dx;
      yy += dy * dy;
      xy += dx * dy;
    }
  }
  xx /= n;
  yy /= n;
  xy /= n;
  const mid = (xx + yy) / 2, spread = Math.sqrt(((xx - yy) / 2) ** 2 + xy * xy);
  // A filled ellipse's spread along an axis is a quarter of that half-width squared.
  return { cx, cy, a: 2 * Math.sqrt(mid + spread), b: 2 * Math.sqrt(Math.max(mid - spread, 0)), angle: 0.5 * Math.atan2(2 * xy, xx - yy) };
}

/**
 * An open thing's floor as its top photograph shows it through the opening
 * (D241): a disc `side` pixels across, the front at its bottom edge as a lid
 * is, what the camera could not see filled from round it.
 */
export function floorOf(view: View, size: RoundSize, side: number): Pixels {
  const r = size.base / 2;
  const data = new Uint8ClampedArray(side * side * 4);
  const seen = new Uint8Array(side * side);
  const rgb = new Float32Array(3);
  const { width: sw, height: sh } = view.photo;
  for (let j = 0; j < side; j++) {
    for (let i = 0; i < side; i++) {
      const u = (2 * (i + 0.5)) / side - 1, v = (2 * (j + 0.5)) / side - 1;
      if (u * u + v * v > 1) continue;
      const o = j * side + i;
      data[o * 4 + 3] = 255;
      // Turned as the photograph was: the camera stood toward its front.
      const x = u * r, z = v * r;
      const c = Math.cos(view.turn), s = Math.sin(view.turn);
      const px = c * x - s * z, pz = s * x + c * z;
      if (!throughTheTop(view.pose, size, px, 0, pz)) continue;
      const theta = Math.atan2(px, pz), rr = Math.hypot(px, pz);
      const at = project(view.pose, view.lens, theta, rr, 0);
      if (!at || at[0] < 0 || at[1] < 0 || at[0] > sw - 1 || at[1] > sh - 1) continue;
      sample(view.photo, at[0], at[1], rgb, 0);
      data[o * 4] = rgb[0]!;
      data[o * 4 + 1] = rgb[1]!;
      data[o * 4 + 2] = rgb[2]!;
      seen[o] = 1;
    }
  }
  const px = { data, width: side, height: side };
  fillGaps(px, seen);
  // Outside the disc stays clear.
  for (let j = 0; j < side; j++) {
    for (let i = 0; i < side; i++) {
      const u = (2 * (i + 0.5)) / side - 1, v = (2 * (j + 0.5)) / side - 1;
      if (u * u + v * v > 1) data[(j * side + i) * 4 + 3] = 0;
    }
  }
  return px;
}

/**
 * Of the face-finder's outlines of a lid or a base, the one that is a disc:
 * the most of it, filling its ellipse most nearly, as a lid does and the
 * table under it does not.
 */
export function discOf(masks: Mask[]): NonNullable<ReturnType<typeof ellipseOf>> | null {
  let best: { e: NonNullable<ReturnType<typeof ellipseOf>>; score: number } | null = null;
  for (const m of masks) {
    const e = ellipseOf(m);
    if (!e || e.b <= 0) continue;
    let area = 0;
    for (const v of m.data) area += v;
    const fill = area / (Math.PI * e.a * e.b);
    const score = (1 - Math.min(Math.abs(1 - fill), 1)) * Math.sqrt(area);
    if (!best || score > best.score) best = { e, score };
  }
  return best?.e ?? null;
}

/**
 * A lid or a base, straightened from a photograph taken square on to it: the
 * ellipse its outline makes stretched back into a circle along its own axes,
 * never turned, so the photograph's bottom edge stays the disc's (the side
 * nearest whoever took it); `side` pixels across, transparent outside it.
 * `scale` takes the outline's pixels to the photograph's.
 */
export function disc(photo: Pixels, e: NonNullable<ReturnType<typeof ellipseOf>>, scale: number, side: number): Pixels {
  const data = new Uint8ClampedArray(side * side * 4);
  const ca = Math.cos(e.angle), sa = Math.sin(e.angle);
  const { data: src, width: sw, height: sh } = photo;
  for (let j = 0; j < side; j++) {
    for (let i = 0; i < side; i++) {
      const u = (2 * (i + 0.5)) / side - 1, v = (2 * (j + 0.5)) / side - 1;
      if (u * u + v * v > 1) continue;
      // Turned onto the ellipse's axes, stretched, and turned back.
      const p = ca * u + sa * v, q = -sa * u + ca * v;
      const x = (e.cx + ca * e.a * p - sa * e.b * q) * scale, y = (e.cy + sa * e.a * p + ca * e.b * q) * scale;
      const fx = Math.min(Math.max(x - 0.5, 0), sw - 1), fy = Math.min(Math.max(y - 0.5, 0), sh - 1);
      const x0 = Math.floor(fx), y0 = Math.floor(fy), x1 = Math.min(x0 + 1, sw - 1), y1 = Math.min(y0 + 1, sh - 1);
      const tx = fx - x0, ty = fy - y0;
      const o = (j * side + i) * 4;
      for (let k = 0; k < 3; k++) {
        const top = src[(y0 * sw + x0) * 4 + k]! * (1 - tx) + src[(y0 * sw + x1) * 4 + k]! * tx;
        const bottom = src[(y1 * sw + x0) * 4 + k]! * (1 - tx) + src[(y1 * sw + x1) * 4 + k]! * tx;
        data[o + k] = top * (1 - ty) + bottom * ty;
      }
      data[o + 3] = 255;
    }
  }
  return { data, width: side, height: side };
}

/** A photograph's edges: how sharply its brightness changes each way, at each pixel of it at some size. */
export interface Edges {
  width: number;
  height: number;
  gx: Float32Array;
  gy: Float32Array;
}

/** Its edges: brightness, softened over a few pixels, then Sobel's slopes across and down. */
export function edgesOf(px: Pixels, soften = 2): Edges {
  const { width, height, data } = px;
  const grey = new Float32Array(width * height);
  for (let o = 0; o < width * height; o++) grey[o] = 0.299 * data[o * 4]! + 0.587 * data[o * 4 + 1]! + 0.114 * data[o * 4 + 2]!;
  // A box blur, across then down.
  const pass = (src: Float32Array, dx: number, dy: number) => {
    const out = new Float32Array(src.length);
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        let s = 0, n = 0;
        for (let k = -soften; k <= soften; k++) {
          const xx = x + k * dx, yy = y + k * dy;
          if (xx < 0 || yy < 0 || xx >= width || yy >= height) continue;
          s += src[yy * width + xx]!;
          n++;
        }
        out[y * width + x] = s / n;
      }
    }
    return out;
  };
  const soft = pass(pass(grey, 1, 0), 0, 1);
  const gx = new Float32Array(width * height), gy = new Float32Array(width * height);
  for (let y = 1; y < height - 1; y++) {
    for (let x = 1; x < width - 1; x++) {
      const at = (i: number, j: number) => soft[(y + j) * width + x + i]!;
      gx[y * width + x] = at(1, -1) + 2 * at(1, 0) + at(1, 1) - at(-1, -1) - 2 * at(-1, 0) - at(-1, 1);
      gy[y * width + x] = at(-1, 1) + 2 * at(0, 1) + at(1, 1) - at(-1, -1) - 2 * at(0, -1) - at(1, -1);
    }
  }
  return { width, height, gx, gy };
}

/**
 * The lines of the shape a photograph shows as edges, as points with the way
 * across each line: its outline, and the front of each ring that is a crease
 * in it (the base, where the band meets the body, the rim). The rim's back
 * shows too when the camera is above it.
 */
export function creases(p: Pose, lens: Lens, size: RoundSize, steps = 180, open = false): { x: number; y: number; nx: number; ny: number }[] {
  const rings = profile(size);
  const out: { x: number; y: number; nx: number; ny: number }[] = [];
  // Looking into an open thing, where its floor meets its wall is a crease too (D241).
  if (open) {
    const r = size.base / 2;
    for (let i = 0; i < steps; i++) {
      const theta = (2 * Math.PI * i) / steps;
      if (!throughTheTop(p, size, r * Math.sin(theta) * 0.98, 1, r * Math.cos(theta) * 0.98)) continue;
      const a = project(p, lens, theta - 0.01, r, 0), b = project(p, lens, theta + 0.01, r, 0), c = project(p, lens, theta, r, 0);
      if (!a || !b || !c) continue;
      const tx = b[0] - a[0], ty = b[1] - a[1], len = Math.hypot(tx, ty);
      if (len > 0) out.push({ x: c[0], y: c[1], nx: -ty / len, ny: tx / len });
    }
  }
  const total = slant(rings);
  let up = 0;
  rings.forEach((ring, k) => {
    if (k > 0) up += Math.hypot(ring.r - rings[k - 1]!.r, ring.y - rings[k - 1]!.y);
    // How the side faces the camera just below the ring, or just above the base.
    const near = along(rings, k === 0 ? Math.min(1, total) : up - 1);
    const rimSeen = k === rings.length - 1 && p.h > ring.y;
    for (let i = 0; i < steps; i++) {
      const theta = (2 * Math.PI * i) / steps;
      if (!rimSeen && facing(p, theta, near) <= 0.1) continue;
      const a = project(p, lens, theta - 0.01, ring.r, ring.y), b = project(p, lens, theta + 0.01, ring.r, ring.y);
      const c = project(p, lens, theta, ring.r, ring.y);
      if (!a || !b || !c) continue;
      const tx = b[0] - a[0], ty = b[1] - a[1], len = Math.hypot(tx, ty);
      if (len === 0) continue;
      out.push({ x: c[0], y: c[1], nx: -ty / len, ny: tx / len });
    }
  });
  // Its outline's sides: the hull's longer edges, a point every few pixels.
  const hull = outlineOf(p, lens, rings, 96);
  for (let i = 0; i < hull.length; i++) {
    const a = hull[i]!, b = hull[(i + 1) % hull.length]!;
    const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
    if (len < 20) continue;
    for (let t = 0.1; t < 0.95; t += 8 / len) out.push({ x: a[0] + (b[0] - a[0]) * t, y: a[1] + (b[1] - a[1]) * t, nx: -(b[1] - a[1]) / len, ny: (b[0] - a[0]) / len });
  }
  return out;
}

/** How strongly the photograph's edges lie along those lines: the mean edge across them, each point's capped. */
function edgeScore(e: Edges, points: { x: number; y: number; nx: number; ny: number }[]): number {
  if (points.length === 0) return 0;
  let s = 0;
  for (const q of points) {
    const x = Math.round(q.x), y = Math.round(q.y);
    if (x < 1 || y < 1 || x >= e.width - 1 || y >= e.height - 1) continue;
    const o = y * e.width + x;
    s += Math.min(Math.abs(e.gx[o]! * q.nx + e.gy[o]! * q.ny), 400);
  }
  return s / points.length;
}

/**
 * The pose put right against the photograph's own edges: the outline the
 * face-finder sees is soft, and how the rings curve, which is how high the
 * camera was, is in its sharp edges. `lens` is at the edges' size.
 */
export function refinePose(e: Edges, lens: Lens, size: RoundSize, from: Pose, open = false): { pose: Pose; edge: number } {
  const cost = (x: number[]) => (impossible(poseOf(x), size) ? 0 : -edgeScore(e, creases(poseOf(x), lens, size, 120, open)));
  const coarse = minimise(cost, numbersOf(from), [from.d * 0.03, size.height * 0.15, 0.01, 0.02, 0.01], 250);
  const fine = minimise(cost, coarse.x, [from.d * 0.005, size.height * 0.02, 0.002, 0.004, 0.002], 250);
  return { pose: poseOf(fine.x), edge: -fine.value };
}

/** How a round thing is drawn among others, in the packing plan (D240): its shape, and its wrapping. */
export interface RoundLook {
  size: RoundSize;
  wrap: Wrap | null;
}

/** A level of an item as the bench has it, round, measured round and so drawn as a tub; null for a box. */
export function roundLook(unit: {
  round: boolean;
  diameter_mm: number | null;
  base_diameter_mm: number | null;
  top_height_mm: number | null;
  size: { height_mm: number } | null;
  wrap: Wrap | null;
}): RoundLook | null {
  if (!unit.round) return null;
  const size = roundSize({ ...unit, height_mm: unit.size?.height_mm ?? null });
  return size && { size, wrap: unit.wrap };
}
