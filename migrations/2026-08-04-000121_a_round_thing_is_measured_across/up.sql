-- A round thing is measured across, at the top and at the base (D213).
--
-- D191 made a subject say what it is packed in, and drew a box only for the
-- six-sided types; anything else was "a thing", photographed as taken. A
-- bucket, a tin or a tub is a thing of a particular shape: round, usually
-- tapering. Measured across its top, across its base and up its height, it can
-- be drawn as what it is rather than as the box it fits in.
--
-- **Three metrics, and the box it fits in is still recorded.** D191: "a size is
-- the box a thing fits in, whatever its shape." For a round thing the tape
-- across the wider end *is* the width of that box, so length and width are
-- recorded from the same look as the diameters, and everything that reads a
-- size (what counts as measured, the bench, packing) goes on reading it.
-- `diameter`, `base_diameter` and `top_height` are the shape's detail. A
-- tapered bucket often has a straight band below its rim: `height` is the
-- whole height, and `top_height` how much of it at the top is straight, at the
-- top's width. A straight-sided thing has neither recorded: the top's width
-- stands for the base's, and all of it is straight.

INSERT INTO metric (tenant_id, code, label, result_kind, dimension_id, reserved, applies_to)
SELECT NULL, m.code, m.label, 'quantity'::metric_result_kind, d.id, true,
       ARRAY['item', 'package', 'item_style', 'item_part', 'lot']
  FROM (VALUES
        ('diameter',      'Across the top'),
        ('base_diameter', 'Across the base'),
        ('top_height',    'Top part''s height')
       ) AS m(code, label)
  JOIN dimension d ON d.code = 'length'
ON CONFLICT DO NOTHING;

DO $$
BEGIN
    IF (SELECT count(*) FROM metric WHERE tenant_id IS NULL AND code IN ('diameter', 'base_diameter', 'top_height')) <> 3 THEN
        RAISE EXCEPTION 'the three round metrics were not made: is the length dimension there?';
    END IF;
END $$;

-- Which of GS1's types are round. Spork's, beside six_sided. Rolls, reels and
-- tubes are round too, but lie on their side, and drawing one standing would be
-- wrong; they join when one is measured.
ALTER TABLE packaging_type ADD COLUMN round boolean NOT NULL DEFAULT false;
UPDATE packaging_type SET round = true WHERE code IN ('BJ', 'CNG', 'CY', 'BA', 'CU', 'PT', 'JR', 'AE');
ALTER TABLE packaging_type ADD CONSTRAINT packaging_type_one_shape_ck CHECK (NOT (round AND six_sided));

COMMENT ON COLUMN packaging_type.round IS
  'Spork''s: the type is round, measured across its top and base and photographed by its side and lid. D213, migration 121.';
