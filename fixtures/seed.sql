-- A deterministic fixture.
--
-- The job-asserted suite reported three passing checks against an empty database
-- and eleven against a seeded one, which makes its numbers a description of
-- whatever happened to be lying around rather than of the schema. Vacuity is
-- meant to report a check with nothing to examine; it is not meant to report
-- that nobody remembered to insert anything.
--
-- So: fixed UUIDs, fixed timestamps, no now() and no random(). Two runs of this
-- file against a fresh database produce byte-identical data, which is what makes
-- "eleven passed, one vacuous" a fact about the code rather than about the
-- afternoon.
--
-- It is deliberately small. The point is to give every implemented check a
-- non-empty population, not to simulate a warehouse.

-- One transaction. A fixture that half-applies is worse than one that fails,
-- because the failure is loud and the half is silent: the next run hits a
-- duplicate key on row two and nobody looks at rows three onward.
BEGIN;

-- ---------------------------------------------------------------------------
-- Tenancy, people, places
-- ---------------------------------------------------------------------------

INSERT INTO tenant (id, name, slug) VALUES
    ('11111111-1111-1111-1111-111111111111', 'Alpha Foods', 'alpha'),
    -- A second tenant exists only so isolation has something to isolate from.
    -- A single-tenant fixture cannot fail a cross-tenant check.
    ('22222222-2222-2222-2222-222222222222', 'Beta Supply', 'beta');

INSERT INTO person (id, display_name, email) VALUES
    ('77770000-0000-0000-0000-000000000001', 'Kyle Phillips', 'kyle@example.test'),
    ('77770000-0000-0000-0000-000000000002', 'Dana Okafor',   'dana@example.test');

INSERT INTO person_tenant (person_id, tenant_id, role, joined_at) VALUES
    ('77770000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'operator', '2026-01-01T00:00:00Z'),
    ('77770000-0000-0000-0000-000000000002', '22222222-2222-2222-2222-222222222222',
     'operator', '2026-01-01T00:00:00Z');

INSERT INTO site (id, tenant_id, name, code, timezone) VALUES
    ('a5170000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'Melbourne', 'MEL', 'Australia/Melbourne'),
    ('a5170000-0000-0000-0000-000000000002', '22222222-2222-2222-2222-222222222222',
     'Sydney', 'SYD', 'Australia/Sydney');

INSERT INTO zone (id, tenant_id, site_id, code, name) VALUES
    ('20e00000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'a5170000-0000-0000-0000-000000000001', 'CHILL', 'Chilled');

INSERT INTO location (id, tenant_id, site_id, zone_id, code, kind) VALUES
    ('10c00000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'a5170000-0000-0000-0000-000000000001', '20e00000-0000-0000-0000-000000000001',
     'A-01-1', 'pick_face'),
    ('10c00000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     'a5170000-0000-0000-0000-000000000001', NULL, 'B-01-1', 'bulk'),
    -- A dock, which belongs to no zone. D46 made zone_id nullable for exactly
    -- this, so the fixture carries one rather than leaving that path untested.
    ('10c00000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     'a5170000-0000-0000-0000-000000000001', NULL, 'DOCK-1', 'dock'),
    -- **The packing station, and it is what a trolley pick lands at** (D166).
    -- `staging` has been an allowed `location.kind` since migration 1 and the
    -- fixture had none, so the arm that separates *picked* from *put down on
    -- the way* had nothing to be tested against. No zone and no pick sequence:
    -- it is not on the walk, it is where the walk ends.
    ('10c00000-0000-0000-0000-000000000004', '11111111-1111-1111-1111-111111111111',
     'a5170000-0000-0000-0000-000000000001', NULL, 'PACK-1', 'staging');

-- **The order a picker walks, so J71 has a population.** Migration 74 added the
-- column and the fixture had none, which made J71 vacuous: a rule of that shape
-- passes when there is nothing to check, and this register says so about itself.
-- Distinct positions, because the disagreement J71 looks for is built and torn
-- down by its own test rather than left resident.
UPDATE location SET pick_sequence = 1 WHERE code = 'A-01-1';
UPDATE location SET pick_sequence = 2 WHERE code = 'B-01-1';
UPDATE location SET pick_sequence = 3 WHERE code = 'DOCK-1';

-- ---------------------------------------------------------------------------
-- Catalogue
-- ---------------------------------------------------------------------------

-- D63. A taxonomy with actual depth, so the closure is exercised beyond the
-- self-rows. Before this the fixture wrote its one closure row by hand, which is
-- a thing only a superuser can do and therefore proved nothing about deployment.
INSERT INTO item_class (id, tenant_id, parent_id, code, name) VALUES
    ('1c1a0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     NULL, 'PPE', 'Protective equipment'),
    ('1c1a0000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     '1c1a0000-0000-0000-0000-000000000001', 'GLOVES', 'Gloves');

-- A counterparty taxonomy too, so the party closure has something to fold.
INSERT INTO party_class (id, tenant_id, parent_id, code, name) VALUES
    ('9c1a0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     NULL, 'SUPPLIER', 'Suppliers'),
    ('9c1a0000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     NULL, 'GLOVE_SUPPLIER', 'Glove suppliers');

-- D76. Where each class started, which is the base the fold had been missing. A
-- class whose creation is unrecorded cannot be placed at any instant before its
-- first move -- measured on this fixture, where GLOVE_SUPPLIER's parent at 00:11
-- was unrecoverable until these rows existed.
--
-- NULL is a destination meaning the root, exactly as in a reparented row, which
-- is why GLOVE_SUPPLIER's creation states NULL rather than omitting the column.
INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
    submitted_at, received_at) VALUES
    ('11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000009',
     'a5170000-0000-0000-0000-000000000001', '77770000-0000-0000-0000-000000000001',
     '2026-08-04T00:00:00Z', '2026-08-04T00:00:01Z');

INSERT INTO policy_change (id, tenant_id, occurred_at, recorded_at, item_class_id,
    party_class_id, new_item_parent_id, new_party_parent_id, tenant_scope_id,
    client_event_id, change_kind, reason, recorded_by_id) VALUES
    ('c0000000-0000-0000-0000-000000000010', '11111111-1111-1111-1111-111111111111',
     '2026-08-04T00:00:00Z', '2026-08-04T00:00:01Z',
     '1c1a0000-0000-0000-0000-000000000001', NULL, NULL, NULL,
     '11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000009',
     'created', 'Top of the product taxonomy', '77770000-0000-0000-0000-000000000001'),
    ('c0000000-0000-0000-0000-000000000011', '11111111-1111-1111-1111-111111111111',
     '2026-08-04T00:00:00Z', '2026-08-04T00:00:01Z',
     '1c1a0000-0000-0000-0000-000000000002', NULL,
     '1c1a0000-0000-0000-0000-000000000001', NULL,
     '11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000009',
     'created', 'Gloves, under protective equipment', '77770000-0000-0000-0000-000000000001'),
    ('c0000000-0000-0000-0000-000000000012', '11111111-1111-1111-1111-111111111111',
     '2026-08-04T00:00:00Z', '2026-08-04T00:00:01Z',
     NULL, '9c1a0000-0000-0000-0000-000000000001', NULL, NULL,
     '11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000009',
     'created', 'Top of the counterparty taxonomy', '77770000-0000-0000-0000-000000000001'),
    ('c0000000-0000-0000-0000-000000000013', '11111111-1111-1111-1111-111111111111',
     '2026-08-04T00:00:00Z', '2026-08-04T00:00:01Z',
     NULL, '9c1a0000-0000-0000-0000-000000000002', NULL, NULL,
     '11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000009',
     'created', 'Glove suppliers, filed at the root', '77770000-0000-0000-0000-000000000001');

-- D72. Glove suppliers were created at the root and belong under Suppliers. The
-- move is a fact with a reason and an actor, folded onto parent_id by
-- projection_taxonomy_rebuild -- so the fixture never writes parent_id for this
-- class after the insert, and could not: the application has no UPDATE on it.
--
-- Before D72 this was an UPDATE with no author, no moment and no reason, which
-- changed which shelf-life policy wins for every supplier underneath.
-- Its own idempotency key rather than a borrowed one: a re-parent is an act, and
-- D40 gives every act a key so replaying the message cannot move the class twice.
INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
    submitted_at, received_at) VALUES
    ('11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000007',
     'a5170000-0000-0000-0000-000000000001', '77770000-0000-0000-0000-000000000001',
     '2026-08-04T00:12:00Z', '2026-08-04T00:12:01Z');

-- D73. The blast radius as it was measured before the move: one active binding
-- changes, the shelf-life policy scoped to Suppliers below. It is frozen rather
-- than derived because after the move it cannot be recomputed -- the shape it was
-- measured against is the shape this row replaced. J60 is what notices a move
-- that recorded nothing.
INSERT INTO policy_change (id, tenant_id, occurred_at, recorded_at, party_class_id,
    new_party_parent_id, affected_binding_count, tenant_scope_id, client_event_id,
    change_kind, reason, recorded_by_id) VALUES
    ('c0000000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     '2026-08-04T00:12:00Z', '2026-08-04T00:12:01Z',
     '9c1a0000-0000-0000-0000-000000000002',
     '9c1a0000-0000-0000-0000-000000000001', 1,
     '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000007', 'reparented',
     'Glove suppliers were filed at the root by mistake',
     '77770000-0000-0000-0000-000000000001');

-- D74. A class created by mistake, and the only exit it has. GLOVES_LEGACY was
-- made before GLOVES and duplicated it; nothing was ever classified into it. It
-- is retired rather than deleted, because the application has no DELETE here and
-- because a taxonomy node is a fact about what somebody once meant.
--
-- Retirement changes no resolution, which is why this class keeps its closure
-- rows and J61 examines it without complaint: no live child hangs under it and
-- no binding was scoped to it afterwards.
INSERT INTO item_class (id, tenant_id, parent_id, code, name) VALUES
    ('1c1a0000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     '1c1a0000-0000-0000-0000-000000000001', 'GLOVES_LEGACY', 'Gloves (superseded)');

INSERT INTO policy_change (id, tenant_id, occurred_at, recorded_at, item_class_id,
    new_item_parent_id, tenant_scope_id, client_event_id, change_kind, reason,
    recorded_by_id) VALUES
    ('c0000000-0000-0000-0000-000000000014', '11111111-1111-1111-1111-111111111111',
     '2026-08-04T00:13:00Z', '2026-08-04T00:13:01Z',
     '1c1a0000-0000-0000-0000-000000000003',
     '1c1a0000-0000-0000-0000-000000000001',
     '11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000009',
     'created', 'Created in error, and retired a minute later',
     '77770000-0000-0000-0000-000000000001');

INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
    submitted_at, received_at) VALUES
    ('11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000008',
     'a5170000-0000-0000-0000-000000000001', '77770000-0000-0000-0000-000000000001',
     '2026-08-04T00:14:00Z', '2026-08-04T00:14:01Z');

INSERT INTO policy_change (id, tenant_id, occurred_at, recorded_at, item_class_id,
    tenant_scope_id, client_event_id, change_kind, reason, recorded_by_id) VALUES
    ('c0000000-0000-0000-0000-000000000005', '11111111-1111-1111-1111-111111111111',
     '2026-08-04T00:14:00Z', '2026-08-04T00:14:01Z',
     '1c1a0000-0000-0000-0000-000000000003',
     '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000008', 'retired',
     'Duplicate of GLOVES, created in error and never used',
     '77770000-0000-0000-0000-000000000001');

SELECT projection_run_all('11111111-1111-1111-1111-111111111111');

INSERT INTO item (id, tenant_id, code, description, base_unit_id, tracking, tracking_effective_from)
SELECT '17e10000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
       'GLOVE-M', 'Nitrile glove, medium', u.id, 'lot', '2026-01-01'
  FROM unit u WHERE u.code = 'ea';

-- Classified into the child. J15 then has to walk PPE -> GLOVES through the
-- closure to see that binding 3's item is under the class beside it, rather than
-- matching on a self-row and proving nothing about the walk.
INSERT INTO item_classification (tenant_id, item_id, item_class_id) VALUES
    ('11111111-1111-1111-1111-111111111111',
     '17e10000-0000-0000-0000-000000000001', '1c1a0000-0000-0000-0000-000000000002');

-- D23 versions the packing config by effective_from so that correcting a case
-- pack cannot rewrite history. D58 makes that versioning load-bearing by having
-- movements name the version that converted them. This one predates every
-- movement in the fixture, which J57 checks.
INSERT INTO item_packing_config (id, tenant_id, item_id, units_per_inner,
    inners_per_carton, cartons_per_layer, layers_per_pallet, effective_from) VALUES
    ('9ac40000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     '17e10000-0000-0000-0000-000000000001', 10, 1, 8, 5, '2026-01-01');

INSERT INTO inventory_status (id, code, name, is_available_for_allocation) VALUES
    ('57a70000-0000-0000-0000-000000000001', 'available', 'Available', true),
    ('57a70000-0000-0000-0000-000000000002', 'quarantine', 'Quarantine', false);

INSERT INTO party (id, tenant_id, name, code, party_class_id) VALUES
    ('9a247000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'Alpha Foods', 'ALPHA', NULL),
    ('9a247000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     'Gloveco', 'GLOVECO', '9c1a0000-0000-0000-0000-000000000002');

-- D58. Where it came from and when it was made, neither of which anybody can
-- reconstruct after the pallet has gone.
INSERT INTO lot (id, tenant_id, item_id, code, expiry_date,
    country_of_origin, production_date) VALUES
    ('10700000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     '17e10000-0000-0000-0000-000000000001', 'L2026-014', '2027-06-30',
     'MY', '2026-05-12');

-- ---------------------------------------------------------------------------
-- The ledger
-- ---------------------------------------------------------------------------

INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id, submitted_at, received_at) VALUES
    ('11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000001',
     'a5170000-0000-0000-0000-000000000001', '77770000-0000-0000-0000-000000000001',
     '2026-08-04T00:00:00Z', '2026-08-04T00:00:01Z'),
    ('11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000002',
     'a5170000-0000-0000-0000-000000000001', '77770000-0000-0000-0000-000000000001',
     '2026-08-04T00:05:00Z', '2026-08-04T00:05:01Z'),
    ('11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000003',
     'a5170000-0000-0000-0000-000000000001', '77770000-0000-0000-0000-000000000001',
     '2026-08-04T00:10:00Z', '2026-08-04T00:10:01Z'),
    ('11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000004',
     'a5170000-0000-0000-0000-000000000001', '77770000-0000-0000-0000-000000000001',
     '2026-08-04T00:30:00Z', '2026-08-04T00:30:01Z'),
    ('11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000005',
     'a5170000-0000-0000-0000-000000000001', '77770000-0000-0000-0000-000000000001',
     '2026-08-04T00:40:00Z', '2026-08-04T00:40:01Z'),
    ('11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000006',
     'a5170000-0000-0000-0000-000000000001', '77770000-0000-0000-0000-000000000001',
     '2026-08-04T00:50:00Z', '2026-08-04T00:50:01Z');

-- Receive 100, then move 40, then discover that only 97 ever arrived (a correction
-- of four that was itself corrected by one). The fold must give 57 and 40, which
-- J1 checks.
-- D58. The receipt carries what it cost and what was actually keyed. Ten cartons
-- were scanned, the config says ten per carton, so the ledger holds 100 and the
-- evidence for that number is on the row rather than in somebody's memory.
-- Correcting the carton size later cannot rewrite what this receipt meant,
-- because it names the version that converted it.
INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
    to_location_id, to_lot_id, to_status_id, to_owner_id,
    reason, occurred_at, recorded_at, recorded_by_id,
    unit_cost_minor, cost_basis_quantity, cost_currency,
    entered_quantity, entered_packaging_level, item_packing_config_id) VALUES
    ('5b000000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000001', '17e10000-0000-0000-0000-000000000001', 100,
     '10c00000-0000-0000-0000-000000000001', '10700000-0000-0000-0000-000000000001',
     '57a70000-0000-0000-0000-000000000001', '9a247000-0000-0000-0000-000000000001',
     'receipt', '2026-08-04T00:00:00Z', '2026-08-04T00:00:01Z',
     '77770000-0000-0000-0000-000000000001',
     180, 100, 'AUD',
     10, 'carton', '9ac40000-0000-0000-0000-000000000001');

INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
    from_location_id, from_lot_id, from_status_id, from_owner_id,
    to_location_id, to_lot_id, to_status_id, to_owner_id,
    reason, occurred_at, recorded_at, recorded_by_id) VALUES
    ('5b000000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000002', '17e10000-0000-0000-0000-000000000001', 40,
     '10c00000-0000-0000-0000-000000000001', '10700000-0000-0000-0000-000000000001',
     '57a70000-0000-0000-0000-000000000001', '9a247000-0000-0000-0000-000000000001',
     '10c00000-0000-0000-0000-000000000002', '10700000-0000-0000-0000-000000000001',
     '57a70000-0000-0000-0000-000000000001', '9a247000-0000-0000-0000-000000000001',
     'move', '2026-08-04T00:05:00Z', '2026-08-04T00:05:01Z',
     '77770000-0000-0000-0000-000000000001');

-- The first correction. Four of the hundred were never there, discovered half an
-- hour later. It carries the receipt's occurred_at, not its own discovery time, so
-- "what was in bin 1 at 00:02" answers the net while "what did we believe at 00:02"
-- still answers 100. Both are true and the fixture proves each is reachable.
--
-- It also runs the other way to the movement it corrects: out of the bin the
-- receipt put them into. J50 checks the mirroring, J51 that it does not exceed
-- its target, J52 that the chain cannot loop. Without this row all three pass
-- while examining nothing, which is what D38 was written to stop.
INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
    from_location_id, from_lot_id, from_status_id, from_owner_id,
    reason, occurred_at, recorded_at, recorded_by_id,
    reverses_movement_id, adjustment_reason_id) VALUES
    ('5b000000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000004', '17e10000-0000-0000-0000-000000000001', 4,
     '10c00000-0000-0000-0000-000000000001', '10700000-0000-0000-0000-000000000001',
     '57a70000-0000-0000-0000-000000000001', '9a247000-0000-0000-0000-000000000001',
     'adjustment', '2026-08-04T00:00:00Z', '2026-08-04T00:30:00Z',
     '77770000-0000-0000-0000-000000000001',
     '5b000000-0000-0000-0000-000000000001',
     (SELECT id FROM adjustment_reason WHERE code = 'miscount' AND tenant_id IS NULL));

-- A correction *to* the correction: one of the four was counted wrong a second
-- time, so one unit is put back. Depth two, resident rather than only in a
-- rolled-back test — which is what left D103's chain arithmetic unexamined by the
-- fixture until now. quantity_received is 97 = 100 − (4 − 1). D102's one-level
-- netting would have left it at 96.
INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
    to_location_id, to_lot_id, to_status_id, to_owner_id,
    reason, occurred_at, recorded_at, recorded_by_id,
    reverses_movement_id, adjustment_reason_id) VALUES
    ('5b000000-0000-0000-0000-0000000000c2', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000004', '17e10000-0000-0000-0000-000000000001', 1,
     '10c00000-0000-0000-0000-000000000001', '10700000-0000-0000-0000-000000000001',
     '57a70000-0000-0000-0000-000000000001', '9a247000-0000-0000-0000-000000000001',
     'adjustment', '2026-08-04T00:00:00Z', '2026-08-04T00:45:00Z',
     '77770000-0000-0000-0000-000000000001',
     '5b000000-0000-0000-0000-000000000003',
     (SELECT id FROM adjustment_reason WHERE code = 'miscount' AND tenant_id IS NULL));

-- ---------------------------------------------------------------------------
-- Containment, including the out-of-order event
-- ---------------------------------------------------------------------------

INSERT INTO package (id, tenant_id, barcode) VALUES
    ('9ac00000-0000-0000-0000-00000000000a', '11111111-1111-1111-1111-111111111111', 'PALLET-A'),
    ('9ac00000-0000-0000-0000-00000000000b', '11111111-1111-1111-1111-111111111111', 'CARTON-B');

-- The carton is placed at B-01-1 at 09:00 and contained in the pallet at 10:05.
-- The placement event is inserted LAST but carries the EARLIER device clock, so
-- the register has to prefer the containment. J6 checks that it did, and the
-- replay property test shuffles arrival order to check it stays that way.
INSERT INTO package_event (id, tenant_id, client_event_id, package_id, kind,
    location_id, source, occurred_at, recorded_at, recorded_by_id) VALUES
    ('9e000000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000003', '9ac00000-0000-0000-0000-00000000000a',
     'placed', '10c00000-0000-0000-0000-000000000001', 'operator_scan',
     '2026-08-04T00:00:00Z', '2026-08-04T00:00:01Z', '77770000-0000-0000-0000-000000000001');

INSERT INTO package_event (id, tenant_id, client_event_id, package_id, kind,
    parent_package_id, source, occurred_at, recorded_at, recorded_by_id) VALUES
    ('9e000000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000003', '9ac00000-0000-0000-0000-00000000000b',
     'contained', '9ac00000-0000-0000-0000-00000000000a', 'operator_scan',
     '2026-08-04T00:05:00Z', '2026-08-04T00:05:01Z', '77770000-0000-0000-0000-000000000001');

INSERT INTO package_event (id, tenant_id, client_event_id, package_id, kind,
    location_id, source, occurred_at, recorded_at, recorded_by_id) VALUES
    ('9e000000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000003', '9ac00000-0000-0000-0000-00000000000b',
     'placed', '10c00000-0000-0000-0000-000000000002', 'operator_scan',
     '2026-08-03T23:00:00Z', '2026-08-04T00:09:00Z', '77770000-0000-0000-0000-000000000001');

-- ---------------------------------------------------------------------------
-- Measurement
-- ---------------------------------------------------------------------------

INSERT INTO observable (id, tenant_id, item_id, packaging_level) VALUES
    ('0b500000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     '17e10000-0000-0000-0000-000000000001', 'each');

INSERT INTO observation_event (id, tenant_id, client_event_id, observable_id,
    observed_at, recorded_at, recorded_by_id, method, ingestion_channel) VALUES
    ('0e000000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000003', '0b500000-0000-0000-0000-000000000001',
     '2026-08-04T00:10:00Z', '2026-08-04T00:10:01Z',
     '77770000-0000-0000-0000-000000000001', 'instrument', 'scale');

-- Entered as 0.5 kg, stored as 500 g. The canonical value is what we compute
-- with and the entered value is what we can quote back.
INSERT INTO observation (id, tenant_id, observation_event_id, client_event_id,
    observable_id, observed_at, metric_id, result_kind, dimension_id,
    value_numeric, entered_value, entered_unit_id)
SELECT '0b000000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
       '0e000000-0000-0000-0000-000000000001', 'ce000000-0000-0000-0000-000000000003',
       '0b500000-0000-0000-0000-000000000001', '2026-08-04T00:10:00Z',
       m.id, 'quantity', m.dimension_id, 500, 0.5, u.id
  FROM metric m, unit u
 WHERE m.code = 'net_weight' AND u.code = 'kg';

-- ---------------------------------------------------------------------------
-- Policy: a platform default and a tenant binding that must beat it
-- ---------------------------------------------------------------------------

-- Three specificity levels, so the resolver's ordering has something to order.
--
-- The platform default carries no scope at all. It was written item-specific and
-- J14 caught it the first time that check ran: policy_binding is under the
-- shared-reference RLS shape, so a platform row is readable by every tenant, and
-- scoping one to a single tenant's item makes a shipped default that is not a
-- default. The all-NULL scope is the shape S12 exists to keep legal, and it is
-- what a platform default actually is.
INSERT INTO policy_binding (id, tenant_id, kind, note) VALUES
    ('b0000000-0000-0000-0000-000000000001', NULL, 'shelf_life',
     'platform default, any scope');
INSERT INTO policy_binding (id, tenant_id, kind, item_class_id, note) VALUES
    ('b0000000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     'shelf_life', '1c1a0000-0000-0000-0000-000000000001', 'tenant binding, class-wide');
-- Both axes of the Product dimension, and they agree: the item is classified
-- into the class named beside it. J15 has nothing to examine without a binding
-- of this shape, and a check that examines nothing proves nothing.
INSERT INTO policy_binding (id, tenant_id, kind, item_class_id, item_id, note) VALUES
    ('b0000000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     'shelf_life', '1c1a0000-0000-0000-0000-000000000001',
     '17e10000-0000-0000-0000-000000000001', 'tenant binding, class and item');

-- D72. Scoped to the counterparty taxonomy, which nothing in the fixture used
-- before: all three bindings above sit on the Product dimension, so the party
-- closure was exercised structurally and no policy ever resolved through it.
-- That made the fixture's own re-parent inconsequential -- it claimed to change
-- which shelf-life policy wins for every supplier underneath, and changed
-- nothing, because there was no such policy to win. This is the one that makes
-- the claim true: with this binding present, Gloveco resolves through Suppliers
-- only because of the move.
--
-- Measured from the finished fixture the number reads backwards, and that is the
-- function being right rather than wrong: the move has already folded, so moving
-- Glove suppliers *under* Suppliers now reports 0 -- it is already there -- and
-- moving it back to the root reports 1. Impact is always relative to where the
-- class currently sits, which is what a screen asking "are you sure" needs.
INSERT INTO policy_binding (id, tenant_id, kind, party_class_id, note) VALUES
    ('b0000000-0000-0000-0000-000000000004', '11111111-1111-1111-1111-111111111111',
     'shelf_life', '9c1a0000-0000-0000-0000-000000000001',
     'tenant binding, counterparty class');

-- Each version comes into force at the moment its policy_change records, which
-- is what J13 pairs them on. The tenant version was effective from January
-- against a change recorded in August, and nothing noticed for five migrations.
INSERT INTO shelf_life_policy (id, policy_binding_id, effective, min_shelf_life_days) VALUES
    ('50000000-0000-0000-0000-000000000001', 'b0000000-0000-0000-0000-000000000001',
     tstzrange('2026-01-01T00:00:00Z', NULL, '[)'), 30),
    ('50000000-0000-0000-0000-000000000002', 'b0000000-0000-0000-0000-000000000002',
     tstzrange('2026-08-04T00:10:00Z', NULL, '[)'), 90),
    ('50000000-0000-0000-0000-000000000003', 'b0000000-0000-0000-0000-000000000003',
     tstzrange('2026-08-04T00:10:00Z', NULL, '[)'), 120),
    ('50000000-0000-0000-0000-000000000004', 'b0000000-0000-0000-0000-000000000004',
     tstzrange('2026-08-04T00:10:00Z', NULL, '[)'), 60);

INSERT INTO policy_change (id, tenant_id, occurred_at, recorded_at, policy_binding_id,
    kind, tenant_scope_id, client_event_id, change_kind, reason, recorded_by_id) VALUES
    ('c0000000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     '2026-08-04T00:10:00Z', '2026-08-04T00:10:01Z', 'b0000000-0000-0000-0000-000000000002',
     'shelf_life', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000003', 'created',
     'Chilled lines need 90 days for the grocery channel', '77770000-0000-0000-0000-000000000001'),
    ('c0000000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     '2026-08-04T00:10:00Z', '2026-08-04T00:10:01Z', 'b0000000-0000-0000-0000-000000000003',
     'shelf_life', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000003', 'created',
     'This line goes to a customer requiring 120', '77770000-0000-0000-0000-000000000001'),
    ('c0000000-0000-0000-0000-000000000004', '11111111-1111-1111-1111-111111111111',
     '2026-08-04T00:10:00Z', '2026-08-04T00:10:01Z', 'b0000000-0000-0000-0000-000000000004',
     'shelf_life', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000003', 'created',
     'Suppliers must deliver with 60 days remaining', '77770000-0000-0000-0000-000000000001');

-- D83. A receiving pair, so the ceiling clamps are exercised rather than only
-- the floor. The platform permits ten percent over-delivery and requires lot
-- capture; the tenant tries for twenty-five and no lots. The tenant binding wins
-- -- it is theirs and it names a counterparty class -- and every one of its three
-- clamped fields is pulled back by the platform rule it cannot exceed.
--
-- **The platform half of the pair is no longer here.** Migration 65 ships it, so
-- that a deployment loading the schema and not this file can still receive goods.
-- Inserting it again would put two all-NULL platform bindings of one kind in the
-- database, which tie on every dimension and are exactly the equal-specificity
-- case D22 raises `policy_ambiguous` for. The pair is still a pair; one half of
-- it now arrives with the schema.
INSERT INTO policy_binding (id, tenant_id, kind, party_class_id, note) VALUES
    ('b0000000-0000-0000-0000-000000000006', '11111111-1111-1111-1111-111111111111',
     'receiving', '9c1a0000-0000-0000-0000-000000000001',
     'tenant receiving, counterparty class');

INSERT INTO receiving_policy (id, policy_binding_id, effective, respond_by_hours,
    require_lot, tolerance_over_pct, tolerance_under_pct) VALUES
    ('4ec00000-0000-0000-0000-000000000002', 'b0000000-0000-0000-0000-000000000006',
     tstzrange('2026-08-04T00:10:00Z', NULL, '[)'), 72, false, 25, 25);

INSERT INTO policy_change (id, tenant_id, occurred_at, recorded_at, policy_binding_id,
    kind, tenant_scope_id, client_event_id, change_kind, reason, recorded_by_id) VALUES
    ('c0000000-0000-0000-0000-000000000006', '11111111-1111-1111-1111-111111111111',
     '2026-08-04T00:10:00Z', '2026-08-04T00:10:01Z', 'b0000000-0000-0000-0000-000000000006',
     'receiving', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000003', 'created',
     'Suppliers asked for more headroom on over-delivery',
     '77770000-0000-0000-0000-000000000001');

-- The platform version carries no policy_change and cannot: policy_change reaches
-- client_event through (tenant_scope_id, client_event_id) and client_event.tenant_id
-- is NOT NULL, so an act belonging to no tenant has no event to name. J13 exempts
-- platform-shipped versions for that reason, and question 138 carries it.

-- ---------------------------------------------------------------------------
-- One finding in the queue, so the checks over it examine something
-- ---------------------------------------------------------------------------
--
-- A short pick, which is D8's ordinary case rather than anything exotic. J16
-- asserts no `policy_ambiguous` row exists, and with an empty queue it passes
-- while examining nothing, which proves the absence of nothing.
INSERT INTO discrepancy (id, tenant_id, kind, item_id, holder_location_id,
    expected_quantity, observed_quantity, detail, detected_at, detected_by_id, state) VALUES
    ('d15c0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'short_pick', '17e10000-0000-0000-0000-000000000001',
     '10c00000-0000-0000-0000-000000000001', 40, 36,
     'Picked 36 against a commitment of 40', '2026-08-04T08:00:00Z',
     '77770000-0000-0000-0000-000000000001', 'open');

-- ---------------------------------------------------------------------------
-- Fold the projections, so the checks have something to disagree with
-- ---------------------------------------------------------------------------

-- D64. One entry point, in the declared order. This block used to be five calls
-- in an order written down nowhere else, which is what question 139 was about:
-- the fixture was the only artefact that knew `projection_package_stamp` runs
-- after the package fold, and a scheduler does not read fixtures.
SELECT projection_run_all('11111111-1111-1111-1111-111111111111');

-- ---------------------------------------------------------------------------
-- Inbound: what we asked a supplier for
-- ---------------------------------------------------------------------------
--
-- D59. The mirror of the customer order below: ours, created here, priced in the
-- document's currency, arriving in a window. J54 checks both sides with one rule.

INSERT INTO source_channel (id, tenant_id, code, name, authority) VALUES
    ('5c000000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     'buyer_portal', 'Purchasing portal', 'local');

INSERT INTO purchase_order (id, tenant_id, site_id, supplier_party_id, order_number,
    source_channel_id, external_ref, currency, state, issued_at, created_at) VALUES
    ('90000000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'a5170000-0000-0000-0000-000000000001', '9a247000-0000-0000-0000-000000000002',
     'PO-2026-0031', '5c000000-0000-0000-0000-000000000003', 'GLV-77120', 'AUD',
     'issued', '2026-07-20T00:00:00Z', '2026-07-19T00:00:00Z');

-- Ordered at 175 per 100 against the 180 the receipt actually cost, which is the
-- gap D58's cost column exists to make visible rather than the same number twice.
INSERT INTO purchase_order_line (id, tenant_id, purchase_order_id, item_id,
    quantity_ordered, line_number, unit_price_minor, price_basis_quantity,
    expected_from, expected_to, owner_party_id, status_id) VALUES
    ('901e0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     '90000000-0000-0000-0000-000000000001', '17e10000-0000-0000-0000-000000000001',
     130, 1, 175, 100,
     '2026-08-03T00:00:00Z', '2026-08-05T00:00:00Z',
     NULL, '57a70000-0000-0000-0000-000000000001');

-- D60. Fold the issued order into a promise, then claim against it. An
-- allocation against future supply records on expected_supply.quantity_allocated
-- and never reaches `stock`, which is what makes cross-dock expressible without
-- inventing stock rows for goods that are not there.
--
-- 130 ordered, 100 arriving below of which three are corrected away net (four
-- less a correction to the correction), and 30 still claimed against the
-- remainder. That leaves promisable positive rather than negative, which is the
-- difference between a promise partly delivered and one whose claims outlive it
-- -- J58.
--
-- The received figure is 97 rather than 100 since D102/D103. It read 100 from
-- migration 21 until D102, against a receipt this same file corrects, because the
-- fold counted the arrival and not the correction to it; D103 then put the
-- second-level correction on the same path rather than only in a rolled-back test.
SELECT projection_run_all('11111111-1111-1111-1111-111111111111');

INSERT INTO stock_allocation (id, tenant_id, expected_supply_id, quantity, state, firm, bound_at)
SELECT 'a110c000-0000-0000-0000-000000000004',
       '11111111-1111-1111-1111-111111111111', e.id, 30, 'allocated', false,
       '2026-08-04T07:00:00Z'
  FROM expected_supply e
 WHERE e.purchase_order_line_id = '901e0000-0000-0000-0000-000000000001';

SELECT projection_run_all('11111111-1111-1111-1111-111111111111');

-- D61. The receipt against that promise, and the last item on the tier-0 list.
-- Ten cartons counted, the config converts them to 100, and the movement above
-- already carries the same evidence. `expected_quantity` is what the paperwork
-- said at the time, frozen: amending the order afterwards cannot rewrite what the
-- receiver was working from.
INSERT INTO goods_receipt (id, tenant_id, site_id, purchase_order_id, received_at,
    recorded_at, client_event_id, recorded_by_id) VALUES
    ('92c00000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'a5170000-0000-0000-0000-000000000001', '90000000-0000-0000-0000-000000000001',
     '2026-08-04T00:00:00Z', '2026-08-04T00:00:01Z',
     'ce000000-0000-0000-0000-000000000001', '77770000-0000-0000-0000-000000000001');

INSERT INTO goods_receipt_line (id, tenant_id, goods_receipt_id, item_id,
    expected_supply_id, expected_quantity, quantity, entered_quantity,
    entered_packaging_level,
    item_packing_config_id, lot_id, accepted_at, accepted_by_id,
    recorded_at, client_event_id, recorded_by_id)
SELECT '92c10000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
       '92c00000-0000-0000-0000-000000000001', '17e10000-0000-0000-0000-000000000001',
       e.id, 130, 100, 10, 'carton', '9ac40000-0000-0000-0000-000000000001',
       '10700000-0000-0000-0000-000000000001',
       '2026-08-04T00:00:00Z', '77770000-0000-0000-0000-000000000001',
       '2026-08-04T00:00:01Z', 'ce000000-0000-0000-0000-000000000001',
       '77770000-0000-0000-0000-000000000001'
  FROM expected_supply e
 WHERE e.purchase_order_line_id = '901e0000-0000-0000-0000-000000000001';

-- The receipt movement names its cause, which is what makes J26's fold a batch
-- load rather than an N+1.
UPDATE stock_movement SET goods_receipt_line_id = '92c10000-0000-0000-0000-000000000001'
 WHERE id = '5b000000-0000-0000-0000-000000000001';

SELECT projection_run_all('11111111-1111-1111-1111-111111111111');

-- D62. The handover, which is what `crates/server/src/receiving.rs` plans and
-- what nothing performed until now. The claim was made against a promise; the
-- promise has partly landed; so the claim moves to the stock it became, its
-- binding is restamped at the moment of arrival, and the promise it came from is
-- retained rather than forgotten.
--
-- Before this the fixture left a claim standing on a promise that had already
-- delivered, which is the state J58 was written from.
UPDATE stock_allocation a
   SET stock_id = s.id,
       expected_supply_id = NULL,
       origin_expected_supply_id = a.expected_supply_id,
       bound_at = '2026-08-04T00:00:00Z'
  FROM stock s
 WHERE a.id = 'a110c000-0000-0000-0000-000000000004'
   AND s.tenant_id = a.tenant_id
   AND s.holder_location_id = '10c00000-0000-0000-0000-000000000001';

SELECT projection_run_all('11111111-1111-1111-1111-111111111111');

-- ---------------------------------------------------------------------------
-- Outbound: one order, two fulfilments, one consignment spanning both
-- ---------------------------------------------------------------------------

-- D54. Two channels with different answers to "whose order is this".
--
-- NetSuite is ours: D39 is explicit that an order arriving from it is our own
-- intention reaching us through a channel, not a counterparty's claim. The EDI
-- channel is not ours, and J44 has nothing to examine without one.
INSERT INTO source_channel (id, tenant_id, code, name, authority) VALUES
    ('5c000000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'netsuite', 'NetSuite', 'local'),
    ('5c000000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     'edi_metcash', 'Metcash EDI', 'external');

-- **The promise is the one date in this file that moves.**
--
-- Everything else here is history and is written down to the second, because the
-- checks are about what happened in what order: a correction that sorts before
-- the change it corrects, a receipt whose occurred_at and recorded_at differ by
-- half an hour, a resolver asked what it knew at noon on the fourth. Shifting
-- any of that would be shifting the questions.
--
-- A promised window is not history. It is a statement about a future, and a
-- fixture loaded today that promises a date last August is stating something
-- false — which is why every row on the pack queue read `overdue` regardless of
-- when the database was built. So the promise is relative and the ledger is not,
-- and the two are different kinds of date rather than the same kind treated
-- inconsistently.
INSERT INTO "order" (id, tenant_id, site_id, customer_party_id, confirmation_number,
    source_channel_id, external_ref, promised_from, promised_to, placed_at, state, currency) VALUES
    ('04de0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'a5170000-0000-0000-0000-000000000001', '9a247000-0000-0000-0000-000000000002',
     'S260041', '5c000000-0000-0000-0000-000000000001', 'NS-88213',
     (CURRENT_DATE - 3)::timestamptz, (CURRENT_DATE + 2)::timestamptz,
     '2026-08-04T00:00:00Z', 'placed', 'AUD');

-- An externally-authoritative order, which is what D48 narrowed J44 for and what
-- D54 narrowed it again on a different axis.
INSERT INTO "order" (id, tenant_id, site_id, customer_party_id, confirmation_number,
    source_channel_id, external_ref, placed_at, state, currency) VALUES
    ('04de0000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     'a5170000-0000-0000-0000-000000000001', '9a247000-0000-0000-0000-000000000002',
     -- Ours: a letter for the kind, the year, then the sequence. `external_ref`
     -- beside it is the counterparty's, and the two never have to agree.
     'P264471', '5c000000-0000-0000-0000-000000000002', 'MC-99120',
     '2026-08-04T00:00:00Z', 'placed', 'AUD');

INSERT INTO order_line (id, tenant_id, order_id, item_id, quantity_ordered, line_number,
    unit_price_minor, price_basis_quantity) VALUES
    -- Transcribed as 10; the amendment below folds it to the 12 their document
    -- actually says.
    ('01e00000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     '04de0000-0000-0000-0000-000000000002', '17e10000-0000-0000-0000-000000000001', 10, 1,
     400, 100);

-- D50. Priced per 100 rather than per each, which is the shape that makes
-- integer minor units survive grocery: 345 per 100 is exact, 0.0345 each is not
-- expressible in cents at all.
--
-- D51. These are the values at order entry, not the values in force. Line 1 is
-- inserted at 50 and 345, and the amendments below fold it to 60 and 320, so the
-- fixture proves the fold actually writes rather than agreeing with what was
-- already there. A fixture where the base and the folded value coincide passes
-- J46 without exercising it.
INSERT INTO order_line (id, tenant_id, order_id, item_id, quantity_ordered, line_number,
    unit_price_minor, price_basis_quantity) VALUES
    ('01e00000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     '04de0000-0000-0000-0000-000000000001', '17e10000-0000-0000-0000-000000000001', 50, 1,
     345, 100),
    -- Line 2 exists to be removed. Nothing commits to ship it, which is what
    -- keeps J55 quiet: a removed line under an active fulfilment is a finding,
    -- and this one has no fulfilment_line.
    ('01e00000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     '04de0000-0000-0000-0000-000000000001', '17e10000-0000-0000-0000-000000000001', 24, 2,
     500, 1);

-- Two fulfilments against one order line: D15's point, that a commitment to ship
-- part of an order is its own thing.
INSERT INTO fulfilment (id, tenant_id, order_id, site_id, state) VALUES
    ('f01f0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     '04de0000-0000-0000-0000-000000000001', 'a5170000-0000-0000-0000-000000000001', 'released'),
    ('f01f0000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     '04de0000-0000-0000-0000-000000000001', 'a5170000-0000-0000-0000-000000000001', 'planned'),
    -- **Something left to pack.** The first two fulfilments are fully covered and
    -- one is part-despatched, which exercises the folds and leaves the fixture
    -- with no work on it: `/sites/{id}/open-lines` returned rows and none of them
    -- had room to claim. A demo of a packing system needs an order waiting to be
    -- packed, and so does any walk through the process.
    ('f01f0000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     '04de0000-0000-0000-0000-000000000001', 'a5170000-0000-0000-0000-000000000001', 'released');

INSERT INTO fulfilment_line (id, tenant_id, fulfilment_id, order_line_id, quantity) VALUES
    ('f11e0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'f01f0000-0000-0000-0000-000000000001', '01e00000-0000-0000-0000-000000000001', 40),
    ('f11e0000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     'f01f0000-0000-0000-0000-000000000002', '01e00000-0000-0000-0000-000000000001', 20),
    -- Uncovered on purpose: nothing is allocated against it, so it is the line an
    -- operator would be handed next.
    ('f11e0000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     'f01f0000-0000-0000-0000-000000000003', '01e00000-0000-0000-0000-000000000001', 6);

-- ---------------------------------------------------------------------------
-- Credentials, so somebody can actually sign on
-- ---------------------------------------------------------------------------
--
-- The password is `dock-station-1` for both, which is fine for a fixture and
-- would not be anywhere else. What matters here is the shape: a PHC string
-- carrying Argon2id at OWASP's m=19456, t=2, p=1, with a per-row salt — the two
-- digests differ because the salts do, which is the property a precomputed table
-- defeats when it is missing.
--
-- Kyle belongs to tenant Alpha and Dana to tenant Beta, so the fixture also
-- exercises the thing D19 makes possible: `person` is global, and which tenant a
-- person is acting for is a property of the session rather than of the person.

INSERT INTO person_credential (person_id, kind, phc) VALUES
    ('77770000-0000-0000-0000-000000000001', 'password',
     '$argon2id$v=19$m=19456,t=2,p=1$U4F5Q9+7wXaVoeC9JPk0oA$CYw6JXjkpvofcx4o3mc8FFql0tSkxw0U6bCo7uSTLYk'),
    ('77770000-0000-0000-0000-000000000002', 'password',
     '$argon2id$v=19$m=19456,t=2,p=1$BddjAjRKlZoTHlDd+eS3gQ$nyQE+O/ckR1LnmkhkAejMavGB+Lh5gYgUQjAjyPiNqw');

INSERT INTO carrier (id, tenant_id, name, code) VALUES
    ('ca440000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'Direct Transport', 'DIRECT'),
    ('ca440000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     'Swift Transport Services', 'SWIFT');

-- **The carrier and the service are separate rows, which is the point.**
-- Architecture: *"The carrier and the service used to book them are recorded
-- separately, so Swift booked through MachShip today and Swift booked directly
-- tomorrow are the same carrier, and the cost history survives the change."*
-- The walkthrough's route table is these: Swift next business day at 07:00 with
-- a manifest, Direct with two labels when the goods are on a pallet.
INSERT INTO carrier_service (id, carrier_id, name, code) VALUES
    ('ca450000-0000-0000-0000-000000000001', 'ca440000-0000-0000-0000-000000000002',
     'Swift next business day', 'SWIFT-NBD'),
    ('ca450000-0000-0000-0000-000000000002', 'ca440000-0000-0000-0000-000000000001',
     'Direct road freight', 'DIRECT-ROAD');
INSERT INTO freight_provider (id, tenant_id, name, kind) VALUES
    ('f9040000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'MachShip', 'machship');

INSERT INTO consignment (id, tenant_id, carrier_id, freight_provider_id, despatch_at) VALUES
    ('c05e0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'ca440000-0000-0000-0000-000000000001', 'f9040000-0000-0000-0000-000000000001',
     '2026-08-05T00:00:00Z');

-- The pallet is now a despatch package on the first fulfilment, and the
-- consignment reaches the fulfilment through it rather than by a direct FK,
-- which is what D15 dropped consignment.fulfilment_id to allow.
UPDATE package SET fulfilment_id = 'f01f0000-0000-0000-0000-000000000001',
                   sequence = 1, dimensions_source = 'computed'
 WHERE id = '9ac00000-0000-0000-0000-00000000000a';

-- D79 brought this table inside the tenancy boundary: it had no tenant_id and no
-- row-level security, so it could link one tenant's package to another's
-- consignment and nothing filtered the read.
INSERT INTO consignment_package (tenant_id, consignment_id, package_id) VALUES
    ('11111111-1111-1111-1111-111111111111',
     'c05e0000-0000-0000-0000-000000000001', '9ac00000-0000-0000-0000-00000000000a');

-- ---------------------------------------------------------------------------
-- This warehouse's boxes
-- ---------------------------------------------------------------------------
--
-- Migration 67 ships PALLET and SKID, which are standards. These are not: the
-- walkthrough describes presets *"named by item code, or by size (small box …
-- large box), or by type"*, and a second tenant packing different goods has
-- different boxes. Tenant-owned, which is also the only arm the application may
-- write — the shared arm belongs to the platform role under D55's policy pair.
--
-- `dimensions_fixed` is true on all three, so the constraint migration 67 added
-- requires all three sizes: a box that claims a fixed size and does not state it
-- is the flag without the fact.

INSERT INTO package_type (
        id, tenant_id, name, carrier_package_code, dimensions_fixed,
        length_mm, width_mm, height_mm, tare_weight_g, reusable, max_payload_g)
VALUES
    ('9a7e0000-0000-0000-0000-0000000000b1', '11111111-1111-1111-1111-111111111111',
     'small box', 'CTN', true, 320, 240, 180, 260, false, 15000),
    ('9a7e0000-0000-0000-0000-0000000000b2', '11111111-1111-1111-1111-111111111111',
     'medium box', 'CTN', true, 430, 305, 250, 420, false, 25000),
    ('9a7e0000-0000-0000-0000-0000000000b3', '11111111-1111-1111-1111-111111111111',
     'large box', 'CTN', true, 600, 400, 400, 700, false, 30000);

-- PALLET-A is the standard pallet, which is what its measurement below is
-- against: the preset says 1165 by 1165 and the scale agrees, while the height
-- is the stack and only the observation knows it.
UPDATE package SET package_type_id = '9a7e0000-0000-0000-0000-000000000001'
 WHERE id = '9ac00000-0000-0000-0000-00000000000a';

-- ---------------------------------------------------------------------------
-- PALLET-A is weighed and measured
-- ---------------------------------------------------------------------------
--
-- **J12 had a writer and nothing to check.** It asserts that an unsealed
-- package's dimension columns equal `observation_current`, and it reported
-- VACUOUS from the day it was written because no fixture package had ever been
-- measured -- the exact failure mode invariants.md says to watch: *"those are the
-- entries that will report success on the day they stop being checked."*
--
-- An Australian standard pallet, 1165 by 1165 millimetres, stacked to 1.4 metres
-- and weighing 312.5 kilograms. Entered in metres and kilograms, because that is
-- what the dock reads off a scale, and stored in the canonical base units the
-- dimension has -- which is Principle 5's pair: `entered_value` with its unit
-- beside `value_numeric` with none, so what the operator saw survives the
-- conversion. **312.5 is the number that matters here**: it is not a double, and
-- the fixture carries it as digits for the same reason the writer parses digits.

INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
    submitted_at, received_at) VALUES
    ('11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-0000000000a1',
     'a5170000-0000-0000-0000-000000000001',
     '77770000-0000-0000-0000-000000000001',
     '2026-08-04T03:10:00Z', '2026-08-04T03:10:01Z');

INSERT INTO observable (id, tenant_id, package_id) VALUES
    ('0b5e0000-0000-0000-0000-0000000000a1',
     '11111111-1111-1111-1111-111111111111',
     '9ac00000-0000-0000-0000-00000000000a');

-- One subject, one moment, four metrics. The scale is the instrument and the
-- channel says so, which is what makes the same fact arriving from EDI or from a
-- keyboard distinguishable without being a different shape.
INSERT INTO observation_event (id, tenant_id, client_event_id, observable_id,
    observed_at, recorded_by_id, method, ingestion_channel) VALUES
    ('0b5e0000-0000-0000-0000-0000000000e1',
     '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-0000000000a1',
     '0b5e0000-0000-0000-0000-0000000000a1',
     '2026-08-04T03:10:00Z', '77770000-0000-0000-0000-000000000001',
     'instrument', 'scale');

INSERT INTO observation (tenant_id, observation_event_id, observable_id, observed_at,
    client_event_id, metric_id, result_kind, dimension_id, value_numeric,
    entered_value, entered_unit_id)
SELECT '11111111-1111-1111-1111-111111111111',
       '0b5e0000-0000-0000-0000-0000000000e1',
       '0b5e0000-0000-0000-0000-0000000000a1',
       '2026-08-04T03:10:00Z',
       'ce000000-0000-0000-0000-0000000000a1',
       m.id, 'quantity', m.dimension_id, v.canonical, v.entered, u.id
  FROM (VALUES
        ('length',        1165::bigint, 1.165::numeric, 'm'),
        ('width',         1165,         1.165,          'm'),
        ('height',        1400,         1.4,            'm'),
        ('gross_weight',  312500,       312.5,          'kg'))
       AS v(metric_code, canonical, entered, unit_code)
  JOIN metric m ON m.code = v.metric_code AND m.tenant_id IS NULL
  JOIN unit u ON u.code = v.unit_code AND u.dimension_id = m.dimension_id;

-- The cache J12 compares against, written in the same breath as the fact. A
-- package carrying NULL against a recorded observation is a finding exactly as
-- loudly as one carrying a wrong number, because J12 compares with
-- `IS DISTINCT FROM` -- so the two are one act rather than a write and a habit.
UPDATE package
   SET length_mm = 1165, width_mm = 1165, height_mm = 1400,
       gross_weight_g = 312500, dimensions_source = 'confirmed'
 WHERE id = '9ac00000-0000-0000-0000-00000000000a';

-- Two amendments to the same order, touching different columns. The second
-- changes only the state, and must not clear the promised window the first set.
-- That is why D42's fold is per column rather than per row.
INSERT INTO intention_amendment (id, tenant_id, client_event_id, order_id, occurred_at,
    recorded_at, recorded_by_id, reason, revision_class, new_promised_to) VALUES
    ('a3e00000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000003', '04de0000-0000-0000-0000-000000000001',
     '2026-08-04T01:00:00Z', '2026-08-04T01:00:01Z', '77770000-0000-0000-0000-000000000001',
     'Customer asked for a later window', 'world_event', (CURRENT_DATE + 1)::timestamptz);

INSERT INTO intention_amendment (id, tenant_id, client_event_id, order_id, occurred_at,
    recorded_at, recorded_by_id, reason, revision_class, new_state) VALUES
    ('a3e00000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000003', '04de0000-0000-0000-0000-000000000001',
     '2026-08-04T02:00:00Z', '2026-08-04T02:00:01Z', '77770000-0000-0000-0000-000000000001',
     'Credit hold pending payment', 'world_event', 'on_hold');

-- D48. The third amendment is a correction rather than a change of mind, and it
-- touches the same column as the first on purpose. The window was typed wrong at
-- entry, so this carries the order's placed_at rather than the 03:00 moment the
-- typo was found. The fold therefore reads it *before* the customer's 01:00
-- change and promised_to stays at the customer's window.
--
-- Stamped 03:00 it would sort last, win, and the order would promise three days
-- earlier than the customer expects, with nothing anywhere reporting a problem.
-- That is the bug question 130 turned out to be, so the fixture holds the shape
-- that proves it fixed. The two values are relative for the reason the order
-- above gives, and only their order matters to the check.
INSERT INTO intention_amendment (id, tenant_id, client_event_id, order_id, occurred_at,
    recorded_at, recorded_by_id, reason, revision_class, new_promised_to) VALUES
    ('a3e00000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000004', '04de0000-0000-0000-0000-000000000001',
     '2026-08-04T00:00:00Z', '2026-08-04T03:00:00Z', '77770000-0000-0000-0000-000000000001',
     'Typed the wrong end of the window at order entry', 'record_error',
     (CURRENT_DATE - 2)::timestamptz);

-- ---------------------------------------------------------------------------
-- D51. The same three shapes again, one level down
-- ---------------------------------------------------------------------------
--
-- Each of these names `order_line_id` and no order-level column, which is what
-- `intention_amendment_subject_ck` requires: one amendment, one subject.

-- A quantity change. The line was entered at 50 and the customer went to 60,
-- which is what the two fulfilments together commit to ship.
INSERT INTO intention_amendment (id, tenant_id, client_event_id, order_id, order_line_id,
    occurred_at, recorded_at, recorded_by_id, reason, revision_class,
    new_quantity_ordered) VALUES
    ('a3e00000-0000-0000-0000-000000000004', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000005', '04de0000-0000-0000-0000-000000000001',
     '01e00000-0000-0000-0000-000000000001',
     '2026-08-04T04:00:00Z', '2026-08-04T04:00:01Z', '77770000-0000-0000-0000-000000000001',
     'Customer increased the line to a full carton', 'world_event', 60);

-- A renegotiation. Both halves of the price move together, because
-- `intention_amendment_price_pair_ck` will not accept one without the other, and
-- that constraint is exactly what lets the fold treat them as separate columns.
INSERT INTO intention_amendment (id, tenant_id, client_event_id, order_id, order_line_id,
    occurred_at, recorded_at, recorded_by_id, reason, revision_class,
    new_unit_price_minor, new_price_basis_quantity) VALUES
    ('a3e00000-0000-0000-0000-000000000005', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000005', '04de0000-0000-0000-0000-000000000001',
     '01e00000-0000-0000-0000-000000000001',
     '2026-08-04T05:00:00Z', '2026-08-04T05:00:01Z', '77770000-0000-0000-0000-000000000001',
     'Volume price agreed for the larger quantity', 'world_event', 320, 100);

-- D48 on a line, and the reason the split had to come first. The price was
-- mis-keyed at order entry: the agreed figure was 350, not 345. Stamped at the
-- moment the typo was found this would sort after the renegotiation and the line
-- would end up at 350, silently undoing a price both parties agreed at 05:00.
--
-- Carrying the order's `placed_at` puts it where it belongs, before the
-- renegotiation, and the line ends at 320 as agreed. The correction is still not
-- pointless: it is what makes the price history read 350 → 320 rather than
-- 345 → 320, which is the difference in a dispute about what was ever offered.
INSERT INTO intention_amendment (id, tenant_id, client_event_id, order_id, order_line_id,
    occurred_at, recorded_at, recorded_by_id, reason, revision_class,
    new_unit_price_minor, new_price_basis_quantity) VALUES
    ('a3e00000-0000-0000-0000-000000000006', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000006', '04de0000-0000-0000-0000-000000000001',
     '01e00000-0000-0000-0000-000000000001',
     '2026-08-04T00:00:00Z', '2026-08-04T06:00:00Z', '77770000-0000-0000-0000-000000000001',
     'Keyed 345 at order entry; the agreed price was 350', 'record_error', 350, 100);

-- D42's `line_removed`, which had nowhere to go until now. The line keeps its
-- row: `quantity_ordered` still says 24 and `line_state` says the customer no
-- longer wants it, which is a different statement from the line never existing.
INSERT INTO intention_amendment (id, tenant_id, client_event_id, order_id, order_line_id,
    occurred_at, recorded_at, recorded_by_id, reason, revision_class,
    new_line_state) VALUES
    ('a3e00000-0000-0000-0000-000000000007', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000006', '04de0000-0000-0000-0000-000000000001',
     '01e00000-0000-0000-0000-000000000002',
     '2026-08-04T07:00:00Z', '2026-08-04T07:00:01Z', '77770000-0000-0000-0000-000000000001',
     'Customer dropped the second line', 'world_event', 'removed');

-- D54, and the case D48 narrowed J44 to permit. We mis-keyed the quantity off
-- Metcash's purchase order: their document says 12 and we transcribed 10. Their
-- intention did not change, our copy of it was wrong, and succession is their
-- mechanism rather than ours — we cannot cancel their purchase order to fix our
-- own parse bug. So a `record_error` amendment is the only honest expression, and
-- J44 permits it on an externally-authoritative order.
--
-- It carries the order's `placed_at`, per J53: the value it revises is the
-- order's own original.
INSERT INTO intention_amendment (id, tenant_id, client_event_id, order_id, order_line_id,
    occurred_at, recorded_at, recorded_by_id, reason, revision_class,
    new_quantity_ordered) VALUES
    ('a3e00000-0000-0000-0000-000000000008', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000006', '04de0000-0000-0000-0000-000000000002',
     '01e00000-0000-0000-0000-000000000003',
     '2026-08-04T00:00:00Z', '2026-08-04T08:00:00Z', '77770000-0000-0000-0000-000000000001',
     'Transcribed 10 off the ORDERS message; their document says 12', 'record_error', 12);

SELECT projection_run_all('11111111-1111-1111-1111-111111111111');

-- ---------------------------------------------------------------------------
-- D53. Supply committed against the two fulfilment lines
-- ---------------------------------------------------------------------------
--
-- Without these every coverage fold examines nothing, and four projections that
-- agree with an empty source prove exactly as much as the checks they replaced.
--
-- `stock.id` is generated by the rebuild rather than written here, so these look
-- the cell up by where it is. That is also the only part of this file whose
-- identifiers are not fixed: everything the fixture writes has a stated uuid,
-- and everything a projection creates does not.
INSERT INTO stock_allocation (id, tenant_id, stock_id, fulfilment_line_id,
    quantity, state, firm, bound_at)
SELECT 'a110c000-0000-0000-0000-000000000001',
       '11111111-1111-1111-1111-111111111111', s.id,
       'f11e0000-0000-0000-0000-000000000001', 40, 'packed', true,
       '2026-08-04T06:00:00Z'
  FROM stock s
 WHERE s.holder_location_id = '10c00000-0000-0000-0000-000000000002'
   AND s.tenant_id = '11111111-1111-1111-1111-111111111111';

INSERT INTO stock_allocation (id, tenant_id, stock_id, fulfilment_line_id,
    quantity, state, firm, bound_at)
SELECT 'a110c000-0000-0000-0000-000000000002',
       '11111111-1111-1111-1111-111111111111', s.id,
       'f11e0000-0000-0000-0000-000000000002', 20, 'allocated', false,
       '2026-08-04T06:05:00Z'
  FROM stock s
 WHERE s.holder_location_id = '10c00000-0000-0000-0000-000000000001'
   AND s.tenant_id = '11111111-1111-1111-1111-111111111111';

-- A released allocation, which covers nothing and holds nothing. It is here so
-- the exclusion is exercised rather than assumed: if `released` ever crept into
-- the covered set, the second line would read 25 against a commitment of 20 and
-- J56 would say so.
INSERT INTO stock_allocation (id, tenant_id, stock_id, fulfilment_line_id,
    quantity, state, firm, bound_at)
SELECT 'a110c000-0000-0000-0000-000000000003',
       '11111111-1111-1111-1111-111111111111', s.id,
       'f11e0000-0000-0000-0000-000000000002', 5, 'released', false,
       '2026-08-04T06:10:00Z'
  FROM stock s
 WHERE s.holder_location_id = '10c00000-0000-0000-0000-000000000001'
   AND s.tenant_id = '11111111-1111-1111-1111-111111111111';

-- Both maintainers, in order. stock_allocation feeds the cell's allocated
-- quantity and the commitment's four, and migration 3 registered the first of
-- those to a function that never wrote it.
SELECT projection_run_all('11111111-1111-1111-1111-111111111111');

-- ---------------------------------------------------------------------------
-- Claims (D21)
-- ---------------------------------------------------------------------------
--
-- A supplier's despatch advice, end to end: the artefact it arrived in, the
-- claim, our position on it, the declared hierarchy, and one comparison against
-- reality. Without these the whole assertion mechanism is a set of empty tables
-- and every check over it is vacuous -- which passes, and proves nothing.
--
-- **A party representing us.** D21 makes the category symmetric: our outbound
-- despatch advice is as unrevisable as theirs, and `author_party_id` is NOT NULL,
-- so an outbound claim needs a party row for the operating company. Nothing marks
-- it as us, which is question 153.
INSERT INTO party (id, tenant_id, name, code) VALUES
    ('9a247000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     'Nylonite Operating', 'SELF');

INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
    submitted_at, received_at) VALUES
    ('11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-00000000000a',
     'a5170000-0000-0000-0000-000000000001', '77770000-0000-0000-0000-000000000001',
     '2026-08-04T02:00:00Z', '2026-08-04T02:00:01Z');

-- The artefact, verbatim. bytea because that is what arrived (principle 3).
INSERT INTO party_message (id, tenant_id, party_id, direction, channel,
    transport_ref, content_type, payload, byte_count, content_hash, occurred_at,
    recorded_at, client_event_id, parse_status, parser_version) VALUES
    ('9e550000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     '9a247000-0000-0000-0000-000000000002', 'inbound', 'edi',
     'IFT-88213', 'application/edifact',
     convert_to('UNH+1+DESADV:D:96A:UN''BGM+351+ASN-4471+9''', 'UTF8'),
     44, sha256(convert_to('ASN-4471', 'UTF8')),
     '2026-08-04T02:00:00Z', '2026-08-04T02:00:01Z',
     'ce000000-0000-0000-0000-00000000000a', 'parsed', 'edifact-d96a/0.1');

-- The subject. It exists whether or not a claim ever arrives, which is what makes
-- blind receipt a schema property rather than a workflow branch.
INSERT INTO inbound_shipment (id, tenant_id, site_id, supplier_party_id,
    vendor_shipment_ref, granularity, estimated_arrival_at) VALUES
    ('1b500000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'a5170000-0000-0000-0000-000000000001',
     '9a247000-0000-0000-0000-000000000002', 'ASN-4471', 'pallet',
     '2026-08-05T07:00:00Z');

-- Their claim.
INSERT INTO assertion (id, tenant_id, kind, direction, author_party_id,
    owner_party_id, site_id, author_reference, author_version, message_function,
    asserted_at, received_at, party_message_id, client_event_id) VALUES
    ('a55e0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'despatch_advice', 'inbound', '9a247000-0000-0000-0000-000000000002',
     NULL, 'a5170000-0000-0000-0000-000000000001',
     'ASN-4471', '1', 'original',
     '2026-08-04T01:55:00Z', '2026-08-04T02:00:01Z',
     '9e550000-0000-0000-0000-000000000001', 'ce000000-0000-0000-0000-00000000000a');

INSERT INTO despatch_advice (assertion_id, tenant_id, inbound_shipment_id,
    ship_from_gln, ship_to_gln, carrier_party_id, conveyance_ref, despatched_at,
    estimated_arrival_at, split_shipment, completes_order, granularity) VALUES
    ('a55e0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     '1b500000-0000-0000-0000-000000000001',
     '9312345000019', '9312345000026', NULL, 'TRK-77', '2026-08-04T01:50:00Z',
     '2026-08-05T07:00:00Z', false, true, 'pallet');

-- D43. The order level is a node: no SSCC, physical children beneath it. S35 is
-- about exactly this row, and D96 gave it the column its sentence needed:
-- `resolved_physical = false` is our reading of their level vocabulary, and the
-- CHECK is what makes "a document node carries no SSCC" true rather than watched.
INSERT INTO asserted_unit (id, tenant_id, assertion_id, parent_asserted_unit_id,
    level_code, sscc, sequence, raw_package_type_code, resolved_physical) VALUES
    ('a5010000-0000-0000-0000-000000000001',
     '11111111-1111-1111-1111-111111111111',
     'a55e0000-0000-0000-0000-000000000001', NULL, 'order', NULL, 1, NULL, false);

INSERT INTO asserted_unit (id, tenant_id, assertion_id, parent_asserted_unit_id,
    level_code, sscc, sequence, raw_package_type_code, resolved_physical) VALUES
    ('a5010000-0000-0000-0000-000000000002',
     '11111111-1111-1111-1111-111111111111',
     'a55e0000-0000-0000-0000-000000000001',
     'a5010000-0000-0000-0000-000000000001',
     'pallet', '393123450000000018', 1, 'PX', true);

-- In their vocabulary, with our resolution alongside it (rule 5).
--
-- This supplier states eaches: `PCE` is the EDIFACT default and resolves to `ea`,
-- whose factor is 1/1, so the declared and canonical quantities are the same
-- number. The row declared 400 against 40 until D92 -- an impossible pair nothing
-- in the suite could see -- and had no `raw_unit_code` at all until D93, so what
-- they actually wrote was not recoverable.
INSERT INTO asserted_unit_content (id, tenant_id, asserted_unit_id, raw_gtin,
    raw_item_code, resolved_item_id, raw_po_reference, quantity, entered_quantity,
    raw_unit_code, resolved_unit_id, lot_code, expiry_date,
    resolved_purchase_order_line_id, resolved_at, resolved_by_id, resolution_method)
SELECT 'a5c00000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
       'a5010000-0000-0000-0000-000000000002',
       '09312345000012', 'GLV-M-NIT', '17e10000-0000-0000-0000-000000000001',
       'PO-9001', 400, 400, 'PCE', u.id, 'L2026-014', '2027-06-30',
       '901e0000-0000-0000-0000-000000000001',
       '2026-08-04T02:00:05Z', '77770000-0000-0000-0000-000000000001', 'gtin_exact'
  FROM unit u WHERE u.code = 'ea';

-- Our position, which is ours: a fact, not a column on their row.
INSERT INTO assertion_stance (id, tenant_id, assertion_id, stance, reason_code,
    occurred_at, recorded_at, client_event_id, automation_key) VALUES
    ('a55c0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'a55e0000-0000-0000-0000-000000000001', 'in_force', 'parsed_clean',
     '2026-08-04T02:00:10Z', '2026-08-04T02:00:11Z',
     'ce000000-0000-0000-0000-00000000000a', 'edi-ingest');

-- Rule 4: a claim never checked is itself a finding. This one was checked and
-- agreed, so it is not.
INSERT INTO assertion_check (id, tenant_id, assertion_id, asserted_unit_id,
    asserted_unit_content_id, outcome, asserted_numeric, observed_numeric,
    checked_at, recorded_at, client_event_id, recorded_by_id) VALUES
    ('a55d0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'a55e0000-0000-0000-0000-000000000001',
     'a5010000-0000-0000-0000-000000000002',
     'a5c00000-0000-0000-0000-000000000001', 'agreed', 400, 400,
     '2026-08-05T07:30:00Z', '2026-08-05T07:30:01Z',
     'ce000000-0000-0000-0000-00000000000a', '77770000-0000-0000-0000-000000000001');

-- D44's half, and the symmetry D21 insisted on. Our order response goes out; their
-- disposition of it comes back. J49 is what checks that a disposition always
-- answers a claim travelling the other way.
INSERT INTO assertion (id, tenant_id, kind, direction, author_party_id, site_id,
    author_reference, author_version, asserted_at, received_at, captured_by_id,
    client_event_id) VALUES
    ('a55e0000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     'order_response', 'outbound', '9a247000-0000-0000-0000-000000000003',
     'a5170000-0000-0000-0000-000000000001', 'ORDRSP-118', '1',
     '2026-08-04T03:00:00Z', '2026-08-04T03:00:00Z',
     '77770000-0000-0000-0000-000000000001', 'ce000000-0000-0000-0000-00000000000a');

INSERT INTO assertion (id, tenant_id, kind, direction, author_party_id, site_id,
    author_reference, author_version, asserted_at, received_at, party_message_id,
    client_event_id) VALUES
    ('a55e0000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     'document_response', 'inbound', '9a247000-0000-0000-0000-000000000002',
     'a5170000-0000-0000-0000-000000000001', 'APERAK-55', '1',
     '2026-08-04T04:00:00Z', '2026-08-04T04:00:01Z',
     '9e550000-0000-0000-0000-000000000001', 'ce000000-0000-0000-0000-00000000000a');

INSERT INTO document_response (assertion_id, tenant_id, subject_assertion_id,
    response_code, response_reason) VALUES
    ('a55e0000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     'a55e0000-0000-0000-0000-000000000002', 'accepted', 'Confirmed in full');

SELECT projection_run_all('11111111-1111-1111-1111-111111111111');

-- D78. A supplier's declared numbers about a declared pallet: the weld between
-- D21 and D23. The weight and the dimensions are theirs, and neither becomes our
-- current value by arriving -- one is accepted and one is not, so J11 has both
-- branches to examine rather than one.
INSERT INTO observable (id, tenant_id, asserted_unit_id) VALUES
    ('0b500000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     'a5010000-0000-0000-0000-000000000002');

INSERT INTO observation_event (id, tenant_id, client_event_id, observable_id,
    observed_at, recorded_at, automation_key, method, ingestion_channel,
    asserted_by_party_id) VALUES
    ('0e000000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-00000000000a', '0b500000-0000-0000-0000-000000000002',
     '2026-08-04T01:55:00Z', '2026-08-04T02:00:01Z',
     'edi-ingest', 'asserted', 'edi', '9a247000-0000-0000-0000-000000000002');

-- Their gross weight. Never accepted, so it stays out of observation_current --
-- D23's own example: never trust their weight over our scale.
INSERT INTO observation (id, tenant_id, observation_event_id, client_event_id,
    observable_id, observed_at, metric_id, result_kind, dimension_id,
    value_numeric, entered_value, entered_unit_id)
SELECT '0b000000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
       '0e000000-0000-0000-0000-000000000002', 'ce000000-0000-0000-0000-00000000000a',
       '0b500000-0000-0000-0000-000000000002', '2026-08-04T01:55:00Z',
       m.id, 'quantity', m.dimension_id, 412000, 412, u.id
  FROM metric m, unit u
 WHERE m.code = 'gross_weight' AND u.code = 'kg';

-- Their pallet height, which we do accept: we have no measurement of our own and
-- a declared dimension is better than none. The acceptance is the act that makes
-- it ours to use.
INSERT INTO observation (id, tenant_id, observation_event_id, client_event_id,
    observable_id, observed_at, metric_id, result_kind, dimension_id,
    value_numeric, entered_value, entered_unit_id)
SELECT '0b000000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
       '0e000000-0000-0000-0000-000000000002', 'ce000000-0000-0000-0000-00000000000a',
       '0b500000-0000-0000-0000-000000000002', '2026-08-04T01:55:00Z',
       m.id, 'quantity', m.dimension_id, 1650, 1650, u.id
  FROM metric m, unit u
 WHERE m.code = 'height' AND u.code = 'mm';

INSERT INTO observation_acceptance (id, tenant_id, observation_id, accepted_at,
    recorded_at, client_event_id, accepted_by_id, reason) VALUES
    ('0ac00000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     '0b000000-0000-0000-0000-000000000003', '2026-08-04T02:05:00Z',
     '2026-08-04T02:05:01Z', 'ce000000-0000-0000-0000-00000000000a',
     '77770000-0000-0000-0000-000000000001',
     'No dimension of our own for this pallet configuration');

SELECT projection_run_all('11111111-1111-1111-1111-111111111111');

-- ---------------------------------------------------------------------------
-- Extension ceiling (D26, D36)
-- ---------------------------------------------------------------------------
--
-- Slots are issued, not configured: the ceiling IS these rows. Three rather than
-- fifty, because a fixture that issued the production number would make the
-- ceiling untestable -- reaching it is the behaviour worth exercising, and a
-- negative control that has to declare fifty schemes first is a control nobody
-- runs.
INSERT INTO extension_slot (tenant_id, kind, ordinal)
SELECT '11111111-1111-1111-1111-111111111111', 'record_scheme', g
  FROM generate_series(1, 3) g;
INSERT INTO extension_slot (tenant_id, kind, ordinal)
SELECT '11111111-1111-1111-1111-111111111111', 'metric', g
  FROM generate_series(1, 2) g;

-- A tenant scheme, claiming its slot through the function rather than by writing
-- claimed_key by hand -- the fixture exercises the enforcement path, because a
-- fixture that sets up the answer proves nothing about how the answer is reached.
SELECT extension_slot_claim('11111111-1111-1111-1111-111111111111',
                            'record_scheme', 'biosec_check');

INSERT INTO record_scheme (id, tenant_id, key, version, provenance, role,
    attaches_to, cardinality, physical_table, source, state, materialised_at,
    created_by_id, slot_ordinal) VALUES
    ('5c4e0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'biosec_check', 1, 'fact', 'reference', 'goods_receipt_line', 'many',
     'ext_biosec_check_v1', 'tenant', 'materialised', '2026-08-04T00:20:00Z',
     '77770000-0000-0000-0000-000000000001', 1);

-- Version 2 of the same key. D36's separation: every version shares one slot, so
-- maintaining a scheme costs nothing and the ceiling counts distinct schemes.
INSERT INTO record_scheme (id, tenant_id, key, version, provenance, role,
    attaches_to, cardinality, physical_table, source, state, created_by_id,
    slot_ordinal) VALUES
    ('5c4e0000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     'biosec_check', 2, 'fact', 'reference', 'goods_receipt_line', 'many',
     'ext_biosec_check_v2', 'tenant', 'declared',
     '77770000-0000-0000-0000-000000000001', 1);

-- The compiled symbol table. Each row becomes a real column with a real type,
-- and the parameter CHECKs are what stop a declaration that cannot compile.
INSERT INTO record_scheme_field (record_scheme_id, tenant_id, ordinal, column_name,
    label, field_type, required)
VALUES
    ('5c4e0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     1, 'inspected_at', 'Inspected at', 'timestamptz', true),
    ('5c4e0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     2, 'passed', 'Passed', 'boolean', true);

INSERT INTO record_scheme_field (record_scheme_id, tenant_id, ordinal, column_name,
    label, field_type, unit_id, required)
SELECT '5c4e0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
       3, 'sample_mass', 'Sample mass', 'quantity', u.id, false
  FROM unit u WHERE u.code = 'kg';

-- ---------------------------------------------------------------------------
-- Observation precedence (D85)
-- ---------------------------------------------------------------------------
--
-- D23's sentence, as a policy a manager owns rather than a rule compiled into a
-- maintainer. The tenant binding says what D78 hard-coded: never their number
-- over ours.
INSERT INTO policy_binding (id, tenant_id, kind, note) VALUES
    ('b0000000-0000-0000-0000-000000000007', '11111111-1111-1111-1111-111111111111',
     'observation_precedence', 'tenant precedence, any scope');

INSERT INTO observation_precedence_policy (id, policy_binding_id, effective,
    prefer_own, accept_counterparty) VALUES
    ('04ec0000-0000-0000-0000-000000000001', 'b0000000-0000-0000-0000-000000000007',
     tstzrange('2026-08-04T00:10:00Z', NULL, '[)'), true, true);

INSERT INTO policy_change (id, tenant_id, occurred_at, recorded_at, policy_binding_id,
    kind, tenant_scope_id, client_event_id, change_kind, reason, recorded_by_id) VALUES
    ('c0000000-0000-0000-0000-000000000007', '11111111-1111-1111-1111-111111111111',
     '2026-08-04T00:10:00Z', '2026-08-04T00:10:01Z', 'b0000000-0000-0000-0000-000000000007',
     'observation_precedence', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000003', 'created',
     'Our scale decides weight, whatever a supplier declares',
     '77770000-0000-0000-0000-000000000001');

-- And a competitor for a metric we have measured ourselves: their net weight,
-- accepted, and fresher than our scale reading. Under prefer_own it loses to our
-- 500 g anyway -- which is the whole point of the field, and it cannot be
-- demonstrated without a metric where both exist.
INSERT INTO observation_event (id, tenant_id, client_event_id, observable_id,
    observed_at, recorded_at, automation_key, method, ingestion_channel,
    asserted_by_party_id) VALUES
    ('0e000000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-00000000000a', '0b500000-0000-0000-0000-000000000001',
     '2026-08-04T01:55:00Z', '2026-08-04T02:00:01Z',
     'edi-ingest', 'asserted', 'edi', '9a247000-0000-0000-0000-000000000002');

INSERT INTO observation (id, tenant_id, observation_event_id, client_event_id,
    observable_id, observed_at, metric_id, result_kind, dimension_id,
    value_numeric, entered_value, entered_unit_id)
SELECT '0b000000-0000-0000-0000-000000000004', '11111111-1111-1111-1111-111111111111',
       '0e000000-0000-0000-0000-000000000003', 'ce000000-0000-0000-0000-00000000000a',
       '0b500000-0000-0000-0000-000000000001', '2026-08-04T01:55:00Z',
       m.id, 'quantity', m.dimension_id, 505, 0.505, u.id
  FROM metric m, unit u WHERE m.code = 'net_weight' AND u.code = 'kg';

INSERT INTO observation_acceptance (id, tenant_id, observation_id, accepted_at,
    recorded_at, client_event_id, automation_key, reason) VALUES
    ('0ac00000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     '0b000000-0000-0000-0000-000000000004', '2026-08-04T02:05:00Z',
     '2026-08-04T02:05:01Z', 'ce000000-0000-0000-0000-00000000000a', 'edi-ingest',
     'Recorded for comparison; our scale still decides');

-- D86. The other half of D23's sentence, which needs a binding that names a
-- metric: never their weight, whatever else we trust them for. D81 put Metric at
-- the head of this kind's precedence order for exactly this row.
INSERT INTO policy_binding (id, tenant_id, kind, metric_id, note)
SELECT 'b0000000-0000-0000-0000-000000000008', '11111111-1111-1111-1111-111111111111',
       'observation_precedence', m.id, 'tenant precedence, weight only'
  FROM metric m WHERE m.code = 'gross_weight';

INSERT INTO observation_precedence_policy (id, policy_binding_id, effective,
    prefer_own, accept_counterparty) VALUES
    ('04ec0000-0000-0000-0000-000000000002', 'b0000000-0000-0000-0000-000000000008',
     tstzrange('2026-08-04T00:10:00Z', NULL, '[)'), true, false);

INSERT INTO policy_change (id, tenant_id, occurred_at, recorded_at, policy_binding_id,
    kind, tenant_scope_id, client_event_id, change_kind, reason, recorded_by_id) VALUES
    ('c0000000-0000-0000-0000-000000000008', '11111111-1111-1111-1111-111111111111',
     '2026-08-04T00:10:00Z', '2026-08-04T00:10:01Z', 'b0000000-0000-0000-0000-000000000008',
     'observation_precedence', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000003', 'created',
     'A declared weight is never our weight, measured or not',
     '77770000-0000-0000-0000-000000000001');

-- Their gross weight is accepted, so the only thing that can keep it out of the
-- current value is the weight-only policy above. Before D86 the two metrics had
-- to share one answer and this row would have been current.
INSERT INTO observation_acceptance (id, tenant_id, observation_id, accepted_at,
    recorded_at, client_event_id, automation_key, reason) VALUES
    ('0ac00000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     '0b000000-0000-0000-0000-000000000002', '2026-08-04T02:05:00Z',
     '2026-08-04T02:05:01Z', 'ce000000-0000-0000-0000-00000000000a', 'edi-ingest',
     'Recorded; the weight policy decides whether it counts');

-- D91. The two halves of the receipt, joined. Until now the fixture held a
-- declared pallet and a counted one with nothing between them, which is the state
-- the schema was in.
--
-- The first line is the short delivery. Four hundred base units were declared, one
-- hundred arrived, and lot and expiry agree -- so the only divergence is quantity,
-- and it is real: `goods_receipt_variance` reads the arrival from the ledger and
-- reports -300 rather than subtracting ten cartons from four hundred units.
UPDATE goods_receipt_line
   SET asserted_unit_content_id = 'a5c00000-0000-0000-0000-000000000001'
 WHERE id = '92c10000-0000-0000-0000-000000000001';

-- The second is the divergence nothing else would catch, and it is why the
-- function compares dates at all. A second lot of the same gloves on the same
-- pallet: the quantities agree exactly, the lot codes agree, and the goods carry
-- September where the supplier declared June. Every count check in the system
-- passes on this line.
INSERT INTO lot (id, tenant_id, item_id, code, expiry_date) VALUES
    ('10700000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     '17e10000-0000-0000-0000-000000000001', 'L2026-021', '2026-09-30');

-- No `assertion_check` against this one, deliberately. D90 could already prove the
-- freeze when a check had compared against a claim; the half it could not prove is
-- a claim frozen by a receipt alone, and a content row with both would not
-- distinguish them.
-- **The carton claim, and the reason D93 exists.** This line of the same despatch
-- advice states `QTY+12:10:CT` -- ten cartons -- which before D93 could only be
-- recorded as ten of something or silently multiplied out. `CT` is kept verbatim,
-- our reading of it is a packaging level, and the case-pack version that sizes it
-- is named so a corrected case pack cannot rewrite what the claim meant. J65
-- converts 10 x 10 and agrees with the 100 base units declared.
INSERT INTO asserted_unit_content (id, tenant_id, asserted_unit_id, raw_gtin,
    raw_item_code, resolved_item_id, raw_po_reference, quantity, entered_quantity,
    raw_unit_code, resolved_packaging_level, item_packing_config_id,
    lot_code, expiry_date, resolved_purchase_order_line_id, resolved_at,
    resolved_by_id, resolution_method)
VALUES ('a5c00000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
       'a5010000-0000-0000-0000-000000000002',
       '09312345000012', 'GLV-M-NIT', '17e10000-0000-0000-0000-000000000001',
       'PO-9001', 100, 10, 'CT', 'carton', '9ac40000-0000-0000-0000-000000000001',
       'L2026-021', '2027-06-30', '901e0000-0000-0000-0000-000000000001',
       '2026-08-04T02:00:05Z', '77770000-0000-0000-0000-000000000001', 'gtin_exact');

-- Accepted, not refused. D5's floor rule and `disposition()` both say a receiver
-- records what is in front of them; a date that disagrees with the paperwork is a
-- finding for somebody else to raise, not a reason to hold the truck. No
-- `expected_supply_id`: this pallet arrived against the despatch advice rather
-- than against a promise the order had already made.
INSERT INTO goods_receipt_line (id, tenant_id, goods_receipt_id, item_id,
    quantity, entered_quantity, entered_packaging_level, item_packing_config_id,
    lot_id, asserted_unit_content_id, accepted_at, accepted_by_id,
    recorded_at, client_event_id, recorded_by_id) VALUES
    ('92c10000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     '92c00000-0000-0000-0000-000000000001', '17e10000-0000-0000-0000-000000000001',
     100, 10, 'carton', '9ac40000-0000-0000-0000-000000000001',
     '10700000-0000-0000-0000-000000000002',
     'a5c00000-0000-0000-0000-000000000002',
     '2026-08-04T00:00:00Z', '77770000-0000-0000-0000-000000000001',
     '2026-08-04T00:00:01Z', 'ce000000-0000-0000-0000-000000000001',
     '77770000-0000-0000-0000-000000000001');

-- The arrival, so the base comparison has something to read. Ten cartons at the
-- same case pack, which is the hundred the ledger stores.
INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
    to_location_id, to_lot_id, to_status_id, to_owner_id,
    reason, occurred_at, recorded_at, recorded_by_id,
    entered_quantity, entered_packaging_level, item_packing_config_id,
    goods_receipt_line_id) VALUES
    ('5b000000-0000-0000-0000-000000000004', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000001', '17e10000-0000-0000-0000-000000000001', 100,
     '10c00000-0000-0000-0000-000000000001', '10700000-0000-0000-0000-000000000002',
     '57a70000-0000-0000-0000-000000000001', '9a247000-0000-0000-0000-000000000001',
     'receipt', '2026-08-04T00:00:00Z', '2026-08-04T00:00:01Z',
     '77770000-0000-0000-0000-000000000001',
     10, 'carton', '9ac40000-0000-0000-0000-000000000001',
     '92c10000-0000-0000-0000-000000000002');

-- D96. The pallet they declared, and the pallet the receiver scanned, are the
-- same pallet -- and until now nothing could say so.
--
-- The package is minted from the scan rather than from the claim, which is J34:
-- an `identified` event carrying the licence plate the receiver read off the
-- label. That the SSCC on it matches the one the despatch advice declared is what
-- makes the collapse evidence rather than assertion, and it is the case
-- `asserted_unit_collapse` refuses when the two disagree.
INSERT INTO package (id, tenant_id) VALUES
    ('9ac00000-0000-0000-0000-00000000000c', '11111111-1111-1111-1111-111111111111');

INSERT INTO package_event (id, tenant_id, package_id, kind, source, occurred_at,
    recorded_at, client_event_id, recorded_by_id, sscc) VALUES
    ('9ae00000-0000-0000-0000-00000000000c', '11111111-1111-1111-1111-111111111111',
     '9ac00000-0000-0000-0000-00000000000c', 'identified', 'operator_scan',
     '2026-08-05T07:15:00Z', '2026-08-05T07:15:01Z',
     'ce000000-0000-0000-0000-000000000001', '77770000-0000-0000-0000-000000000001',
     '393123450000000018');

INSERT INTO package_event (id, tenant_id, package_id, kind, source, occurred_at,
    recorded_at, client_event_id, recorded_by_id, location_id) VALUES
    ('9ae00000-0000-0000-0000-00000000000d', '11111111-1111-1111-1111-111111111111',
     '9ac00000-0000-0000-0000-00000000000c', 'placed', 'operator_scan',
     '2026-08-05T07:16:00Z', '2026-08-05T07:16:01Z',
     'ce000000-0000-0000-0000-000000000001', '77770000-0000-0000-0000-000000000001',
     '10c00000-0000-0000-0000-000000000001');

-- The fold has to have run before the collapse can check the SSCC, because
-- package.sscc is a projection of the event above.
SELECT projection_run_all('11111111-1111-1111-1111-111111111111');

SELECT asserted_unit_collapse('a5010000-0000-0000-0000-000000000002',
                              '9ac00000-0000-0000-0000-00000000000c',
                              '77770000-0000-0000-0000-000000000001');

-- D87. The line was accepted against a policy, and now says which one: the
-- version the tenant's receiving binding carries, whose tolerances the acceptance
-- was measured against.
UPDATE goods_receipt_line
   SET receiving_policy_id = '4ec00000-0000-0000-0000-000000000002'
 WHERE accepted_at IS NOT NULL OR rejected_at IS NOT NULL;

SELECT projection_run_all('11111111-1111-1111-1111-111111111111');

-- ---------------------------------------------------------------------------
-- The second commitment reaches the truck
-- ---------------------------------------------------------------------------
--
-- D99 gave the ledger its outbound cause and D100 moved the fold onto it, and
-- **all three of the checks that arrived with them examined nothing.** J68, J69 and
-- J70 were vacuous against this fixture for the reason this file exists to prevent:
-- not that there was nothing to check, but that nobody had inserted anything.
--
-- One line, picked into a carton, sealed, and despatched. The same shape the
-- outbound walk runs, written with the fixed identifiers and fixed clocks this file
-- requires, so what the checks report is a fact about the code.
--
-- The allocation against this line is deliberately left at `allocated`. Nothing
-- advances it, because nothing has to: that is the split D100 adopted, and this row
-- is where it is visible. Coverage reads 20 from an intention nobody carried out
-- while picked and packed read 20 and despatched reads 15 (five of the despatch
-- reversed back into the carton). Under migration 15's fold the same row would
-- have read 20, 0, 0, 0 -- a commitment covered by a plan, against a carton already
-- on a truck.

INSERT INTO package (id, tenant_id, barcode, fulfilment_id, sequence,
    dimensions_source) VALUES
    ('9ac00000-0000-0000-0000-00000000000d', '11111111-1111-1111-1111-111111111111',
     'CARTON-D', 'f01f0000-0000-0000-0000-000000000002', 1, 'computed');

-- D97: an event asserting a placement says where. A carton comes into existence at
-- the packing bench, and before migration 51 the same event without a holder was
-- accepted and then stopped every projection for the tenant.
INSERT INTO package_event (id, tenant_id, package_id, kind, source, occurred_at,
    recorded_at, client_event_id, recorded_by_id, location_id) VALUES
    ('9ae00000-0000-0000-0000-0000000000d1', '11111111-1111-1111-1111-111111111111',
     '9ac00000-0000-0000-0000-00000000000d', 'created', 'operator_scan',
     '2026-08-05T08:00:00Z', '2026-08-05T08:00:01Z',
     'ce000000-0000-0000-0000-000000000009', '77770000-0000-0000-0000-000000000001',
     '10c00000-0000-0000-0000-000000000003');

-- The pick. `from_location_id` is set, which is D99's picked shape and what
-- separates it from the holder-to-holder re-handling that may follow a pick.
INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
    from_location_id, from_lot_id, from_status_id, from_owner_id,
    to_package_id, to_lot_id, to_status_id, to_owner_id,
    reason, occurred_at, recorded_at, recorded_by_id, fulfilment_line_id) VALUES
    ('5b000000-0000-0000-0000-0000000000d1', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000009', '17e10000-0000-0000-0000-000000000001', 20,
     '10c00000-0000-0000-0000-000000000001', '10700000-0000-0000-0000-000000000001',
     '57a70000-0000-0000-0000-000000000001', '9a247000-0000-0000-0000-000000000001',
     '9ac00000-0000-0000-0000-00000000000d', '10700000-0000-0000-0000-000000000001',
     '57a70000-0000-0000-0000-000000000001', '9a247000-0000-0000-0000-000000000001',
     'pick', '2026-08-05T08:02:00Z', '2026-08-05T08:02:01Z',
     '77770000-0000-0000-0000-000000000001',
     'f11e0000-0000-0000-0000-000000000002');

-- Sealing moves no stock, which is exactly why `packed_quantity` cannot be a
-- grouping over the ledger alone and reads the carton's status instead.
INSERT INTO package_event (id, tenant_id, package_id, kind, source, occurred_at,
    recorded_at, client_event_id, recorded_by_id) VALUES
    ('9ae00000-0000-0000-0000-0000000000d2', '11111111-1111-1111-1111-111111111111',
     '9ac00000-0000-0000-0000-00000000000d', 'sealed', 'operator_scan',
     '2026-08-05T08:05:00Z', '2026-08-05T08:05:01Z',
     'ce000000-0000-0000-0000-000000000009', '77770000-0000-0000-0000-000000000001');

INSERT INTO package_event (id, tenant_id, package_id, kind, source, occurred_at,
    recorded_at, client_event_id, recorded_by_id) VALUES
    ('9ae00000-0000-0000-0000-0000000000d3', '11111111-1111-1111-1111-111111111111',
     '9ac00000-0000-0000-0000-00000000000d', 'despatched', 'operator_scan',
     '2026-08-05T08:10:00Z', '2026-08-05T08:10:01Z',
     'ce000000-0000-0000-0000-00000000000a', '77770000-0000-0000-0000-000000000001');

-- The stock leaves. No `to` side at all, which the whole-key CHECKs permit
-- precisely so goods can exit the building, and which is D45's arrival test
-- mirrored. It leaves the carton the pick filled, so J70 reads it as re-handling
-- rather than as a pick out of package-held storage.
INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
    from_package_id, from_lot_id, from_status_id, from_owner_id,
    reason, occurred_at, recorded_at, recorded_by_id, fulfilment_line_id) VALUES
    ('5b000000-0000-0000-0000-0000000000d2', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-00000000000a', '17e10000-0000-0000-0000-000000000001', 20,
     '9ac00000-0000-0000-0000-00000000000d', '10700000-0000-0000-0000-000000000001',
     '57a70000-0000-0000-0000-000000000001', '9a247000-0000-0000-0000-000000000001',
     'despatch', '2026-08-05T08:10:00Z', '2026-08-05T08:10:01Z',
     '77770000-0000-0000-0000-000000000001',
     'f11e0000-0000-0000-0000-000000000002');

-- Five of the twenty were despatched in error and come back into the carton.
-- Partial reverse on the outbound arm: despatched_quantity must read 15, not 0
-- (D100's bug) and not 20 (ignoring the correction). The units land package-held,
-- which is the shape D101 needed a fixture cell for — stock in a carton that is
-- still there after the run, not only mid-walk. Without this row every package-held
-- cell was despatched empty and the dual-writer bug had nothing left to fail on.
INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
    to_package_id, to_lot_id, to_status_id, to_owner_id,
    reason, occurred_at, recorded_at, recorded_by_id,
    reverses_movement_id, adjustment_reason_id, fulfilment_line_id) VALUES
    ('5b000000-0000-0000-0000-0000000000d3', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-00000000000a', '17e10000-0000-0000-0000-000000000001', 5,
     '9ac00000-0000-0000-0000-00000000000d', '10700000-0000-0000-0000-000000000001',
     '57a70000-0000-0000-0000-000000000001', '9a247000-0000-0000-0000-000000000001',
     'adjustment', '2026-08-05T08:10:00Z', '2026-08-05T08:15:00Z',
     '77770000-0000-0000-0000-000000000001',
     '5b000000-0000-0000-0000-0000000000d2',
     (SELECT id FROM adjustment_reason WHERE code = 'miscount' AND tenant_id IS NULL),
     'f11e0000-0000-0000-0000-000000000002');

-- ---------------------------------------------------------------------------
-- The work as it is actually handed out
-- ---------------------------------------------------------------------------
--
-- Everything above this line was built to exercise a fold. What follows is built
-- to be recognised: the numbers a packer is given, an item with the code and the
-- description that are printed on its label, and a carton that is one of the
-- three sizes on the shelf behind the bench.
--
-- **Migration 71's column, populated.** The three fulfilments against S260041
-- were indistinguishable from each other on any screen, because the only string
-- any of them carried was the order's. Now each is called by its own number,
-- which is what makes a queue of them readable at all.
UPDATE fulfilment SET reference = 'IF265591'
 WHERE id = 'f01f0000-0000-0000-0000-000000000001';
UPDATE fulfilment SET reference = 'IF265592'
 WHERE id = 'f01f0000-0000-0000-0000-000000000002';
UPDATE fulfilment SET reference = 'IF265593'
 WHERE id = 'f01f0000-0000-0000-0000-000000000003';

-- The medium box, corrected to the one on the shelf: 450 by 340 by 410. The
-- previous numbers were invented to give migration 67's constraint three values
-- to check, and a demo that shows a made-up carton beside a real item code is
-- showing that neither was looked up.
--
-- Its weight is not here and cannot be. `tare_weight_g` is what the empty box
-- weighs; what goes on the consignment is the gross, and that is knowable only
-- once the boots are in it. Stage 4 of the walkthrough is that one measurement.
UPDATE package_type
   SET length_mm = 450, width_mm = 340, height_mm = 410, tare_weight_g = 480,
       max_payload_g = 25000
 WHERE id = '9a7e0000-0000-0000-0000-0000000000b2';

-- An item that is not lot-tracked, which no fixture item was.
--
-- Every bench line so far has had a lot code beside it because GLOVE-M is the
-- only thing anybody ships here, and D33 says tracking is a property of the item.
-- Boots are not batched, so this is also the first fixture row that proves the
-- pick path works with `to_lot_id` null rather than only with it set.
-- **The style, because the boot is sold in thirteen sizes and packed in one
-- box.** A boot catalogue carries a code per size, `-03` through `-13`, while
-- the prepack list that measures them names the style alone and says nothing
-- about which size was on the scale.
-- Migration 73 exists for that gap, so the fixture holds both ends of it: a
-- style with a carton measured against it, and a variant with none of its own.
INSERT INTO item_style (id, tenant_id, code, description) VALUES
    ('57110000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     'STY-7720', 'Ridgeway StepSure Gumboot - Steel Toe - Green');

INSERT INTO item (id, tenant_id, code, description, base_unit_id, tracking, style_id)
SELECT '17e10000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
       'STY-7720-08',
       'Ridgeway StepSure Gumboot - Steel Toe - Pair - Green - AU8 EU42',
       u.id, 'none', '57110000-0000-0000-0000-000000000001'
  FROM unit u WHERE u.code = 'ea';

INSERT INTO item_classification (tenant_id, item_id, item_class_id) VALUES
    ('11111111-1111-1111-1111-111111111111',
     '17e10000-0000-0000-0000-000000000002', '1c1a0000-0000-0000-0000-000000000001');

INSERT INTO party (id, tenant_id, name, code, party_class_id) VALUES
    ('9a247000-0000-0000-0000-000000000004', '11111111-1111-1111-1111-111111111111',
     'Kalgoorlie Mine Supplies', 'KALMINE', NULL);

INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
    submitted_at, received_at) VALUES
    ('11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000012',
     'a5170000-0000-0000-0000-000000000001', '77770000-0000-0000-0000-000000000001',
     '2026-08-04T06:00:00Z', '2026-08-04T06:00:01Z');

-- Sixty pairs on the pick face, so the queue below has stock to be packed from.
INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
    to_location_id, to_status_id, to_owner_id,
    reason, occurred_at, recorded_at, recorded_by_id,
    unit_cost_minor, cost_basis_quantity, cost_currency) VALUES
    ('5b000000-0000-0000-0001-000000000001', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000012', '17e10000-0000-0000-0000-000000000002', 60,
     '10c00000-0000-0000-0000-000000000001',
     '57a70000-0000-0000-0000-000000000001', '9a247000-0000-0000-0000-000000000001',
     'receipt', '2026-08-04T06:00:00Z', '2026-08-04T06:00:01Z',
     '77770000-0000-0000-0000-000000000001',
     3850, 1, 'AUD');

INSERT INTO "order" (id, tenant_id, site_id, customer_party_id, confirmation_number,
    source_channel_id, external_ref, promised_from, promised_to, placed_at, state,
    currency) VALUES
    ('04de0000-0000-0000-0000-000000000003', '11111111-1111-1111-1111-111111111111',
     'a5170000-0000-0000-0000-000000000001', '9a247000-0000-0000-0000-000000000004',
     'S260052', '5c000000-0000-0000-0000-000000000001', 'NS-88240',
     CURRENT_DATE::timestamptz, CURRENT_DATE::timestamptz,
     '2026-08-04T05:40:00Z', 'placed', 'AUD');

INSERT INTO order_line (id, tenant_id, order_id, item_id, quantity_ordered, line_number,
    unit_price_minor, price_basis_quantity) VALUES
    ('01e00000-0000-0000-0000-000000000004', '11111111-1111-1111-1111-111111111111',
     '04de0000-0000-0000-0000-000000000003', '17e10000-0000-0000-0000-000000000002', 24, 1,
     8900, 1);

-- IF400187, which is the number the request that built this screen quoted.
INSERT INTO fulfilment (id, tenant_id, order_id, site_id, state, reference) VALUES
    ('f01f0000-0000-0000-0000-000000000004', '11111111-1111-1111-1111-111111111111',
     '04de0000-0000-0000-0000-000000000003', 'a5170000-0000-0000-0000-000000000001',
     'released', 'IF400187');

INSERT INTO fulfilment_line (id, tenant_id, fulfilment_id, order_line_id, quantity) VALUES
    ('f11e0000-0000-0000-0000-000000000004', '11111111-1111-1111-1111-111111111111',
     'f01f0000-0000-0000-0000-000000000004', '01e00000-0000-0000-0000-000000000004', 24);

-- ---------------------------------------------------------------------------
-- How big a carton of boots is, which is a fact about the kind and not the box
-- ---------------------------------------------------------------------------
--
-- **This is the first item observation in the project.** `observable` has had an
-- item arm since migration 7 — `(item_id, packaging_level, item_packing_config_id)`,
-- with `applies_to` on length, width, height and the three weights already
-- listing `item` — and nothing has ever written one, because until a prepack
-- list arrives there is nothing to write. Migration 72 fixed the hole that would
-- have made loading one mint a rival subject per line.
--
-- A carton needs a case pack to be a definite object, so the config comes first.
-- **Six pairs to a carton is a placeholder** and is the one number here waiting
-- on the real prepack list; the carton's size is not, because the boots go in
-- the medium box and the medium box is measured above.
INSERT INTO item_packing_config (id, tenant_id, item_id, units_per_inner,
    inners_per_carton, cartons_per_layer, layers_per_pallet, effective_from) VALUES
    ('9ac40000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     '17e10000-0000-0000-0000-000000000002', 1, 6, 6, 4, '2026-01-01');

-- Against the *style*. Nobody measured a carton of size 8; somebody measured a
-- carton of these boots, and every size inherits it until one is measured on its
-- own. Writing it onto the variant would be four measurements where there was one.
INSERT INTO observable (id, tenant_id, item_style_id, packaging_level, item_packing_config_id) VALUES
    ('0b5e0000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111',
     '57110000-0000-0000-0000-000000000001', 'carton',
     '9ac40000-0000-0000-0000-000000000002');

INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
    submitted_at, received_at) VALUES
    ('11111111-1111-1111-1111-111111111111', 'ce000000-0000-0000-0000-000000000013',
     'a5170000-0000-0000-0000-000000000001', '77770000-0000-0000-0000-000000000001',
     '2026-08-04T05:00:00Z', '2026-08-04T05:00:01Z');

-- `transcribed` off a `csv`, because that is what a prepack list is and the
-- vocabulary already had both words. A cube read off a cubing scanner is
-- `instrument` off a `scanner` and is a different fact about the same subject —
-- which is the disagreement this system exists to surface rather than average.
INSERT INTO observation_event (id, tenant_id, client_event_id, observable_id,
    observed_at, recorded_by_id, method, ingestion_channel) VALUES
    ('0b5e0000-0000-0000-0000-00000000000e', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000013', '0b5e0000-0000-0000-0000-000000000001',
     '2026-08-04T05:00:00Z', '77770000-0000-0000-0000-000000000001',
     'transcribed', 'csv');

-- Canonical units only: there is no unit column on `observation`, so a value
-- stored in anything but millimetres and grams is unrepresentable rather than
-- discouraged. `entered_value` and `entered_unit_id` keep what the sheet said.
INSERT INTO observation (id, tenant_id, observation_event_id, observable_id, observed_at,
    client_event_id, metric_id, result_kind, dimension_id, value_numeric,
    entered_value, entered_unit_id)
SELECT v.id, '11111111-1111-1111-1111-111111111111',
       '0b5e0000-0000-0000-0000-00000000000e', '0b5e0000-0000-0000-0000-000000000001',
       '2026-08-04T05:00:00Z', 'ce000000-0000-0000-0000-000000000013',
       m.id, 'quantity', m.dimension_id, v.canonical, v.entered, u.id
  FROM (VALUES
        ('0b5e0000-0000-0000-0000-000000000011'::uuid, 'length',       450::bigint, 45::numeric,   'cm'),
        ('0b5e0000-0000-0000-0000-000000000012'::uuid, 'width',        340,         34,            'cm'),
        ('0b5e0000-0000-0000-0000-000000000013'::uuid, 'height',       410,         41,            'cm'),
        ('0b5e0000-0000-0000-0000-000000000014'::uuid, 'gross_weight', 11400,       11.4,          'kg')
       ) AS v(id, metric, canonical, entered, unit)
  JOIN metric m ON m.code = v.metric AND m.tenant_id IS NULL
  JOIN unit u ON u.code = v.unit;

-- **And one measurement against the size itself, so specificity has something to
-- resolve.** A pair of size 8 boots was weighed here; a carton of them was not.
-- D108 resolves per fact, so this code answers `each` from its own row and
-- `carton` from its style's, which is the shape the real data has.
INSERT INTO observable (id, tenant_id, item_id, packaging_level) VALUES
    ('0b5e0000-0000-0000-0000-000000000002', '11111111-1111-1111-1111-111111111111',
     '17e10000-0000-0000-0000-000000000002', 'each');

INSERT INTO observation_event (id, tenant_id, client_event_id, observable_id,
    observed_at, recorded_by_id, method, ingestion_channel) VALUES
    ('0b5e0000-0000-0000-0000-00000000000f', '11111111-1111-1111-1111-111111111111',
     'ce000000-0000-0000-0000-000000000013', '0b5e0000-0000-0000-0000-000000000002',
     '2026-08-04T05:00:00Z', '77770000-0000-0000-0000-000000000001',
     'instrument', 'scale');

INSERT INTO observation (id, tenant_id, observation_event_id, observable_id, observed_at,
    client_event_id, metric_id, result_kind, dimension_id, value_numeric,
    entered_value, entered_unit_id)
SELECT '0b5e0000-0000-0000-0000-000000000021', '11111111-1111-1111-1111-111111111111',
       '0b5e0000-0000-0000-0000-00000000000f', '0b5e0000-0000-0000-0000-000000000002',
       '2026-08-04T05:00:00Z', 'ce000000-0000-0000-0000-000000000013',
       m.id, 'quantity', m.dimension_id, 1900, 1.9, u.id
  FROM metric m, unit u
 WHERE m.code = 'gross_weight' AND m.tenant_id IS NULL AND u.code = 'kg';

-- **This insert is two joins, and a join that matches nothing inserts nothing
-- and reports success.** It happened on the first run of the block above: `cm`
-- was not a unit yet, so three of the four rows evaporated and the fixture
-- loaded clean. Migration 7 warns about exactly this in its own metric seed;
-- the warning is worth nothing unless the next join says it too.
DO $$
DECLARE n integer;
BEGIN
    SELECT count(*) INTO n FROM observation
     WHERE observable_id = '0b5e0000-0000-0000-0000-000000000001';
    IF n <> 4 THEN
        RAISE EXCEPTION 'the carton measurements inserted % rows, expected 4', n;
    END IF;
END
$$;

SELECT projection_run_all('11111111-1111-1111-1111-111111111111');

-- **Beta is folded too, and until D95 it never had been.** The fixture ran the
-- maintainers for alpha eleven times and for beta not once, which was invisible
-- while nothing recorded that a fold had happened: beta's projections were empty
-- because beta has little to project, and "correctly empty" and "never computed"
-- looked identical. J66's never-ran arm found it on its first run against a fresh
-- database, which is the arm that exists because it is the one that hides.
--
-- A second tenant that proves tenant isolation should be maintained the way the
-- first one is, or it proves isolation of a state no deployment would be in.
-- ---------------------------------------------------------------------------
-- What a scan resolves to (D34, migration 79)
-- ---------------------------------------------------------------------------
--
-- Three bindings, chosen so the fixture reaches every arm the resolver has.
--
-- The gumboot carries a **GTIN**, stored in the fourteen characters everything
-- is compared in. Its printed EAN-13 is `9312345678907` and the GS1-128 on the
-- carton carries `09312345678907` under AI 01 — the same trade item, which is
-- the whole reason normalisation happens on write. A resolver that held both
-- spellings would miss the carton scan, silently, which D34 calls the most
-- common integration defect in this area.
--
-- The glove carries an **internal** code, which is the ordinary case here: a
-- Code 128 of something we made up, with a quantity, because only a GTIN may
-- leave `quantity` NULL and only because a variable-measure GTIN carries the
-- weight in the barcode instead.
--
-- And one **closed** binding, so the `effective` range is exercised rather than
-- merely present: `LEGACY-9` meant the glove until the end of 2025 and means
-- nothing now. A scan of it today resolves to nothing, and the row is still
-- what explains a movement recorded against it last year — which is the reason
-- D31 retains these indefinitely and the reason this is a range rather than an
-- `active` boolean.
INSERT INTO item_barcode (tenant_id, item_id, barcode, scheme, unit_id, quantity, effective)
SELECT '11111111-1111-1111-1111-111111111111',
       '17e10000-0000-0000-0000-000000000002',
       '09312345678907', 'gtin', u.id, NULL,
       daterange('2026-01-01', NULL)
  FROM unit u WHERE u.code = 'ea';

INSERT INTO item_barcode (tenant_id, item_id, barcode, scheme, unit_id, quantity, effective)
SELECT '11111111-1111-1111-1111-111111111111',
       '17e10000-0000-0000-0000-000000000001',
       'GLV-M-100', 'internal', u.id, 100,
       daterange('2026-01-01', NULL)
  FROM unit u WHERE u.code = 'ea';

INSERT INTO item_barcode (tenant_id, item_id, barcode, scheme, unit_id, quantity, effective)
SELECT '11111111-1111-1111-1111-111111111111',
       '17e10000-0000-0000-0000-000000000001',
       'LEGACY-9', 'internal', u.id, 1,
       daterange('2024-01-01', '2026-01-01')
  FROM unit u WHERE u.code = 'ea';

DO $$
DECLARE n int;
BEGIN
    SELECT count(*) INTO n FROM item_barcode
     WHERE effective @> CURRENT_DATE;
    IF n <> 2 THEN
        RAISE EXCEPTION 'expected 2 live barcode bindings and a closed one, got % live', n;
    END IF;
END
$$;

SELECT projection_run_all('22222222-2222-2222-2222-222222222222');

COMMIT;
