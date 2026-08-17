import { useCallback, useEffect, useRef, useState } from "react";
import { useLive, useWriting } from "@app/acting";
// Aliased: this desk has a `reason` of its own — the words the operator types
// to justify accepting a finding — and that one is the domain noun.
import { ApiError, api, reason as phrase } from "@domain/api";
import type { DiscrepancyRow, Uuid } from "@domain/types";
import { statesOf, viewFor } from "./views";
import type { ViewKey } from "./views";

/**
 * The findings queue, as logic.
 *
 * Same split as the three screens before it: this fetches and mutates, the
 * component takes data and emits primitives, and the presentational half
 * renders from a fixture with no network.
 *
 * # What this screen does and does not do
 *
 * Investigate and accept. **Not resolve.** Resolution writes a movement and
 * goes through `/adjustments` with `resolving_movement_id` — a different act
 * with a different grant, and the Adjust row of the screen map. A resolved
 * finding is drawn here, with who closed it and why; closing one is somewhere
 * else. Doing both from one screen would put a ledger write behind a button
 * whose neighbours do not write to the ledger at all.
 *
 * # The finding is in the path (D135)
 *
 * `at` is the id the URL names and `place` is how this screen changes it, and
 * between them they are the whole of what D135 was waiting for: *"a screen
 * whose state is on the server — a fulfilment, a receipt, a finding — where a
 * deep link restores everything it names."* Selection used to live here and
 * nowhere else, so a refresh dropped the operator on the bare queue and the
 * evidence they were reading could not be sent to anybody.
 *
 * The tab is **not** in the URL. A tab is a filter and a filter is not a place
 * — the same argument `/orders` makes about `?reference=` — so it is derived
 * from the finding rather than carried beside it: see `viewFor` next door.
 */

export type FindingsStatus =
  | { kind: "loading" }
  | { kind: "ready"; findings: DiscrepancyRow[] }
  | { kind: "failed"; message: string };

export interface FindingsDesk {
  status: FindingsStatus;
  view: ViewKey;
  /** The finding the rail is showing, and the one the path names. */
  selected: DiscrepancyRow | null;
  /** What the operator has typed as a reason to accept the selected finding. */
  reason: string;
  busy: boolean;
  problem: string | null;
  /** Warnings from the last act — "it was already investigating" and the like,
   *  which are answers rather than failures and are shown as such. */
  said: string[] | null;
  look: (view: ViewKey) => void;
  select: (finding: DiscrepancyRow | null) => void;
  typeReason: (next: string) => void;
  dismiss: () => void;
  investigate: (id: Uuid) => Promise<void>;
  accept: (id: Uuid) => Promise<void>;
  /** Offer a photograph in support of the selected finding. D140. */
  attach: (image: Blob) => Promise<void>;
}

export function useFindings(
  at: string | null,
  place: (id: string | null) => void,
): FindingsDesk {
  const [view, setView] = useState<ViewKey>("live");
  const [status, setStatus] = useState<FindingsStatus>({ kind: "loading" });
  const [selected, setSelected] = useState<DiscrepancyRow | null>(null);
  /** The finding the path names that this screen could not read. Asked once. */
  const [missed, setMissed] = useState<string | null>(null);
  /**
   * Whether the first queue read is still waiting on the finding in the path.
   *
   * A link decides the tab, so reading the queue before the finding is known is
   * a read of the wrong one: three requests, and a visible flash from Live to
   * Closed for anybody following a link to a closed finding. This holds the
   * first read and nothing else — moving between findings afterwards keeps the
   * rows already on the screen, and re-reads only if the tab actually moves.
   */
  const [held, setHeld] = useState(at !== null);
  const [reason, setReason] = useState("");
  const [said, setSaid] = useState<string[] | null>(null);

  // **No re-read here**, because both acts answer with the whole finding and
  // the ones that need the list re-read it themselves — see `after`.
  const { busy, problem, dismiss, say, press } = useWriting();
  /** Which photograph this is, so two of one finding are two acts. */
  const photoAt = useRef(0);

  const live = useLive();

  const reload = useCallback(async (v: ViewKey): Promise<DiscrepancyRow[]> => {
    try {
      const findings = await api.findings(statesOf(v));
      if (live.current) setStatus({ kind: "ready", findings });
      return findings;
    } catch (error) {
      const message = phrase(error, "The findings could not be read.");
      if (live.current) setStatus({ kind: "failed", message });
      return [];
    }
  }, []);

  /** A finding has arrived from somewhere. Show it, on a tab that can. */
  const arrive = useCallback((f: DiscrepancyRow) => {
    setSelected(f);
    setReason("");
    setView((current) => viewFor(f.state, current));
  }, []);

  /** The path names a finding the rail is not showing and has not given up on. */
  const pending = at !== null && selected?.id !== at && missed !== at;

  /**
   * The finding in the path, made the finding on the rail.
   *
   * **The queue is asked first and the server second.** Following a link from
   * one row to another, or pressing Back between two, is a lookup in rows that
   * are already here; only a finding this screen has never read — a refresh, or
   * a link somebody was sent — costs a request. That request is what makes the
   * link work at all, because the queue answers a set of states and a link
   * names a finding without saying which state it is in.
   */
  useEffect(() => {
    if (at === null) {
      // Back, out of a finding: the panel closes and the queue keeps its rows.
      if (selected !== null) {
        setSelected(null);
        setReason("");
      }
      setHeld(false);
      return;
    }
    if (!pending) return;

    const here = status.kind === "ready" ? status.findings.find((f) => f.id === at) : undefined;
    if (here) {
      arrive(here);
      setHeld(false);
      return;
    }

    let stop = false;
    void (async () => {
      try {
        const finding = await api.finding(at);
        if (stop || !live.current) return;
        arrive(finding);
      } catch (error) {
        if (stop || !live.current) return;
        // **Written down, so it is asked once.** Without this the effect would
        // ask again on every render for a finding the server has already said
        // it does not have, which is a 404 in a loop.
        setMissed(at);
        say(
          error instanceof ApiError && error.status === 404
            ? "That finding is not here. It may belong to another company."
            : phrase(error, "That finding could not be read."),
        );
      } finally {
        if (!stop && live.current) setHeld(false);
      }
    })();
    return () => {
      stop = true;
    };
  }, [at, pending, selected, status, arrive, say]);

  useEffect(() => {
    if (held) return;
    setStatus({ kind: "loading" });
    void reload(view);
  }, [reload, view, held]);


  /**
   * Both acts answer with the whole finding, so the row and the rail are
   * updated from the response rather than re-read. **And the list is re-read
   * anyway**, because a finding that moves out of the current view has to
   * leave it — `investigate` moves a row off the Open tab, and a queue that
   * keeps showing it is a queue telling somebody to do a job that is done.
   */
  const after = useCallback(
    async (updated: DiscrepancyRow & { warnings: string[] }, v: ViewKey) => {
      if (!live.current) return;
      setSaid(updated.warnings.length > 0 ? updated.warnings : null);
      setSelected(updated);
      setReason("");
      await reload(v);
    },
    [reload],
  );

  return {
    status,
    view,
    selected,
    reason,
    busy,
    problem,
    said,

    look: (next) => {
      setView(next);
      // The rail keeps its finding across a tab change on purpose: the common
      // move after investigating something is to look at what else is open,
      // and losing the evidence you were reading to do that is the thing
      // "inspect without leaving" exists to prevent.
      dismiss();
      setSaid(null);
    },

    select: (finding) => {
      setSelected(finding);
      setReason("");
      dismiss();
      setSaid(null);
      // **The path is where the selection lives (D135).** Written here rather
      // than in an effect watching `selected`, so one history entry is one
      // click and Back is the row before this one.
      place(finding?.id ?? null);
    },

    typeReason: setReason,

    dismiss: () => {
      dismiss();
      setSaid(null);
    },

    investigate: (id) =>
      press(`investigate:${id}`, async () => {
        const updated = await api.investigate(id);
        await after(updated, view);
      }),

    accept: (id) =>
      press(`accept:${id}:${reason.trim()}`, async () => {
        const why = reason.trim();
        // **Refused here as well as by the server**, because the server's
        // refusal arrives after a round trip and this one arrives before the
        // operator has looked away. Same rule, said twice, and the server's is
        // the one that binds.
        if (!why) {
          throw new ApiError("Say why this can be accepted.", 400);
        }
        const updated = await api.accept(id, why);
        await after(updated, view);
      }),

    /**
     * **The subject is the most specific thing the finding names**, and the
     * screen says which so the operator can see it before they press.
     *
     * D140 refuses to infer the subject in the *writer*, because a finding
     * carries an item, a bin and a carton without those being exclusive and
     * only the person holding the camera knows which one is in front of them.
     * A screen choosing a sensible default from what the finding names is a
     * different thing from the database deciding — and the default is the
     * narrowest, because a photograph of a carton is a photograph of what is
     * in it and the reverse is not true.
     */
    attach: (image) =>
      press(`evidence:${selected?.id ?? "none"}:${image.size}:${image.type}:${photoAt.current}`, async (act) => {
        const f = selected;
        if (!f) throw new ApiError("Nothing is selected to attach that to.", 400);
        const subject = evidenceSubject(f);
        if (!subject) {
          throw new ApiError(
            "That finding names nothing to photograph.",
            400,
          );
        }
        // **The photograph is in the key, by size and time.** Two pictures of
        // one finding are two acts; the same picture sent again after a lost
        // response is one. A `Blob` has no identity, so this is the closest
        // honest stand-in — and it errs towards a new act, which costs a
        // duplicate photograph rather than a lost one.
        await api.attachEvidence({
          record: { discrepancy_id: f.id },
          subject,
          image,
          act,
        });
        photoAt.current += 1;
        // **Re-read and re-select, rather than patching the row in place.**
        // The digest the rail is about to draw comes from the finding read's
        // own aggregate, and a client that appended a guess would be drawing a
        // picture the server has not yet agreed exists.
        const rows = await reload(view);
        if (live.current) {
          setSelected(rows.find((r) => r.id === f.id) ?? f);
        }
      }),
  };
}

/**
 * What an evidence photograph of this finding is a photograph *of*.
 *
 * Narrowest first. Exported because the rail draws the answer beside the
 * camera: a control that silently picks one of three subjects is a control
 * whose record nobody can explain afterwards.
 */
export function evidenceSubject(
  f: DiscrepancyRow,
): { package_id: Uuid } | { location_id: Uuid } | { item_id: Uuid } | null {
  if (f.holder_package_id) return { package_id: f.holder_package_id };
  if (f.holder_location_id) return { location_id: f.holder_location_id };
  if (f.item_id) return { item_id: f.item_id };
  return null;
}

/** The word for it, for the label on the camera. */
export function evidenceSubjectName(f: DiscrepancyRow): string | null {
  if (f.holder_package_id) return f.package_barcode ?? "the carton";
  if (f.holder_location_id) return f.location_code ?? "the bin";
  if (f.item_id) return f.item_code ?? "the item";
  return null;
}
