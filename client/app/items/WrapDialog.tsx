import { Check, ChevronLeft, ChevronRight } from "lucide-react";

import { Suspense, lazy, useCallback, useEffect, useState } from "react";

import { Alert, Button, Dialog, IconButton, Skeleton, Stack, Tabs } from "@ui/index";
import type { CaptureSubject, ItemView, Unwrapped } from "@domain/types";
import { Faint } from "@app/common/cells";

import { isRound } from "./box";
import { handheld } from "./crop";
import { roundSize } from "./round";
import { nameOf, photosOf } from "./subjects";
import { useWraps, type WrapDesk, type Wrapping } from "./useWrapQueue";
import s from "./items.module.css";

// three.js is its own chunk, fetched the first time a tub is shown.
const RoundView = lazy(() => import("./RoundView"));

/** Under this, how well the shape fitted a photograph is worth a look before saving. */
const FITS = 0.9;
/** Turned more than this between neighbouring photographs, the side between them was seen only edge-on. */
const TURNED = 100;
/** Its side's photographs, as the wrapping names them. */
const SIDE_FACES = ["front", "right", "back", "left"];

/** What to look at in a wrapping made (D240): how well it fitted each photo, a turn too far, and what went wrong. */
export function WrapNotes({ w }: { w: Wrapping }) {
  if (w.state !== "made" || !w.made) return null;
  const unsure = w.made.sides.filter((p) => p.fit < FITS);
  const far = w.made.turned.filter((t) => t.degrees > TURNED);
  return (
    <>
      {unsure.length > 0 ? (
        <span className={s.cropLieOut}>
          The shape fits the {unsure.map((p) => p.face).join(" and ")} {unsure.length === 1 ? "photo" : "photos"} loosely: check the label lines up
        </span>
      ) : (
        <Faint>Fitted to every photo: check the label reads right round it</Faint>
      )}
      {far.map((t) => (
        <span key={t.from} className={s.cropLieOut}>
          Turned {t.degrees}° from the {t.from} photo to the {t.to}, so the side between them was only seen edge-on: retake one a quarter turn from the other
        </span>
      ))}
      {w.step && <span className={s.cropLieOut}>{w.step}</span>}
    </>
  );
}

/** A bucket with no lid is open: its top photo looks into it (D241). Said before it is made, or after. */
export function WrapTop({ w, desk }: { w: Wrapping; desk: WrapDesk }) {
  if (w.state === "saving") return null;
  return (
    <Tabs
      aria-label="Its top"
      value={w.open ? "open" : "lid"}
      onValueChange={(v) => desk.setOpen(w.key, v === "open")}
      items={[
        { value: "lid", label: "Has a lid" },
        { value: "open", label: "Open at the top" },
      ]}
    />
  );
}

/** Where a photograph's middle falls across the unwrapped side, which runs once round from the back with the front in the middle. */
const across = (turn: number) => {
  const at = (((turn + Math.PI) / (2 * Math.PI)) % 1 + 1) % 1;
  return Math.min(Math.max(at, 0.07), 0.93) * 100;
};

/**
 * A round thing's wrapping, made and turned by hand (D240). The tub as it
 * would look, its side unwrapped under it, and under that each photo of its
 * side where its middle falls, to turn a degree at a time left or right till
 * the seams on either side of it line up. Saved, its side is stitched again at
 * full size at the turns it was left at.
 */
export function WrapDialog({ w, desk, onClose }: { w: Wrapping; desk: WrapDesk; onClose: () => void }) {
  const e = w.entry;
  const saving = w.state === "saving";
  const made = (w.state === "made" || saving) && w.made && w.preview ? { made: w.made, preview: w.preview } : null;
  // Kept, it is done.
  useEffect(() => {
    if (w.state === "saved") onClose();
  }, [w.state, onClose]);

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && !saving && onClose()}
      width={760}
      title={`Wrap ${e.code} in its photos`}
      description="Each photo of its side is wrapped round it where it was taken. Where a seam does not line up, turn the photo on either side of it a degree at a time, and watch the seam."
      footer={
        <>
          <Button onClick={onClose} disabled={saving}>
            Close
          </Button>
          <Button onClick={() => desk.again(w.key)} disabled={saving || (w.state !== "made" && w.state !== "failed")}>
            Make again
          </Button>
          <Button variant="primary" icon={<Check />} loading={saving} disabled={!made || w.stitching} onClick={() => void desk.save(w.key)}>
            Save
          </Button>
        </>
      }
    >
      <Stack gap={3}>
        {w.state === "failed" && <Alert tone="danger">{w.step}</Alert>}
        {(w.state === "waiting" || w.state === "making") && (
          <Stack gap={2}>
            <Faint>{w.step || "Waiting…"}</Faint>
            <Skeleton width="100%" />
          </Stack>
        )}
        {made && (
          <>
            <div className={s.wrapLook}>
              <Suspense fallback={<div className={s.box} />}>
                <RoundView size={made.made.size} wrap={made.preview} label={e.code} urlOf={(u) => u} />
              </Suspense>
            </div>
            <figure className={s.wrapStrip} data-stitching={w.stitching || undefined}>
              <img className={s.wrapSide} src={made.preview.side} alt={`${e.code}, its side unwrapped`} />
              <div className={s.wrapTurns}>
                {made.made.sides.map((p, k) => {
                  const name = p.face.charAt(0).toUpperCase() + p.face.slice(1);
                  return (
                    <span key={p.face} className={s.wrapTurn} style={{ left: `${across(made.made.turns[k]!)}%` }}>
                      <IconButton size="sm" label={`Turn the ${p.face} photo left`} icon={<ChevronLeft />} disabled={saving} onClick={() => desk.nudge(w.key, k, -1)} />
                      {name}
                      <IconButton size="sm" label={`Turn the ${p.face} photo right`} icon={<ChevronRight />} disabled={saving} onClick={() => desk.nudge(w.key, k, 1)} />
                    </span>
                  );
                })}
              </div>
            </figure>
            {w.stitching ? <Faint>Stitching…</Faint> : <WrapNotes w={w} />}
          </>
        )}
        <WrapTop w={w} desk={desk} />
      </Stack>
    </Dialog>
  );
}

/** Whether a round thing can be wrapped again from its page (D240): measured round, two sides of it photographed, at a computer. */
export function wrappable(item: ItemView, subject: CaptureSubject): boolean {
  if (handheld() || !isRound(subject) || !roundSize(subject)) return false;
  const photos = photosOf(item, subject);
  return SIDE_FACES.filter((f) => photos.has(f)).length >= 2;
}

/** A round thing wrapped again from its page: made from its photographs as soon as it opens (D240). */
export function WrapAgain({ item, subject, onClose, onSaved }: { item: ItemView; subject: CaptureSubject; onClose: () => void; onSaved: () => void }) {
  const [entry] = useState<Unwrapped>(() => ({
    item_id: subject.item_id,
    item_style_id: subject.item_style_id,
    lot_id: subject.lot_id,
    item_part_id: subject.item_part_id,
    packaging_level: subject.packaging_level as Unwrapped["packaging_level"],
    open_item: item.item_id,
    code: `${item.code} · ${nameOf(subject, item)}`,
    description: subject.description ?? item.description,
  }));
  const desk = useWraps(useCallback(() => Promise.resolve([entry]), [entry]));
  const w = desk.wraps[0];
  const close = useCallback(() => {
    if (w?.state === "saved") onSaved();
    onClose();
  }, [w?.state, onSaved, onClose]);
  if (!w) return null;
  return <WrapDialog w={w} desk={desk} onClose={close} />;
}
