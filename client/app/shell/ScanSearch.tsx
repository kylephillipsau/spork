import { useEffect, useRef } from "react";
import { CircleAlert, Search, X } from "lucide-react";

import { Kbd, Link, Spinner } from "@ui/index";
import { href } from "@app/routing/location";
import { useChromeScan } from "@app/scan/useScan";

import s from "./header.module.css";

/**
 * The header's search box, which also takes a scan (D111, D149).
 *
 * A barcode wedge types and presses Enter, so a scan and a typed code are the
 * same thing here. It never takes focus by itself (D117): Ctrl+K or `/` does.
 */
export function ScanSearch() {
  const scan = useChromeScan();
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing =
        e.target instanceof HTMLElement &&
        (e.target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName));
      if ((e.key === "k" && (e.ctrlKey || e.metaKey)) || (e.key === "/" && !typing)) {
        e.preventDefault();
        input.current?.focus();
        input.current?.select();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const landing = scan.landing && scan.landing.kind !== "go" ? scan.landing : null;

  return (
    <div className={s.search}>
      <form
        role="search"
        className={s.searchBox}
        onSubmit={(e) => {
          e.preventDefault();
          void scan.scan();
        }}
      >
        {scan.busy ? <Spinner size={14} /> : <Search className={s.searchIcon} aria-hidden />}
        <input
          ref={input}
          className={s.searchInput}
          value={scan.value}
          onChange={(e) => scan.type(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              scan.dismiss();
              input.current?.blur();
            }
          }}
          placeholder="Search or scan…"
          aria-label="Search or scan a code"
          autoComplete="off"
          spellCheck={false}
        />
        <span className={s.searchHint} aria-hidden>
          <Kbd>Ctrl</Kbd>
          <Kbd>K</Kbd>
        </span>
      </form>

      {landing && (
        <div className={s.results} role="status">
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
                      <span>{o.label}</span>
                      {o.detail && <span className={s.resultsDetail}>{o.detail}</span>}
                    </Link>
                  </li>
                ))}
              </ul>
            </>
          )}
          {landing.kind === "unknown" && <p className={s.resultsText}>No match for this code.</p>}
          {landing.kind === "unrecognised" && <p className={s.resultsText}>Not a recognised code.</p>}
          {landing.kind === "nowhere" && (
            <p className={s.resultsText}>Found a {landing.what}, but there is no page for it yet.</p>
          )}
        </div>
      )}
    </div>
  );
}
