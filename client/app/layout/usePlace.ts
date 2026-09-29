import { useCallback, useEffect, useState } from "react";

import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { BinView, PlaceView } from "@domain/types";

export type Read<T> = { kind: "loading" } | { kind: "ready"; value: T } | { kind: "failed"; message: string };

export interface BinDesk {
  read: Read<BinView>;
}

export interface PlaceDesk {
  read: Read<PlaceView>;
}

/** Load one thing by id, again whenever the id changes. */
function useRead<T>(id: string, load: (id: string) => Promise<T>, failure: string): Read<T> {
  const live = useLive();
  const [read, setRead] = useState<Read<T>>({ kind: "loading" });

  const refresh = useCallback(async () => {
    try {
      const value = await load(id);
      if (live.current) setRead({ kind: "ready", value });
    } catch (error) {
      if (live.current) setRead({ kind: "failed", message: reason(error, failure) });
    }
  }, [id, load, failure, live]);

  useEffect(() => {
    setRead({ kind: "loading" });
    void refresh();
  }, [refresh]);

  return read;
}

/**
 * A bin and where it is (D173). **Where a scanned bin code lands**: the page
 * answers "where is this", which is the question a bin label is scanned to ask.
 */
export function useBin(locationId: string): BinDesk {
  return { read: useRead(locationId, api.bin, "Could not load the bin.") };
}

/** One place, reached from a breadcrumb or from the layout list. */
export function usePlace(placeId: string): PlaceDesk {
  return { read: useRead(placeId, api.place, "Could not load the place.") };
}
