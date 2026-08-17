import type { BenchScreen } from "@domain/types";

/**
 * IF400187 mid-pack, as the endpoint returns it.
 *
 * The same job the walk uses: gumboots, the only fixture job whose item is not
 * lot-tracked. Two cartons — one sealed and weighed, one open with a preset
 * that states its footprint but not its height, because a pallet's height is
 * the stack and only the measurement knows it.
 *
 * **The screen renders from this with no network**, which is what makes both
 * densities and both faces reviewable without a running warehouse.
 */
export const PACK_FIXTURE: BenchScreen = {
  reference: "IF400187",
  order_reference: "S260052",
  customer: "Harbourline Provisions Pty Ltd",
  site: "MEL",
  dock_id: "10c00000-0000-0000-0000-000000000003",
  lines: [
    {
      line_id: "f11e0000-0000-0000-0000-000000000001",
      item_code: "GLV-NIT-BLU-M",
      description: "Nitrile glove, blue, medium",
      remaining: 0,
      cells: [
        {
          stock_id: "570c0000-0000-0000-0000-000000000001",
          location: "K.32.01",
          lot: null,
          available: 124,
        },
      ],
    },
    {
      line_id: "f11e0000-0000-0000-0000-000000000002",
      item_code: "HRN-DSP-WHT",
      description: "Hair net, disposable, white",
      remaining: 0,
      cells: [
        {
          stock_id: "570c0000-0000-0000-0000-000000000002",
          location: "I.48.07",
          lot: "L-24118",
          available: 40,
        },
      ],
    },
    {
      line_id: "f11e0000-0000-0000-0000-000000000003",
      item_code: "APR-PE-CLR-L",
      description: "Apron, polythene, clear, large",
      remaining: 1,
      cells: [
        {
          stock_id: "570c0000-0000-0000-0000-000000000003",
          location: "K.36.05",
          lot: null,
          available: 9,
        },
        {
          stock_id: "570c0000-0000-0000-0000-000000000004",
          location: "B.02.11",
          lot: null,
          available: 3,
        },
      ],
    },
  ],
  cartons: [
    {
      id: "ca470000-0000-0000-0000-000000000001",
      sequence: "1",
      package_type: "small box",
      sealed: true,
      gross_weight_g: 4200,
      height_mm: 190,
      stated_size: { length_mm: 400, width_mm: 300, height_mm: 190 },
      // **Weighed, and it agrees.** Eight gloves at 496 g apiece on a 230 g
      // box is 4198; the scale said 4200. The interesting part of the state is
      // that it is settled, so the screen has a case where nothing needs saying.
      expected: {
        grams: 4198,
        n: 41,
        borrowed: false,
        established: true,
        delta_g: 2,
        delta_per_mille: 0,
      },
      contents: [
        {
          item_code: "GLV-NIT-BLU-M",
          description: "Nitrile glove, blue, medium",
          lot_code: null,
          quantity: 8,
          picks: [["3f0e0000-0000-0000-0000-000000000001", 8]],
        },
      ],
    },
    {
      id: "ca470000-0000-0000-0000-000000000002",
      sequence: "2",
      package_type: "PALLET",
      sealed: false,
      gross_weight_g: null,
      height_mm: null,
      // A pallet states its footprint and not its height: the stack is what it
      // is, and only the measurement knows.
      stated_size: { length_mm: 1165, width_mm: 1165, height_mm: 150 },
      // **Nothing on the scale yet, and the figure is borrowed and thin.** Two
      // weighings, and of another size in the same box — which is the state the
      // whole `borrowed` flag exists for, so the fixture has to hold one or the
      // treatment gets built and never seen.
      expected: {
        grams: 25_360,
        n: 2,
        borrowed: true,
        established: false,
        delta_g: null,
        delta_per_mille: null,
      },
      contents: [
        {
          item_code: "HRN-DSP-WHT",
          description: "Hair net, disposable, white",
          lot_code: "L-24118",
          quantity: 3,
          picks: [
            ["3f0e0000-0000-0000-0000-000000000002", 2],
            ["3f0e0000-0000-0000-0000-000000000003", 1],
          ],
        },
      ],
    },
    {
      // **A carton just raised and not yet filled.** The state exists the
      // moment somebody presses Start a carton, and it had no representation
      // in the fixture — so the empty treatment was built and then never
      // rendered on the one surface it was built to be reviewed on. A fixture
      // that omits a real state is a fixture that hides the work.
      id: "ca470000-0000-0000-0000-000000000003",
      sequence: "3",
      package_type: "SKID",
      sealed: false,
      gross_weight_g: null,
      height_mm: null,
      stated_size: { length_mm: 1165, width_mm: 1165, height_mm: 150 },
      // Nothing in it, so there is nothing it should weigh. Null rather than
      // the bare tare: an empty pallet is not the answer to a question about a
      // full one.
      expected: null,
      contents: [],
    },
  ],
  presets: [
    { id: "9a7e0000-0000-0000-0000-0000000000b1", name: "small box" },
    { id: "9a7e0000-0000-0000-0000-0000000000b2", name: "medium box" },
    { id: "9a7e0000-0000-0000-0000-0000000000c1", name: "PALLET" },
    { id: "9a7e0000-0000-0000-0000-0000000000c2", name: "SKID" },
  ],
  wrong_box_reason_id: "4ea50000-0000-0000-0000-000000000001",
};
