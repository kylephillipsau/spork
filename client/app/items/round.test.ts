import { strict as assert } from "node:assert";
import { test } from "node:test";

import { fitPose, outlineOf, profile, type Lens, type Mask, type Pose, type RoundSize } from "./round.ts";

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
