-- Migration 101: what a carton of an item holds is said at the item. D178.
--
-- An item is modelled as itself, its `each`, and its carton as a box of so
-- many of it: `item_packing_config` is that relationship, the carton and the
-- child it holds. Until now only the prepack loader wrote one, so an item the
-- prepack list never named had no carton to measure, and the writer refused
-- one as "not yet a definite thing to measure" (D23). Somebody holding the
-- carton is the person who knows what is in it.
--
-- # The act travels on the row
--
-- A case pack said on the floor is somebody's statement, made at a moment,
-- with a retry identity (D5) like every other act. So the row carries its
-- `client_event` and its person. Rows the loader wrote carry neither, which is
-- what tells the two apart.
--
-- # Saying is filling in or a new version, never a rewrite
--
-- The loader records "there is a carton and nobody has said what is in it"
-- as a row of null counts. Saying the count fills that row in: it is the same
-- carton, now described, and the measurements already taken of it stay its
-- own. Saying a different count where one was said is a different carton
-- (D23), so it is a new row from that day, and the old one still explains
-- what was measured against it. The row in force is the newest by
-- `effective_from`, then by id, and ids are UUIDv7 (migration 18), so two
-- versions said on one day are in the order they were said.

ALTER TABLE item_packing_config
    ADD COLUMN client_event_id uuid,
    ADD COLUMN recorded_by_id  uuid REFERENCES person(id),
    ADD CONSTRAINT item_packing_config_event_fk
        FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),
    -- One act, one case pack: a retried press finds the row it already said.
    ADD CONSTRAINT item_packing_config_event_key
        UNIQUE (tenant_id, client_event_id),
    ADD CONSTRAINT item_packing_config_act_ck
        CHECK ((client_event_id IS NULL) = (recorded_by_id IS NULL));

COMMENT ON COLUMN item_packing_config.client_event_id IS
    'The act that said what a carton of this item holds; absent on a row a '
    'loader wrote. Migration 101, D178.';
COMMENT ON COLUMN item_packing_config.recorded_by_id IS
    'Who said it. Migration 101, D178.';
