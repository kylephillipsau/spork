import type { Act } from "@domain/acts";
import { partOf } from "@domain/acts";
import { api } from "@domain/api";
import type { CaptureSubject, ItemView, Uuid } from "@domain/types";
import { encodeWebp } from "@domain/webp";

import type { Pixels, Point } from "./cut";
import { draw, loadPhoto, pixelsOf } from "./crop";
import { SAM_SIZE } from "./faceFind";
import {
  blend,
  disc,
  discOf,
  edgesOf,
  fitPose,
  floorOf,
  gains,
  lineUp,
  phoneLens,
  profile,
  refinePose,
  roundSize,
  scaled,
  slant,
  unwrap,
  type Mask,
  type RoundSize,
  type View,
} from "./round";
import { photosOf } from "./subjects";

/**
 * A round thing wrapped in its photographs, at a computer (D240): the parts
 * that need a browser. Each photograph of its side is loaded, the
 * face-finder outlines the thing in it, the measured shape is fitted to the
 * outline and then to the photograph's own edges, and the side is read off
 * each and laid together ([`round`]). Its lid and base, photographed square
 * on, are straightened into discs. Nothing is kept until the person saves.
 */

const model = () => import("./faceModel");

/** Its side, walked round from the front a quarter turn at a time. */
const SIDES = [
  ["front", 0],
  ["right", 1],
  ["back", 2],
  ["left", 3],
] as const;

/**
 * Where the face-finder is asked to look: the middle; higher, where a lid
 * is; and top, middle and bottom at once, for the whole thing with its lid,
 * which it can take for a part of its own.
 */
const PROMPTS: Point[][] = [[[0.5, 0.55]], [[0.5, 0.3]], [[0.5, 0.25], [0.5, 0.5], [0.5, 0.75]]];

/** Round its side, in pixels: as sharp as the photographs it comes from, and no wider than a graphics card takes. */
const AROUND_PX = 4096;
/** Lined up at this size first: a fraction of the time, and the same answer. */
const SMALL_PX = 720;
/** How far a photograph may be from a quarter turn from the last, either way: 45°. */
const REACH = SMALL_PX / 8;
/** Edges are read at this size: sharp enough to place a rim to a pixel or two. */
const EDGES_PX = 1536;
const DISC_PX = 1024;

/** A wrapping made and not yet kept. */
export interface Made {
  size: RoundSize;
  side: Pixels;
  lid: Pixels | null;
  base: Pixels | null;
  /** Open (D241): its inside wall and its floor, read off its top photograph. */
  inside: Pixels | null;
  floor: Pixels | null;
  made_from: Uuid[];
  /** How well the shape fitted each photograph of its side, 0 to 1: under 0.9, look at it. */
  fits: { face: string; fit: number }[];
}

/** The outlines the face-finder sees in a photograph, at its size. */
async function outlines(key: string, image: HTMLImageElement): Promise<Mask[]> {
  const px = pixelsOf(image, SAM_SIZE, true);
  const { width, height } = px;
  const found = await (await model()).findWholes(key, px, PROMPTS);
  return found.map((data) => ({ data, width, height }));
}

/** A photograph's camera, fitted to its outline and then to its edges. */
async function viewOf(
  image: HTMLImageElement,
  key: string,
  size: RoundSize,
  seen: "side" | "above",
): Promise<(View & { fit: number }) | null> {
  const masks = await outlines(key, image);
  const lens = phoneLens(image.naturalWidth, image.naturalHeight);
  const at = masks[0] ? masks[0].width / image.naturalWidth : 1;
  const fitted = fitPose(masks, scaled(lens, at), size, seen);
  if (!fitted) return null;
  const sharp = pixelsOf(image, EDGES_PX, true);
  const { pose } = refinePose(edgesOf(sharp), scaled(lens, sharp.width / image.naturalWidth), size, fitted.pose, seen === "above");
  const full = pixelsOf(image, Math.max(image.naturalWidth, image.naturalHeight));
  return { fit: fitted.fit, photo: full, lens: scaled(lens, full.width / image.naturalWidth), pose, turn: 0, within: masks[fitted.mask] };
}

/**
 * Make a round thing's wrapping from its photographs: two sides or more,
 * and its lid and base where taken; or `open` (D241), its top photograph
 * read onto its inside wall and floor instead of a lid. `said` hears each
 * step, to show.
 */
export async function makeWrap(
  item: Pick<ItemView, "photos">,
  subject: CaptureSubject,
  said: (step: string) => void,
  open = false,
): Promise<Made> {
  const size = roundSize(subject);
  if (!size) throw new Error("Measure it across the top and its height first.");
  const photos = photosOf(item, subject);
  const views: (View & { face: string; fit: number })[] = [];
  for (const [face, quarter] of SIDES) {
    const photo = photos.get(face);
    if (!photo) continue;
    said(`Finding it in the ${face} photo…`);
    const view = await viewOf(await loadPhoto(photo.digest), photo.image_id, size, "side");
    if (view) views.push({ ...view, face, turn: (quarter * Math.PI) / 2 });
  }
  if (views.length < 2) throw new Error("Photograph at least two sides of it, a quarter turn apart, to wrap it.");

  said("Lining up the photos…");
  const high = Math.round((AROUND_PX * slant(profile(size))) / ((Math.PI * (size.top + size.base)) / 2));
  const low = Math.round((SMALL_PX * high) / AROUND_PX);
  // The band and the ribs under a rim repeat round it; line up on the body below.
  const rows: [number, number] = [Math.round(low * 0.35), Math.round(low * 0.97)];
  const ring = views.length === SIDES.length;
  let best: { turns: number[]; match: number } | null = null;
  // Turned left or right between photographs: whichever lines up.
  for (const way of [1, -1]) {
    const turns = views.map((v) => way * v.turn);
    const small = views.map((v, k) => unwrap({ ...v, turn: turns[k]! }, size, SMALL_PX, low));
    const lined = lineUp(small, REACH, rows, ring);
    if (!best || lined.match > best.match) best = { turns: turns.map((t, k) => t + ((2 * Math.PI) / SMALL_PX) * lined.at[k]!), match: lined.match };
  }

  said("Wrapping the photos round it…");
  const layers = views.map((v, k) => unwrap({ ...v, turn: best!.turns[k]! }, size, AROUND_PX, high));
  const side = blend(layers, gains(layers));

  const flat = async (face: "top" | "bottom"): Promise<Pixels | null> => {
    const photo = photos.get(face);
    if (!photo) return null;
    said(face === "top" ? "Straightening the lid…" : "Straightening the base…");
    const image = await loadPhoto(photo.digest);
    const masks = await outlines(photo.image_id, image);
    const e = discOf(masks);
    if (!e || !masks[0]) return null;
    const full = pixelsOf(image, Math.max(image.naturalWidth, image.naturalHeight));
    return disc(full, e, full.width / masks[0].width, DISC_PX);
  };
  // Open, its top photograph looks into it: read onto its inside and floor
  // from where the camera stood, its front toward the photograph's bottom.
  let inside: Pixels | null = null, floor: Pixels | null = null;
  const top = photos.get("top");
  if (open && top) {
    said("Looking inside it…");
    const view = await viewOf(await loadPhoto(top.digest), top.image_id, size, "above");
    if (view) {
      inside = blend([unwrap(view, size, AROUND_PX, high, true)]);
      floor = floorOf(view, size, DISC_PX);
    }
  }
  const lid = open ? null : await flat("top");
  const base = await flat("bottom");
  const used = [...views.map((v) => photos.get(v.face)!.image_id), ...(["top", "bottom"] as const).flatMap((f) => (photos.get(f) ? [photos.get(f)!.image_id] : []))];
  return { size, side, lid, base, inside, floor, made_from: used, fits: views.map((v) => ({ face: v.face, fit: v.fit })) };
}

/** A made picture as WebP, kept by its content address. */
async function keep(px: Pixels): Promise<string> {
  const canvas = draw(new Uint8ClampedArray(px.data), px.width, px.height);
  try {
    return (await api.storeImage(await encodeWebp(canvas))).digest;
  } finally {
    canvas.width = canvas.height = 0;
  }
}

/** Keep a wrapping made (D240): its pictures stored, then the wrapping said of the subject. */
export async function saveWrap(subject: CaptureSubject, made: Made, act: Act): Promise<void> {
  const side = await keep(made.side);
  const lid = made.lid ? await keep(made.lid) : null;
  const base = made.base ? await keep(made.base) : null;
  const inside = made.inside ? await keep(made.inside) : null;
  const floor = made.floor ? await keep(made.floor) : null;
  await api.recordWrap(subject, { side, lid, base, inside, floor, made_from: made.made_from }, partOf(act, "wrap"));
}
