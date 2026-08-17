import { cx } from "../cx";
import styles from "../materials/face.module.css";

/**
 * A container with nothing in it is still a container.
 *
 * **Absence was being drawn as absence.** An empty carton was a line of faint
 * text in a tall dark gap, so the fullest state on the screen and the emptiest
 * one occupied the same amount of nothing — and an empty box on a bench does
 * not read as nothing. It reads as a box, waiting.
 *
 * So the box is drawn: an isometric wireframe at hairline weight, with the
 * occluded edges dashed the way an engineering drawing dashes them. When the
 * preset states a size the wireframe is drawn **to those proportions**, so a
 * pallet is squat and wide and a small box is not — which makes this ornament
 * that measures something rather than a picture of a generic carton.
 */
export function EmptySlot({
  label,
  note,
  size,
}: {
  label: string;
  note?: string;
  /** Millimetres from the preset. Proportions only — nothing here is to scale
   *  against anything else on the page. */
  size?: { length_mm: number; width_mm: number; height_mm: number };
}) {
  return (
    <div data-layer="instrument" className={styles.empty}>
      <Ghost {...(size ? { size } : {})} />
      <span className={styles.emptyText}>
        <span className={styles.emptyLabel}>{label}</span>
        {note && <span className={styles.emptyNote}>{note}</span>}
      </span>
    </div>
  );
}

/** Isometric projection: x goes right-down, y left-down, z up. */
const COS30 = Math.cos(Math.PI / 6);
const SIN30 = 0.5;

function Ghost({
  size = { length_mm: 400, width_mm: 400, height_mm: 320 },
}: {
  size?: { length_mm: number; width_mm: number; height_mm: number };
}) {
  // Normalise so the largest dimension fills the frame, keeping the ratios the
  // preset actually states.
  const largest = Math.max(size.length_mm, size.width_mm, size.height_mm) || 1;
  const w = (size.length_mm / largest) * 10;
  const d = (size.width_mm / largest) * 10;
  // Height is eased rather than scaled straight. A skid is 150mm tall on a
  // 1165mm footprint, which in true proportion draws a flat diamond nobody
  // reads as a container — the drawing has to stay a box to mean anything.
  // The exponent keeps the ordering (a pallet is still visibly squatter than
  // a carton) while lifting the extreme back into legibility.
  const h = Math.pow(size.height_mm / largest, 0.55) * 10;

  const s = 4.2;
  const at = (x: number, y: number, z: number): [number, number] => [
    44 + (x - y) * COS30 * s,
    40 + ((x + y) * SIN30 - z) * s,
  ];

  const b0 = at(0, 0, 0);
  const b1 = at(w, 0, 0);
  const b2 = at(w, d, 0);
  const b3 = at(0, d, 0);
  const t0 = at(0, 0, h);
  const t1 = at(w, 0, h);
  const t2 = at(w, d, h);
  const t3 = at(0, d, h);

  const line = (a: [number, number], b: [number, number]) =>
    `M${a[0].toFixed(1)} ${a[1].toFixed(1)}L${b[0].toFixed(1)} ${b[1].toFixed(1)}`;

  // Everything but the far bottom corner, which is occluded.
  const solid = [
    line(b1, b2),
    line(b2, b3),
    line(t0, t1),
    line(t1, t2),
    line(t2, t3),
    line(t3, t0),
    line(b1, t1),
    line(b2, t2),
    line(b3, t3),
  ].join("");

  const hidden = [line(b0, b1), line(b0, b3), line(b0, t0)].join("");

  return (
    <svg
      className={styles.emptyGhost}
      viewBox="0 0 88 80"
      role="img"
      aria-label="An empty container"
    >
      <path d={hidden} className={cx(styles.ghostLine, styles.ghostHidden)} />
      <path d={solid} className={styles.ghostLine} />
    </svg>
  );
}
