import { useCallback, useEffect, useRef, useState } from "react";
import { useLive, useWriting } from "@app/acting";
import { useChanges } from "@app/changes";
import { kept, useOutbox } from "@app/outbox";
import { useSession } from "@app/session/SessionContext";
import { keptAct, partOf, uuid, type Act } from "@domain/acts";
import { ApiError, api, reason } from "@domain/api";
import type { Held } from "@domain/outbox";
import type { BenchScreen, PlannedParcel, Uuid } from "@domain/types";

import { boardOf, making, sourcesFor, suggest as suggested, type Outcome } from "./board";
import { Unwritable, sendOn, unset, writer, writesOf, type BenchWrite, type HeldPress } from "./writes";

/**
 * The pack bench, as logic.
 *
 * No JSX below this line and no fetching above it: `PackBench.tsx` takes what
 * this returns and emits primitives. That split is what lets the screen render
 * from a fixture with no network, which is what makes both densities and both
 * faces reviewable without a running warehouse.
 *
 * **Every act reloads the bench rather than patching state.** The maud page did
 * this by reloading the document and it was right for a reason that survives
 * the port: a pick moves stock, which moves availability, which moves what the
 * other lines may claim, and a client that predicts all of that is a second
 * implementation of the ledger's folds. The server holds the answer; asking it
 * costs one request.
 */

export type BenchStatus =
  | { kind: "loading" }
  | { kind: "ready"; screen: BenchScreen }
  | { kind: "failed"; message: string };

export interface PackBench {
  status: BenchStatus;
  /** An act is in flight; the keys are disabled and nothing else may start. */
  busy: boolean;
  /** What went wrong, in the sentence the handler wrote. */
  problem: string | null;
  dismiss: () => void;
  /** Read the bench again: something it shows changed elsewhere, such as a size
   *  recorded in the item drawer, which the suggested arrangement is made from. */
  refresh: () => void;
  /**
   * So many of a line's item, shipped as they are (migration 98, D196): whole
   * cartons, inner packs, or eaches in boxes of their own. Each one made at
   * the pack location as one of the item at that level, filled with what one
   * holds, and sealed, because it arrived sealed. Filled as the board puts
   * goods in: from the bench, then a bin, then handed over.
   */
  shipAsIs: (input: {
    line: Uuid;
    item: Uuid;
    level: "each" | "inner" | "carton";
    /** The case pack, for an inner or a carton: where the count comes from. */
    config: Uuid | null;
    /** Eaches in one. */
    units: number;
    count: number;
  }) => Promise<void>;
  /** A line checked (D244): what was shown, and what the packer saw. */
  check: (input: { line: Uuid; expected: number; seen: number }) => Promise<void>;
  measure: (input: { carton: Uuid; weightKg?: string; heightMm?: string }) => Promise<void>;
  seal: (carton: Uuid) => Promise<void>;
  discard: (carton: Uuid) => Promise<void>;

  /**
   * Do what a drop on the board does (D245): the plan kept, then the acts in
   * order, each its own part of one act so a retry replays what landed. What
   * it did can be undone.
   */
  apply: (outcome: Outcome, label: string) => Promise<void>;
  /** What Undo would undo, or nothing. */
  undoable: string | null;
  /** Go back one step: the plan as it was, the opposite acts (settled 5). */
  undo: () => Promise<void>;
  /** Make a planned parcel into a carton and put its goods in (D245). */
  make: (parcel: Uuid) => Promise<void>;
  /** Plan everything not placed by hand again round what was. */
  suggest: () => Promise<void>;
  /** Open a sealed carton again, to change what is in it. */
  open: (carton: Uuid) => Promise<void>;

  /**
   * Presses kept on this device until the server has them (D207). While one
   * of this person's waits, `busy` holds and the board waits for it.
   */
  kept: {
    waiting: KeptPress[];
    refused: (KeptPress & { reason: string })[];
    /** Somebody else's, waiting for them to sign in on this device. */
    others: { name: string; count: number }[];
    send: () => void;
    dismiss: (key: string) => void;
  };
}

/** A kept press, as the bench says it. */
export interface KeptPress {
  key: string;
  label: string;
  /** The order's number, for one made on another order's bench. */
  order: string;
}

/** A change done, and how to undo it. */
interface Done {
  label: string;
  undo: NonNullable<Outcome["undo"]>;
}

/** The bench's presses, kept on this device until the server has them (D207). */
export const presses = kept<HeldPress>("spork.outbox.bench");

/** One kept write, sent as its part of the press. */
function sendWrite(w: BenchWrite, act: Act): Promise<unknown> {
  switch (w.kind) {
    case "plan":
      return api.savePlan(w.fulfilment, w.plan);
    case "repack":
      return api.repack({ ...w, act });
    case "pickInto":
      return api.pickInto({ ...w, act });
    case "handOver":
      return api.handOver({ ...w, act });
    case "takeOut":
      return api.takeOut({ ...w, act });
    case "startCarton":
      return api.startCarton({ ...w, act });
    case "startAsIs":
      return api.startAsIs({ ...w, act });
    case "checkLine":
      return api.checkLine({ ...w, act });
    case "measure":
      return api.measure({ ...w, act });
    case "contain":
      return api.contain(w.carton, w.pallet, w.at, act);
    case "place":
      return api.placePackage(w.carton, w.location, act);
    case "seal":
      return api.seal(w.carton, act);
    case "open":
      return api.open(w.carton, act);
    case "discard":
      return api.discard(w.carton, act);
  }
}

/** A kept press sent on, each id and each landing kept in the outbox as it goes. */
const sendPress = (body: HeldPress, entry: Held<HeldPress>, mint?: (name: string) => Uuid) =>
  sendOn(body, sendWrite, () => presses.update(entry), mint);

/** What a press writes, and what to say once it is sent. */
interface Worked {
  writes: BenchWrite[];
  notes?: string[];
}

export function usePackBench(fulfilment: Uuid): PackBench {
  const [status, setStatus] = useState<BenchStatus>({ kind: "loading" });

  // A reload that lands after the component is gone is a state update on
  // nothing. Guarded rather than ignored, because React only warns about it in
  // development and the bench outlives a lot of navigations.
  const live = useLive();

  const reload = useCallback(async () => {
    try {
      const screen = await api.bench(fulfilment);
      if (live.current) setStatus({ kind: "ready", screen });
    } catch (error) {
      const message = reason(error, "Could not load the pack bench.");
      // Read again with no connection, the board stays as last read: a press
      // that couldn't go is kept, and the board waits for it.
      if (live.current) setStatus((s) => (s.kind === "ready" ? s : { kind: "failed", message }));
    }
  }, [fulfilment]);

  useEffect(() => {
    setStatus({ kind: "loading" });
    void reload();
  }, [reload]);

  // A pick for this order landing at the bench, or a carton sealed at the
  // next one, read as it happens (D206). The bench re-reads after its own acts
  // already, so this is the same read for somebody else's.
  useChanges(() => void reload());

  /**
   * One act at a time, and the bench is re-read afterwards whatever happened.
   *
   * **The key is (what is being done, to what), and it is held only between a
   * refusal and pressing again.** A press that lands releases its key, so
   * starting a second carton of the same preset is a new act, while pressing
   * again after a refusal is the same one. See `domain/acts.ts`. A press with
   * no answer is not pressed again: it is kept on the device and sent on as
   * the same act (D207, `send` below).
   *
   * **Re-reading after a failure is deliberate.** A write can fail having
   * partly landed — `pickInto` claims and then picks, and the claim may hold
   * while the pick does not. Leaving the screen showing what it hoped happened
   * is how a bench starts disagreeing with the ledger it is meant to describe.
   */
  const { busy, problem, dismiss, press } = useWriting(reload);

  const screen = status.kind === "ready" ? status.screen : null;

  // What can be undone, newest last. In memory: a reload keeps it, leaving the page does not.
  const done = useRef<Done[]>([]);
  const [undoable, setUndoable] = useState<string | null>(null);
  const remember = (step: Done | null) => {
    if (step) done.current.push(step);
    setUndoable(done.current[done.current.length - 1]?.label ?? null);
  };

  // Whose presses these are: a kept press is sent only as the person,
  // workspace and site that made it, as a kept pick is (D207).
  const session = useSession();
  const who = session.kind === "signed-in" && session.who.site_id ? session.who : null;
  const owner = who ? `${who.person_id}:${who.tenant_id}:${who.site_id}` : null;
  const outbox = useOutbox(presses, owner, sendPress);
  // A press on its way is the press, and `busy` says so; kept, it waits.
  const held = outbox.waiting.filter((e) => !presses.pressing(e.key));
  const waiting = held.length > 0;
  // Once nothing waits, read the bench again: what was kept has landed, or
  // was refused part way, having moved what it moved.
  const wasWaiting = useRef(waiting);
  useEffect(() => {
    if (wasWaiting.current && !waiting) void reload();
    wasWaiting.current = waiting;
  }, [waiting, reload]);

  /**
   * The plan, shown at once: the board answers a drop before the server
   * does, and the read after puts right anything it disagrees with.
   */
  const showPlan = (plan: PlannedParcel[]) => {
    if (live.current) setStatus((s) => (s.kind === "ready" ? { kind: "ready", screen: { ...s.screen, plan } } : s));
  };
  const planWrite = (plan: PlannedParcel[]): BenchWrite => ({ part: "plan", kind: "plan", fulfilment, plan });

  /**
   * What a board change writes: its plan kept first, then its acts, worked
   * out against the plan as it will be, so a pallet planned by the same drop
   * can be made.
   */
  const planned = (change: Pick<Outcome, "plan" | "acts">, at: BenchScreen): Worked => {
    const { writes, notes } = writesOf(change.acts, change.plan ? { ...at, plan: change.plan } : at, fulfilment);
    return { writes: change.plan ? [planWrite(change.plan), ...writes] : writes, notes };
  };

  /**
   * Keep a press on the device, then send it (D207).
   *
   * Its writes are worked out first, against the screen as read, and kept
   * with every id they mint. With no answer it stays kept, and the board
   * waits until it has gone; a refusal is shown, as before. What the writes
   * leave to say is said once they are sent. True once it has landed.
   *
   * Its ids are minted through the press's own act, so pressing again after
   * a refusal is the same act, as `acts.ts` requires.
   */
  const send = async (act: Act, at: BenchScreen, label: string, work: (act: Act) => Worked): Promise<boolean> => {
    const ids: Record<string, Uuid> = {};
    let worked: Worked;
    try {
      worked = work(keptAct(ids, act.at, undefined, act.id));
    } catch (error) {
      throw error instanceof Unwritable ? new ApiError(error.message, 409) : error;
    }
    if (worked.writes.length > 0) {
      const body: HeldPress = { label, order: at.order_reference, at: act.at, ids, writes: worked.writes, done: 0 };
      const entry: Held<HeldPress> = { key: uuid(), owner: owner ?? "", ownerName: who?.display_name ?? "", body };
      const now = (b: HeldPress, e: Held<HeldPress>) => sendPress(b, e, act.id);
      // Nobody to send it as later: sent now or not at all, as before.
      if (!owner) await now(body, entry);
      else if (!(await presses.now(entry, now))) return false;
    }
    const notes = worked.notes ?? [];
    if (notes.length > 0) throw new ApiError(notes.join(" "), 409);
    return true;
  };

  /** A press, unless a kept one is waiting: then the board waits for it, and this does nothing. */
  const hold = (key: string, run: (act: Act, at: BenchScreen) => Promise<unknown>): Promise<void> =>
    waiting || !screen
      ? Promise.resolve()
      : press(key, async (act) => {
          await run(act, screen);
        });

  /** A press of one write to a carton. */
  const toCarton = (kind: "seal" | "open" | "discard", verb: string) => (carton: Uuid) =>
    hold(`${kind}:${carton}`, (act, at) => send(act, at, `${verb} ${titleOf(at, carton)}`, () => ({ writes: [{ part: "", kind, carton }] })));

  return {
    status,
    busy: busy || waiting,
    problem,
    dismiss,
    refresh: () => void reload(),

    shipAsIs: ({ line, item, level, config, units, count }) =>
      hold(`own:${line}:${level}:${count}`, async (act, at) => {
        const label = `Ship ${count} × ${codeOf(at, line)} as ${count === 1 ? "it is" : "they are"}`;
        // Undone newest first, each taken apart.
        const cartons: Uuid[] = [];
        const landed = await send(act, at, label, (kept) => {
          const dock = at.dock_id;
          if (!dock) throw unset();
          if (!(count > 0) || !(units > 0)) throw new Unwritable("How many?");
          const l = at.lines.find((x) => x.line_id === line);
          const { short } = l ? sourcesFor(l, units * count) : { short: units * count };
          if (short > 0) throw new Unwritable(`Only ${units * count - short} of ${units * count} ${codeOf(at, line)} are here.`);
          // One at a time, whole: made, filled, sealed. Stopped part way, the
          // ones before are complete and this one open, and sent on it
          // carries on from the write it stopped at.
          const w = writer(at, fulfilment);
          for (let n = 1; n <= count; n++) {
            w.push({ part: `carton-${n}:make`, kind: "startAsIs", fulfilment, item, level, config: level === "each" ? null : config, dock });
            const carton = partOf(kept, `carton-${n}:make`).id("package");
            cartons.unshift(carton);
            w.add({ kind: "put", line, units, carton }, `carton-${n}:fill`);
            w.push({ part: `carton-${n}:seal`, kind: "seal", carton });
          }
          return w;
        });
        if (landed) remember({ label, undo: { plan: null, acts: cartons.map((carton) => ({ kind: "unmake", carton, pallet: null })) } });
      }),

    check: ({ line, expected, seen }) =>
      hold(`check:${line}:${expected}:${seen}`, (act, at) =>
        send(act, at, `Check ${codeOf(at, line)}: ${seen}`, () => {
          if (!(seen >= 0)) throw new Unwritable("How many do you see?");
          return { writes: [{ part: "", kind: "checkLine", line, expected, seen }] };
        }),
      ),

    measure: ({ carton, weightKg, heightMm }) =>
      hold(`measure:${carton}`, (act, at) =>
        send(act, at, `Weigh ${titleOf(at, carton)}`, () => {
          if (!weightKg && !heightMm) throw new Unwritable("Enter a weight.");
          return { writes: [{ part: "", kind: "measure", carton, ...(weightKg ? { weightKg } : {}), ...(heightMm ? { heightMm } : {}) }] };
        }),
      ),

    seal: toCarton("seal", "Seal"),
    open: toCarton("open", "Open"),
    discard: toCarton("discard", "Discard"),

    apply: (outcome, label) =>
      hold(`apply:${label}:${JSON.stringify(outcome.acts)}`, async (act, at) => {
        if (outcome.plan) showPlan(outcome.plan);
        const landed = await send(act, at, label, () => planned(outcome, at));
        // Undone only once it has landed: a kept press may yet be refused.
        const undo = outcome.undo;
        if (landed) remember(undo && (undo.plan || undo.acts.length > 0) ? { label, undo } : null);
      }),

    undoable,
    undo: () => {
      const last = done.current[done.current.length - 1];
      if (!last) return Promise.resolve();
      return hold(`undo:${done.current.length}:${last.label}`, async (act, at) => {
        if (last.undo.plan) showPlan(last.undo.plan);
        await send(act, at, `Undo: ${last.label}`, () => planned(last.undo, at));
        // Landed or kept, it is not there to undo again.
        done.current.pop();
        remember(null);
      });
    },

    make: (parcel) =>
      hold(`make:${parcel}`, async (act, at) => {
        const label = `Make ${titleOf(at, parcel)}`;
        const made = making(at, parcel, uuid);
        if (await send(act, at, label, () => planned(made, at))) remember(made.undo && { label, undo: made.undo });
      }),

    suggest: () =>
      hold(`suggest:${screen?.plan.map((p) => p.id).join(",") ?? ""}`, async (act, at) => {
        const before = at.plan;
        const plan = suggested(at, boardOf(at), uuid);
        showPlan(plan);
        if (await send(act, at, "Suggest", () => ({ writes: [planWrite(plan)] }))) remember({ label: "Suggest", undo: { plan: before, acts: [] } });
      }),

    kept: {
      waiting: held.map((e) => ({ key: e.key, label: e.body.label, order: e.body.order })),
      refused: outbox.refused.map((e) => ({ key: e.key, label: e.body.label, order: e.body.order, reason: e.refused ?? "" })),
      others: outbox.others,
      send: () => void outbox.send(),
      dismiss: outbox.dismiss,
    },
  };
}

/** A line's item code, to say what a press did. */
function codeOf(at: BenchScreen, line: Uuid): string {
  return at.lines.find((l) => l.line_id === line)?.item_code ?? "it";
}

/** A parcel's name on the board, to say what a press did. */
function titleOf(at: BenchScreen, parcel: Uuid): string {
  return boardOf(at).parcels.find((p) => p.id === parcel)?.title ?? "the carton";
}
