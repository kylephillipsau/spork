import { createContext, useContext, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

import s from "./frames.module.css";

/**
 * The handheld dock (D134): a bar pinned to the bottom of the screen, where a
 * thumb is, holding the screen's primary action.
 *
 * The frame draws the bar; the screen fills it with `<Dock>`. A portal rather
 * than a prop, because what goes in the dock is the screen's state — the
 * frame outlives the screen, and a portal moves the DOM without moving the
 * context or the events. The bar hides while it is empty (`:empty`).
 */
const DockContext = createContext<HTMLElement | null>(null);

export function DockHost({ children }: { children: ReactNode }) {
  // Callback ref into state: a plain ref would not re-render, and the dock
  // would appear one paint late.
  const [dock, setDock] = useState<HTMLElement | null>(null);
  return (
    <DockContext.Provider value={dock}>
      <div className={s.handheld}>
        {children}
        <div ref={setDock} className={s.dock} data-region="dock" />
      </div>
    </DockContext.Provider>
  );
}

/** Renders into the frame's dock. Nothing outside a DockHost. */
export function Dock({ children }: { children: ReactNode }) {
  const into = useContext(DockContext);
  return into ? createPortal(children, into) : null;
}
