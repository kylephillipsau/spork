/**
 * THE LIGHT SOLVER (D121)
 *
 * One lamp, fixed in viewport space, above and slightly left. Every panel
 * on the page reflects that one source, which is what makes the page read
 * as one object rather than thirty with private suns.
 *
 * Three things this deliberately does not do, each of which shipped in an
 * earlier draft and each of which was wrong from first principles:
 *
 *  1. It does not light the body of a panel. Brightness on a matte surface
 *     is a function of the angle between its normal and the light, and a
 *     flat panel has one normal everywhere — so it is evenly lit. A
 *     highlight travelling across the field is a point light held an inch
 *     off the surface, which is why it read as unnatural however carefully
 *     it was tuned. Only the chamfers can change, because only they have a
 *     different normal.
 *
 *  2. It does not hang the lamp off the pointer. That is a torch in your
 *     hand rather than a lit room. The pointer applies a bounded parallax
 *     standing in for a viewer leaning a few degrees; scrolling is the
 *     primary motion, and it is the one that works on a device with no
 *     pointer at all.
 *
 *  3. It does not lerp. A fixed-factor lerp snaps hard, trails for a long
 *     time, and runs at whatever speed the frame rate happens to be. This
 *     is a critically damped second-order system integrated against real
 *     elapsed time: it accelerates, decelerates, and arrives without
 *     overshoot.
 *
 * It binds to `[data-material]` rather than to class names, so the CSS
 * Modules hash is never load-bearing (D123). Custom properties are the only
 * channel between this file and paint.
 */

/** Where the lamp rests, as a fraction of the viewport. */
const REST_X = 0.38;
const REST_Y = -0.06;

/** How far the pointer may move the viewer. Vertical gets the smaller
 *  budget: vertical decides which edge is lit, and that is scroll's to
 *  command. */
const PARALLAX_X = 0.34;
const PARALLAX_X_MAX = 0.15;
const PARALLAX_Y = 0.2;
const PARALLAX_Y_MAX = 0.09;

/** Spring stiffness, rad/s. Critically damped, so ζ = 1 and it settles in
 *  a little under a second with no overshoot. */
const OMEGA = 4.6;

/** Nylon's body sheen is the only consumer of --lx/--ly left, and satin
 *  barely moves, so its mapping is gentle and mostly driven by scroll. */
const SHEEN_DAMP_X = 0.15;
const SHEEN_DAMP_Y = 0.22;

/** A dropped frame must not let the spring overshoot. */
const MAX_DT = 1 / 30;

/** Below this, a custom property is not worth the style invalidation. The
 *  cost that matters here is never the arithmetic. */
const EPSILON = 0.006;
const EPSILON_PCT = 0.25;

type Solved = {
  t: number;
  b: number;
  l: number;
  r: number;
  g: number;
  x: number;
  y: number;
};

const NEVER_WRITTEN: Solved = { t: -9, b: -9, l: -9, r: -9, g: -9, x: -9, y: -9 };

export type LightSolver = {
  /** Re-scan the DOM for `[data-material]`. Call after mounting a route. */
  refresh(): void;
  stop(): void;
};

function clamp(v: number, max: number): number {
  return v < -max ? -max : v > max ? max : v;
}

export function startLightSolver(root: ParentNode = document): LightSolver {
  const reduced =
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  let panels: HTMLElement[] = [];
  let rects: DOMRect[] = [];
  let last: Solved[] = [];

  let vw = window.innerWidth;
  let vh = window.innerHeight;

  // Where the lamp is heading, where it is, and how fast.
  let targetX = vw * REST_X;
  let targetY = vh * REST_Y;
  let lampX = targetX;
  let lampY = targetY;
  let velX = 0;
  let velY = 0;

  let running = false;
  let pending = false;
  let prevTime = 0;
  let frame = 0;

  function refresh(): void {
    panels = Array.from(root.querySelectorAll<HTMLElement>("[data-material]"));
    rects = new Array(panels.length);
    last = panels.map(() => ({ ...NEVER_WRITTEN }));
    measure();
    paint();
  }

  function restLamp(): void {
    targetX = vw * REST_X;
    targetY = vh * REST_Y;
  }

  /** Read every rect, then write every property. Never interleave. */
  function measure(): void {
    for (let i = 0; i < panels.length; i++) {
      rects[i] = panels[i]!.getBoundingClientRect();
    }
  }

  /**
   * Per panel: which way the lamp lies from its centre, and how near it is.
   * Direction weights the four chamfers; distance breathes the sparkle.
   *
   * This is per-object analytic lighting — one direction and one falloff
   * per panel, evaluated once a frame. The cheap ancestor of a ray trace
   * rather than an imitation of one.
   */
  function paint(): void {
    for (let i = 0; i < panels.length; i++) {
      const rect = rects[i];
      const prev = last[i];
      if (!rect || !prev || rect.height === 0) continue;
      if (rect.bottom < -320 || rect.top > vh + 320) continue;

      const style = panels[i]!.style;

      const midX = rect.left + rect.width / 2;
      const midY = rect.top + rect.height / 2;
      const dx = lampX - midX;
      const dy = lampY - midY;
      const dist = Math.sqrt(dx * dx + dy * dy) || 1;
      const nx = dx / dist;
      const ny = dy / dist;

      // Squared, so an edge nearly side-on to the lamp stays quiet and the
      // light concentrates where the lamp actually is.
      const t = ny < 0 ? ny * ny : 0;
      const b = ny > 0 ? ny * ny : 0;
      const l = nx < 0 ? nx * nx : 0;
      const r = nx > 0 ? nx * nx : 0;

      // Falls off over roughly one viewport height.
      const near = 1 - Math.min(1, dist / (vh * 0.95));
      const g = near * near;

      if (Math.abs(t - prev.t) > EPSILON) {
        style.setProperty("--e-t", t.toFixed(3));
        prev.t = t;
      }
      if (Math.abs(b - prev.b) > EPSILON) {
        style.setProperty("--e-b", b.toFixed(3));
        prev.b = b;
      }
      if (Math.abs(l - prev.l) > EPSILON) {
        style.setProperty("--e-l", l.toFixed(3));
        prev.l = l;
      }
      if (Math.abs(r - prev.r) > EPSILON) {
        style.setProperty("--e-r", r.toFixed(3));
        prev.r = r;
      }
      if (Math.abs(g - prev.g) > EPSILON) {
        style.setProperty("--glint", g.toFixed(3));
        prev.g = g;
      }

      // Nylon alone still reads these.
      const sheenY = 50 + (((lampY - rect.top) / rect.height) * 100 - 50) * SHEEN_DAMP_Y;
      const sheenX = 50 + (((lampX - rect.left) / rect.width) * 100 - 50) * SHEEN_DAMP_X;
      if (Math.abs(sheenY - prev.y) > EPSILON_PCT) {
        style.setProperty("--ly", `${sheenY.toFixed(1)}%`);
        prev.y = sheenY;
      }
      if (Math.abs(sheenX - prev.x) > EPSILON_PCT) {
        style.setProperty("--lx", `${sheenX.toFixed(1)}%`);
        prev.x = sheenX;
      }
    }
  }

  function loop(now: number): void {
    let dt = prevTime ? (now - prevTime) / 1000 : 0.016;
    prevTime = now;
    if (dt > MAX_DT) dt = MAX_DT;

    // x" = -2ζω x' - ω²(x - target), with ζ = 1
    velX += (-2 * OMEGA * velX - OMEGA * OMEGA * (lampX - targetX)) * dt;
    velY += (-2 * OMEGA * velY - OMEGA * OMEGA * (lampY - targetY)) * dt;
    lampX += velX * dt;
    lampY += velY * dt;

    measure();
    paint();

    const settled =
      Math.abs(lampX - targetX) < 0.4 &&
      Math.abs(lampY - targetY) < 0.4 &&
      Math.abs(velX) < 1.5 &&
      Math.abs(velY) < 1.5;

    const again = pending;
    pending = false;

    if (!settled || again) {
      frame = requestAnimationFrame(loop);
    } else {
      running = false;
      prevTime = 0;
    }
  }

  function kick(): void {
    pending = true;
    if (!running) {
      running = true;
      prevTime = 0;
      frame = requestAnimationFrame(loop);
    }
  }

  function onPointerMove(event: PointerEvent): void {
    const ox = clamp((event.clientX / vw - 0.5) * PARALLAX_X, PARALLAX_X_MAX);
    const oy = clamp((event.clientY / vh - 0.5) * PARALLAX_Y, PARALLAX_Y_MAX);
    targetX = vw * (REST_X + ox);
    targetY = vh * (REST_Y + oy);
    kick();
  }

  function onLeave(): void {
    restLamp();
    kick();
  }

  function onScroll(): void {
    kick();
  }

  function onResize(): void {
    vw = window.innerWidth;
    vh = window.innerHeight;
    restLamp();
    if (reduced) {
      lampX = targetX;
      lampY = targetY;
      measure();
      paint();
    } else {
      kick();
    }
  }

  refresh();

  window.addEventListener("resize", onResize, { passive: true });
  if (!reduced) {
    document.addEventListener("pointermove", onPointerMove, { passive: true });
    document.addEventListener("pointerleave", onLeave, { passive: true });
    window.addEventListener("blur", onLeave);
    window.addEventListener("scroll", onScroll, { passive: true });
  }

  return {
    refresh,
    stop() {
      cancelAnimationFrame(frame);
      running = false;
      window.removeEventListener("resize", onResize);
      document.removeEventListener("pointermove", onPointerMove);
      document.removeEventListener("pointerleave", onLeave);
      window.removeEventListener("blur", onLeave);
      window.removeEventListener("scroll", onScroll);
    },
  };
}
