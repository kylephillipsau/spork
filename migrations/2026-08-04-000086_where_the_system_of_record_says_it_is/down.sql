-- Migration 86 down: the shelves become unknown again.
--
-- Nothing else reads this table yet, so reversing costs only what was loaded —
-- and what was loaded is a report that can be exported again. That is the
-- difference between a snapshot and a ledger, and it is why this is a snapshot.

DROP TABLE IF EXISTS reported_stock;
