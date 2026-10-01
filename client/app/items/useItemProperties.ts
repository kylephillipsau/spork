import { useCallback, useEffect, useRef, useState } from "react";

import { useLive, useWriting } from "@app/acting";
import { ApiError, api, reason } from "@domain/api";
import type { BoundBarcode, CaptureSubject, ItemView, Uuid } from "@domain/types";

import { measurementsOf, type Figures } from "./figures";
import { NO_FIGURES, presentationNeeded, subjectKey, type Face } from "./subjects";

/**
 * One item's properties, as logic: what it is and what is known of it, and
 * weighing, measuring, photographing and labelling each of its subjects.
 *
 * **One place for all of it** (D174). The item list's panel and the item's own
 * page draw the same thing from this, so finding an item and changing what is
 * known about it are one step, wherever it was found.
 *
 * **One action open at a time.** Weighing the carton and measuring the each are
 * two things in two hands; a screen with both forms open is a screen that has
 * to be aimed at before it is typed into.
 *
 * The writes are the ones the Weigh and Capture screens made, unchanged:
 * `POST /weighings` for a scale reading, `POST /observations` for figures (one
 * act, D133), its images for photographs (one look, D132), and
 * `POST /items/{id}/barcodes` for a label (D164).
 */

export type ItemRead = { kind: "loading" } | { kind: "ready"; item: ItemView } | { kind: "failed"; message: string };

export type Action = "weigh" | "measure" | "photos" | "barcodes";

/** Which subject has which action open. */
export interface Open {
  key: string;
  action: Action;
}

/** What the last act said, worth showing once. */
export interface Said {
  tone: "success" | "warning";
  text: string;
  /** A finding the act raised, to open. */
  finding?: Uuid | undefined;
}

export interface PropertiesDesk {
  read: ItemRead;
  open: Open | null;
  show: (subject: CaptureSubject, action: Action) => void;
  close: () => void;

  /** The scale, as typed: a string, never a number. */
  reading: string;
  unit: string;
  typeReading: (next: string) => void;
  setUnit: (next: string) => void;
  weigh: (subject: CaptureSubject) => Promise<void>;

  figures: Figures;
  type: (field: "weight" | "length" | "width" | "height", next: string) => void;
  choosePresentation: (code: string) => void;
  toggleNoDimensions: () => void;
  measure: (subject: CaptureSubject) => Promise<void>;

  /** Faces photographed in this look. */
  taken: Face[];
  attach: (subject: CaptureSubject, face: Face, image: Blob) => Promise<void>;

  barcodes: BoundBarcode[];
  binding: string;
  typeBinding: (next: string) => void;
  count: string;
  typeCount: (next: string) => void;
  bind: (subject: CaptureSubject) => Promise<void>;

  busy: boolean;
  problem: string | null;
  said: Said | null;
  dismiss: () => void;
}

/** Grams, as the scale is read: "0.52 kg". */
function kg(g: number): string {
  return `${(g / 1000).toLocaleString(undefined, { maximumFractionDigits: 3 })} kg`;
}

/** The subject, as the writes name it. */
function named(s: CaptureSubject) {
  return {
    ...(s.item_id ? { item: s.item_id } : {}),
    ...(s.item_style_id ? { style: s.item_style_id } : {}),
    ...(s.item_part_id ? { part: s.item_part_id } : {}),
  };
}

export function useItemProperties(itemId: string | null): PropertiesDesk {
  const live = useLive();
  const [read, setRead] = useState<ItemRead>({ kind: "loading" });
  const [open, setOpen] = useState<Open | null>(null);
  const [reading, setReading] = useState("");
  const [unit, setUnit] = useState("kg");
  const [figures, setFigures] = useState<Figures>(NO_FIGURES);
  const [taken, setTaken] = useState<Face[]>([]);
  const [barcodes, setBarcodes] = useState<BoundBarcode[]>([]);
  const [binding, setBinding] = useState("");
  const [count, setCount] = useState("");
  const [said, setSaid] = useState<Said | null>(null);
  // The look photographs hang off: the measuring act's, or one taken for its
  // photographs when they are all this visit is for. Which subject it is of,
  // so a look is never borrowed by another.
  const look = useRef<{ key: string; event: Uuid } | null>(null);
  // Each opening of the camera is a new look; a retried press inside one is not.
  const looks = useRef(0);
  const asking = useRef(0);
  const later = useRef<number[]>([]);

  const { busy, problem, dismiss, press } = useWriting();

  const reload = useCallback(async () => {
    if (!itemId) return;
    const n = ++asking.current;
    try {
      const item = await api.item(itemId);
      if (live.current && n === asking.current) setRead({ kind: "ready", item });
    } catch (error) {
      if (live.current && n === asking.current) setRead({ kind: "failed", message: reason(error, "Could not load the item.") });
    }
  }, [itemId, live]);

  /**
   * **Read again shortly after a figure is recorded.** The figures shown come
   * from `observation_current`, which the scheduler rebuilds a few seconds
   * after a write (D107); the app may not rebuild it itself. So the act is
   * said at once and the card catches up on these reads.
   */
  const settle = useCallback(() => {
    for (const ms of [2500, 6000, 12000]) later.current.push(window.setTimeout(() => void reload(), ms));
  }, [reload]);
  useEffect(() => () => later.current.forEach((t) => window.clearTimeout(t)), []);

  // A different item is a fresh start: nothing typed for one is for another.
  useEffect(() => {
    later.current.forEach((t) => window.clearTimeout(t));
    later.current = [];
    setRead({ kind: "loading" });
    setOpen(null);
    setSaid(null);
    setReading("");
    setFigures(NO_FIGURES);
    setTaken([]);
    setBarcodes([]);
    look.current = null;
    dismiss();
    void reload();
    if (itemId) {
      void api
        .itemBarcodes(itemId)
        .then((rows) => {
          if (live.current) setBarcodes(rows);
        })
        .catch(() => {
          // Silent: a label list nobody could read is a section that shows
          // none, and the figures are what this is for.
        });
    }
  }, [itemId]); // eslint-disable-line react-hooks/exhaustive-deps

  const show = (subject: CaptureSubject, action: Action) => {
    setOpen({ key: subjectKey(subject), action });
    setSaid(null);
    dismiss();
    setReading("");
    setBinding("");
    setCount("");
    // Measuring starts from nothing typed; photographing after measuring keeps
    // the measuring look, and photographing on its own starts a new one.
    if (action === "measure") setFigures(NO_FIGURES);
    if (action === "photos") {
      setTaken([]);
      if (look.current?.key !== subjectKey(subject)) look.current = null;
      looks.current += 1;
    }
    if (action !== "photos") look.current = null;
  };

  return {
    read,
    open,
    show,
    close: () => {
      setOpen(null);
      look.current = null;
      dismiss();
    },

    reading,
    unit,
    typeReading: setReading,
    setUnit,
    /**
     * A scale reading. **The reading is in the act's name**: put on the scale
     * twice because the first answer was lost is one act; read again after a
     * correction is another.
     */
    weigh: (subject) => {
      const entered = reading.trim();
      return press(`weigh:${subjectKey(subject)}:${entered}:${unit}`, async (act) => {
        if (!entered) throw new ApiError("Read the scale first.", 400);
        if (!subject.packaging_level) throw new ApiError("A part is weighed with its size: use Measure.", 400);
        const answer = await api.weigh({
          ...(subject.item_id ? { item: subject.item_id } : {}),
          ...(subject.item_style_id ? { style: subject.item_style_id } : {}),
          level: subject.packaging_level,
          entered,
          unit,
          act,
        });
        if (!live.current) return;
        setReading("");
        setOpen(null);
        setSaid(
          answer.disagreed
            ? {
                tone: "warning",
                text: `Weighed ${kg(answer.recorded_g)}. That is a long way from the ${kg(answer.previous_g ?? 0)} on record, so a finding was raised to look into it.`,
                finding: answer.discrepancy_id ?? undefined,
              }
            : {
                tone: "success",
                text:
                  answer.previous_g === null
                    ? `Weighed ${kg(answer.recorded_g)}. It shows here in a moment.`
                    : `Weighed ${kg(answer.recorded_g)}, against ${kg(answer.previous_g)} on record. It shows here in a moment.`,
              },
        );
        await reload();
        settle();
      });
    },

    figures,
    type: (field, next) => setFigures((f) => ({ ...f, [field]: next })),
    choosePresentation: (code) => setFigures((f) => ({ ...f, presentation: code })),
    // Clearing the lengths is part of declaring there are none (D138): typed
    // numbers behind a "none" are two answers, and one of them would be sent.
    toggleNoDimensions: () =>
      setFigures((f) => (f.noDimensions ? { ...f, noDimensions: false } : { ...f, noDimensions: true, length: "", width: "", height: "" })),
    /**
     * Figures, as one act (D133): everything typed in one request, and the
     * event it answers with is what the photographs taken next hang off.
     */
    measure: (subject) => {
      const measurements = measurementsOf(figures);
      return press(`measure:${subjectKey(subject)}:${JSON.stringify(measurements)}:${figures.presentation}`, async (act) => {
        if (measurements.length === 0) throw new ApiError("Nothing has been measured yet.", 400);
        // Said sooner than the server would say it (D138).
        const lengths = measurements.some((m) => !m.absent_reason && m.metric !== "gross_weight");
        if (presentationNeeded(subject) && lengths && !figures.presentation) {
          throw new ApiError("Choose how it was arranged: folded, flat, as supplied…", 400);
        }
        const response = await api.recordCapture({
          ...named(subject),
          ...(figures.presentation ? { presentation: figures.presentation } : {}),
          level: subject.packaging_level,
          measurements,
          act,
        });
        if (!live.current) return;
        look.current = { key: subjectKey(subject), event: response.observation_event_id };
        setFigures(NO_FIGURES);
        setTaken([]);
        setOpen({ key: subjectKey(subject), action: "photos" });
        setSaid({
          tone: "success",
          text: `${response.observation_ids.length} ${response.observation_ids.length === 1 ? "figure" : "figures"} recorded, shown here in a moment. Photograph it now, while it is in front of you.`,
        });
        await reload();
        settle();
      });
    },

    taken,
    /**
     * One face. **Uploaded as it is taken**, so a failed upload is one photo to
     * retake rather than a session to redo; a retake is a new row (D132).
     * Photographing without measuring first takes a look of its own.
     */
    attach: (subject, face, image) =>
      press(`photo:${subjectKey(subject)}:${looks.current}:${face}:${image.size}`, async (act) => {
        let event = look.current?.key === subjectKey(subject) ? look.current.event : null;
        if (!event) {
          const response = await api.recordCapture({
            ...named(subject),
            level: subject.packaging_level,
            measurements: [],
            photographs: true,
            act,
          });
          event = response.observation_event_id;
          look.current = { key: subjectKey(subject), event };
        }
        await api.photograph(event, face, image);
        if (!live.current) return;
        setTaken((t) => (t.includes(face) ? t : [...t, face]));
        await reload();
      }),

    barcodes,
    binding,
    typeBinding: (next) => {
      setBinding(next);
      dismiss();
    },
    count,
    typeCount: setCount,
    /**
     * Say that this label means this box (D164). The level is the subject's,
     * not asked again: the label in hand is on the thing being looked at.
     */
    bind: (subject) =>
      press(`bind:${subjectKey(subject)}:${binding.trim()}`, async () => {
        const scanned = binding.trim();
        if (!scanned) return;
        if (!subject.item_id || !subject.packaging_level) {
          throw new ApiError("Only an item's own carton or each takes a label.", 400);
        }
        const typed = count.trim();
        const bound = await api.bindBarcode(subject.item_id, {
          scan: scanned,
          packaging_level: subject.packaging_level,
          // A blank is not a zero: somebody who does not know the case pack
          // has said nothing about it.
          quantity: typed === "" ? null : Number(typed),
        });
        if (!live.current) return;
        setBinding("");
        setCount("");
        setSaid(
          bound.already
            ? { tone: "success", text: `${scanned} was already bound to this ${subject.packaging_level}.` }
            : { tone: bound.warnings.length ? "warning" : "success", text: [`Bound ${scanned}.`, ...bound.warnings].join(" ") },
        );
        const rows = await api.itemBarcodes(subject.item_id);
        if (live.current) setBarcodes(rows);
      }),

    busy,
    problem,
    said,
    dismiss: () => {
      dismiss();
      setSaid(null);
    },
  };
}
