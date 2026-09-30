import { useCallback, useEffect, useRef, useState } from "react";

import { useLive } from "@app/acting";
import { href } from "@app/routing/location";
import { api, reason } from "@domain/api";
import type { BinsList, DraftReport, LayoutView } from "@domain/types";

import type { Read } from "./usePlace";

/**
 * The warehouse, as logic: the site's layout, a first draft of it from the bin
 * list, and the bins of whichever place is chosen (D173).
 *
 * The draft is always previewed first: the preview is the apply rolled back,
 * so what it shows is what applying does.
 *
 * **What is chosen lives in the address**, as the item list's question does:
 * `?place=` names a place or `unplaced`, and `?q=` a search for a bin across
 * the site. It is rewritten in place, so choosing neither scrolls nor piles up
 * history.
 */
export type DraftState =
  | { kind: "idle" }
  | { kind: "working"; what: "preview" | "apply" }
  | { kind: "previewed"; report: DraftReport }
  | { kind: "applied"; report: DraftReport }
  | { kind: "failed"; message: string };

/** A place by its id, or the bins on no place at all. */
export type Chosen = string | "unplaced" | null;

export interface WarehouseDesk {
  read: Read<LayoutView>;
  draft: DraftState;
  preview: () => Promise<void>;
  apply: () => Promise<void>;
  dismiss: () => void;
  chosen: Chosen;
  choose: (next: Chosen) => void;
  /** The bins of what is chosen, or of the search when there is one. */
  bins: Read<BinsList> | { kind: "idle" };
  asked: string;
  typed: string;
  type: (q: string) => void;
  search: () => void;
}

export function chosenFrom(search: string): { chosen: Chosen; q: string } {
  const p = new URLSearchParams(search);
  return { chosen: p.get("place") || null, q: p.get("q") ?? "" };
}

export function useWarehouse(initial: { chosen: Chosen; q: string } = { chosen: null, q: "" }): WarehouseDesk {
  const live = useLive();
  const [read, setRead] = useState<Read<LayoutView>>({ kind: "loading" });
  const [draft, setDraft] = useState<DraftState>({ kind: "idle" });
  const [chosen, setChosen] = useState<Chosen>(initial.chosen);
  const [asked, setAsked] = useState(initial.q);
  const [typed, setTyped] = useState(initial.q);
  const [bins, setBins] = useState<WarehouseDesk["bins"]>({ kind: "idle" });
  const asking = useRef(0);

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

  // Nothing chosen yet: the first place that holds bins, so the screen opens
  // on something rather than on a question.
  useEffect(() => {
    if (chosen !== null || read.kind !== "ready") return;
    const first = read.value.places.find((p) => p.bins > 0);
    if (first) setChosen(first.place_id);
  }, [chosen, read]);

  useEffect(() => {
    const p = new URLSearchParams();
    if (chosen) p.set("place", chosen);
    if (asked.trim()) p.set("q", asked.trim());
    const qs = p.toString();
    window.history.replaceState(null, "", href(`/warehouse${qs ? `?${qs}` : ""}`));

    const q = asked.trim();
    if (!q && !chosen) {
      setBins({ kind: "idle" });
      return;
    }
    const n = ++asking.current;
    setBins({ kind: "loading" });
    const query = q ? { q } : chosen === "unplaced" ? { unplaced: true } : { place: chosen! };
    api
      .bins(query)
      .then((value) => {
        if (live.current && n === asking.current) setBins({ kind: "ready", value });
      })
      .catch((error) => {
        if (live.current && n === asking.current) setBins({ kind: "failed", message: reason(error, "Could not load the bins.") });
      });
  }, [chosen, asked, live]);

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

  return {
    read,
    draft,
    preview,
    apply,
    dismiss,
    chosen,
    // Choosing a place clears a search, which would otherwise hide it.
    choose: (next) => {
      setChosen(next);
      setAsked("");
      setTyped("");
    },
    bins,
    asked,
    typed,
    type: setTyped,
    search: () => setAsked(typed),
  };
}
