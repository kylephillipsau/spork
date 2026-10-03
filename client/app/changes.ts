import { useEffect, useRef } from "react";

import { openChanges } from "@domain/api";
import { Channel } from "@domain/changes";

/**
 * THE REACT SIDE OF HEARING THAT SOMETHING CHANGED (D206).
 *
 * One stream for the device, shared by every screen that listens, and closed
 * when none does. `domain/changes.ts` is the stream; this is the hook a
 * screen calls with the read it already makes.
 */
const channel = new Channel(openChanges);

let woken = false;

/**
 * A handheld put down and picked up again, or a laptop back on the network,
 * has been away however long the stream says: read now rather than at the
 * next change.
 */
function wake(): void {
  if (woken || typeof document === "undefined") return;
  woken = true;
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") channel.nudge();
  });
  window.addEventListener("online", () => channel.nudge());
}

/**
 * Call `read` whenever something changes at this site, and when the device
 * comes back from being away. `read` is the screen's own quiet re-read: it
 * should not show a loading state, because a list that blanks for every pick
 * somebody else makes is worse than one a moment behind.
 *
 * Off when `listening` is false, as it is in a fixture, which must not touch
 * the network.
 */
export function useChanges(read: () => void, listening = true): void {
  const latest = useRef(read);
  latest.current = read;
  useEffect(() => {
    if (!listening) return;
    wake();
    return channel.subscribe(() => latest.current());
  }, [listening]);
}
