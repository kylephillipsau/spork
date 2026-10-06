import { useEffect, useState } from "react";

import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { Found } from "@domain/types";

import type { Read } from "@app/layout/usePlace";

/** As many as the search gives in one answer: a page of results, not the header's few. */
const MOST = 50;

export interface SearchDesk {
  /** What was searched for. */
  q: string;
  read: Read<Found[]>;
}

/** Everything the search matches for `q`, for the results page (D227). */
export function useSearchResults(q: string): SearchDesk {
  const live = useLive();
  const [read, setRead] = useState<Read<Found[]>>({ kind: "loading" });
  useEffect(() => {
    const asked = q.trim();
    if (!asked) {
      setRead({ kind: "ready", value: [] });
      return;
    }
    setRead({ kind: "loading" });
    api
      .search(asked, MOST)
      .then((a) => live.current && setRead({ kind: "ready", value: a.results }))
      .catch((error) => live.current && setRead({ kind: "failed", message: reason(error, "The search did not answer.") }));
  }, [q, live]);
  return { q, read };
}
