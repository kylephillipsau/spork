import { useCallback, useEffect, useRef, useState } from "react";

import { useLive, useWriting } from "@app/acting";
import { partOf, pressing } from "@domain/acts";
import { ApiError, api, imageUrl, reason } from "@domain/api";
import type { CaptureSubject, ItemView, UncutPhoto, Uuid } from "@domain/types";

import { faceName, measuredAspect } from "./box";
import { handheld, loadPhoto, pixelsOf, ratioOf, straightened } from "./crop";
import { WHOLE, fromCorners, isFace } from "./cut";
import { SAM_SIZE } from "./faceFind";
import type { CropDesk } from "./FaceCrop";
import { subjectKey } from "./subjects";

/**
 * The photographs waiting to be cut, worked through at a computer (D181).
 *
 * A phone takes the photographs and has not the memory to find their faces.
 * This finds each one, a photograph at a time, the oldest first, and keeps
 * the corners it found; the person looks down the results and saves each one
 * that is right with one press. One that is wrong, or where no face was
 * found, opens in the crop screen to be put right. **Nothing is kept until
 * the person saves** (D177): a cut is their judgement, and an act of theirs.
 */

const model = () => import("./faceModel");

export type QueueState = "waiting" | "finding" | "found" | "missed" | "failed" | "saving" | "saved";

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
}

export type QueueRead = { kind: "loading" } | { kind: "ready" } | { kind: "failed"; message: string };

export interface QueueDesk {
  read: QueueRead;
  queued: Queued[];
  /** A phone: nothing is found here, and the screen says to open it at a computer. */
  phone: boolean;
  /**
   * Save a photograph's cut where its face was found. Saves wait their turn,
   * so the next can be pressed before the last has landed.
   */
  save: (image: Uuid) => Promise<void>;
  /** The one open in the crop screen. */
  adjusting: Queued | null;
  adjust: (image: Uuid | null) => void;
  /** Look again at every photograph that could not be read: after the server or the WiFi came back. */
  again: () => void;
  /** Use a photograph as it was taken: a glove on a bench has no face to cut it to. */
  keep: (image: Uuid) => Promise<void>;
  /**
   * Move every photograph of a look to the item and level they are really of
   * (D190): filed again there, the first filing shown nowhere. True when moved.
   */
  move: (look: Uuid, code: string, level: string) => Promise<boolean>;
  /** What the crop screen asks of the queue. */
  crop: CropDesk;
}

/** An item drawn as its box once front, right and top are cut (D186). Never fails a save. */
async function drawBox(item: Uuid): Promise<void> {
  try {
    const { ensureBoxPicture } = await import("./boxPicture");
    await ensureBoxPicture(await api.item(item));
  } catch (error) {
    console.warn("box drawing:", error);
  }
}

export function usePhotoQueue(): QueueDesk {
  const live = useLive();
  const [read, setRead] = useState<QueueRead>({ kind: "loading" });
  const [queued, setQueued] = useState<Queued[]>([]);
  const [adjusting, setAdjusting] = useState<Uuid | null>(null);
  const items = useRef(new Map<Uuid, Promise<ItemView>>());
  // Read again after a move: its photographs come back under their item.
  const [round, setRound] = useState(0);
  const writing = useWriting();
  const phone = handheld();
  // Saves run one after another, each its own act until it lands.
  const saves = useRef(pressing());
  const turn = useRef<Promise<void>>(Promise.resolve());

  const update = useCallback((image: Uuid, next: Partial<Queued>) => {
    setQueued((qs) => qs.map((q) => (q.photo.image_id === image ? { ...q, ...next } : q)));
  }, []);

  /**
   * An item's page, read once however many of its photographs are waiting.
   * **A failed read is not kept**: kept, one dropped request (the server
   * restarting, the WiFi) failed every photograph of that item until the page
   * was reloaded.
   */
  const itemOf = useCallback((id: Uuid) => {
    let found = items.current.get(id);
    if (!found) {
      found = api.item(id);
      items.current.set(id, found);
      found.catch(() => {
        if (items.current.get(id) === found) items.current.delete(id);
      });
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
        })),
      );
      setRead({ kind: "ready" });

      for (const photo of photos) {
        if (stopped || !live.current) return;
        const id = photo.image_id;
        try {
          const item = await itemOf(photo.item_id);
          const shown = item.photos.find((p) => p.image_id === id);
          // Its own card; or, for a photograph of the family's carton, which an
          // item's page no longer shows (D190), the item's card at that level:
          // it is only named and proportioned by it, and cut where it is.
          const subject = shown
            ? (item.subjects.find((s) => subjectKey(s) === subjectKey(shown)) ??
              item.subjects.find((s) => s.item_id === item.item_id && s.packaging_level === shown.packaging_level) ??
              null)
            : null;
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
          update(id, { corners: found ? corners : null, state: found ? "found" : "missed" });
        } catch (error) {
          console.warn("photo queue:", error);
          if (live.current) update(id, { state: "failed" });
        }
      }
    })();
    return () => {
      stopped = true;
    };
  }, [itemOf, live, phone, update, round]);

  const open = queued.find((q) => q.photo.image_id === adjusting) ?? null;

  return {
    read,
    queued,
    phone,
    save: (image) => {
      const q = queued.find((x) => x.photo.image_id === image);
      if (!q?.corners || q.state !== "found") return turn.current;
      const corners = q.corners;
      update(image, { state: "saving" });
      turn.current = turn.current.then(async () => {
        const key = `save:${image}`;
        try {
          // Straightened from the photograph at full size, as the crop screen does.
          const photo = await loadPhoto(q.photo.digest);
          const quad = fromCorners(corners);
          const kept = await api.storeImage(await straightened(photo, quad, ratioOf(photo, quad, q.aspect)));
          await api.recordCut(image, { digest: kept.digest, corners, act: saves.current.attempt(key) });
          saves.current.landed(key);
          if (live.current) update(image, { state: "saved" });
          await drawBox(q.photo.item_id);
        } catch (error) {
          if (!live.current) return;
          update(image, { state: "found" });
          writing.say(reason(error, "Could not save the photo."));
        }
      });
      return turn.current;
    },
    again: () => {
      items.current.clear();
      setRound((r) => r + 1);
    },
    keep: (image) => {
      const q = queued.find((x) => x.photo.image_id === image);
      return writing.press(`keep:${image}`, async (act) => {
        if (!q) return;
        // The photograph's own bytes, named as its cut: nothing is redrawn.
        await api.recordCut(image, { digest: q.photo.digest, corners: WHOLE, act });
        if (live.current) update(image, { state: "saved", corners: WHOLE });
      });
    },
    move: async (look, code, level) => {
      let moved = false;
      const photos = queued.filter((q) => q.photo.look_id === look && q.state !== "saved");
      await writing.press(`move:${look}:${code.trim()}:${level}`, async (act) => {
        const page = await api.items({ q: code.trim() });
        const item = page.items.find((i) => i.code.toLowerCase() === code.trim().toLowerCase());
        if (!item) throw new ApiError(`No item has the code ${code.trim()}.`, 400);
        // A carton needs saying before it is photographed (D178); a no-op when on file.
        if (level !== "each") await api.sayCarton(item.item_id, { holds: null, act: partOf(act, "carton") });
        const look2 = await api.recordCapture({ item: item.item_id, level, measurements: [], photographs: true, act: partOf(act, "look") });
        for (const q of photos) {
          const bytes = await (await fetch(imageUrl(q.photo.digest))).blob();
          const kept = await api.photographAsIs(look2.observation_event_id, q.photo.face, bytes);
          await api.movePhoto(q.photo.image_id, kept.image_id, partOf(act, q.photo.image_id));
        }
        moved = true;
      });
      if (moved && live.current) {
        items.current.clear();
        setRound((r) => r + 1);
      }
      return moved;
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
          if (live.current) {
            update(open.photo.image_id, { state: "saved", corners });
            setAdjusting(null);
          }
          await drawBox(open.photo.item_id);
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
