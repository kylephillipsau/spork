import * as ort from "onnxruntime-web/wasm";
import mjs from "onnxruntime-web/ort-wasm-simd-threaded.mjs?url";
import wasm from "onnxruntime-web/ort-wasm-simd-threaded.wasm?url";

import { toCorners, type Pixels, type Point } from "./cut";
import { SAM_SIZE, faceFrom, samInput, wholesFrom } from "./faceFind";

/**
 * The face-finder's worker (D177): the model runs here, off the page's
 * thread, so the crop screen can be dragged while it thinks.
 *
 * **Its own bundle, and the runtime's glue a file of its own.** The runtime
 * starts its threads (on HTTPS or at localhost, where the page is isolated)
 * from the script its glue came from. Bundled into this worker or the page,
 * that script would be the application's, which cannot run as a thread; as
 * its own file it is only the glue. Vite resolves `onnxruntime-web/wasm` to
 * the build that loads the glue by URL (`vite.config.ts`).
 *
 * **The photograph is encoded once.** That is the slow half, several seconds
 * on a phone over plain HTTP; asking again at another point reuses it.
 */

ort.env.wasm.wasmPaths = { mjs, wasm };

/** Where `scripts/fetch-model.mjs` puts the model, which the build copies to `dist/assets/`. */
const MODEL = `${import.meta.env.BASE_URL}assets/models/slimsam-77-5850ab45`;

export type Ask =
  | { id: number; kind: "warm" }
  | { id: number; kind: "find"; key: string; pixels: Pixels; at: Point }
  | { id: number; kind: "wholes"; key: string; pixels: Pixels; at: Point[][] };
/** A face's corners, the outlines a whole thing could be (D240), or why there is neither. */
export type Answer = { id: number; corners: number[] | null } | { id: number; outlines: Uint8Array[] } | { id: number; error: string };

let sessions: Promise<[ort.InferenceSession, ort.InferenceSession]> | undefined;
function warm() {
  sessions ??= Promise.all([
    ort.InferenceSession.create(`${MODEL}/vision_encoder_quantized.onnx`),
    ort.InferenceSession.create(`${MODEL}/prompt_encoder_mask_decoder_quantized.onnx`),
  ]).catch((error: unknown) => {
    // Not kept: a failed fetch is tried again the next time it is asked.
    sessions = undefined;
    throw error;
  });
  return sessions;
}

/** One run at a time: the runtime takes one at a time on a model. */
let line: Promise<unknown> = Promise.resolve();
function inTurn<T>(job: () => Promise<T>): Promise<T> {
  const next = line.then(job, job);
  line = next.catch(() => undefined);
  return next;
}

/** The last photograph encoded, by the key it was asked under. */
let encoded: { key: string; embeddings: Promise<ort.InferenceSession.OnnxValueMapType> } | null = null;

/**
 * The model's outlines of what is at a point of a photograph, or of the one
 * thing at all of several, and how sure it is of each.
 */
async function outlines(key: string, pixels: Pixels, at: Point | Point[]): Promise<{ logits: Float32Array; scores: Float32Array; point: Point }> {
  if (Math.max(pixels.width, pixels.height) !== SAM_SIZE) throw new Error(`the model takes ${SAM_SIZE} px on the longest side`);
  const [encoder, decoder] = await warm();
  if (encoded?.key !== key) {
    const input = new ort.Tensor("float32", samInput(pixels), [1, 3, SAM_SIZE, SAM_SIZE]);
    const embeddings = inTurn(() => encoder.run({ pixel_values: input }));
    encoded = { key, embeddings };
    // A failed encoding is not kept: the next ask tries again.
    embeddings.catch(() => {
      if (encoded?.embeddings === embeddings) encoded = null;
    });
  }
  const embeddings = await encoded.embeddings;
  const points = (Array.isArray(at[0]) ? at : [at]) as Point[];
  const scaled = points.map(([x, y]): Point => [x * pixels.width, y * pixels.height]);
  const point = scaled[0]!;
  const out = await inTurn(() =>
    decoder.run({
      input_points: new ort.Tensor("float32", Float32Array.from(scaled.flat()), [1, 1, scaled.length, 2]),
      input_labels: new ort.Tensor("int64", BigInt64Array.from(scaled.map(() => 1n)), [1, 1, scaled.length]),
      image_embeddings: embeddings.image_embeddings!,
      image_positional_embeddings: embeddings.image_positional_embeddings!,
    }),
  );
  return { logits: out.pred_masks!.data as Float32Array, scores: out.iou_scores!.data as Float32Array, point };
}

async function find(key: string, pixels: Pixels, at: Point): Promise<number[] | null> {
  const { logits, scores, point } = await outlines(key, pixels, at);
  const face = faceFrom(logits, scores, pixels.width, pixels.height, point);
  return face ? toCorners(face) : null;
}

addEventListener("message", (e: MessageEvent<Ask>) => {
  const ask = e.data;
  const failed = (error: unknown) => postMessage({ id: ask.id, error: String(error) } satisfies Answer);
  if (ask.kind === "wholes") {
    const { width: w, height: h } = ask.pixels;
    // One point after another: the photograph is encoded once, for the first.
    ask.at
      .reduce<Promise<Uint8Array[]>>(
        (so, at) =>
          so.then((found) => outlines(ask.key, ask.pixels, at).then(({ logits, scores, point }) => [...found, ...wholesFrom(logits, scores, w, h, point)])),
        Promise.resolve([]),
      )
      .then((found) => postMessage({ id: ask.id, outlines: found } satisfies Answer, { transfer: found.map((o) => o.buffer as ArrayBuffer) }), failed);
    return;
  }
  const answer = ask.kind === "warm" ? warm().then(() => null) : find(ask.key, ask.pixels, ask.at);
  answer.then((corners) => postMessage({ id: ask.id, corners } satisfies Answer), failed);
});
