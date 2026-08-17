-- Migration 69 down: a carton may be listed on two consignments again.

DROP INDEX IF EXISTS consignment_package_one_truck_idx;
