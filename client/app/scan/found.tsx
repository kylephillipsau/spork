import type { ReactNode } from "react";
import { Boxes, ClipboardList, MapPin } from "lucide-react";

import type { Found } from "@domain/types";

/** What a search finds, grouped by what it is, in the order they are shown: the header's results and the results page alike. */
export const FOUND_GROUPS: { kind: Found["kind"]; label: string; icon: ReactNode }[] = [
  { kind: "item", label: "Items", icon: <Boxes aria-hidden /> },
  { kind: "bin", label: "Bins", icon: <MapPin aria-hidden /> },
  { kind: "order", label: "Orders", icon: <ClipboardList aria-hidden /> },
];
