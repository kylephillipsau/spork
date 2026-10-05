/**
 * A photograph cut to the face it is of (D176): the face's four corners,
 * which way up it is, and how its pixels are straightened into a rectangle.
 * No DOM here, so a test runner can read it.
 *
 * **The order of the corners is which way up the face is.** The first is the
 * face's top-left, wherever it falls in the photograph, so a top photographed
 * sideways is put right by naming its corners from another one ([`turn`])
 * rather than by a second setting.
 */

/** x, y as fractions of the photograph shown the right way up. */
export type Point = [number, number];
/** The face's top-left, top-right, bottom-right and bottom-left, in that order. */
export type Quad = [Point, Point, Point, Point];

/** The whole photograph, its corners as taken: a photograph used as it is. */
export const WHOLE = [0, 0, 1, 0, 1, 1, 0, 1];

/** Where the corners start before anybody moves them: a margin in from the edges. */
export const START: Quad = [
  [0.15, 0.15],
  [0.85, 0.15],
  [0.85, 0.85],
  [0.15, 0.85],
];

/** The eight numbers the server keeps. */
export const toCorners = (q: Quad): number[] => q.flat();

export function fromCorners(c: readonly number[]): Quad {
  return [
    [c[0]!, c[1]!],
    [c[2]!, c[3]!],
    [c[4]!, c[5]!],
    [c[6]!, c[7]!],
  ];
}

/** A quarter turn clockwise: what was the face's left edge is now its top. */
export function turn(q: Quad): Quad {
  return [q[3], q[0], q[1], q[2]];
}

/**
 * Whether the corners make a face: clockwise as the photograph is shown, not
 * folded or crossed, and round at least a hundredth of it. The server's rule,
 * so the button that would be refused is not offered.
 */
export function isFace(q: Quad): boolean {
  let twice = 0;
  for (let i = 0; i < 4; i++) {
    const [a, b, c] = [q[i]!, q[(i + 1) % 4]!, q[(i + 2) % 4]!];
    twice += a[0] * b[1] - b[0] * a[1];
    if ((b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]) <= 0) return false;
  }
  return twice / 2 >= 0.01;
}

const length = (a: Point, b: Point, w: number, h: number) => Math.hypot((b[0] - a[0]) * w, (b[1] - a[1]) * h);

/**
 * The face's width over its height as photographed: the mean of its top and
 * bottom edges over the mean of its sides, in the photograph's pixels. Used
 * when its size has not been measured.
 */
export function aspectOf(q: Quad, width: number, height: number): number {
  const across = (length(q[0], q[1], width, height) + length(q[3], q[2], width, height)) / 2;
  const down = (length(q[0], q[3], width, height) + length(q[1], q[2], width, height)) / 2;
  return down > 0 ? across / down : 1;
}

/**
 * How the corners lie against the face as it was measured (D214): the right
 * way round, a quarter turn out, or neither.
 *
 * A measured face is straightened to its measured proportions whatever its
 * corners say, so corners named from the wrong one squash it: a carton front
 * 52 cm wide and 24 tall, its corners marked from its side, comes out a
 * front half as wide and twice as tall. The corners' own proportions as
 * photographed tell the two apart. They can't tell a quarter turn one way
 * from the other, nor right way up from upside down: that is the thick edge,
 * and the person's to check.
 */
export type Lie = "matches" | "turned" | "neither";

/**
 * How far the corners' proportions may stray from the face's and still be
 * it. A face photographed a little off square comes out foreshortened.
 */
export const LIE_TOLERANCE = 1.45;
/** A face nearer square than this looks the same turned, so it is never called turned. */
export const NEAR_SQUARE = 1.2;

/** `shot`, the corners' width over height as photographed ([`aspectOf`]); `measured`, the face's. */
export function lieOf(shot: number, measured: number): Lie {
  const near = Math.log(LIE_TOLERANCE);
  const straight = Math.abs(Math.log(shot / measured));
  if (straight <= near) return "matches";
  const turned = Math.abs(Math.log(shot * measured));
  const square = Math.abs(Math.log(measured)) < Math.log(NEAR_SQUARE);
  return !square && turned <= near && turned < straight ? "turned" : "neither";
}

/**
 * The straightened size: the face's proportions, as large as the photograph
 * has pixels for along its longest edge, and at most `longest` on a side.
 */
export function cutSize(q: Quad, width: number, height: number, aspect: number, longest = 2048): [number, number] {
  const edges = [0, 1, 2, 3].map((i) => length(q[i]!, q[(i + 1) % 4]!, width, height));
  const side = Math.max(1, Math.min(longest, Math.round(Math.max(...edges))));
  return aspect >= 1
    ? [side, Math.max(1, Math.round(side / aspect))]
    : [Math.max(1, Math.round(side * aspect)), side];
}

/**
 * The map from the straightened face onto the photograph: `(u, v)`, across and
 * down the face from 0 to 1, to the point in the photograph's pixels. The
 * square-to-quadrilateral projection (Heckbert, 1989), which keeps straight
 * lines straight, as a camera does.
 */
export function projection(q: Quad, width: number, height: number): (u: number, v: number) => Point {
  const [x0, y0] = [q[0][0] * width, q[0][1] * height];
  const [x1, y1] = [q[1][0] * width, q[1][1] * height];
  const [x2, y2] = [q[2][0] * width, q[2][1] * height];
  const [x3, y3] = [q[3][0] * width, q[3][1] * height];
  const dx1 = x1 - x2, dx2 = x3 - x2, dx3 = x0 - x1 + x2 - x3;
  const dy1 = y1 - y2, dy2 = y3 - y2, dy3 = y0 - y1 + y2 - y3;
  let g = 0;
  let h = 0;
  if (dx3 !== 0 || dy3 !== 0) {
    const det = dx1 * dy2 - dx2 * dy1;
    g = (dx3 * dy2 - dx2 * dy3) / det;
    h = (dx1 * dy3 - dx3 * dy1) / det;
  }
  const a = x1 - x0 + g * x1, b = x3 - x0 + h * x3, c = x0;
  const d = y1 - y0 + g * y1, e = y3 - y0 + h * y3, f = y0;
  return (u, v) => {
    const z = g * u + h * v + 1;
    return [(a * u + b * v + c) / z, (d * u + e * v + f) / z];
  };
}

/** RGBA pixels, row by row, as a canvas holds them. */
export interface Pixels {
  data: Uint8ClampedArray;
  width: number;
  height: number;
}

/**
 * The face straightened: every pixel of a `w` by `h` rectangle, read from
 * where it falls in the photograph and blended from the four pixels round it.
 */
export function straighten(src: Pixels, q: Quad, w: number, h: number): Uint8ClampedArray<ArrayBuffer> {
  const out = new Uint8ClampedArray(w * h * 4);
  const at = projection(q, src.width, src.height);
  const { data, width: sw, height: sh } = src;
  for (let j = 0; j < h; j++) {
    for (let i = 0; i < w; i++) {
      // Pixel centres on both sides, so a picture straightened onto itself is itself.
      const [x, y] = at((i + 0.5) / w, (j + 0.5) / h);
      const fx = Math.min(Math.max(x - 0.5, 0), sw - 1);
      const fy = Math.min(Math.max(y - 0.5, 0), sh - 1);
      const x0 = Math.floor(fx), y0 = Math.floor(fy);
      const x1 = Math.min(x0 + 1, sw - 1), y1 = Math.min(y0 + 1, sh - 1);
      const tx = fx - x0, ty = fy - y0;
      const p00 = (y0 * sw + x0) * 4, p10 = (y0 * sw + x1) * 4;
      const p01 = (y1 * sw + x0) * 4, p11 = (y1 * sw + x1) * 4;
      const o = (j * w + i) * 4;
      for (let k = 0; k < 4; k++) {
        const top = data[p00 + k]! + (data[p10 + k]! - data[p00 + k]!) * tx;
        const bottom = data[p01 + k]! + (data[p11 + k]! - data[p01 + k]!) * tx;
        out[o + k] = top + (bottom - top) * ty;
      }
    }
  }
  return out;
}
