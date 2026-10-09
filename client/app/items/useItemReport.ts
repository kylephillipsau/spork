import { useEffect, useState } from "react";

import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { BoundBarcode, ExportRow, ItemView, PackagingType } from "@domain/types";

/**
 * Everything recorded of the items a list asks for, read for the full report
 * (D242): each item's page as it shows it, what the export says of who
 * recorded each card and how (D216), and its barcodes. The list's own
 * question, from the page's address, as Items › Export asks it.
 */

export interface ReportItem {
  item: ItemView;
  row: ExportRow;
  barcodes: BoundBarcode[];
}

export type ReportRead =
  | { kind: "loading"; done: number; of: number }
  | { kind: "ready"; title: string; items: ReportItem[]; types: PackagingType[]; at: Date }
  | { kind: "failed"; message: string };

/** How many items are read at once: a list of a few dozen, without asking for all of them together. */
const AT_ONCE = 4;

/** What the item list asks, by the names it puts in its address. */
const ASKED = ["q", "stock", "needs", "has", "list", "order"] as const;

/** The list's question, as the item list puts it in its address. */
function askedOf(search: string): Partial<Record<(typeof ASKED)[number], string>> {
  const p = new URLSearchParams(search);
  const out: Partial<Record<(typeof ASKED)[number], string>> = {};
  for (const k of ASKED) {
    const v = p.get(k);
    if (v) out[k] = v;
  }
  return out;
}

export function useItemReport(search: string): ReportRead {
  const live = useLive();
  const [read, setRead] = useState<ReportRead>({ kind: "loading", done: 0, of: 0 });
  useEffect(() => {
    const asked = askedOf(search);
    (async () => {
      const [rows, types, lists] = await Promise.all([
        api.itemsExport(asked),
        api.packagingTypes(),
        asked.list ? api.itemLists() : Promise.resolve([]),
      ]);
      const title = lists.find((l) => l.item_list_id === asked.list)?.name ?? (asked.q ? `Items matching “${asked.q}”` : "Items");
      if (live.current) setRead({ kind: "loading", done: 0, of: rows.length });
      const items: ReportItem[] = [];
      for (let k = 0; k < rows.length; k += AT_ONCE) {
        const batch = rows.slice(k, k + AT_ONCE);
        items.push(
          ...(await Promise.all(
            batch.map(async (row) => {
              const [item, barcodes] = await Promise.all([api.item(row.item_id), api.itemBarcodes(row.item_id)]);
              return { item, row, barcodes };
            }),
          )),
        );
        if (live.current) setRead({ kind: "loading", done: items.length, of: rows.length });
      }
      if (live.current) setRead({ kind: "ready", title, items, types, at: new Date() });
    })().catch((error: unknown) => live.current && setRead({ kind: "failed", message: reason(error, "Could not read the items.") }));
  }, [search, live]);
  return read;
}
