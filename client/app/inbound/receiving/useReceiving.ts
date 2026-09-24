import { useCallback, useEffect, useState } from "react";
import { useLive, useWriting } from "@app/acting";
import { ApiError, api, reason } from "@domain/api";
import type { ExpectedLine, ReceivingScreen, Uuid } from "@domain/types";
import { useSite } from "@app/session/SessionContext";
import { baseUnits, fold, levelNamed, receiptKey, whatIsMissing } from "./delivery";
import { witness } from "@app/measurement/baseline";

/**
 * Checking a delivery in, as logic.
 *
 * # The delivery is a session, and that is the whole structure
 *
 * D43 and Q172: one delivery is one `goods_receipt`, and lines join by carrying
 * the same client-minted id. So a truck is a thing the screen holds open — the
 * first line mints the id and every line after it repeats it, which is what
 * makes a five-line drop one delivery rather than five. Closing it is the
 * operator's act, because only they can see that the last pallet is off.
 *
 * # Where the goods land is asked once
 *
 * The same argument as the pick walk's destination: goods are checked in at one
 * place for the length of a delivery, and a screen asking per line would be
 * asking a question it had watched somebody answer. `POST /receipts` takes
 * "dock or bin", so a floor that receives straight to the shelf scans a bin here
 * and never opens the put-away list at all.
 */

/** Where the goods are being put down while they are checked in. */
export interface Bay {
  id: Uuid;
  code: string;
}

export type ReceivingStatus =
  | { kind: "loading" }
  | { kind: "ready"; screen: ReceivingScreen }
  | { kind: "failed"; message: string };

/** What the last line did, which is more than "it worked". */
export interface Landed {
  code: string;
  /** Base units the server wrote, after its own conversion. */
  quantity: number;
  entered: string;
  /** False when the policy refused it — `lot_missing` is the case. */
  accepted: boolean;
  finding: Uuid | null;
  warnings: string[];
}

export interface ReceivingBench {
  status: ReceivingStatus;
  busy: boolean;
  problem: string | null;
  dismiss: () => void;

  /** The delivery being checked in. Null until the first line lands. */
  delivery: Uuid | null;
  /** Where it is being put down, named once. */
  bay: Bay | null;
  closeDelivery: () => void;

  scan: { typed: string; refocus: number };
  typeScan: (next: string) => void;
  /** Resolve a scan: the bay when none is set, otherwise a line and its lot. */
  read: (scanned: string) => Promise<void>;

  counting: ExpectedLine | null;
  choose: (line: ExpectedLine) => void;
  release: () => void;

  entered: string;
  typeEntered: (next: string) => void;
  level: string;
  chooseLevel: (level: string) => void;
  lotCode: string;
  typeLot: (next: string) => void;
  lotExpiry: string;
  typeExpiry: (next: string) => void;
  owner: Uuid | null;
  chooseOwner: (owner: Uuid) => void;

  /** What this count comes to in base units, shown before the press. */
  base: number | null;
  /** What the scale says, as typed. A string, never a number — the same
   *  argument `observing.rs` makes about `entered_value`. */
  weighed: string;
  typeWeighed: (next: string) => void;
  /** The scale's own reading of the count, when there is a baseline to divide
   *  by and something on the scale. Null is the ordinary case. */
  scale: ReturnType<typeof witness>;
  /** What is stopping it. Null when ready. */
  missing: string | null;

  receive: () => Promise<void>;
  /** Close the promise short: what was ordered is not all coming. */
  closeShort: () => Promise<void>;
  landed: Landed | null;

  refresh: () => Promise<void>;
}

export function useReceiving(): ReceivingBench {
  const site = useSite();
  const [status, setStatus] = useState<ReceivingStatus>({ kind: "loading" });
  const [delivery, setDelivery] = useState<Uuid | null>(null);
  const [bay, setBay] = useState<Bay | null>(null);
  const [scan, setScan] = useState({ typed: "", refocus: 0 });
  const [counting, setCounting] = useState<ExpectedLine | null>(null);
  const [entered, setEntered] = useState("");
  const [level, setLevel] = useState("each");
  const [weighed, setWeighed] = useState("");
  const [lotCode, setLotCode] = useState("");
  const [lotExpiry, setLotExpiry] = useState("");
  const [owner, setOwner] = useState<Uuid | null>(null);
  const [landed, setLanded] = useState<Landed | null>(null);

  const live = useLive();
  const { busy, problem, dismiss, press } = useWriting();

  const load = useCallback(async () => {
    if (!site) return;
    try {
      const screen = await api.receiving(site);
      if (live.current) setStatus({ kind: "ready", screen });
    } catch (error) {
      const message = reason(error, "Could not load expected deliveries.");
      if (live.current) setStatus({ kind: "failed", message });
    }
  }, [site, live]);

  useEffect(() => {
    setStatus({ kind: "loading" });
    void load();
  }, [load]);

  const lines = status.kind === "ready" ? status.screen.lines : [];

  const choose = useCallback((line: ExpectedLine) => {
    setCounting(line);
    setEntered("");
    setLevel("each");
    setWeighed("");
    setLotCode("");
    setLotExpiry("");
    // The promise's own owner wins; otherwise the one already holding this item
    // here, when there is exactly one. Two is a question, not a default.
    setOwner(
      line.owner_id ?? (line.owners.length === 1 ? (line.owners[0]?.owner_id ?? null) : null),
    );
    setLanded(null);
  }, []);

  const base = counting ? baseUnits(entered, levelNamed(counting, level)) : null;

  // **The third witness.** The paperwork advised, the receiver counted, and the
  // scale is the observation belonging to neither of them. Nothing is written
  // from this: see `baseline.ts`, and the comment on the weight field about
  // what has to be on the scale for it to mean anything.
  const chosen = counting ? levelNamed(counting, level) : null;
  const kg = Number.parseFloat(weighed.trim());
  const scale = witness(
    weighed.trim() === "" || !Number.isFinite(kg) ? null : Math.round(kg * 1000),
    chosen?.baseline ?? null,
    Number.isInteger(Number.parseInt(entered.trim(), 10))
      ? Number.parseInt(entered.trim(), 10)
      : null,
    level,
  );
  const missing = counting ? whatIsMissing(counting, entered, lotCode, owner) : null;

  const put = useCallback(
    (supply: Uuid, moved: number) =>
      setStatus((s) =>
        s.kind === "ready"
          ? {
              kind: "ready",
              screen: { ...s.screen, lines: fold(s.screen.lines, supply, moved) },
            }
          : s,
      ),
    [],
  );

  return {
    status,
    busy,
    problem,
    dismiss: () => {
      dismiss();
      setLanded(null);
    },

    delivery,
    bay,
    closeDelivery: () => {
      // **A new truck is a new header.** Dropping the id is the whole act: the
      // next line mints a fresh one, and D43's "one delivery, one receipt" holds
      // without anything needing to be told the truck has gone.
      setDelivery(null);
      setCounting(null);
      setLanded(null);
    },

    scan,
    typeScan: (next) => setScan((c) => ({ ...c, typed: next })),

    read: (scanned) =>
      press(`read:${scanned}:${bay ? "line" : "bay"}`, async () => {
        const found = await api.resolve(scanned);
        if (!live.current) return;
        setScan((c) => ({ typed: "", refocus: c.refocus + 1 }));

        if (!bay) {
          const where = found.subjects.find((s) => s.kind === "location");
          if (!where) {
            throw new ApiError("Not a location. Scan the dock or a bin.", 400);
          }
          setBay({ id: where.id, code: where.code });
          return;
        }

        const item = found.subjects.find((s) => s.kind === "item");
        if (!item) {
          throw new ApiError("Not an item. Scan the item.", 400);
        }
        const line = lines.find((l) => l.item_id === item.id);
        if (!line) {
          throw new ApiError(`${item.code} is not expected here.`, 400);
        }
        choose(line);
        // **One scan answers three questions.** A GS1-128 carries the lot and
        // the expiry beside the GTIN, and this is the first write path to use
        // them — `Resolution` has carried both since the locator was built.
        if (found.lot) setLotCode(found.lot);
        if (found.expiry) setLotExpiry(found.expiry);
      }),

    counting,
    choose,
    release: () => {
      setCounting(null);
      setLanded(null);
    },

    entered,
    typeEntered: setEntered,
    weighed,
    typeWeighed: setWeighed,
    scale,
    level,
    chooseLevel: setLevel,
    lotCode,
    typeLot: setLotCode,
    lotExpiry,
    typeExpiry: setLotExpiry,
    owner,
    chooseOwner: setOwner,
    base,
    missing,

    receive: () =>
      press(receiptKey(counting, delivery ?? "new", entered, level, lotCode), async (act) => {
        if (!counting || !bay) return;
        if (missing) throw new ApiError(missing, 400);
        const chosen = levelNamed(counting, level);
        const count = Number.parseInt(entered.trim(), 10);

        // **The first line of a truck mints the header id from its own act.**
        // Not a fresh uuid: a replayed first line has to reach the same header,
        // or a lost response turns one delivery into two — the exact thing D43
        // and Q172 built the shared id to prevent. `act.id` is memoised per act
        // and the act per key, so the second press names what the first named.
        const header = delivery ?? act.id("delivery");

        const answer = await api.receive({
          supply: counting.expected_supply_id,
          to: bay.id,
          delivery: header,
          entered: count,
          level,
          config: chosen?.item_packing_config_id ?? null,
          owner: counting.owner_id ? null : owner,
          lotCode: lotCode.trim() || null,
          lotExpiry: lotExpiry.trim() || null,
          act,
        });
        if (!live.current) return;

        setDelivery(answer.goods_receipt_id);
        setLanded({
          code: counting.item_code,
          quantity: answer.quantity,
          entered: `${count} ${level}`,
          accepted: answer.accepted,
          finding: answer.discrepancy_id,
          warnings: answer.warnings,
        });
        // **Only an accepted line moved anything.** A refusal leaves the promise
        // where it was, and folding it would show progress the ledger does not
        // have — which is the state a receiver would then try to put away.
        if (answer.accepted) {
          put(counting.expected_supply_id, answer.quantity);
          setCounting(null);
          setEntered("");
          setWeighed("");
          setLotCode("");
          setLotExpiry("");
        }
        setScan((c) => ({ typed: "", refocus: c.refocus + 1 }));
      }),

    closeShort: () =>
      press(`close-short:${counting?.expected_supply_id ?? "none"}`, async (act) => {
        if (!counting || !bay) return;
        // **Nothing more is coming.** A zero-quantity receipt would be a lie
        // about a movement; closing the promise short is the act that says the
        // rest is not on its way, and the shortfall lands in
        // `quantity_closed_short` where a supplier conversation can find it.
        const answer = await api.receive({
          supply: counting.expected_supply_id,
          to: bay.id,
          delivery: delivery ?? act.id("delivery"),
          entered: 0,
          level: "each",
          owner: counting.owner_id ? null : owner,
          closePromise: true,
          act,
        });
        if (!live.current) return;
        setDelivery(answer.goods_receipt_id);
        put(counting.expected_supply_id, counting.outstanding);
        setCounting(null);
        setLanded({
          code: counting.item_code,
          quantity: 0,
          entered: "closed short",
          accepted: answer.accepted,
          finding: answer.discrepancy_id,
          warnings: answer.warnings,
        });
      }),

    landed,
    refresh: async () => {
      dismiss();
      setLanded(null);
      await load();
    },
  };
}
