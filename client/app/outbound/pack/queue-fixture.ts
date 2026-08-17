import type { PackJob } from "@domain/types";
import type { QueueBench, QueueState } from "./useQueue";

/**
 * The pack queue, with no network.
 *
 * The four groups need four kinds of job to draw, and a real site rarely has
 * all four at once — which is exactly why a fixture is the only place the
 * grouping can be looked at as a whole.
 */
const noop = async () => {};

export function fixtureQueue(state: QueueState, term = ""): QueueBench {
  return { state, term, type: () => {}, search: noop };
}

const job = (over: Partial<PackJob> & Pick<PackJob, "fulfilment_id" | "stage">): PackJob => ({
  reference: "IF265591",
  order_reference: "S260041",
  customer: "Marden Trade Supplies",
  lines: 3,
  committed: 24,
  picked: 0,
  cartons: 0,
  due: "due tomorrow",
  ...over,
});

export const QUEUE: QueueState = {
  kind: "ready",
  jobs: [
    job({ fulfilment_id: "f1", stage: "ready", due: "overdue" }),
    job({ fulfilment_id: "f2", stage: "ready", reference: "IF265592", customer: "Gloveco" }),
    job({
      fulfilment_id: "f3",
      stage: "on_the_bench",
      reference: "IF265593",
      picked: 6,
      cartons: 1,
      customer: "Oates",
    }),
    job({
      fulfilment_id: "f4",
      stage: "packed",
      reference: "IF265594",
      picked: 24,
      cartons: 2,
      due: null,
    }),
    job({
      fulfilment_id: "f5",
      stage: "nothing_committed",
      reference: null,
      lines: 0,
      committed: 0,
      due: null,
    }),
  ],
};

/** Nothing at this site. The system working, not a screen that failed. */
export const CLEAR: QueueState = { kind: "ready", jobs: [] };
