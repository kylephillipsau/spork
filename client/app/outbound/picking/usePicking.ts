import { useCallback, useEffect, useState } from "react";
import { useLive, useWriting } from "@app/acting";
import { ApiError, api, reason } from "@domain/api";
import type { PickLine, PickListScreen, Uuid } from "@domain/types";
import { claimFor, fold, offered } from "./walk";
import { useSite } from "@app/session/SessionContext";

/**
 * The pick walk, as logic.
 *
 * **It was a read and nothing else, and now it is not.** `crate::picking_list`
 * declined to record the pick because which carton a pick goes into is a
 * workflow question the model does not answer for somebody else's floor — pick
 * into the despatch carton, into a tote, or a whole wave into one cage are
 * three different screens. D166 answered it, and the answer was none of the
 * three: **the picker says where, by scanning it.**
 *
 * There is no route parameter for the site. The session already names one, the
 * same way the capture worklist and the despatch bench take theirs, and a
 * picker does not walk two sites in one shift.
 */

/**
 * Where the goods are being put down.
 *
 * **Exactly one arm** — the exclusive pair the ledger has carried since
 * migration 2. A pallet, a cage or a tote is a `package`; the packing station
 * is a `location`. A trolley is neither, deliberately: it is a person's hands
 * with wheels, and naming it as a holder would be inventing a fact to keep the
 * model tidy.
 *
 * Not a wire type. It reaches the server decomposed into `to_package_id` /
 * `to_location_id`, which is why it lives here rather than in `domain/types`.
 */
export interface Destination {
  kind: "package" | "location";
  id: Uuid;
  /** What the label said, so the dock can show back what was scanned. */
  code: string;
}

export type PickStatus =
  | { kind: "loading" }
  | { kind: "ready"; screen: PickListScreen }
  | { kind: "failed"; message: string };

export interface PickBench {
  status: PickStatus;
  busy: boolean;
  problem: string | null;
  dismiss: () => void;

  /**
   * Set once, at the start of the walk, and held until it is changed.
   *
   * Not asked per line: a picker loads a pallet or fills a trolley and walks,
   * and a screen that asked where each item was going after each item would be
   * asking a question whose answer it watched them give.
   */
  destination: Destination | null;
  /** What has been scanned but not yet resolved. */
  scan: { typed: string; refocus: number };
  typeScan: (next: string) => void;
  /** Resolve a scan: the destination when none is set, otherwise a line. */
  read: (scanned: string) => Promise<void>;
  clearDestination: () => void;

  /** The line a scan confirmed, and what the picker is about to take. */
  confirmed: PickLine | null;
  quantity: string;
  typeQuantity: (next: string) => void;
  choose: (line: PickLine) => void;
  release: () => void;

  /** Record it. Refuses until a destination and a line are both named. */
  take: () => Promise<void>;
  /** What the last pick did, for the dock to say so. */
  took: { code: string; quantity: number; warnings: string[] } | null;

  /** Fetch again. A walk list goes stale as other people pick. */
  refresh: () => Promise<void>;
}

export function usePicking(): PickBench {
  const site = useSite();
  const [status, setStatus] = useState<PickStatus>({ kind: "loading" });
  const [destination, setDestination] = useState<Destination | null>(null);
  const [scan, setScan] = useState({ typed: "", refocus: 0 });
  const [confirmed, setConfirmed] = useState<PickLine | null>(null);
  const [quantity, setQuantity] = useState("");
  const [took, setTook] = useState<PickBench["took"]>(null);

  const live = useLive();

  // **No re-read after a pick**, and this is the screen the rule was written
  // for: a pick changes what is left on the line it served, and refetching
  // would renumber the walk under somebody standing in an aisle. The row is
  // patched from the response instead, which is why `POST /picks` answers with
  // the live fold.
  const { busy, problem, dismiss, say, press } = useWriting();

  const load = useCallback(async () => {
    if (!site) return;
    try {
      const screen = await api.picking(site);
      if (live.current) setStatus({ kind: "ready", screen });
    } catch (error) {
      const message = reason(error, "The walk list could not be read.");
      if (live.current) setStatus({ kind: "failed", message });
    }
  }, [site, live]);

  useEffect(() => {
    setStatus({ kind: "loading" });
    void load();
  }, [load]);

  const lines = status.kind === "ready" ? status.screen.lines : [];

  const choose = useCallback((line: PickLine) => {
    setConfirmed(line);
    setQuantity(String(offered(line)));
    setTook(null);
  }, []);

  return {
    status,
    busy,
    problem,
    dismiss: () => {
      dismiss();
      setTook(null);
    },

    destination,
    scan,
    typeScan: (next) => setScan((c) => ({ ...c, typed: next })),
    clearDestination: () => {
      setDestination(null);
      setConfirmed(null);
      setTook(null);
    },

    /**
     * One scanner, two questions, and which one is asked depends on what has
     * been answered already.
     *
     * The destination first, because until it is named there is nothing to do
     * with an item. After that every scan is *is this the thing on the row*.
     * The alternative was two scan fields on one screen, which is two places to
     * aim a reader and one of them always wrong.
     */
    read: (scanned) =>
      press(`read:${scanned}:${destination ? "line" : "where"}`, async () => {
        const found = await api.resolve(scanned);
        if (!live.current) return;
        setScan((c) => ({ typed: "", refocus: c.refocus + 1 }));

        if (!destination) {
          // **A package or a location, and the server is not asked to police
          // it.** `picking::landing` checks that exactly one arm is named and
          // that the location exists; it does not check the kind, so neither
          // does this. A picker who scans a shelf gets a pick onto a shelf,
          // which is a move they meant to make often enough that refusing it
          // here would be this screen inventing a rule.
          const where = found.subjects.find(
            (s) => s.kind === "package" || s.kind === "location",
          );
          if (!where) {
            throw new ApiError(
              found.subjects.length > 0
                ? "That is an item. Scan the pallet, cage or station the goods are going onto."
                : "Nothing here holds that code.",
              400,
            );
          }
          setDestination({ kind: where.kind as Destination["kind"], id: where.id, code: where.code });
          return;
        }

        // **The walk's own rows, not a second enumeration.** A scan cannot
        // confirm a line the list would not show, which is what keeps a picker
        // from recording a pick against work that is not theirs to do.
        const item = found.subjects.find((s) => s.kind === "item");
        if (!item) {
          throw new ApiError("That is not an item. Scan the thing you are picking.", 400);
        }
        const matches = lines.filter((l) => l.item_id === item.id);
        if (matches.length === 0) {
          throw new ApiError(`${item.code} is not on this walk.`, 400);
        }
        // **The lot is checked when both sides know one.** A GS1-128 carton
        // label carries it beside the GTIN, so one scan answers two questions —
        // and the right item from the wrong lot is exactly the mistake worth
        // catching at the shelf rather than at the bench.
        const onLot = found.lot
          ? matches.filter((l) => l.lot_code === null || l.lot_code === found.lot)
          : matches;
        if (onLot.length === 0) {
          throw new ApiError(
            `${item.code} is on this walk, but lot ${found.lot} is not the one it wants.`,
            400,
          );
        }
        // Walk order, so two rows wanting one item confirm the nearer bin.
        const first = onLot[0];
        if (first) choose(first);
      }),

    confirmed,
    quantity,
    typeQuantity: setQuantity,
    choose,
    release: () => {
      setConfirmed(null);
      setTook(null);
    },

    take: () =>
      press(takeKey(confirmed, destination, quantity), async (act) => {
        if (!confirmed || !destination) return;
        if (!confirmed.stock_id) {
          throw new ApiError("There is nothing at this site to pick that from.", 400);
        }
        const asked = Number.parseInt(quantity.trim(), 10);
        if (!Number.isFinite(asked) || asked <= 0) {
          throw new ApiError("Say how many are in your hand.", 400);
        }
        if (asked > confirmed.remaining) {
          throw new ApiError(
            `The line wants ${confirmed.remaining}, and this would pick ${asked}.`,
            400,
          );
        }

        const answer = await api.pick({
          line: confirmed.fulfilment_line_id,
          stock: confirmed.stock_id,
          to: { kind: destination.kind, id: destination.id },
          quantity: asked,
          claim: claimFor(confirmed, asked),
          act,
        });
        if (!live.current) return;

        // **Patched, not re-read.** The response carries the live fold for the
        // line just served, so the row is corrected from what happened rather
        // than from a second query — and the walk keeps its order and its
        // position under somebody standing in an aisle.
        setStatus((s) => (s.kind === "ready" ? { kind: "ready", screen: fold(s.screen, confirmed.fulfilment_line_id, answer.ledger.picked_quantity, answer.ledger.covered_quantity) } : s));
        setTook({ code: confirmed.item_code, quantity: asked, warnings: answer.warnings });
        setConfirmed(null);
        setQuantity("");
        setScan((c) => ({ typed: "", refocus: c.refocus + 1 }));
      }),

    took,
    refresh: async () => {
      say(null);
      setTook(null);
      await load();
    },
  };
}

/**
 * The name of a pick.
 *
 * Subject, quantity and destination: a second press behind a lost response is
 * the same act and folds (D5), while the same line picked again into a
 * different pallet is a different act and rightly. Minted outside the run
 * because `press` needs the name before it starts — an act is identified by
 * what was asked for, not by what came back.
 */
function takeKey(
  line: PickLine | null,
  to: Destination | null,
  quantity: string,
): string {
  if (!line || !to) return "pick:none";
  return `pick:${line.fulfilment_line_id}:${line.stock_id ?? "nowhere"}:${quantity.trim()}:${to.kind}:${to.id}`;
}
