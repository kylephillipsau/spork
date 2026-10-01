-- Migration 101 down: a carton is said by the loader only.
--
-- The case packs people said stay, as rows a loader might have written: what
-- a carton holds is still true, and the measurements taken against them name
-- them. Only who said it, and when, is lost.

ALTER TABLE item_packing_config
    DROP CONSTRAINT IF EXISTS item_packing_config_act_ck,
    DROP CONSTRAINT IF EXISTS item_packing_config_event_key,
    DROP CONSTRAINT IF EXISTS item_packing_config_event_fk,
    DROP COLUMN IF EXISTS recorded_by_id,
    DROP COLUMN IF EXISTS client_event_id;
