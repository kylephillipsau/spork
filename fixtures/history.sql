-- A year of seeded history, for the questions that cannot be answered by reading.
--
-- Question 122: *"The receiving queries are written and reasoned about, not
-- measured."* Question 142 asks which foreign keys need covering indexes and says
-- the answer should be a considered subset chosen from measurement rather than
-- from taste. Question 76 wants a partitioning plan. **None of the three can be
-- answered by anything in `seed.sql`**, which has two stock movements.
--
-- So this is the other fixture, and the two have opposite jobs:
--
--   seed.sql      small, hand-written, every value stated. Gives each implemented
--                 check a non-empty population. Two runs are byte-identical.
--   history.sql   large, generated. Gives the measurement questions something to
--                 measure and the invariants something to hold at volume.
--
-- It loads **after** `seed.sql` and into its **own tenant**, so the small
-- fixture's numbers stay exactly what they were. A check that examined six things
-- yesterday still examines six of them plus whatever this adds, and the story
-- `seed.sql` tells about each one is undisturbed.
--
-- ---------------------------------------------------------------------------
-- Determinism, which is the whole difficulty
-- ---------------------------------------------------------------------------
--
-- `seed.sql` is deterministic by writing every value down. That does not scale to
-- a quarter of a million rows, and a fixture whose contents differ per run makes
-- every measurement a description of the afternoon — which is the exact criticism
-- that produced `seed.sql` in the first place.
--
-- So: `setseed` once, and every subsequent `random()` is a function of it. Two
-- runs against a fresh database produce identical data, which the load script
-- checks by comparing a checksum rather than by trusting this comment.
--
-- ---------------------------------------------------------------------------
-- Scale, and why these numbers
-- ---------------------------------------------------------------------------
--
-- The proposal's timed baseline is a standard order in one minute forty-five at
-- three hundred orders a day. That is the volume this reproduces, over a year:
--
--   365 days
--   500 items, skewed so a few move constantly and most barely move
--   40 people over the year, with turnover, per question 147
--   200 locations across two zones and a dock
--   8 purchase orders a day, 12 lines each   ->  35,040 promises
--   1 receipt per order, received later      ->  35,040 receipt lines
--   600 picks a day                          -> 219,000 outbound movements
--
-- Roughly 290,000 stock movements, which is what a year at that volume actually
-- looks like. The skew matters more than the total: a uniform distribution over
-- 500 items would make every index look equally good and answer question 142
-- wrongly.

\set ON_ERROR_STOP on

-- ---------------------------------------------------------------------------
-- How long a year is, so the schema can be checked against this file cheaply
-- ---------------------------------------------------------------------------
--
-- The default is the year everything above describes. `scripts/measure.sh` takes
-- it as-is, because measuring a smaller population would answer question 142
-- about a database nobody runs.
--
-- `verify-migrations.sh` overrides it with `-v days=0` and loads a single day.
-- **The point of that pass is not the data but the statements**: this file went
-- three migrations unable to load at all, because D92 added
-- `goods_receipt_line.quantity` with a CHECK pairing it to `entered_quantity` and
-- the generated year still wrote only the entered form. Nothing noticed, because
-- the only script that loads this file was the one nothing runs automatically. A
-- day costs seconds and catches that entire class: every mismatch between these
-- INSERTs and the schema they are written against.
--
-- Determinism is unaffected. `setseed` makes each value a function of the seed,
-- so two runs at the same `days` are identical, which is what measure.sh's
-- checksum compares.
\if :{?days}
\else
  \set days 364
\endif

BEGIN;

SELECT setseed(0.20260804);

-- ---------------------------------------------------------------------------
-- Reference
-- ---------------------------------------------------------------------------

INSERT INTO tenant (id, name, slug) VALUES
    ('33333333-3333-3333-3333-333333333333', 'Gamma Distribution', 'gamma');

-- ---------------------------------------------------------------------------
-- A workforce, because three people is not one
-- ---------------------------------------------------------------------------
--
-- Question 147. D66 measured every index candidate except the actor columns and
-- could not measure those, because this fixture had three people: one of them
-- matched thirty-eight per cent of the ledger, and the number described the
-- fixture rather than the domain. **The instrument had to change before the
-- reading meant anything.**
--
-- Forty over the year, with turnover. Twenty-four from the start; the rest join
-- through the year; one in five of the originals leaves. `person_tenant` carries
-- `joined_at` and `left_at` for exactly this, and until now nothing exercised
-- `left_at` at all. Question 70's retention and deletion work wants the same
-- shape, which is why it is built once here rather than twice later.
--
-- Person is global by D19 -- one person may work for two tenants -- so these are
-- people, and their membership of this tenant is the row that carries the dates.

INSERT INTO person (id, display_name, email)
SELECT ('77770000-0003-4000-8000-' || lpad(n::text, 12, '0'))::uuid,
       'Operator ' || n,
       'op' || n || '@gamma.test'
  FROM generate_series(0, 39) AS n;

INSERT INTO person_tenant (person_id, tenant_id, role, joined_at, left_at)
SELECT ('77770000-0003-4000-8000-' || lpad(n::text, 12, '0'))::uuid,
       '33333333-3333-3333-3333-333333333333',
       CASE WHEN n < 3 THEN 'receiver' ELSE 'operator' END,
       (DATE '2025-08-01' + (CASE WHEN n < 24 THEN 0 ELSE (n - 24) * 20 END))::timestamptz,
       CASE WHEN n < 24 AND n % 5 = 0
            THEN (DATE '2025-08-01' + 200 + n * 3)::timestamptz END
  FROM generate_series(0, 39) AS n;

INSERT INTO site (id, tenant_id, name, code, timezone) VALUES
    ('a5170000-0000-0000-0000-000000000003', '33333333-3333-3333-3333-333333333333',
     'Brisbane', 'BNE', 'Australia/Brisbane');

INSERT INTO zone (id, tenant_id, site_id, code, name) VALUES
    ('20e00000-0000-0000-0000-000000000003', '33333333-3333-3333-3333-333333333333',
     'a5170000-0000-0000-0000-000000000003', 'PICK', 'Pick faces'),
    ('20e00000-0000-0000-0000-000000000004', '33333333-3333-3333-3333-333333333333',
     'a5170000-0000-0000-0000-000000000003', 'BULK', 'Bulk');

-- 200 locations: 120 pick faces, 79 bulk, and one dock that belongs to no zone,
-- which is the shape D46 made zone_id nullable for.
INSERT INTO location (id, tenant_id, site_id, zone_id, code, kind, aisle, bay, level)
SELECT ('10c00000-0003-4000-8000-' || lpad(n::text, 12, '0'))::uuid,
       '33333333-3333-3333-3333-333333333333',
       'a5170000-0000-0000-0000-000000000003',
       CASE WHEN n < 120 THEN '20e00000-0000-0000-0000-000000000003'::uuid
            WHEN n < 199 THEN '20e00000-0000-0000-0000-000000000004'::uuid
            ELSE NULL END,
       CASE WHEN n < 120 THEN 'P-' WHEN n < 199 THEN 'B-' ELSE 'DOCK-' END
         || lpad(n::text, 3, '0'),
       CASE WHEN n < 120 THEN 'pick_face' WHEN n < 199 THEN 'bulk' ELSE 'dock' END,
       chr(65 + (n % 6)), lpad(((n / 6) % 20)::text, 2, '0'), ((n % 4) + 1)::text
  FROM generate_series(0, 199) AS n;

INSERT INTO item_class (id, tenant_id, parent_id, code, name) VALUES
    ('1c1a0000-0000-0000-0003-000000000001', '33333333-3333-3333-3333-333333333333',
     NULL, 'ALL', 'All goods'),
    ('1c1a0000-0000-0000-0003-000000000002', '33333333-3333-3333-3333-333333333333',
     '1c1a0000-0000-0000-0003-000000000001', 'PPE', 'Protective equipment'),
    ('1c1a0000-0000-0000-0003-000000000003', '33333333-3333-3333-3333-333333333333',
     '1c1a0000-0000-0000-0003-000000000001', 'CHEM', 'Cleaning chemicals');

-- 500 items. `tracking` alternates so the lot path is exercised on part of the
-- catalogue rather than all or none of it, which is D33's point about tracking
-- being a property of a product rather than a mode the system runs in.
INSERT INTO item (id, tenant_id, code, description, base_unit_id, tracking,
                  tracking_effective_from)
SELECT ('17e10000-0003-4000-8000-' || lpad(n::text, 12, '0'))::uuid,
       '33333333-3333-3333-3333-333333333333',
       'G-' || lpad(n::text, 4, '0'),
       'Generated item ' || n,
       (SELECT id FROM unit WHERE code = 'ea'),
       CASE WHEN n % 3 = 0 THEN 'lot' ELSE 'none' END,
       CASE WHEN n % 3 = 0 THEN DATE '2025-01-01' ELSE NULL END
  FROM generate_series(0, 499) AS n;

INSERT INTO item_classification (tenant_id, item_id, item_class_id)
SELECT i.tenant_id, i.id,
       CASE WHEN (right(i.code, 4)::int) % 2 = 0
            THEN '1c1a0000-0000-0000-0003-000000000002'::uuid
            ELSE '1c1a0000-0000-0000-0003-000000000003'::uuid END
  FROM item i WHERE i.tenant_id = '33333333-3333-3333-3333-333333333333';

-- One packing config per item, effective before the first movement, so J57 has a
-- population and every receipt can name the version that converted it.
INSERT INTO item_packing_config (id, tenant_id, item_id, units_per_inner,
                                 inners_per_carton, cartons_per_layer,
                                 layers_per_pallet, effective_from)
SELECT ('9ac40000-0003-4000-8000-' || lpad(n::text, 12, '0'))::uuid,
       '33333333-3333-3333-3333-333333333333',
       ('17e10000-0003-4000-8000-' || lpad(n::text, 12, '0'))::uuid,
       6 + (n % 7), 2, 8, 5, DATE '2025-01-01'
  FROM generate_series(0, 499) AS n;

-- 10 suppliers, and the tenant's own party for stock it owns.
INSERT INTO party (id, tenant_id, name, code)
SELECT ('9a247000-0003-4000-8000-' || lpad(n::text, 12, '0'))::uuid,
       '33333333-3333-3333-3333-333333333333',
       CASE WHEN n = 0 THEN 'Gamma Distribution' ELSE 'Supplier ' || n END,
       CASE WHEN n = 0 THEN 'GAMMA' ELSE 'SUP-' || lpad(n::text, 2, '0') END
  FROM generate_series(0, 10) AS n;

INSERT INTO source_channel (id, tenant_id, code, name, authority) VALUES
    ('5c000000-0000-0000-0003-000000000001', '33333333-3333-3333-3333-333333333333',
     'buyer_portal', 'Purchasing portal', 'local');

-- One lot per lot-tracked item per quarter, so lot-tracked receipts have
-- something real to land in and FEFO has a spread of expiries to sort.
INSERT INTO lot (id, tenant_id, item_id, code, expiry_date, production_date,
                 country_of_origin)
SELECT ('10700000-0003-4000-' || lpad(n::text, 4, '0') || '-' || lpad(q::text, 12, '0'))::uuid,
       '33333333-3333-3333-3333-333333333333',
       ('17e10000-0003-4000-8000-' || lpad(n::text, 12, '0'))::uuid,
       'L' || n || '-Q' || q,
       DATE '2025-08-01' + (q * 90) + 540,
       DATE '2025-08-01' + (q * 90) - 30,
       (ARRAY['MY','VN','CN','AU'])[1 + (n % 4)]
  FROM generate_series(0, 499) AS n, generate_series(0, 4) AS q
 WHERE n % 3 = 0;

-- ---------------------------------------------------------------------------
-- The year
-- ---------------------------------------------------------------------------
--
-- Item choice is skewed rather than uniform: `power(random(), 2)` puts most of
-- the mass on low indices, so a few items move constantly and most barely move.
-- That is what makes the measurement worth taking. A uniform draw over 500 items
-- would make every index look equally good and answer question 142 wrongly.

CREATE TEMP TABLE gen_po AS
SELECT ('9000d000-0003-4000-' || lpad(d::text, 4, '0') || '-' || lpad(k::text, 12, '0'))::uuid AS po_id,
       d, k,
       DATE '2025-08-01' + d AS ordered_on
  FROM generate_series(0, :days) AS d, generate_series(0, 7) AS k;

INSERT INTO purchase_order (id, tenant_id, site_id, supplier_party_id, order_number,
                            source_channel_id, currency, state, issued_at, created_at)
SELECT po_id, '33333333-3333-3333-3333-333333333333',
       'a5170000-0000-0000-0000-000000000003',
       ('9a247000-0003-4000-8000-' || lpad((1 + (k % 10))::text, 12, '0'))::uuid,
       'PO-' || to_char(ordered_on, 'YYYYMMDD') || '-' || k,
       '5c000000-0000-0000-0003-000000000001', 'AUD', 'issued',
       ordered_on::timestamptz, ordered_on::timestamptz
  FROM gen_po;

CREATE TEMP TABLE gen_pol AS
SELECT ('901e0000-' || lpad(d::text, 4, '0') || '-4000-' || lpad(k::text, 4, '0') || '-' || lpad(l::text, 12, '0'))::uuid AS pol_id,
       g.po_id, g.d, g.k, l,
       least(499, floor(500 * power(random(), 2))::int) AS item_n,
       (20 + floor(random() * 180))::bigint AS qty,
       g.ordered_on
  FROM gen_po g, generate_series(0, 11) AS l;

INSERT INTO purchase_order_line (id, tenant_id, purchase_order_id, item_id,
                                 quantity_ordered, line_number, unit_price_minor,
                                 price_basis_quantity, expected_from, expected_to,
                                 status_id)
SELECT pol_id, '33333333-3333-3333-3333-333333333333', po_id,
       ('17e10000-0003-4000-8000-' || lpad(item_n::text, 12, '0'))::uuid,
       qty, l + 1,
       100 + (item_n % 400), 100,
       (ordered_on + 4)::timestamptz, (ordered_on + 6)::timestamptz,
       (SELECT id FROM inventory_status WHERE code = 'available')
  FROM gen_pol;

-- The promises. Everything downstream names these rather than the order lines,
-- which is the whole point of D24 unifying the arms.
SELECT projection_run_all('33333333-3333-3333-3333-333333333333');

-- ---------------------------------------------------------------------------
-- Receipts
-- ---------------------------------------------------------------------------

INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
                          submitted_at, received_at)
SELECT '33333333-3333-3333-3333-333333333333',
       ('ce000000-0003-4000-' || lpad(d::text, 4, '0') || '-' || lpad(k::text, 12, '0'))::uuid,
       'a5170000-0000-0000-0000-000000000003',
       ('77770000-0003-4000-8000-' || lpad((k % 3)::text, 12, '0'))::uuid,
       (DATE '2025-08-01' + d + 5)::timestamptz + interval '7 hours',
       (DATE '2025-08-01' + d + 5)::timestamptz + interval '7 hours 1 minute'
  FROM gen_po;

INSERT INTO goods_receipt (id, tenant_id, site_id, purchase_order_id, received_at,
                           recorded_at, client_event_id, recorded_by_id)
SELECT ('92c00000-0003-4000-' || lpad(d::text, 4, '0') || '-' || lpad(k::text, 12, '0'))::uuid,
       '33333333-3333-3333-3333-333333333333',
       'a5170000-0000-0000-0000-000000000003', po_id,
       (ordered_on + 5)::timestamptz + interval '7 hours',
       (ordered_on + 5)::timestamptz + interval '7 hours 2 minutes',
       ('ce000000-0003-4000-' || lpad(d::text, 4, '0') || '-' || lpad(k::text, 12, '0'))::uuid,
       ('77770000-0003-4000-8000-' || lpad((k % 3)::text, 12, '0'))::uuid
  FROM gen_po;

-- Ninety-four per cent of lines arrive in full. The rest arrive short, which is
-- what makes quantity_outstanding a number worth reading and gives the receiving
-- queries a realistic mix rather than a uniform one.
CREATE TEMP TABLE gen_recv AS
SELECT p.pol_id, p.po_id, p.d, p.k, p.l, p.item_n, p.ordered_on,
       e.id AS expected_supply_id,
       CASE WHEN random() < 0.94 THEN p.qty
            ELSE greatest(1, (p.qty * 0.7)::bigint) END AS received_qty,
       p.qty AS expected_qty
  FROM gen_pol p
  JOIN expected_supply e ON e.purchase_order_line_id = p.pol_id;

-- D92: the counted quantity in base units, beside the entered form. `each` at a
-- factor of one, so the two agree here -- which is exactly the case the pair
-- CHECK still has to hold for.
INSERT INTO goods_receipt_line (id, tenant_id, goods_receipt_id, item_id,
                                expected_supply_id, expected_quantity,
                                quantity, entered_quantity, entered_packaging_level,
                                item_packing_config_id, lot_id,
                                accepted_at, accepted_by_id,
                                recorded_at, client_event_id, recorded_by_id)
SELECT ('92c10000-' || lpad(d::text, 4, '0') || '-4000-' || lpad(k::text, 4, '0') || '-' || lpad(l::text, 12, '0'))::uuid,
       '33333333-3333-3333-3333-333333333333',
       ('92c00000-0003-4000-' || lpad(d::text, 4, '0') || '-' || lpad(k::text, 12, '0'))::uuid,
       ('17e10000-0003-4000-8000-' || lpad(item_n::text, 12, '0'))::uuid,
       expected_supply_id, expected_qty,
       received_qty, received_qty, 'each',
       NULL,
       CASE WHEN item_n % 3 = 0
            THEN ('10700000-0003-4000-' || lpad(item_n::text, 4, '0') || '-'
                  || lpad((least(4, (d / 90)))::text, 12, '0'))::uuid
            ELSE NULL END,
       (ordered_on + 5)::timestamptz + interval '7 hours 5 minutes',
       ('77770000-0003-4000-8000-' || lpad((k % 3)::text, 12, '0'))::uuid,
       (ordered_on + 5)::timestamptz + interval '7 hours 5 minutes',
       ('ce000000-0003-4000-' || lpad(d::text, 4, '0') || '-' || lpad(k::text, 12, '0'))::uuid,
       ('77770000-0003-4000-8000-' || lpad((k % 3)::text, 12, '0'))::uuid
  FROM gen_recv;

-- The arrival: onto the dock, naming its cause so J26's fold has a key to group
-- on and the receipt is answerable back to the promise it satisfied.
INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
                            to_location_id, to_lot_id, to_status_id, to_owner_id,
                            goods_receipt_line_id, reason, occurred_at, recorded_at,
                            recorded_by_id, unit_cost_minor, cost_basis_quantity,
                            cost_currency)
SELECT ('5b000000-' || lpad(d::text, 4, '0') || '-4000-' || lpad(k::text, 4, '0') || '-' || lpad(l::text, 12, '0'))::uuid,
       '33333333-3333-3333-3333-333333333333',
       ('ce000000-0003-4000-' || lpad(d::text, 4, '0') || '-' || lpad(k::text, 12, '0'))::uuid,
       ('17e10000-0003-4000-8000-' || lpad(item_n::text, 12, '0'))::uuid,
       received_qty,
       '10c00000-0003-4000-8000-000000000199'::uuid,
       CASE WHEN item_n % 3 = 0
            THEN ('10700000-0003-4000-' || lpad(item_n::text, 4, '0') || '-'
                  || lpad((least(4, (d / 90)))::text, 12, '0'))::uuid
            ELSE NULL END,
       (SELECT id FROM inventory_status WHERE code = 'available'),
       '9a247000-0003-4000-8000-000000000000'::uuid,
       ('92c10000-' || lpad(d::text, 4, '0') || '-4000-' || lpad(k::text, 4, '0') || '-' || lpad(l::text, 12, '0'))::uuid,
       'receipt',
       (ordered_on + 5)::timestamptz + interval '7 hours 10 minutes',
       (ordered_on + 5)::timestamptz + interval '7 hours 11 minutes',
       ('77770000-0003-4000-8000-' || lpad((k % 3)::text, 12, '0'))::uuid,
       90 + (item_n % 300), 100, 'AUD'
  FROM gen_recv;

-- Putaway: dock to a bin, chosen by item so the same product lands consistently,
-- which is what makes a pick face a pick face.
INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
                            from_location_id, from_lot_id, from_status_id, from_owner_id,
                            to_location_id, to_lot_id, to_status_id, to_owner_id,
                            reason, occurred_at, recorded_at, recorded_by_id)
SELECT ('5b010000-' || lpad(d::text, 4, '0') || '-4000-' || lpad(k::text, 4, '0') || '-' || lpad(l::text, 12, '0'))::uuid,
       '33333333-3333-3333-3333-333333333333',
       ('ce000000-0003-4000-' || lpad(d::text, 4, '0') || '-' || lpad(k::text, 12, '0'))::uuid,
       ('17e10000-0003-4000-8000-' || lpad(item_n::text, 12, '0'))::uuid,
       received_qty,
       '10c00000-0003-4000-8000-000000000199'::uuid,
       CASE WHEN item_n % 3 = 0
            THEN ('10700000-0003-4000-' || lpad(item_n::text, 4, '0') || '-'
                  || lpad((least(4, (d / 90)))::text, 12, '0'))::uuid
            ELSE NULL END,
       (SELECT id FROM inventory_status WHERE code = 'available'),
       '9a247000-0003-4000-8000-000000000000'::uuid,
       ('10c00000-0003-4000-8000-' || lpad((item_n % 199)::text, 12, '0'))::uuid,
       CASE WHEN item_n % 3 = 0
            THEN ('10700000-0003-4000-' || lpad(item_n::text, 4, '0') || '-'
                  || lpad((least(4, (d / 90)))::text, 12, '0'))::uuid
            ELSE NULL END,
       (SELECT id FROM inventory_status WHERE code = 'available'),
       '9a247000-0003-4000-8000-000000000000'::uuid,
       'putaway',
       (ordered_on + 5)::timestamptz + interval '9 hours',
       (ordered_on + 5)::timestamptz + interval '9 hours 1 minute',
       ('77770000-0003-4000-8000-' || lpad((k % 3)::text, 12, '0'))::uuid
  FROM gen_recv;

-- ---------------------------------------------------------------------------
-- Picks
-- ---------------------------------------------------------------------------
--
-- Who was on the floor that day, from the membership dates rather than from a
-- constant. A pick is assigned round-robin across the people actually employed
-- on the day it happened, so nobody records a movement before they joined or
-- after they left -- which is the coherence that makes the selectivity real
-- rather than arithmetic.
CREATE TEMP TABLE gen_shift AS
SELECT d, n,
       (row_number() OVER (PARTITION BY d ORDER BY n) - 1)::int AS slot,
       (count(*) OVER (PARTITION BY d))::int AS on_floor
  FROM generate_series(0, :days) AS d
  CROSS JOIN generate_series(3, 39) AS n
 WHERE d >= (CASE WHEN n < 24 THEN 0 ELSE (n - 24) * 20 END)
   AND (n >= 24 OR n % 5 <> 0 OR d < 200 + n * 3);

-- One work session per person per day they worked, and every pick that day
-- carries theirs. That is what a `client_event` is: a submission, not a row.
INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
                          submitted_at, received_at)
SELECT '33333333-3333-3333-3333-333333333333',
       ('ce010000-0003-4000-' || lpad(d::text, 4, '0') || '-' || lpad(n::text, 12, '0'))::uuid,
       'a5170000-0000-0000-0000-000000000003',
       ('77770000-0003-4000-8000-' || lpad(n::text, 12, '0'))::uuid,
       (DATE '2025-08-01' + d)::timestamptz + interval '8 hours',
       (DATE '2025-08-01' + d)::timestamptz + interval '8 hours 1 minute'
  FROM gen_shift;

-- Despatch: out of a bin and out of the system. D24's convention, and the reason
-- in-transit stock needs no virtual location.
INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
                            from_location_id, from_lot_id, from_status_id, from_owner_id,
                            reason, occurred_at, recorded_at, recorded_by_id)
SELECT ('5b020000-0003-4000-' || lpad(picks.d::text, 4, '0') || '-' || lpad(picks.p::text, 12, '0'))::uuid,
       '33333333-3333-3333-3333-333333333333',
       ('ce010000-0003-4000-' || lpad(picks.d::text, 4, '0') || '-' || lpad(sh.n::text, 12, '0'))::uuid,
       ('17e10000-0003-4000-8000-' || lpad(picks.item_n::text, 12, '0'))::uuid,
       picks.pick_qty,
       ('10c00000-0003-4000-8000-' || lpad((picks.item_n % 199)::text, 12, '0'))::uuid,
       CASE WHEN picks.item_n % 3 = 0
            THEN ('10700000-0003-4000-' || lpad(picks.item_n::text, 4, '0') || '-'
                  || lpad((least(4, (picks.d / 90)))::text, 12, '0'))::uuid
            ELSE NULL END,
       (SELECT id FROM inventory_status WHERE code = 'available'),
       '9a247000-0003-4000-8000-000000000000'::uuid,
       'despatch',
       (DATE '2025-08-01' + picks.d)::timestamptz + interval '8 hours' + (picks.p * interval '30 seconds'),
       (DATE '2025-08-01' + picks.d)::timestamptz + interval '8 hours' + (picks.p * interval '31 seconds'),
       ('77770000-0003-4000-8000-' || lpad(sh.n::text, 12, '0'))::uuid
  FROM (
    SELECT d, p,
           least(499, floor(500 * power(random(), 2))::int) AS item_n,
           (1 + floor(random() * 12))::bigint AS pick_qty
      FROM generate_series(0, :days) AS d, generate_series(0, 599) AS p
  ) picks
  JOIN gen_shift sh ON sh.d = picks.d AND sh.slot = picks.p % sh.on_floor;

DROP TABLE gen_shift;
DROP TABLE gen_recv;
DROP TABLE gen_pol;
DROP TABLE gen_po;

COMMIT;

-- The fold, over everything. Outside the transaction so its cost is visible
-- separately from the insert cost, which is the first thing question 122 wants
-- to know.
SELECT projection_run_all('33333333-3333-3333-3333-333333333333');
