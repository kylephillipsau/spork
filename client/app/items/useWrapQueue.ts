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
 * The round things waiting to be wrapped in their photographs, worked through
 * at a computer beside the photographs to crop (D240). Each is made in turn,
 * the oldest first, and shown as it would look; **nothing is kept until the
 * person saves it**, as with a cut (D177).
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

export function useWrapQueue(): WrapDesk {
  const live = useLive();
  const [read, setRead] = useState<WrapRead>({ kind: "loading" });
  const [wraps, setWraps] = useState<Wrapping[]>([]);
  const phone = handheld();
  const turn = useRef<Promise<void>>(Promise.resolve());
  const urls = useRef<string[]>([]);

  const update = useCallback((key: string, next: Partial<Wrapping>) => {
    setWraps((ws) => ws.map((w) => (w.key === key ? { ...w, ...next } : w)));
  }, []);

  /** Make one, after whatever is being made: one at a time, as the face-finder takes one at a time. */
  const make = useCallback(
    (entry: Unwrapped, open?: boolean) => {
      const key = keyOf(entry);
      turn.current = turn.current.then(async () => {
        if (!live.current) return;
        update(key, { state: "making", step: "Reading the item…" });
        try {
          const item = await api.item(entry.open_item);
          const subject = item.subjects.find((s) => subjectKey(s) === key) ?? null;
          if (!subject) throw new Error("Its page no longer shows it.");
          // Open as it was said to be, or as it was wrapped last time.
          const opened = open ?? subject.wrap?.inside != null;
          update(key, { open: opened });
          const { makeWrap } = await wrapping();
          const made = await makeWrap(item, subject, (step) => live.current && update(key, { step }), opened);
          const [side, lid, base, inside, floor] = await Promise.all(
            [made.side, made.lid, made.base, made.inside, made.floor].map((px) => (px ? blobUrl(px) : Promise.resolve(null))),
          );
          urls.current.push(...([side, lid, base, inside, floor].filter(Boolean) as string[]));
          if (!live.current) return;
          const preview = { side: side!, lid: lid ?? null, base: base ?? null, inside: inside ?? null, floor: floor ?? null, made_from: made.made_from };
          update(key, { state: "made", step: "", subject, made, preview });
        } catch (error) {
          if (live.current) update(key, { state: "failed", step: reason(error, "It could not be wrapped here.") });
        }
      });
    },
    [live, update],
  );

  useEffect(() => {
    api.unwrapped().then(
      (entries) => {
        if (!live.current) return;
        setWraps(entries.map((entry) => ({ entry, key: keyOf(entry), state: "waiting", step: "", subject: null, made: null, preview: null, open: false })));
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
      update(key, { open, state: "waiting", step: "" });
      make(w.entry, open);
    },
  };
}
