import { useCallback, useEffect, useState } from "react";
import { useLive, useWriting } from "@app/acting";
import { useChanges } from "@app/changes";
import { partOf } from "@domain/acts";
import { ApiError, api, reason } from "@domain/api";
import type { BenchScreen, Uuid } from "@domain/types";

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
  /** The carton "Add" puts things into. There is at most one. */
  openCarton: Uuid | null;
  /** An act is in flight; the keys are disabled and nothing else may start. */
  busy: boolean;
  /** What went wrong, in the sentence the handler wrote. */
  problem: string | null;
  dismiss: () => void;
  /** Read the bench again: something it shows changed elsewhere, such as a size
   *  recorded in the item drawer, which the suggested arrangement is made from. */
  refresh: () => void;
  startCarton: (preset: Uuid) => Promise<void>;
  addToCarton: (input: { line: Uuid; stock: Uuid; quantity: number }) => Promise<void>;
  /** Goods picked elsewhere: into the open carton, else to the staging spot. */
  handOver: (input: { line: Uuid; quantity: number }) => Promise<void>;
  /**
   * So many of a line's item, shipped as they are (migration 98, D196): whole
   * cartons, inner packs, or eaches in boxes of their own. Each one made at
   * the pack location as one of the item at that level, filled with what one
   * holds, and sealed, because it arrived sealed. From a bin when `stock` is
   * named, else handed over.
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
    stock?: Uuid;
  }) => Promise<void>;
  measure: (input: { carton: Uuid; weightKg?: string; heightMm?: string }) => Promise<void>;
  takeOut: (input: { picks: [Uuid, number][]; quantity: number }) => Promise<void>;
  seal: (carton: Uuid) => Promise<void>;
  discard: (carton: Uuid) => Promise<void>;
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
  const openCarton = screen?.cartons.find((carton) => !carton.sealed)?.id ?? null;

  return {
    status,
    openCarton,
    busy,
    problem,
    dismiss,
    refresh: () => void reload(),

    startCarton: (preset) =>
      press(`carton:${preset}`, async (act) => {
        const dock = screen?.dock_id;
        // D97 wants somewhere for `created` to name. A site with no location is
        // a fixture problem, and saying so beats a 400 from the handler.
        if (!dock) throw new ApiError("This site has no packing location.", 409);
        await api.startCarton({ fulfilment, preset, dock, act });
      }),

    addToCarton: ({ line, stock, quantity }) =>
      press(`pick:${line}:${stock}:${quantity}`, async (act) => {
        if (!openCarton) throw new ApiError("Start a carton first.", 409);
        if (!(quantity > 0)) throw new ApiError("How many?", 400);
        await api.pickInto({ line, stock, carton: openCarton, quantity, act });
      }),

    handOver: ({ line, quantity }) =>
      press(`handover:${line}:${quantity}`, async (act) => {
        if (!(quantity > 0)) throw new ApiError("Enter a quantity.", 400);
        if (openCarton) {
          await api.handOver({ line, quantity, carton: openCarton, act });
          return;
        }
        const staging = screen?.staging_id;
        if (!staging) throw new ApiError("Start a carton first. This site has no staging location.", 409);
        await api.handOver({ line, quantity, location: staging, act });
      }),

    shipAsIs: ({ line, item, level, config, units, count, stock }) =>
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
          const fill = partOf(act, `carton-${n}:fill`);
          if (stock) await api.pickInto({ line, stock, carton, quantity: units, act: fill });
          else await api.handOver({ line, quantity: units, carton, act: fill });
          await api.seal(carton, partOf(act, `carton-${n}:seal`));
        }
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

    takeOut: ({ picks, quantity }) =>
      // **The whole list, not its first movement.** A partial failure is
      // followed by a reload, and the recomputed picks can shrink or reorder —
      // so keying on `picks[0]` would mint a fresh act for a retry of the same
      // intent, and the per-movement names inside it would be fresh with it.
      press(`takeout:${picks.map(([movement]) => movement).join(",")}:${quantity}`, async (act) => {
        const reason = screen?.wrong_box_reason_id;
        if (!reason) throw new ApiError("No 'wrong location' reason is configured.", 409);
        await api.takeOut({ picks, quantity, reason, act });
      }),

    seal: (carton) =>
      press(`seal:${carton}`, (act) => api.seal(carton, act).then(() => undefined)),
    discard: (carton) =>
      press(`discard:${carton}`, (act) => api.discard(carton, act).then(() => undefined)),
  };
}
