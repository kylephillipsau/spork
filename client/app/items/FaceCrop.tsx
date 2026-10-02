import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent } from "react";
import { RotateCw } from "lucide-react";

import { Alert, Button, Dialog } from "@ui/index";
import { imageUrl } from "@domain/api";
import { Faint } from "@app/common/cells";

import { draw, handheld, loadPhoto, pixelsOf, ratioOf, straightened } from "./crop";
import { START, WHOLE, cutSize, fromCorners, isFace, straighten, toCorners, turn, type Pixels, type Point, type Quad } from "./cut";
import { SAM_SIZE } from "./faceFind";
import type { PropertiesDesk, Cropping } from "./useItemProperties";
import s from "./items.module.css";

/**
 * A photograph cut to the face it is of (D176). Four corners to drag onto the
 * face's corners, the face straightened beside them as they move, and a turn
 * for a face photographed sideways. The same screen from an item's photo, at
 * a computer's queue of photos to cut (D181), and on a phone by hand.
 *
 * **Dragged by how far the pointer moves**, not to where it is, so a finger
 * on a corner moves it without jumping it under the fingertip, and arrow keys
 * move one a little at a time.
 *
 * **The corners are found first** (D177): a model looks for the face in the
 * middle of a photograph not cut before, and a tap on the photograph asks it
 * again at that point. Its answer never moves a corner somebody has moved
 * since they asked. Not on a phone, which has not the memory (D181): there the
 * corners are dragged.
 *
 * `desk` is what the screen asks of whoever opened it, so an item's
 * properties and the queue open the same screen.
 */
export type CropDesk = Pick<PropertiesDesk, "findFace" | "cut" | "uncrop" | "busy" | "problem" | "dismiss">;

/** What the face-finder is doing, as the screen says it. */
type Finding = "looking" | "found" | "missed" | "broken" | "phone" | null;
const FINDING: Record<Exclude<Finding, null>, string> = {
  looking: "Finding the face…",
  found: "Found it. Drag a corner to correct it, or tap the face to look again.",
  missed: "No face found there. Tap the face, or drag the corners.",
  broken: "The face-finder could not run here. Drag the corners.",
  phone: "Drag the corners onto the face. At a computer, the face-finder places them.",
};

const CORNERS = ["Top-left", "Top-right", "Bottom-right", "Bottom-left"] as const;
/** How far an arrow key moves a corner, and with Shift. */
const NUDGE = 0.004;
const SHOVE = 0.02;

const clamp = (n: number) => Math.min(1, Math.max(0, n));

export function FaceCrop({
  cropping,
  name,
  aspect,
  desk,
}: {
  cropping: Cropping;
  /** The face, as the screen names it: "Front", "Label", "Photo". */
  name: string;
  /** Its width over its height when its size is measured; otherwise from the corners. */
  aspect: number | null;
  desk: CropDesk;
}) {
  const [quad, setQuad] = useState<Quad>(() => (cropping.corners ? fromCorners(cropping.corners) : START));
  const [photo, setPhoto] = useState<{ image: HTMLImageElement; small: Pixels } | "failed" | null>(null);
  const [finding, setFinding] = useState<Finding>(null);
  const stage = useRef<HTMLDivElement>(null);
  const preview = useRef<HTMLCanvasElement>(null);
  const drag = useRef<{ corner: number; x: number; y: number; from: Point } | null>(null);
  const tap = useRef<{ x: number; y: number } | null>(null);
  // Bumped by every ask and every hand on a corner: an answer applies only
  // to the ask it answers, and only while nobody has moved a corner since.
  const asked = useRef(0);
  const face = name.toLowerCase();

  const find = (image: HTMLImageElement, at?: Point) => {
    if (handheld()) return setFinding("phone");
    const ask = ++asked.current;
    setFinding("looking");
    desk.findFace(cropping.image_id, pixelsOf(image, SAM_SIZE, true), at).then(
      (corners) => {
        if (ask !== asked.current) return;
        const found = corners && fromCorners(corners);
        if (found && isFace(found)) {
          setQuad(found);
          setFinding("found");
        } else setFinding("missed");
      },
      (error: unknown) => {
        if (ask !== asked.current) return;
        // Said on the screen in words; the reason is for whoever opens the console.
        console.warn("face-finder:", error);
        setFinding("broken");
      },
    );
  };

  // The photograph, and a small copy of its pixels for the preview.
  useEffect(() => {
    let live = true;
    loadPhoto(cropping.digest).then(
      (image) => {
        if (!live) return;
        setPhoto({ image, small: pixelsOf(image, 720) });
        // A photograph cut before keeps its corners; a new one is looked at.
        if (!cropping.corners) find(image);
      },
      () => live && setPhoto("failed"),
    );
    return () => {
      live = false;
    };
  }, [cropping.digest]); // eslint-disable-line react-hooks/exhaustive-deps

  const loaded = photo && photo !== "failed" ? photo : null;
  const ratio = loaded ? ratioOf(loaded.image, quad, aspect) : 1;
  const whole = isFace(quad);

  // The face straightened, small, redrawn as the corners move: once a frame at most.
  useEffect(() => {
    if (!loaded || !whole || !preview.current) return;
    const frame = requestAnimationFrame(() => {
      const [w, h] = cutSize(quad, loaded.small.width, loaded.small.height, ratio, 320);
      draw(straighten(loaded.small, quad, w, h), w, h, preview.current!);
    });
    return () => cancelAnimationFrame(frame);
  }, [loaded, quad, ratio, whole]);

  const place = (corner: number, [x, y]: Point) => {
    // A hand on a corner outranks an answer still on its way.
    asked.current++;
    if (finding === "looking") setFinding(null);
    setQuad((q) => q.map((p, i) => (i === corner ? [clamp(x), clamp(y)] : p)) as Quad);
  };

  // A tap on the photograph, not a drag: ask where the face is there.
  const stageEvents = {
    onPointerDown: (e: PointerEvent<HTMLDivElement>) => {
      if (e.target === e.currentTarget || e.target instanceof SVGElement || e.target instanceof HTMLImageElement) {
        tap.current = { x: e.clientX, y: e.clientY };
      }
    },
    onPointerUp: (e: PointerEvent<HTMLDivElement>) => {
      const down = tap.current;
      tap.current = null;
      const box = stage.current?.getBoundingClientRect();
      if (!down || !box || !loaded || Math.hypot(e.clientX - down.x, e.clientY - down.y) > 8) return;
      find(loaded.image, [clamp((e.clientX - box.left) / box.width), clamp((e.clientY - box.top) / box.height)]);
    },
  };

  const handle = (corner: number) => ({
    onPointerDown: (e: PointerEvent<HTMLButtonElement>) => {
      e.currentTarget.setPointerCapture(e.pointerId);
      drag.current = { corner, x: e.clientX, y: e.clientY, from: quad[corner]! };
    },
    onPointerMove: (e: PointerEvent<HTMLButtonElement>) => {
      const held = drag.current;
      const box = stage.current?.getBoundingClientRect();
      if (!held || held.corner !== corner || !box) return;
      place(corner, [held.from[0] + (e.clientX - held.x) / box.width, held.from[1] + (e.clientY - held.y) / box.height]);
    },
    onPointerUp: () => (drag.current = null),
    onPointerCancel: () => (drag.current = null),
    onKeyDown: (e: KeyboardEvent<HTMLButtonElement>) => {
      const step = e.shiftKey ? SHOVE : NUDGE;
      const by = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[e.key];
      if (!by) return;
      e.preventDefault();
      const [x, y] = quad[corner]!;
      place(corner, [x + by[0]!, y + by[1]!]);
    },
  });

  const save = () => {
    if (!loaded) return;
    const { image } = loaded;
    // Straightened from the photograph at full size, not from the preview's copy.
    void desk.cut(toCorners(quad), () => straightened(image, quad, ratio));
  };

  const [top, right] = [quad[0], quad[1]];
  return (
    <Dialog
      open
      onOpenChange={(open) => !open && !desk.busy && desk.uncrop()}
      width={960}
      title={`Crop the ${face}`}
      description="Tap the face, or drag the corners onto its corners. The thick edge is its top."
      footer={
        <>
          <Button onClick={desk.uncrop} disabled={desk.busy}>
            Cancel
          </Button>
          <Button
            onClick={() =>
              // Not a box face, or framed already: the photograph as it is, its own bytes.
              void desk.cut(WHOLE, async () => (await fetch(imageUrl(cropping.digest))).blob())
            }
            disabled={desk.busy}
          >
            Use as taken
          </Button>
          <Button variant="primary" onClick={save} disabled={!loaded || !whole || desk.busy}>
            {desk.busy ? "Saving…" : "Save"}
          </Button>
        </>
      }
    >
      {desk.problem && (
        <Alert tone="danger" onDismiss={desk.dismiss}>
          {desk.problem}
        </Alert>
      )}
      {photo === "failed" ? (
        <Faint>The photograph could not be loaded.</Faint>
      ) : (
        <div className={s.crop}>
          <div
            ref={stage}
            className={s.cropStage}
            style={{ "--photo": loaded ? loaded.image.naturalWidth / loaded.image.naturalHeight : 0.75 } as CSSProperties}
            {...stageEvents}
          >
            {loaded && <img className={s.cropPhoto} src={imageUrl(cropping.digest)} alt="" draggable={false} />}
            <svg className={s.cropMarks} viewBox="0 0 1 1" preserveAspectRatio="none" aria-hidden="true">
              <path className={s.cropShade} fillRule="evenodd" d={`M0 0H1V1H0Z M${quad.map(([x, y]) => `${x} ${y}`).join(" L")}Z`} />
              <polygon className={s.cropEdge} points={quad.map((p) => p.join(",")).join(" ")} />
              <line className={s.cropTop} x1={top[0]} y1={top[1]} x2={right[0]} y2={right[1]} />
            </svg>
            {quad.map(([x, y], i) => (
              <button
                key={i}
                type="button"
                className={s.corner}
                style={{ left: `${x * 100}%`, top: `${y * 100}%` }}
                aria-label={`${CORNERS[i]} corner of the ${face}`}
                {...handle(i)}
              />
            ))}
          </div>
          <div className={s.cropSide}>
            <canvas ref={preview} className={s.cropPreview} role="img" aria-label={`The ${face}, straightened`} />
            {!whole && <Faint>Those corners cross or fold in.</Faint>}
            <Button
              size="sm"
              icon={<RotateCw />}
              onClick={() => {
                asked.current++;
                setQuad(turn);
              }}
              disabled={desk.busy}
            >
              Turn
            </Button>
            {finding && (
              <p className={s.cropStatus} role="status">
                {FINDING[finding]}
              </p>
            )}

          </div>
        </div>
      )}
    </Dialog>
  );
}
