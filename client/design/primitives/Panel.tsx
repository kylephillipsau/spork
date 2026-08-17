import type { ReactNode } from "react";
import { cx, type Elevation } from "../cx";
import styles from "../materials/anodise.module.css";

/**
 * L1 — an anodised panel.
 *
 * Takes no `className` and no `style` (D123). There is no subset of the
 * anodise recipe that is still anodised aluminium, so the only variation a
 * caller gets is what the system says is variable: how far off the ground
 * it sits, and whether it carries the relief groove.
 */
export function Panel({
  elevation = "raised",
  frame = "flush",
  as: Tag = "div",
  children,
}: {
  elevation?: Elevation;
  /**
   * How much metal shows around what the panel holds.
   *
   * `flush` — the content covers the panel. For anything that draws its own
   * edge and wants no metal around it.
   *
   * `bezel` — a margin of panel around the content. **What a face wants.** An
   * instrument face is seated *into* metal, and its reveal and bezel catch are
   * drawing that seating; with no margin they land on the panel's own chamfer
   * and read as three concentric lines.
   *
   * The relief groove that `engraved` used to add is gone: it was a real
   * machined detail and the most literal piece of hardware in the system, and
   * the edge below carries the light it was carrying.
   */
  frame?: "flush" | "bezel";
  as?: "div" | "section" | "article" | "header" | "aside";
  children?: ReactNode;
}) {
  return (
    <Tag
      data-layer="panel"
      data-material="anodise"
      className={cx(styles.panel, styles[elevation], frame === "bezel" && styles.bezel)}
    >
      {/* Always. The edge is the panel's outline, not an option. */}
      <span className={styles.edge} aria-hidden="true" />
      {children}
    </Tag>
  );
}

/**
 * L2 — a machined pocket. The chamfer inverts: light beneath, shadow
 * above. This is the well cut into the chassis; the one cut into an
 * instrument face is `FaceWell`, and on the night face it lifts rather
 * than sinks because a recess in an already-dark field is invisible.
 */
export function Well({ children }: { children?: ReactNode }) {
  return (
    <div data-layer="well" className={styles.well}>
      {children}
    </div>
  );
}
