-- Reverse of 2026-08-04-000001_reference.
--
-- Dropped in dependency order. The roles are deliberately not dropped: they may
-- own objects created by later migrations, and a down migration that removes a
-- role out from under them fails in a way that is harder to diagnose than the
-- leftover role it was trying to avoid.

DROP TABLE IF EXISTS item_classification;
DROP TABLE IF EXISTS item_class_closure;
DROP TABLE IF EXISTS item_class;
DROP TABLE IF EXISTS item;
DROP TABLE IF EXISTS location;
DROP TABLE IF EXISTS zone;
DROP TABLE IF EXISTS site;

-- dimension and unit reference each other, so the composite FK has to go before
-- either table can be dropped.
ALTER TABLE IF EXISTS dimension DROP CONSTRAINT IF EXISTS dimension_canonical_unit_fk;
DROP TABLE IF EXISTS unit;
DROP TABLE IF EXISTS dimension;

DROP TABLE IF EXISTS person_tenant;
DROP TABLE IF EXISTS person;
DROP TABLE IF EXISTS tenant;

DROP FUNCTION IF EXISTS current_tenant();
