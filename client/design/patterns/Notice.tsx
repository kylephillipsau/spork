import type { ReactNode } from "react";
import { Face } from "../primitives/Face";
import { Key } from "../primitives/Key";
import { Lamp } from "../primitives/Lamp";
import type { LampKind } from "../primitives/Lamp";
import styles from "./notice.module.css";

/**
 * WHAT THE SCREEN HAS TO SAY, AND THE ONE WAY TO PUT IT DOWN (D167).
 *
 * Nine screens built this by hand out of `Face`, `Row`, `Lamp`, `Spacer` and a
 * small `Key`, and — unusually — all nine agreed. That is the argument for
 * naming it rather than against: nine identical copies are nine chances to
 * drift, and the tenth screen has to guess which of the nine to copy.
 *
 * It pairs with `useWriting` (`app/acting.ts`), which produces exactly the two
 * things this takes: a `problem` and a `dismiss`.
 *
 *     {bench.problem && <Notice onDismiss={bench.dismiss}>{bench.problem}</Notice>}
 *
 * # One notice per screen
 *
 * Not a rule this can enforce, but the reason `kind` exists rather than a
 * second component: a screen that has both refused something and recorded
 * something says so in one place, in the tone of whichever happened, instead of
 * stacking two banners the operator has to dismiss in turn.
 *
 * **Not for a receipt with a body.** Weigh's recorded weight and Tokens' minted
 * secret put content *under* the sentence — a figure to check, a string to
 * copy — and that is a panel that happens to open, not a notice. They keep
 * their own shape.
 */
export function Notice({
  kind = "finding",
  children,
  notes,
  onDismiss,
  label = "Dismiss",
}: {
  /** `finding` for a refusal — the interface's only amber (D115). `recorded`
   *  for something that landed. */
  kind?: LampKind;
  /** The sentence. The server's words where there are any (D114). */
  children: ReactNode;
  /** Qualifications that came back with it, beside the sentence rather than
   *  under it — a consignment's carrier warnings are the case this exists for. */
  notes?: ReactNode;
  /**
   * Omitted where there is nothing to dismiss.
   *
   * A screen that could not load says so and stays saying so — there is no
   * state under the message to get back to, and a key that clears the only
   * thing on the screen is a key that empties it. Seven screens drew that
   * state, each inside its own Panel, and the only thing they disagreed about
   * was the Panel.
   */
  onDismiss?: () => void;
  /** "Done" where dismissing is the end of the job rather than clearing a
   *  refusal. */
  label?: string;
}) {
  return (
    <Face>
      <div className={styles.notice}>
        <div className={styles.said}>
          <Lamp kind={kind} />
          <span>{children}</span>
          {notes}
        </div>
        {onDismiss && (
          <div className={styles.act}>
            <Key size="small" onClick={onDismiss}>
              {label}
            </Key>
          </div>
        )}
      </div>
    </Face>
  );
}
