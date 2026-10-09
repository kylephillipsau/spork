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
  staging_id: "10c00000-0000-0000-0000-000000000004",
  unready: null,
  lines: [
    {
      line_id: "f11e0000-0000-0000-0000-000000000001",
      item_id: "01990000-0000-7000-8000-00000000a001",
      item_code: "GLV-NIT-BLU-M",
      description: "Nitrile glove, blue, medium",
      tags: { art_no: null, shown: [], warnings: [] },
      remaining: 0,
      committed: 8,
      elsewhere: null,
      own_carton: null,
      picture: null,
      kit: null,
      packs: [],
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
      item_id: "01990000-0000-7000-8000-00000000a002",
      item_code: "HRN-DSP-WHT",
      description: "Hair net, disposable, white",
      tags: { art_no: null, shown: [], warnings: [] },
      remaining: 0,
      committed: 3,
      elsewhere: null,
      own_carton: null,
      picture: null,
      kit: null,
      packs: [],
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
      item_id: "01990000-0000-7000-8000-00000000a003",
      item_code: "APR-PE-CLR-L",
      description: "Apron, polythene, clear, large",
      tags: { art_no: null, shown: [], warnings: [] },
      remaining: 6,
      committed: 6,
      elsewhere: null,
      own_carton: null,
      picture: null,
      // Measured as an each, so the suggestion has something to arrange.
      kit: { item_code: "PPE-KIT-06", description: "Apron and oversleeves kit", ordered: 6 },
      packs: [
        {
          level: "each",
          units: 1,
          size: { length_mm: 280, width_mm: 220, height_mm: 30 },
          ships_as_is: false,
          upright: false,
          no_size: false,
          gross_weight_g: 180,
          source: "own",
          style_code: null,
          faces: {},
          round: false,
          diameter_mm: null,
          base_diameter_mm: null,
          top_height_mm: null,
          wrap: null,
        },
      ],
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
    {
      line_id: "f11e0000-0000-0000-0000-000000000004",
      item_id: "01990000-0000-7000-8000-00000000a004",
      item_code: "SLV-PE-BLU",
      description: "Oversleeve, polythene, blue",
      tags: { art_no: null, shown: [], warnings: [] },
      remaining: 20,
      committed: 30,
      cells: [],
      picture: null,
      // Ten to a carton, and a carton ships as it is unless somebody says not (D196).
      kit: { item_code: "PPE-KIT-06", description: "Apron and oversleeves kit", ordered: 6 },
      packs: [
        {
          level: "carton",
          units: 10,
          ships_as_is: true,
          upright: false,
          size: { length_mm: 420, width_mm: 310, height_mm: 260 },
          no_size: false,
          gross_weight_g: 3600,
          source: "style",
          style_code: "SLV-PE",
          faces: {},
          round: false,
          diameter_mm: null,
          base_diameter_mm: null,
          top_height_mm: null,
          wrap: null,
        },
      ],
      // **Thirty picked, ten already shipped in their own carton.** Ten to a
      // carton, so the other twenty are two more cartons as they came, which is
      // the press the fixture exists to show (migration 98).
      own_carton: {
        item_packing_config_id: "9ac40000-0000-0000-0000-00000000000b",
        units: 10,
        size: { length_mm: 420, width_mm: 310, height_mm: 260 },
        listed_weight_g: 3600,
        method: "transcribed",
        source: "style",
        style_code: "SLV-PE",
      },
      elsewhere: {
        reported: 30,
        handed: 10,
        document: "IF400187",
        picked_by: "Casual Melbourne",
        provenance: "Picked in NetSuite · IF400187 · by Casual Melbourne",
      },
    },
  ],
  cartons: [
    {
      id: "ca470000-0000-0000-0000-000000000001",
      sequence: "1",
      package_type: "small box",
      own_carton_of: null,
      own_level: null,
      listed_weight_g: null,
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
          item_id: "01990000-0000-7000-8000-00000000a001",
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
      own_carton_of: null,
      own_level: null,
      listed_weight_g: null,
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
          item_id: "01990000-0000-7000-8000-00000000a002",
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
      own_carton_of: null,
      own_level: null,
      listed_weight_g: null,
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
    {
      // **The product's own carton**, as it came: no box type, the size its
      // item's carton is recorded at, and a listed weight nobody here weighed.
      id: "ca470000-0000-0000-0000-000000000004",
      sequence: "4",
      package_type: null,
      own_carton_of: "SLV-PE-BLU",
      own_level: "carton",
      listed_weight_g: 3600,
      sealed: true,
      gross_weight_g: null,
      height_mm: null,
      stated_size: { length_mm: 420, width_mm: 310, height_mm: 260 },
      expected: null,
      contents: [
        {
          item_id: "01990000-0000-7000-8000-00000000a004",
          item_code: "SLV-PE-BLU",
          description: "Oversleeve, polythene, blue",
          lot_code: null,
          quantity: 10,
          picks: [["3f0e0000-0000-0000-0000-000000000004", 10]],
        },
      ],
    },
  ],
  presets: [
    { id: "9a7e0000-0000-0000-0000-0000000000b1", name: "small box", size: { length_mm: 400, width_mm: 300, height_mm: 190 }, suggested: true, max_payload_g: null, tare_weight_g: null },
    { id: "9a7e0000-0000-0000-0000-0000000000b2", name: "medium box", size: { length_mm: 450, width_mm: 340, height_mm: 410 }, suggested: true, max_payload_g: null, tare_weight_g: null },
    { id: "9a7e0000-0000-0000-0000-0000000000c1", name: "PALLET", size: null, suggested: true, max_payload_g: null, tare_weight_g: null },
    { id: "9a7e0000-0000-0000-0000-0000000000c2", name: "SKID", size: null, suggested: true, max_payload_g: null, tare_weight_g: null },
  ],
  wrong_box_reason_id: "4ea50000-0000-0000-0000-000000000001",
};

/**
 * The same job with a small box open and two aprons in it (D198): the
 * suggestion fills this carton first, the two shown as in, the plan on the
 * next layer with something to put in.
 */
export const PACK_FILLING: BenchScreen = {
  ...PACK_FIXTURE,
  lines: PACK_FIXTURE.lines.map((l) => (l.item_code === "APR-PE-CLR-L" ? { ...l, remaining: 4 } : l)),
  cartons: [
    PACK_FIXTURE.cartons[0]!,
    {
      id: "ca470000-0000-0000-0000-000000000005",
      sequence: "2",
      package_type: "small box",
      own_carton_of: null,
      own_level: null,
      listed_weight_g: null,
      sealed: false,
      gross_weight_g: null,
      height_mm: null,
      stated_size: { length_mm: 400, width_mm: 300, height_mm: 190 },
      expected: null,
      contents: [
        {
          item_id: "01990000-0000-7000-8000-00000000a003",
          item_code: "APR-PE-CLR-L",
          description: "Apron, polythene, clear, large",
          lot_code: null,
          quantity: 2,
          picks: [["3f0e0000-0000-0000-0000-000000000005", 2]],
        },
      ],
    },
  ],
};

/**
 * An order that is mostly cartons as they came, with loose things nobody has
 * measured yet (D224): the cartons shown as themselves, and what the loose
 * things weigh, so they can be boxed by hand and the box weighed against it.
 * One of them was never weighed either.
 */
export const PACK_LOOSE: BenchScreen = {
  ...PACK_FIXTURE,
  cartons: [],
  lines: PACK_FIXTURE.lines.map((l) => {
    const unmeasured = (grams: number | null) => ({
      ...l,
      remaining: l.committed,
      kit: null,
      packs: [{ ...l.packs[0]!, level: "each" as const, units: 1, size: null, ships_as_is: false, gross_weight_g: grams }],
    });
    if (l.item_code === "GLV-NIT-BLU-M") return unmeasured(520);
    if (l.item_code === "HRN-DSP-WHT") return unmeasured(null);
    if (l.item_code === "APR-PE-CLR-L") return unmeasured(180);
    return { ...l, remaining: l.committed, kit: null, own_carton: null };
  }),
};
