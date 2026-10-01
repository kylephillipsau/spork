/**
 * A photograph as it is sent: upright, WebP, at most 4096 px on its longest
 * side, and without the camera's metadata (D175).
 *
 * **Converted on the phone, before it crosses the WiFi.** A phone's JPEG is a
 * few megabytes; the same picture as WebP is a fraction of that, and it is the
 * upload that a warehouse's WiFi feels. Drawing it on a canvas turns it the
 * way the camera held it and leaves behind what a canvas does not carry, the
 * location included.
 *
 * **WebKit cannot encode WebP.** Safari and every browser on an iPhone answer a
 * request for one with a PNG, not an error, so the type of what comes back is
 * the test. Those get libwebp compiled to WebAssembly instead, fetched the
 * first time it is needed.
 */

/**
 * The longest side sent. A 12-megapixel photo fits whole; bigger ones are
 * scaled down to it, because iOS will not draw a canvas of more than about
 * 16.7 million pixels.
 */
export const LONGEST_PX = 4096;
const QUALITY = 0.85;

/** The size to draw at: its own, or scaled down so its longest side is `longest`. */
export function fit(width: number, height: number, longest = LONGEST_PX): [number, number] {
  const scale = Math.min(1, longest / Math.max(width, height));
  return [Math.round(width * scale), Math.round(height * scale)];
}

let native: Promise<boolean> | undefined;

/**
 * Whether this browser's canvas makes WebP, asked once of a single pixel:
 * asked of the photo itself, WebKit would spend a second making a full-size
 * PNG only to have it thrown away.
 */
function encodesWebp(): Promise<boolean> {
  return (native ??= new Promise((resolve) => {
    const probe = document.createElement("canvas");
    probe.width = probe.height = 1;
    probe.toBlob((blob) => resolve(blob?.type === "image/webp"), "image/webp");
  }));
}

/**
 * What is on a canvas, as WebP: the browser's own encoder where it has one,
 * libwebp in WebAssembly where it does not. A photograph and a face cut from
 * one (D176) are both made this way.
 */
export async function encodeWebp(canvas: HTMLCanvasElement): Promise<Blob> {
  if (await encodesWebp()) {
    const native = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/webp", QUALITY));
    if (native) return native;
  }
  const { default: encode } = await import("@jsquash/webp/encode");
  const g = canvas.getContext("2d")!;
  // Effort 2 of 6: two and a half times quicker than the default for a file
  // within a few percent of its size, which on a phone is the difference
  // between half a second a photo and a second and a half.
  const bytes = await encode(g.getImageData(0, 0, canvas.width, canvas.height), { quality: QUALITY * 100, method: 2 });
  return new Blob([bytes], { type: "image/webp" });
}

/**
 * The photo as WebP, or the photo as it is when this browser cannot read it:
 * the server reads the type from the bytes, and a photo sent as taken beats
 * no photo.
 */
export async function asWebp(photo: Blob): Promise<Blob> {
  const url = URL.createObjectURL(photo);
  const canvas = document.createElement("canvas");
  try {
    const image = new Image();
    image.src = url;
    await image.decode();
    // The natural size is the size the right way up.
    [canvas.width, canvas.height] = fit(image.naturalWidth, image.naturalHeight);
    canvas.getContext("2d")!.drawImage(image, 0, 0, canvas.width, canvas.height);
    return await encodeWebp(canvas);
  } catch {
    return photo;
  } finally {
    URL.revokeObjectURL(url);
    // iOS holds a canvas's memory until its size is zero, and a shift's photos add up.
    canvas.width = canvas.height = 0;
  }
}
