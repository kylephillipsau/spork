import { useCallback, useEffect, useState } from "react";
import { useLive, useWriting } from "@app/acting";
import { ApiError, api, reason } from "@domain/api";
import type { PutawayCell, PutawayScreen, Uuid } from "@domain/types";
import { useSite } from "@app/session/SessionContext";
import { awayKey, fold, offered } from "./walk";

/**
 * Putting away, as logic.
 *
 * # The inversion, which is the whole shape of this screen
 *
 * **Picking is one destination and many sources**: the picker names the pallet
 * or the station once and every scan after it asks *is this the thing on the
 * row*. **Put-away is one source and many destinations**: everything starts on
 * the dock and each item has its own home, so the goods are selected and the
 * bin is scanned, once per trip.
 *
 * That is why the two screens read as mirror images and why neither could be
 * the other with the arguments swapped.
 *
 * # Where it goes is not answered here either
 *
 * `docs/inbound-analysis.md` sketches the directed version — a `putaway_policy`
 * table, a `location_occupancy` projection, six more columns on `location` — and
 * the competitor analysis warns about exactly that accretion. The location
 * survey those scores would read has not happened. So the operator names the
 * bin, and what the screen offers instead is `homes`: where this item already
 * lives. Information rather than instruction, and a scored suggestion can be
 * added later without changing what the ledger records.
 */

/** Where the goods are going. A location, always: a bin is not a package. */
export interface Bin {
  id: Uuid;
  code: string;
}

export type PutawayStatus =
  | { kind: "loading" }
  | { kind: "ready"; screen: PutawayScreen }
  | { kind: "failed"; message: string };

export interface PutawayBench {
  status: PutawayStatus;
  busy: boolean;
  problem: string | null;
  dismiss: () => void;

  /** What has been scanned but not yet resolved. */
  scan: { typed: string; refocus: number };
  typeScan: (next: string) => void;
  /** Resolve a scan: the goods when none are held, otherwise the bin. */
  read: (scanned: string) => Promise<void>;

  /** What is in the operator's hands, and where it is going. */
  holding: PutawayCell | null;
  bin: Bin | null;
  quantity: string;
  typeQuantity: (next: string) => void;
  choose: (cell: PutawayCell) => void;
  chooseBin: (bin: Bin) => void;
  release: () => void;

  /** Record it. */
  away: () => Promise<void>;
  /** What the last put-away did, for the dock to say so. */
  stowed: { code: string; quantity: number; bin: string } | null;

  refresh: () => Promise<void>;
}

export function usePutaway(): PutawayBench {
  const site = useSite();
  const [status, setStatus] = useState<PutawayStatus>({ kind: "loading" });
  const [scan, setScan] = useState({ typed: "", refocus: 0 });
  const [holding, setHolding] = useState<PutawayCell | null>(null);
  const [bin, setBin] = useState<Bin | null>(null);
  const [quantity, setQuantity] = useState("");
  const [stowed, setStowed] = useState<PutawayBench["stowed"]>(null);

  const live = useLive();
  const { busy, problem, dismiss, press } = useWriting();

  const load = useCallback(async () => {
    if (!site) return;
    try {
      const screen = await api.putaway(site);
      if (live.current) setStatus({ kind: "ready", screen });
    } catch (error) {
      const message = reason(error, "Could not load the dock list.");
      if (live.current) setStatus({ kind: "failed", message });
    }
  }, [site, live]);

  useEffect(() => {
    setStatus({ kind: "loading" });
    void load();
  }, [load]);

  const cells = status.kind === "ready" ? status.screen.cells : [];

  const choose = useCallback((cell: PutawayCell) => {
    setHolding(cell);
    setQuantity(String(offered(cell)));
    setBin(null);
    setStowed(null);
  }, []);

  return {
    status,
    busy,
    problem,
    dismiss: () => {
      dismiss();
      setStowed(null);
    },

    scan,
    typeScan: (next) => setScan((c) => ({ ...c, typed: next })),

    /**
     * One scanner, two questions, and which one is asked depends on whether
     * anything is in your hands.
     *
     * The goods first, because a bin scanned before there is anything to put in
     * it names nothing. After that every scan is *this is the bin*.
     */
    read: (scanned) =>
      press(`read:${scanned}:${holding ? "bin" : "goods"}`, async () => {
        const found = await api.resolve(scanned);
        if (!live.current) return;
        setScan((c) => ({ typed: "", refocus: c.refocus + 1 }));

        if (!holding) {
          const item = found.subjects.find((s) => s.kind === "item");
          if (!item) {
            throw new ApiError("Not an item. Scan the item.", 400);
          }
          // **The dock's own cells, not a second enumeration.** A scan cannot
          // select goods this list would not show, which is what keeps a
          // put-away from being recorded against stock that is not on the dock.
          const matches = cells.filter((c) => c.item_id === item.id);
          // The lot is checked when both sides know one: two pallets of the
          // same item under different lots are two cells, and putting the wrong
          // one away is a mistake that only surfaces at a recall.
          const onLot = found.lot
            ? matches.filter((c) => c.lot_code === null || c.lot_code === found.lot)
            : matches;
          const first = onLot[0];
          if (!first) {
            throw new ApiError(
              matches.length > 0 && found.lot
                ? `${item.code} is on the dock, but not under lot ${found.lot}.`
                : `${item.code} is not on the dock.`,
              400,
            );
          }
          choose(first);
          return;
        }

        // **A location, and its kind is not policed.** `POST /moves` checks that
        // the location exists and is this tenant's, and refuses a move to where
        // the goods already are. It does not check the kind, so neither does
        // this: a put-away onto another dock is a move somebody meant.
        const where = found.subjects.find((s) => s.kind === "location");
        if (!where) {
          throw new ApiError("Not a bin. Scan the bin label.", 400);
        }
        setBin({ id: where.id, code: where.code });
      }),

    holding,
    bin,
    quantity,
    typeQuantity: setQuantity,
    choose,
    chooseBin: setBin,
    release: () => {
      setHolding(null);
      setBin(null);
      setStowed(null);
    },

    away: () =>
      press(awayKey(holding, bin, quantity), async (act) => {
        if (!holding || !bin) return;
        const asked = Number.parseInt(quantity.trim(), 10);
        if (!Number.isFinite(asked) || asked <= 0) {
          throw new ApiError("Enter a quantity.", 400);
        }
        if (asked > holding.quantity) {
          throw new ApiError(
            `Only ${holding.quantity} on the dock. Cannot put away ${asked}.`,
            400,
          );
        }

        await api.putAway({
          stock: holding.stock_id,
          to: bin.id,
          quantity: asked,
          // **From a dock it is a put-away.** The word follows where the goods
          // came from rather than which screen recorded it, which is a fact
          // about what happened rather than a claim about intent.
          reason: "putaway",
          act,
        });
        if (!live.current) return;

        setStatus((s) =>
          s.kind === "ready"
            ? { kind: "ready", screen: fold(s.screen, holding.stock_id, asked) }
            : s,
        );
        setStowed({ code: holding.item_code, quantity: asked, bin: bin.code });
        setHolding(null);
        setBin(null);
        setQuantity("");
        setScan((c) => ({ typed: "", refocus: c.refocus + 1 }));
      }),

    stowed,
    refresh: async () => {
      dismiss();
      setStowed(null);
      await load();
    },
  };
}
