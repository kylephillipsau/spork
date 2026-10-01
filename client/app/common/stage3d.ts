import { Color, Scene, WebGLRenderer, type OrthographicCamera, type PerspectiveCamera, type Vector3 } from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";

/**
 * A three.js canvas in a pane, and the parts every 3D view here shares: the
 * renderer, a turnable camera, a frame drawn only when something changes, the
 * pane's size, the theme's colours, a smooth move of the camera, and putting it
 * all away again. The warehouse's site and an item's box each build on one of
 * these and keep only what is theirs.
 *
 * **Drawn on demand.** A frame is drawn when the view moves or a caller says
 * something changed, and never otherwise, so an idle pane costs nothing.
 */

/** Where the camera is, what it looks at, and how far in. */
export interface Pose {
  position: Vector3;
  target: Vector3;
  zoom: number;
}

export interface StageHooks {
  /** Before each frame is drawn. True while the caller is still moving something. */
  frame?: ((now: number) => boolean) | undefined;
  /** After each frame is drawn. */
  drawn?: (() => void) | undefined;
  /** The pane changed size; the renderer already has. */
  resized?: ((width: number, height: number) => void) | undefined;
  /** The theme changed: colours read from tokens should be read again. */
  themed?: (() => void) | undefined;
}

export class Stage<C extends PerspectiveCamera | OrthographicCamera> {
  /** Whether this browser can draw it at all: three.js needs WebGL 2. */
  static supported(): boolean {
    try {
      return !!document.createElement("canvas").getContext("webgl2");
    } catch {
      return false;
    }
  }

  readonly renderer = new WebGLRenderer({ antialias: true, alpha: true });
  readonly scene = new Scene();
  readonly controls: OrbitControls;
  /** Reduced motion asked for: nothing glides. */
  readonly still = matchMedia("(prefers-reduced-motion: reduce)").matches;

  private readonly probe = document.createElement("span");
  private readonly resize: ResizeObserver;
  private readonly themed: MutationObserver;
  private pending = 0;
  private tween: { from: Pose; to: Pose; start: number; ms: number } | null = null;

  constructor(
    readonly host: HTMLElement,
    readonly camera: C,
    private readonly hooks: StageHooks = {},
  ) {
    this.renderer.setClearColor(0x000000, 0);
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    this.canvas.setAttribute("aria-hidden", "true");
    this.probe.hidden = true;
    host.append(this.canvas, this.probe);
    // Sized now, and told to the owner when the observer first reports, by
    // which time the owner has finished being made.
    this.size(false);

    this.controls = new OrbitControls(camera, this.canvas);
    this.controls.enableDamping = !this.still;
    this.controls.dampingFactor = 0.12;
    this.controls.rotateSpeed = 0.7;
    this.controls.addEventListener("change", this.invalidate);
    // A driver reset takes the context away; three.js rebuilds what it needs
    // when it comes back, and the frame has to be asked for again.
    this.canvas.addEventListener("webglcontextrestored", this.invalidate);

    this.resize = new ResizeObserver(() => {
      this.size();
      this.invalidate();
    });
    this.resize.observe(host);
    this.themed = new MutationObserver(() => {
      this.hooks.themed?.();
      this.invalidate();
    });
    this.themed.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  }

  get canvas(): HTMLCanvasElement {
    return this.renderer.domElement;
  }

  /** A token's colour as the page has it now, whatever it was written as. */
  token(name: string): Color {
    this.probe.style.color = `var(${name})`;
    const [r = 0, g = 0, b = 0] = (getComputedStyle(this.probe).color.match(/[\d.]+/g) ?? []).map(Number);
    return new Color().setRGB(r / 255, g / 255, b / 255, "srgb");
  }

  /** Put the camera here at once. */
  pose(p: Pose): void {
    this.tween = null;
    this.camera.position.copy(p.position);
    this.controls.target.copy(p.target);
    this.camera.zoom = p.zoom;
    this.camera.updateProjectionMatrix();
    this.controls.update();
    this.invalidate();
  }

  /** Move the camera there smoothly, or at once when motion is not wanted. */
  animate(to: Pose, ms = 360): void {
    if (this.still) {
      this.pose(to);
      return;
    }
    const from: Pose = { position: this.camera.position.clone(), target: this.controls.target.clone(), zoom: this.camera.zoom };
    this.tween = { from, to, start: performance.now(), ms };
    this.invalidate();
  }

  /** Stop a move in progress: the person has taken hold of the view. */
  hold(): void {
    this.tween = null;
  }

  readonly invalidate = (): void => {
    if (this.pending) return;
    this.pending = requestAnimationFrame(this.frame);
  };

  dispose(): void {
    cancelAnimationFrame(this.pending);
    this.resize.disconnect();
    this.themed.disconnect();
    this.controls.removeEventListener("change", this.invalidate);
    this.controls.dispose();
    this.canvas.removeEventListener("webglcontextrestored", this.invalidate);
    this.renderer.dispose();
    this.canvas.remove();
    this.probe.remove();
  }

  private size(tell = true): void {
    const w = this.host.clientWidth;
    const h = this.host.clientHeight;
    if (w === 0 || h === 0) return;
    const ratio = this.renderer.getPixelRatio();
    if (this.canvas.width !== Math.round(w * ratio) || this.canvas.height !== Math.round(h * ratio)) {
      this.renderer.setSize(w, h, true);
    }
    if (tell) this.hooks.resized?.(w, h);
  }

  private readonly frame = (now: number): void => {
    this.pending = 0;
    let moving = false;
    if (this.tween) {
      const { from, to, start, ms } = this.tween;
      const t = Math.min(1, (now - start) / ms);
      const e = 1 - (1 - t) ** 3;
      this.camera.position.lerpVectors(from.position, to.position, e);
      this.controls.target.lerpVectors(from.target, to.target, e);
      this.camera.zoom = from.zoom + (to.zoom - from.zoom) * e;
      this.camera.updateProjectionMatrix();
      if (t >= 1) this.tween = null;
      moving = true;
    }
    // Damping carries a turn on after the pointer lets go.
    moving = this.controls.update() || moving;
    moving = (this.hooks.frame?.(now) ?? false) || moving;
    this.renderer.render(this.scene, this.camera);
    this.hooks.drawn?.();
    if (moving || this.tween) this.invalidate();
  };
}
