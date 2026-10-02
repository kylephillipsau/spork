-- Migration 111: a subject says what it is packed in. D191.
--
-- A carton has six flat sides; a brush in shrink-wrap has none. Photographed
-- as a box, the brush's sides showed the bench behind it, and the drawing
-- pasted them onto a box. Whether a thing is a box is not a guess from its
-- level: an each can come boxed, and an inner pack can be a banded bundle.
--
-- # The standard's words
--
-- Product data already has this attribute: GS1's packaging type code, said at
-- every level of a trade item (the each, the pack, the case). Its codes and
-- definitions are kept here as GS1 publishes them, so what is recorded today
-- can be sent to a catalogue tomorrow without a mapping. A shipping carton is
-- GS1's `CS` (case); its `CT` (carton) is an egg carton.
--
-- Two columns are Spork's own. `six_sided` says the type is a box with six
-- flat faces: photographed side by side, cut to its faces and drawn (D176,
-- D186). `common` orders the few offered first.
--
-- # Said of a subject, not an observable
--
-- An item's carton is one observable per case pack (D23), and a corrected
-- case pack is a new one. What the carton is made of does not change with
-- the count in it, so the fact names the subject (item and level, family and
-- level, variant, or part) the way `POST /observations` does. Facts only:
-- the newest saying wins.

CREATE TABLE packaging_type (
    code        text PRIMARY KEY,
    name        text NOT NULL,
    definition  text NOT NULL,
    six_sided   boolean NOT NULL DEFAULT false,
    common      smallint
);

COMMENT ON TABLE packaging_type IS
    'GS1 packaging type codes, shipped by us and never tenant-scoped. '
    'six_sided and common are Spork''s. D191, migration 111.';

INSERT INTO packaging_type (code, name, definition, six_sided, common) VALUES
('AA',  'Intermediate Bulk Container, rigid plastic', 'A Rigid Intermediate Bulk Container (RIBC) that is attached to a pallet or has the pallet integrated into the RIBC.', false, NULL),
('AE',  'Aerosol', 'A gas-tight, pressure-resistant container with a valve and propellant.', false, NULL),
('AM',  'Ampoule', 'A relatively small container in tube shape, the end of which is drawn into a stem and closed by fusion after filling.', false, NULL),
('BA',  'Barrel', 'A cylindrical packaging whose bottom end is permanently fixed to the body and top end (head) is either removable or non-removable.', false, NULL),
('BBG', 'Bag in Box', 'A type of container for the storage and transportation of liquids consisting of a strong bladder, usually seated inside a corrugated fibreboard box.', true, NULL),
('BG',  'Bag', 'A preformed, flexible container, generally enclosed on all but one side, which forms an opening that may or may not be sealed after filling.', false, 3),
('BJ',  'Bucket', 'A container, usually cylindrical, can be equipped with a lid and a handle.', false, NULL),
('BK',  'Basket', 'A semi rigid container usually open at the top traditionally used for gathering, shipping and marketing agricultural products.', false, NULL),
('BL',  'Berlingot', 'Packaging most often tetrahedral or cylindrical in shape used to contain liquids.', false, NULL),
('BO',  'Bottle', 'A container having a round neck of relatively smaller diameter than the body, as compared with a jar, and an opening capable of holding a closure.', false, 8),
('BPG', 'Blister pack', 'A type of packaging in which the item is secured between a cavity or pocket made from a formable web, usually transparent, and a flat backing.', false, 7),
('BRI', 'Brick', 'A rectangular-shaped, stackable package designed primarily for liquids such as juice or milk.', true, NULL),
('BX',  'Box', 'A non-specific term used to refer to a rigid container with closed faces that completely enclose its contents.', true, 2),
('CG',  'Cage', 'A container enclosed on at least one side by a grating of wires or bars that lets in air and light.', false, NULL),
('CHB', 'Chub', 'A type of container formed by a tube of flexible, non-edible packaging material with the appearance of a sausage with the ends sealed by metal crimps or clips.', false, NULL),
('CM',  'Card', 'A flat package to which the product is hung or attached for display.', false, NULL),
('CMS', 'Clam Shell', 'A one-piece container consisting of two halves joined by a hinge area which allows the structure to come together to close.', false, NULL),
('CNG', 'Can/Tin', 'A metallic and generally cylindrical container of unspecified size which can be used for items of consumer and institutional sizes.', false, NULL),
('CP',  'Capsule', 'Small packaging element generally cylindrical with round edges and containing any substances like liquids, small objects like toys, coffee capsules.', false, NULL),
('CQ',  'Cartridge', 'A container holding an item or substance, designed for insertion into a mechanism.', false, NULL),
('CR',  'Crate', 'A non-specific term usually referring to a rigid three-dimensional container with semi-closed faces that enclose its contents for shipment or storage.', true, NULL),
('CS',  'Case', 'A non-specific term for a container designed to hold, house, and sheath or encase its content while protecting it during distribution, storage and/or exhibition.', true, 1),
('CT',  'Carton', 'A non-specific term for a re-closable container used mostly for perishable foods (e.g. eggs, fruit).', true, NULL),
('CU',  'Cup/Tub/Bowl', 'A flat-bottomed container that has a base of any shape and which may or not be closed with a lid.', false, NULL),
('CY',  'Cylinder', 'A rigid cylindrical container with straight sides and circular ends of equal size.', false, NULL),
('EN',  'Envelope', 'A predominantly flat container of flexible material having only two faces, and joined at three edges to form an enclosure.', false, NULL),
('GTG', 'Gable top', 'A rectangular-shaped, non-stackable package designed primarily for liquids such as juice or milk.', false, NULL),
('HG',  'Hanger', 'A support for hanging products. Can be curved, triangular, or other various shapes.', false, NULL),
('JG',  'Jug', 'A container, normally cylindrical, with a handle and/or a lid or spout for holding and pouring liquids.', false, NULL),
('JR',  'Jar', 'A rigid container with a large opening, which is used to store products, e.g., jams, cosmetics.', false, NULL),
('MPG', 'Multipack', 'A bundle of products held together for ease of carriage by the consumer. A multipack is always a consumer unit.', false, NULL),
('NE',  'Not packed', 'The item is provided without packaging.', false, 11),
('NT',  'Net', 'A container of meshwork material made from threads or strips twisted or woven to form a regular pattern with spaces between the threads.', false, NULL),
('PB',  'Pallet Box', 'A three-dimensional container which either has a pallet platform permanently attached at its base or alternatively requires a platform.', true, NULL),
('PLP', 'Peel pack', 'A package used for hygienic/sterile products which may be torn open without touching the product inside.', false, NULL),
('PO',  'Pouch', 'A preformed, flexible container, generally enclosed with a gusset seal at the bottom of the pack.', false, NULL),
('PT',  'Pot', 'A flat-bottomed container that has a base of any shape and which may or not be closed with a lid.', false, NULL),
('PU',  'Tray', 'A shallow container, which may or may not have a cover, used for displaying or carrying items.', false, NULL),
('PUG', 'Packed, unspecified', 'Packaging of the product (or products) is currently not on the list.', false, NULL),
('PX',  'Pallet', 'A platform used to hold or transport unit loads.', false, NULL),
('RK',  'Rack', 'A non specific term identifying a framework or stand for carrying, holding, or storing items.', false, NULL),
('RL',  'Reel', 'A spool on which thread, wire, film, etc, is wound. Any device on which a material may be wound.', false, NULL),
('RO',  'Roll', 'Roll', false, 10),
('SG',  'Syringe', 'Device consisting of a piston and a cylindrical pump body, which is used for administering, for example a liquid or joint compound.', false, NULL),
('STR', 'Stretchwrapped', 'In packaging, a high-tensile film, stretched and wrapped repeatedly around an item or group of items to secure and maintain unit integrity.', false, NULL),
('SW',  'Shrinkwrapped', 'In packaging, a film around an item or group of items which is heated causing the film to shrink, securing the unit integrity.', false, 4),
('SY',  'Sleeve', 'A sleeve or banderole that is open-ended and is slid over the contents for protection or presentation.', false, NULL),
('TU',  'Tube', 'A cylindrical container sealed on one end that could be closed with a cap or dispenser on the other end.', false, 9),
('WIRE','Wire', 'A packaging in the form of very flexible thread or slender rod.', false, NULL),
('WRP', 'Wrapper', 'The process of enclosing all or part of an item with layer(s) of flexible wrapping material.', false, 5),
('X11', 'Banded package', 'Something used to bind, tie, or encircle the item or its packaging to secure and maintain unit integrity.', false, 6),
('ZU',  'Flexible Intermediate Bulk Container', 'A non-rigid container used for transport and storage of fluids and other bulk materials.', false, NULL);

GRANT SELECT ON packaging_type TO spork_app, spork_platform;

CREATE TABLE subject_packaging (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),

    -- The subject, as `POST /observations` names one: exactly one arm, and a
    -- level with the item and family arms only.
    item_id          uuid,
    item_style_id    uuid,
    lot_id           uuid,
    item_part_id     uuid,
    packaging_level  packaging_level,

    packaging_type   text NOT NULL REFERENCES packaging_type(code),

    client_event_id  uuid NOT NULL,
    recorded_by_id   uuid NOT NULL REFERENCES person(id),
    recorded_at      timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT subject_packaging_one_arm_ck
        CHECK (num_nonnulls(item_id, item_style_id, lot_id, item_part_id) = 1),
    CONSTRAINT subject_packaging_level_ck
        CHECK ((packaging_level IS NOT NULL) = (item_id IS NOT NULL OR item_style_id IS NOT NULL)),
    CONSTRAINT subject_packaging_item_fk
        FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id),
    CONSTRAINT subject_packaging_style_fk
        FOREIGN KEY (item_style_id, tenant_id) REFERENCES item_style(id, tenant_id),
    CONSTRAINT subject_packaging_lot_fk
        FOREIGN KEY (lot_id, tenant_id) REFERENCES lot(id, tenant_id),
    CONSTRAINT subject_packaging_part_fk
        FOREIGN KEY (item_part_id, tenant_id) REFERENCES item_part(id, tenant_id),
    CONSTRAINT subject_packaging_event_fk
        FOREIGN KEY (tenant_id, client_event_id) REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT subject_packaging_event_key UNIQUE (tenant_id, client_event_id)
);

COMMENT ON TABLE subject_packaging IS
    'What a subject is packed in, as a GS1 packaging type code. The newest '
    'saying wins. D191, migration 111.';

CREATE INDEX subject_packaging_item_idx ON subject_packaging (item_id, packaging_level, recorded_at DESC) WHERE item_id IS NOT NULL;
CREATE INDEX subject_packaging_style_idx ON subject_packaging (item_style_id, packaging_level, recorded_at DESC) WHERE item_style_id IS NOT NULL;
CREATE INDEX subject_packaging_lot_idx ON subject_packaging (lot_id, recorded_at DESC) WHERE lot_id IS NOT NULL;
CREATE INDEX subject_packaging_part_idx ON subject_packaging (item_part_id, recorded_at DESC) WHERE item_part_id IS NOT NULL;

ALTER TABLE subject_packaging ENABLE ROW LEVEL SECURITY;
ALTER TABLE subject_packaging FORCE ROW LEVEL SECURITY;
CREATE POLICY subject_packaging_tenant_scoped ON subject_packaging
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

GRANT SELECT, INSERT ON subject_packaging TO spork_app;

-- What a subject is packed in, and whose saying it is: its own; for a
-- variant, its item's carton's (`item`, D182); for an item's carton or a
-- variant, its family's carton's (`style`, D190). Nothing said, no row.
-- Invoker's rights, so the tenant's policy applies.
CREATE FUNCTION packed_in(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS TABLE (packaging_type text, source text)
LANGUAGE sql STABLE AS $$
    WITH asked (rank, source, item_id, item_style_id, lot_id, item_part_id, level) AS (
        SELECT 0, 'own', p_item, p_style, p_lot, p_part, p_level
        UNION ALL
        SELECT 1, 'item', l.item_id, NULL::uuid, NULL::uuid, NULL::uuid, 'carton'::packaging_level
          FROM lot l
         WHERE l.id = p_lot
        UNION ALL
        SELECT 2, 'style', NULL::uuid, i.style_id, NULL::uuid, NULL::uuid, 'carton'::packaging_level
          FROM item i
         WHERE i.style_id IS NOT NULL
           AND ((i.id = p_item AND p_level = 'carton')
                OR i.id = (SELECT l.item_id FROM lot l WHERE l.id = p_lot))
    )
    SELECT sp.packaging_type, a.source
      FROM asked a
      JOIN subject_packaging sp
        ON sp.item_id IS NOT DISTINCT FROM a.item_id
       AND sp.item_style_id IS NOT DISTINCT FROM a.item_style_id
       AND sp.lot_id IS NOT DISTINCT FROM a.lot_id
       AND sp.item_part_id IS NOT DISTINCT FROM a.item_part_id
       AND sp.packaging_level IS NOT DISTINCT FROM a.level
     ORDER BY a.rank, sp.recorded_at DESC, sp.id DESC
     LIMIT 1
$$;

COMMENT ON FUNCTION packed_in(uuid, uuid, uuid, uuid, packaging_level) IS
    'What a subject is packed in, own before inherited, and whose saying it is. D191.';
