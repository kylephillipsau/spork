import { useCallback, useEffect, useRef, useState } from "react";

import { useLive, useWriting } from "@app/acting";
import { href } from "@app/routing/location";
import { ApiError, api, reason } from "@domain/api";
import type { ItemListRow, ItemRow, ListChange } from "@domain/types";

import { codesFrom } from "./lists";

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
 *
 * **A list of items to work through narrows it to a sheet** (D179): the codes
 * somebody else drew up, kept in the order on their paper, made here by
 * pasting them in.
 */

export type Stock = "" | "here";
export type Needs = "" | "weighing" | "measuring" | "photo" | "packing";
/** What has been done, to look over: the other way round from `Needs`. */
export type Has = "" | "measured" | "photographed" | "both";
export type Order = "" | "demand" | "packing" | "walk" | "list";

export interface Asked {
  q: string;
  stock: Stock;
  needs: Needs;
  has: Has;
  /** A list of items to work through, by id; "" for none (D179). */
  list: string;
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

  /** The lists worked here, newest first; null until read. */
  lists: ItemListRow[] | null;
  /** Narrow to a list, in its order, or to none. */
  pick: (listId: string) => void;
  /** Make a list from pasted codes and narrow to it. True when it was made. */
  makeList: (name: string, pasted: string) => Promise<boolean>;
  /** The list narrowed to, as the picker has it; null for none. */
  list: ItemListRow | null;
  /**
   * Rename the list narrowed to, add pasted codes to it, take an item off it,
   * or put it away (D235). True when done.
   */
  renameList: (name: string) => Promise<boolean>;
  addToList: (pasted: string) => Promise<boolean>;
  takeOffList: (itemId: string) => Promise<boolean>;
  removeList: () => Promise<boolean>;
  /** Where the list as asked downloads, every row of it (D216). */
  exportUrl: (format: "csv" | "xlsx" | "pdf") => string;
  making: { busy: boolean; problem: string | null; dismiss: () => void };
}

const NEEDS: readonly Needs[] = ["weighing", "measuring", "photo", "packing"];
const HAS: readonly Has[] = ["measured", "photographed", "both"];
const ORDERS: readonly Order[] = ["demand", "packing", "walk", "list"];

/** The question in a URL's query string, and back. */
export function askedFrom(search: string): Asked & { item: string | null } {
  const p = new URLSearchParams(search);
  const needs = p.get("needs") as Needs;
  const has = p.get("has") as Has;
  const order = p.get("order") as Order;
  const list = p.get("list") ?? "";
  return {
    q: p.get("q") ?? "",
    stock: p.get("stock") === "here" ? "here" : "",
    needs: NEEDS.includes(needs) ? needs : "",
    has: HAS.includes(has) ? has : "",
    list,
    // A list's own order needs the list.
    order: ORDERS.includes(order) && (order !== "list" || list) ? order : "",
    item: p.get("item"),
  };
}

function queryOf(a: Asked): {
  q?: string;
  stock?: "here";
  needs?: Exclude<Needs, "">;
  has?: Exclude<Has, "">;
  list?: string;
  order?: Exclude<Order, "">;
} {
  return {
    ...(a.q.trim() ? { q: a.q.trim() } : {}),
    ...(a.stock ? { stock: a.stock } : {}),
    ...(a.needs ? { needs: a.needs } : {}),
    ...(a.has ? { has: a.has } : {}),
    ...(a.list ? { list: a.list } : {}),
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
  const [asked, setAsked] = useState<Asked>({
    q: initial.q,
    stock: initial.stock,
    needs: initial.needs,
    has: initial.has,
    list: initial.list,
    order: initial.order,
  });
  const [lists, setLists] = useState<ItemListRow[] | null>(null);
  const making = useWriting();
  const [typed, setTyped] = useState(initial.q);
  const [chosen, setChosen] = useState<string | null>(initial.item ?? null);
  const [state, setState] = useState<ItemsState>({ kind: "loading" });
  // The newest question wins: an answer to one asked before it is dropped.
  const asking = useRef(0);
  // Asked again, the question unchanged: a list's items changed under it (D235).
  const [again, setAgain] = useState(0);

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
  }, [asked, again, live]);

  const readLists = useCallback(async () => {
    try {
      const rows = await api.itemLists();
      if (live.current) setLists(rows);
    } catch {
      // Silent: a picker with no lists in it is the item list as it was.
      if (live.current) setLists([]);
    }
  }, [live]);
  useEffect(() => {
    void readLists();
  }, [readLists]);

  /** One change to the list narrowed to, as an act; the lists read again after it. */
  const change = async (key: string, what: () => ListChange): Promise<boolean> => {
    const list = asked.list;
    if (!list) return false;
    let done = false;
    await making.press(`list:${list}:${key}`, async (act) => {
      await api.changeItemList(list, what(), act);
      done = true;
    });
    if (done && live.current) await readLists();
    return done;
  };

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

    lists,
    pick: (list) => setAsked((a) => ({ ...a, list, order: list ? "list" : a.order === "list" ? "" : a.order })),
    exportUrl: (format) => api.itemsExportUrl(queryOf(asked), format),
    makeList: async (name, pasted) => {
      let made: ItemListRow | null = null;
      await making.press(`list:${name.trim()}:${pasted}`, async (act) => {
        const codes = codesFrom(pasted);
        if (!name.trim()) throw new ApiError("Give the list a name.", 400);
        if (codes.length === 0) throw new ApiError("Paste the item codes, one a line.", 400);
        made = await api.makeItemList({ name: name.trim(), codes, act });
      });
      const list = made as ItemListRow | null;
      if (!list || !live.current) return false;
      await readLists();
      setChosen(null);
      setAsked((a) => ({ ...a, q: "", stock: "", needs: "", has: "", list: list.item_list_id, order: "list" }));
      setTyped("");
      return true;
    },
    list: (lists ?? []).find((l) => l.item_list_id === asked.list) ?? null,
    renameList: (name) => change(`rename:${name.trim()}`, () => {
      if (!name.trim()) throw new ApiError("Give the list a name.", 400);
      return { rename: { name: name.trim() } };
    }),
    addToList: (pasted) =>
      change(`add:${pasted}`, () => {
        const codes = codesFrom(pasted);
        if (codes.length === 0) throw new ApiError("Paste the item codes, one a line.", 400);
        return { add: { codes } };
      }).then((done) => {
        if (done) setAgain((n) => n + 1);
        return done;
      }),
    takeOffList: (itemId) =>
      change(`take-off:${itemId}`, () => ({ take_off: { item_ids: [itemId] } })).then((done) => {
        if (done) setAgain((n) => n + 1);
        return done;
      }),
    removeList: () =>
      change("remove", () => "remove").then((done) => {
        if (done) {
          setChosen(null);
          setAsked((a) => ({ ...a, list: "", order: a.order === "list" ? "" : a.order }));
        }
        return done;
      }),
    making: { busy: making.busy, problem: making.problem, dismiss: making.dismiss },
  };
}
