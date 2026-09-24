-- Migration 32: retiring a class governs the editor, not the resolver.
--
-- D74, settling question 149, which D72 raised by closing the door it names:
--
--   "D72 takes DELETE on `item_class` and `party_class` away from the
--    application, because the row carries a projection column and deleting it
--    takes the projection with it -- J36's rule. The capability was already
--    nearly gone (`policy_change` holds a foreign key, so any class with history
--    was undeletable), but 'nearly' is not a design: a class created by mistake
--    now has no exit at all. An `active` flag is the obvious answer and the
--    obvious answer is what needs the thought."

-- ---------------------------------------------------------------------------
-- 1. The acts already had names
-- ---------------------------------------------------------------------------
--
-- `policy_change_kind` has held `retired` and `reinstated` since migration 8,
-- where they describe a policy *version* leaving and returning to force. A class
-- leaves and returns the same way, so this adds no enum values and invents no
-- vocabulary -- it points the two that exist at a second subject, exactly as D72
-- pointed `reparented` at one.
--
-- Nothing else about `policy_change` changes. D72's subject CHECK already permits
-- a taxonomy arm with no binding and no `kind`, so a retirement is a legal row
-- today; what was missing was somewhere for it to land.

-- ---------------------------------------------------------------------------
-- 2. What retirement means, which is the whole question
-- ---------------------------------------------------------------------------
--
-- **Retirement does not change resolution.** A retired class keeps its closure
-- rows, its bindings keep matching, and every member still classified into it
-- keeps the rules it had this morning.
--
-- That is the decision, and the alternative is the one that looks tidier and is
-- indefensible. If retiring a class withdrew it from the closure, then every item
-- underneath it would silently lose the policies scoped to it -- **the exact
-- failure D72 and D73 exist to prevent**, arriving through a door marked
-- housekeeping. A taxonomy edit that changes which stock ships must be a decision
-- with a blast radius in front of it, and "retire" would be one that changed
-- everything while displaying nothing.
--
-- So retirement is a statement to the *editor*: do not offer this class for new
-- classifications, do not scope new bindings to it, do not show it in the picker.
-- It says nothing to the resolver.
--
-- Two consequences worth stating rather than discovering:
--
--   * **A retirement has no blast radius**, and that is now a claim rather than an
--     omission. D73's CHECK ties `affected_binding_count` to `reparented` rows,
--     which reads as an accident until this migration; it is correct, because the
--     number a retirement would carry is zero by construction.
--
--   * **Retiring is not deleting, and there is still no delete.** A class created
--     by mistake is retired with a reason, and the row stays. That is this
--     record's position on every other fact it holds -- D51 does not delete an
--     order line, it amends it -- and a taxonomy node that something was once
--     classified into is a fact about the past whether or not it was a mistake.

ALTER TABLE item_class  ADD COLUMN retired_at timestamptz;
ALTER TABLE party_class ADD COLUMN retired_at timestamptz;

COMMENT ON COLUMN item_class.retired_at IS
    '@projection -- folded by projection_taxonomy_rebuild from retired and '
    'reinstated acts. When the class stopped being offered for new work. It does '
    'not affect resolution: members and bindings underneath it are unchanged, '
    'which is the point. D74.';
COMMENT ON COLUMN party_class.retired_at IS
    '@projection -- folded by projection_taxonomy_rebuild from retired and '
    'reinstated acts. See item_class.retired_at. D74.';

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('item_class',  'retired_at', 'projection_taxonomy_rebuild'),
    ('party_class', 'retired_at', 'projection_taxonomy_rebuild');

-- ---------------------------------------------------------------------------
-- 3. The fold gains a second column
-- ---------------------------------------------------------------------------
--
-- Same register order as D72's parentage fold and D24's amendments: last writer
-- per class by (occurred_at, recorded_at, id). `reinstated` folds to NULL, so the
-- pair is symmetric and a class can come back without a special case.
--
-- Four statements rather than two joined ones, each carrying its own D68 guard.
-- A class touched on both columns is counted twice, which is what "rows touched"
-- means -- and a run that changes nothing still writes nothing, which is the
-- property that matters.

CREATE OR REPLACE FUNCTION projection_taxonomy_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
    n bigint;
    total bigint := 0;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    -- Last writer per class, in D24's register order, exactly as J46 folds an
    -- amendment. Verbatim rather than coalesced: a reparented row always states
    -- the answer, and NULL is a real answer meaning the root.
    WITH folded AS (
        SELECT DISTINCT ON (item_class_id)
               item_class_id, new_item_parent_id AS new_parent_id
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind = 'reparented'
           AND item_class_id IS NOT NULL
         ORDER BY item_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE item_class c
           SET parent_id = f.new_parent_id
          FROM folded f
         WHERE c.id = f.item_class_id AND c.tenant_id = p_tenant
           AND c.parent_id IS DISTINCT FROM f.new_parent_id
        RETURNING c.id)
    SELECT count(*) INTO touched FROM updated;
    total := total + touched;

    WITH folded AS (
        SELECT DISTINCT ON (party_class_id)
               party_class_id, new_party_parent_id AS new_parent_id
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind = 'reparented'
           AND party_class_id IS NOT NULL
         ORDER BY party_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE party_class c
           SET parent_id = f.new_parent_id
          FROM folded f
         WHERE c.id = f.party_class_id AND c.tenant_id = p_tenant
           AND c.parent_id IS DISTINCT FROM f.new_parent_id
        RETURNING c.id)
    SELECT count(*) INTO n FROM updated;
    total := total + n;

    -- D74. Retirement and reinstatement, folded as one alternating history so the
    -- latest act wins whichever it is.
    WITH folded AS (
        SELECT DISTINCT ON (item_class_id)
               item_class_id,
               CASE WHEN change_kind = 'retired' THEN occurred_at END AS retired_at
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind IN ('retired', 'reinstated')
           AND item_class_id IS NOT NULL
         ORDER BY item_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE item_class c
           SET retired_at = f.retired_at
          FROM folded f
         WHERE c.id = f.item_class_id AND c.tenant_id = p_tenant
           AND c.retired_at IS DISTINCT FROM f.retired_at
        RETURNING c.id)
    SELECT count(*) INTO n FROM updated;
    total := total + n;

    WITH folded AS (
        SELECT DISTINCT ON (party_class_id)
               party_class_id,
               CASE WHEN change_kind = 'retired' THEN occurred_at END AS retired_at
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind IN ('retired', 'reinstated')
           AND party_class_id IS NOT NULL
         ORDER BY party_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE party_class c
           SET retired_at = f.retired_at
          FROM folded f
         WHERE c.id = f.party_class_id AND c.tenant_id = p_tenant
           AND c.retired_at IS DISTINCT FROM f.retired_at
        RETURNING c.id)
    SELECT count(*) INTO n FROM updated;
    total := total + n;

    RETURN total;
END
$$;

ALTER FUNCTION projection_taxonomy_rebuild(uuid) OWNER TO spork_projection_owner;

-- ---------------------------------------------------------------------------
-- 4. What the editor must show before it does this
-- ---------------------------------------------------------------------------
--
-- D73's lesson applied a second time: derive the number so the screen cannot get
-- it wrong. Retiring is not resolution-affecting, so the question is not "what
-- breaks" but "is anyone still using this" -- and that is three counts, not one,
-- because they mean different things to whoever is deciding.
--
-- Members and bindings are counted over the **subtree**, because retiring a
-- parent is a statement about a branch. Children are counted directly, because a
-- live child under a retired parent is the specific incoherence J61 reports.

CREATE FUNCTION item_class_retire_impact(p_class_id uuid)
    RETURNS TABLE (members bigint, bindings bigint, live_children bigint)
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    SELECT
        (SELECT count(*) FROM item_classification ic
           JOIN item_class_closure cc ON cc.descendant_id = ic.item_class_id
          WHERE cc.ancestor_id = p_class_id),
        (SELECT count(*) FROM policy_binding b
           JOIN item_class_closure cc ON cc.descendant_id = b.item_class_id
          WHERE cc.ancestor_id = p_class_id
            AND NOT EXISTS (SELECT 1 FROM policy_binding s WHERE s.supersedes_id = b.id)),
        (SELECT count(*) FROM item_class ch
          WHERE ch.parent_id = p_class_id AND ch.retired_at IS NULL)
$$;

CREATE FUNCTION party_class_retire_impact(p_class_id uuid)
    RETURNS TABLE (members bigint, bindings bigint, live_children bigint)
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    SELECT
        (SELECT count(*) FROM party p
           JOIN party_class_closure cc ON cc.descendant_id = p.party_class_id
          WHERE cc.ancestor_id = p_class_id),
        (SELECT count(*) FROM policy_binding b
           JOIN party_class_closure cc ON cc.descendant_id = b.party_class_id
          WHERE cc.ancestor_id = p_class_id
            AND NOT EXISTS (SELECT 1 FROM policy_binding s WHERE s.supersedes_id = b.id)),
        (SELECT count(*) FROM party_class ch
          WHERE ch.parent_id = p_class_id AND ch.retired_at IS NULL)
$$;

COMMENT ON FUNCTION item_class_retire_impact(uuid) IS
    'What is still using this class and the branch under it: members, live '
    'bindings, and children that are not themselves retired. Retiring changes no '
    'resolution, so this is a "does anyone still need it" question rather than a '
    'blast radius. D74.';
COMMENT ON FUNCTION party_class_retire_impact(uuid) IS
    'The same on the Counterparty axis. D74.';

GRANT EXECUTE ON FUNCTION item_class_retire_impact(uuid),
    party_class_retire_impact(uuid)
    TO spork_app, spork_platform, spork_scheduler, spork_projection_owner;

-- ---------------------------------------------------------------------------
-- 5. What is not enforced, and why it cannot be
-- ---------------------------------------------------------------------------
--
-- **"No new members after retirement" is not checkable from the data**, because
-- `item_classification` carries no timestamp -- it is a current-state table with
-- primary key (tenant_id, item_id) and nothing recording when the classification
-- was made. Comparing it to `retired_at` would compare a fact to a moment it does
-- not have.
--
-- A declarative form was looked for and rejected. A foreign key onto a partial
-- unique index over the unretired classes would forbid new members, and would
-- also **forbid retiring a class that still has any** -- which is the ordinary
-- case and the one the editor exists to warn about rather than prevent.
--
-- So J61 checks the two things that are visible: a live child under a retired
-- parent, and a binding created after the class it names was retired
-- (`policy_binding.created_at` against `retired_at`, both of which are real). The
-- third -- classification after retirement -- is invisible until classification
-- becomes an act with a moment, which it is not today and which no question has
-- yet asked for. Naming it here rather than implying coverage.

GRANT SELECT ON policy_change TO spork_projection_owner;
