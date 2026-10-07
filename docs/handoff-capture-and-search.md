# Handoff: capture, crops, search, backups and picking

Written 2026-10-02, brought up to date 2026-10-07. Read this first. The
earlier handoff, [handoff-orders-bridge-3d.md](./handoff-orders-bridge-3d.md),
still covers the toolchain, tests, layout and packing.

## State

- Spork `main` is pushed through D231: D183's amendment, picking (D230,
  D231), its printed tickets and the walk on the 3D map.
- `warehouse-scripts` is pushed. **Spork Bridge 0.9.1** is published. It sends
  NetSuite's open orders every five minutes (D231), without customer notes:
  the order's `custbody_internalcustomernotes` is NOT_EXPOSED to SuiteQL (see
  Known limits). On 2026-10-07 the first load brought 105 orders and 785
  lines; every item was known, and 752 lines had a bin with stock.
- The database is at migration 128: 127 adds `cut_of`, 128
  `reported_order_line`. After pulling, rebuild the release server, restart,
  then build the client (`local.ps1 start` migrates but never rebuilds, and a
  client build goes live at once).
- 2026-10-07: the full server suite passes on a fresh database except
  `picking_http::a_trolley_pick…`. It passes alone on a fresh database, but
  fails after other tests have used the walk's fixture line. Order-dependent,
  not yet looked into.
- 2026-10-06, on a fresh database: every migration up and down; the server's
  unit tests (283) and integration tests (154) all pass; the client's tests,
  laws, contract and render (133 fixtures) pass.
- The invariants: S7 and S10 fail, as they have since migrations 118 and 119
  (a trigger that is not a projection's, a jsonb column). After the
  integration suite has written to the database, J1 also finds two GLOVE-M
  stock cells that disagree with the movement ledger; on the fresh database
  it passes. Not yet looked into.

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
- D207. A pick is kept on the device (`localStorage`, `spork.outbox.picks`)
  before it is sent, and sent again until the server has it (`domain/outbox.ts`,
  `app/outbox.ts`). The walk counts kept picks as taken (`overlay` in
  `walk.ts`). Picks only; other acts still live in the screen's memory.
- D208. Inventory › Bin map (`/map`): every placed bin as a box in its cell
  (`cellsOf` in `blocks.ts`, `GET /layout/bins`), coloured by what's here or by
  reach (`layers.ts`). A search flies to a bin's face, and the chosen bin's rack
  stays solid while the rest fade. The card sets the rack's reach.
- D209, migration 119. Warehouse › Edit layout (`/warehouse/edit`): drag,
  nudge, turn about the middle, resize, add (wall, column, dock, packing
  station, area) and take away places, with undo, saved as one act against the
  layout's `version` (`POST /layout/edit`). Each change is kept in
  `place_change`. Grids are not editable. The arithmetic is `edit.ts`, tested
  against the server's composition.
- D210, migration 120. `site.cell_mm`, set once (`POST /layout/scale`); the
  editor's "Measure in metres" sets 1000. Then: size racks from a bay width,
  side depth and level height; set chosen racks out in a row; type a place's
  room to its neighbour or the wall. There's no floor plan to copy, so the
  user is measuring the floor.
- D211. The picking walk comes in route order (`routing.rs` pure: floor, A*,
  Dijkstra, Held-Karp, insertion with 2-opt and Or-opt; `walk_route.rs` glue;
  `picking_list.rs`), from the packing bench when it is on the layout, with
  the typed order's length beside it. The bin map draws today's walk. A bin
  from the tray can go on the plan as a spot (`spots`). At Melbourne the
  ledger holds no stock in bins, so the real walk has nothing to route yet;
  see D211's last paragraph.
- D212. NetSuite keeps the shelves; Spork keeps the floor's work and what is
  in hand. The Bridge (0.5.0, in `warehouse-scripts`) loads NetSuite's
  inventory balance every five minutes into the same feed as the manual
  import, and the bin map says how old it is (`MapBins.reported_as_at`).
  Next: the walk routing to NetSuite's bins, a pick into custody, and the
  write-back to NetSuite.
- D213. A round thing (a bucket, can, jar; `packaging_type.round`) is
  measured across its top and base, its height and the straight part under
  its rim (`diameter`, `base_diameter`, `top_height`), with the box it fits in
  recorded beside them. Its photos are side, lid, label, close-up. Not drawn
  as a bucket yet.
- D214. A cut is checked against the face as measured before it is kept
  (`lieOf` in `cut.ts`): Photos to crop confirms a matching face with one
  press, and sends one found a quarter turn out to the crop screen to be
  turned. The crop screen says how the corners lie beside the preview.
- D215. On an item's page, "Not here" on a bin NetSuite lists it in, and
  "Found it in another bin", each raise a finding for someone to put right in
  NetSuite (`listed.rs`, `POST /items/{id}/bin-flags`, migration 122). The
  page shows the open ones. An adjustment can't resolve one; accept it.
- D185, amended: "How many in it" is the whole carton's count, and the packs
  are worked out from it. A carton said the old way shows its inflated total
  when opened, which is how to find and fix one.
- D216. Items → Export: the list as asked, every row, as CSV or a workbook
  with each item's picture (`export.rs`, `GET /items/export`). An item's
  levels come from `capture::subjects_for_items`, which assembles each item
  with the item page's own `assemble`; `items::ListAsk`/`list_rows` are the
  list's filters and rows, shared by the page and the export.
- D217. Export → Capture sheet (PDF): the printed capture sheet's exact
  layout (`sheet.rs`, metrics in `pdf_metrics.rs`) with recorded figures in
  the boxes. Unit and Supplier Part No. are `reported_item` (migration 123),
  loaded by Spork Bridge 0.7.1 via `POST /import/item-details` from the
  custom fields `custitem_packunit` and `custitem_alternativecode`, probed in
  the real NetSuite (its standard `saleunit` is refused in SuiteQL). NetSuite
  also has `custitem_length`, `custitem_width`, `custitem_height`,
  `custitem_eachpercarton` and friends: where a write-back would go.
- D218. Each item has a unit, the level that is one in NetSuite: said in
  Spork (`item_unit`, `POST /items/{id}/unit`) or taken from its Pack Unit by
  `unit_level_of` (migration 124, view `item_unit_level`). The item page leads
  with it and keeps offers (an unsaid carton, a single product inside) as
  quiet lines; the bench counts in NetSuite units (`bench::per_level`); the
  capture sheet and exports use the unit's figures.
- D219. **Move…** on an item's card moves everything recorded on it to
  another of its levels (`refile.rs`, `POST /items/{id}/refile`): events
  mirrored with `derived_from_event_id`, figures copied and retracted where
  they were, photos refiled with their cuts and marked moved. Refused onto a
  card with records of its own. The items list's needs/has filters count the
  unit's figures.
- D220. A rack is numbered from either end of its front (`place.from_right`,
  migration 125). The draft's preview asks (**From the right**, **Every rack
  from the right**), and the plan editor's **Numbered from** turns a rack
  already made round: its bins keep their names, each moved to the mirror of
  its column. Cells stay counted from the front's left, so only labels change.
- D221. On the bin map an empty bin is a hollow box (`MIX.empty.hollow`, read
  by the scene and the legend alike), and the chosen bin's card lists what is
  on it as the packing bench does: `ItemLine` (photo, code that opens the
  `ItemDrawer`, description), NetSuite's count and Spork's. The card reads
  `GET /bins/{id}`, which now carries `contents`; `pictures::of` is the one
  read for a list of items' pictures.
- D222. **Move…** can take a card's records to another item's card:
  `to_item` on `POST /items/{id}/refile`, the same copy-and-retract. The
  dialog finds the item by its code (`GET /resolve?expect=item`)
  and offers its cards. Move… now shows on any card with its own records,
  so an item with one card (a kit sold by the each) can be put right too.
- D223 (migration 126). A kit is ordered and its parts are packed, loose. The
  bridge sends `item_type` and `kit_line` on each fulfilment line;
  `importing::orders::roles` reads them into a `Role` per line. A kit's own
  line is loaded with nothing to pick and no picks recorded, so it is on the
  order and nowhere in the work. A part's `order_line.kit_line_id` names its
  kit, and the bench says "Part of KIT × n" under it. A kit line committed
  before the bridge said stays committed and is reported in `differs`
  (`field: "kit"`).
- D224. What an order leaves as is freight (`client/app/outbound/pack/freight.ts`):
  each parcel's count, outside size and weight, read three ways. The
  suggestion scores candidate plans by an objective (fewest parcels, the
  default, or least chargeable weight at 250 kg/m³) and keeps the cheapest;
  the bench shows each parcel's size and weight and the loose goods' weight;
  "For the booking" lists the parcels in whole cm and kg with Copy. A box
  says what it weighs empty (`POST /package-types/{id}/empty-weight`,
  Workspace › Boxes › Empty). The packing plan opens in 3D, remembered per
  browser (`app/common/remembered.ts`), with cartons' cut sides on them.
  Copy works over the LAN's plain http too (`app/common/copy.ts`, which
  Import tokens uses as well).
- Light mode's highlighted row in a menu or a select is `--ui-accent-soft`,
  now `#e0e7ff`, strong enough to see on white.
- D225. Photographing, every side has Choose beside its camera button, for a
  photo already on the phone, and the next-side button has From photos, which
  takes several at once and fills the sides still to take in walk order
  (`FromPhotos` and `FilePress` in `ItemProperties.tsx`).
- D226. Weighing, measuring or photographing an item's own pack with no case
  pack on file says one first, its counts unsaid, as the carton already did:
  an item sold by the box could not have its box measured.
- D227. Enter in the header's search on what matches more than one thing opens
  `/search?q=` (`app/scan/SearchPage.tsx`), every match grouped as the header
  groups them (`app/scan/found.tsx`); one match still opens it. Search fields
  neither capitalise nor autocorrect on a phone.
- D228. An item's page lists its family with what is on each one's cards, and
  Match… copies the cards ticked from one of them (`POST /items/{id}/match`,
  `Family` and `MatchDialog` in `ItemProperties.tsx`). It shares the copying
  with Move… (`refile::file_again`), copying rather than moving: nothing
  retracted, nothing marked moved, the copies keeping when they were taken.
  A pack's or carton's size moved or copied onto an each is said to be as
  supplied; the move to another item's each (D222) had left it unsaid, which
  J72 reports.
- D229. A carton's Holds has Change (`HoldsForm` in `ItemProperties.tsx`),
  which puts a wrongly said count right on the case pack in force
  (`correction` on `POST /items/{id}/carton`) rather than starting a new one
  from today, so what was measured under it stays.
- Items can be narrowed to what has been done as well as what needs doing:
  Measured, Photographed, or both (`GET /items?has=`), the other way round
  from `needs`, family figures and pictures counting as they do there.
- D183, amended (migration 127). A side said to look like another ("Same as
  front") now shows on the packing plan's 3D boxes as it does on the item
  page. The cut a photograph shows is one SQL function, `cut_of(image)`,
  joined by the item page, the bench's sides and the list picture; the bench
  and the picture had looked only for a cut of the row itself. In 3D, both
  scenes put a photo on a face with `faceTexture` (`app/items/box3d.ts`).

**Picking (2026-10-07)**
- D231, migration 128. NetSuite's open orders are a report,
  `reported_order_line`: goods lines with something left to pick
  (committed − picked + shipped), each with its order's ship-to, Picking
  Instructions (`memo`), Internal Customer Notes
  (`custbody_internalcustomernotes`) and Art No. (`custcol_artno`). Bridge
  0.9.0 sends them every five minutes, and "sync open orders now" sends them
  at once. Never work: picks are still recorded in NetSuite (D212).
- `GET /sites/{id}/to-pick` (`to_pick.rs`) looks up a pasted batch by the
  orders' digits. It gives each order's state, and each line's bins by the
  item list's rule (`items::piles`, racking only), the batch drawing bins down.
- D230. `pick_groups.rs` shares a batch of up to 40 waiting orders by people
  picking and most orders a trip, with "walk to a shelf once" as a setting,
  over `walk_route::distances`.
- Outbound › **To pick** (`ToPickPage.tsx`, `useToPick.ts`): paste, states,
  share out, Print, and **Walk** on each trip (`WalkView.tsx`). Walk shows the
  stop on screen and highlights its bin on the 3D map (`Map3D`), with the
  trip's A* route on the floor. The trip is kept in `localStorage` while it's
  walked.
- `GET /print/pick-tickets/{site}` (`web/tickets.rs`): one order to an A4
  landscape page with a Code 128 barcode, and a walk sheet per trip with
  `walk=true`. The approved proof is the artifact
  https://claude.ai/artifact/HgDWBqk783d9zDrudcuPhw.

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
- Match… (D228) copies figures and photos only. What a card is packed in,
  whether it ships as it is and which way up it goes are not copied; say them
  on the card.
- A carton count put right (D229) keeps no record of the wrong count: the case
  pack's row names the act that corrected it.
- The search page (D227) shows no pictures; `Found` carries none.
- Freight is scored by parcel count or chargeable weight (D224). No carrier's
  rates are known yet, so neither is a price.
- Routes and trips are as good as the layout. The draft's racks still number
  from the left until Edit layout › Numbered from › The right end is saved
  (D220), and the floor isn't measured (D210), so a trip's minutes are
  estimates.
- A sales order's kit lines haven't been seen with real data. A kit with
  none of its parts on the order is printed as itself.
- Where Spork's ledger and NetSuite's report both hold an item in one bin,
  the bin is listed twice. That doesn't happen at Melbourne (D212).
- Tickets print no customer notes yet. The order's Internal Customer Notes is
  shown from the customer and not stored on the order, so SuiteQL refuses it
  (NOT_EXPOSED). The customer's own field is needed (Show Internal IDs on a
  customer record), then the Bridge reads it through `JOIN customer`.
- Picks are recorded in NetSuite: the walk keeps its place only in the
  browser that walks it. Recording picks in Spork waits on the NetSuite
  write-back (D212).
- A batch of more than 40 waiting orders is listed but not shared out
  (`to_pick::MOST_PLANNED`).

## Next

1. Done 2026-10-03: the full server suite passed on a fresh database (142),
   and a real backup was restored on the Mac (D194 came of it).
2. Measure what the packing worklist lists (D197), and see the suggestion
   on real orders.
3. Reach on the Warehouse screen and rack face, and a forklift mark on the
   item list's bin.
4. A completed sheet for a list: each item with its own and its carton's
   figures, as CSV.
5. Measure the floor and build it in Edit layout (D210): building size,
   each rack make, row starts and aisles, and which aisle each rack's 01
   faces. Then set each rack's reach on the bin map.
6. Cut a non-box item's photo out onto white at the computer.
7. Freight by price: carriers' rates as a third objective beside fewest
   parcels and least chargeable weight (D224), chosen in configuration.
8. Picking on the floor, from 2026-10-08:
   - paste a real batch;
   - print tickets on the real printer;
   - scan a ticket's barcode with the gun;
   - walk a trip on the device carried.
   Then: customer notes from the customer's field, the first kit on a sales
   order, and measuring the floor (5) so routes and minutes are real.
