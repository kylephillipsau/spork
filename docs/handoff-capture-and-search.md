# Handoff: capture, crops, search and backups

Written 2026-10-02. Read this first. The earlier handoff,
[handoff-orders-bridge-3d.md](./handoff-orders-bridge-3d.md), still covers the
toolchain, tests, layout and packing.

## State

- Spork `main` is pushed, through D193.
- `warehouse-scripts` is pushed. Spork Bridge 0.4.0 is published.
- The database is at migration 112 after the next `local.ps1 start`.
- The full server suite last passed before D191. Since then only the affected
  test files have run: packaging, capture, pictures and backup. A full run was
  stopped for low memory. Run it on a fresh database.

## What landed

Each item is one decision in [domain-model.md](./domain-model.md).

**Measuring items**
- D178. An item's carton is a box of N of the item. The carton card asks how
  many are in it and records the case pack before its figures
  (`POST /items/{id}/carton`, migration 101).
- D185. A carton can hold packs: 6 packs of 24. An item whose case pack has
  packs gets an Inner pack card.
- D190. On an item's page the carton card is that item's own. A family's
  figures show on it, marked as the family's, until the item is measured.
- D182, D184. A print run that looks different is a variant (a lot of the
  item) with its own card, photos and figures (`POST /items/{id}/lots`,
  migration 104). One variant can stand for the item's carton (migration 106).

**Lists and reach**
- D179. A sheet of items is a list: codes pasted in order, worked in that
  order from the item list (`POST /item-lists`, `GET /items?list=`, migration
  102).
- D180. A rack says how many levels are reachable from the floor
  (`POST /places/{id}/reach`, migration 103). The item list sends you to the
  reachable bin with the most stock. Off the layout, NetSuite's Pick bins
  count as reachable. There is no screen yet to set reach.

**Photos**
- D181. A phone takes photos and never runs the face-finder. It needs about
  1.5 GB above the page, and iOS reloads the tab. Photos to crop (`/photos`)
  finds each face at a computer. Each photo has Save, or Adjust first.
- The panel offers the next side as one button, in the order front, right,
  back, left, top, bottom, label. Photos send in the background.
- D183. A side printed like its opposite is recorded as "Same as front" and
  shares that photo and its crop (migration 105).
- D190. Photos to crop shows whose photos they are. Move refiles a whole look
  under the right item and level, and records the original as moved
  (migration 110). Use as taken keeps a photo uncropped.
- D186. Once front, right and top are cropped, the computer draws the item as
  a 3D box and the list shows that drawing (migration 107).
- D188. One item can picture its family (migration 109).
- D191. Each card says what it is packed in, as a GS1 packaging type code
  (migration 111). A case or box is photographed side by side and drawn.
  Anything else gets a photo, then back, label or close-up, and is never
  cropped or drawn.

**NetSuite**
- D187. The bridge asks Spork which NetSuite fulfilments are still open and
  reports those NetSuite packed, shipped or deleted. Spork closes them
  (migration 108). The first run closed 24 of 32.

**Search**
- D189. The header box searches items, bins and orders as you type
  (`GET /search`). Tantivy, one index per tenant, in memory, rebuilt from the
  database at most a minute after a change. On a phone it opens full screen.

**Photos to crop, revised**
- Each photo has its own Save, or Adjust first. Saves queue, so the next can
  be pressed before the last lands.

**Backups**
- D192. `person_tenant.role` means something: `administrator`. Setup writes
  it; migration 112 gives it to each workspace's earliest member. Read through
  the definer `role_in_tenant()`. Only an administrator sees Backup.
- D193. Settings, Backup downloads one zip: every row with the workspace's
  `tenant_id`, the workspace, its people and sign-ins, and every photo, each
  entry AES-256 encrypted with a password of 12 or more characters.
  `scripts\local.ps1 restore <zip>` (`spork-restore`) puts it back into an
  empty Spork at the same migration, in one transaction, as the database owner.
- D194. The first real restore lost every metric: a migration draws the
  platform's metric ids as it runs, the restore passed over the backup's as
  already here, and the measurements named ids this server never had. A backup
  now holds the catalogue its rows refer to, a restore matches shared rows by
  their key and repoints what named them, and a reference to nothing refuses
  the restore.

**Packing**
- D195. The pack bench shows each line's picture, and each carton row's. Above
  the cartons it suggests a box for what is left and how it goes in, layer by
  layer: a plan from above (`Suggestion.tsx`) or 3D (`pack3d.ts`, its own
  chunk). The arrangement is `arrange.ts`, tested in `arrange.test.ts`. The
  bench read now carries each line's `picture` and `packs` (an each and an
  inner, with sizes and cut sides) and each preset's `size`. An each with no
  size is listed with Measure, not guessed at.
- D196, migration 113. A box says whether the suggestion may choose it
  (Workspace, Boxes). A thing says whether it ships as it is (the item card's
  "Ships"; unsaid, a carton does and an each or inner does not). A package can
  be one of an item as it is at any level (`package.own_item_id`,
  `own_level`), and the bench ships them with one press. The suggestion lists
  what ships as it is and boxes only the rest.
- D197. Items can be narrowed to what needs a size for packing, most needed
  first: still to pack on an open order, with no each size, not "no size",
  and not shipping as it is. The packing queue links to it with a count.
- D198. With a carton open, the suggestion fills it first: what is in it
  and what is left arranged together, what is in shown as done, the plan on
  the next layer to put in. Client only (`arrange.ts` `fillOpen`).
- D199. A box can say the most its goods may weigh (Workspace, Boxes, Max
  weight; empty is no limit). The suggestion fills a box no heavier.
- D200, migration 114. A thing can say this way up (the item card's "Way
  up"; unsaid, any way up). The suggestion turns it round, never onto its
  side.
- D201. Despatch, the carrier manifest and the packing list name a parcel
  that is one of a product ("JWR-1002R as it is") and give its item's size.
- D202. The bench's packing plan opens on the whole order: every line,
  ticked itself once it is all in cartons; every parcel, packed or planned,
  with its press; a tally of every unit; and all parcels side by side in 3D
  (`order.ts`, tested in `order.test.ts`).
- Migration 115. The platform's stock statuses ("available", "quarantine")
  were only ever inserted by `fixtures/seed.sql`, so on a database built by
  migrations alone every handover into a carton failed. They are a migration
  now, with the seed's fixed ids.
- D203, migration 116. A claim's hold on a bin is reduced by what has been
  picked from it (`stock_claim_hold`, J3 amended); a picked bin no longer
  reads fewer than nothing available.
- D204, migration 117. The walk and its badge read what is left to pick live
  (`line_to_pick`), and leave out what NetSuite reports picked.
- D205. People, under Settings, for administrators (`people.rs`).
- D206, migration 118. Devices hear `changed` on `GET /changes` when an act
  is recorded or an order moves at their site (`live.rs`,
  `client/domain/changes.ts`, `useChanges`). The walk merges the fresh read
  in place (`merge` in `walk.ts`). `/live` was already the liveness probe.
- Items can be narrowed to what has been done as well as what needs doing:
  Measured, Photographed, or both (`GET /items?has=`), the other way round
  from `needs`, family figures and pictures counting as they do there.

## Known limits

- Firefox finds faces about 13 times slower than Chrome or Edge: 78 s against
  6 s per photo on this PC.
- Search has not been timed on the real catalogue.
- Moving a look leaves its "same as" sides on the old subject.
- The bin's reach has no screen. Set it with `POST /places/{id}/reach` or wait
  for the Warehouse setting.
- `spork-restore` is proved by the round-trip test (`backup_http.rs`), which
  now restores into a database with other ids for the drawn rows (D194).
- A backup taken before D194 has no catalogue. One with a measurement taken
  in a stated presentation is refused, naming the column. Take it again, or
  give the empty database the source's presentation ids before restoring.
- People (D205): an administrator adds people with a first password, gives them a role and takes them out, which ends their sessions. Only two roles exist, administrator and operator. Finer roles are still Q176.
- A non-box item can be photographed but not trimmed: its photo is used as
  taken. A white-background cutout is the planned next step for those.

## Next

1. Done 2026-10-03: the full server suite passed on a fresh database (142),
   and a real backup was restored on the Mac (D194 came of it).
2. Measure what the packing worklist lists (D197), and see the suggestion
   on real orders.
3. Reach on the Warehouse screen and rack face, and a forklift mark on the
   item list's bin.
4. A completed sheet for a list: each item with its own and its carton's
   figures, as CSV.
5. Apply the real layout, so A to D can say which levels are reachable.
6. Cut a non-box item's photo out onto white at the computer.
