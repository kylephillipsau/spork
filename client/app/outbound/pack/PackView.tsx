import { useEffect, useRef, useState } from "react";

import { imageUrl } from "@domain/api";

import type { Dims, Layer } from "./arrange";
import { PackScene } from "./pack3d";
import s from "./pack-bench.module.css";

/**
 * A suggested arrangement in 3D (D195), built up to `upTo` layers and turned by
 * dragging. Its own chunk with three.js, fetched the first time it is shown.
 * Without WebGL it says so: the layer plan beside it says the same thing flat.
 */
export default function PackView({ size, layers, upTo, label }: { size: Dims; layers: Layer[]; upTo: number; label: string }) {
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
    scene.current?.set(size, layers);
    scene.current?.upTo(upTo);
  }, [size[0], size[1], size[2], layers]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => scene.current?.upTo(upTo), [upTo]);

  if (!supported) return <p className={s.aside}>This browser can't draw in 3D. The layers show the same arrangement from above.</p>;
  return <div ref={host} className={s.view3d} role="img" aria-label={label} />;
}
