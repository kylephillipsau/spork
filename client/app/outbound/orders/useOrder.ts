import { useCallback, useEffect, useState } from "react";

import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { OrderView } from "@domain/types";

export type OrderRead = { kind: "loading" } | { kind: "ready"; order: OrderView } | { kind: "failed"; message: string };

export interface OrderDesk {
  read: OrderRead;
  refresh: () => Promise<void>;
}

/**
 * One order, by the id in the path.
 *
 * The list finds orders; this is where one lives. An id rather than a
 * reference, because a reference can name two orders (D44) and a page has to
 * be about one.
 */
export function useOrder(orderId: string): OrderDesk {
  const live = useLive();
  const [read, setRead] = useState<OrderRead>({ kind: "loading" });

  const refresh = useCallback(async () => {
    try {
      const order = await api.order(orderId);
      if (live.current) setRead({ kind: "ready", order });
    } catch (error) {
      if (live.current) setRead({ kind: "failed", message: reason(error, "Could not load the order.") });
    }
  }, [orderId, live]);

  useEffect(() => {
    setRead({ kind: "loading" });
    void refresh();
  }, [refresh]);

  return { read, refresh };
}
