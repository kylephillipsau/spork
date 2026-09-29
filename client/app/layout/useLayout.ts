import { useCallback, useEffect, useState } from "react";

import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { DraftReport, LayoutView } from "@domain/types";

import type { Read } from "./usePlace";

/**
 * Drafting a layout from the bin list (D173). Always previewed first: the
 * preview is the apply rolled back, so what it shows is what applying does.
 */
export type DraftState =
  | { kind: "idle" }
  | { kind: "working"; what: "preview" | "apply" }
  | { kind: "previewed"; report: DraftReport }
  | { kind: "applied"; report: DraftReport }
  | { kind: "failed"; message: string };

export interface LayoutDesk {
  read: Read<LayoutView>;
  draft: DraftState;
  preview: () => Promise<void>;
  apply: () => Promise<void>;
  dismiss: () => void;
}

export function useLayout(): LayoutDesk {
  const live = useLive();
  const [read, setRead] = useState<Read<LayoutView>>({ kind: "loading" });
  const [draft, setDraft] = useState<DraftState>({ kind: "idle" });

  const refresh = useCallback(async () => {
    try {
      const value = await api.layout();
      if (live.current) setRead({ kind: "ready", value });
    } catch (error) {
      if (live.current) setRead({ kind: "failed", message: reason(error, "Could not load the layout.") });
    }
  }, [live]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = useCallback(
    async (apply: boolean) => {
      setDraft({ kind: "working", what: apply ? "apply" : "preview" });
      try {
        const report = await api.draftLayout({ apply });
        if (!live.current) return;
        setDraft(apply ? { kind: "applied", report } : { kind: "previewed", report });
        if (apply) await refresh();
      } catch (error) {
        if (live.current) setDraft({ kind: "failed", message: reason(error, "The draft did not run.") });
      }
    },
    [live, refresh],
  );

  const preview = useCallback(() => run(false), [run]);
  const apply = useCallback(() => run(true), [run]);
  const dismiss = useCallback(() => setDraft({ kind: "idle" }), []);

  return { read, draft, preview, apply, dismiss };
}
