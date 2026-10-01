/**
 * From a photograph to a model's question, and from its answer to a face's
 * four corners (D177). No DOM and no model here, so a test runner can read it.
 *
 * The model (SlimSAM, Segment Anything made small) is asked one thing: given
 * a point on the photograph, what is the thing there? It answers with three
 * outlines, from a part to the whole, each with how sure it is. A box's face
 * is the outline that is four-sided; the corners are where its outline turns.
 */

import { type Pixels, type Point, type Quad } from "./cut.ts";

/** The model looks at the photograph scaled so its longest side is this, padded square. */
export const SAM_SIZE = 1024;

/**
 * What the face-finder answers on a device where it was seen to take the page
 * down: a phone that reloaded the page while it was finding a face. It is not
 * asked again there until somebody says to try it anyway.
 */
export const UNFIT = "the face-finder needs more memory than this device gives a page";
/** Its outlines are a quarter of that across. */
const MASK = 256;
const MEAN = [0.485, 0.456, 0.406];
const STD = [0.229, 0.224, 0.225];

/**
 * The photograph as the model takes it: pixels already scaled to
 * [`SAM_SIZE`] on their longest side, each channel normalised the way it was
 * trained, laid out a channel at a time, and padded with zeros to a square.
 */
export function samInput(px: Pixels): Float32Array<ArrayBuffer> {
  const plane = SAM_SIZE * SAM_SIZE;
  const out = new Float32Array(3 * plane);
  for (let y = 0; y < px.height; y++) {
    for (let x = 0; x < px.width; x++) {
      const i = (y * px.width + x) * 4;
      for (let k = 0; k < 3; k++) out[k * plane + y * SAM_SIZE + x] = (px.data[i + k]! / 255 - MEAN[k]!) / STD[k]!;
    }
  }
  return out;
}

/**
 * One of the model's outlines at the photograph's scaled size: its scores
 * read between the cells they were given on, and kept where they say yes.
 */
export function outline(logits: Float32Array, which: number, w: number, h: number): Uint8Array {
  const out = new Uint8Array(w * h);
  const base = which * MASK * MASK;
  const scale = SAM_SIZE / MASK;
  for (let y = 0; y < h; y++) {
    const fy = Math.min(Math.max((y + 0.5) / scale - 0.5, 0), MASK - 1);
    const y0 = Math.floor(fy), y1 = Math.min(y0 + 1, MASK - 1), ty = fy - y0;
    for (let x = 0; x < w; x++) {
      const fx = Math.min(Math.max((x + 0.5) / scale - 0.5, 0), MASK - 1);
      const x0 = Math.floor(fx), x1 = Math.min(x0 + 1, MASK - 1), tx = fx - x0;
      const top = logits[base + y0 * MASK + x0]! * (1 - tx) + logits[base + y0 * MASK + x1]! * tx;
      const bottom = logits[base + y1 * MASK + x0]! * (1 - tx) + logits[base + y1 * MASK + x1]! * tx;
      out[y * w + x] = top * (1 - ty) + bottom * ty > 0 ? 1 : 0;
    }
  }
  return out;
}

/** The connected piece of an outline under a point, and its size; nothing when the point is outside it. */
export function pieceAt(mask: Uint8Array, w: number, h: number, [px, py]: Point): { piece: Uint8Array; area: number } {
  const piece = new Uint8Array(w * h);
  const x0 = Math.min(Math.max(Math.round(px), 0), w - 1), y0 = Math.min(Math.max(Math.round(py), 0), h - 1);
  const start = y0 * w + x0;
  if (!mask[start]) return { piece, area: 0 };
  const stack = [start];
  piece[start] = 1;
  let area = 0;
  while (stack.length) {
    const i = stack.pop()!;
    area++;
    const x = i % w;
    for (const j of [x > 0 ? i - 1 : -1, x < w - 1 ? i + 1 : -1, i - w, i + w]) {
      if (j >= 0 && j < w * h && mask[j] && !piece[j]) {
        piece[j] = 1;
        stack.push(j);
      }
    }
  }
  return { piece, area };
}

const cross = (o: Point, a: Point, b: Point) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);

/** The convex hull of a piece: its row ends, wrapped (Andrew's monotone chain). */
export function hull(piece: Uint8Array, w: number, h: number): Point[] {
  const pts: Point[] = [];
  for (let y = 0; y < h; y++) {
    let first = -1, last = -1;
    for (let x = 0; x < w; x++) {
      if (piece[y * w + x]) {
        if (first < 0) first = x;
        last = x;
      }
    }
    if (first >= 0) pts.push([first, y], [last + 1, y], [first, y + 1], [last + 1, y + 1]);
  }
  pts.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const lower: Point[] = [], upper: Point[] = [];
  for (const p of pts) {
    while (lower.length >= 2 && cross(lower.at(-2)!, lower.at(-1)!, p) <= 0) lower.pop();
    lower.push(p);
  }
  for (const p of pts.reverse()) {
    while (upper.length >= 2 && cross(upper.at(-2)!, upper.at(-1)!, p) <= 0) upper.pop();
    upper.push(p);
  }
  return lower.slice(0, -1).concat(upper.slice(0, -1));
}

const triangle = (a: Point, b: Point, c: Point) => Math.abs(cross(a, b, c)) / 2;

/**
 * The largest four-sided shape with its corners on the hull. A face seen at
 * an angle is still four-sided, so its corners are where this one's are,
 * however the photograph was held.
 */
export function biggestQuad(points: Point[]): Quad | null {
  // At most 80 candidates: a hull of a few hundred points gains nothing more.
  const step = Math.max(1, Math.ceil(points.length / 80));
  const p = points.filter((_, i) => i % step === 0);
  const n = p.length;
  if (n < 4) return null;
  let best: Quad | null = null;
  let most = -1;
  for (let i = 0; i < n; i++) {
    for (let k = i + 2; k < n; k++) {
      let j = -1, l = -1, a = -1, b = -1;
      for (let m = i + 1; m < k; m++) {
        const t = triangle(p[i]!, p[m]!, p[k]!);
        if (t > a) [a, j] = [t, m];
      }
      for (let m = k + 1; m < n + i; m++) {
        const t = triangle(p[k]!, p[m % n]!, p[i]!);
        if (t > b) [b, l] = [t, m % n];
      }
      if (j >= 0 && l >= 0 && a + b > most) {
        most = a + b;
        best = [p[i]!, p[j]!, p[k]!, p[l]!];
      }
    }
  }
  return best;
}

/** Corners named clockwise from the one nearest the photograph's top-left. */
export function fromTopLeft(q: Quad): Quad {
  const cx = (q[0][0] + q[1][0] + q[2][0] + q[3][0]) / 4;
  const cy = (q[0][1] + q[1][1] + q[2][1] + q[3][1]) / 4;
  const round = [...q].sort((a, b) => Math.atan2(a[1] - cy, a[0] - cx) - Math.atan2(b[1] - cy, b[0] - cx));
  const start = round.reduce((best, p, i) => (p[0] + p[1] < round[best]![0] + round[best]![1] ? i : best), 0);
  return [0, 1, 2, 3].map((i) => round[(start + i) % 4]!) as Quad;
}

/**
 * How well four corners describe a piece: the overlap of the piece and the
 * four-sided shape, over both together. A face is near 1; a face with the
 * shelf under it, or a label on it, is not.
 */
export function quadFit(piece: Uint8Array, area: number, q: Quad, w: number, h: number): number {
  const inside = (x: number, y: number) => {
    for (let i = 0; i < 4; i++) if (cross(q[i]!, q[(i + 1) % 4]!, [x, y]) < 0) return false;
    return true;
  };
  let both = 0, quad = 0;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (!inside(x + 0.5, y + 0.5)) continue;
      quad++;
      if (piece[y * w + x]) both++;
    }
  }
  return both / (area + quad - both);
}

/** An outline taken for a face, with how it scored. */
export interface Candidate {
  quad: Quad;
  /** Its share of the photograph. */
  share: number;
  /** How well four corners describe it: see [`quadFit`]. */
  fit: number;
  /** How sure the model was of it. */
  score: number;
}

/** How well four corners must describe an outline for it to be a face. */
const FACE_FIT = 0.9;

/**
 * The face, from the model's three outlines around a point: of the ones four
 * corners describe well, the largest, because the model's smaller outlines are
 * parts of a face (a lid, a label) and its larger one may run on past the face.
 * When none is four-sided, the best described. Corners as fractions of the
 * photograph, named from its top-left; nothing when the point is on nothing.
 */
export function faceFrom(logits: Float32Array, scores: Float32Array, w: number, h: number, at: Point): Quad | null {
  const candidates: Candidate[] = [];
  for (let m = 0; m < scores.length; m++) {
    const { piece, area } = pieceAt(outline(logits, m, w, h), w, h, at);
    const share = area / (w * h);
    if (share < 0.02 || share > 0.97 || scores[m]! < 0.5) continue;
    const quad = biggestQuad(hull(piece, w, h));
    if (!quad) continue;
    candidates.push({ quad, share, fit: quadFit(piece, area, quad, w, h), score: scores[m]! });
  }
  const pick = choose(candidates);
  return pick ? (fromTopLeft(pick.quad).map(([x, y]) => [x / w, y / h]) as Quad) : null;
}

export function choose(candidates: Candidate[]): Candidate | null {
  const faces = candidates.filter((c) => c.fit >= FACE_FIT);
  if (faces.length) return faces.reduce((a, b) => (b.share > a.share ? b : a));
  return candidates.reduce<Candidate | null>((a, b) => (!a || b.fit * b.score > a.fit * a.score ? b : a), null);
}
