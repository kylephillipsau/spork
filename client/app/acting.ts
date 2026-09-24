import { useCallback, useEffect, useRef, useState } from "react";
import { reason } from "@domain/api";
import { pressing } from "@domain/acts";
import type { Act } from "@domain/acts";

/**
 * THE REACT SIDE OF AN ACT.
 *
 * `domain/acts.ts` is the pure half — it mints an id for a press and remembers
 * which presses are still in the air — and it is pure so that a fixture can
 * drive it with a literal clock. This is the half that needs React: the busy
 * flag a key reads to disable itself, the refusal a panel draws, and the guard
 * that stops a response arriving into a screen the operator has already left.
 */

/**
 * Whether this component is still mounted.
 *
 * Seventeen hooks declared this ref and its effect verbatim. It exists because
 * every write here ends in `setState`, and an operator who presses Consign and
 * walks to the next screen gets that response after the component is gone —
 * React does not warn about it any more, but the state is set on a dead tree
 * and the next mount reads a stale one.
 *
 * **The ref, not a wrapper.** Call sites keep writing `if (live.current)`,
 * which is the same line they wrote before and reads at the point it matters.
 * A `guard(fn)` would have moved the condition somewhere it could be forgotten.
 */
export function useLive(): { readonly current: boolean } {
  const live = useRef(true);
  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
    };
  }, []);
  return live;
}

export interface Writing {
  /** A press is in the air. Every key on the screen reads this. */
  busy: boolean;
  /** The refusal, in the server's words. Null until something is refused. */
  problem: string | null;
  /** Clear the refusal. Screens pair it with whatever else their notice holds. */
  dismiss: () => void;
  /**
   * Put a refusal up that no press produced.
   *
   * A deep link to a finding that is not here has to say so, and it says so in
   * the same place a refused Accept would — one notice per screen, not two that
   * can both be showing. `dismiss` is `say(null)`.
   */
  say: (message: string | null) => void;
  /**
   * Do the thing, once.
   *
   * `key` names the act: the subject and what was typed, never a counter. Two
   * presses of the same key are the same act and the server folds the second
   * into the first (D5); a re-measure after correcting a figure is a different
   * key and rightly a different act.
   */
  press: (key: string, run: (act: Act) => Promise<void>) => Promise<void>;
}

/**
 * The write machine every bench and desk was writing out for itself.
 *
 * Five hooks had this body — two as an extracted `press`, three inlined into
 * the action itself — and the five agreed on all of it: refuse a second press
 * while one is in the air, clear the last refusal, mint an act, land it on
 * success, phrase the failure, and drop `busy` in a `finally` so a thrown
 * error cannot leave the screen locked.
 *
 * @param after What to do once the press has settled, landed or not. The two
 * benches that write the ledger re-read their screen here, because a despatch
 * changes what is left to despatch. The three that do not are the ones whose
 * own comments say why: a weighing removes its subject from the worklist, and
 * refetching would renumber the queue under the operator between one box and
 * the next.
 *
 * **`after` runs even when the press was refused.** A refusal is not always a
 * no-op — a batch that stopped at the third carton moved two — so the screen is
 * re-read from what actually happened rather than from what was asked for.
 */
export function useWriting(after?: () => Promise<void>): Writing {
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const live = useLive();

  /** The acts begun and not yet landed. See `domain/acts.ts`. */
  const doing = useRef(pressing());

  const press = useCallback(
    async (key: string, run: (act: Act) => Promise<void>) => {
      if (busy) return;
      setBusy(true);
      setProblem(null);
      try {
        await run(doing.current.attempt(key));
        doing.current.landed(key);
      } catch (error) {
        if (live.current) setProblem(reason(error, "Request failed."));
      } finally {
        if (after) await after();
        if (live.current) setBusy(false);
      }
    },
    [busy, after, live],
  );

  return {
    busy,
    problem,
    dismiss: useCallback(() => setProblem(null), []),
    say: setProblem,
    press,
  };
}
