import { useEffect, useRef, useState } from "react";

import { imageUrl } from "@domain/api";
import { Thumb } from "@app/common/Thumb";

import { BOX_FACES, type BoxFace } from "./box";
import { BoxScene } from "./box3d";
import s from "./items.module.css";

/**
 * An item as a box made of its photographs, turned by dragging (D174). It
 * turns to show `facing` when that changes, which is the side just taken.
 *
 * Its own chunk with three.js, fetched the first time a box is shown. Without
 * WebGL it shows the sides as tiles instead.
 */
export default function BoxView({
  faces,
  size,
  facing,
  label,
}: {
  faces: Partial<Record<BoxFace, string>>;
  size: [number, number, number] | null;
  facing: BoxFace | null;
  label: string;
}) {
  const host = useRef<HTMLDivElement>(null);
  const scene = useRef<BoxScene | null>(null);
  const [supported] = useState(BoxScene.supported);
  const key = JSON.stringify([faces, size]);

  useEffect(() => {
    if (!supported || !host.current) return;
    const made = new BoxScene(host.current, imageUrl);
    scene.current = made;
    return () => {
      made.dispose();
      scene.current = null;
    };
  }, [supported]);

  useEffect(() => scene.current?.set(faces, size), [key]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (facing) scene.current?.show(facing);
  }, [facing]);

  const taken = BOX_FACES.filter((f) => faces[f]).length;
  if (!supported) {
    return (
      <ul className={s.faces} aria-label={label}>
        {BOX_FACES.filter((f) => faces[f]).map((f) => (
          <li key={f} className={s.faceTile}>
            <Thumb picture={{ digest: faces[f]!, source: "own" }} alt={`${label}, ${f}`} />
            <span className={s.faceName}>{f}</span>
          </li>
        ))}
      </ul>
    );
  }
  return (
    <figure className={s.boxFigure}>
      <div ref={host} className={s.box} role="img" aria-label={`${label} as a box, ${taken} of 6 sides photographed`} />
      <figcaption className={s.boxCaption}>Drag to turn it</figcaption>
    </figure>
  );
}
