import { useEffect, useRef, type ReactNode } from "react";
import { startLightSolver, type LightSolver } from "./solver";
import { drawHangar } from "./hangar";
import styles from "./light-room.module.css";

/**
 * Mounted once, at the app root. Everything that renders a material must
 * sit inside it, because the lamp is a property of the room rather than of
 * any panel.
 *
 * `density` is the surface, not the viewport (D109): Bench and Desk are
 * both wide screens and want different row heights, and the Floor handheld
 * wants 48px targets whatever its width.
 */
export function LightRoom({
  density,
  children,
}: {
  density: "floor" | "desk";
  children: ReactNode;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const solverRef = useRef<LightSolver | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas) drawHangar(canvas);

    const onResize = () => {
      if (canvasRef.current) drawHangar(canvasRef.current);
    };
    window.addEventListener("resize", onResize, { passive: true });

    const solver = startLightSolver(document);
    solverRef.current = solver;

    return () => {
      window.removeEventListener("resize", onResize);
      solver.stop();
      solverRef.current = null;
    };
  }, []);

  return (
    <div data-density={density} className={styles.room}>
      <canvas ref={canvasRef} data-layer="ground" aria-hidden="true" />
      <div className={styles.grain} aria-hidden="true" />
      {children}
    </div>
  );
}

/**
 * Re-scan for panels after a route change. The solver caches the node list,
 * so a screen that mounts new materials must say so.
 */
export function useRelight(solver: LightSolver | null, deps: unknown[]): void {
  useEffect(() => {
    solver?.refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
}
