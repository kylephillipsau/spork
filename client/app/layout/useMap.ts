import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useLive } from "@app/acting";
import { useSite } from "@app/session/SessionContext";
import { useChanges } from "@app/changes";
import { href } from "@app/routing/location";
import { api, reason } from "@domain/api";
import type { BinView, LayoutPlace, LayoutView, MapBin, MapBins, Uuid, WalkRoute } from "@domain/types";

import { keep, recall } from "@app/common/remembered";

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
  /** The chosen bin as its own read has it: where it is, and what is on it (D221). */
  detail: Read<BinView> | { kind: "idle" };
  /** Say how many of the chosen bin's rack's levels are reached from the floor. */
  setReach: (levels: number) => Promise<void>;
  busy: boolean;
  problem: string | null;
  dismiss: () => void;
  /** Today's picking walk, drawn on the floor when asked for (D211). */
  showWalk: boolean;
  setShowWalk: (on: boolean) => void;
  walk: Read<WalkRoute | null> | { kind: "idle" };
}

const REMEMBERED = "spork.map.layer";

export function mapFrom(search: string): { bin: Uuid | null; layer: Layer | null } {
  const p = new URLSearchParams(search);
  const layer = p.get("layer");
  return { bin: p.get("bin") || null, layer: LAYERS.some((l) => l.id === layer) ? (layer as Layer) : null };
}


export function useMap(initial: { bin: Uuid | null; layer: Layer | null } = { bin: null, layer: null }): MapDesk {
  const live = useLive();
  const [read, setRead] = useState<Read<MapSite>>({ kind: "loading" });
  const [layer, setLayerState] = useState<Layer>(() => initial.layer ?? recall(REMEMBERED, LAYERS.map((l) => l.id)) ?? "stock");
  const [chosenId, setChosenId] = useState<Uuid | null>(initial.bin);
  // A bin named in the address is flown to once the map has it.
  const [flight, setFlight] = useState(initial.bin ? 1 : 0);
  const [detail, setDetail] = useState<MapDesk["detail"]>({ kind: "idle" });
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const asking = useRef(0);
  const site = useSite();
  const [showWalk, setShowWalk] = useState(false);
  const [walk, setWalk] = useState<MapDesk["walk"]>({ kind: "idle" });

  const readWalk = useCallback(async () => {
    if (!site) return;
    try {
      const screen = await api.picking(site);
      if (live.current) setWalk({ kind: "ready", value: screen.route });
    } catch (error) {
      if (live.current) setWalk({ kind: "failed", message: reason(error, "Could not read today's walk.") });
    }
  }, [site, live]);

  useEffect(() => {
    if (!showWalk) {
      setWalk({ kind: "idle" });
      return;
    }
    setWalk({ kind: "loading" });
    void readWalk();
  }, [showWalk, readWalk]);

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

  // Stock moves as people pick, and racks as somebody edits the layout
  // (D206, D209); the message says where, never which, so both are read.
  useChanges(() => {
    void load();
    if (showWalk) void readWalk();
  });

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

  // The chosen bin, read on its own: the map reads only counts.
  const binId = chosen?.location_id ?? null;
  useEffect(() => {
    if (!binId) {
      setDetail({ kind: "idle" });
      return;
    }
    const n = ++asking.current;
    setDetail({ kind: "loading" });
    api
      .bin(binId)
      .then((value) => {
        if (live.current && n === asking.current) setDetail({ kind: "ready", value });
      })
      .catch((error) => {
        if (live.current && n === asking.current) setDetail({ kind: "failed", message: reason(error, "Could not read what is in it.") });
      });
  }, [binId, live]);

  return {
    read,
    layer,
    setLayer: (next) => {
      setLayerState(next);
      keep(REMEMBERED, next);
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
    showWalk,
    setShowWalk,
    walk,
  };
}
