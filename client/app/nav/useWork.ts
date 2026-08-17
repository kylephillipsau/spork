import { useCallback, useEffect, useState } from "react";
import { useLive } from "@app/acting";
import { api } from "@domain/api";
import type { WorkWaiting } from "@domain/types";
import type { BadgeKey } from "./rail";

/**
 * The counts the rail's badges show.
 *
 * **The same read the landing screen makes**, so the number beside Pack in the
 * rail and the number on `/` cannot disagree. Two endpoints would be two
 * computations of one fact.
 *
 * Refreshed on navigation rather than polled. A badge that updates on its own
 * timer is a badge that changes while somebody is looking at the row it labels;
 * arriving at a screen is the moment its count is worth being right.
 *
 * **The screen, not the path**, and the difference arrived with
 * `/findings/:finding`. Choosing a row on the findings queue is a navigation
 * now, so keyed on the path this read fired once per row an operator looked at:
 * twenty reads of `/work` to triage twenty findings. Keyed on the screen,
 * opening the first one is an arrival and the nineteen after it are not — one
 * read, which is what "on arrival" meant before a route carried a subject.
 *
 * A failure is silent. A badge is an aid to navigation, and an error message in
 * the chrome about a count is noise on every screen at once.
 */
export function useWork(screen: string): Readonly<Partial<Record<BadgeKey, number>>> {
  const [counts, setCounts] = useState<Partial<Record<BadgeKey, number>>>({});
  const live = useLive();

  const read = useCallback(async () => {
    try {
      const w: WorkWaiting = await api.work();
      if (!live.current) return;
      setCounts(
        w.no_site
          ? {}
          : { pack: w.pack, pick: w.pick, despatch: w.despatch, findings: w.findings },
      );
    } catch {
      /* a count nobody could read is a badge that does not appear */
    }
  }, []);

  useEffect(() => {
    void read();
  }, [read, screen]);

  return counts;
}
