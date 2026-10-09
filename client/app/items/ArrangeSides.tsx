import { useState, type CSSProperties, type DragEvent } from "react";

import { Alert, Button, Dialog } from "@ui/index";
import { useWriting } from "@app/acting";
import { partOf } from "@domain/acts";
import { api, imageUrl } from "@domain/api";
import type { CaptureSubject, ItemView, SubjectPhoto } from "@domain/types";
import { Faint } from "@app/common/cells";

import { boxSize, faceName } from "./box";
import { nameOf, photosOf, shown, type Face } from "./subjects";
import s from "./items.module.css";

/**
 * A box's sides laid out flat, its photographs on them, to put right (D243).
 *
 * The measured box unfolded: its top above its front, its left, front, right
 * and back in a row, its bottom below; each face drawn to its measured shape
 * with its photograph on it. **Turn the box** until the photographs sit their
 * faces, when it was measured lying another way from how it was
 * photographed; or **drag a photograph onto the side it is of**, swapping
 * the two, when they were taken in another order. A press on one and then
 * another swaps them too.
 *
 * Saved together: the turn as a correction of its figures, as of when they
 * were measured (D236); each photograph that changed side filed again under
 * it (`POST /observation-images/{id}/face`), to be cut again to its shape.
 */

/** The faces unfolded: where each sits in the net, by row and column. */
const NET: { face: Face; row: number; col: number }[] = [
  { face: "top", row: 1, col: 2 },
  { face: "left", row: 2, col: 1 },
  { face: "front", row: 2, col: 2 },
  { face: "right", row: 2, col: 3 },
  { face: "back", row: 2, col: 4 },
  { face: "bottom", row: 3, col: 2 },
];
/** What is photographed beside the six sides. */
const EXTRAS: Face[] = ["label", "detail"];
/** How wide the net is drawn, in pixels. */
const ACROSS = 560;

type Size = [number, number, number];
/** The three ways to turn it: which two of length, width and height trade places. */
const TURNS: { label: string; swap: [0 | 1 | 2, 0 | 1 | 2] }[] = [
  { label: "Tip it forward", swap: [1, 2] },
  { label: "Stand it on its end", swap: [0, 2] },
  { label: "Turn it round", swap: [0, 1] },
];
const METRICS = ["length", "width", "height"] as const;

export function ArrangeSides({
  item,
  subject,
  onClose,
  onSaved,
}: {
  item: ItemView;
  subject: CaptureSubject;
  onClose: () => void;
  onSaved: () => void;
}) {
  const taken = photosOf(item, subject);
  const [placed, setPlaced] = useState<Partial<Record<Face, SubjectPhoto>>>(() => Object.fromEntries(taken));
  const measured = boxSize(subject);
  const [size, setSize] = useState<Size | null>(measured);
  const [picked, setPicked] = useState<Face | null>(null);
  const { busy, problem, dismiss, press } = useWriting();

  const swap = (a: Face, b: Face) => {
    if (a === b) return;
    setPlaced((was) => ({ ...was, [a]: was[b], [b]: was[a] }));
    setPicked(null);
  };
  const turn = ([i, j]: [number, number]) =>
    setSize((was) => {
      if (!was) return was;
      const next = [...was] as Size;
      [next[i], next[j]] = [next[j]!, next[i]!];
      return next;
    });
  // Each photograph that is now on another side than it was filed as.
  const moved = Object.entries(placed).flatMap(([face, photo]) => (photo && photo.face !== face ? [{ photo, face: face as Face }] : []));
  const turned = size && measured ? METRICS.flatMap((m, k) => (size[k] !== measured[k] ? [{ metric: m, mm: size[k]! }] : [])) : [];
  const changed = moved.length > 0 || turned.length > 0;

  const save = () =>
    press(`arrange:${subject.item_id}:${subject.packaging_level}:${JSON.stringify(turned)}:${moved.map((m) => `${m.photo.image_id}=${m.face}`).join(",")}`, async (act) => {
      if (turned.length && subject.item_id && subject.packaging_level) {
        await api.correctFigures(
          subject.item_id,
          subject.packaging_level,
          turned.map((t) => ({ metric: t.metric, entered_value: (t.mm / 10).toFixed(1), unit: "cm" })),
          partOf(act, "turn"),
        );
      }
      for (const m of moved) await api.fileAsFace(m.photo.image_id, m.face, partOf(act, `face:${m.photo.image_id}`));
      onSaved();
      onClose();
    });

  // The net to its measured shape: length across the front and back, width
  // across the ends and down the top, height up the sides.
  const [l, w, h] = size ?? [1, 1, 1];
  const k = ACROSS / (2 * l + 2 * w);
  const tile = (face: Face, style: CSSProperties) => {
    const photo = placed[face];
    const name = faceName(face, subject);
    return (
      <div
        key={face}
        className={s.netFace}
        data-picked={picked === face || undefined}
        style={style}
        role="button"
        tabIndex={0}
        aria-label={photo ? `${name}: the photo taken as the ${faceName(photo.face, subject).toLowerCase()}` : `${name}: no photo`}
        draggable={photo !== undefined}
        onDragStart={(e: DragEvent) => e.dataTransfer.setData("text/plain", face)}
        onDragOver={(e: DragEvent) => e.preventDefault()}
        onDrop={(e: DragEvent) => {
          e.preventDefault();
          const from = e.dataTransfer.getData("text/plain") as Face;
          if (from) swap(from, face);
        }}
        onClick={() => (picked ? swap(picked, face) : photo && setPicked(face))}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            if (picked) swap(picked, face);
            else if (photo) setPicked(face);
          }
        }}
      >
        {photo && <img src={imageUrl(shown(photo))} alt="" draggable={false} />}
        <span className={s.netName}>
          {name}
          {photo && photo.face !== face && <Faint> · was {faceName(photo.face, subject).toLowerCase()}</Faint>}
        </span>
      </div>
    );
  };

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && !busy && onClose()}
      width={640}
      title={`Arrange the sides of ${item.code}`}
      description={`${nameOf(subject, item)}: each side drawn to its measured shape with the photo filed as it. Turn the box till the photos sit their sides, or drag a photo onto the side it is of.`}
      footer={
        <>
          <Button onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button variant="primary" loading={busy} disabled={!changed} onClick={() => void save()}>
            Save
          </Button>
        </>
      }
    >
      {problem && (
        <Alert tone="danger" onDismiss={dismiss}>
          {problem}
        </Alert>
      )}
      {size && (
        <div className={s.netTurns}>
          {TURNS.map((t) => (
            <Button key={t.label} size="sm" onClick={() => turn(t.swap)} disabled={busy}>
              {t.label}
            </Button>
          ))}
          <Faint>
            {size.map((v) => (v / 10).toFixed(1)).join(" × ")} cm
            {turned.length > 0 && measured && ` (measured ${measured.map((v) => (v / 10).toFixed(1)).join(" × ")})`}
          </Faint>
        </div>
      )}
      <div
        className={s.net}
        style={{
          gridTemplateColumns: `${w * k}px ${l * k}px ${w * k}px ${l * k}px`,
          gridTemplateRows: `${w * k}px ${h * k}px ${w * k}px`,
        }}
      >
        {NET.map((n) => tile(n.face, { gridRow: n.row, gridColumn: n.col }))}
      </div>
      <div className={s.netExtras}>{EXTRAS.map((face) => tile(face, {}))}</div>
      <p className={s.note}>
        {changed ? whatChanges(turned.length > 0, moved.length, Object.values(placed).some((p) => p?.cut)) : "Nothing changed yet."}
      </p>
    </Dialog>
  );
}

/** What saving does, in words. */
function whatChanges(turned: boolean, moved: number, cut: boolean): string {
  const parts = [turned && "Its figures are turned", moved > 0 && `${moved} ${moved === 1 ? "photo is" : "photos are"} filed under another side`].filter(Boolean);
  const after = [
    moved > 0 && "A photo that changes side is cut again under Photos to crop.",
    turned && cut && "A side already cut keeps its cut: crop it again from the card if it looks stretched.",
  ].filter(Boolean);
  return `${parts.join(", and ")}. ${after.join(" ")}`.trim();
}
