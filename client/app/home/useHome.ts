import { useCallback, useEffect, useState } from "react";
import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { WorkWaiting } from "@domain/types";

/**
 * What is waiting for you, as logic.
 *
 * One read, shared with the rail's badges when those arrive, so a count cannot
 * appear twice with two answers.
 */

export type HomeState =
  | { kind: "loading" }
  | { kind: "ready"; work: WorkWaiting }
  | { kind: "failed"; message: string };

export interface HomeBench {
  state: HomeState;
  refresh: () => Promise<void>;
}

export function useHome(): HomeBench {
  const [state, setState] = useState<HomeState>({ kind: "loading" });
  const live = useLive();

  const refresh = useCallback(async () => {
    try {
      const work = await api.work();
      if (live.current) setState({ kind: "ready", work });
    } catch (error) {
      if (live.current) {
        setState({
          kind: "failed",
          message: reason(error, "That could not be read."),
        });
      }
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { state, refresh };
}
