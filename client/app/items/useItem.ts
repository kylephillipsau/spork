import { useCallback, useEffect, useState } from "react";

import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { ItemView } from "@domain/types";

export type ItemRead = { kind: "loading" } | { kind: "ready"; item: ItemView } | { kind: "failed"; message: string };

export interface ItemDesk {
  read: ItemRead;
}

/**
 * One item, by the id in the path. **Where a scanned item lands** (D111): the
 * question a product is scanned to ask is what it is and where it lives.
 */
export function useItem(itemId: string): ItemDesk {
  const live = useLive();
  const [read, setRead] = useState<ItemRead>({ kind: "loading" });

  const refresh = useCallback(async () => {
    try {
      const item = await api.item(itemId);
      if (live.current) setRead({ kind: "ready", item });
    } catch (error) {
      if (live.current) setRead({ kind: "failed", message: reason(error, "Could not load the item.") });
    }
  }, [itemId, live]);

  useEffect(() => {
    setRead({ kind: "loading" });
    void refresh();
  }, [refresh]);

  return { read };
}
