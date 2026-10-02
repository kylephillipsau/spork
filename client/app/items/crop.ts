import { imageUrl } from "@domain/api";
import { LONGEST_PX, encodeWebp, fit } from "@domain/webp";

import { aspectOf, cutSize, straighten, type Pixels, type Quad } from "./cut";

/**
 * A photograph cut to its face, the parts that need a browser: its pixels off
 * a canvas, and the straightened face as WebP (D176). One place, for the crop
 * screen and for the queue a computer works through (D181).
 */

/**
 * A phone: a finger for a pointer and nothing that hovers. A phone's page has
 * not the memory to find a face (D177, D181), so it never fetches the model:
 * its photographs are cut at a computer. A laptop with a touch screen has a
 * trackpad that hovers, and is a computer.
 */
export const handheld = () =>
  typeof window.matchMedia === "function" && window.matchMedia("(pointer: coarse) and (hover: none)").matches;

/** A photograph, loaded and decoded. */
export async function loadPhoto(digest: string): Promise<HTMLImageElement> {
  const image = new Image();
  image.src = imageUrl(digest);
  await image.decode();
  return image;
}

/**
 * The photograph's pixels, scaled so its longest side is at most `longest`,
 * or exactly that when `exactly`: the model takes its own size, up or down.
 */
export function pixelsOf(image: HTMLImageElement, longest: number, exactly = false): Pixels {
  const scale = longest / Math.max(image.naturalWidth, image.naturalHeight);
  const [width, height] = exactly
    ? [Math.round(image.naturalWidth * scale), Math.round(image.naturalHeight * scale)]
    : fit(image.naturalWidth, image.naturalHeight, longest);
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const g = canvas.getContext("2d", { willReadFrequently: true })!;
  g.drawImage(image, 0, 0, width, height);
  const { data } = g.getImageData(0, 0, width, height);
  // iOS holds a canvas's memory until its size is zero.
  canvas.width = canvas.height = 0;
  return { data, width, height };
}

/** Pixels drawn onto a canvas, a new one or the one given. */
export function draw(data: Uint8ClampedArray<ArrayBuffer>, width: number, height: number, into = document.createElement("canvas")) {
  into.width = width;
  into.height = height;
  into.getContext("2d")!.putImageData(new ImageData(data, width, height), 0, 0);
  return into;
}

/** Its width over its height: the face's measured proportions when known, otherwise its corners'. */
export function ratioOf(image: HTMLImageElement, quad: Quad, aspect: number | null): number {
  return aspect ?? aspectOf(quad, image.naturalWidth, image.naturalHeight);
}

/** The face straightened from the photograph at full size, as WebP: what a cut keeps. */
export async function straightened(image: HTMLImageElement, quad: Quad, ratio: number): Promise<Blob> {
  const full = pixelsOf(image, LONGEST_PX);
  const [w, h] = cutSize(quad, full.width, full.height, ratio);
  const canvas = draw(straighten(full, quad, w, h), w, h);
  try {
    return await encodeWebp(canvas);
  } finally {
    canvas.width = canvas.height = 0;
  }
}
