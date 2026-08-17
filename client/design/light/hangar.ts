/**
 * THE GROUND (L0) — an isometric hangar.
 *
 * Depth comes from the ground being rich, not from the panels working
 * harder: elements floating over a real space read as dimensional, and the
 * same elements over a flat colour read as stickers however elaborate their
 * shadows. That is also why the panels stayed opaque — metal occludes, and
 * translucency buys depth by contradicting the material.
 *
 * Drawn once per resize. Never holds content.
 */

const ISO = Math.PI / 6;
const GRID_STEP = 64;

/**
 * Canvas cannot read a custom property, so the ground resolves the same two
 * light tokens the materials use and builds its own strings. Without this
 * the hangar would be the one surface lit by colours that live nowhere in
 * tokens.css — a hole the laws checker cannot see, since it reads CSS.
 */
function lightTokens(): { hi: string; lo: string } {
  const root = getComputedStyle(document.documentElement);
  return {
    hi: root.getPropertyValue("--hi").trim() || "214, 228, 255",
    lo: root.getPropertyValue("--lo").trim() || "10, 7, 5",
  };
}

function bay(
  ctx: CanvasRenderingContext2D,
  ox: number,
  oy: number,
  scale: number,
  mirrored: boolean,
  hi: string,
): void {
  const c = Math.cos(ISO);
  const s = Math.sin(ISO);
  const sign = mirrored ? -1 : 1;
  const p = (x: number, y: number, z: number): [number, number] => [
    ox + (x - y) * c * scale * sign,
    oy + ((x + y) * s - z) * scale,
  ];

  ctx.strokeStyle = `rgba(${hi}, 0.052)`;
  ctx.beginPath();

  // uprights
  for (let u = 0; u <= 3; u++) {
    for (let v = 0; v <= 1; v++) {
      const a = p(u * 2, v * 2, 0);
      const b = p(u * 2, v * 2, 9);
      ctx.moveTo(a[0], a[1]);
      ctx.lineTo(b[0], b[1]);
    }
  }

  // beams at four levels
  for (let level = 0; level <= 3; level++) {
    const z = level * 3;
    const p0 = p(0, 0, z);
    const p1 = p(6, 0, z);
    const p2 = p(6, 2, z);
    const p3 = p(0, 2, z);
    ctx.moveTo(p0[0], p0[1]);
    ctx.lineTo(p1[0], p1[1]);
    ctx.moveTo(p3[0], p3[1]);
    ctx.lineTo(p2[0], p2[1]);
    ctx.moveTo(p1[0], p1[1]);
    ctx.lineTo(p2[0], p2[1]);
    ctx.moveTo(p0[0], p0[1]);
    ctx.lineTo(p3[0], p3[1]);
  }

  ctx.stroke();
}

export function drawHangar(canvas: HTMLCanvasElement): void {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const w = window.innerWidth;
  const h = window.innerHeight;

  canvas.width = w * dpr;
  canvas.height = h * dpr;
  canvas.style.width = `${w}px`;
  canvas.style.height = `${h}px`;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);

  const { hi, lo } = lightTokens();

  // Technical-drawing paper, seen almost edge on.
  ctx.lineWidth = 1;
  ctx.strokeStyle = `rgba(${hi}, 0.028)`;
  ctx.beginPath();
  const span = w + h * 2;
  const run = h / Math.tan(Math.PI / 2 - ISO);
  for (let i = -span; i < span; i += GRID_STEP) {
    ctx.moveTo(i, 0);
    ctx.lineTo(i + run, h);
    ctx.moveTo(i, 0);
    ctx.lineTo(i - run, h);
  }
  ctx.stroke();

  // Racking at both aisle edges, in outline only.
  bay(ctx, -40, h * 0.42, 26, false, hi);
  bay(ctx, w + 40, h * 0.3, 26, true, hi);

  // The lamp's pool, at the lamp's resting position — these must agree with
  // REST_X / REST_Y in solver.ts, or the room is lit from one place and
  // glowing from another.
  const pool = ctx.createRadialGradient(w * 0.38, -h * 0.06, 0, w * 0.38, -h * 0.06, h * 1.15);
  pool.addColorStop(0, `rgba(${hi}, 0.075)`);
  pool.addColorStop(0.45, `rgba(${hi}, 0.022)`);
  pool.addColorStop(1, `rgba(${hi}, 0)`);
  ctx.fillStyle = pool;
  ctx.fillRect(0, 0, w, h);

  const vignette = ctx.createRadialGradient(
    w * 0.5,
    h * 0.42,
    h * 0.25,
    w * 0.5,
    h * 0.5,
    h * 1.05,
  );
  vignette.addColorStop(0, `rgba(${lo}, 0)`);
  vignette.addColorStop(1, `rgba(${lo}, 0.64)`);
  ctx.fillStyle = vignette;
  ctx.fillRect(0, 0, w, h);
}
