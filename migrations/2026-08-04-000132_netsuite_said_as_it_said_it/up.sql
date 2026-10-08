-- Migration 132: NetSuite's word on an item, every field as it said it (D238).
--
-- D237 took NetSuite's weight, size and UPC into columns of their own, read
-- into grams and millimetres as they arrived. That decided, at the door, what
-- each of NetSuite's fields means; and NetSuite holds more that helps on the
-- floor (colour, size, an alert, its own pack counts, carton barcodes) than
-- any fixed set of columns.
--
-- # An observation of NetSuite's record, not of the product
--
-- What NetSuite holds about an item is what somebody once typed into it: an
-- abstraction of the product, not the product. So every field the feed sends
-- is kept **as NetSuite said it**, its text under NetSuite's own name, with
-- when NetSuite said it (`reported_item_field`). A value NetSuite changes, or
-- stops saying, is closed, and the new one opened beside it: the history of
-- what NetSuite said is kept, as an observation's would be. Nothing of it is
-- Spork's record of the product, and nothing of Spork's is written from it.
--
-- # What a field means is a setting
--
-- Which of NetSuite's fields is a weight, a barcode at a level, a pack count,
-- a picture, something shown beside the code or only kept, is Spork's word
-- (`reported_field_said`), newest first, over defaults for the names this
-- NetSuite is known to use (`reported_field_default`). `reported_item_said`
-- is what NetSuite says of each item now, each field with its meaning.

DROP VIEW IF EXISTS item_netsuite_differs;
ALTER TABLE reported_item
    DROP COLUMN picture_file,
    DROP COLUMN weight_g,
    DROP COLUMN length_mm,
    DROP COLUMN width_mm,
    DROP COLUMN height_mm,
    DROP COLUMN upc;

CREATE TABLE reported_item_field (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id   uuid NOT NULL REFERENCES tenant(id),
    item_id     uuid NOT NULL,
    -- The feed it came by, and the field as the feed names it.
    source      text NOT NULL,
    field       text NOT NULL,
    -- As NetSuite said it: text, unread.
    value       text NOT NULL,
    -- The load that first said it, and the load that said otherwise or
    -- nothing; open while NetSuite still says it.
    said_from   timestamptz NOT NULL,
    said_to     timestamptz,
    CONSTRAINT reported_item_field_item_fk FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id),
    CONSTRAINT reported_item_field_name_ck CHECK (length(btrim(field)) BETWEEN 1 AND 120),
    CONSTRAINT reported_item_field_span_ck CHECK (said_to IS NULL OR said_to >= said_from)
);

-- One value said now, a field, an item, a feed.
CREATE UNIQUE INDEX reported_item_field_now_key
    ON reported_item_field (tenant_id, item_id, source, field) WHERE said_to IS NULL;
CREATE INDEX reported_item_field_item_idx ON reported_item_field (item_id, field);

COMMENT ON TABLE reported_item_field IS
    'What NetSuite said of an item, field by field, as it said it and when: an observation of its record, '
    'not of the product. Closed, never edited, when it says otherwise. D238, migration 132.';

ALTER TABLE reported_item_field ENABLE ROW LEVEL SECURITY;
ALTER TABLE reported_item_field FORCE ROW LEVEL SECURITY;
CREATE POLICY reported_item_field_own ON reported_item_field FOR ALL
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- Said, then closed: a value is never rewritten.
GRANT SELECT, INSERT ON reported_item_field TO spork_app;
GRANT UPDATE (said_to) ON reported_item_field TO spork_app;
GRANT SELECT ON reported_item_field TO spork_platform;

-- What a field can be read as.
--   kept          recorded, and shown with the rest of what NetSuite says
--   shown         beside the code wherever the item is: a colour, a size
--   warning       beside the item at the bench and on its page: an alert
--   note          on the item's page
--   art_no        its article number: found by, and shown under the code
--   picture       its picture's file, brought by the Bridge (D237)
--   weight        a weight; its unit fixed, or the value of another field
--   length, width, height   a size, the same
--   barcode       a barcode at a level, or at the level NetSuite counts
--   per_carton    how many of the item a carton holds
--   per_inner     how many of the item a pack holds
--   inners_per_carton       how many packs a carton holds
CREATE FUNCTION reported_field_role_ok(p_role text) RETURNS boolean
LANGUAGE sql IMMUTABLE AS $$
    SELECT p_role IN ('kept', 'shown', 'warning', 'note', 'art_no', 'picture', 'weight', 'length',
                      'width', 'height', 'barcode', 'per_carton', 'per_inner', 'inners_per_carton')
$$;

CREATE TABLE reported_field_default (
    field       text PRIMARY KEY,
    role        text NOT NULL CHECK (reported_field_role_ok(role)),
    unit        text,
    unit_field  text,
    level       packaging_level
);

COMMENT ON TABLE reported_field_default IS
    'How a field NetSuite sends is read until somebody says otherwise: the names this NetSuite uses. D238.';

INSERT INTO reported_field_default (field, role, unit, unit_field, level) VALUES
    ('Picture', 'picture', NULL, NULL, NULL),
    ('Transaction Image', 'picture', NULL, NULL, NULL),
    ('Supplier Part No.', 'art_no', NULL, NULL, NULL),
    ('Article No.', 'art_no', NULL, NULL, NULL),
    ('Weight', 'weight', NULL, 'Weight Unit', NULL),
    ('Item Weight', 'weight', NULL, 'Weight Unit', NULL),
    ('Length', 'length', NULL, 'Dimension Unit', NULL),
    ('Width', 'width', NULL, 'Dimension Unit', NULL),
    ('Height', 'height', NULL, 'Dimension Unit', NULL),
    ('Length (cm)', 'length', 'cm', NULL, NULL),
    ('Width (cm)', 'width', 'cm', NULL, NULL),
    ('Height (cm)', 'height', 'cm', NULL, NULL),
    ('UPC', 'barcode', NULL, NULL, NULL),
    ('UPC Code', 'barcode', NULL, NULL, NULL),
    ('APN (Carton)', 'barcode', NULL, NULL, 'carton'),
    ('APN (Box/Inner)', 'barcode', NULL, NULL, 'inner'),
    ('Each/Carton', 'per_carton', NULL, NULL, NULL),
    ('Each/Inner', 'per_inner', NULL, NULL, NULL),
    ('Inner/Carton', 'inners_per_carton', NULL, NULL, NULL),
    ('Colour', 'shown', NULL, NULL, NULL),
    ('Size', 'shown', NULL, NULL, NULL),
    ('Size (PPE)', 'shown', NULL, NULL, NULL),
    ('Style', 'shown', NULL, NULL, NULL),
    ('Alert', 'warning', NULL, NULL, NULL),
    ('External Notes', 'note', NULL, NULL, NULL),
    ('Internal Notes', 'note', NULL, NULL, NULL);

GRANT SELECT ON reported_field_default TO spork_app, spork_platform;

CREATE TABLE reported_field_said (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id       uuid NOT NULL REFERENCES tenant(id),
    source          text NOT NULL,
    field           text NOT NULL,
    role            text NOT NULL CHECK (reported_field_role_ok(role)),
    -- A weight's or a size's unit: fixed, or the value of another field.
    unit            text,
    unit_field      text,
    -- A barcode's level; none, the level NetSuite counts (D218).
    level           packaging_level,
    client_event_id uuid NOT NULL,
    recorded_by_id  uuid NOT NULL REFERENCES person(id),
    recorded_at     timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT reported_field_said_event_fk
        FOREIGN KEY (tenant_id, client_event_id) REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT reported_field_said_event_key UNIQUE (tenant_id, client_event_id)
);

CREATE INDEX reported_field_said_field_idx ON reported_field_said (source, field, recorded_at DESC);

COMMENT ON TABLE reported_field_said IS
    'Spork''s word on how one of NetSuite''s fields is read: newest first, over its default. D238.';

ALTER TABLE reported_field_said ENABLE ROW LEVEL SECURITY;
ALTER TABLE reported_field_said FORCE ROW LEVEL SECURITY;
CREATE POLICY reported_field_said_own ON reported_field_said
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

GRANT SELECT, INSERT ON reported_field_said TO spork_app;

-- **What one of NetSuite's fields means**: the newest saying, or its
-- default, or only kept. One rule for every reader: each item's fields, and
-- the settings that list them.
CREATE FUNCTION reported_field_meaning(p_source text, p_field text)
RETURNS TABLE (role text, unit text, unit_field text, level packaging_level, said boolean)
LANGUAGE sql STABLE AS $$
    SELECT coalesce(s.role, d.role, 'kept'),
           CASE WHEN s.id IS NOT NULL THEN s.unit ELSE d.unit END,
           CASE WHEN s.id IS NOT NULL THEN s.unit_field ELSE d.unit_field END,
           CASE WHEN s.id IS NOT NULL THEN s.level ELSE d.level END,
           s.id IS NOT NULL
      FROM (SELECT 1) AS one
      LEFT JOIN LATERAL (SELECT x.* FROM reported_field_said x
                          WHERE x.source = p_source AND x.field = p_field
                          ORDER BY x.recorded_at DESC, x.id DESC
                          LIMIT 1) s ON true
      LEFT JOIN reported_field_default d ON d.field = p_field
$$;

-- **What NetSuite says of each item now**, each field with what it means.
CREATE VIEW reported_item_said WITH (security_invoker = true) AS
SELECT f.item_id, f.source, f.field, f.value, f.said_from, m.role, m.unit, m.unit_field, m.level
  FROM reported_item_field f
 CROSS JOIN LATERAL reported_field_meaning(f.source, f.field) m
 WHERE f.said_to IS NULL;

COMMENT ON VIEW reported_item_said IS
    'What NetSuite says of each item now, each field with what Spork reads it as. D238, migration 132.';

GRANT SELECT ON reported_item_said TO spork_app;

-- Reading a field's text as a figure, for comparing. None where it isn't a
-- number more than nothing, or its unit is one these don't know.
CREATE FUNCTION number_in(p_text text) RETURNS numeric
LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE WHEN btrim(replace(p_text, ',', '')) ~ '^[0-9]+(\.[0-9]+)?$'
                THEN btrim(replace(p_text, ',', ''))::numeric END
$$;

CREATE FUNCTION grams_of(p_value text, p_unit text) RETURNS numeric
LANGUAGE sql IMMUTABLE AS $$
    SELECT nullif(number_in(p_value) * CASE lower(btrim(coalesce(p_unit, '')))
               WHEN 'g' THEN 1 WHEN 'gram' THEN 1 WHEN 'grams' THEN 1
               WHEN 'kg' THEN 1000 WHEN 'kgs' THEN 1000 WHEN 'kilogram' THEN 1000 WHEN 'kilograms' THEN 1000
               WHEN 'lb' THEN 453.59237 WHEN 'lbs' THEN 453.59237 WHEN 'pound' THEN 453.59237
               WHEN 'oz' THEN 28.349523125 WHEN 'ounce' THEN 28.349523125
           END, 0)
$$;

CREATE FUNCTION millimetres_of(p_value text, p_unit text) RETURNS numeric
LANGUAGE sql IMMUTABLE AS $$
    SELECT nullif(number_in(p_value) * CASE lower(btrim(coalesce(p_unit, '')))
               WHEN 'mm' THEN 1 WHEN 'cm' THEN 10 WHEN 'm' THEN 1000
               WHEN 'in' THEN 25.4 WHEN 'inch' THEN 25.4 WHEN 'inches' THEN 25.4
               WHEN 'ft' THEN 304.8
           END, 0)
$$;

-- **The article numbers in one of NetSuite's**, which sometimes holds several:
-- "BSG21/EBR330/1600" is one, "7008 or 22003F" and "300410LARGE / U.MG20-111WH-L"
-- are two. Split at " / " and " or ", never at a bare "/", which codes use.
CREATE FUNCTION art_numbers_in(p_value text) RETURNS text[]
LANGUAGE sql IMMUTABLE AS $$
    SELECT coalesce(array_agg(btrim(p) ORDER BY n) FILTER (WHERE btrim(p) <> ''), '{}')
      FROM regexp_split_to_table(coalesce(p_value, ''), '\s+/\s+|\s+or\s+', 'i') WITH ORDINALITY AS t(p, n)
$$;

-- **Where NetSuite and Spork disagree**, read through what each field means,
-- never stored. Against the unit's figures measured here (D219), the
-- barcodes scanned at a level, and the case pack in force:
--
-- - weight more than 10 g and 5 % apart;
-- - size where any side, both sorted longest first, is more than 5 mm and
--   5 % apart;
-- - a barcode at a level where Spork has GTINs there and none is NetSuite's;
-- - a pack count that is neither Spork's count of single ones nor of what
--   NetSuite counts (a pair is two, D233).
CREATE VIEW item_netsuite_differs WITH (security_invoker = true) AS
WITH said AS (
    SELECT r.*,
           -- A unit fixed, or the value of the field it names.
           coalesce(r.unit, (SELECT u.value FROM reported_item_said u
                              WHERE u.item_id = r.item_id AND u.source = r.source AND u.field = r.unit_field)) AS in_unit
      FROM reported_item_said r
     WHERE r.role IN ('weight', 'length', 'width', 'height', 'barcode', 'per_carton', 'per_inner', 'inners_per_carton')
),
items AS (
    SELECT DISTINCT s.item_id, i.style_id,
           coalesce(u.level::text, 'each') AS level, coalesce(u.singles, 1) AS singles
      FROM said s
      JOIN item i ON i.id = s.item_id
      LEFT JOIN item_unit_level u ON u.item_id = s.item_id
),
spork AS (
    SELECT it.*,
           (SELECT max(oc.value_numeric)
              FROM observable o
              JOIN observation_current oc ON oc.observable_id = o.id
              JOIN metric m ON m.id = oc.metric_id
             WHERE (o.item_id = it.item_id OR (it.style_id IS NOT NULL AND o.item_style_id = it.style_id))
               AND o.packaging_level::text = it.level AND m.code = 'gross_weight'
               AND oc.method IN ('instrument', 'scan', 'keyed', 'derived', 'photographed')
               AND it.singles = 1) AS weight_g,
           (SELECT array_agg(oc.value_numeric ORDER BY oc.value_numeric DESC)
              FROM observable o
              JOIN observation_current oc ON oc.observable_id = o.id
              JOIN metric m ON m.id = oc.metric_id
             WHERE (o.item_id = it.item_id OR (it.style_id IS NOT NULL AND o.item_style_id = it.style_id))
               AND o.packaging_level::text = it.level AND m.code IN ('length', 'width', 'height')
               AND oc.value_numeric IS NOT NULL
               AND oc.method IN ('instrument', 'scan', 'keyed', 'derived', 'photographed')
               AND it.singles = 1) AS sides,
           (SELECT ARRAY[c.units_per_inner, c.inners_per_carton]
              FROM item_packing_config c
             WHERE c.item_id = it.item_id AND c.effective_from <= current_date
             ORDER BY c.effective_from DESC, c.id DESC LIMIT 1) AS case_pack
      FROM items it
),
netsuite AS (
    SELECT sp.*,
           (SELECT max(grams_of(s.value, s.in_unit)) FROM said s
             WHERE s.item_id = sp.item_id AND s.role = 'weight') AS ns_weight_g,
           (SELECT array_agg(x ORDER BY x DESC)
              FROM (SELECT millimetres_of(s.value, s.in_unit) AS x FROM said s
                     WHERE s.item_id = sp.item_id AND s.role IN ('length', 'width', 'height')) l
             WHERE x IS NOT NULL) AS ns_sides
      FROM spork sp
)
SELECT n.item_id,
       n.ns_weight_g IS NOT NULL AND n.weight_g IS NOT NULL
         AND abs(n.weight_g - n.ns_weight_g) > greatest(10, 0.05 * n.ns_weight_g) AS weight_differs,
       cardinality(n.ns_sides) = 3 AND cardinality(n.sides) = 3
         AND EXISTS (SELECT 1 FROM generate_series(1, 3) k
                      WHERE abs(n.sides[k] - n.ns_sides[k]) > greatest(5, 0.05 * n.ns_sides[k])) AS size_differs,
       EXISTS (
           SELECT 1 FROM said s
            WHERE s.item_id = n.item_id AND s.role = 'barcode'
              AND regexp_replace(s.value, '[^0-9]', '', 'g') <> ''
              AND EXISTS (SELECT 1 FROM item_barcode b
                           WHERE b.item_id = n.item_id AND b.scheme = 'gtin'
                             AND (b.packaging_level IS NULL
                                  OR b.packaging_level::text = coalesce(s.level::text, n.level)))
              AND NOT EXISTS (SELECT 1 FROM item_barcode b
                               WHERE b.item_id = n.item_id AND b.scheme = 'gtin'
                                 AND b.barcode = lpad(regexp_replace(s.value, '[^0-9]', '', 'g'), 14, '0'))
       ) AS barcode_differs,
       EXISTS (
           SELECT 1 FROM said s
            WHERE s.item_id = n.item_id AND n.case_pack IS NOT NULL
              AND number_in(s.value) IS NOT NULL
              AND CASE s.role
                    WHEN 'per_carton' THEN
                      n.case_pack[2] IS NOT NULL
                      AND number_in(s.value) <> coalesce(n.case_pack[1], 1) * n.case_pack[2]
                      AND number_in(s.value) * n.singles <> coalesce(n.case_pack[1], 1) * n.case_pack[2]
                    WHEN 'per_inner' THEN
                      n.case_pack[1] IS NOT NULL AND n.case_pack[1] > 1
                      AND number_in(s.value) <> n.case_pack[1]
                      AND number_in(s.value) * n.singles <> n.case_pack[1]
                    WHEN 'inners_per_carton' THEN
                      n.case_pack[2] IS NOT NULL AND coalesce(n.case_pack[1], 1) > 1
                      AND number_in(s.value) <> n.case_pack[2]
                    ELSE false
                  END
       ) AS pack_differs
  FROM netsuite n;

COMMENT ON VIEW item_netsuite_differs IS
    'Where what NetSuite says of an item, read through what each field means, disagrees with what Spork '
    'measured, scanned and said: never stored. D237, D238, migration 132.';

GRANT SELECT ON item_netsuite_differs TO spork_app;
