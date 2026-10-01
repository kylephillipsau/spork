import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Maximize, Minus, Plus } from "lucide-react";

import { IconButton } from "@ui/index";
import type { LayoutPlace, PlanShape } from "@domain/types";

import { SiteScene } from "./scene3d";
import { sceneOf } from "./blocks";
import s from "./layout.module.css";

/**
 * The site in 3D, beside its plan (D173). It confirms what the plan says, the
 * way a glance across the floor would: solid places stand up, walk-through
 * places lie flat, a rack shows its bays and levels. Choosing a place here is
 * choosing it on the plan and the list.
 *
 * Turned by dragging, moved by dragging with the middle button or with Ctrl or
 * ⌘ held, and zoomed by scrolling or pinching. The buttons zoom and go back to the whole site for
 * anyone who would rather press. Nothing is edited in 3D.
 *
 * Its own chunk: three.js loads when the pane is first shown, never before.
 */
export default function Site3D({
  plan,
  places,
  chosen,
  choose,
}: {
  plan: PlanShape[];
  places: LayoutPlace[];
  chosen: string | null;
  choose: (placeId: string) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const tag = useRef<HTMLSpanElement>(null);
  const scene = useRef<SiteScene | null>(null);
  const [supported] = useState(SiteScene.supported);
  const [hovered, setHovered] = useState<string | null>(null);
  const site = useMemo(() => sceneOf(plan, places), [plan, places]);

  // The label follows the pointer, and otherwise names what is chosen.
  const named = site.blocks.find((b) => b.target && b.place_id === (hovered ?? chosen)) ?? null;
  const marked = site.blocks.find((b) => b.target && b.place_id === chosen) ?? null;

  const latest = useRef({ choose, named });
  latest.current = { choose, named };

  // Pinned to the top of the place it names, frame by frame, without React.
  const pin = () => {
    const el = tag.current;
    const b = latest.current.named;
    if (!el || !b || !scene.current) return;
    const { x, y } = scene.current.project(b.top);
    el.style.transform = `translate(${x}px, ${y}px) translate(-50%, calc(-100% - var(--ui-space-2)))`;
  };

  useEffect(() => {
    if (!supported || !host.current) return;
    const made = new SiteScene(host.current, {
      choose: (id) => latest.current.choose(id),
      hover: setHovered,
      drawn: pin,
    });
    scene.current = made;
    return () => {
      made.dispose();
      scene.current = null;
    };
  }, [supported]);

  useEffect(() => scene.current?.set(site), [site]);
  useEffect(() => scene.current?.choose(chosen), [chosen]);
  useLayoutEffect(pin, [named]);

  if (!supported) {
    return (
      <div className={s.scene}>
        <p className={s.sceneNote}>The 3D view needs WebGL, which this browser has turned off.</p>
      </div>
    );
  }

  const coarse = matchMedia("(pointer: coarse)").matches;
  const modifier = /Mac|iPhone|iPad/.test(navigator.userAgent) ? "⌘" : "Ctrl";

  return (
    <div className={s.scene}>
      <div
        ref={host}
        className={s.canvas}
        role="img"
        aria-label={marked ? `The site in 3D, with ${marked.name} marked` : "The site in 3D"}
      />
      {named && (
        <span ref={tag} className={s.tag} aria-hidden>
          {named.name}
        </span>
      )}
      <div className={s.sceneTools}>
        <IconButton size="sm" label="Zoom in" icon={<Plus />} onClick={() => scene.current?.zoomIn()} />
        <IconButton size="sm" label="Zoom out" icon={<Minus />} onClick={() => scene.current?.zoomOut()} />
        <IconButton size="sm" label="Show the whole site" icon={<Maximize />} onClick={() => scene.current?.reset()} />
      </div>
      <p className={s.sceneHint}>
        {coarse ? "Drag to turn, pinch to zoom and move" : `Drag to turn · ${modifier}-drag to move · scroll to zoom`}
      </p>
    </div>
  );
}
