import { useState } from "react";

import styles from "../materials/face.module.css";

/**
 * A photograph of the thing, and whose photograph it is.
 *
 * **The label is not decoration.** D141 lets a picture inherit from the item's
 * style so a picker looking for size 10 gets the size 8 carton rather than
 * nothing, and the whole of what makes that honest is saying so. An inherited
 * picture drawn unlabelled claims to be a photograph of the code in front of
 * you when it is a photograph of a different one — the sentence D108 earned
 * about figures, about pixels this time.
 *
 * So `source` is required, not optional, and there is no way to render one of
 * these without it.
 *
 * **The bytes come from the API, not from a CDN.** `GET /images/{digest}` runs
 * inside the tenant scope, so the browser's session cookie is what authorises
 * the pixels and a content address is not a password. Same-origin, per D130,
 * which is what lets an `<img>` tag carry the credential without a fetch.
 */
export function Photo({
  src,
  source,
  alt,
  size = "thumb",
}: {
  /**
   * Where the bytes are.
   *
   * **A URL rather than a digest**, because the design package does not know
   * that an API exists — nothing under `design/` imports from `domain/`, and
   * this primitive was the one place that knew an endpoint's shape. It built
   * `/images/{digest}` itself, so moving the API under a prefix would have
   * needed the prefix written down twice. The caller knows; this draws.
   */
  src: string;
  /** `own` or `style`. */
  source: string;
  alt: string;
  size?: "thumb" | "plate";
}) {
  // **A missing file is a state, not a fault.** D132 keeps the bytes on a
  // volume and the rows in the database, and says plainly what that costs: a
  // restore of one without the other leaves rows addressing absent files, and
  // the read answers 404 rather than 500 because it is a real state. The screen
  // has to agree — a browser's broken-image glyph with the alt text spilling
  // out of the frame is the worst of both, and on a handheld held at arm's
  // length it reads as the software being broken rather than a picture being
  // gone.
  const [missing, setMissing] = useState(false);
  if (missing) return <NoPhoto size={size} />;

  return (
    <span
      data-layer="face"
      className={size === "plate" ? styles.photoPlate : styles.photo}
    >
      <img
        src={src}
        alt={alt}
        loading="lazy"
        onError={() => setMissing(true)}
      />
      {/* Zero ceremony when it is this code's own picture: the ordinary case
          says nothing, and only the borrowed one announces itself. Same rule
          the capture worklist follows for an inherited measurement. */}
      {source !== "own" && <span className={styles.photoFrom}>{source}</span>}
    </span>
  );
}

/**
 * The space a photograph would occupy, when there is none.
 *
 * **Drawn rather than collapsed**, on `EmptySlot`'s argument: a row with no
 * picture and a row with one must line up, or the list becomes a ragged thing
 * to read at arm's length. It also makes *nobody has photographed this* visible
 * work rather than an absence nobody notices.
 */
export function NoPhoto({ size = "thumb" }: { size?: "thumb" | "plate" }) {
  return (
    <span
      data-layer="face"
      className={size === "plate" ? styles.photoPlate : styles.photo}
      aria-hidden="true"
    >
      <span className={styles.photoAbsent} />
    </span>
  );
}
