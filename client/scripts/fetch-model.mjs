#!/usr/bin/env node
/**
 * THE MODEL THAT FINDS A BOX'S FACE, FETCHED AND CHECKED (D177).
 *
 * SlimSAM-77, quantized, in the ONNX form transformers.js publishes
 * (Xenova/slimsam-77-uniform, Apache-2.0). Two files, about 14 MB, kept out of
 * git and fetched here into `public/assets/models/`, which the build copies
 * to `dist/assets/`, where the server keeps them cached for a year.
 *
 * **Pinned twice.** The download names a commit rather than `main`, so the
 * address cannot drift, and each file's SHA-256 is checked before it is kept,
 * so what lands is what was tested. A file already here and matching is left
 * alone, so a build offline after the first is fine.
 *
 *   node scripts/fetch-model.mjs
 */
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = "Xenova/slimsam-77-uniform";
const REVISION = "5850ab45f587c112167512ffef949107115e26a0";
/** Where the client asks for them: `client/app/items/faceModel.ts` says the same. */
export const MODEL_DIR = "assets/models/slimsam-77-5850ab45";
const FILES = {
  "vision_encoder_quantized.onnx": "cce23c7b2e5d4f330932738fb67ba518e04b0d99ccdd1cccd22a7da4e01f2971",
  "prompt_encoder_mask_decoder_quantized.onnx": "cb90b279f549d2cab7fd6e20c38522438c65d84bdcca3d2a764cff7d857fdce2",
};

const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, "..", "public", MODEL_DIR);
mkdirSync(out, { recursive: true });

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

for (const [name, want] of Object.entries(FILES)) {
  const path = join(out, name);
  if (existsSync(path) && sha256(readFileSync(path)) === want) continue;
  const url = `https://huggingface.co/${REPO}/resolve/${REVISION}/onnx/${name}`;
  const response = await fetch(url);
  if (!response.ok) throw new Error(`fetching ${url}: ${response.status}`);
  const bytes = Buffer.from(await response.arrayBuffer());
  const got = sha256(bytes);
  if (got !== want) throw new Error(`${name} is not the file that was tested: sha256 ${got}, expected ${want}`);
  writeFileSync(`${path}.part`, bytes);
  renameSync(`${path}.part`, path);
  console.log(`model: ${name}, ${(bytes.length / 1e6).toFixed(1)} MB, checked`);
}

writeFileSync(
  join(out, "NOTICE"),
  `SlimSAM-77 (uniform), converted to ONNX by Xenova: https://huggingface.co/${REPO}\n` +
    `Revision ${REVISION}. Licensed under the Apache License, Version 2.0.\n` +
    `SlimSAM: https://github.com/czg1225/SlimSAM. Segment Anything: Meta AI, Apache-2.0.\n`,
);
