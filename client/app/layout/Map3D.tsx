import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Maximize, Minus, Plus } from "lucide-react";

import { IconButton } from "@ui/index";
import type { LayoutPlace, MapBin, PlanShape } from "@domain/types";

import { cellsOf, sceneOf } from "./blocks";
import type { Layer } from "./layers";
import { SiteScene } from "./scene3d";
import s from "./map.module.css";

/**
 * The bin map's 3D view (D208): the site as the Warehouse screen draws it, with
 * every bin a box in its cell, coloured by the layer. Clicking a bin chooses
 * it, and a search flies to it. Its own chunk, with three.js.
 */
export default function Map3D({
  plan,
  places,
  bins,
  layer,
  chosen,
  choose,
  flight,
}: {
  plan: PlanShape[];
  places: LayoutPlace[];
  bins: MapBin[];
  layer: Layer;
  chosen: string | null;
  choose: (locationId: string | null) => void;
  /** Bumped when the view should fly to what is chosen. */
  flight: number;
}) {
  const host = useRef<HTMLDivElement>(null);
  const tag = useRef<HTMLSpanElement>(null);
  const scene = useRef<SiteScene | null>(null);
  const [supported] = useState(SiteScene.supported);
  const [hovered, setHovered] = useState<string | null>(null);
  const site = useMemo(() => sceneOf(plan, places), [plan, places]);
  const cells = useMemo(() => cellsOf(plan, places, bins), [plan, places, bins]);
  const codes = useMemo(() => new Map(bins.map((b) => [b.location_id, b.code])), [bins]);

  // The label names the bin under the pointer, and otherwise the chosen one.
  const named = hovered ?? chosen;
  const latest = useRef({ choose, named });
  latest.current = { choose, named };

  const pin = () => {
    const el = tag.current;
    const id = latest.current.named;
    const top = id ? scene.current?.binTop(id) : null;
    if (!el || !top || !scene.current) return;
    const { x, y } = scene.current.project(top);
    el.style.transform = `translate(${x}px, ${y}px) translate(-50%, calc(-100% - var(--ui-space-2)))`;
  };

  useEffect(() => {
    if (!supported || !host.current) return;
    const made = new SiteScene(host.current, {
      choose: () => {},
      hover: () => {},
      drawn: pin,
      chooseBin: (id) => latest.current.choose(id),
      hoverBin: setHovered,
    });
    scene.current = made;
    return () => {
      made.dispose();
      scene.current = null;
    };
  }, [supported]);

  useEffect(() => scene.current?.set(site), [site]);
  useEffect(() => scene.current?.setBins(cells), [cells]);
  useEffect(() => scene.current?.setLayer(layer), [layer]);
  useEffect(() => scene.current?.chooseBin(chosen), [chosen, cells]);
  // Each flight once, as soon as its bin is drawn. A click chooses a bin
  // already in view, and a quiet refresh of the stock flies nowhere.
  const flown = useRef(0);
  useEffect(() => {
    if (flight <= flown.current || !chosen || !scene.current?.binTop(chosen)) return;
    flown.current = flight;
    scene.current.flyTo(chosen);
  }, [flight, chosen, cells]);
  useLayoutEffect(pin, [named]);

  if (!supported) {
    return <p className={s.note}>The bin map needs WebGL, which this browser has turned off.</p>;
  }

  return (
    <>
      <div ref={host} className={s.canvas} role="img" aria-label={chosen ? `The bin map, with ${codes.get(chosen) ?? "a bin"} chosen` : "The bin map"} />
      {named && codes.get(named) && (
        <span ref={tag} className={s.tag} aria-hidden>
          {codes.get(named)}
        </span>
      )}
      <div className={s.tools}>
        <IconButton size="sm" label="Zoom in" icon={<Plus />} onClick={() => scene.current?.zoomIn()} />
        <IconButton size="sm" label="Zoom out" icon={<Minus />} onClick={() => scene.current?.zoomOut()} />
        <IconButton size="sm" label="Show the whole site" icon={<Maximize />} onClick={() => scene.current?.reset()} />
      </div>
    </>
  );
}
