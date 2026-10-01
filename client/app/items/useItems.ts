import { useCallback, useEffect, useRef, useState } from "react";

import { useLive } from "@app/acting";
import { href } from "@app/routing/location";
import { api, reason } from "@domain/api";
import type { ItemRow } from "@domain/types";

/**
 * The item list, as logic.
 *
 * **Searched on submit, narrowed at once.** A code or a barcode arrives whole,
 * typed or scanned, and a query per keystroke would race its own answers back;
 * the narrowings are one click each and mean nothing to wait for.
 *
 * **The question lives in the address**, and so does the item open beside it,
 * so a link to "in stock here, needs weighing, most ordered first" opens on
 * that list, and Back returns to it. It is rewritten in place rather than
 * navigated, so changing a filter neither scrolls to the top nor piles up
 * history. The Weigh and Capture screens were these lists (D174).
 */

export type Stock = "" | "here";
export type Needs = "" | "weighing" | "measuring" | "photo";
export type Order = "" | "demand" | "walk";

export interface Asked {
  q: string;
  stock: Stock;
  needs: Needs;
  order: Order;
}

export type ItemsState =
  | { kind: "loading" }
  | { kind: "ready"; items: ItemRow[]; total: number; next: string | null; more: boolean }
  | { kind: "failed"; message: string };

export interface ItemsDesk {
  state: ItemsState;
  asked: Asked;
  /** What is in the search box, which is not asked until it is submitted. */
  typed: string;
  type: (q: string) => void;
  search: () => void;
  narrow: (next: Partial<Omit<Asked, "q">>) => void;
  more: () => Promise<void>;
  /** The item open beside the list, by id. */
  chosen: string | null;
  choose: (itemId: string | null) => void;
}

const NEEDS: readonly Needs[] = ["weighing", "measuring", "photo"];
const ORDERS: readonly Order[] = ["demand", "walk"];

/** The question in a URL's query string, and back. */
export function askedFrom(search: string): Asked & { item: string | null } {
  const p = new URLSearchParams(search);
  const needs = p.get("needs") as Needs;
  const order = p.get("order") as Order;
  return {
    q: p.get("q") ?? "",
    stock: p.get("stock") === "here" ? "here" : "",
    needs: NEEDS.includes(needs) ? needs : "",
    order: ORDERS.includes(order) ? order : "",
    item: p.get("item"),
  };
}

function queryOf(a: Asked): { q?: string; stock?: "here"; needs?: Exclude<Needs, "">; order?: Exclude<Order, ""> } {
  return {
    ...(a.q.trim() ? { q: a.q.trim() } : {}),
    ...(a.stock ? { stock: a.stock } : {}),
    ...(a.needs ? { needs: a.needs } : {}),
    ...(a.order ? { order: a.order } : {}),
  };
}

function addressOf(a: Asked, item: string | null): string {
  const p = new URLSearchParams(queryOf(a));
  if (item) p.set("item", item);
  const s = p.toString();
  return `/items${s ? `?${s}` : ""}`;
}

export function useItems(initial: Asked & { item?: string | null }): ItemsDesk {
  const live = useLive();
  const [asked, setAsked] = useState<Asked>({ q: initial.q, stock: initial.stock, needs: initial.needs, order: initial.order });
  const [typed, setTyped] = useState(initial.q);
  const [chosen, setChosen] = useState<string | null>(initial.item ?? null);
  const [state, setState] = useState<ItemsState>({ kind: "loading" });
  // The newest question wins: an answer to one asked before it is dropped.
  const asking = useRef(0);

  useEffect(() => {
    window.history.replaceState(null, "", href(addressOf(asked, chosen)));
  }, [asked, chosen]);

  useEffect(() => {
    const n = ++asking.current;
    setState({ kind: "loading" });
    api
      .items(queryOf(asked))
      .then((page) => {
        if (live.current && n === asking.current)
          setState({ kind: "ready", items: page.items, total: page.total, next: page.next, more: false });
      })
      .catch((error) => {
        if (live.current && n === asking.current) setState({ kind: "failed", message: reason(error, "Could not load items.") });
      });
  }, [asked, live]);

  const more = useCallback(async () => {
    if (state.kind !== "ready" || !state.next || state.more) return;
    const n = asking.current;
    setState({ ...state, more: true });
    try {
      const page = await api.items({ ...queryOf(asked), after: state.next });
      if (live.current && n === asking.current)
        setState({ kind: "ready", items: [...state.items, ...page.items], total: page.total, next: page.next, more: false });
    } catch (error) {
      if (live.current && n === asking.current) setState({ kind: "failed", message: reason(error, "Could not load more items.") });
    }
  }, [state, asked, live]);

  return {
    state,
    asked,
    typed,
    type: setTyped,
    search: () => setAsked((a) => ({ ...a, q: typed })),
    narrow: (next) => setAsked((a) => ({ ...a, ...next })),
    more,
    chosen,
    choose: setChosen,
  };
}
