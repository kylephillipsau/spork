-- Migration 69: a carton goes on one truck.
--
-- `consignment_package` is keyed on `(consignment_id, package_id)`, which stops
-- the same carton being listed twice on one consignment and permits it being
-- listed on two. D15 makes many packages per consignment the point — *"a
-- consignment reaches its fulfilments through packages rather than a direct
-- foreign key, which is what lets one collection carry parcels from several
-- commitments"* — and says nothing about the other direction, because the other
-- direction is physically impossible. A carton is on one truck.
--
-- **Enforced rather than checked, and the reason is a defect this review already
-- found once.** The application can ask whether a package is already consigned
-- and then insert, and two requests arriving together both see no row and both
-- insert: exactly the check-then-insert race that `ensure_goods_receipt` shipped
-- with, demonstrated against a live database before it was fixed. A predicate
-- the database holds cannot lose that race.
--
-- Not partial and not deferrable: every column is NOT NULL, and there is no
-- moment during a legitimate write when a carton is on two consignments.
--
-- Re-consigning is still expressible. It is a DELETE of the old link and an
-- INSERT of the new, which is the right shape for a grouping table — no fact is
-- destroyed, because the movements and events that record what physically
-- happened are elsewhere and untouched.

CREATE UNIQUE INDEX consignment_package_one_truck_idx
    ON consignment_package (tenant_id, package_id);

COMMENT ON INDEX consignment_package_one_truck_idx IS
    'A carton is collected by one carrier. The table''s primary key allows a '
    'package on two consignments; this refuses it, in the database rather than '
    'in a handler that would race itself. D15, migration 69.';
