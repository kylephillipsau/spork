import { useEffect, useRef, useState, type ReactNode } from "react";
import { Boxes, CircleAlert, ClipboardList, MapPin, Search, X } from "lucide-react";

import { Kbd, Link, Spinner, cx } from "@ui/index";
import type { Found } from "@domain/types";
import { href } from "@app/routing/location";
import { useChromeScan, type ChromeScan } from "@app/scan/useScan";

import s from "./header.module.css";

/**
 * The header's search box, which also takes a scan (D111, D149, D189).
 *
 * **Typing searches.** Bins, items and orders are found by any part of a code
 * or any word as it is typed, grouped by what they are, and picked with the
 * arrows or a tap. **Enter on a whole code goes to it**: a barcode wedge types
 * and presses Enter, so a scan lands where it always did, and Enter on
 * anything else opens the best match.
 *
 * On a phone the box is a magnifier in the header, and opens the search over
 * the whole screen, the keyboard up and the results below it.
 *
 * It never takes focus by itself (D117): Ctrl+K or `/` does.
 */

const GROUPS: { kind: Found["kind"]; label: string; icon: ReactNode }[] = [
  { kind: "item", label: "Items", icon: <Boxes aria-hidden /> },
  { kind: "bin", label: "Bins", icon: <MapPin aria-hidden /> },
  { kind: "order", label: "Orders", icon: <ClipboardList aria-hidden /> },
];

export function ScanSearch({ fixed, opened = false }: { fixed?: ChromeScan | undefined; opened?: boolean }) {
  const live = useChromeScan();
  const scan = fixed ?? live;
  const input = useRef<HTMLInputElement>(null);
  // On a phone: the search over the whole screen.
  const [open, setOpen] = useState(opened);
  const [focused, setFocused] = useState(opened);
  // The result picked with the arrows, as an index into them in order shown.
  const [at, setAt] = useState(-1);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing =
        e.target instanceof HTMLElement &&
        (e.target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName));
      if ((e.key === "k" && (e.ctrlKey || e.metaKey)) || (e.key === "/" && !typing)) {
        e.preventDefault();
        setOpen(true);
        input.current?.focus();
        input.current?.select();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // A new set of results starts with nothing picked.
  useEffect(() => setAt(-1), [scan.results]);

  const results = scan.results ?? [];
  const shown = GROUPS.flatMap((g) => results.filter((r) => r.kind === g.kind));
  const landing = scan.landing && scan.landing.kind !== "go" ? scan.landing : null;
  const typed = scan.value.trim() !== "";
  // What a scan said shows until dismissed; results while the box is in use.
  const panel = landing !== null || ((focused || open) && typed && scan.results !== null);

  const close = () => {
    setOpen(false);
    setFocused(false);
    scan.dismiss();
    input.current?.blur();
  };
  const pick = (found: Found) => {
    setOpen(false);
    setFocused(false);
    scan.open(found);
  };

  return (
    <div className={cx(s.search, open && s.open)}>
      <button
        type="button"
        className={s.searchToggle}
        aria-label="Search"
        onClick={() => {
          setOpen(true);
          // Focused in the same tap, so a phone raises its keyboard.
          input.current?.focus();
        }}
      >
        <Search aria-hidden />
      </button>
      <form
        role="search"
        className={s.searchBox}
        onSubmit={(e) => {
          e.preventDefault();
          void scan.scan(shown[at] ?? null).then(() => setOpen(false));
        }}
      >
        {scan.busy ? <Spinner size={14} /> : <Search className={s.searchIcon} aria-hidden />}
        <input
          ref={input}
          data-scan="header"
          className={s.searchInput}
          value={scan.value}
          onChange={(e) => scan.type(e.target.value)}
          onFocus={() => setFocused(true)}
          // Later than a click on a result, which would otherwise lose its target.
          onBlur={() => window.setTimeout(() => setFocused(false), 150)}
          onKeyDown={(e) => {
            if (e.key === "Escape") close();
            else if (e.key === "ArrowDown" && shown.length) {
              e.preventDefault();
              setAt((i) => (i + 1) % shown.length);
            } else if (e.key === "ArrowUp" && shown.length) {
              e.preventDefault();
              setAt((i) => (i <= 0 ? shown.length - 1 : i - 1));
            }
          }}
          placeholder="Search items, bins, orders…"
          aria-label="Search, or scan a code"
          aria-expanded={panel}
          aria-controls="header-search-results"
          aria-activedescendant={at >= 0 ? `found-${shown[at]?.id}` : undefined}
          role="combobox"
          aria-autocomplete="list"
          autoComplete="off"
          spellCheck={false}
          enterKeyHint="search"
        />
        <span className={s.searchHint} aria-hidden>
          <Kbd>Ctrl</Kbd>
          <Kbd>K</Kbd>
        </span>
        <button type="button" className={s.searchCancel} onClick={close}>
          Cancel
        </button>
      </form>

      {panel && (
        <div className={s.results} id="header-search-results">
          {landing ? (
            <Landed scan={scan} />
          ) : shown.length === 0 ? (
            <p className={s.resultsText}>Nothing matches “{scan.value.trim()}”.</p>
          ) : (
            GROUPS.filter((g) => shown.some((r) => r.kind === g.kind)).map((g) => (
              <section key={g.kind} className={s.resultsGroup} aria-label={g.label}>
                <h3 className={s.resultsGroupName}>{g.label}</h3>
                <ul className={s.resultsList} role="listbox">
                  {shown
                    .filter((r) => r.kind === g.kind)
                    .map((r) => {
                      const i = shown.indexOf(r);
                      return (
                        <li key={r.id} id={`found-${r.id}`} role="option" aria-selected={i === at}>
                          <button
                            type="button"
                            className={cx(s.resultsOption, i === at && s.resultsPicked)}
                            onMouseDown={(e) => e.preventDefault()}
                            onClick={() => pick(r)}
                          >
                            <span className={s.resultsIconCell}>{g.icon}</span>
                            <span className={s.resultsTitle}>{r.title}</span>
                            {r.detail && <span className={s.resultsDetail}>{r.detail}</span>}
                          </button>
                        </li>
                      );
                    })}
                </ul>
              </section>
            ))
          )}
        </div>
      )}
    </div>
  );
}

/** What an exact scan said when it was not one thing (D111). */
function Landed({ scan }: { scan: ChromeScan }) {
  const landing = scan.landing!;
  if (landing.kind === "go") return null;
  return (
    <div role="status">
      <div className={s.resultsHead}>
        <CircleAlert className={s.resultsIcon} aria-hidden />
        <code className={s.resultsCode}>{landing.scanned}</code>
        <button type="button" className={s.resultsClose} onClick={scan.dismiss} aria-label="Dismiss">
          <X />
        </button>
      </div>
      {landing.kind === "choose" && (
        <>
          <p className={s.resultsText}>More than one match:</p>
          <ul className={s.resultsList}>
            {landing.options.map((o) => (
              <li key={o.label}>
                <Link variant="plain" href={href(o.path)} className={s.resultsOption}>
                  <span className={s.resultsTitle}>{o.label}</span>
                  {o.detail && <span className={s.resultsDetail}>{o.detail}</span>}
                </Link>
              </li>
            ))}
          </ul>
        </>
      )}
      {landing.kind === "unknown" && <p className={s.resultsText}>No match for this code.</p>}
      {landing.kind === "unrecognised" && <p className={s.resultsText}>Not a recognised code.</p>}
      {landing.kind === "nowhere" && <p className={s.resultsText}>Found a {landing.what}, but there is no page for it yet.</p>}
    </div>
  );
}
