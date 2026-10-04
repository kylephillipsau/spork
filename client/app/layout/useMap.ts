import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useLive } from "@app/acting";
import { useChanges } from "@app/changes";
import { href } from "@app/routing/location";
import { api, reason } from "@domain/api";
import type { BinRow, LayoutPlace, LayoutView, MapBin, MapBins, Uuid } from "@domain/types";

import { LAYERS, type Layer } from "./layers";
import type { Read } from "./usePlace";

/**
 * The bin map (D208): every bin on the site in its cell, coloured by a layer,
 * with a search that flies to one and a card for the one chosen.
 *
 * The choice and the layer live in the address (`?bin=`, `?layer=`), so a bin
 * can be linked to. The layer is also remembered by the browser, so the map
 * opens the way it was last read.
 */

export interface MapSite {
  layout: LayoutView;
  bins: MapBins;
}

export interface MapDesk {
  read: Read<MapSite>;
  layer: Layer;
  setLayer: (layer: Layer) => void;
  /** The bin chosen, and its place. */
  chosen: MapBin | null;
  place: LayoutPlace | null;
  choose: (locationId: Uuid | null) => void;
  /** Choose a bin and fly to it. */
  find: (bin: MapBin) => void;
  /** Bumped each time the view should fly to what is chosen. */
  flight: number;
  /** What the chosen bin holds, by both records. */
  detail: Read<BinRow | null> | { kind: "idle" };
  /** Say how many of the chosen bin's rack's levels are reached from the floor. */
  setReach: (levels: number) => Promise<void>;
  busy: boolean;
  problem: string | null;
  dismiss: () => void;
}

const REMEMBERED = "spork.map.layer";

export function mapFrom(search: string): { bin: Uuid | null; layer: Layer | null } {
  const p = new URLSearchParams(search);
  const layer = p.get("layer");
  return { bin: p.get("bin") || null, layer: LAYERS.some((l) => l.id === layer) ? (layer as Layer) : null };
}

function remembered(): Layer {
  try {
    const was = localStorage.getItem(REMEMBERED);
    return LAYERS.some((l) => l.id === was) ? (was as Layer) : "stock";
  } catch {
    return "stock";
  }
}

export function useMap(initial: { bin: Uuid | null; layer: Layer | null } = { bin: null, layer: null }): MapDesk {
  const live = useLive();
  const [read, setRead] = useState<Read<MapSite>>({ kind: "loading" });
  const [layer, setLayerState] = useState<Layer>(() => initial.layer ?? remembered());
  const [chosenId, setChosenId] = useState<Uuid | null>(initial.bin);
  // A bin named in the address is flown to once the map has it.
  const [flight, setFlight] = useState(initial.bin ? 1 : 0);
  const [detail, setDetail] = useState<MapDesk["detail"]>({ kind: "idle" });
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const asking = useRef(0);

  const load = useCallback(async () => {
    try {
      const [layout, bins] = await Promise.all([api.layout(), api.mapBins()]);
      if (live.current) setRead({ kind: "ready", value: { layout, bins } });
    } catch (error) {
      if (live.current) setRead({ kind: "failed", message: reason(error, "Could not load the map.") });
    }
  }, [live]);

  useEffect(() => {
    void load();
  }, [load]);

  // Stock moves as people pick and put away (D206); the map follows quietly.
  useChanges(() => void load());

  const chosen = useMemo(
    () => (read.kind === "ready" && chosenId ? (read.value.bins.bins.find((b) => b.location_id === chosenId) ?? null) : null),
    [read, chosenId],
  );
  const place = useMemo(
    () => (read.kind === "ready" && chosen ? (read.value.layout.places.find((p) => p.place_id === chosen.place_id) ?? null) : null),
    [read, chosen],
  );

  useEffect(() => {
    const p = new URLSearchParams();
    if (chosenId) p.set("bin", chosenId);
    p.set("layer", layer);
    window.history.replaceState(null, "", href(`/map?${p.toString()}`));
  }, [chosenId, layer]);

  // The chosen bin's contents, read on their own: the map reads only counts.
  const code = chosen?.code ?? null;
  useEffect(() => {
    if (!code) {
      setDetail({ kind: "idle" });
      return;
    }
    const n = ++asking.current;
    setDetail({ kind: "loading" });
    api
      .bins({ q: code })
      .then((list) => {
        if (live.current && n === asking.current) {
          setDetail({ kind: "ready", value: list.bins.find((b) => b.code === code) ?? null });
        }
      })
      .catch((error) => {
        if (live.current && n === asking.current) setDetail({ kind: "failed", message: reason(error, "Could not read what is in it.") });
      });
  }, [code, live]);

  return {
    read,
    layer,
    setLayer: (next) => {
      setLayerState(next);
      try {
        localStorage.setItem(REMEMBERED, next);
      } catch {
        /* remembered for this visit only */
      }
    },
    chosen,
    place,
    choose: (id) => {
      setChosenId(id);
      setProblem(null);
    },
    find: (bin) => {
      setChosenId(bin.location_id);
      setProblem(null);
      setFlight((n) => n + 1);
    },
    flight,
    detail,
    setReach: async (levels) => {
      if (!place) return;
      setBusy(true);
      setProblem(null);
      try {
        await api.setReach(place.place_id, levels);
        await load();
      } catch (error) {
        if (live.current) setProblem(reason(error, "Could not say how far up can be reached."));
      } finally {
        if (live.current) setBusy(false);
      }
    },
    busy,
    problem,
    dismiss: () => setProblem(null),
  };
}
