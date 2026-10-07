import { useEffect, useState } from "react";
import { useLive } from "@app/acting";
import { useSite } from "@app/session/SessionContext";
import { codesFrom } from "@app/items/lists";
import { keep, recall } from "@app/common/remembered";
import { api, reason, toPickQuery, type ToPickAsk } from "@domain/api";
import type { ToPick } from "@domain/types";

/**
 * A batch of picking tickets (D231): pasted from the sheet they went out on,
 * looked up against what NetSuite has still to pick, and shared out between
 * the people picking (D230). With nothing pasted, everything open here.
 */

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
  };
}
