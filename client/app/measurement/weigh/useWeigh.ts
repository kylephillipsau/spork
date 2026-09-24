import { useCallback, useEffect, useState } from "react";
import { useLive, useWriting } from "@app/acting";
import { ApiError, api, reason } from "@domain/api";
import type { ToWeigh, WeighingRecorded } from "@domain/types";

/**
 * The weighing bench, as logic.
 *
 * # One thing on the scale at a time
 *
 * The worklist is a queue and the screen shows its head, not a table of fifty
 * rows with fifty inputs. A bench operator has one scale and one pair of hands,
 * and a screen offering fifty places to type is a screen that has to be aimed
 * at before it can be used — which is the reach the whole surface exists to
 * remove. Skipping is how you get past one you cannot reach.
 *
 * # The ranking is the server's and stays the server's
 *
 * `crate::revalidation` decides never-measured before overdue, and ranks by how
 * often a thing is actually ordered — ABC analysis applied to weights instead
 * of quantities. Re-sorting here would be a second opinion about the same
 * question, and the two would drift. The client takes the order it is given.
 */

export type WeighStatus =
  | { kind: "loading" }
  | { kind: "ready"; queue: ToWeigh[] }
  | { kind: "failed"; message: string };

export interface WeighBench {
  status: WeighStatus;
  /** How far into the queue the operator has walked. */
  at: number;
  /** What the scale says, as typed. A string, never a number. */
  reading: string;
  /** `kg` or `g` — what the scale in front of them is set to. */
  unit: string;
  busy: boolean;
  problem: string | null;
  /** The last weighing, kept until the operator moves on. */
  recorded: (WeighingRecorded & { code: string }) | null;
  type: (next: string) => void;
  setUnit: (next: string) => void;
  /** Past this one without weighing it — it is on a shelf you cannot reach. */
  skip: () => void;
  dismiss: () => void;
  record: () => Promise<void>;
}

/** The head of the queue, or nothing left to do. */
export function current(status: WeighStatus, at: number): ToWeigh | null {
  return status.kind === "ready" ? (status.queue[at] ?? null) : null;
}

export function useWeigh(): WeighBench {
  const [status, setStatus] = useState<WeighStatus>({ kind: "loading" });
  const [at, setAt] = useState(0);
  const [reading, setReading] = useState("");
  const [unit, setUnit] = useState("kg");
  const [recorded, setRecorded] = useState<WeighBench["recorded"]>(null);

  // **No re-read after a weighing**, and the action below says why: a weighing
  // removes its subject from the worklist, so refetching would renumber
  // everything under the operator between one box and the next.
  const { busy, problem, dismiss, press } = useWriting();

  const live = useLive();

  const reload = useCallback(async () => {
    try {
      const queue = await api.toWeigh();
      if (live.current) setStatus({ kind: "ready", queue });
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
    at,
    reading,
    unit,
    busy,
    problem,
    recorded,

    type: setReading,
    setUnit,

    skip: () => {
      setAt((n) => n + 1);
      setReading("");
      dismiss();
      setRecorded(null);
    },

    dismiss: () => {
      dismiss();
      setRecorded(null);
    },

    record: async () => {
      const subject = current(status, at);
      if (!subject) return;
      const entered = reading.trim();

      // **The reading is in the key.** A weight put on the scale twice because
      // the first response was lost is one act; the same subject weighed again
      // after a correction is a different reading and a different act. See
      // `domain/acts.ts`.
      const key = `weigh:${subject.item_id ?? subject.item_style_id}:${subject.packaging_level}:${entered}`;

      await press(key, async (act) => {
        if (!entered) throw new ApiError("Read the scale first.", 400);

        const answer = await api.weigh({
          ...(subject.item_id ? { item: subject.item_id } : {}),
          ...(subject.item_style_id ? { style: subject.item_style_id } : {}),
          level: subject.packaging_level,
          entered,
          unit,
          act,
        });

        if (live.current) {
          setRecorded({ ...answer, code: subject.code });
          setReading("");
          // **On to the next one, and the queue is not re-read.** A weighing
          // removes its subject from the worklist, so refetching would
          // renumber everything under the operator between one box and the
          // next. Walking the list we were given is the only way the position
          // means anything.
          setAt((n) => n + 1);
        }
      });
    },
  };
}
