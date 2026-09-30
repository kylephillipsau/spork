# Handoff: the bridge, orders, items, packing, and the warehouse

Written 2026-09-28, updated 2026-09-30. Read this before continuing on the
client's UI work, packing, items, or anything spatial.

The short version:
- Picks made on the WMS handheld reach Spork (D172). A userscript feeds them
  in, and it stays out of sight unless something is wrong.
- Every order and every item has its own page, and there is an item list to
  work from.
- The packing bench knows where the site packs and whose stock it holds. It
  ships a product in its own carton.
- The warehouse has a layout: **places**, drawn relative to each other and
  never measured (D173). Inventory › Warehouse drafts it from the bin list and
  lists each place's bins beside a plan of the site. A scanned bin lands on the
  face of the rack that holds it.
- The local instance runs on the business's own data, read from NetSuite
  exports.

**The 3D view is next, then the plan editor.**

Everything below is committed and pushed. The working tree was clean when this
was written.

---

## Before you start

**Toolchain in Git Bash.** Cargo builds need the OpenSSL environment that
`scripts/local.ps1` sets in PowerShell:

```sh
export OPENSSL_DIR='C:\Program Files\OpenSSL-Win64' \
       OPENSSL_LIB_DIR='C:\Program Files\OpenSSL-Win64\lib\VC\x64\MD' \
       OPENSSL_INCLUDE_DIR='C:\Program Files\OpenSSL-Win64\include' \
       CARGO_HTTP_CHECK_REVOKE=false \
       PATH="/c/Program Files/OpenSSL-Win64/bin:/c/Program Files/PostgreSQL/18/bin:$PATH"
```

The Rust package is `spork-server`. To run a `.ps1` script from bash, call
PowerShell explicitly:
`powershell -ExecutionPolicy Bypass -File scripts/local.ps1 setup`.

**Do not run `cargo fmt` over the crate.** The tree is not rustfmt-clean, and a
crate-wide format once reformatted about ninety unrelated files. Format only
what you touch, if at all.

**Tests on a fresh database.** The suite is not re-runnable against a database
it has written to:

```sh
psql -h localhost -p 55432 -U postgres -c "DROP DATABASE IF EXISTS spork_fresh WITH (FORCE)" -c "CREATE DATABASE spork_fresh"
export DATABASE_URL=postgres://postgres:spork@localhost:55432/spork_fresh
scripts/verify-migrations.sh && scripts/migrate.sh --baseline
cargo test --workspace --no-fail-fast
```

Use `--no-fail-fast`, because without it cargo stops at the first failing
binary and hides the rest. **The full suite passes.**
- Four tests used to fail in a full run, because they depended on which suites
  ran first. On 2026-09-30 they were changed to assert against their own data.
- The walks that took the first open line in the queue now take one with stock
  behind it. A fulfilment picked in NetSuite has none, by design.

**The server's integration tests are one program**, `crates/server/tests/it`,
one module per file, so a change to the server links once rather than forty
times. Tests from different files never run at once (`common::file_gate`),
which is the isolation the suite was written for. Run one file with a filter:
`cargo test -p spork-server --test it pack_walk_http::`.

On this machine:
- a fresh database takes about 20 s;
- rebuilding the tests after a server change takes about 30 s;
- the whole suite takes about three and a half minutes.

Two things made it that fast (50ea84c): rust-lld links on Windows
(`.cargo/config.toml`), and debug builds carry only line tables, none for
dependencies. `migrate.sh` and `verify-migrations.sh` no longer launch psql per
row or per file.

**Client gates.** `npm run verify` runs typecheck, tests, contract and laws.
Run `npm run build:review` before `npm run render`. The render reads
`dist-review/`, so without the build it renders a stale one. Screenshots land
in `client/.render/`.

**The local instance.** `scripts/local.ps1 setup` then `start` serves
`http://localhost:18080`. Since 2026-09-30 the working `spork` database holds
the business's own data, read from NetSuite exports and never written back:
- the item master;
- the prepack list's box types and carton measurements;
- one warehouse's bins;
- its inventory balance.

[local.md](./local.md) has the importers and their flags. A draft of the real
layout has been previewed, not applied: apply it from Inventory › Warehouse
once it reads right. Real codes stay out of the repository, as everywhere else.

---

## What landed

### The warehouse screen (52c09e3)

`/warehouse`, under Inventory, replaces Settings › Layout.
- **Places** on the left, each under the one it is inside, with "Not on the
  layout" last. It is a list rather than a table, because a table in a column
  that narrow stacks.
- **The site's plan** on the right. Clicking a place on the plan chooses it,
  as the list does.
- **The chosen place's bins** below the plan, with a search across the site.
  Each bin shows:
  - its cell in the rack's own words ("bay 03, level 2");
  - what NetSuite's last balance put on the shelf;
  - what Spork's ledger holds there.

  A row opens the rack face.
- The choice and the search live in the query string (`?place=`, `?q=`),
  rewritten in place.
- Drafting, its preview and the empty state are unchanged.

The server side:
- `GET /layout` gained `plan`: every place on the site as a `PlanShape`
  (corners on the site, `z`, `height`, `nesting`, `solid`). Positions are
  composed when read, as before.
- `GET /bins` lists bins by `place`, `unplaced=true` or `q`, 500 at a time, in
  code order, with the total.
- NetSuite's report is totalled once for the site and joined on. It has no
  index by bin, and a lookup per bin took 760 ms on the real bin list. It now
  takes 36 ms.

### Items: the page, the list, and the loaders

- **The item page** (ab8cefc): `/items/:id` and `GET /items/{id}`. It shows:
  - what the item is, its photo and its family (D108);
  - its carton and measurements, its own or its family's;
  - where it is, with NetSuite's report kept apart from Spork's own ledger
    (migration 86).

  An item scan lands there, and order lines link to it.
- **The item list** (c78e238): `/items` and `GET /items`. It searches code,
  description and barcode, 50 at a time. It can be narrowed to "in stock here"
  and to "needs measuring" or "needs a photo": the list to work from when
  recording photos, measurements and weights. A figure copied from a list is
  not a measurement, so the prepack list's cartons still count as needing it.
- **The prepack importer** (8971567) reads the export's carton notes after the
  code, such as `(CTN, x25)`, and the one-letter colour families.
  - A product's carton is never made a box type.
  - `--not-a-box` leaves out names a person knows better.
  - Its acts are keyed by NetSuite's record id, so a re-export is a replay.
- **The stock loader** (8ebd0f4) sums a shelf's lots into one balance. It skips
  and counts negative balances instead of aborting. `import_stock` is its
  terminal front end, and `import_bins --only` loads one warehouse.

### Packing: where a site packs, whose stock, and whole cartons

- **Migration 97** (7860049): `site.pack_location_id`. The bench used to
  guess the location from the first code and the first staging location. It
  now reads only this one, and says so when a site hasn't set it or hasn't set
  an owner (migration 95). Workspace has a "Packing at <site>" card:
  - `POST /workspace/sites/{id}/pack-location` names a location by code, and
    makes a staging location if there is none;
  - `POST /workspace/sites/{id}/owner` takes `business`.
- **Migration 98** (ac6b3f3): `package.item_packing_config_id`. A package is a
  box type or a product's own carton, never both. A line that fills whole
  cartons offers "2 own cartons of 10". The press makes, fills and seals each
  carton at the pack location, with parts named by `acts::partOf` so a retry
  replays. A listed weight is shown as listed, never as an expectation.
- **The site's clock** (3e3f510): "due today" and "due tomorrow" are read in the
  site's time zone, not UTC. Before this, a site ahead of UTC read tomorrow's
  promises as due today every morning.

### D173: a place for every bin

Written up in [domain-model.md](./domain-model.md) (D173), with a plain account
in [layout.md](./layout.md). Migration 96. J76, J77 and J78 are new in the
register.

- **Places**: `place` rows, each a box inside a parent place (or the site), in
  cells of the parent, turned by any whole degree, walk-through or solid, with a
  free name. A walk-through place may have an outline (an L-shaped building). A
  place with a grid is a rectangle.
- **Every bin sits in one cell** (`location.place_id` and `slot_bay`,
  `slot_level`, `slot_row`, `slot_position`), and a unique index keeps one bin
  to a cell. The six millimetre columns on `location` are dropped.
- **Positions on the site are never stored.** `layout::compose` and
  `layout::frames` work them out from the chain of parents when read.
- **Naming patterns** (`C-{bay:02}-{level}`, `{level:A}`) drop bins into cells,
  with the first bay number and a step (2 for odd-only or even-only sides).
- **Drafting from the bin list** (`POST /layout/draft`, previewed unless
  `?apply=true`): bins go into places already drawn when a pattern names them,
  and the rest are grouped by the shape of their codes into new places laid out
  in rows. A bin is only placed where its pattern spells its code exactly, and
  nothing already in a cell moves.
- **Reads**: `GET /bins/{id}` (a bin, its cell and its place),
  `GET /places/{id}`, `GET /layout` (now with the site's plan) and `GET /bins`.
- **Client**:
  - a scanned or searched bin code goes to `/bins/:id`, the rack face: the way
    in as chips, bays across and levels up with the labels as the rack prints
    them, the bin's cell lit, and a plan of the building;
  - `/places/:id` shows what is inside a place;
  - Inventory › Warehouse drafts, lists and finds (above).
- The first version of D173 (measured racks, rack and floor CSV imports) was
  replaced before release. Its commit `d5ac37a` is in history, superseded by
  `273a246`.

### The interface, researched

The user asked for the layout to be usable by a warehouse worker with little
computer experience, and pleasant for power users, "like an Apple product". The
research report is [layout-interface-analysis.md](./layout-interface-analysis.md).
Its working notes stayed on the machine it was written on. Its recommendation,
which the work above follows:

- **One place model, three depths, no modes.**
  - *Find and check* (everyone): a scan opens the rack face.
  - *Put it right* (trusted floor staff): correct which bin is in which cell by
    scanning where it really is, never by dragging on a handheld.
  - *Shape the layout* (desk): the editor.
- **No global "advanced" switch.** Logic Pro removed its switch in January 2026.
  Depth comes from role.
- **2D answers "where"; 3D confirms.** Editing happens on the 2D plan. The
  handheld never opens in 3D.
- **The editor:**
  - It opens on a draft, never an empty canvas.
  - You edit inside one place at a time, with a breadcrumb out.
  - Constraints replace error messages: children stay inside, solid places stop
    flush, and deleting sends bins to an Unplaced tray.
  - An inspector follows the selection, with one "More options".
  - The naming pattern reads as a sentence, with live match counts.
  - Undo is unlimited and named, and anything that moves bins between cells is
    previewed.
  - Every command has a visible home; shortcuts and search only duplicate it.
- **Test on the floor first.** Time workers finding a bin with the rack face
  against the bare bin code before polishing further.

Scanning the building with a phone is out. Apple's RoomPlan is designed for
rooms up to 15 m × 15 m and 3.6 m high.

### The order page

`GET /orders/{order_id}` returns an `OrderView`: what the search answers per
order, plus its lines with ordered, committed, picked, picked elsewhere, packed
and despatched. In the client it is `/orders/:order`, reached from:
- the dashboard;
- the orders list's drawer ("Open order");
- links.

`FulfilmentSummary` gained the fulfilment's own `reference`.

### D172: a pick made elsewhere is reported, not moved

Written up in [domain-model.md](./domain-model.md) (D172), with J74, J75 and
the amended J56 in the invariant register. Migrations 93 to 95.

- `external_pick` is a fact. Its level is folded into
  `fulfilment_line.external_picked_quantity`.
- A handover (`POST /handovers`) moves the goods to where they were put, with
  reason `handover`. J75 keeps the amount handed over at or below the amount
  reported.
- Packing progress is `greatest(picked, boxed)`. The bench shows "picked
  elsewhere" with its provenance.

### The Spork Bridge userscript (0.3.0)

This lives in the separate `warehouse-scripts` repo as
`public/spork-bridge.user.js`. **A push to its main deploys to everyone who has
it installed.**

- **Read-only against NetSuite.** It reads Picked item fulfilments at one
  location with SuiteQL through the page's own `require(['N/query'])`, and
  POSTs each one to `/api/import/fulfilment?apply=true` with an import token.
  An item fulfilment is only resent when its `lastmodifieddate` changes.
- **Out of the way unless something is wrong** (0.2.0). While syncing works,
  nothing shows on the page. The last sync is the first line of the Tampermonkey
  menu, and choosing it syncs now. When a sync fails, a small red dot appears in
  the bottom-left corner. Hovering says why, and a click retries.
- **One tab syncs, by Web Lock** (0.3.0). Every NetSuite screen is a new page,
  so the old lease in storage stayed with the page just left. Now:
  - `navigator.locks` elects the syncing page, and the browser hands the lock on
    the moment that page goes away;
  - a second lock keeps "sync now" from running beside the timer;
  - the status reaches every tab through `GM_addValueChangeListener`.
- It uses `anonymous: true`. Spork reads a session cookie before a bearer
  token, so a cookie sent alongside the token gets the import refused.

---

## Next

1. **Apply the layout on real data.** Open Inventory › Warehouse, preview the
   draft, and read it. Families the draft gets wrong are the first thing to
   fix. Bins no pattern fits wait in the tray to be placed by hand.
2. **The 3D view** beside the plan on the Warehouse screen (below).
3. **Record photos, measurements and weights**, working from the item list
   narrowed to "in stock here" and "needs measuring" or "needs a photo".
4. **Test the rack face on the floor** against the bare bin code, as above.
5. **"Something's wrong here"** on the rack face: a worker scans the bin where
   it really is and taps its cell. Trusted roles apply it, and others raise a
   flag for the office.
6. **The plan editor** (below).
7. A history of layout changes (who moved what, and when), and roles for who
   may edit. Until roles exist, anyone signed in can draft.

Deferred and not forgotten:

- a per-site option to cancel instead of raising a finding when an item
  fulfilment changes;
- an owner other than the business itself. Workspace sets only `business`, so a
  site holding a customer's stock can't say so yet;
- NetSuite write-back;
- MachShip.

---

## The 3D view and the editor: the plan

**Stack.** three.js with React Three Fiber and a small subset of drei, on WebGL,
**lazy-loaded as its own route chunk** so the rest of the app pays nothing. The
main chunk is already about 570 kB in the review build, and Vite warns. An imperative scene
class owns the meshes and R3F only hosts it, which keeps a hand-written
renderer possible later. Scene data sits in typed arrays outside React, redrawn
on demand (`frameloop="demand"`).

Why not the alternatives:
- Babylon is heavier, and its React binding has one maintainer.
- PlayCanvas's binding is still at 0.x.
- deck.gl is built for maps.
- Needle is commercially licensed.

**What it draws.** `GET /layout`'s `plan` is already the input: every place's
footprint on the site, how far up it starts and how tall it is. That is the
same data the Warehouse screen's 2D plan draws. Solid places are blocks and
walk-through places are floor.
- **Selection.** The pane shares the screen's selection (`WarehouseDesk.chosen`),
  so choosing a rack in the list, on the plan or in 3D is one choice.
- **Controls.** Orbit and zoom only. Nothing is edited in 3D.
- **Picking.** A ray against the places' boxes, which are few enough to test
  directly.
- **Cells are not in the plan.** Drawing a rack's bays and levels needs its
  grid. `GET /places/{id}` has it per place; a site-wide view would need it
  added to `plan`.

**The editor** is the 2D plan with the 3D pane beside it, following the
research above.
- Its first commands: drag to move, drag a handle to resize, rotate with
  quarter-turn snapping, add a box or an outline, add a grid, and set the
  naming pattern.
- Changes that move bins between cells are previewed.
- The Unplaced tray holds bins nothing names.
- Writes are a person's act on a session, recorded with who and when.

**Later.**

- Colour-by layers computed by the server: fill, findings, count age, and the
  difference between reported and ledger stock. `GET /bins` already carries
  both numbers per bin.
- Pick paths, which treat solid places as obstacles.
- Stock inside places, updated live.
- A handheld mini-map.
