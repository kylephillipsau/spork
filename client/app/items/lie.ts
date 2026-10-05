import { centimetres } from "../common/format.ts";

import type { Lie } from "./cut.ts";

/**
 * How a cut's corners lie against its face as measured, in words (D214): said
 * beside the cut before it is kept, so the person checks it and then keeps it.
 * `face` as the screen names it, lower case; `measured` across and down, in mm.
 */
export function lieSaid(lie: Lie | null, face: string, measured: [number, number] | null): { out: boolean; text: string } {
  if (!measured || !lie) {
    return { out: false, text: "No measured size to check it against: it is cut to its corners. Check the thick edge is its top." };
  }
  const [across, down] = measured;
  const size = `${centimetres(across)} × ${centimetres(down)} cm`;
  const shape = across >= down ? "wider than tall" : "taller than wide";
  switch (lie) {
    case "matches":
      return { out: false, text: `Matches the ${face} as measured, ${size}. Check the thick edge is its top.` };
    case "turned":
      return {
        out: true,
        text: `A quarter turn out: the ${face} is ${shape} (${size}), and these corners mark it ${across >= down ? "taller than wide" : "wider than tall"}. Turn it until the thick edge is its top.`,
      };
    case "neither":
      return {
        out: true,
        text: `These corners don’t match the ${face} as measured (${size}) either way round. Check this is the ${face}, and the corners are on its corners.`,
      };
  }
}
