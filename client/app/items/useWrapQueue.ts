import { useCallback, useEffect, useRef, useState } from "react";

import { useLive } from "@app/acting";
import { anAct } from "@domain/acts";
import { api, reason } from "@domain/api";
import type { CaptureSubject, Unwrapped, Wrap } from "@domain/types";

import { draw, handheld } from "./crop";
import type { Pixels } from "./cut";
import type { Made } from "./wrap";
import { subjectKey } from "./subjects";

/**
 * Round things wrapped in their photographs at a computer (D240): those
 * waiting, beside the photographs to crop, or one wrapped again from its
 * page. Each is made in turn and shown as it would look, and can be turned
 * by hand, a photograph at a time, till its seams line up; **nothing is kept
 * until the person saves it**, as with a cut (D177).
 */

const wrapping = () => import("./wrap");

export type WrapState = "waiting" | "making" | "made" | "failed" | "saving" | "saved";

export interface Wrapping {
  entry: Unwrapped;
  key: string;
  state: WrapState;
  /** What it is doing now, or why it failed. */
  step: string;
  subject: CaptureSubject | null;
  made: Made | null;
  /** The made pictures as the page can show them, before they are kept: blob addresses in a wrapping's shape. */
  preview: Wrap | null;
  /** Open at the top, its top photograph looking into it (D241), rather than a lid. */
  open: boolean;
  /** Turned by hand, and being stitched again. */
  stitching: boolean;
}

export type WrapRead = { kind: "loading" } | { kind: "ready" } | { kind: "failed"; message: string };

export interface WrapDesk {
  read: WrapRead;
  wraps: Wrapping[];
  /** A phone: nothing is made here, and the screen says to open it at a computer. */
  phone: boolean;
  save: (key: string) => Promise<void>;
  /** Make one again: after a retake, or a failure. */
  again: (key: string) => void;
  /** Say it is open at the top, or has a lid, and make it again so. */
  setOpen: (key: string, open: boolean) => void;
  /** Turn one photograph of its side by so many degrees round it, and stitch it again. */
  nudge: (key: string, side: number, degrees: number) => void;
}

const keyOf = (u: Unwrapped) => subjectKey({ ...u, packaging_level: u.packaging_level });

/** A made picture at an address the page can load, until it is let go. */
async function blobUrl(px: Pixels): Promise<string> {
  const canvas = draw(new Uint8ClampedArray(px.data), px.width, px.height);
  try {
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/webp", 0.9));
    if (!blob) throw new Error("the picture could not be drawn");
    return URL.createObjectURL(blob);
  } finally {
    canvas.width = canvas.height = 0;
  }
}

/** The round things waiting to be wrapped, beside the photographs to crop. */
export const useWrapQueue = () => useWraps(api.unwrapped);

/** Round things wrapped in their photographs: those `load` lists, each made as soon as it is listed, unless on a phone. */
export function useWraps(load: () => Promise<Unwrapped[]>): WrapDesk {
  const live = useLive();
  const source = useRef(load);
  const [read, setRead] = useState<WrapRead>({ kind: "loading" });
  const [wraps, setWraps] = useState<Wrapping[]>([]);
  const phone = handheld();
  const turn = useRef<Promise<void>>(Promise.resolve());
  const urls = useRef<string[]>([]);
  // Whether each was last said to be open: a making for the other is not shown.
  const wanted = useRef(new Map<string, boolean>());
  // The newest made of each, and the turns pressed since and not yet stitched:
  // pressed faster than they are stitched, they are stitched together.
  const latest = useRef(new Map<string, { made: Made; preview: Wrap }>());
  const pending = useRef(new Map<string, number[]>());

  const update = useCallback((key: string, next: Partial<Wrapping>) => {
    setWraps((ws) => ws.map((w) => (w.key === key ? { ...w, ...next } : w)));
  }, []);

  /** Make one, after whatever is being made: one at a time, as the face-finder takes one at a time. */
  const make = useCallback(
    (entry: Unwrapped, open?: boolean) => {
      const key = keyOf(entry);
      turn.current = turn.current.then(async () => {
        if (!live.current) return;
        pending.current.delete(key);
        update(key, { state: "making", step: "Reading the item…" });
        try {
          const item = await api.item(entry.open_item);
          const subject = item.subjects.find((s) => subjectKey(s) === key) ?? null;
          if (!subject) throw new Error("Its page no longer shows it.");
          // Open as it was said to be, or as it was wrapped last time.
          const opened = open ?? wanted.current.get(key) ?? subject.wrap?.inside != null;
          if (wanted.current.has(key) && wanted.current.get(key) !== opened) return;
          update(key, { open: opened });
          const { makeWrap } = await wrapping();
          const made = await makeWrap(item, subject, (step) => live.current && update(key, { step }), opened);
          const [side, lid, base, inside, floor] = await Promise.all(
            [made.side, made.lid, made.base, made.inside, made.floor].map((px) => (px ? blobUrl(px) : Promise.resolve(null))),
          );
          urls.current.push(...([side, lid, base, inside, floor].filter(Boolean) as string[]));
          if (!live.current || (wanted.current.has(key) && wanted.current.get(key) !== opened)) return;
          const preview = { side: side!, lid: lid ?? null, base: base ?? null, inside: inside ?? null, floor: floor ?? null, made_from: made.made_from };
          latest.current.set(key, { made, preview });
          update(key, { state: "made", step: "", subject, made, preview });
        } catch (error) {
          if (live.current) update(key, { state: "failed", step: reason(error, "It could not be wrapped here.") });
        }
      });
    },
    [live, update],
  );

  useEffect(() => {
    source.current().then(
      (entries) => {
        if (!live.current) return;
        setWraps(entries.map((entry) => ({ entry, key: keyOf(entry), state: "waiting", step: "", subject: null, made: null, preview: null, open: false, stitching: false })));
        setRead({ kind: "ready" });
        if (!phone) for (const entry of entries) make(entry);
      },
      (error) => live.current && setRead({ kind: "failed", message: reason(error, "Could not read what is waiting to be wrapped.") }),
    );
  }, [live, make, phone]);

  useEffect(() => () => urls.current.forEach((u) => URL.revokeObjectURL(u)), []);

  return {
    read,
    wraps,
    phone,
    save: async (key) => {
      const w = wraps.find((x) => x.key === key);
      if (!w?.made || !w.subject) return;
      update(key, { state: "saving" });
      try {
        const { saveWrap } = await wrapping();
        await saveWrap(w.subject, w.made, anAct());
        // Its picture for lists, drawn from the wrapping now kept (D186, D240).
        try {
          const { ensureBoxPicture } = await import("./boxPicture");
          await ensureBoxPicture(await api.item(w.entry.open_item));
        } catch (error) {
          console.warn("drawing:", error);
        }
        if (live.current) update(key, { state: "saved" });
      } catch (error) {
        if (live.current) update(key, { state: "made", step: reason(error, "It could not be saved.") });
      }
    },
    again: (key) => {
      const w = wraps.find((x) => x.key === key);
      if (w) make(w.entry, w.open);
    },
    setOpen: (key, open) => {
      const w = wraps.find((x) => x.key === key);
      if (!w || w.open === open) return;
      // Said while it is being made, it is made again after: the newer making wins.
      update(key, { open, state: "waiting", step: "", made: null, preview: null });
      wanted.current.set(key, open);
      make(w.entry, open);
    },
    nudge: (key, side, degrees) => {
      const by = pending.current.get(key) ?? [];
      by[side] = (by[side] ?? 0) + degrees;
      pending.current.set(key, by);
      update(key, { stitching: true });
      turn.current = turn.current.then(async () => {
        const now = latest.current.get(key);
        const asked = pending.current.get(key);
        pending.current.delete(key);
        if (!now || !asked || !live.current) return;
        try {
          const { turnTo } = await wrapping();
          const next = await turnTo(now.made, now.made.turns.map((t, k) => t + ((asked[k] ?? 0) * Math.PI) / 180));
          const side = await blobUrl(next.side);
          urls.current.push(side);
          const preview = { ...now.preview, side };
          latest.current.set(key, { made: next.made, preview });
          if (live.current) update(key, { made: next.made, preview, step: "" });
        } catch (error) {
          if (live.current) update(key, { step: reason(error, "It could not be stitched again.") });
        } finally {
          if (live.current && !pending.current.has(key)) update(key, { stitching: false });
        }
      });
    },
  };
}
