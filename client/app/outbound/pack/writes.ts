import type { api } from "@domain/api";
import type { BenchLine, BenchScreen, PlannedParcel, Spot, Uuid } from "@domain/types";

import { keptAct, partOf, type Act } from "../../../domain/acts.ts";
import { byLine, sourcesFor, type BoardAct } from "./board.ts";

/**
 * A press at the pack bench as the writes it makes (D207, for the bench).
 *
 * One press is often several writes: a drop puts goods in from the bench, a
 * bin and a handover; Make makes a pallet, a carton on it, and fills it. They
 * are worked out here against the screen as read, before anything is sent,
 * and kept as plain data, so the device can keep the press and send it again
 * after a reload. Each write names its part of the press's act, so a resend
 * mints nothing new (`partOf`, `keptAct`).
 *
 * No React and no fetching here, so a test runner can read it.
 */

/** A call's input as kept: everything but the act, which is the press's. */
type Input<F extends (input: never) => unknown> = Omit<Parameters<F>[0], "act">;

export type BenchWrite = { part: string } & (
  | { kind: "plan"; fulfilment: Uuid; plan: PlannedParcel[] }
  | ({ kind: "repack" } & Input<typeof api.repack>)
  | ({ kind: "pickInto" } & Input<typeof api.pickInto>)
  | ({ kind: "handOver" } & Input<typeof api.handOver>)
  | ({ kind: "takeOut" } & Input<typeof api.takeOut>)
  | ({ kind: "startCarton" } & Input<typeof api.startCarton>)
  | ({ kind: "startAsIs" } & Input<typeof api.startAsIs>)
  | ({ kind: "checkLine" } & Input<typeof api.checkLine>)
  | ({ kind: "measure" } & Input<typeof api.measure>)
  | { kind: "contain"; carton: Uuid; pallet: Uuid; at: Spot | null }
  | { kind: "place"; carton: Uuid; location: Uuid }
  | { kind: "seal" | "open" | "discard"; carton: Uuid }
);

/** A press kept on the device until the server has every write of it. */
export interface HeldPress {
  /** What the packer did, to say so while it waits. */
  label: string;
  /** The order's number, for a press left waiting on another order's bench. */
  order: string;
  /** When the packer pressed. */
  at: string;
  /** Every id minted for it, by name. */
  ids: Record<string, Uuid>;
  writes: BenchWrite[];
  /** How many have landed: sent again, it carries on from here. */
  done: number;
}

/** Why a press can't be worked out, said before anything is sent. */
export class Unwritable extends Error {}

export const unset = () => new Unwritable("This site doesn't say where it packs yet. Set it in Workspace.");

const under = (prefix: string, part: string) => (prefix ? `${prefix}:${part}` : part);

/** A press's writes as they are worked out, and anything to say once they are sent. */
export interface Writer {
  /** A board act's writes, at `part` of the press. */
  add: (step: BoardAct, part: string) => void;
  /** A write of its own. */
  push: (write: BenchWrite) => void;
  writes: BenchWrite[];
  notes: string[];
}

/**
 * Writes against the screen as read. What one put takes from the bench, a bin
 * or a handover is gone for the next in the same press, so two cartons filled
 * by one press are not both filled from the same units.
 */
export function writer(screen: BenchScreen, fulfilment: Uuid): Writer {
  const writes: BenchWrite[] = [];
  const notes: string[] = [];
  const used = new Map<string, number>();
  const use = (key: string, n: number) => used.set(key, (used.get(key) ?? 0) + n);
  const less = (line: BenchLine): BenchLine => ({
    ...line,
    on_bench: line.on_bench - (used.get(`bench:${line.line_id}`) ?? 0),
    cells: line.cells.map((c) => ({ ...c, available: c.available - (used.get(`cell:${c.stock_id}`) ?? 0) })),
    elsewhere: line.elsewhere && { ...line.elsewhere, handed: line.elsewhere.handed + (used.get(`handover:${line.line_id}`) ?? 0) },
  });
  const bench = () => {
    if (!screen.staging_id) throw unset();
    return screen.staging_id;
  };

  const add = (step: BoardAct, part: string): void => {
    switch (step.kind) {
      case "put": {
        const line = screen.lines.find((l) => l.line_id === step.line);
        if (!line) throw new Unwritable("That line is not on this order any more.");
        const { sources, short } = sourcesFor(less(line), step.units);
        if (sources.length === 0) throw new Unwritable(`No ${line.item_code} is here to put in.`);
        for (const [i, src] of sources.entries()) {
          const at = under(part, `${i}:${src.kind}`);
          if (src.kind === "bench") {
            use(`bench:${line.line_id}`, src.units);
            writes.push({ part: at, kind: "repack", line: line.line_id, from: { location: bench() }, to: { package: step.carton }, quantity: src.units });
          } else if (src.kind === "cell") {
            use(`cell:${src.stock}`, src.units);
            writes.push({ part: at, kind: "pickInto", line: line.line_id, stock: src.stock, carton: step.carton, quantity: src.units });
          } else {
            use(`handover:${line.line_id}`, src.units);
            writes.push({ part: at, kind: "handOver", line: line.line_id, quantity: src.units, carton: step.carton });
          }
        }
        if (short > 0) notes.push(`${step.units - short} of ${step.units} ${line.item_code} went in: the rest is not here.`);
        return;
      }
      case "unput": {
        const picks = (screen.cartons.find((c) => c.id === step.carton)?.contents ?? [])
          .filter((r) => r.line_id === step.line)
          .flatMap((r) => r.picks);
        const reason = screen.wrong_box_reason_id;
        if (!reason) throw new Unwritable("No 'wrong location' reason is configured.");
        writes.push({ part, kind: "takeOut", picks, quantity: step.units, reason });
        return;
      }
      case "repack":
        writes.push({
          part,
          kind: "repack",
          line: step.line,
          from: { package: step.from },
          to: step.to ? { package: step.to } : { location: bench() },
          quantity: step.units,
        });
        return;
      case "make": {
        const pp = screen.plan.find((p) => p.id === step.parcel);
        // Made already, by a press that landed.
        if (!pp) return;
        if (!screen.dock_id) throw unset();
        // On a planned pallet: the pallet first, so there is something to put it on.
        if (pp.on_parcel_id) add({ kind: "make", parcel: pp.on_parcel_id }, under(part, "pallet"));
        const dock = screen.dock_id;
        if (pp.as_is) {
          const { item_id: item, level, config_id: config } = pp.as_is;
          writes.push({ part: under(part, "carton"), kind: "startAsIs", fulfilment, item, level, config, dock, id: pp.id });
        } else if (pp.package_type_id) {
          writes.push({ part: under(part, "carton"), kind: "startCarton", fulfilment, preset: pp.package_type_id, dock, id: pp.id });
        }
        const pallet = pp.on_parcel_id ?? pp.on_package_id;
        if (pallet) writes.push({ part: under(part, "on"), kind: "contain", carton: pp.id, pallet, at: pp.at });
        for (const l of pp.lines) add({ kind: "put", line: l.line_id, units: l.quantity, carton: pp.id }, under(part, `put:${l.line_id}`));
        // A thing as it is is sealed as it is made, as Ship does.
        if (pp.as_is) writes.push({ part: under(part, "seal"), kind: "seal", carton: pp.id });
        return;
      }
      case "contain":
        writes.push({ part, kind: "contain", carton: step.carton, pallet: step.pallet, at: null });
        return;
      case "takeOff":
        writes.push({ part, kind: "place", carton: step.carton, location: bench() });
        return;
      case "unmake": {
        // Gone already, by an undo that landed: nothing to take apart.
        const c = screen.cartons.find((x) => x.id === step.carton);
        if (c) {
          if (c.sealed) writes.push({ part: under(part, "open"), kind: "open", carton: c.id });
          for (const [line, units] of byLine(c.contents.map((r) => ({ line: r.line_id, units: r.quantity }))))
            add({ kind: "unput", line, units, carton: c.id }, under(part, `out:${line}`));
          // Off its pallet first: a pallet with anything on it can't be discarded.
          if (c.on) writes.push({ part: under(part, "off"), kind: "place", carton: c.id, location: bench() });
          writes.push({ part: under(part, "discard"), kind: "discard", carton: c.id });
        }
        if (step.pallet && screen.cartons.some((x) => x.id === step.pallet))
          writes.push({ part: under(part, "pallet"), kind: "discard", carton: step.pallet });
        return;
      }
    }
  };

  return { add, push: (write) => writes.push(write), writes, notes };
}

/** What board acts write, in order: each its own part, `<i>:<kind>`. */
export function writesOf(steps: readonly BoardAct[], screen: BenchScreen, fulfilment: Uuid): { writes: BenchWrite[]; notes: string[] } {
  const w = writer(screen, fulfilment);
  for (const [i, step] of steps.entries()) w.add(step, `${i}:${step.kind}`);
  return { writes: w.writes, notes: w.notes };
}

/**
 * Send a kept press on from the write it got to. Each id is kept as it is
 * minted, and each write as it lands, so a press stopped part way by the wifi
 * or a reload is sent on as the same act. `mint` is the press's own act's,
 * when it is the press sending.
 */
export async function sendOn(
  body: HeldPress,
  send: (write: BenchWrite, act: Act) => Promise<unknown>,
  keep: () => void,
  mint?: (name: string) => Uuid,
): Promise<void> {
  const act = keptAct(body.ids, body.at, keep, mint);
  for (let w = body.writes[body.done]; w; w = body.writes[body.done]) {
    await send(w, w.part ? partOf(act, w.part) : act);
    body.done += 1;
    keep();
  }
}
