import { useId, useSyncExternalStore } from "react";

import { cx } from "@ui/index";
import { BIN, BINS, type BinState } from "./rackS";
import s from "./brand.module.css";

/**
 * Spork's mark and wordmark.
 *
 * The mark is the Rack S: an S lit on a rack face of bins, four across and
 * five up, in black anodised nylon, its bins lit off-white and one in hi-vis,
 * as the bin page lights the bin you are after.
 *
 * The wordmark is "spork" in B612 Bold, the face drawn for Airbus cockpit
 * screens, **outlined from the font** so it needs none and sits the same
 * everywhere. Under it a rule measures each letter, a tick at each edge; the
 * hi-vis length under the o is the reading. The o is a reticle: open, ready;
 * closed, measuring; dimmed with the reading out, offline. Hovering it scans
 * once, the notch turning as the reading runs to the s, the k and back.
 */

export type BrandState = "ready" | "measuring" | "offline";

/** How each bin is lit. */
const BIN_CLASS: Record<BinState, string | undefined> = { spot: s.spot, lit: s.lit, off: s.off };

/** "sp" and "rk" in B612 Bold at 100 units, baseline at 80, each at its advance and 3 units apart. */
const LETTERS =
  "M36 38.9Q34.45 37.95 32.78 37.22Q31.1 36.5 29.53 36Q27.95 35.5 26.6 35.25Q25.25 35 24.45 35Q18.6 35 18.6 38.9Q18.6 40.2 19.7 41.2Q20.8 42.2 22.6 43.1Q24.4 44 26.73 44.9Q29.05 45.8 31.45 46.9Q33.85 48 36.18 49.33Q38.5 50.65 40.3 52.43Q42.1 54.2 43.2 56.5Q44.3 58.8 44.3 61.8Q44.3 67 42.5 70.6Q40.7 74.2 37.55 76.5Q34.4 78.8 30.13 79.83Q25.85 80.85 20.95 80.85Q19.05 80.85 16.5 80.4Q13.95 79.95 11.33 79.2Q8.7 78.45 6.25 77.43Q3.8 76.4 2.1 75.2L7.1 65.7Q8.9 66.6 10.88 67.4Q12.85 68.2 14.75 68.78Q16.65 69.35 18.4 69.68Q20.15 70 21.5 70Q22.9 70 24.23 69.6Q25.55 69.2 26.55 68.38Q27.55 67.55 28.15 66.28Q28.75 65 28.75 63.2Q28.75 61.4 27.68 60.08Q26.6 58.75 24.83 57.65Q23.05 56.55 20.78 55.6Q18.5 54.65 16.13 53.58Q13.75 52.5 11.48 51.25Q9.2 50 7.43 48.33Q5.65 46.65 4.58 44.48Q3.5 42.3 3.5 39.35Q3.5 35.3 5.03 32.4Q6.55 29.5 9.32 27.67Q12.1 25.85 16 25Q19.9 24.15 24.6 24.15Q26.8 24.15 29.03 24.57Q31.25 25 33.35 25.67Q35.45 26.35 37.28 27.2Q39.1 28.05 40.45 28.95L36 38.9Z" +
  "M73.3 70L77.2 70Q80.55 70 83.2 68.53Q85.85 67.05 87.7 64.5Q89.55 61.95 90.53 58.53Q91.5 55.1 91.5 51.2Q91.5 47.15 90.68 44.1Q89.85 41.05 88.58 39.03Q87.3 37 85.73 36Q84.15 35 82.65 35Q81.3 35 79.97 35.4Q78.65 35.8 77.45 36.42Q76.25 37.05 75.2 37.85Q74.15 38.65 73.3 39.45L73.3 70ZM57 105L57 25.35L73.3 23.7L73.3 32.8L79.5 24.5Q80.55 24.35 81.68 24.25Q82.8 24.15 84 24.15Q88.65 24.15 92.73 26.1Q96.8 28.05 99.83 31.67Q102.85 35.3 104.63 40.4Q106.4 45.5 106.4 51.8Q106.4 57.95 104.28 63.13Q102.15 68.3 98.23 72.05Q94.3 75.8 88.75 77.9Q83.2 80 76.35 80L73.3 80L73.3 105L57 105Z" +
  "M214.95 36.65Q213.95 36.15 213.15 35.82Q212.35 35.5 211.55 35.3Q210.75 35.1 209.88 35.05Q209 35 207.85 35Q206.45 35 205.03 35.67Q203.6 36.35 202.28 37.5Q200.95 38.65 199.85 40.13Q198.75 41.6 198 43.2L198 80L181.7 80L181.7 25.35L198 23.7L198 35.25L202.7 25Q203.5 24.7 204.98 24.42Q206.45 24.15 207.75 24.15Q210.5 24.15 212.28 24.57Q214.05 25 215.65 25.8L214.95 36.65Z" +
  "M225 80L225 10.15L223.9 0L240.3 -1.55L240.3 41.55L253.55 25L270.6 25L252.5 45L242.3 48.65L251.75 51.8L275.65 80L258.25 80L240.3 57.1L240.3 80L225 80Z";
/** The o, apart so its notch can be cut; its middle is at 142.7, 52.5. */
const O =
  "M142.7 80.85Q136.7 80.85 131.75 78.68Q126.8 76.5 123.23 72.68Q119.65 68.85 117.68 63.65Q115.7 58.45 115.7 52.35Q115.7 46.3 117.6 41.13Q119.5 35.95 123.03 32.17Q126.55 28.4 131.53 26.28Q136.5 24.15 142.7 24.15Q148.7 24.15 153.68 26.28Q158.65 28.4 162.23 32.17Q165.8 35.95 167.78 41.13Q169.75 46.3 169.75 52.35Q169.75 58.45 167.85 63.65Q165.95 68.85 162.43 72.68Q158.9 76.5 153.9 78.68Q148.9 80.85 142.7 80.85ZM153.45 53.6Q153.45 50.2 152.85 46.9Q152.25 43.6 150.93 40.97Q149.6 38.35 147.53 36.75Q145.45 35.15 142.5 35.15Q139.75 35.15 137.75 36.53Q135.75 37.9 134.48 40.2Q133.2 42.5 132.6 45.43Q132 48.35 132 51.45Q132 54.9 132.6 58.2Q133.2 61.5 134.53 64.08Q135.85 66.65 137.9 68.25Q139.95 69.85 142.85 69.85Q145.6 69.85 147.6 68.47Q149.6 67.1 150.9 64.8Q152.2 62.5 152.82 59.6Q153.45 56.7 153.45 53.6Z";
/** The rule under the letters, a tick at each letter's edge. */
const RULE = "M0 113H275" + [0, 48, 111, 174, 217, 275].map((x) => `M${x} 113V118`).join("");

/** The Rack S. Named, so a sidebar showing it alone still says what it is. */
export function BrandMark({ className }: { className?: string | undefined }) {
  const id = useId().replace(/:/g, "");
  return (
    <svg viewBox="0 0 120 120" className={className} role="img" aria-label="Spork">
      <defs>
        <pattern id={`${id}rip`} width="28" height="28" patternUnits="userSpaceOnUse">
          <path d="M0 1H28M1 0V28" className={s.rip} />
        </pattern>
        <linearGradient id={`${id}sheen`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" className={s.sheenHi} />
          <stop offset="1" className={s.sheenLo} />
        </linearGradient>
      </defs>
      <rect width="120" height="120" rx="24" className={s.tile} />
      <rect width="120" height="120" rx="24" fill={`url(#${id}rip)`} />
      <rect width="120" height="120" rx="24" fill={`url(#${id}sheen)`} />
      <rect x="1" y="1" width="118" height="118" rx="23" className={s.edge} />
      {BINS.map((b) => (
        <rect key={`${b.x},${b.y}`} x={b.x} y={b.y} width={BIN.width} height={BIN.height} rx={BIN.rx} className={BIN_CLASS[b.state]} />
      ))}
    </svg>
  );
}

/** "spork" over its rule, the o and the reading showing its state. In `currentColor`. */
export function BrandWordmark({ className, state = "ready" }: { className?: string | undefined; state?: BrandState }) {
  const id = useId().replace(/:/g, "");
  return (
    <svg viewBox="-2 -5 279 126" className={cx(className, s[state])} aria-hidden>
      <defs>
        <mask id={`${id}notch`} maskUnits="userSpaceOnUse" x="-10" y="-10" width="300" height="140">
          <rect x="-10" y="-10" width="300" height="140" fill="white" />
          <g className={s.turn}>
            <rect x="156.1" y="48.5" width="22" height="8" transform="rotate(-45 142.7 52.5)" className={s.notch} />
          </g>
        </mask>
      </defs>
      <path d={LETTERS} fill="currentColor" />
      <path d={O} fill="currentColor" mask={`url(#${id}notch)`} className={s.o} />
      <path d={RULE} className={s.rule} />
      <path d={state === "measuring" ? "M2 113H273" : "M119 113H166"} className={s.reading} />
    </svg>
  );
}

/**
 * The mark and the wordmark together, scanning once on hover. `offline`
 * when the server cannot be reached; the classes size each part where it is
 * shown, and let a collapsed sidebar hide the wordmark.
 */
export function Brand({ markClassName, wordmarkClassName, state = "ready" }: { markClassName?: string | undefined; wordmarkClassName?: string | undefined; state?: BrandState }) {
  return (
    <span className={s.brand} title={state === "offline" ? "Offline: the server can't be reached" : undefined}>
      <BrandMark className={markClassName} />
      <BrandWordmark className={wordmarkClassName} state={state} />
    </span>
  );
}

const subscribe = (changed: () => void) => {
  window.addEventListener("online", changed);
  window.addEventListener("offline", changed);
  return () => {
    window.removeEventListener("online", changed);
    window.removeEventListener("offline", changed);
  };
};

/** Whether the device has a network at all: without one, nothing reaches the server. */
export function useOnline(): boolean {
  return useSyncExternalStore(subscribe, () => navigator.onLine, () => true);
}
