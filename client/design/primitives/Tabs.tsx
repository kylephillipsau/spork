import { cx } from "../cx";
import styles from "../materials/face.module.css";

/**
 * A closed set of views over the same work.
 *
 * **Not keys.** The first draft of the findings screen drew its four views as
 * `Key` with `live` on the current one, which breaks the rule `Key` states
 * about itself: *"`live` is the one key on a screen that is lit from within:
 * the thing the operator is about to do. Two lit keys on one screen means
 * neither is."* A tab is not a thing you are about to do — it is where you
 * already are — and lighting it spends the screen's one emphasis on a label.
 * The pack screen learned the same lesson with three violet keys that said
 * three primary actions and meant none.
 *
 * So the current tab is marked the way everything else here marks state: by
 * ground and an edge, which is occlusion rather than paint (D128), leaving the
 * lamp free for the act.
 *
 * Radio semantics rather than buttons, because that is what this is: one of a
 * set, exactly one chosen. A screen reader says "Open, radio button, 2 of 4"
 * instead of reading four unrelated buttons.
 */
export function Tabs<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  /** Names the set for a screen reader. Not drawn. */
  label: string;
  value: T;
  options: { value: T; label: string; count?: number }[];
  onChange: (next: T) => void;
}) {
  return (
    <div
      data-layer="instrument"
      className={styles.tabs}
      role="radiogroup"
      aria-label={label}
    >
      {options.map((option) => {
        const on = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={on}
            className={cx(styles.tab, on && styles.tabOn)}
            onClick={() => onChange(option.value)}
          >
            {option.label}
            {/* D112: zero hides. A tab reading (0) is a tab telling somebody
                there is nothing there, which the empty state says better. */}
            {option.count !== undefined && option.count > 0 && (
              <span className={styles.tabCount}>{option.count}</span>
            )}
          </button>
        );
      })}
    </div>
  );
}
