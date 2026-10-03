import { useEffect, useRef, useState } from "react";

import { imageUrl } from "@domain/api";

import { PackScene, type ParcelShape } from "./pack3d";
import s from "./pack-bench.module.css";

/**
 * A suggested arrangement in 3D (D195), built up to `upTo` layers and turned by
 * dragging; or the whole order's parcels side by side (D202). Its own chunk
 * with three.js, fetched the first time it is shown. Without WebGL it says so:
 * the plan and the list beside it say the same thing flat.
 */
export default function PackView({ groups, upTo, label }: { groups: ParcelShape[]; upTo: number; label: string }) {
  const host = useRef<HTMLDivElement>(null);
  const scene = useRef<PackScene | null>(null);
  const [supported] = useState(PackScene.supported);

  useEffect(() => {
    if (!supported || !host.current) return;
    const made = new PackScene(host.current, imageUrl);
    scene.current = made;
    return () => {
      made.dispose();
      scene.current = null;
    };
  }, [supported]);

  useEffect(() => {
    scene.current?.show(groups);
    scene.current?.upTo(upTo);
  }, [groups]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => scene.current?.upTo(upTo), [upTo]);

  if (!supported) return <p className={s.aside}>This browser can't draw in 3D. The list and the layers say the same thing flat.</p>;
  return <div ref={host} className={s.view3d} role="img" aria-label={label} />;
}
