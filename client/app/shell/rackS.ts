/**
 * The Rack S: an S lit on a rack face of bins, four across and five up, one
 * of them in hi-vis as the bin page lights the bin you are after. Drawn on a
 * 120-unit tile. The geometry and the colours are here once, for the mark in
 * the app (`Brand.tsx`) and for every icon made from it (`scripts/icons.mjs`).
 *
 * No React and no DOM, so a script can read it.
 */

/** The bins that light the S, by row and column; the first is the hi-vis one. */
const LIT: [number, number][] = [
  [0, 3],
  [0, 1],
  [0, 2],
  [1, 0],
  [2, 1],
  [2, 2],
  [3, 3],
  [4, 0],
  [4, 1],
  [4, 2],
];

export type BinState = "spot" | "lit" | "off";

/** Every bin of the face: its row and column, where it sits on the tile, and whether it is lit. */
export const BINS: { row: number; col: number; x: number; y: number; state: BinState }[] = Array.from({ length: 20 }, (_, i) => {
  const row = Math.floor(i / 4), col = i % 4;
  const lit = LIT.findIndex(([r, c]) => r === row && c === col);
  return { row, col, x: 24 + col * 19, y: 17 + row * 18, state: lit === 0 ? "spot" : lit > 0 ? "lit" : "off" };
});
export const BIN = { width: 15, height: 14, rx: 3 };

/** Black anodised, lit off-white, one bin in hi-vis. `ui/tokens.css` names the same for the app. */
export const COLOURS = { tile: "#101114", lit: "#eef0f3", hivis: "#dfff1a" };

/**
 * The tile's outline: its own rounded square; the whole image, for a platform
 * that rounds its own corners or masks it; a circle, for a round launcher
 * icon; or none, the face alone over a colour the platform lays under it.
 */
export type TileShape = "rounded" | "square" | "circle" | "none";

/**
 * The mark as a standalone SVG, for icons. `inset` shrinks the face within
 * the image (1 is the tile's own margins), for a mask that may crop it;
 * `texture` adds the anodised sheen, worth it from about 64 pixels, and draws
 * the lit edge finer. (The ripstop the app's nylon has is a texture at 30
 * pixels and a second grid fighting the bins at 512, so no icon has it.)
 */
export function rackSvg({ shape = "rounded", inset = 1, texture = false }: { shape?: TileShape; inset?: number; texture?: boolean } = {}): string {
  const c = COLOURS;
  const face = BINS.map(
    (b) =>
      `<rect x="${b.x}" y="${b.y}" width="${BIN.width}" height="${BIN.height}" rx="${BIN.rx}" fill="${b.state === "spot" ? c.hivis : c.lit}"${b.state === "off" ? ' fill-opacity="0.13"' : ""}/>`,
  ).join("");
  const outline = (fill: string) =>
    shape === "circle" ? `<circle cx="60" cy="60" r="60" fill="${fill}"/>` : `<rect width="120" height="120" rx="${shape === "rounded" ? 24 : 0}" fill="${fill}"/>`;
  const tile = shape === "none" ? "" : outline(c.tile);
  const sheen =
    texture && shape !== "none"
      ? `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#d6e4ff" stop-opacity="0.1"/><stop offset="1" stop-color="#000" stop-opacity="0.3"/></linearGradient></defs>${outline("url(#g)")}`
      : "";
  // Its edge lit, so a black tile still reads on a dark tab bar: a hairline when large.
  const [ew, eo] = texture ? [1, 0.2] : [4, 0.22];
  const edge =
    shape === "rounded"
      ? `<rect x="${ew / 2}" y="${ew / 2}" width="${120 - ew}" height="${120 - ew}" rx="${24 - ew / 2}" fill="none" stroke="#d6e4ff" stroke-opacity="${eo}" stroke-width="${ew}"/>`
      : "";
  const k = 1 / inset, shift = 60 - 60 * k;
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120">${tile}${sheen}${edge}` +
    `<g transform="translate(${shift} ${shift}) scale(${k})">${face}</g></svg>`
  );
}

/**
 * The mark drawn to whole pixels, for a favicon: at 16 and 32 the face
 * scaled down falls between pixels and blurs, so here each bin is a whole
 * number of them, 2 with a gap of 1 at 16, 4 with a gap of 2 at 32.
 */
export function rackPixelSvg(size: 16 | 32): string {
  const c = COLOURS;
  const [bin, gap, left, top, rx] = size === 16 ? [2, 1, 3, 1, 3] : [4, 2, 5, 2, 6];
  const face = BINS.map(
    (b) =>
      `<rect x="${left + b.col * (bin + gap)}" y="${top + b.row * (bin + gap)}" width="${bin}" height="${bin}" fill="${b.state === "spot" ? c.hivis : c.lit}"${b.state === "off" ? ' fill-opacity="0.16"' : ""}/>`,
  ).join("");
  const edge = size === 32 ? `<rect x="0.5" y="0.5" width="31" height="31" rx="${rx - 0.5}" fill="none" stroke="#d6e4ff" stroke-opacity="0.22"/>` : "";
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${size} ${size}" shape-rendering="crispEdges"><rect width="${size}" height="${size}" rx="${rx}" fill="${c.tile}" shape-rendering="auto"/>${edge}${face}</svg>`;
}
