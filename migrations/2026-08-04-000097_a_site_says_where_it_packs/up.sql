-- Migration 97: a site says where it packs.
--
-- The pack bench needs one place at the site for two things: where a new
-- carton comes into being (D97: `created` asserts placement), and where goods
-- picked elsewhere are put down before they are boxed (D172's handover to
-- staging). It chose both by guessing: the first location by code for the
-- carton, and the first `staging` location by code for the goods.
--
-- # Why the guess fails on a real bin list
--
-- The fixture has one staging location, `PACK-1`, and the guess found it. A real
-- warehouse's bin list uses its WMS's "Staging" type for whatever that site
-- decided it means. At one real site it is the upper levels of every rack,
-- hundreds of them, so goods handed over at the bench would be recorded on the
-- sixth level of the first rack. And the first location by code is a rack
-- bin, so every carton would come into being there.
--
-- So a site names the location: `site.pack_location_id`. It is a location like
-- any other, one that holds stock and appears in the ledger. When a site has
-- not said, the bench says so rather than guessing, as a site with no owner
-- does for a handover (migration 95).
--
-- The location must be at the site. A composite key against (id, site_id)
-- holds that, as `location_place_site_fk` holds it for places (S37): a pack
-- bench in another building is in the wrong building.

ALTER TABLE location
    ADD CONSTRAINT location_site_key UNIQUE (id, site_id);

ALTER TABLE site
    ADD COLUMN pack_location_id uuid,
    -- S51: a reference carries its tenant, so no site can name another
    -- tenant's location.
    ADD CONSTRAINT site_pack_location_tenant_fk FOREIGN KEY (pack_location_id, tenant_id)
        REFERENCES location(id, tenant_id),
    ADD CONSTRAINT site_pack_location_fk FOREIGN KEY (pack_location_id, id)
        REFERENCES location(id, site_id);

COMMENT ON COLUMN site.pack_location_id IS
    'Where this site packs: new cartons are made here and goods picked elsewhere '
    'are put down here before they are boxed. NULL until a person says. Migration 97.';

GRANT INSERT (pack_location_id), UPDATE (pack_location_id) ON site TO spork_app;
