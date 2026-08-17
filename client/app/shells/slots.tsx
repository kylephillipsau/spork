import { createContext, useContext, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import type { ReactNode } from "react";

/**
 * The regions a shell owns that a screen fills.
 *
 * # Why a portal and not a prop
 *
 * The shell is above the screen and outlives it: that is the whole point of the
 * frame, and it is what stops a navigation from tearing down the room, the
 * session and the rail. But two of the regions a shell draws hold *the screen's*
 * state — Floor's dock changes as a capture session moves (D109), and Desk's
 * evidence panel shows whichever row is selected (D111). Passing those up as
 * props would mean the shell holding state it does not own, or the screen being
 * called for its return value rather than rendered, and neither is a thing React
 * does.
 *
 * So the screen renders them where its state is, and they land where the shell
 * put the container. React's tree is unchanged — a portal moves the DOM, not the
 * context or the events — so `useCapture` still owns the dock's buttons and the
 * dock is still a child of the screen as far as anything in the client is
 * concerned.
 *
 * # The containers are always there
 *
 * A shell cannot know whether the screen inside it will fill a region, so it
 * draws the container unconditionally and the stylesheet hides it while it is
 * empty (`:empty`). That is one code path rather than two, and it is why the
 * node exists before the screen looks for it.
 *
 * # Fixtures pass props instead
 *
 * A fixture draws its own shell with literal chrome and no network, so there is
 * no frame above it and no context to portal into. The shells still take `dock`
 * and `evidence` as props, and they render into the same container — so the two
 * mechanisms cannot both be in use, and neither has a second copy of the layout.
 */

interface Regions {
  readonly dock: HTMLElement | null;
  readonly evidence: HTMLElement | null;
}

const EMPTY: Regions = { dock: null, evidence: null };

const RegionContext = createContext<Regions>(EMPTY);

/**
 * A container the shell draws and a screen fills.
 *
 * Callback ref into state rather than a `useRef`: a ref does not re-render, so
 * the screen would look for the node on the render where it is still null and
 * the region would appear one paint late. Setting state from the ref callback
 * happens during commit, so the portal is in the same paint as the container.
 */
export function useRegion(): [HTMLElement | null, (node: HTMLElement | null) => void] {
  return useState<HTMLElement | null>(null);
}

/** Publishes the shell's regions to the screen inside it. */
export function Regions({
  dock = null,
  evidence = null,
  children,
}: {
  dock?: HTMLElement | null;
  evidence?: HTMLElement | null;
  children: ReactNode;
}) {
  const regions = useMemo(() => ({ dock, evidence }), [dock, evidence]);
  return <RegionContext.Provider value={regions}>{children}</RegionContext.Provider>;
}

function Slot({ into, children }: { into: HTMLElement | null; children: ReactNode }) {
  // No container means no frame — a fixture, drawing its own shell and passing
  // the same content as a prop. Rendering here as well would draw it twice.
  return into ? createPortal(children, into) : null;
}

/** The reachable third of a Floor screen (D134). */
export function Dock({ children }: { children: ReactNode }) {
  return <Slot into={useContext(RegionContext).dock}>{children}</Slot>;
}

/** The panel beside the work on a Desk screen (D111). */
export function Evidence({ children }: { children: ReactNode }) {
  return <Slot into={useContext(RegionContext).evidence}>{children}</Slot>;
}
