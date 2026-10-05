-- Migration 124: which level of an item is one in NetSuite (D218).
--
-- Spork's three levels are physical: the each is the single product, an inner
-- pack holds so many, a carton holds so many packs. NetSuite counts an item in
-- one unit, and that unit can be any of the three: Foodcare sells a catalogue
-- by the each, earplugs by the box of 100 (D185's inner), gloves by the
-- carton of 1,000. Nothing in Spork said which, so:
--
--   - every item was offered a carton (D178), a catalogue as much as a glove;
--   - a box of ten respirators was measured on the carton card;
--   - the pack bench read an order for two cartons of gloves as two gloves,
--     because it counted what was ordered in eaches.
--
-- So an item says which level is one of what NetSuite counts. Unsaid, it is
-- taken from NetSuite's Pack Unit (`reported_item.selling_unit`, D217) by one
-- rule, `unit_level_of`; said here, the newest saying wins, as what a subject
-- is packed in does (D191).

CREATE TABLE item_unit (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id       uuid NOT NULL REFERENCES tenant(id),
    item_id         uuid NOT NULL,
    level           packaging_level NOT NULL,
    client_event_id uuid NOT NULL,
    recorded_by_id  uuid NOT NULL REFERENCES person(id),
    recorded_at     timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT item_unit_item_fk
        FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id),
    CONSTRAINT item_unit_event_fk
        FOREIGN KEY (tenant_id, client_event_id) REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT item_unit_event_key UNIQUE (tenant_id, client_event_id)
);

COMMENT ON TABLE item_unit IS
    'Which level of an item is one in NetSuite, said in Spork. The newest saying wins; '
    'unsaid, NetSuite''s Pack Unit decides (unit_level_of). D218, migration 124.';

CREATE INDEX item_unit_item_idx ON item_unit (item_id, recorded_at DESC);

ALTER TABLE item_unit ENABLE ROW LEVEL SECURITY;
ALTER TABLE item_unit FORCE ROW LEVEL SECURITY;
CREATE POLICY item_unit_tenant_scoped ON item_unit
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

GRANT SELECT, INSERT ON item_unit TO spork_app;

-- **The one rule** from NetSuite's Pack Unit to a level. A carton by any of
-- its names is the carton; a box or a pack is the inner pack, as DEJ-8040's
-- box of 100 earplugs was always measured; everything else, and nothing set,
-- is the each: Each, Pair, Roll, Drum and UNT are each one thing.
CREATE FUNCTION unit_level_of(p_unit text) RETURNS packaging_level
LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE upper(btrim(coalesce(p_unit, '')))
               WHEN 'CTN' THEN 'carton'
               WHEN 'CARTON' THEN 'carton'
               WHEN 'CS' THEN 'carton'
               WHEN 'CASE' THEN 'carton'
               WHEN 'BOX' THEN 'inner'
               WHEN 'BX' THEN 'inner'
               WHEN 'PACK' THEN 'inner'
               WHEN 'PK' THEN 'inner'
               ELSE 'each'
           END::packaging_level
$$;

-- The unit of every item Spork or NetSuite has said anything of: said here,
-- or NetSuite's by the rule. **An item in neither is sold by the each**, and
-- has no row: most are, and a row for each of nine thousand items made the
-- capture worklist's one query several times slower. Set-based rather than
-- a lookup per item, for the same reason. Invoker's rights, so the tenant's
-- policy applies to what it reads.
CREATE VIEW item_unit_level WITH (security_invoker = true) AS
SELECT coalesce(said.item_id, ns.item_id) AS item_id,
       coalesce(said.level, unit_level_of(ns.selling_unit)) AS level,
       said.level IS NOT NULL AS said,
       ns.selling_unit AS netsuite_unit
  FROM (SELECT DISTINCT ON (item_id) item_id, level FROM item_unit
         ORDER BY item_id, recorded_at DESC, id DESC) said
  FULL JOIN (SELECT DISTINCT ON (item_id) item_id, selling_unit FROM reported_item
              ORDER BY item_id, as_at DESC) ns
    ON ns.item_id = said.item_id;

COMMENT ON VIEW item_unit_level IS
    'Which level of an item is one in NetSuite, and whether Spork said so; an item not here is sold by '
    'the each. D218, migration 124.';

GRANT SELECT ON item_unit_level TO spork_app;
