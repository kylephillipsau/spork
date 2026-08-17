import { useCallback, useEffect, useState } from "react";
import { useLive, useWriting } from "@app/acting";
import { ApiError, api, reason } from "@domain/api";
import type { BookedCarton, DespatchScreen, Uuid, CarrierLine } from "@domain/types";
import { useSite } from "@app/session/SessionContext";

/**
 * The despatch bench, as logic.
 *
 * Same split as the pack bench: this fetches and mutates and returns data, the
 * component takes data and emits primitives, and the presentational half
 * renders from a fixture with no network.
 *
 * Every act re-reads afterwards, including after a failure — a consignment can
 * fail having partly landed, and a screen showing what it hoped happened is how
 * a bench starts disagreeing with the ledger.
 */

export type DespatchStatus =
  | { kind: "loading" }
  | { kind: "ready"; screen: DespatchScreen }
  | { kind: "failed"; message: string };

export interface DespatchBench {
  status: DespatchStatus;
  busy: boolean;
  problem: string | null;
  /** What the last booking produced. **The carrier lines themselves, not a
   *  count of them.** They are the whole output of consigning: the weight and
   *  dimensions somebody confirmed at the bench, folded into the shape a
   *  freight provider is handed. Keeping only the length threw away the one
   *  thing worth showing. */
  booked: {
    warnings: string[];
    packages: number;
    carrier: string | null;
    service: string | null;
    total_gross_weight_g: number | null;
    lines: CarrierLine[];
  } | null;
  /** Whether the notice about the last act is still up. **Separate from
   *  `booked` because dismissing a notice used to discard the manifest with
   *  it** — the banner is a passing remark, the manifest is the result of the
   *  act, and one Dismiss should not take both. */
  notice: boolean;
  dismiss: () => void;
  consign: (input: {
    packages: Uuid[];
    carrier?: Uuid;
    service?: Uuid;
    despatchAt?: string;
  }) => Promise<void>;
  despatchCarton: (carton: BookedCarton) => Promise<void>;
  despatchAll: (packages: BookedCarton[]) => Promise<void>;
}

/**
 * **No site argument.** Where you are despatching from is where you signed on,
 * which the session says — the route table used to supply a constant, so every
 * live despatch bench showed one fixed warehouse. `useSite()` returning null
 * means the session has not resolved yet, not that there is nothing to do.
 */
export function useDespatch(): DespatchBench {
  const site = useSite();
  const [status, setStatus] = useState<DespatchStatus>({ kind: "loading" });
  const [booked, setBooked] = useState<DespatchBench["booked"]>(null);
  const [notice, setNotice] = useState(false);

  const live = useLive();

  const reload = useCallback(async () => {
    if (!site) return;
    try {
      const screen = await api.despatchBench(site);
      if (live.current) setStatus({ kind: "ready", screen });
    } catch (error) {
      const message = reason(error, "The despatch bench could not be read.");
      if (live.current) setStatus({ kind: "failed", message });
    }
  }, [site]);

  useEffect(() => {
    setStatus({ kind: "loading" });
    void reload();
  }, [reload]);

  // Re-read after every press, landed or refused: a batch that stopped at the
  // third carton still moved two.
  const { busy, problem, dismiss, press } = useWriting(reload);

  return {
    status,
    busy,
    problem,
    booked,
    notice,
    dismiss: () => {
      dismiss();
      setNotice(false);
    },

    consign: (input) =>
      press(`consign:${input.packages.join(",")}`, async (act) => {
        if (input.packages.length === 0) {
          throw new ApiError("A consignment of no cartons is not a consignment.", 400);
        }
        const response = await api.consign({ ...input, act });
        if (live.current) {
          setNotice(true);
          setBooked({
            warnings: response.warnings,
            packages: response.package_count,
            carrier: response.carrier_name,
            service: response.carrier_service_name,
            total_gross_weight_g: response.total_gross_weight_g,
            lines: response.carrier_lines,
          });
        }
      }),

    despatchCarton: (carton) =>
      press(`despatch:${carton.id}`, (act) =>
        api.despatchCarton({ carton: carton.id, lines: carton.lines, act }),
      ),

    /**
     * **One carton at a time, and it stops at the first refusal.** A despatch
     * is a fact about goods leaving a building; pushing on past a rejection
     * would record some of them as gone and leave the operator to work out
     * which — the partial-consignment problem the write path already refuses
     * for the same reason.
     */
    despatchAll: (packages) =>
      // **One press, one act, many cartons.** The names inside carry the
      // carton and the line, so a batch that stopped at the third replays the
      // two that went and attempts the third as the first attempt it is.
      press(`despatch-all:${packages.map((p) => p.id).join(",")}`, async (act) => {
        for (const carton of packages) {
          if (carton.despatched) continue;
          await api.despatchCarton({ carton: carton.id, lines: carton.lines, act });
        }
      }),
  };
}
