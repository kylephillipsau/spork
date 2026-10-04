import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useLive, useWriting } from "@app/acting";
import { guardLeaving } from "@app/routing/location";
import { api, reason } from "@domain/api";
import { uuid } from "@domain/acts";
import type { LayoutView, PlanShape, Uuid } from "@domain/types";

import { changeCount, changesOf, draftsOf, freeName, planOf, type Draft, type PlaceBox, type Preset } from "./edit";
import type { Read } from "./usePlace";

/**
 * The plan editor (D209): the site's places, moved, turned, resized, drawn
 * and taken away on the plan, and saved together as one act.
 *
 * **Nothing is saved until Save.** The changes are held here, with an undo for
 * each, so nobody sees a half-moved warehouse and a slip of the mouse costs a
 * press of Undo. Save sends them all against the layout as it was read; if
 * somebody else changed it meanwhile, the save is refused and says so.
 */

export interface PlanDesk {
  read: Read<LayoutView>;
  drafts: Draft[];
  /** The plan as the changes leave it. */
  plan: PlanShape[];
  selected: Draft | null;
  select: (placeId: Uuid | null) => void;
  /** Change a place, as one step to undo. */
  change: (placeId: Uuid, next: (d: Draft) => Draft) => void;
  /** A drag begins: one undo step for the whole of it. */
  begin: () => void;
  /** A drag goes on: the box as it is now, with no step of its own. */
  drag: (placeId: Uuid, box: PlaceBox) => void;
  add: (preset: Preset, at: [number, number]) => void;
  remove: (placeId: Uuid) => void;
  undo: () => void;
  redo: () => void;
  canUndo: boolean;
  canRedo: boolean;
  /** How many places a save would change. */
  count: number;
  /** How long a cell is, once the site says; null while the plan isn't to scale (D210). */
  cellMm: number | null;
  /** Measure in metres from now on: a cell is a metre. Once. */
  measureInMetres: () => Promise<void>;
  save: () => Promise<void>;
  discard: () => void;
  busy: boolean;
  problem: string | null;
  said: string | null;
  dismiss: () => void;
}

export function usePlanEditor(): PlanDesk {
  const live = useLive();
  const [read, setRead] = useState<Read<LayoutView>>({ kind: "loading" });
  const [drafts, setDrafts] = useState<Draft[]>([]);
  const [selectedId, setSelectedId] = useState<Uuid | null>(null);
  const [past, setPast] = useState<Draft[][]>([]);
  const [future, setFuture] = useState<Draft[][]>([]);
  const [said, setSaid] = useState<string | null>(null);
  const { busy, problem, dismiss, press, say } = useWriting();
  const latest = useRef(drafts);
  latest.current = drafts;

  const load = useCallback(async () => {
    try {
      const value = await api.layout();
      if (!live.current) return;
      setRead({ kind: "ready", value });
      setDrafts(draftsOf(value));
      setPast([]);
      setFuture([]);
    } catch (error) {
      if (live.current) setRead({ kind: "failed", message: reason(error, "Could not load the layout.") });
    }
  }, [live]);

  useEffect(() => {
    void load();
  }, [load]);

  const view = read.kind === "ready" ? read.value : null;
  const plan = useMemo(() => planOf(drafts), [drafts]);
  const count = useMemo(() => (view ? changeCount(view, drafts) : 0), [view, drafts]);

  // Leaving with changes not saved asks first: closing the tab or reloading,
  // and going to another screen in the app, which the browser doesn't see.
  useEffect(() => {
    if (count === 0) return;
    const ask = (e: BeforeUnloadEvent) => e.preventDefault();
    window.addEventListener("beforeunload", ask);
    guardLeaving(() =>
      window.confirm(`Leave without saving? ${count === 1 ? "1 change" : `${count} changes`} to the layout will be lost.`),
    );
    return () => {
      window.removeEventListener("beforeunload", ask);
      guardLeaving(null);
    };
  }, [count]);

  const step = useCallback((next: Draft[]) => {
    setPast((p) => [...p.slice(-99), latest.current]);
    setFuture([]);
    setDrafts(next);
    setSaid(null);
  }, []);

  return {
    read,
    drafts,
    plan,
    selected: drafts.find((d) => d.place_id === selectedId) ?? null,
    select: setSelectedId,
    change: (id, next) => step(latest.current.map((d) => (d.place_id === id ? next(d) : d))),
    begin: () => {
      setPast((p) => [...p.slice(-99), latest.current]);
      setFuture([]);
      setSaid(null);
    },
    drag: (id, box) => setDrafts((all) => all.map((d) => (d.place_id === id ? { ...d, box } : d))),
    add: (preset, [x, y]) => {
      const all = latest.current;
      // Inside the outermost place, where it was asked for.
      const parent = all.find((d) => d.parent_id === null) ?? null;
      const place_id = uuid();
      step([
        ...all,
        {
          place_id,
          parent_id: parent?.place_id ?? null,
          name: freeName(all, parent?.place_id ?? null, preset.name),
          solid: preset.solid,
          box: { x, y, z: 0, length: preset.length, depth: preset.depth, height: preset.height, turn: 0 },
          outline: null,
          sides: 1,
          bins: 0,
          fresh: true,
        },
      ]);
      setSelectedId(place_id);
    },
    remove: (id) => {
      step(latest.current.filter((d) => d.place_id !== id));
      setSelectedId(null);
    },
    undo: () => {
      const before = past[past.length - 1];
      if (!before) return;
      setFuture((f) => [latest.current, ...f]);
      setPast((p) => p.slice(0, -1));
      setDrafts(before);
    },
    redo: () => {
      const after = future[0];
      if (!after) return;
      setPast((p) => [...p, latest.current]);
      setFuture((f) => f.slice(1));
      setDrafts(after);
    },
    canUndo: past.length > 0,
    canRedo: future.length > 0,
    count,
    cellMm: view?.cell_mm ?? null,
    measureInMetres: async () => {
      await press("scale:1000", async () => {
        await api.setScale(1000);
        if (!live.current) return;
        // The scale is the site's, not the drawing's: the changes stand.
        setRead((r) => (r.kind === "ready" ? { kind: "ready", value: { ...r.value, cell_mm: 1000 } } : r));
        setSaid("The plan is in metres now. Measure each rack and set it, and the rest follows.");
      });
    },
    save: async () => {
      if (!view || count === 0) return;
      const c = changesOf(view, latest.current);
      // The same changes against the same read are the same act, so a retry
      // after a lost answer is not a second wall.
      await press(`layout:${view.version}:${JSON.stringify(c)}`, async (act) => {
        const done = await api.editLayout({ version: view.version, ...c, act });
        if (!live.current) return;
        await load();
        setSaid(savedSentence(done.changed, done.added, done.removed));
      });
    },
    discard: () => {
      if (!view) return;
      step(draftsOf(view));
      setSelectedId(null);
    },
    busy,
    problem,
    said,
    dismiss: () => {
      dismiss();
      say(null);
      setSaid(null);
    },
  };
}

/** What a save did, in a sentence. */
export function savedSentence(changed: number, added: number, removed: number): string {
  const parts = [
    changed > 0 ? `${changed} ${changed === 1 ? "place" : "places"} moved or changed` : null,
    added > 0 ? `${added} added` : null,
    removed > 0 ? `${removed} taken away` : null,
  ].filter(Boolean);
  return parts.length > 0 ? `Saved: ${parts.join(", ")}.` : "Saved.";
}
