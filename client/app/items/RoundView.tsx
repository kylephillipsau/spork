import { useEffect, useRef, useState } from "react";

import { imageUrl } from "@domain/api";
import type { Wrap } from "@domain/types";

import type { RoundSize } from "./round";
import { RoundScene } from "./round3d";
import s from "./items.module.css";

/**
 * A round thing as the tub it is, its photographs wrapped round it, turned by
 * dragging (D240): the item page's box view (`BoxView`) for a bucket. Its own
 * chunk with three.js, fetched the first time one is shown. Without WebGL it
 * shows the unwrapped side instead.
 */
export default function RoundView({
  size,
  wrap,
  label,
  urlOf = imageUrl,
}: {
  size: RoundSize;
  wrap: Wrap | null;
  label: string;
  /** Where a picture is: a kept one by its content address, or one made and not yet kept. */
  urlOf?: ((digest: string) => string) | undefined;
}) {
  const host = useRef<HTMLDivElement>(null);
  const scene = useRef<RoundScene | null>(null);
  const [supported] = useState(RoundScene.supported);
  const key = JSON.stringify([size, wrap]);

  useEffect(() => {
    if (!supported || !host.current) return;
    const made = new RoundScene(host.current, urlOf);
    scene.current = made;
    return () => {
      made.dispose();
      scene.current = null;
    };
  }, [supported]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => scene.current?.set(size, wrap), [key]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!supported) {
    return wrap ? <img className={s.wrapSide} src={urlOf(wrap.side)} alt={`${label}, its side unwrapped`} /> : null;
  }
  return (
    <figure className={s.boxFigure}>
      <div ref={host} className={s.box} role="img" aria-label={`${label} as its tub, its photographs round it`} />
      <figcaption className={s.boxCaption}>Drag to turn it</figcaption>
    </figure>
  );
}
