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
 * the two narrowings are one click each and mean nothing to wait for.
 *
 * **The question lives in the address**, so a link to "in stock here, needs a
 * photo" opens on that list, and Back returns to it. It is rewritten in place
 * rather than navigated, so changing a filter neither scrolls to the top nor
 * piles up history.
 */

export type Stock = "" | "here";
export type Needs = "" | "measuring" | "photo";

export interface Asked {
  q: string;
  stock: Stock;
  needs: Needs;
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
}

/** The question in a URL's query string, and back. */
export function askedFrom(search: string): Asked {
  const p = new URLSearchParams(search);
  const stock = p.get("stock") === "here" ? "here" : "";
  const needs = p.get("needs");
  return { q: p.get("q") ?? "", stock, needs: needs === "measuring" || needs === "photo" ? needs : "" };
}

function queryOf(a: Asked): string {
  const p = new URLSearchParams();
  if (a.q.trim()) p.set("q", a.q.trim());
  if (a.stock) p.set("stock", a.stock);
  if (a.needs) p.set("needs", a.needs);
  const s = p.toString();
  return s ? `?${s}` : "";
}

export function useItems(initial: Asked): ItemsDesk {
  const live = useLive();
  const [asked, setAsked] = useState<Asked>(initial);
  const [typed, setTyped] = useState(initial.q);
  const [state, setState] = useState<ItemsState>({ kind: "loading" });
  // The newest question wins: an answer to one asked before it is dropped.
  const asking = useRef(0);

  useEffect(() => {
    const n = ++asking.current;
    setState({ kind: "loading" });
    window.history.replaceState(null, "", href(`/items${queryOf(asked)}`));
    const query = {
      ...(asked.q.trim() ? { q: asked.q.trim() } : {}),
      ...(asked.stock ? { stock: asked.stock } : {}),
      ...(asked.needs ? { needs: asked.needs } : {}),
    };
    api
      .items(query)
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
      const page = await api.items({
        ...(asked.q.trim() ? { q: asked.q.trim() } : {}),
        ...(asked.stock ? { stock: asked.stock } : {}),
        ...(asked.needs ? { needs: asked.needs } : {}),
        after: state.next,
      });
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
  };
}
