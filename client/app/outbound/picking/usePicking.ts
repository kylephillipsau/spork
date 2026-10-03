import { useCallback, useEffect, useMemo, useState } from "react";
import { useLive, useWriting } from "@app/acting";
import { ApiError, api, reason } from "@domain/api";
import { transient } from "@domain/outbox";
import type { PickLine, PickListScreen, RecordPickResponse, Uuid } from "@domain/types";
import { useChanges } from "@app/changes";
import { picks, pressing, usePickOutbox, type HeldPick } from "@app/outbox";
import { claimFor, fold, merge, offered, overlay } from "./walk";
import { useSession, useSite } from "@app/session/SessionContext";

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
  /**
   * What the last pick did, for the dock to say so. `held` when it couldn't
   * be sent and is kept on the device instead (D207).
   */
  took: { code: string; quantity: number; warnings: string[]; held?: boolean } | null;

  /** Picks kept on this device until they can be sent (D207). */
  outbox: {
    waiting: number;
    refused: { key: string; code: string; quantity: number; where: string; reason: string }[];
    /** Somebody else's, waiting for them to sign in on this device. */
    others: { name: string; count: number }[];
    send: () => void;
    dismiss: (key: string) => void;
  };

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

  // Whose picks these are: a pick kept on the device is sent only as the
  // person, workspace and site that made it (D207).
  const session = useSession();
  const who = session.kind === "signed-in" && session.who.site_id ? session.who : null;
  const owner = who ? `${who.person_id}:${who.tenant_id}:${who.site_id}` : null;
  const outbox = usePickOutbox(owner, (entry, reply) => {
    // The row takes the server's figures before the kept pick stops counting,
    // so it doesn't flash back to what it was.
    if (live.current) setStatus((s) => (s.kind === "ready" ? { kind: "ready", screen: landed(s.screen, entry.body.line, reply) } : s));
  });

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
      const message = reason(error, "Could not load the pick list.");
      if (live.current) setStatus({ kind: "failed", message });
    }
  }, [site, live]);

  useEffect(() => {
    setStatus({ kind: "loading" });
    void load();
  }, [load]);

  // **Somebody else picked** (D206). Read again and lay it over the walk on
  // screen rather than replacing it, so every row stays where the picker saw
  // it. A failed read here says nothing: the next change, or Refresh, tries
  // again, and a banner for a background read is noise in an aisle.
  useChanges(() => {
    if (!site) return;
    void api.picking(site).then(
      (fresh) => {
        if (!live.current) return;
        setStatus((s) => ({ kind: "ready", screen: s.kind === "ready" ? merge(s.screen, fresh) : fresh }));
      },
      () => undefined,
    );
  });

  // What the server said, less what this device picked and hasn't sent.
  const shown = useMemo<PickStatus>(
    () =>
      status.kind === "ready"
        ? {
            kind: "ready",
            screen: overlay(
              status.screen,
              outbox.waiting.map((e) => ({ line: e.body.line, quantity: e.body.quantity, claim: e.body.claim })),
            ),
          }
        : status,
    [status, outbox.waiting],
  );
  const lines = shown.kind === "ready" ? shown.screen.lines : [];

  // The row about to be picked moves with the walk: fresh figures if somebody
  // else took some of it, and released, with a word, if they took the rest.
  useEffect(() => {
    if (!confirmed || shown.kind !== "ready") return;
    const now = shown.screen.lines.find((l) => l.fulfilment_line_id === confirmed.fulfilment_line_id);
    if (!now) {
      setConfirmed(null);
      say(`Somebody else has picked the rest of ${confirmed.item_code}.`);
    } else if (now !== confirmed) {
      setConfirmed(now);
      // What the picker typed stands, unless it is now more than is left.
      setQuantity((typed) => (Number.parseInt(typed, 10) > now.remaining ? String(offered(now)) : typed));
    }
  }, [shown]); // eslint-disable-line react-hooks/exhaustive-deps

  const choose = useCallback((line: PickLine) => {
    setConfirmed(line);
    setQuantity(String(offered(line)));
    setTook(null);
  }, []);

  return {
    status: shown,
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
                ? "That is an item. Scan the pallet, cage or station."
                : "No match for this code.",
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
          throw new ApiError("Not an item. Scan the item.", 400);
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
            `Wrong lot. Lot ${found.lot} of ${item.code} is not on this pick list.`,
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
          throw new ApiError("No stock of this item at this site.", 400);
        }
        const asked = Number.parseInt(quantity.trim(), 10);
        if (!Number.isFinite(asked) || asked <= 0) {
          throw new ApiError("Enter a quantity.", 400);
        }
        if (asked > confirmed.remaining) {
          throw new ApiError(
            `The line needs ${confirmed.remaining}. Cannot pick ${asked}.`,
            400,
          );
        }

        // **Kept on the device before it is sent** (D207). A dead spot, a
        // reload or a flat battery between here and the server's answer no
        // longer loses the record of goods already in the tote.
        const body: HeldPick = {
          line: confirmed.fulfilment_line_id,
          stock: confirmed.stock_id,
          to: { kind: destination.kind, id: destination.id },
          quantity: asked,
          claim: claimFor(confirmed, asked),
          ids: { event: act.id("event"), allocation: act.id("allocation") },
          at: act.at,
          code: confirmed.item_code,
          where: destination.code,
        };
        const key = body.ids.event;
        if (owner && who) picks.put({ key, owner, ownerName: who.display_name, body });

        let answer: RecordPickResponse;
        pressing.add(key);
        try {
          answer = await api.pick({ ...body, act });
        } catch (error) {
          // No answer: it stays kept, the walk counts it, and it goes when
          // the connection does. An answer is the server's, as before.
          if (owner && transient(error)) {
            if (!live.current) return;
            setTook({ code: body.code, quantity: asked, warnings: [], held: true });
            setConfirmed(null);
            setQuantity("");
            setScan((c) => ({ typed: "", refocus: c.refocus + 1 }));
            return;
          }
          picks.drop(key);
          throw error;
        } finally {
          pressing.delete(key);
        }

        // **Patched, not re-read.** The response carries the live fold for the
        // line just served, so the row is corrected from what happened rather
        // than from a second query — and the walk keeps its order and its
        // position under somebody standing in an aisle.
        if (live.current) setStatus((s) => (s.kind === "ready" ? { kind: "ready", screen: landed(s.screen, body.line, answer) } : s));
        picks.drop(key);
        if (!live.current) return;
        setTook({ code: confirmed.item_code, quantity: asked, warnings: answer.warnings });
        setConfirmed(null);
        setQuantity("");
        setScan((c) => ({ typed: "", refocus: c.refocus + 1 }));
        // It got through, so anything kept from before may too.
        void outbox.send();
      }),

    outbox: {
      waiting: outbox.waiting.length,
      refused: outbox.refused.map((e) => ({
        key: e.key,
        code: e.body.code,
        quantity: e.body.quantity,
        where: e.body.where,
        reason: e.refused ?? "",
      })),
      others: outbox.others,
      send: () => void outbox.send(),
      dismiss: outbox.dismiss,
    },

    took,
    refresh: async () => {
      say(null);
      setTook(null);
      await load();
    },
  };
}

/** A pick's reply laid onto the line it served. */
function landed(screen: PickListScreen, line: Uuid, reply: RecordPickResponse): PickListScreen {
  return fold(screen, line, reply.ledger.picked_quantity, reply.ledger.covered_quantity);
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
