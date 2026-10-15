-- Migration 143 down: a closed order's plan is kept again, unseen.

DROP FUNCTION pack_plan_drop_closed();
REVOKE SELECT (id, state, closed_elsewhere) ON fulfilment FROM spork_projection_owner;
REVOKE SELECT, DELETE ON pack_plan_parcel, pack_plan_line FROM spork_projection_owner;
