import { useCallback, useEffect, useState } from "react";
import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { PackJob, Stage } from "@domain/types";

/**
 * What there is to pack, as logic.
 *
 * The landing page of the server-rendered application, and the screen an
 * operator has open all day. It opens on the work rather than on a search box —
 * one of the few things the old pages got right and worth keeping.
 */

export type QueueState =
  | { kind: "loading" }
  | { kind: "ready"; jobs: PackJob[] }
  | { kind: "failed"; message: string };

export interface QueueBench {
  state: QueueState;
  term: string;
  type: (next: string) => void;
  search: () => Promise<void>;
}

/** The order the groups are drawn in. Not alphabetical, not the enum's: it is
 *  the order work moves through, so the eye goes to the top of the queue. */
export const STAGES: readonly Stage[] = ["ready", "on_the_bench", "packed", "nothing_committed"];

/** What each group is called. The server names the stage; this names the
 *  heading, which is formatting rather than judgement (D114). */
export const STAGE_LABELS: Readonly<Record<Stage, string>> = {
  ready: "Ready to pack",
  on_the_bench: "On the bench",
  packed: "Packed",
  nothing_committed: "Nothing committed",
};

export function useQueue(): QueueBench {
  const [term, setTerm] = useState("");
  const [state, setState] = useState<QueueState>({ kind: "loading" });

  const live = useLive();

  const search = useCallback(async () => {
    setState({ kind: "loading" });
    try {
      const jobs = await api.packingQueue(term);
      if (live.current) setState({ kind: "ready", jobs });
    } catch (error) {
      if (live.current) {
        setState({
          kind: "failed",
          message: reason(error, "The queue could not be read."),
        });
      }
    }
  }, [term]);

  useEffect(() => {
    void api
      .packingQueue("")
      .then((jobs) => live.current && setState({ kind: "ready", jobs }))
      .catch((error: unknown) => {
        if (!live.current) return;
        setState({
          kind: "failed",
          message: reason(error, "The queue could not be read."),
        });
      });
  }, []);

  return { state, term, type: setTerm, search };
}
