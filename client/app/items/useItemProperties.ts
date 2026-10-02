import { useCallback, useEffect, useRef, useState } from "react";

import { useLive, useWriting } from "@app/acting";
import { anAct, partOf, type Act } from "@domain/acts";
import { ApiError, api, reason } from "@domain/api";
import type { BoundBarcode, CaptureSubject, ItemView, Uuid } from "@domain/types";

import type { Pixels, Point } from "./cut";
import { handheld } from "./crop";
import { measurementsOf, type Figures } from "./figures";
import { NO_FIGURES, cartonHolds, isOwnCarton, presentationNeeded, readHolds, sayFirst, subjectKey, type Face } from "./subjects";

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
 * **Photographs are sent while the next is taken.** Each goes into a line of
 * its own, one after another, so the camera is never waiting on the WiFi; a
 * photograph that did not send says so on its tile and is sent again from
 * there, as the same act.
 *
 * The writes are the ones the Weigh and Capture screens made, unchanged:
 * `POST /weighings` for a scale reading, `POST /observations` for figures (one
 * act, D133), its images for photographs (one look, D132), and
 * `POST /items/{id}/barcodes` for a label (D164). A photograph is cut to its
 * face with `POST /images` and then its cut (D176), its corners first placed
 * by a model in the browser (D177).
 *
 * **An item's own carton is a box of so many of it** (D178). Weighing,
 * measuring or photographing it says what it holds first, with
 * `POST /items/{id}/carton`, when no carton is on file or a different count
 * was typed: the same press, two writes.
 */

/** The face-finding model: its own chunk, with the ONNX runtime, fetched when first wanted. */
const model = () => import("./faceModel");

export type ItemRead = { kind: "loading" } | { kind: "ready"; item: ItemView } | { kind: "failed"; message: string };

export type Action = "weigh" | "measure" | "photos" | "barcodes";

/** Which subject has which action open. */
export interface Open {
  key: string;
  action: Action;
}

/**
 * A photograph open to be cut to its face (D176): which subject and face it
 * is, the photograph, and the corners last marked on it, if any.
 */
export interface Cropping {
  subject: CaptureSubject;
  face: Face;
  image_id: Uuid;
  digest: string;
  corners: number[] | null;
}

/** A photograph on its way: being sent, or not sent and waiting to be sent again. */
export type Sending = "sending" | "failed";

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

  /** How many of the item its carton holds, as typed (D178). */
  holds: string;
  typeHolds: (next: string) => void;

  figures: Figures;
  type: (field: "weight" | "length" | "width" | "height", next: string) => void;
  choosePresentation: (code: string) => void;
  toggleNoDimensions: () => void;
  measure: (subject: CaptureSubject) => Promise<void>;

  /** Faces photographed in this look, sent or still on their way. */
  taken: Face[];
  /** Faces whose photograph is on its way, or did not send. */
  sending: Partial<Record<Face, Sending>>;
  /** Send a face in the background; at a desk, open it to be cut once sent. */
  attach: (subject: CaptureSubject, face: Face, image: Blob) => void;
  /** Send again a face that did not send. */
  resend: (face: Face) => void;

  /** The photograph open to be cut, if one is. */
  cropping: Cropping | null;
  crop: (cropping: Cropping) => void;
  uncrop: () => void;
  /**
   * Cut the open photograph at these corners. `make` straightens it into the
   * picture to keep, inside the press, so the screen is busy while it works.
   */
  cut: (corners: number[], make: () => Promise<Blob>) => Promise<void>;
  /**
   * Where the face is in a photograph (D177): eight corner fractions, or
   * nothing. `key` names the photograph, so asking again at another point
   * reuses its encoding; `pixels` is it at the model's size; `at` is a point
   * on the face as fractions, the middle when not given.
   */
  findFace: (key: string, pixels: Pixels, at?: Point) => Promise<number[] | null>;

  /** Name a run of the item that looks different, to photograph on its own (D182). True when named. */
  addVariant: (code: string) => Promise<boolean>;

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
    ...(s.lot_id ? { lot: s.lot_id } : {}),
  };
}

export function useItemProperties(itemId: string | null): PropertiesDesk {
  const live = useLive();
  const [read, setRead] = useState<ItemRead>({ kind: "loading" });
  const [open, setOpen] = useState<Open | null>(null);
  const [reading, setReading] = useState("");
  const [unit, setUnit] = useState("kg");
  const [figures, setFigures] = useState<Figures>(NO_FIGURES);
  const [holds, setHolds] = useState("");
  const [taken, setTaken] = useState<Face[]>([]);
  const [sending, setSending] = useState<Partial<Record<Face, Sending>>>({});
  // Photographs being sent, one after another, and each one's act so that
  // sending it again is the same act.
  const line = useRef<Promise<void>>(Promise.resolve());
  const held = useRef(new Map<Face, { subject: CaptureSubject; image: Blob; act: Act }>());
  // The item on screen, which a photograph sent for another must not touch.
  const showing = useRef(itemId);
  const [cropping, setCropping] = useState<Cropping | null>(null);
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
    setSending({});
    held.current.clear();
    showing.current = itemId;
    setCropping(null);
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
    // The count on file, to keep or correct; nothing, for a carton not said.
    const known = read.kind === "ready" && isOwnCarton(subject) ? cartonHolds(read.item.packing) : null;
    setHolds(known === null ? "" : String(known));
    // Measuring starts from nothing typed; photographing after measuring keeps
    // the measuring look, and photographing on its own starts a new one.
    if (action === "measure") setFigures(NO_FIGURES);
    if (action === "photos") {
      // At a desk the face-finder's model is fetched while the camera is in
      // use, so it is ready by the first photo's crop. Not on a phone, whose
      // camera wants the memory.
      if (!handheld()) {
        void model()
          .then((m) => m.warm())
          .catch(() => undefined);
      }
      setTaken([]);
      if (look.current?.key !== subjectKey(subject)) look.current = null;
      looks.current += 1;
    }
    if (action !== "photos") look.current = null;
  };

  /**
   * Say what the item's own carton holds, when the act needs it said first:
   * no carton on file, or a count typed that is not the one on file. A part
   * of the press it is in, so a retry is the same act.
   */
  const sayCartonFirst = async (subject: CaptureSubject, act: Act) => {
    if (!isOwnCarton(subject) || !subject.item_id || read.kind !== "ready") return;
    const typed = readHolds(holds);
    if ("problem" in typed) throw new ApiError(typed.problem, 400);
    if (!sayFirst(read.item.packing, typed.holds)) return;
    await api.sayCarton(subject.item_id, { holds: typed.holds, act: partOf(act, "carton") });
  };

  /**
   * One face, sent behind whatever is already on its way. **Taken at once,
   * sent when its turn comes**, so the next face can be photographed straight
   * away. Photographing without measuring first takes a look of its own, made
   * by the first photograph sent; a retake is a new row (D132).
   */
  const send = (face: Face) => {
    const job = held.current.get(face);
    if (!job) return;
    const item = itemId;
    const here = () => live.current && showing.current === item;
    setTaken((t) => (t.includes(face) ? t : [...t, face]));
    setSending((s) => ({ ...s, [face]: "sending" }));
    line.current = line.current.then(async () => {
      try {
        let event = look.current?.key === subjectKey(job.subject) ? look.current.event : null;
        if (!event) {
          await sayCartonFirst(job.subject, job.act);
          const response = await api.recordCapture({
            ...named(job.subject),
            level: job.subject.packaging_level,
            measurements: [],
            photographs: true,
            act: job.act,
          });
          event = response.observation_event_id;
          if (here()) look.current = { key: subjectKey(job.subject), event };
        }
        const kept = await api.photograph(event, face, job.image);
        if (held.current.get(face) === job) held.current.delete(face);
        if (!here()) return;
        setSending((s) => {
          const { [face]: _, ...rest } = s;
          return rest;
        });
        // At a desk, straight on to marking its corners, unless a crop is
        // already open: one in hand is not swapped for the next, which waits
        // in Photos to crop. A phone's photographs are cut at a computer,
        // which finds their faces (D181).
        if (!handheld()) {
          setCropping((open) => open ?? { subject: job.subject, face, image_id: kept.image_id, digest: kept.digest, corners: null });
        }
        await reload();
      } catch (error) {
        console.warn("photograph:", error);
        if (here()) setSending((s) => ({ ...s, [face]: "failed" }));
      }
    });
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
      return press(`weigh:${subjectKey(subject)}:${entered}:${unit}:${holds.trim()}`, async (act) => {
        if (!entered) throw new ApiError("Read the scale first.", 400);
        if (!subject.packaging_level) throw new ApiError("A part is weighed with its size: use Measure.", 400);
        await sayCartonFirst(subject, act);
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

    holds,
    typeHolds: setHolds,

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
      return press(`measure:${subjectKey(subject)}:${JSON.stringify(measurements)}:${figures.presentation}:${holds.trim()}`, async (act) => {
        if (measurements.length === 0) throw new ApiError("Nothing has been measured yet.", 400);
        // Said sooner than the server would say it (D138).
        const lengths = measurements.some((m) => !m.absent_reason && m.metric !== "gross_weight");
        if (presentationNeeded(subject) && lengths && !figures.presentation) {
          throw new ApiError("Choose how it was arranged: folded, flat, as supplied…", 400);
        }
        await sayCartonFirst(subject, act);
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
    sending,
    attach: (subject, face, image) => {
      held.current.set(face, { subject, image, act: anAct() });
      send(face);
    },
    resend: send,

    cropping,
    crop: (next) => {
      dismiss();
      setCropping(next);
    },
    uncrop: () => {
      dismiss();
      setCropping(null);
    },
    findFace: (key, pixels, at) => model().then((m) => m.findFace(key, pixels, at)),
    cut: (corners, make) =>
      press(`cut:${cropping?.image_id ?? "none"}:${corners.join(",")}`, async (act) => {
        if (!cropping) return;
        const kept = await api.storeImage(await make());
        await api.recordCut(cropping.image_id, { digest: kept.digest, corners, act });
        if (!live.current) return;
        setCropping(null);
        await reload();
      }),

    addVariant: async (code) => {
      let named = false;
      await press(`variant:${code.trim()}`, async () => {
        if (!itemId) return;
        if (!code.trim()) throw new ApiError("Give the variant a name: what is printed on its carton.", 400);
        const added = await api.addLot(itemId, code.trim());
        if (!live.current) return;
        named = true;
        setSaid({ tone: "success", text: added.added ? `${added.code} added. Photograph it and measure it below.` : `${added.code} was already a variant.` });
        await reload();
      });
      return named;
    },

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
