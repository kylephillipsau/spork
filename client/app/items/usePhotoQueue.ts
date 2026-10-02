import { useCallback, useEffect, useRef, useState } from "react";

import { useLive, useWriting } from "@app/acting";
import { partOf } from "@domain/acts";
import { api, reason } from "@domain/api";
import type { CaptureSubject, ItemView, UncutPhoto, Uuid } from "@domain/types";

import { faceName, measuredAspect } from "./box";
import { handheld, loadPhoto, pixelsOf, ratioOf, straightened } from "./crop";
import { fromCorners, isFace } from "./cut";
import { SAM_SIZE } from "./faceFind";
import type { CropDesk } from "./FaceCrop";
import { subjectKey } from "./subjects";

/**
 * The photographs waiting to be cut, worked through at a computer (D181).
 *
 * A phone takes the photographs and has not the memory to find their faces.
 * This finds each one, a photograph at a time, the oldest first, and keeps
 * the corners it found; the person looks down the results, leaves ticked
 * what is right, and saves them together. One that is wrong, or where no face
 * was found, opens in the crop screen to be put right. **Nothing is kept
 * until the person saves** (D177): a cut is their judgement, and an act of
 * theirs.
 */

const model = () => import("./faceModel");

export type QueueState = "waiting" | "finding" | "found" | "missed" | "failed" | "saved";

export interface Queued {
  photo: UncutPhoto;
  /** What it is a photograph of, from its item's page; null until read, or when the page does not show it. */
  subject: CaptureSubject | null;
  /** The face, as the crop screen names it. */
  name: string;
  /** Its width over its height, when its size is measured. */
  aspect: number | null;
  /** Where the face-finder put its corners; null until found, or when it found none. */
  corners: number[] | null;
  state: QueueState;
  /** Left ticked, it is saved with the rest. */
  ticked: boolean;
}

export type QueueRead = { kind: "loading" } | { kind: "ready" } | { kind: "failed"; message: string };

export interface QueueDesk {
  read: QueueRead;
  queued: Queued[];
  /** A phone: nothing is found here, and the screen says to open it at a computer. */
  phone: boolean;
  tick: (image: Uuid, on: boolean) => void;
  /** Save every ticked photograph's cut, one after another. */
  save: () => Promise<void>;
  /** How far the saving has got, while it runs. */
  saving: { done: number; of: number } | null;
  /** The one open in the crop screen. */
  adjusting: Queued | null;
  adjust: (image: Uuid | null) => void;
  /** What the crop screen asks of the queue. */
  crop: CropDesk;
}

export function usePhotoQueue(): QueueDesk {
  const live = useLive();
  const [read, setRead] = useState<QueueRead>({ kind: "loading" });
  const [queued, setQueued] = useState<Queued[]>([]);
  const [saving, setSaving] = useState<QueueDesk["saving"]>(null);
  const [adjusting, setAdjusting] = useState<Uuid | null>(null);
  const items = useRef(new Map<Uuid, Promise<ItemView>>());
  const writing = useWriting();
  const phone = handheld();

  const update = useCallback((image: Uuid, next: Partial<Queued>) => {
    setQueued((qs) => qs.map((q) => (q.photo.image_id === image ? { ...q, ...next } : q)));
  }, []);

  /** An item's page, read once however many of its photographs are waiting. */
  const itemOf = useCallback((id: Uuid) => {
    let found = items.current.get(id);
    if (!found) {
      found = api.item(id);
      items.current.set(id, found);
    }
    return found;
  }, []);

  // The queue, then each photograph in turn: what it is of, and its face.
  useEffect(() => {
    let stopped = false;
    void (async () => {
      let photos: UncutPhoto[];
      try {
        photos = await api.uncutPhotos();
      } catch (error) {
        if (live.current) setRead({ kind: "failed", message: reason(error, "Could not load the photos to crop.") });
        return;
      }
      if (!live.current) return;
      setQueued(
        photos.map((photo) => ({
          photo,
          subject: null,
          name: faceName(photo.face, { dimensions_absent: false }),
          aspect: null,
          corners: null,
          state: "waiting",
          ticked: false,
        })),
      );
      setRead({ kind: "ready" });

      for (const photo of photos) {
        if (stopped || !live.current) return;
        const id = photo.image_id;
        try {
          const item = await itemOf(photo.item_id);
          const shown = item.photos.find((p) => p.image_id === id);
          const subject = shown ? (item.subjects.find((s) => subjectKey(s) === subjectKey(shown)) ?? null) : null;
          if (!subject) {
            update(id, { state: "failed" });
            continue;
          }
          const described = { subject, name: faceName(photo.face, subject), aspect: measuredAspect(subject, photo.face) };
          // On a phone nothing is looked for: the face-finder needs a computer.
          if (phone) {
            update(id, { ...described, state: "missed" });
            continue;
          }
          update(id, { ...described, state: "finding" });
          const image = await loadPhoto(photo.digest);
          const corners = await (await model()).findFace(id, pixelsOf(image, SAM_SIZE, true));
          if (stopped || !live.current) return;
          const found = corners !== null && isFace(fromCorners(corners));
          update(id, { corners: found ? corners : null, state: found ? "found" : "missed", ticked: found });
        } catch (error) {
          console.warn("photo queue:", error);
          if (live.current) update(id, { state: "failed" });
        }
      }
    })();
    return () => {
      stopped = true;
    };
  }, [itemOf, live, phone, update]);

  const open = queued.find((q) => q.photo.image_id === adjusting) ?? null;

  return {
    read,
    queued,
    phone,
    tick: (image, on) => update(image, { ticked: on }),
    saving,
    /**
     * Every ticked cut, as one press: the person said once that these are
     * right. Each cut is its own write, named apart so a retry is the same
     * act for each (`partOf`).
     */
    save: () => {
      const ready = queued.filter((q) => q.ticked && q.corners && q.state === "found");
      return writing.press(`save:${ready.map((q) => q.photo.image_id).join(",")}`, async (act) => {
        try {
          let done = 0;
          for (const q of ready) {
            if (!live.current) return;
            setSaving({ done, of: ready.length });
            // Straightened from the photograph at full size, as the crop screen does.
            const image = await loadPhoto(q.photo.digest);
            const quad = fromCorners(q.corners!);
            const kept = await api.storeImage(await straightened(image, quad, ratioOf(image, quad, q.aspect)));
            await api.recordCut(q.photo.image_id, { digest: kept.digest, corners: q.corners!, act: partOf(act, q.photo.image_id) });
            if (live.current) update(q.photo.image_id, { state: "saved", ticked: false });
            done += 1;
          }
        } finally {
          if (live.current) setSaving(null);
        }
      });
    },
    adjusting: open,
    adjust: (image) => {
      writing.dismiss();
      setAdjusting(image);
    },
    crop: {
      findFace: (key, pixels, at) => model().then((m) => m.findFace(key, pixels, at)),
      cut: (corners, make) =>
        writing.press(`cut:${open?.photo.image_id ?? "none"}:${corners.join(",")}`, async (act) => {
          if (!open) return;
          const kept = await api.storeImage(await make());
          await api.recordCut(open.photo.image_id, { digest: kept.digest, corners, act });
          if (!live.current) return;
          update(open.photo.image_id, { state: "saved", ticked: false, corners });
          setAdjusting(null);
        }),
      uncrop: () => {
        writing.dismiss();
        setAdjusting(null);
      },
      busy: writing.busy,
      problem: writing.problem,
      dismiss: writing.dismiss,
    },
  };
}
