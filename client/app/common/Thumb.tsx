import { useState } from "react";
import { ImageOff } from "lucide-react";

import { imageUrl } from "@domain/api";
import type { Picture } from "@domain/types";

import s from "./thumb.module.css";

/**
 * An item's photograph on a work row, or the tile where one would be.
 *
 * A missing file is a state, not a fault (D132): a restore can leave rows
 * addressing absent files and the read answers 404, so the row shows the empty
 * tile rather than a broken image. A borrowed picture says where it came from
 * (D141), because drawn unlabelled it claims to be a photograph of this code.
 * The tile is drawn even with no picture, so rows line up.
 */
export function Thumb({ picture, alt }: { picture: Picture | null; alt: string }) {
  const [missing, setMissing] = useState(false);
  if (!picture || missing) {
    return (
      <span className={s.thumb} role="img" aria-label="No photo">
        <ImageOff aria-hidden />
      </span>
    );
  }
  return (
    <span className={s.thumb}>
      <img src={imageUrl(picture.digest)} alt={alt} loading="lazy" onError={() => setMissing(true)} />
      {picture.source !== "own" && <span className={s.from}>{picture.source === "netsuite" ? "NetSuite" : picture.source}</span>}
    </span>
  );
}
