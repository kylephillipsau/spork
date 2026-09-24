-- Migration 30: re-parenting is a policy change, and now it says so.
--
-- D72, settling the recordable half of question 78. Its full text, from
-- `mechanism-design.md` rather than from the register:
--
--   "Taxonomy re-parenting silently changes specificity. Moving chilled_dairy
--    under a different parent changes its depth and therefore which bindings win,
--    for every future resolution and for any replay. Past decisions are safe
--    (they hold value-row FKs); 'what should have applied' becomes wrong. The UI
--    must surface 'this move changes N active resolutions' -- but the semantic
--    hazard is real and unsolved."
--
-- Three claims. The first is the one to fix; the second is already true and worth
-- confirming; the third is what remains after this and gets its own question.

-- ---------------------------------------------------------------------------
-- 1. Why this belongs in policy_change and not in a table of its own
-- ---------------------------------------------------------------------------
--
-- D22 built `policy_change` because *"a weight change with no reason is how
-- tuning becomes superstition"*, and gave it a mandatory reason, an actor and a
-- client event. Re-parenting changes which policy applies exactly as surely as
-- revaluing one does, and until now it needed nothing.
--
-- S50 settled the classification a fortnight of decisions early: it asserts that
-- the taxonomy is an input to a resolution, alongside the scope and the value
-- tables. **An input to a resolution changing is a policy change.** One log, one
-- place to answer "why did this resolution move", rather than a second table with
-- the same columns and a different name.
--
-- The arm set is a discriminated union under D23's rule, so `= 1` rather than
-- `<= 1`: a change with no subject is meaningless. The constraint is deliberately
-- not named `%_cause_ck` or `%_demand_ck`, because S3's rule is about grouping
-- tables reaching for a demand they may not have, and this is neither.

-- The value has to exist before a CHECK can name it, and a CHECK naming it is
-- the point: `reparented` is not one more thing that can happen to a policy, it
-- is the thing that makes a subject other than a binding legal at all.
ALTER TYPE policy_change_kind ADD VALUE IF NOT EXISTS 'reparented';

ALTER TABLE policy_change
    ADD COLUMN item_class_id  uuid REFERENCES item_class(id),
    ADD COLUMN party_class_id uuid REFERENCES party_class(id),
    -- One destination column per taxonomy, not one untyped column serving both.
    -- The fixture found this the first time it moved a party class: a single
    -- `new_parent_id` has to point somewhere, and pointing it at one taxonomy
    -- makes the other unrepresentable while pointing it at neither is the
    -- polymorphic pair D10 refused. Typed arms cost a column and buy a foreign
    -- key that means something.
    --
    -- Taken verbatim rather than coalesced, which is the difference between this
    -- fold and D42's. A `reparented` row always states the parentage, and NULL
    -- means the root -- so a COALESCE would make "moved to the root"
    -- inexpressible, which is the bug D51 avoided by making one amendment name
    -- one subject.
    ADD COLUMN new_item_parent_id  uuid REFERENCES item_class(id),
    ADD COLUMN new_party_parent_id uuid REFERENCES party_class(id);

ALTER TABLE policy_change ALTER COLUMN policy_binding_id DROP NOT NULL;
ALTER TABLE policy_change ALTER COLUMN kind DROP NOT NULL;

ALTER TABLE policy_change
    ADD CONSTRAINT policy_change_subject_ck
        CHECK (num_nonnulls(policy_binding_id, item_class_id, party_class_id) = 1),
    -- `kind` is the policy kind, which a taxonomy move does not have: moving a
    -- class moves it for every kind at once.
    ADD CONSTRAINT policy_change_kind_pairs_ck
        CHECK ((kind IS NOT NULL) = (policy_binding_id IS NOT NULL)),
    ADD CONSTRAINT policy_change_reparent_ck
        CHECK (change_kind <> 'reparented'
               OR num_nonnulls(item_class_id, party_class_id) = 1),
    ADD CONSTRAINT policy_change_parent_only_on_reparent_ck
        CHECK ((new_item_parent_id IS NULL AND new_party_parent_id IS NULL)
               OR change_kind = 'reparented'),
    -- Each destination belongs to its own subject. A move of an item class
    -- cannot name a party class as its new home.
    ADD CONSTRAINT policy_change_parent_matches_subject_ck
        CHECK ((new_item_parent_id  IS NULL OR item_class_id  IS NOT NULL)
           AND (new_party_parent_id IS NULL OR party_class_id IS NOT NULL)),
    ADD CONSTRAINT policy_change_not_own_parent_ck
        CHECK ((new_item_parent_id  IS NULL OR new_item_parent_id  <> item_class_id)
           AND (new_party_parent_id IS NULL OR new_party_parent_id <> party_class_id));

CREATE INDEX policy_change_item_class_idx ON policy_change (item_class_id)
    WHERE item_class_id IS NOT NULL;
CREATE INDEX policy_change_party_class_idx ON policy_change (party_class_id)
    WHERE party_class_id IS NOT NULL;

COMMENT ON COLUMN policy_change.new_item_parent_id IS
    'Where the item class moved to. NULL on a reparented row means the root, '
    'which is why the fold takes this verbatim rather than coalescing onto the '
    'row. D72.';
COMMENT ON COLUMN policy_change.new_party_parent_id IS
    'The same, for the counterparty taxonomy. Two typed columns rather than one '
    'untyped: D10''s rule, and the alternative has no foreign key. D72.';

-- ---------------------------------------------------------------------------
-- 2. Parentage becomes a fold, which is what makes the move unsilenceable
-- ---------------------------------------------------------------------------
--
-- Recording the move beside the mutation would leave two representations that
-- agree until somebody updates one, which is the failure this record has found in
-- projection markers, pending reasons and its own counts. So `parent_id` stops
-- being a column the application writes and becomes what D42 made `order`'s
-- covered columns: a fold of the log.
--
-- The application cannot re-parent without writing the fact, because the only
-- verb that moves a class is an INSERT into `policy_change` -- which carries a
-- mandatory reason and an actor by construction. **Not a convention, not a code
-- review item: a grant.**

CREATE FUNCTION projection_taxonomy_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
    n bigint;
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

    RETURN touched + n;
END
$$;

ALTER FUNCTION projection_taxonomy_rebuild(uuid) OWNER TO spork_projection_owner;
REVOKE EXECUTE ON FUNCTION projection_taxonomy_rebuild(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_taxonomy_rebuild(uuid)
    TO spork_scheduler, spork_platform;
GRANT SELECT, UPDATE ON item_class, party_class TO spork_projection_owner;
GRANT SELECT ON policy_change TO spork_projection_owner;

-- Before the closures, which read what this writes. D64 exists so that ordering
-- is a row rather than something the fixture happens to get right.
INSERT INTO projection_step (function_name, ordinal, note) VALUES
    ('projection_taxonomy_rebuild', 5,
     'Folds reparented policy_change rows onto item_class.parent_id and party_class.parent_id. Before the closures at 10 and 20, which read it.');

COMMENT ON COLUMN item_class.parent_id IS
    '@projection of policy_change via projection_taxonomy_rebuild (D22, D72). A move is a fact with a reason, an actor and a moment, because it changes which policy wins for everything underneath.';
COMMENT ON COLUMN party_class.parent_id IS
    '@projection of policy_change via projection_taxonomy_rebuild (D22, D72).';

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('item_class',  'parent_id', 'projection_taxonomy_rebuild'),
    ('party_class', 'parent_id', 'projection_taxonomy_rebuild');

-- The grant that makes it true. Everything else about a class stays the
-- application's; its position in the tree does not.
REVOKE INSERT, UPDATE, DELETE ON item_class FROM spork_app;
REVOKE INSERT, UPDATE, DELETE ON party_class FROM spork_app;

GRANT INSERT (id, tenant_id, parent_id, code, name), UPDATE (code, name)
    ON item_class TO spork_app;
GRANT INSERT (id, tenant_id, parent_id, code, name), UPDATE (code, name)
    ON party_class TO spork_app;

-- INSERT keeps `parent_id`, for D42's reason and with D51's caveat: a class is
-- created somewhere, that original is the base of the fold, and question 137
-- already carries the fact that the base and the result share storage.
--
-- **DELETE goes too, and J36 is what said so.** Its rule is that no login role may
-- UPDATE a projection column *or DELETE from the table carrying one*, and it
-- fired the moment `parent_id` became one. Deleting the row would take the
-- projection with it, which is the same objection whether the table is wholly
-- maintained like `stock` or a reference row with one folded column like this.
-- `order` and `package` are already in that position and neither has DELETE.
--
-- That removes a capability: a class created by mistake could be deleted and now
-- cannot. It was already nearly unusable -- `policy_change.item_class_id` is a
-- foreign key, so any class with history was undeletable already -- but "nearly"
-- is not a design. **Retiring a class is an act nobody has designed**, and it is
-- question 149 rather than an `active` flag invented here to close a gap this
-- migration opened.

-- ---------------------------------------------------------------------------
-- 3. The warning the question asked for
-- ---------------------------------------------------------------------------
--
-- "The UI must surface 'this move changes N active resolutions'". N is derivable,
-- so it is derived here rather than left to whoever writes the screen.
--
-- **Two mistakes were made writing it, and both are worth keeping on the record
-- because both returned a number rather than an error.**
--
-- The first: one function taking a bare `uuid`, computing the answer entirely
-- from the item closure. Asked about a party class it returned **0** -- no item
-- class has that id, and the count of nothing is zero. A screen reading "this
-- move changes 0 resolutions" before a move that changes several is worse than no
-- screen. An untyped id cannot tell two taxonomies apart, so it must not be asked
-- to; hence one function each.
--
-- The second was the definition. It counted bindings scoped *into* the subtree
-- being moved, which is the wrong set: **a binding scoped to GLOVES is exactly as
-- specific wherever GLOVES hangs.** Nothing about it changes. What changes is
-- which *ancestor* bindings reach the subtree's members -- a policy on Suppliers
-- starts applying to Gloveco the moment Glove suppliers moves underneath it.
--
-- That set is the symmetric difference of the old ancestor chain and the new one,
-- and computing it needs the destination. So the function takes the pair the
-- caller is about to record: the class and where it is going. NULL destination
-- means the root, the same convention the fold uses.

CREATE FUNCTION item_class_move_impact(p_class_id uuid, p_new_parent_id uuid)
    RETURNS bigint
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    WITH old_anc AS (
        SELECT ancestor_id AS id FROM item_class_closure
         WHERE descendant_id = p_class_id AND ancestor_id <> p_class_id),
    new_anc AS (
        SELECT ancestor_id AS id FROM item_class_closure
         WHERE descendant_id = p_new_parent_id),
    gained AS (SELECT id FROM new_anc EXCEPT SELECT id FROM old_anc),
    lost   AS (SELECT id FROM old_anc EXCEPT SELECT id FROM new_anc)
    SELECT count(*) FROM policy_binding b
     WHERE b.item_class_id IN (SELECT id FROM gained UNION SELECT id FROM lost)
$$;

-- The party side reaches its members through `party.party_class_id` rather than a
-- classification table, which is D45's asymmetry -- an item may sit in several
-- classes and a counterparty sits in one. It makes no difference here, because
-- only class-scoped bindings can change: a binding naming a party directly names
-- it wherever its class hangs.
CREATE FUNCTION party_class_move_impact(p_class_id uuid, p_new_parent_id uuid)
    RETURNS bigint
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    WITH old_anc AS (
        SELECT ancestor_id AS id FROM party_class_closure
         WHERE descendant_id = p_class_id AND ancestor_id <> p_class_id),
    new_anc AS (
        SELECT ancestor_id AS id FROM party_class_closure
         WHERE descendant_id = p_new_parent_id),
    gained AS (SELECT id FROM new_anc EXCEPT SELECT id FROM old_anc),
    lost   AS (SELECT id FROM old_anc EXCEPT SELECT id FROM new_anc)
    SELECT count(*) FROM policy_binding b
     WHERE b.party_class_id IN (SELECT id FROM gained UNION SELECT id FROM lost)
$$;

COMMENT ON FUNCTION item_class_move_impact(uuid, uuid) IS
    'How many bindings start or stop reaching this subtree if the class moves to '
    'the given parent -- the symmetric difference of the old and new ancestor '
    'chains. NULL parent means the root. Question 78 asked for this number on the '
    'screen; it is derived so the screen cannot get it wrong. D72.';
COMMENT ON FUNCTION party_class_move_impact(uuid, uuid) IS
    'The same for the counterparty taxonomy. Separate from the item form because '
    'one function taking an untyped id answered 0 for the taxonomy it could not '
    'see. D72.';

GRANT EXECUTE ON FUNCTION item_class_move_impact(uuid, uuid),
    party_class_move_impact(uuid, uuid)
    TO spork_app, spork_platform, spork_scheduler, spork_projection_owner;

-- ---------------------------------------------------------------------------
-- 4. What this does not solve
-- ---------------------------------------------------------------------------
--
-- 78's second claim is true and now checkable: past *decisions* are safe, because
-- `expected_supply` freezes `receiving_policy_id` and `allocation_policy_id` on
-- the row at first use, so a decision already taken holds a foreign key to the
-- value it used and no move can reach it.
--
-- The third is not solved. **"What should have applied" is still answered by the
-- current closure**, so a replay after a move gets today's tree rather than the
-- one that was in force. The move is now reconstructible -- the log has every
-- parentage change with its moment -- so the historical tree is derivable, but
-- deriving it is not the same as querying it, and D24 built `package_containment`
-- rather than replaying `package_event` for exactly that reason.
--
-- A temporal closure is the answer and it is a table with a validity range and an
-- exclusion constraint, on the idiom migration 5 already established. It is
-- question 148 and not this migration, because building it before anything asks
-- an as-at question would be guessing at the shape of the question.
