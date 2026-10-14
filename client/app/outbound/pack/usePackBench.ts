import { useCallback, useEffect, useRef, useState } from "react";
import { useLive, useWriting } from "@app/acting";
import { useChanges } from "@app/changes";
import { partOf, uuid, type Act } from "@domain/acts";
import { ApiError, api, reason } from "@domain/api";
import type { BenchScreen, PlannedParcel, Uuid } from "@domain/types";

import { boardOf, sourcesFor, suggest as suggested, type BoardAct, type Outcome } from "./board";

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
}

/** A change done, and how to undo it. */
interface Done {
  label: string;
  undo: NonNullable<Outcome["undo"]>;
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
      if (live.current) setStatus({ kind: "failed", message });
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
   * The acts this screen has begun and not yet landed.
   *
   * **The key is (what is being done, to what), and it is held only between a
   * failure and its retry.** A press that succeeds releases its key, so
   * starting a second carton of the same preset, or picking the same line
   * twice on purpose, is a new act — while pressing again after a lost
   * response is the same one. See `domain/acts.ts`.
   */
  /**
   * One act at a time, and the bench is re-read afterwards whatever happened.
   *
   * **Re-reading after a failure is deliberate.** A write can fail having
   * partly landed — `pickInto` claims and then picks, and the claim may hold
   * while the pick does not. Leaving the screen showing what it hoped happened
   * is how a bench starts disagreeing with the ledger it is meant to describe.
   *
   * And that partial landing is exactly why the identity has to survive the
   * failure: the retry has to arrive as the same act, or the claim that held
   * becomes a second claim.
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

  /**
   * The plan kept, shown at once: the board answers a drop before the
   * server does, and the read after puts right anything it disagrees with.
   */
  const keepPlan = async (plan: PlannedParcel[]) => {
    if (live.current) setStatus((s) => (s.kind === "ready" ? { kind: "ready", screen: { ...s.screen, plan } } : s));
    await api.savePlan(fulfilment, plan);
  };

  const unset = () => new ApiError("This site doesn't say where it packs yet. Set it in Workspace.", 409);

  /** One board act, against the screen as it was read. */
  const run = async (step: BoardAct, act: Act, at: BenchScreen): Promise<void> => {
    const bench = at.staging_id;
    switch (step.kind) {
      case "put": {
        const line = at.lines.find((l) => l.line_id === step.line);
        if (!line) throw new ApiError("That line is not on this order any more.", 409);
        const { sources, short } = sourcesFor(line, step.units);
        if (sources.length === 0) throw new ApiError(`No ${line.item_code} is here to put in.`, 409);
        for (const [i, src] of sources.entries()) {
          const part = partOf(act, `${i}:${src.kind}`);
          if (src.kind === "bench") {
            if (!bench) throw unset();
            await api.repack({ line: line.line_id, from: { location: bench }, to: { package: step.carton }, quantity: src.units, act: part });
          } else if (src.kind === "cell") {
            await api.pickInto({ line: line.line_id, stock: src.stock, carton: step.carton, quantity: src.units, act: part });
          } else {
            await api.handOver({ line: line.line_id, quantity: src.units, carton: step.carton, act: part });
          }
        }
        if (short > 0) throw new ApiError(`${step.units - short} of ${step.units} ${line.item_code} went in: the rest is not here.`, 409);
        return;
      }
      case "unput": {
        const picks = (at.cartons.find((c) => c.id === step.carton)?.contents ?? []).filter((r) => r.line_id === step.line).flatMap((r) => r.picks);
        const reason = at.wrong_box_reason_id;
        if (!reason) throw new ApiError("No 'wrong location' reason is configured.", 409);
        await api.takeOut({ picks, quantity: step.units, reason, act });
        return;
      }
      case "repack": {
        if (!step.to && !bench) throw unset();
        await api.repack({
          line: step.line,
          from: { package: step.from },
          to: step.to ? { package: step.to } : { location: bench! },
          quantity: step.units,
          act,
        });
        return;
      }
      case "make": {
        const pp = at.plan.find((p) => p.id === step.parcel);
        // Made already, by a retry that landed.
        if (!pp) return;
        if (!at.dock_id) throw unset();
        // On a planned pallet: the pallet first, so there is something to put it on.
        if (pp.on_parcel_id) await run({ kind: "make", parcel: pp.on_parcel_id }, partOf(act, "pallet"), at);
        await api.startCarton({ fulfilment, preset: pp.package_type_id, dock: at.dock_id, act: partOf(act, "carton"), id: pp.id });
        const pallet = pp.on_parcel_id ?? pp.on_package_id;
        if (pallet) await api.contain(pp.id, pallet, pp.at, partOf(act, "on"));
        for (const l of pp.lines) {
          await run({ kind: "put", line: l.line_id, units: l.quantity, carton: pp.id }, partOf(act, `put:${l.line_id}`), at);
        }
        return;
      }
      case "contain":
        await api.contain(step.carton, step.pallet, null, act);
        return;
      case "takeOff":
        if (!bench) throw unset();
        await api.placePackage(step.carton, bench, act);
        return;
    }
  };

  const applying = async (outcome: Pick<Outcome, "plan" | "acts">, act: Act, at: BenchScreen) => {
    if (outcome.plan) await keepPlan(outcome.plan);
    for (const [i, step] of outcome.acts.entries()) await run(step, partOf(act, `${i}:${step.kind}`), at);
  };

  return {
    status,
    busy,
    problem,
    dismiss,
    refresh: () => void reload(),

    shipAsIs: ({ line, item, level, config, units, count }) =>
      press(`own:${line}:${level}:${count}`, async (act) => {
        const dock = screen?.dock_id;
        if (!dock) throw new ApiError("This site doesn't say where it packs yet. Set it in Workspace.", 409);
        if (!(count > 0) || !(units > 0)) throw new ApiError("How many?", 400);
        // One at a time, whole: made, filled, sealed. A failure part way
        // leaves the ones before it complete and this one open, and a retry
        // replays every part already done as the same write.
        for (let n = 1; n <= count; n++) {
          const make = partOf(act, `carton-${n}:make`);
          await api.startAsIs({ fulfilment, item, level, config: level === "each" ? null : config, dock, act: make });
          const carton = make.id("package");
          if (screen) await run({ kind: "put", line, units, carton }, partOf(act, `carton-${n}:fill`), screen);
          await api.seal(carton, partOf(act, `carton-${n}:seal`));
        }
      }),

    check: ({ line, expected, seen }) =>
      press(`check:${line}:${expected}:${seen}`, async (act) => {
        if (!(seen >= 0)) throw new ApiError("How many do you see?", 400);
        await api.checkLine({ line, expected, seen, act });
      }),

    measure: ({ carton, weightKg, heightMm }) =>
      press(`measure:${carton}`, async (act) => {
        if (!weightKg && !heightMm) throw new ApiError("Enter a weight.", 400);
        await api.measure({
          carton,
          ...(weightKg ? { weightKg } : {}),
          ...(heightMm ? { heightMm } : {}),
          act,
        });
      }),

    seal: (carton) =>
      press(`seal:${carton}`, (act) => api.seal(carton, act).then(() => undefined)),
    open: (carton) =>
      press(`open:${carton}`, (act) => api.open(carton, act).then(() => undefined)),

    apply: (outcome, label) =>
      press(`apply:${label}:${JSON.stringify(outcome.acts)}`, async (act) => {
        if (!screen) return;
        await applying(outcome, act, screen);
        const undo = outcome.undo;
        remember(undo && (undo.plan || undo.acts.length > 0) ? { label, undo } : null);
      }),

    undoable,
    undo: () => {
      const last = done.current[done.current.length - 1];
      if (!last) return Promise.resolve();
      return press(`undo:${done.current.length}:${last.label}`, async (act) => {
        if (!screen) return;
        await applying(last.undo, act, screen);
        done.current.pop();
        remember(null);
      });
    },

    make: (parcel) =>
      press(`make:${parcel}`, async (act) => {
        if (!screen) return;
        await run({ kind: "make", parcel }, act, screen);
      }),

    suggest: () =>
      press(`suggest:${screen?.plan.map((p) => p.id).join(",") ?? ""}`, async () => {
        if (!screen) return;
        const before = screen.plan;
        await keepPlan(suggested(screen, boardOf(screen), uuid));
        remember({ label: "Suggest", undo: { plan: before, acts: [] } });
      }),
    discard: (carton) =>
      press(`discard:${carton}`, (act) => api.discard(carton, act).then(() => undefined)),
  };
}
