-- Reverse of 2026-08-04-000048_mediated_writes.
--
-- The mediation goes back to being a convention. `asserted_unit_content_resolve`
-- becomes unrunnable by the app again and `goods_receipt_line_dispose` becomes
-- optional again, which is the state D89 and D90 were both described in and
-- neither was.

DROP TABLE IF EXISTS mediated_write;

-- The app gets the disposition columns back, which is what makes the function
-- bypassable.
GRANT UPDATE (accepted_at, accepted_by_id, rejected_at, rejected_by_id,
              rejected_reason_id, receiving_policy_id)
    ON goods_receipt_line TO nylonite_app;

GRANT EXECUTE ON FUNCTION
    asserted_unit_content_resolve(uuid, uuid, uuid, uuid, text) TO PUBLIC;
GRANT EXECUTE ON FUNCTION
    goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid) TO PUBLIC;

ALTER FUNCTION goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid)
    OWNER TO CURRENT_USER;
ALTER FUNCTION goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid)
    SECURITY INVOKER;

ALTER FUNCTION asserted_unit_content_resolve(uuid, uuid, uuid, uuid, text)
    OWNER TO CURRENT_USER;
ALTER FUNCTION asserted_unit_content_resolve(uuid, uuid, uuid, uuid, text)
    SECURITY INVOKER;

REVOKE ALL ON discrepancy FROM nylonite_mediation_owner;
REVOKE ALL ON asserted_unit_content FROM nylonite_mediation_owner;
REVOKE ALL ON assertion_check FROM nylonite_mediation_owner;
REVOKE ALL ON goods_receipt_line FROM nylonite_mediation_owner;
REVOKE USAGE ON SCHEMA public FROM nylonite_mediation_owner;

-- The role itself is left in place, as migration 1 and 3 leave theirs: a role is
-- cluster-wide and another database on the same cluster may hold objects owned by
-- it, so dropping it here is not this migration's decision to make.
