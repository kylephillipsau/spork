import { strict as assert } from "node:assert";
import { test } from "node:test";

import { discOf, fitPose, lineUp, outlineOf, profile, turnedBetween, type Layer, type Lens, type Mask, type Pose, type RoundSize } from "./round.ts";

/** The outline a camera would see, filled in, as the face-finder gives one. */
function maskOf(pose: Pose, lens: Lens, size: RoundSize, width: number, height: number): Mask {
  const poly = outlineOf(pose, lens, profile(size));
  const data = new Uint8Array(width * height);
  for (let y = 0; y < height; y++) {
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
    for (let x = Math.max(0, Math.ceil(lo)); x < Math.min(width, hi); x++) data[y * width + x] = 1;
  }
  return { data, width, height };
}

test("the camera behind a photograph of a bucket is found again from its outline (D240)", () => {
  const size: RoundSize = { top: 303, base: 256, height: 310, band: 60 };
  const lens: Lens = { f: 768, cx: 384, cy: 512 };
  const truth: Pose = { d: 480, h: 240, yaw: 0.01, pitch: 0.17, roll: -0.01 };
  const outline = maskOf(truth, lens, size, 768, 1024);
  // The outline without its band, which the face-finder can take for a lid of its own.
  const body = maskOf(truth, lens, { ...size, height: size.height - size.band, band: 0 }, 768, 1024);
  const found = fitPose([body, outline], lens, size)!;
  assert.equal(found.mask, 1, "the whole bucket, not its body alone");
  assert.ok(found.fit > 0.97, `fits ${found.fit}`);
  assert.ok(Math.abs(found.pose.d - truth.d) / truth.d < 0.05, `distance ${found.pose.d}`);
});

test("a lid is found as the largest outline that is an ellipse, not the floor round it (D240)", () => {
  const width = 300, height = 400;
  const lid = (x: number, y: number) => (x - 150) ** 2 + (y - 200) ** 2 <= 70 ** 2;
  const shadow = (x: number, y: number) => (x - 80) ** 2 + (y - 140) ** 2 <= 50 ** 2;
  const mask = (inside: (x: number, y: number) => boolean): Mask => {
    const data = new Uint8Array(width * height);
    for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) data[y * width + x] = inside(x + 0.5, y + 0.5) ? 1 : 0;
    return { data, width, height };
  };
  // The floor with the lid a hole in it; the lid with its shadow; the lid; the panel in it.
  const masks = [mask((x, y) => !lid(x, y)), mask((x, y) => lid(x, y) || shadow(x, y)), mask(lid), mask((x, y) => (x - 150) ** 2 + (y - 200) ** 2 <= 50 ** 2)];
  const e = discOf(masks)!;
  assert.ok(Math.abs(e.cx - 150) < 1 && Math.abs(e.cy - 200) < 1, `middle ${e.cx}, ${e.cy}`);
  assert.ok(Math.abs(e.a - 70) < 2 && Math.abs(e.b - 70) < 2, `half-widths ${e.a}, ${e.b}`);
});

test("how far it was turned between neighbouring photos is found either way round, the last back to the first (D240)", () => {
  const rad = (d: number) => (d * Math.PI) / 180;
  const deg = (t: { by: number }[]) => t.map((x) => Math.round((x.by * 180) / Math.PI));
  // A bucket turned 65°, 85°, 129° and 81°, lined up turning right, and then left.
  assert.deepEqual(deg(turnedBetween([0, 1, 2, 3], [0, 65, 150, 279].map(rad))), [65, 85, 129, 81]);
  assert.deepEqual(deg(turnedBetween([0, 1, 2, 3], [0, -76, -206, -280].map(rad))), [76, 130, 74, 80]);
  // Without its left photo, nothing to say past the back.
  assert.deepEqual(deg(turnedBetween([0, 1, 2], [0, 65, 150].map(rad))), [65, 85]);
});

test("photos a quarter turn apart are lined up though they share the side between them only at a slant (D240)", () => {
  // A side 360 columns round, a degree each, patterned at random; four photos
  // of it meant a quarter turn apart and each a few degrees out, each trusted
  // less and less away from where it faced, as a photo from nearby is.
  const width = 360, height = 20, out = [0, 5, -4, 3];
  let seed = 7;
  const random = () => ((seed = (seed * 16807) % 2147483647) / 2147483647) * 255;
  const side = Float32Array.from({ length: width * height }, random);
  const layers: Layer[] = out.map((e, k) => {
    const rgb = new Float32Array(width * height * 3), weight = new Float32Array(width * height);
    for (let j = 0; j < height; j++) {
      for (let i = 0; i < width; i++) {
        const o = j * width + i, v = side[j * width + ((i + e + width) % width)]!;
        rgb.fill(v, o * 3, o * 3 + 3);
        weight[o] = Math.max(Math.cos(((i - k * 90) * Math.PI) / 180), 0) ** 12;
      }
    }
    return { width, height, rgb, weight };
  });
  assert.deepEqual(lineUp(layers, 45).at, out);
});
