-- Migration 36: a reference to another tenant's row stops being writable.
--
-- D79, settling question 155, raised by D78 after two migrations running found a
-- table with `PRIMARY KEY (id)` alone when a composite tenant foreign key wanted
-- a target -- `discrepancy`, then `observation`.
--
-- The audit that followed the second one found the convention genuinely mixed:
-- **114 foreign keys between tenant-scoped tables carried no tenant column, and
-- 69 did.** Worse, the justification written into migration 34 for following the
-- weaker form -- "tenant agreement on an item reference is J20's job" -- turned
-- out to be far more generous than J20 is. **J20 checks two joins**,
-- `stock_movement.item_id` and `package_event.package_id`, not the class.
--
-- RLS is not the answer either. It filters what a tenant can *read*; it does not
-- stop a write from naming another tenant's row, and D55 found exactly that hole
-- open since migration 1 and closed it by hand.

-- ---------------------------------------------------------------------------
-- 1. The rule, and why the convention was only half wrong
-- ---------------------------------------------------------------------------
--
-- The split is not oversight against diligence. It is two genuinely different
-- kinds of table, and the audit only became actionable once they were separated:
--
--   **Strictly owned** -- `tenant_id NOT NULL`. Every row belongs to exactly one
--   tenant. A reference from another tenant is a bug with no legitimate reading,
--   so the foreign key must carry the tenant and refuse it at the write.
--
--   **Shared reference** -- `tenant_id` nullable, the shape D55 gave a read and
--   write policy pair. A platform row has no tenant, so a composite foreign key
--   to one is *impossible*: the child's tenant is real and the parent's is NULL.
--   `item` is one of these, which is why twelve tables reference it by id alone
--   and why that was right rather than lazy.
--
-- So: **every foreign key to a strictly-owned table carries `tenant_id`.** For
-- shared-reference targets the property is different -- the row is the tenant's
-- own or the platform's -- and it cannot be a key, so J64 checks it as data.
--
-- Two constraints are exempt and named, because both are already
-- tenant-determined through a column another invariant requires:
--
--   `location_zone_fk`        -- (zone_id, site_id), and S37 requires that form.
--   `intention_amendment_line_fk` -- (order_line_id, order_id), required by S41.
--
-- The body below is mechanical and was generated from the catalogue rather than
-- typed, which is the only honest way to touch sixty-four constraints.

-- Keys first: a composite foreign key needs a composite target.
ALTER TABLE "fulfilment" ADD CONSTRAINT fulfilment_tenant_key UNIQUE (id, tenant_id);
ALTER TABLE "fulfilment_line" ADD CONSTRAINT fulfilment_line_tenant_key UNIQUE (id, tenant_id);
ALTER TABLE "location" ADD CONSTRAINT location_tenant_key UNIQUE (id, tenant_id);
ALTER TABLE "order" ADD CONSTRAINT order_tenant_key UNIQUE (id, tenant_id);
ALTER TABLE "order_line" ADD CONSTRAINT order_line_tenant_key UNIQUE (id, tenant_id);
ALTER TABLE "package_event" ADD CONSTRAINT package_event_tenant_key UNIQUE (id, tenant_id);
ALTER TABLE "stock" ADD CONSTRAINT stock_tenant_key UNIQUE (id, tenant_id);
ALTER TABLE "stock_movement" ADD CONSTRAINT stock_movement_tenant_key UNIQUE (id, tenant_id);
ALTER TABLE "zone" ADD CONSTRAINT zone_tenant_key UNIQUE (id, tenant_id);

-- 70 foreign keys, each rewritten to carry the tenant.

ALTER TABLE "assertion" DROP CONSTRAINT assertion_correction_of_assertion_id_fkey;
ALTER TABLE "assertion" ADD CONSTRAINT assertion_correction_of_assertion_id_fkey
    FOREIGN KEY (correction_of_assertion_id, tenant_id) REFERENCES "assertion" (id, tenant_id);
ALTER TABLE "assertion" DROP CONSTRAINT assertion_supersedes_assertion_id_fkey;
ALTER TABLE "assertion" ADD CONSTRAINT assertion_supersedes_assertion_id_fkey
    FOREIGN KEY (supersedes_assertion_id, tenant_id) REFERENCES "assertion" (id, tenant_id);

ALTER TABLE "discrepancy" DROP CONSTRAINT discrepancy_holder_location_id_fkey;
ALTER TABLE "discrepancy" ADD CONSTRAINT discrepancy_holder_location_id_fkey
    FOREIGN KEY (holder_location_id, tenant_id) REFERENCES "location" (id, tenant_id);
ALTER TABLE "discrepancy" DROP CONSTRAINT discrepancy_lot_id_fkey;
ALTER TABLE "discrepancy" ADD CONSTRAINT discrepancy_lot_id_fkey
    FOREIGN KEY (lot_id, tenant_id) REFERENCES "lot" (id, tenant_id);
ALTER TABLE "discrepancy" DROP CONSTRAINT discrepancy_holder_package_id_fkey;
ALTER TABLE "discrepancy" ADD CONSTRAINT discrepancy_holder_package_id_fkey
    FOREIGN KEY (holder_package_id, tenant_id) REFERENCES "package" (id, tenant_id);
ALTER TABLE "discrepancy" DROP CONSTRAINT discrepancy_package_event_id_fkey;
ALTER TABLE "discrepancy" ADD CONSTRAINT discrepancy_package_event_id_fkey
    FOREIGN KEY (package_event_id, tenant_id) REFERENCES "package_event" (id, tenant_id);
ALTER TABLE "discrepancy" DROP CONSTRAINT discrepancy_counterparty_party_id_fkey;
ALTER TABLE "discrepancy" ADD CONSTRAINT discrepancy_counterparty_party_id_fkey
    FOREIGN KEY (counterparty_party_id, tenant_id) REFERENCES "party" (id, tenant_id);
ALTER TABLE "discrepancy" DROP CONSTRAINT discrepancy_owner_id_fkey;
ALTER TABLE "discrepancy" ADD CONSTRAINT discrepancy_owner_id_fkey
    FOREIGN KEY (owner_id, tenant_id) REFERENCES "party" (id, tenant_id);
ALTER TABLE "discrepancy" DROP CONSTRAINT discrepancy_resolving_movement_id_fkey;
ALTER TABLE "discrepancy" ADD CONSTRAINT discrepancy_resolving_movement_id_fkey
    FOREIGN KEY (resolving_movement_id, tenant_id) REFERENCES "stock_movement" (id, tenant_id);
ALTER TABLE "discrepancy" DROP CONSTRAINT discrepancy_stock_movement_id_fkey;
ALTER TABLE "discrepancy" ADD CONSTRAINT discrepancy_stock_movement_id_fkey
    FOREIGN KEY (stock_movement_id, tenant_id) REFERENCES "stock_movement" (id, tenant_id);

ALTER TABLE "expected_supply" DROP CONSTRAINT expected_supply_owner_id_fkey;
ALTER TABLE "expected_supply" ADD CONSTRAINT expected_supply_owner_id_fkey
    FOREIGN KEY (owner_id, tenant_id) REFERENCES "party" (id, tenant_id);

ALTER TABLE "fulfilment" DROP CONSTRAINT fulfilment_order_id_fkey;
ALTER TABLE "fulfilment" ADD CONSTRAINT fulfilment_order_id_fkey
    FOREIGN KEY (order_id, tenant_id) REFERENCES "order" (id, tenant_id);

ALTER TABLE "fulfilment_line" DROP CONSTRAINT fulfilment_line_fulfilment_id_fkey;
ALTER TABLE "fulfilment_line" ADD CONSTRAINT fulfilment_line_fulfilment_id_fkey
    FOREIGN KEY (fulfilment_id, tenant_id) REFERENCES "fulfilment" (id, tenant_id);
ALTER TABLE "fulfilment_line" DROP CONSTRAINT fulfilment_line_order_line_id_fkey;
ALTER TABLE "fulfilment_line" ADD CONSTRAINT fulfilment_line_order_line_id_fkey
    FOREIGN KEY (order_line_id, tenant_id) REFERENCES "order_line" (id, tenant_id);

ALTER TABLE "intention_amendment" DROP CONSTRAINT intention_amendment_order_id_fkey;
ALTER TABLE "intention_amendment" ADD CONSTRAINT intention_amendment_order_id_fkey
    FOREIGN KEY (order_id, tenant_id) REFERENCES "order" (id, tenant_id);

ALTER TABLE "item_class" DROP CONSTRAINT item_class_parent_fk;
ALTER TABLE "item_class" ADD CONSTRAINT item_class_parent_fk
    FOREIGN KEY (parent_id, tenant_id) REFERENCES "item_class" (id, tenant_id);

ALTER TABLE "observable" DROP CONSTRAINT observable_item_packing_config_id_fkey;
ALTER TABLE "observable" ADD CONSTRAINT observable_item_packing_config_id_fkey
    FOREIGN KEY (item_packing_config_id, tenant_id) REFERENCES "item_packing_config" (id, tenant_id);
ALTER TABLE "observable" DROP CONSTRAINT observable_location_id_fkey;
ALTER TABLE "observable" ADD CONSTRAINT observable_location_id_fkey
    FOREIGN KEY (location_id, tenant_id) REFERENCES "location" (id, tenant_id);
ALTER TABLE "observable" DROP CONSTRAINT observable_lot_id_fkey;
ALTER TABLE "observable" ADD CONSTRAINT observable_lot_id_fkey
    FOREIGN KEY (lot_id, tenant_id) REFERENCES "lot" (id, tenant_id);
ALTER TABLE "observable" DROP CONSTRAINT observable_package_id_fkey;
ALTER TABLE "observable" ADD CONSTRAINT observable_package_id_fkey
    FOREIGN KEY (package_id, tenant_id) REFERENCES "package" (id, tenant_id);

ALTER TABLE "observation" DROP CONSTRAINT observation_corrects_observation_id_fkey;
ALTER TABLE "observation" ADD CONSTRAINT observation_corrects_observation_id_fkey
    FOREIGN KEY (corrects_observation_id, tenant_id) REFERENCES "observation" (id, tenant_id);
ALTER TABLE "observation" DROP CONSTRAINT observation_retracts_observation_id_fkey;
ALTER TABLE "observation" ADD CONSTRAINT observation_retracts_observation_id_fkey
    FOREIGN KEY (retracts_observation_id, tenant_id) REFERENCES "observation" (id, tenant_id);

ALTER TABLE "observation_event" DROP CONSTRAINT observation_event_observable_id_fkey;
ALTER TABLE "observation_event" ADD CONSTRAINT observation_event_observable_id_fkey
    FOREIGN KEY (observable_id, tenant_id) REFERENCES "observable" (id, tenant_id);
ALTER TABLE "observation_event" DROP CONSTRAINT observation_event_derived_from_event_id_fkey;
ALTER TABLE "observation_event" ADD CONSTRAINT observation_event_derived_from_event_id_fkey
    FOREIGN KEY (derived_from_event_id, tenant_id) REFERENCES "observation_event" (id, tenant_id);
ALTER TABLE "observation_event" DROP CONSTRAINT observation_event_asserted_by_party_id_fkey;
ALTER TABLE "observation_event" ADD CONSTRAINT observation_event_asserted_by_party_id_fkey
    FOREIGN KEY (asserted_by_party_id, tenant_id) REFERENCES "party" (id, tenant_id);

ALTER TABLE "order" DROP CONSTRAINT order_supersedes_order_id_fkey;
ALTER TABLE "order" ADD CONSTRAINT order_supersedes_order_id_fkey
    FOREIGN KEY (supersedes_order_id, tenant_id) REFERENCES "order" (id, tenant_id);
ALTER TABLE "order" DROP CONSTRAINT order_customer_party_id_fkey;
ALTER TABLE "order" ADD CONSTRAINT order_customer_party_id_fkey
    FOREIGN KEY (customer_party_id, tenant_id) REFERENCES "party" (id, tenant_id);

ALTER TABLE "order_line" DROP CONSTRAINT order_line_order_id_fkey;
ALTER TABLE "order_line" ADD CONSTRAINT order_line_order_id_fkey
    FOREIGN KEY (order_id, tenant_id) REFERENCES "order" (id, tenant_id);

ALTER TABLE "package" DROP CONSTRAINT package_fulfilment_id_fkey;
ALTER TABLE "package" ADD CONSTRAINT package_fulfilment_id_fkey
    FOREIGN KEY (fulfilment_id, tenant_id) REFERENCES "fulfilment" (id, tenant_id);
ALTER TABLE "package" DROP CONSTRAINT package_location_id_fkey;
ALTER TABLE "package" ADD CONSTRAINT package_location_id_fkey
    FOREIGN KEY (location_id, tenant_id) REFERENCES "location" (id, tenant_id);
ALTER TABLE "package" DROP CONSTRAINT package_resolved_location_id_fkey;
ALTER TABLE "package" ADD CONSTRAINT package_resolved_location_id_fkey
    FOREIGN KEY (resolved_location_id, tenant_id) REFERENCES "location" (id, tenant_id);
ALTER TABLE "package" DROP CONSTRAINT package_parent_package_id_fkey;
ALTER TABLE "package" ADD CONSTRAINT package_parent_package_id_fkey
    FOREIGN KEY (parent_package_id, tenant_id) REFERENCES "package" (id, tenant_id);
ALTER TABLE "package" DROP CONSTRAINT package_placement_event_id_fkey;
ALTER TABLE "package" ADD CONSTRAINT package_placement_event_id_fkey
    FOREIGN KEY (placement_event_id, tenant_id) REFERENCES "package_event" (id, tenant_id);

ALTER TABLE "package_containment" DROP CONSTRAINT package_containment_location_id_fkey;
ALTER TABLE "package_containment" ADD CONSTRAINT package_containment_location_id_fkey
    FOREIGN KEY (location_id, tenant_id) REFERENCES "location" (id, tenant_id);
ALTER TABLE "package_containment" DROP CONSTRAINT package_containment_package_id_fkey;
ALTER TABLE "package_containment" ADD CONSTRAINT package_containment_package_id_fkey
    FOREIGN KEY (package_id, tenant_id) REFERENCES "package" (id, tenant_id);
ALTER TABLE "package_containment" DROP CONSTRAINT package_containment_parent_package_id_fkey;
ALTER TABLE "package_containment" ADD CONSTRAINT package_containment_parent_package_id_fkey
    FOREIGN KEY (parent_package_id, tenant_id) REFERENCES "package" (id, tenant_id);
ALTER TABLE "package_containment" DROP CONSTRAINT package_containment_source_event_id_fkey;
ALTER TABLE "package_containment" ADD CONSTRAINT package_containment_source_event_id_fkey
    FOREIGN KEY (source_event_id, tenant_id) REFERENCES "package_event" (id, tenant_id);

ALTER TABLE "package_event" DROP CONSTRAINT package_event_location_id_fkey;
ALTER TABLE "package_event" ADD CONSTRAINT package_event_location_id_fkey
    FOREIGN KEY (location_id, tenant_id) REFERENCES "location" (id, tenant_id);
ALTER TABLE "package_event" DROP CONSTRAINT package_event_package_id_fkey;
ALTER TABLE "package_event" ADD CONSTRAINT package_event_package_id_fkey
    FOREIGN KEY (package_id, tenant_id) REFERENCES "package" (id, tenant_id);
ALTER TABLE "package_event" DROP CONSTRAINT package_event_parent_package_id_fkey;
ALTER TABLE "package_event" ADD CONSTRAINT package_event_parent_package_id_fkey
    FOREIGN KEY (parent_package_id, tenant_id) REFERENCES "package" (id, tenant_id);

ALTER TABLE "party_class" DROP CONSTRAINT party_class_parent_id_fkey;
ALTER TABLE "party_class" ADD CONSTRAINT party_class_parent_id_fkey
    FOREIGN KEY (parent_id, tenant_id) REFERENCES "party_class" (id, tenant_id);

ALTER TABLE "party_message" DROP CONSTRAINT party_message_acknowledges_party_message_id_fkey;
ALTER TABLE "party_message" ADD CONSTRAINT party_message_acknowledges_party_message_id_fkey
    FOREIGN KEY (acknowledges_party_message_id, tenant_id) REFERENCES "party_message" (id, tenant_id);

ALTER TABLE "policy_binding" DROP CONSTRAINT policy_binding_owner_party_id_fkey;
ALTER TABLE "policy_binding" ADD CONSTRAINT policy_binding_owner_party_id_fkey
    FOREIGN KEY (owner_party_id, tenant_id) REFERENCES "party" (id, tenant_id);
ALTER TABLE "policy_binding" DROP CONSTRAINT policy_binding_party_id_fkey;
ALTER TABLE "policy_binding" ADD CONSTRAINT policy_binding_party_id_fkey
    FOREIGN KEY (party_id, tenant_id) REFERENCES "party" (id, tenant_id);
ALTER TABLE "policy_binding" DROP CONSTRAINT policy_binding_site_id_fkey;
ALTER TABLE "policy_binding" ADD CONSTRAINT policy_binding_site_id_fkey
    FOREIGN KEY (site_id, tenant_id) REFERENCES "site" (id, tenant_id);
ALTER TABLE "policy_binding" DROP CONSTRAINT policy_binding_zone_id_fkey;
ALTER TABLE "policy_binding" ADD CONSTRAINT policy_binding_zone_id_fkey
    FOREIGN KEY (zone_id, tenant_id) REFERENCES "zone" (id, tenant_id);

ALTER TABLE "policy_change" DROP CONSTRAINT policy_change_item_class_id_fkey;
ALTER TABLE "policy_change" ADD CONSTRAINT policy_change_item_class_id_fkey
    FOREIGN KEY (item_class_id, tenant_id) REFERENCES "item_class" (id, tenant_id);
ALTER TABLE "policy_change" DROP CONSTRAINT policy_change_new_item_parent_id_fkey;
ALTER TABLE "policy_change" ADD CONSTRAINT policy_change_new_item_parent_id_fkey
    FOREIGN KEY (new_item_parent_id, tenant_id) REFERENCES "item_class" (id, tenant_id);
ALTER TABLE "policy_change" DROP CONSTRAINT policy_change_new_party_parent_id_fkey;
ALTER TABLE "policy_change" ADD CONSTRAINT policy_change_new_party_parent_id_fkey
    FOREIGN KEY (new_party_parent_id, tenant_id) REFERENCES "party_class" (id, tenant_id);
ALTER TABLE "policy_change" DROP CONSTRAINT policy_change_party_class_id_fkey;
ALTER TABLE "policy_change" ADD CONSTRAINT policy_change_party_class_id_fkey
    FOREIGN KEY (party_class_id, tenant_id) REFERENCES "party_class" (id, tenant_id);

ALTER TABLE "purchase_order" DROP CONSTRAINT purchase_order_supplier_party_id_fkey;
ALTER TABLE "purchase_order" ADD CONSTRAINT purchase_order_supplier_party_id_fkey
    FOREIGN KEY (supplier_party_id, tenant_id) REFERENCES "party" (id, tenant_id);

ALTER TABLE "purchase_order_line" DROP CONSTRAINT purchase_order_line_owner_party_id_fkey;
ALTER TABLE "purchase_order_line" ADD CONSTRAINT purchase_order_line_owner_party_id_fkey
    FOREIGN KEY (owner_party_id, tenant_id) REFERENCES "party" (id, tenant_id);

ALTER TABLE "stock" DROP CONSTRAINT stock_holder_location_id_fkey;
ALTER TABLE "stock" ADD CONSTRAINT stock_holder_location_id_fkey
    FOREIGN KEY (holder_location_id, tenant_id) REFERENCES "location" (id, tenant_id);
ALTER TABLE "stock" DROP CONSTRAINT stock_resolved_location_id_fkey;
ALTER TABLE "stock" ADD CONSTRAINT stock_resolved_location_id_fkey
    FOREIGN KEY (resolved_location_id, tenant_id) REFERENCES "location" (id, tenant_id);
ALTER TABLE "stock" DROP CONSTRAINT stock_lot_id_fkey;
ALTER TABLE "stock" ADD CONSTRAINT stock_lot_id_fkey
    FOREIGN KEY (lot_id, tenant_id) REFERENCES "lot" (id, tenant_id);
ALTER TABLE "stock" DROP CONSTRAINT stock_holder_package_id_fkey;
ALTER TABLE "stock" ADD CONSTRAINT stock_holder_package_id_fkey
    FOREIGN KEY (holder_package_id, tenant_id) REFERENCES "package" (id, tenant_id);
ALTER TABLE "stock" DROP CONSTRAINT stock_owner_id_fkey;
ALTER TABLE "stock" ADD CONSTRAINT stock_owner_id_fkey
    FOREIGN KEY (owner_id, tenant_id) REFERENCES "party" (id, tenant_id);

ALTER TABLE "stock_allocation" DROP CONSTRAINT stock_allocation_expected_supply_id_fkey;
ALTER TABLE "stock_allocation" ADD CONSTRAINT stock_allocation_expected_supply_id_fkey
    FOREIGN KEY (expected_supply_id, tenant_id) REFERENCES "expected_supply" (id, tenant_id) ON DELETE RESTRICT;
ALTER TABLE "stock_allocation" DROP CONSTRAINT stock_allocation_origin_expected_supply_id_fkey;
ALTER TABLE "stock_allocation" ADD CONSTRAINT stock_allocation_origin_expected_supply_id_fkey
    FOREIGN KEY (origin_expected_supply_id, tenant_id) REFERENCES "expected_supply" (id, tenant_id);
ALTER TABLE "stock_allocation" DROP CONSTRAINT stock_allocation_fulfilment_line_id_fkey;
ALTER TABLE "stock_allocation" ADD CONSTRAINT stock_allocation_fulfilment_line_id_fkey
    FOREIGN KEY (fulfilment_line_id, tenant_id) REFERENCES "fulfilment_line" (id, tenant_id);
ALTER TABLE "stock_allocation" DROP CONSTRAINT stock_allocation_stock_id_fkey;
ALTER TABLE "stock_allocation" ADD CONSTRAINT stock_allocation_stock_id_fkey
    FOREIGN KEY (stock_id, tenant_id) REFERENCES "stock" (id, tenant_id) ON DELETE RESTRICT;

ALTER TABLE "stock_movement" DROP CONSTRAINT stock_movement_from_location_id_fkey;
ALTER TABLE "stock_movement" ADD CONSTRAINT stock_movement_from_location_id_fkey
    FOREIGN KEY (from_location_id, tenant_id) REFERENCES "location" (id, tenant_id);
ALTER TABLE "stock_movement" DROP CONSTRAINT stock_movement_to_location_id_fkey;
ALTER TABLE "stock_movement" ADD CONSTRAINT stock_movement_to_location_id_fkey
    FOREIGN KEY (to_location_id, tenant_id) REFERENCES "location" (id, tenant_id);
ALTER TABLE "stock_movement" DROP CONSTRAINT stock_movement_from_lot_id_fkey;
ALTER TABLE "stock_movement" ADD CONSTRAINT stock_movement_from_lot_id_fkey
    FOREIGN KEY (from_lot_id, tenant_id) REFERENCES "lot" (id, tenant_id);
ALTER TABLE "stock_movement" DROP CONSTRAINT stock_movement_to_lot_id_fkey;
ALTER TABLE "stock_movement" ADD CONSTRAINT stock_movement_to_lot_id_fkey
    FOREIGN KEY (to_lot_id, tenant_id) REFERENCES "lot" (id, tenant_id);
ALTER TABLE "stock_movement" DROP CONSTRAINT stock_movement_from_package_id_fkey;
ALTER TABLE "stock_movement" ADD CONSTRAINT stock_movement_from_package_id_fkey
    FOREIGN KEY (from_package_id, tenant_id) REFERENCES "package" (id, tenant_id);
ALTER TABLE "stock_movement" DROP CONSTRAINT stock_movement_to_package_id_fkey;
ALTER TABLE "stock_movement" ADD CONSTRAINT stock_movement_to_package_id_fkey
    FOREIGN KEY (to_package_id, tenant_id) REFERENCES "package" (id, tenant_id);
ALTER TABLE "stock_movement" DROP CONSTRAINT stock_movement_from_owner_id_fkey;
ALTER TABLE "stock_movement" ADD CONSTRAINT stock_movement_from_owner_id_fkey
    FOREIGN KEY (from_owner_id, tenant_id) REFERENCES "party" (id, tenant_id);
ALTER TABLE "stock_movement" DROP CONSTRAINT stock_movement_to_owner_id_fkey;
ALTER TABLE "stock_movement" ADD CONSTRAINT stock_movement_to_owner_id_fkey
    FOREIGN KEY (to_owner_id, tenant_id) REFERENCES "party" (id, tenant_id);
ALTER TABLE "stock_movement" DROP CONSTRAINT stock_movement_reverses_movement_id_fkey;
ALTER TABLE "stock_movement" ADD CONSTRAINT stock_movement_reverses_movement_id_fkey
    FOREIGN KEY (reverses_movement_id, tenant_id) REFERENCES "stock_movement" (id, tenant_id);

-- ---------------------------------------------------------------------------
-- 2. The table that was outside the boundary altogether
-- ---------------------------------------------------------------------------
--
-- S51 was written before this and found it on the first run: `consignment` is
-- referenced and strictly owned but offered no key to reference it by. The reason
-- turned out to be worse than a missing key.
--
-- **`consignment_package` had no `tenant_id`, and no row-level security.** Two
-- columns, both foreign keys, linking a tenant's packages to a tenant's
-- consignments -- and nothing said they had to be the *same* tenant, nothing
-- filtered reads, and the application held INSERT, UPDATE and DELETE on all of
-- it. It is the D55 class exactly: *a policy with no `WITH CHECK` authorises
-- writing what it meant to allow reading*, one step further along, with no policy
-- at all.
--
-- It is the only table of its shape. The others without `tenant_id` are global
-- reference (`unit`, `dimension`, `person`, `tenant`), policy value tables that
-- reach tenancy through `policy_binding`, or the projection registries.

ALTER TABLE consignment_package ADD COLUMN tenant_id uuid;

UPDATE consignment_package cp
   SET tenant_id = c.tenant_id
  FROM consignment c
 WHERE c.id = cp.consignment_id;

ALTER TABLE consignment_package
    ALTER COLUMN tenant_id SET NOT NULL,
    ADD CONSTRAINT consignment_package_tenant_fk FOREIGN KEY (tenant_id) REFERENCES tenant(id);

ALTER TABLE consignment_package
    DROP CONSTRAINT consignment_package_consignment_id_fkey,
    DROP CONSTRAINT consignment_package_package_id_fkey;

ALTER TABLE consignment
    ADD CONSTRAINT consignment_tenant_key UNIQUE (id, tenant_id);

ALTER TABLE consignment_package
    ADD CONSTRAINT consignment_package_consignment_id_fkey
        FOREIGN KEY (consignment_id, tenant_id) REFERENCES consignment (id, tenant_id),
    ADD CONSTRAINT consignment_package_package_id_fkey
        FOREIGN KEY (package_id, tenant_id) REFERENCES package (id, tenant_id);

ALTER TABLE consignment_package ENABLE ROW LEVEL SECURITY;
ALTER TABLE consignment_package FORCE ROW LEVEL SECURITY;
CREATE POLICY consignment_package_tenant_scoped ON consignment_package
    USING (tenant_id = current_tenant());

COMMENT ON COLUMN consignment_package.tenant_id IS
    'Added by D79. Without it this table linked two tenant-scoped things across '
    'the boundary and no policy filtered it. D55, D79.';
