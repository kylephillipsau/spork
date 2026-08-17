import type { DespatchScreen } from "@domain/types";

/**
 * A dock mid-afternoon.
 *
 * **Every state the screen draws is reachable from here**, which D131 makes a
 * gate rather than a courtesy: a job waiting with a carton nobody weighed, a
 * consignment booked and half gone, a consignment fully gone, and something
 * already away today. A fixture that omits a state hides the work built for it,
 * and the render check fails on exactly that.
 */
export const DESPATCH_FIXTURE: DespatchScreen = {
  site: "MEL",
  waiting: [
    {
      fulfilment_id: "f01f0000-0000-0000-0000-000000000004",
      reference: "IF400187",
      order_reference: "S260052",
      customer: "Harbourline Provisions Pty Ltd",
      // Null on purpose: CTN 2 was never weighed, so the job's total is
      // unknown rather than light.
      gross_weight_g: null,
      cartons: [
        {
          id: "ca470000-0000-0000-0000-000000000001",
          sequence: "1",
          package_type: "small box",
          gross_weight_g: 4200,
          weighed: true,
        },
        {
          id: "ca470000-0000-0000-0000-000000000002",
          sequence: "2",
          package_type: "PALLET",
          gross_weight_g: null,
          weighed: false,
        },
      ],
    },
    {
      fulfilment_id: "f01f0000-0000-0000-0000-000000000007",
      reference: "IF265601",
      order_reference: "S260058",
      customer: "Kelmore Foods",
      gross_weight_g: 18600,
      cartons: [
        {
          id: "ca470000-0000-0000-0000-000000000011",
          sequence: "1",
          package_type: "medium box",
          gross_weight_g: 18600,
          weighed: true,
        },
      ],
    },
  ],
  booked: [
    {
      consignment_id: "c0c00000-0000-0000-0000-000000000001",
      carrier_name: "Swift Transport Services",
      carrier_service_name: "Swift next business day",
      status: null,
      despatch_at: "2026-08-16T07:00:00Z",
      package_count: 2,
      awaiting_despatch: 1,
      gross_weight_g: 31400,
      packages: [
        {
          id: "ca470000-0000-0000-0000-000000000021",
          sequence: "1",
          reference: "IF265604",
          despatched: true,
          lines: [],
        },
        {
          id: "ca470000-0000-0000-0000-000000000022",
          sequence: "2",
          reference: "IF265604",
          despatched: false,
          lines: [
            {
              fulfilment_line_id: "f11e0000-0000-0000-0000-000000000021",
              quantity: 6,
            },
          ],
        },
      ],
    },
  ],
  gone_today: [
    {
      consignment_id: "c0c00000-0000-0000-0000-000000000002",
      carrier_name: "Direct Transport",
      package_count: 4,
      last_despatched_at: "2026-08-15T02:40:00Z",
    },
  ],
  carriers: [
    {
      id: "ca440000-0000-0000-0000-000000000001",
      name: "Direct Transport",
      code: "DIRECT",
      services: [
        {
          id: "ca450000-0000-0000-0000-000000000002",
          name: "Direct road freight",
          code: "DIRECT-ROAD",
        },
      ],
    },
    {
      id: "ca440000-0000-0000-0000-000000000002",
      name: "Swift Transport Services",
      code: "SWIFT",
      services: [
        {
          id: "ca450000-0000-0000-0000-000000000001",
          name: "Swift next business day",
          code: "SWIFT-NBD",
        },
      ],
    },
  ],
  providers: [
    {
      id: "f9040000-0000-0000-0000-000000000001",
      name: "MachShip",
      kind: "machship",
    },
  ],
};

/**
 * What a booking just produced, for the state the screen could not previously
 * draw.
 *
 * Two lines on purpose. The first is the ordinary case: four identical cartons,
 * so the line states a size. The second is the honest one: three cartons of one
 * type that disagree about their dimensions, so `uniform` is false and the line
 * carries the count without inventing a size to cover them.
 */
export const BOOKED = {
  warnings: [
    "3 cartons of large box do not all carry the same dimensions; a carrier given one line for them would be quoted the smallest",
  ],
  packages: 7,
  carrier: "Swift Transport Services",
  service: "Swift next business day",
  total_gross_weight_g: 41800,
  lines: [
    {
      package_type: "small box",
      carrier_package_code: "CTN",
      package_count: 4,
      gross_weight_g: 16800,
      length_mm: 320,
      width_mm: 240,
      height_mm: 180,
      uniform: true,
    },
    {
      package_type: "large box",
      carrier_package_code: "CTN3",
      package_count: 3,
      gross_weight_g: 25000,
      length_mm: 480,
      width_mm: 380,
      height_mm: 300,
      uniform: false,
    },
  ],
};
