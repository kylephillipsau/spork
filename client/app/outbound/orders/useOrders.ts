import { useCallback, useEffect, useState } from "react";
import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { OrderMatch } from "@domain/types";

/**
 * Finding an order, as logic.
 *
 * Stage 1 of the recorded process: a customer quotes a number down the phone
 * and somebody has to find what it names. Today that is four screens across two
 * systems.
 *
 * **Searched on submit, never as you type.** A confirmation number is read off
 * a screen or heard down a phone and arrives all at once; a keystroke search
 * would fire eight queries for one question and race its own answers back.
 *
 * # A search box is not a way in
 *
 * The screen opened on an empty field, which is only usable by somebody who
 * already knows the answer. A person who has just been handed the job knows it
 * came in recently and nothing else. So the latest orders load on arrival, and
 * searching is what you do when the list is not enough.
 */

export type OrdersState =
  | { kind: "idle" }
  | { kind: "searching" }
  /** The latest, unasked for. Distinct from `found` so the screen can say which
   *  it is showing: a list of everything recent and a list of what matched a
   *  reference look identical and mean different things. */
  | { kind: "listed"; orders: OrderMatch[] }
  | { kind: "found"; reference: string; orders: OrderMatch[] }
  | { kind: "failed"; message: string };

export interface OrdersDesk {
  state: OrdersState;
  reference: string;
  type: (next: string) => void;
  search: () => Promise<void>;
  /** Back to the latest, from a search that answered or did not. */
  list: () => Promise<void>;
}

export function useOrders(initial = ""): OrdersDesk {
  const [reference, setReference] = useState(initial);
  const [state, setState] = useState<OrdersState>({ kind: "idle" });

  const live = useLive();

  const search = useCallback(async () => {
    const asked = reference.trim();
    if (!asked) return;
    setState({ kind: "searching" });
    try {
      const orders = await api.findOrders(asked);
      if (live.current) setState({ kind: "found", reference: asked, orders });
    } catch (error) {
      if (live.current) {
        setState({
          kind: "failed",
          message: reason(error, "Search failed."),
        });
      }
    }
  }, [reference]);

  const list = useCallback(async () => {
    setState({ kind: "searching" });
    try {
      const orders = await api.latestOrders();
      if (live.current) setState({ kind: "listed", orders });
    } catch (error) {
      if (live.current) {
        setState({
          kind: "failed",
          message: reason(error, "Could not load orders."),
        });
      }
    }
  }, []);

  // A deep link carries the reference, so the same URL finds the same order —
  // which is the property D135 said the router had to buy. Without one, the
  // latest are what the screen opens on.
  useEffect(() => {
    if (initial.trim()) void search();
    else void list();
    // Only on mount, and only for the reference the URL arrived with.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return { state, reference, type: setReference, search, list };
}
