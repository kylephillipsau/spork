import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";

import { carried, choose, dropHow, type ChooseMode, type DropHow } from "./choosing";
import s from "./drag.module.css";

/**
 * One drag for the app (D245): a pointer drag, choosing several, and a
 * keyboard way to do everything a drag does. No library.
 *
 * A `DragZone` holds what is chosen and what is being dragged. Inside it,
 * anything can be a source (`useDraggable`) or a target (`useDropTarget`),
 * named by a key the screen gives it; the screen says what a key is and what
 * a drop does. A press that moves less than a few pixels is a click and
 * chooses; one that moves further drags what is chosen, or it alone.
 *
 * The keyboard, on a source: Space chooses it or lets it go, the arrows move
 * between sources and with Shift choose those too, M moves what is chosen
 * (the screen asks where, and how many), Delete takes it out, Z undoes, and
 * Escape lets everything go.
 */

/** A drag under way: what it carries, where it is, and the target under it. */
export interface DragNow {
  keys: string[];
  over: string | null;
  x: number;
  y: number;
}

interface ZoneValue {
  chosen: string[];
  choose: (key: string, mode: ChooseMode) => void;
  drag: DragNow | null;
  press: (key: string, e: PointerEvent<HTMLElement>) => void;
  keys: (key: string, e: KeyboardEvent<HTMLElement>) => void;
}

const Zone = createContext<ZoneValue | null>(null);

/** How far a press moves before it is a drag rather than a click, in CSS pixels. */
const SLOP = 4;

export function DragZone({
  chosen: controlled,
  onChoose,
  label,
  onDrop,
  onMove,
  onRemove,
  onUndo,
  frozen = null,
  children,
}: {
  /** What is chosen, when the screen keeps it; otherwise the zone does. */
  chosen?: string[] | undefined;
  onChoose?: ((chosen: string[]) => void) | undefined;
  /** What the ghost under the pointer says of these: "WPE-0750 × 10". */
  label: (keys: string[]) => string;
  onDrop: (keys: string[], target: string, how: DropHow) => void;
  /** M: move these, asking where and how many. */
  onMove?: ((keys: string[]) => void) | undefined;
  /** Delete: take these out. */
  onRemove?: ((keys: string[]) => void) | undefined;
  /** Z: undo the last change. */
  onUndo?: (() => void) | undefined;
  /** A drag held still where it is, for a screen drawn from a fixture. */
  frozen?: { keys: string[]; over: string | null } | null | undefined;
  children: ReactNode;
}) {
  const [own, setOwn] = useState<string[]>([]);
  const chosen = controlled ?? own;
  const setChosen = useCallback(
    (next: string[]) => {
      if (onChoose) onChoose(next);
      else setOwn(next);
    },
    [onChoose],
  );
  const [drag, setDrag] = useState<DragNow | null>(null);
  // The latest of each, for the listeners on the window.
  const latest = useRef({ chosen, onDrop, setChosen });
  latest.current = { chosen, onDrop, setChosen };

  const press = useCallback((key: string, e: PointerEvent<HTMLElement>) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button, a, input, select, textarea, [data-no-drag]")) return;
    const start = { x: e.clientX, y: e.clientY };
    let now: DragNow | null = null;
    const targetAt = (x: number, y: number) => document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-drop]")?.dataset["drop"] ?? null;
    const move = (m: globalThis.PointerEvent) => {
      if (!now && Math.hypot(m.clientX - start.x, m.clientY - start.y) < SLOP) return;
      if (!now) {
        const keys = carried(latest.current.chosen, key);
        latest.current.setChosen(keys);
        now = { keys, over: null, x: m.clientX, y: m.clientY };
      }
      now = { ...now, x: m.clientX, y: m.clientY, over: targetAt(m.clientX, m.clientY) };
      setDrag(now);
    };
    const stop = (u: globalThis.PointerEvent | null) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("keydown", escape);
      if (u && now?.over) latest.current.onDrop(now.keys, now.over, dropHow(u));
      now = null;
      setDrag(null);
    };
    const up = (u: globalThis.PointerEvent) => stop(u);
    const escape = (k: globalThis.KeyboardEvent) => {
      if (k.key === "Escape") stop(null);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("keydown", escape);
  }, []);

  const keys = useCallback(
    (key: string, e: KeyboardEvent<HTMLElement>) => {
      const these = carried(chosen, key);
      const step = (by: 1 | -1) => {
        const all = [...(e.currentTarget.closest("[data-drag-zone]")?.querySelectorAll<HTMLElement>("[data-drag]") ?? [])];
        const next = all[all.indexOf(e.currentTarget) + by];
        if (!next) return;
        next.focus();
        const nextKey = next.dataset["drag"]!;
        if (e.shiftKey) setChosen(choose(choose(chosen, key, "add"), nextKey, "add"));
      };
      switch (e.key) {
        case " ":
          setChosen(choose(chosen, key, "toggle"));
          break;
        case "ArrowDown":
        case "ArrowRight":
          step(1);
          break;
        case "ArrowUp":
        case "ArrowLeft":
          step(-1);
          break;
        case "m":
        case "M":
          if (!onMove) return;
          setChosen(these);
          onMove(these);
          break;
        case "Delete":
        case "Backspace":
          if (!onRemove) return;
          onRemove(these);
          break;
        case "z":
        case "Z":
          if (!onUndo) return;
          onUndo();
          break;
        case "Escape":
          setChosen([]);
          break;
        default:
          return;
      }
      e.preventDefault();
    },
    [chosen, setChosen, onMove, onRemove, onUndo],
  );

  const shown = frozen ? { keys: frozen.keys, over: frozen.over, x: 0, y: 0 } : drag;
  const value = useMemo<ZoneValue>(
    () => ({ chosen: frozen?.keys ?? chosen, choose: (key, mode) => setChosen(choose(chosen, key, mode)), drag: shown, press, keys }),
    [chosen, setChosen, shown, press, keys, frozen],
  );

  return (
    <Zone.Provider value={value}>
      <div data-drag-zone="" className={drag ? s.dragging : undefined}>
        {children}
      </div>
      {drag &&
        createPortal(
          <div className={s.ghost} style={{ left: drag.x + 14, top: drag.y + 14 }} aria-hidden="true">
            {label(drag.keys)}
          </div>,
          document.body,
        )}
    </Zone.Provider>
  );
}

function useZone(): ZoneValue {
  const zone = useContext(Zone);
  if (!zone) throw new Error("a drag source or target is inside a DragZone");
  return zone;
}

/**
 * Make an element something to drag and to choose. Spread `props` on it; it
 * says `data-chosen` when chosen and `data-dragged` while it is carried.
 */
export function useDraggable(key: string) {
  const zone = useZone();
  const chosen = zone.chosen.includes(key);
  return {
    chosen,
    props: {
      tabIndex: 0,
      "data-drag": key,
      "data-chosen": chosen || undefined,
      "data-dragged": zone.drag?.keys.includes(key) || undefined,
      // The innermost source answers: goods in a carton, not the carton round them.
      onPointerDown: (e: PointerEvent<HTMLElement>) => {
        e.stopPropagation();
        zone.press(key, e);
      },
      onClick: (e: MouseEvent<HTMLElement>) => {
        e.stopPropagation();
        if ((e.target as HTMLElement).closest("button, a, input, select, textarea, [data-no-drag]")) return;
        zone.choose(key, e.shiftKey || e.ctrlKey || e.metaKey ? "toggle" : "only");
      },
      onKeyDown: (e: KeyboardEvent<HTMLElement>) => {
        if (e.target === e.currentTarget) zone.keys(key, e);
      },
    },
  };
}

/** Make an element somewhere to drop. It says `data-over` while something is held over it. */
export function useDropTarget(key: string) {
  const zone = useZone();
  const over = zone.drag?.over === key;
  return { over, props: { "data-drop": key, "data-over": over || undefined } };
}

/** The drag under way, or none: for a screen to show where it would go, and what it would do. */
export function useDragNow(): DragNow | null {
  return useZone().drag;
}

/** What is chosen, by key. */
export function useChosen(): string[] {
  return useZone().chosen;
}
