-- Migration 65: every deployment can receive.
--
-- `resolve_receiving_policy` refuses the act when no binding matches, and until
-- now the only `receiving` binding that matched everything lived in
-- `fixtures/seed.sql`. A deployment that loaded the schema and not the fixture
-- therefore could not record that a truck had arrived, which is the one thing D5
-- says a warehouse system may never do:
--
--     Scans are records of what happened, not requests for permission.
--
-- **The fix is not to soften the refusal.** A refusal is right when a policy
-- genuinely cannot be resolved, because `disposition()` would otherwise have no
-- version to stamp and J63 would have nothing to read. What is wrong is that the
-- condition was reachable at all. Architecture: *"The system is complete on its
-- own. Every external system it talks to is one implementation of a join that has
-- a working default behind it."* The default belongs in the schema, beside the
-- mechanism it completes, rather than in a fixture the fixture's own comments
-- describe as a demonstration.
--
-- So the row moves rather than being written again. Same id, same values, same
-- note, deleted from `seed.sql` in the same commit -- because a second all-NULL
-- platform binding of the same kind would tie with the first on every dimension,
-- which is precisely the equal-specificity case D22 raises `policy_ambiguous`
-- for. Moving it also keeps every answer in `fixtures/resolver-golden.txt` byte
-- for byte, including the recorded `require_lot:0->1` clamp: J22 exists to make
-- someone read the diff when a shipped default changes meaning, and this change
-- deliberately has no such diff to read.
--
-- **What it ships is a ceiling, and that is the point.** D83's clamps make a
-- platform value a bound on a tenant's rather than a value the tenant inherits,
-- and S15 puts tenancy at index 0 of every ordering so a tenant's own binding
-- always outranks this one. A tenant configuring nothing gets ten percent
-- over-delivery tolerance and lot capture; a tenant configuring something gets
-- their own answer, clamped.

INSERT INTO policy_binding (id, tenant_id, kind, note) VALUES
    ('b0000000-0000-0000-0000-000000000005', NULL, 'receiving',
     'platform receiving ceiling');

-- No `policy_change` beside it, and there cannot be one: `policy_change` reaches
-- `client_event` through `(tenant_scope_id, client_event_id)` and
-- `client_event.tenant_id` is NOT NULL, so an act belonging to no tenant has no
-- event to name. J13 exempts platform-shipped versions for that reason and
-- question 138 carries it.
INSERT INTO receiving_policy (id, policy_binding_id, effective, respond_by_hours,
    require_lot, tolerance_over_pct, tolerance_under_pct) VALUES
    ('4ec00000-0000-0000-0000-000000000001', 'b0000000-0000-0000-0000-000000000005',
     tstzrange('2026-01-01T00:00:00Z', NULL, '[)'), 48, true, 10, 10);
