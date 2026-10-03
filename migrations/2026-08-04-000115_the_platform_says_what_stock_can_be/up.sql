-- Migration 115: the platform says what stock can be.
--
-- Every movement into stock names a status, and the handover and the claim
-- paths name the platform's "available": `inventory_status` with code
-- 'available' and no tenant. Only `fixtures/seed.sql` ever inserted it. A
-- database built by migrations alone, which is every real one, had no status
-- at all, so the first handover into a carton at the bench failed with "query
-- returned an unexpected number of rows" and left the carton empty. It was
-- found on 2026-10-03, shipping brushes in their own carton (D196) on a
-- workspace restored from a backup (D193).
--
-- The rows are the seed's, with the seed's ids: fixed ids, so two Sporks
-- migrated apart agree on them and a restore has nothing to match (D194).

INSERT INTO inventory_status (id, tenant_id, code, name, is_available_for_allocation) VALUES
    ('57a70000-0000-0000-0000-000000000001', NULL, 'available', 'Available', true),
    ('57a70000-0000-0000-0000-000000000002', NULL, 'quarantine', 'Quarantine', false)
ON CONFLICT DO NOTHING;
