import { useEffect, useState } from "react";
import { useLive } from "@app/acting";
import { useSite } from "@app/session/SessionContext";
import { codesFrom } from "@app/items/lists";
import { keep, recall } from "@app/common/remembered";
import { readMapSite, type MapSite } from "@app/layout/useMap";
import { api, reason, toPickQuery, type ToPickAsk } from "@domain/api";
import type { Picture, PlannedTrip, ToPick } from "@domain/types";

/**
 * A batch of picking tickets (D231): pasted from the sheet they went out on,
 * looked up against what NetSuite has still to pick, and shared out between
 * the people picking (D230). With nothing pasted, everything open here.
 */

/**
 * A trip being walked: kept by the browser from the moment it starts, so the
 * walk doesn't change under the picker as NetSuite hears of their picks and
 * the batch is planned again, and a phone that reloads the tab keeps its place.
 * Nothing here is recorded in Spork: the picks are NetSuite's (D212).
 */
export interface TripWalk {
  /** "Group A · trip 1". */
  label: string;
  trip: PlannedTrip;
  /** What each product looks like, by code, from the batch's lines. */
  pictures: Record<string, Picture | null>;
  /** Who each order is for. */
  customers: Record<string, string | null>;
  /** The stop on screen, and the stops done. */
  at: number;
  done: number[];
}

const WALK = "spork.toPick.walk";

function recallWalk(): TripWalk | null {
  try {
    const kept = localStorage.getItem(WALK);
    return kept ? (JSON.parse(kept) as TripWalk) : null;
  } catch {
    return null;
  }
}

function keepWalk(walk: TripWalk | null): void {
  try {
    if (walk) localStorage.setItem(WALK, JSON.stringify(walk));
    else localStorage.removeItem(WALK);
  } catch {
    // kept for this visit only
  }
}

/** The warehouse as the 3D map draws it, read when a walk starts. */
export type MapRead = { kind: "loading" } | { kind: "ready"; site: MapSite } | { kind: "failed"; message: string };

export type ToPickState = { kind: "loading" } | { kind: "ready"; batch: ToPick } | { kind: "failed"; message: string };

export interface ToPickDesk {
  state: ToPickState;
  /** The paste box, as typed. */
  pasted: string;
  paste: (text: string) => void;
  /** The orders the batch on screen was asked for; none is everything open. */
  asked: readonly string[];
  lookUp: () => void;
  everything: () => void;
  pickers: number;
  setPickers: (n: number) => void;
  /** The most orders a trip takes; null for no limit. */
  perTrip: number | null;
  setPerTrip: (n: number | null) => void;
  /** One trip collects a shelf other groups want too. */
  gather: boolean;
  setGather: (on: boolean) => void;
  /** Where the printed tickets for the batch on screen are, with walk sheets or not. */
  printed: (walk: boolean) => string | null;
  /** The trip being walked, if one is, and the warehouse to draw it on. */
  walking: TripWalk | null;
  map: MapRead;
  walkTrip: (label: string, trip: PlannedTrip) => void;
  /** Go to a stop. */
  goTo: (stop: number) => void;
  /** The stop on screen is picked, or isn't after all. Got it moves on. */
  got: () => void;
  notGot: () => void;
  leaveWalk: () => void;
}

export const PICKERS = ["1", "2", "3", "4", "5", "6"] as const;
export const PER_TRIP = ["none", "2", "3", "4", "5", "6", "8", "10", "12"] as const;

const remembered = (key: string, choices: readonly string[], fallback: string) => recall(key, choices) ?? fallback;

export function useToPick(): ToPickDesk {
  const site = useSite();
  const live = useLive();
  const [pasted, paste] = useState("");
  const [asked, setAsked] = useState<readonly string[]>([]);
  const [pickers, setPickersState] = useState(() => Number(remembered("spork.toPick.pickers", PICKERS, "1")));
  const [perTrip, setPerTripState] = useState<number | null>(() => {
    const n = remembered("spork.toPick.perTrip", PER_TRIP, "none");
    return n === "none" ? null : Number(n);
  });
  const [gather, setGatherState] = useState(() => remembered("spork.toPick.gather", ["on", "off"], "off") === "on");
  const [state, setState] = useState<ToPickState>({ kind: "loading" });
  const [walking, setWalking] = useState<TripWalk | null>(recallWalk);
  const [map, setMap] = useState<MapRead>({ kind: "loading" });
  const isWalking = walking !== null;
  useEffect(() => {
    if (!isWalking) return;
    readMapSite().then(
      (site) => live.current && setMap({ kind: "ready", site }),
      (error: unknown) => live.current && setMap({ kind: "failed", message: reason(error, "Could not load the map.") }),
    );
  }, [isWalking]); // eslint-disable-line react-hooks/exhaustive-deps
  const walkAs = (next: TripWalk | null) => {
    keepWalk(next);
    setWalking(next);
  };

  const ask: ToPickAsk = { orders: asked, pickers, perTrip, gather };

  useEffect(() => {
    if (!site) {
      setState({ kind: "failed", message: "Choose the site you're working at first." });
      return;
    }
    setState({ kind: "loading" });
    api.toPick(site, ask).then(
      (batch) => live.current && setState({ kind: "ready", batch }),
      (error: unknown) => live.current && setState({ kind: "failed", message: reason(error, "Could not look the orders up.") }),
    );
  }, [site, asked, pickers, perTrip, gather]); // eslint-disable-line react-hooks/exhaustive-deps

  return {
    state,
    pasted,
    paste,
    asked,
    lookUp: () => setAsked(codesFrom(pasted.replace(/\s+/g, "\n"))),
    everything: () => {
      paste("");
      setAsked([]);
    },
    pickers,
    setPickers: (n) => {
      keep("spork.toPick.pickers", String(n));
      setPickersState(n);
    },
    perTrip,
    setPerTrip: (n) => {
      keep("spork.toPick.perTrip", n === null ? "none" : String(n));
      setPerTripState(n);
    },
    gather,
    setGather: (on) => {
      keep("spork.toPick.gather", on ? "on" : "off");
      setGatherState(on);
    },
    printed: (walk) => (site ? `/print/pick-tickets/${site}?${toPickQuery(ask, walk)}` : null),
    walking,
    map,
    walkTrip: (label, trip) => {
      const batch = state.kind === "ready" ? state.batch : null;
      const lines = batch?.orders.flatMap((o) => o.lines) ?? [];
      walkAs({
        label,
        trip,
        pictures: Object.fromEntries(lines.map((l) => [l.code, l.picture])),
        customers: Object.fromEntries((batch?.orders ?? []).map((o) => [o.number ?? o.asked, o.customer])),
        at: 0,
        done: [],
      });
    },
    goTo: (stop) => walking && walkAs({ ...walking, at: stop }),
    got: () => {
      if (!walking) return;
      const done = [...new Set([...walking.done, walking.at])];
      // On to the next stop not yet done, after this one, then from the start.
      const n = walking.trip.stops.length;
      const next = [...Array(n).keys()].map((k) => (walking.at + 1 + k) % n).find((i) => !done.includes(i));
      walkAs({ ...walking, at: next ?? walking.at, done });
    },
    notGot: () => walking && walkAs({ ...walking, done: walking.done.filter((d) => d !== walking.at) }),
    leaveWalk: () => walkAs(null),
  };
}
