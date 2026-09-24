-- Migration 48: the mediated write, actually mediated.
--
-- D94, settling question 161.
--
-- D90 named a pattern on its fourth use: *"where a cross-row rule cannot be a
-- constraint, it becomes a grant plus a function."* Take UPDATE away from the
-- application, hand it a function that refuses when the rule says refuse.
--
-- **Both halves were stated and neither was built, in opposite directions.**
--
--   D90  the function has no privilege     -> mediation is impossible
--   D89  the app still has the privilege   -> mediation is optional
--
-- `asserted_unit_content_resolve` is not `SECURITY DEFINER`, so it runs with the
-- caller's rights and the caller is the role we just took the rights from. Called
-- as `spork_app` it fails on its own first statement:
--
--   ERROR:  permission denied for table asserted_unit_content
--   CONTEXT:  SQL statement "SELECT 1 FROM asserted_unit_content
--                            WHERE id = p_content FOR UPDATE"
--
-- before reaching the freeze it exists to enforce. D90 measured its refusals and
-- they were real -- the refusal path raises before touching a row -- so the half
-- that was never exercised is the success path, which is the half D21 insists on.
--
-- `goods_receipt_line_dispose` has the opposite failure. Migration 21 granted the
-- app UPDATE on the disposition columns, so the function is a courtesy: the app
-- can write `accepted_at` directly and D89's *"a change of mind is a correction,
-- not a second disposition"* holds only for callers who choose to go through it.
-- Measured before this migration: the direct UPDATE succeeded.
--
-- D72 and D77 are unaffected. `item_class` grants the app `code` and `name` only,
-- so `parent_id` is protected, and D77 removed UPDATE on the claim entirely.

-- ---------------------------------------------------------------------------
-- 1. An owner for mediated writes, and why it is not the projection owner
-- ---------------------------------------------------------------------------
--
-- `SECURITY DEFINER` alone is the dangerous fix. Owned by `postgres` the function
-- would run as a superuser, and **a superuser bypasses row-level security
-- unconditionally**, so every mediated write would become a tenancy escape --
-- the hole D55 spent a migration closing, reopened through a function the app is
-- handed on purpose.
--
-- The safe shape already exists here: all fifteen `SECURITY DEFINER` projection
-- functions are owned by `spork_projection_owner`, which has neither SUPERUSER
-- nor BYPASSRLS, and every table they touch has FORCE ROW LEVEL SECURITY, which
-- applies RLS to the table owner as well. The owner is inside the fence.
--
-- A separate role rather than reusing the projection owner, because the two
-- concerns are different and the grants should be too: the projection owner may
-- rewrite whole projection columns, and a mediation owner may write exactly the
-- columns its functions govern and nothing else.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'spork_mediation_owner') THEN
        CREATE ROLE spork_mediation_owner NOLOGIN;
    END IF;
END $$;

COMMENT ON ROLE spork_mediation_owner IS
    'Owns the SECURITY DEFINER functions that mediate a write the application is '
    'not allowed to make directly. Never SUPERUSER and never BYPASSRLS: the '
    'functions must stay inside row-level security, or the mediation buys a '
    'tenancy hole. D89, D90, D94.';

GRANT USAGE ON SCHEMA public TO spork_mediation_owner;

-- Exactly the columns the two functions write, and the reads they need to decide.
GRANT SELECT ON asserted_unit_content, assertion_check, goods_receipt_line
    TO spork_mediation_owner;
GRANT SELECT, INSERT ON discrepancy TO spork_mediation_owner;

GRANT UPDATE (resolved_item_id, resolved_purchase_order_line_id,
              resolved_at, resolved_by_id, resolution_method)
    ON asserted_unit_content TO spork_mediation_owner;

GRANT UPDATE (accepted_at, accepted_by_id, rejected_at, rejected_by_id,
              rejected_reason_id, receiving_policy_id)
    ON goods_receipt_line TO spork_mediation_owner;

-- ---------------------------------------------------------------------------
-- 2. The functions become definers, owned by that role
-- ---------------------------------------------------------------------------
--
-- `SET search_path` is already on both, which is the other half of writing a
-- definer safely and was there from the start.

ALTER FUNCTION asserted_unit_content_resolve(uuid, uuid, uuid, uuid, text)
    SECURITY DEFINER;
ALTER FUNCTION asserted_unit_content_resolve(uuid, uuid, uuid, uuid, text)
    OWNER TO spork_mediation_owner;

ALTER FUNCTION goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid)
    SECURITY DEFINER;
ALTER FUNCTION goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid)
    OWNER TO spork_mediation_owner;

-- **J37 caught this on the first run after the ALTERs above**, which is the suite
-- doing precisely what it is for. Postgres grants EXECUTE to PUBLIC by default,
-- which is harmless on an invoker function and is a privilege escalation on a
-- definer: every role in the cluster could call it and write as the owner. The
-- explicit grants to `spork_app` from migrations 43 and 44 survive the revoke.

REVOKE EXECUTE ON FUNCTION
    asserted_unit_content_resolve(uuid, uuid, uuid, uuid, text) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION
    goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid) FROM PUBLIC;

-- ---------------------------------------------------------------------------
-- 3. The grant half, which was the one never taken away
-- ---------------------------------------------------------------------------
--
-- D89's own words, now true: *"Disposition and the late match are the only things
-- that legitimately move afterwards."* The late match still moves by UPDATE --
-- `matched_at`, `matched_by_id`, `expected_supply_id` and the D91 link are not
-- mediated and nothing says they should be. The disposition is.

REVOKE UPDATE (accepted_at, accepted_by_id, rejected_at, rejected_by_id,
               rejected_reason_id, receiving_policy_id)
    ON goods_receipt_line FROM spork_app;

-- ---------------------------------------------------------------------------
-- 4. The registry, so the property is checkable rather than remembered
-- ---------------------------------------------------------------------------
--
-- Same shape and same reason as `projection_rebuild`: a declared side, so a
-- structural check can diff it against the catalogue in both directions. Without
-- it S54 would have to guess which columns a function is responsible for, and
-- guessing is what let both halves of this pattern go unbuilt.

CREATE TABLE mediated_write (
    table_name    text NOT NULL,
    column_name   text NOT NULL,
    function_name text NOT NULL,
    PRIMARY KEY (table_name, column_name)
);

COMMENT ON TABLE mediated_write IS
    'REFERENCE, platform-owned. Every column the application may not UPDATE '
    'directly because a function decides whether the write is allowed. The '
    'declared side of S54s diff: the app must lack UPDATE on each column, the '
    'mediation owner must hold it, and the named function must be a definer owned '
    'by that role. D89, D90, D94.';

INSERT INTO mediated_write (table_name, column_name, function_name) VALUES
    ('asserted_unit_content', 'resolved_item_id',                'asserted_unit_content_resolve'),
    ('asserted_unit_content', 'resolved_purchase_order_line_id', 'asserted_unit_content_resolve'),
    ('asserted_unit_content', 'resolved_at',                     'asserted_unit_content_resolve'),
    ('asserted_unit_content', 'resolved_by_id',                  'asserted_unit_content_resolve'),
    ('asserted_unit_content', 'resolution_method',               'asserted_unit_content_resolve'),
    ('goods_receipt_line',    'accepted_at',                     'goods_receipt_line_dispose'),
    ('goods_receipt_line',    'accepted_by_id',                  'goods_receipt_line_dispose'),
    ('goods_receipt_line',    'rejected_at',                     'goods_receipt_line_dispose'),
    ('goods_receipt_line',    'rejected_by_id',                  'goods_receipt_line_dispose'),
    ('goods_receipt_line',    'rejected_reason_id',              'goods_receipt_line_dispose'),
    ('goods_receipt_line',    'receiving_policy_id',             'goods_receipt_line_dispose');

GRANT SELECT ON mediated_write
    TO spork_app, spork_platform, spork_scheduler, spork_projection_owner;
