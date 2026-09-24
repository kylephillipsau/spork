import { useCallback, useEffect, useState } from "react";
import { useLive, useWriting } from "@app/acting";
import { ApiError, api, reason } from "@domain/api";
import { measurementsOf, type Figures } from "./figures";

// `Figures` and `measurementsOf` live in `./figures`, which has no React in it
// so a test runner can read it. Re-exported here because that is where callers
// have always found them, and moving a type is not a reason to touch nine
// imports.
export { measurementsOf, type Figures } from "./figures";
import type {
  BoundBarcode,
  CaptureScreen,
  CaptureSubject,
  Resolution,
  Uuid,
} from "@domain/types";

/**
 * The capture session, as logic.
 *
 * Same split as the two benches: this fetches and mutates and returns data, the
 * component takes data and emits primitives, and the presentational half renders
 * from a fixture with no network.
 *
 * # The session is in memory, and there is no route for it
 *
 * The subject being captured lives in this hook and not in the URL, which is a
 * deliberate refusal of the obvious thing. Capture was named as the screen that
 * would force a real router, and it turns out to be the screen that argues
 * against one: **a URL naming the subject promises a resumability the model
 * deliberately does not offer.** D133 makes the whole session one act, so
 * nothing an operator types is durable until they press Record — a link that
 * reopened `/capture/17e1…` after a refresh would restore the subject and
 * silently drop the three figures beside it, which is worse than losing both.
 *
 * The router arrives with a screen whose state is on the server.
 */

/** The seven, in the order the operator walks around the box. `label` last
 *  because it is the one that is not a geometric face. */
export const FACES = ["front", "back", "left", "right", "top", "bottom", "label"] as const;
export type Face = (typeof FACES)[number];

/**
 * The seven arrangements, and the order is the order they are likely.
 *
 * D138. A shipped vocabulary, so this list and `presentation` in the database
 * are the same seven words — a code here the database has never heard of is a
 * rejection, which is the check working rather than a bug to route around.
 */
export const PRESENTATIONS = [
  { value: "as_supplied", label: "As supplied" },
  { value: "folded", label: "Folded" },
  { value: "flat", label: "Laid flat" },
  { value: "rolled", label: "Rolled" },
  { value: "assembled", label: "Assembled" },
  { value: "knocked_down", label: "Knocked down" },
  { value: "compressed", label: "Compressed" },
] as const;


export const NO_FIGURES: Figures = {
  weight: "",
  length: "",
  width: "",
  height: "",
  presentation: "",
  noDimensions: false,
};

/**
 * Whether this subject's lengths *need* the arrangement recorded with them.
 *
 * **The writer's rule exactly, and that matters more than the rule being the
 * best one.** `POST /observations` requires a presentation for a length at
 * `each` and nowhere else. A client that asked for more would refuse writes the
 * server accepts, and one that asked for less would send requests it knows will
 * be rejected — two rules that agree about a case and disagree about the edge
 * is the shape that produces a screen nobody can explain.
 *
 * A part is offered the chooser and not required to answer, because a part is
 * usually the rigid half of a set: a 1200mm handle has one arrangement.
 */
export function presentationNeeded(subject: CaptureSubject): boolean {
  return subject.packaging_level === "each";
}

/** Whether to offer it at all. Never for a carton: one arrangement, no question. */
export function presentationOffered(subject: CaptureSubject): boolean {
  return presentationNeeded(subject) || subject.item_part_id !== null;
}

export type CaptureStatus =
  | { kind: "loading" }
  | { kind: "ready"; screen: CaptureScreen }
  | { kind: "failed"; message: string };

/** Where a session has got to. */
export type Stage =
  /** Choosing what to walk to. */
  | { kind: "worklist" }
  /** A box in hand and nothing recorded yet. */
  | { kind: "figures"; subject: CaptureSubject }
  /** The act has landed, and the photographs hang off the event it returned. */
  | { kind: "photographs"; subject: CaptureSubject; event: Uuid };

/** What the last scan turned out to be, or nothing yet. */
export interface ScanState {
  /** What is in the box, as typed or as the reader sent it. */
  typed: string;
  /** The last answer. Null before the first scan of a session. */
  found: Resolution | null;
  /** Bumped after each scan so `ScanInput` takes focus back — a stack of
   *  boxes is the common case and reaching for the field between them is the
   *  reach this screen exists to remove. */
  refocus: number;
}

export interface CaptureBench {
  status: CaptureStatus;
  stage: Stage;
  scan: ScanState;
  /** Type into the locator. */
  typeScan: (next: string) => void;
  /** Resolve what was scanned. Narrowed to items: this screen measures things
   *  of a kind, so a carton or a bin scanned here is reported rather than
   *  navigated to. */
  lookUp: (scanned: string) => Promise<void>;
  /** Forget the last scan and go back to the worklist. */
  clearScan: () => void;
  figures: Figures;
  /** Faces uploaded in this session, in the order they were taken. */
  taken: Face[];
  busy: boolean;
  problem: string | null;
  /** What the act said when it landed, worth showing once. */
  recorded: { measurements: number; warnings: string[] } | null;
  dismiss: () => void;
  choose: (subject: CaptureSubject) => void;
  /** Back to the worklist, discarding anything not yet recorded. */
  leave: () => void;
  type: (field: "weight" | "length" | "width" | "height", next: string) => void;
  /** D138. One of [`PRESENTATIONS`]. */
  choosePresentation: (code: string) => void;
  /** D138. Declare that this thing has no bounding box, or take it back. */
  toggleNoDimensions: () => void;
  record: () => Promise<void>;
  attach: (face: Face, image: Blob) => Promise<void>;
  finish: () => Promise<void>;

  /**
   * What the subject in hand already answers to (D164).
   *
   * Read when a subject is chosen rather than sent with the worklist: the
   * worklist is a page and a page costing a round trip per subject is what the
   * handheld cannot afford, and nobody needs a box's barcodes until they are
   * holding the box.
   */
  barcodes: BoundBarcode[];
  /** What is in the bind field, which is not the worklist's scan bar: that one
   *  answers *what is this*, and this one says *this is that*. */
  binding: string;
  typeBinding: (next: string) => void;
  /** How many base units one scan of it means, where the operator knows. */
  count: string;
  typeCount: (next: string) => void;
  bind: () => Promise<void>;
}


/**
 * The name of a capture press.
 *
 * **D133 made this one act; this is what makes it one act twice.** The whole
 * session — four figures, one `observation_event` — travels in one call, and
 * pressing Record again after a lost response has to arrive as the same act or
 * the box is measured twice. The name is the subject and what was typed:
 * re-measuring after correcting a figure is a different act, and rightly.
 *
 * It is a function rather than a line inside the run because `press` needs the
 * name before it starts, which is the point — an act is identified by what was
 * asked for, not by what came back.
 */
function recordKey(stage: Stage, figures: Figures): string {
  if (stage.kind === "worklist") return "capture:none";
  const s = stage.subject;
  const who = s.item_id ?? s.item_style_id ?? s.item_part_id;
  return `capture:${who}:${s.packaging_level ?? "part"}:${JSON.stringify(measurementsOf(figures))}`;
}

export function useCapture(): CaptureBench {
  const [status, setStatus] = useState<CaptureStatus>({ kind: "loading" });
  const [stage, setStage] = useState<Stage>({ kind: "worklist" });
  const [figures, setFigures] = useState<Figures>(NO_FIGURES);
  const [taken, setTaken] = useState<Face[]>([]);
  const [barcodes, setBarcodes] = useState<BoundBarcode[]>([]);
  const [binding, setBinding] = useState("");
  const [count, setCount] = useState("");
  const [recorded, setRecorded] = useState<CaptureBench["recorded"]>(null);
  const [scan, setScan] = useState<ScanState>({ typed: "", found: null, refocus: 0 });

  // **No re-read after a press.** `finish` re-reads, and it is the only one
  // that should: the worklist changes when a box is done with, not between the
  // four figures and the seven photographs of the same box.
  const { busy, problem, dismiss, press } = useWriting();

  const live = useLive();

  const reload = useCallback(async () => {
    try {
      const screen = await api.capture();
      if (live.current) setStatus({ kind: "ready", screen });
    } catch (error) {
      const message = reason(error, "Could not load the worklist.");
      if (live.current) setStatus({ kind: "failed", message });
    }
  }, []);

  useEffect(() => {
    setStatus({ kind: "loading" });
    void reload();
  }, [reload]);

  return {
    status,
    stage,
    figures,
    taken,
    busy,
    problem,
    recorded,

    scan,

    typeScan: (next) => setScan((c) => ({ ...c, typed: next })),

    /**
     * **`expect=item`, because this screen measures a kind of thing.**
     *
     * A carton scanned here is a particular physical object, and what a capture
     * session records is what a carton of these measures — different subjects
     * under D23. Narrowing means the operator is told that rather than being
     * taken somewhere they did not ask to go, which is D34's third step doing
     * the job it exists for.
     */
    lookUp: (scanned) =>
      press(`look-up:${scanned}`, async () => {
        const found = await api.resolve(scanned, "item");
        if (!live.current) return;
        setScan((c) => ({ typed: "", found, refocus: c.refocus + 1 }));

        // **One subject, one capture target: go straight in.** Two taps to
        // start measuring a box you are already holding is one tap too many,
        // and the whole argument for a scanner is that it removes the choosing.
        // Anything less certain than that draws the options instead.
        const only = found.subjects.length === 1 ? found.subjects[0] : undefined;
        if (only?.capture?.length === 1 && only.capture[0]) {
          const subject = only.capture[0];
          setStage({ kind: "figures", subject });
          setFigures(NO_FIGURES);
          setTaken([]);
          setRecorded(null);
        }
      }),

    clearScan: () =>
      setScan((c) => ({ typed: "", found: null, refocus: c.refocus + 1 })),

    dismiss: () => {
      dismiss();
      setRecorded(null);
    },

    choose: (subject) => {
      setStage({ kind: "figures", subject });
      setFigures(NO_FIGURES);
      setTaken([]);
      dismiss();
      setRecorded(null);
      // What it already answers to. Cleared first rather than left standing:
      // the previous subject's barcodes on this subject's screen is the worst
      // available answer, and it is what showing stale state looks like.
      setBarcodes([]);
      setBinding("");
      setCount("");
      if (subject.item_id) {
        void api
          .itemBarcodes(subject.item_id)
          .then((rows) => {
            if (live.current) setBarcodes(rows);
          })
          .catch(() => {
            // Silent. A binding list nobody could read is a section that does
            // not appear, and the figures are what this screen is for.
          });
      }
    },

    leave: () => {
      setStage({ kind: "worklist" });
      setFigures(NO_FIGURES);
      setTaken([]);
      setBarcodes([]);
      setBinding("");
      setCount("");
      dismiss();
      setRecorded(null);
      setScan((c) => ({ typed: "", found: null, refocus: c.refocus + 1 }));
    },

    type: (field, next) => setFigures((current) => ({ ...current, [field]: next })),

    choosePresentation: (code) =>
      setFigures((current) => ({ ...current, presentation: code })),

    // **Clearing the three fields is part of declaring the absence**, not a
    // tidy-up beside it. Leaving typed numbers behind a checked box is a screen
    // holding two contradictory answers and sending one of them.
    toggleNoDimensions: () =>
      setFigures((current) =>
        current.noDimensions
          ? { ...current, noDimensions: false }
          : { ...current, noDimensions: true, length: "", width: "", height: "" },
      ),

    /**
     * The one act. Everything typed goes in a single request, and the event it
     * returns is what the photographs then hang off.
     */
    record: () =>
      press(recordKey(stage, figures), async (act) => {
        if (stage.kind !== "figures") return;
        const measurements = measurementsOf(figures);
        if (measurements.length === 0) {
          throw new ApiError("Nothing has been measured yet.", 400);
        }
        const subject = stage.subject;
        // **Refused here rather than by the server**, because the server's
        // refusal arrives after a round trip against a screen the operator has
        // already walked away from. Same rule, said sooner.
        const needsArrangement =
          presentationNeeded(subject) &&
          measurements.some((m) => !m.absent_reason && m.metric !== "gross_weight");
        if (needsArrangement && !figures.presentation) {
          throw new ApiError(
            "Choose an arrangement: folded or flat.",
            400,
          );
        }
        const response = await api.recordCapture({
          ...(subject.item_id ? { item: subject.item_id } : {}),
          ...(subject.item_style_id ? { style: subject.item_style_id } : {}),
          ...(subject.item_part_id ? { part: subject.item_part_id } : {}),
          ...(figures.presentation ? { presentation: figures.presentation } : {}),
          level: subject.packaging_level,
          measurements,
          act,
        });
        if (live.current) {
          setStage({
            kind: "photographs",
            subject,
            event: response.observation_event_id,
          });
          setRecorded({
            measurements: response.observation_ids.length,
            warnings: response.warnings,
          });
        }
      }),

    /**
     * **Uploaded one at a time, as it is taken.** Holding seven images in
     * memory to send at the end is the one thing on this hardware that is
     * expensive to hold, and an upload that fails is then a photograph to
     * retake rather than a session to redo. A retake is a new row (D132), so
     * pressing a face twice is safe.
     */
    attach: (face, image) =>
      press(`attach:${face}`, async () => {
        if (stage.kind !== "photographs") return;
        await api.photograph(stage.event, face, image);
        if (live.current) {
          setTaken((current) => (current.includes(face) ? current : [...current, face]));
        }
      }),

    /** Done with this box. The worklist is re-read, because it has changed. */
    finish: () =>
      press("finish", async () => {
        await reload();
        if (live.current) {
          setStage({ kind: "worklist" });
          setFigures(NO_FIGURES);
          setTaken([]);
          setBarcodes([]);
          setBinding("");
          setCount("");
          setScan((c) => ({ typed: "", found: null, refocus: c.refocus + 1 }));
        }
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
     * Say that this label means this box (D164).
     *
     * **The level comes from the subject rather than from a chooser.** The
     * operator is capturing a carton or an each and the label in their hand is
     * on the thing they are capturing; asking them to say which again is asking
     * a question whose answer is already on the screen, and it is a question
     * they could answer wrongly.
     *
     * A part is not offered this at all: a part has no packaging level, and the
     * writer would refuse one.
     */
    bind: () =>
      press(`bind:${binding.trim()}`, async () => {
        if (stage.kind === "worklist") return;
        const subject = stage.subject;
        const level = subject.packaging_level;
        if (!subject.item_id || !level) {
          throw new ApiError("Parts have no packaging level to bind a barcode to.", 400);
        }
        const scanned = binding.trim();
        if (!scanned) return;
        const typed = count.trim();
        const bound = await api.bindBarcode(subject.item_id, {
          scan: scanned,
          packaging_level: level,
          // **A blank is not a zero and not a count.** The same rule the
          // figures follow: an operator who does not know the case pack has
          // said nothing about it, and the request should say that rather than
          // send a number nobody typed.
          quantity: typed === "" ? null : Number(typed),
        });
        if (!live.current) return;
        setBinding("");
        setCount("");
        // Re-read rather than appending the response: the row the screen draws
        // should be the row the server holds, and `already` says whether this
        // act added one at all.
        const rows = await api.itemBarcodes(subject.item_id);
        if (live.current) {
          setBarcodes(rows);
          setRecorded(
            bound.already
              ? { measurements: 0, warnings: ["that label was already bound to this box"] }
              : { measurements: 0, warnings: bound.warnings },
          );
        }
      }),
  };
}
