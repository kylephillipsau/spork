# Handoff: the bridge, orders, items and their photos, packing, and the warehouse

Written 2026-09-28, updated 2026-10-01. Read this before continuing on the
client's UI work, packing, items and their photos, or anything spatial.

The short version:
- Picks made on the WMS handheld reach Spork (D172). A userscript feeds them
  in, and it stays out of sight unless something is wrong.
- Every order and every item has its own page, and there is an item list to
  work from.
- The packing bench knows where the site packs and whose stock it holds. It
  ships a product in its own carton.
- The warehouse has a layout: **places**, drawn relative to each other and
  never measured (D173). Inventory › Warehouse drafts it from the bin list and
  lists each place's bins beside a plan of the site, with the same site in 3D
  beside the plan. A scanned bin lands on the face of the rack that holds it.
- An item's weight, size and photos are recorded at the item (D174), on a
  phone or at a desk. A photo goes up as WebP (D175), is cut to its face where
  a model in the browser finds it and a person checks it (D176, D177), and is
  drawn on a 3D box of the item.
- The local instance runs on the business's own data, read from NetSuite
  exports, and a phone on the WiFi can use it.

**Applying the real layout is next, then the plan editor.** The photo work is
in use; what is open on it is under Next.

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

**Migration 100** (a photo's cuts, D176) is the newest, and the working `spork`
database is at 100. `scripts/local.ps1 start` applies anything pending before
it starts the server.

**Tests on a fresh database.** The suite is not re-runnable against a database
it has written to:

```sh
psql -h localhost -p 55432 -U postgres -c "DROP DATABASE IF EXISTS spork_fresh WITH (FORCE)" -c "CREATE DATABASE spork_fresh"
export DATABASE_URL=postgres://postgres:spork@localhost:55432/spork_fresh
scripts/verify-migrations.sh && scripts/migrate.sh --baseline
cargo test --workspace --no-fail-fast
```

Use `--no-fail-fast`, because without it cargo stops at the first failing
binary and hides the rest. **The full suite passes**: 132 integration tests on
a fresh database on 2026-10-01. A database it has already written to fails
three of them, which is the warning above, not a fault.
- Four tests used to fail in a full run, because they depended on which suites
  ran first. On 2026-09-30 they were changed to assert against their own data.
- A fifth, `baseline_read`, failed whenever `pack_walk_http` ran first: its
  photograph test left a carton measurement behind. It removes its rows now
  (a8580a0).
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

**npm behind the proxy.** The network re-signs TLS, so in Git Bash npm needs
the Windows certificate store: `NODE_OPTIONS=--use-system-ca npm install …`.
`scripts/local.ps1` sets it for PowerShell. Never turn off `strict-ssl`.

**Client gates.** `npm run verify` runs typecheck, tests, contract and laws.
Run `npm run build:review` before `npm run render`. The render reads
`dist-review/`, so without the build it renders a stale one. Screenshots land
in `client/.render/`. `npm run build` also fetches the face-finder's model the
first time (14 MB, from a pinned commit, checked); the review build and the
render gate do not need it.

**Playwright has WebKit as well as Chromium.** WebKit 26.5 (`webkit-2336`, with
its `winldd-1007` helper) was downloaded by hand into `%LOCALAPPDATA%\ms-playwright`
the same way Chromium was, so the iPhone's engine can be tested from here:
`require("playwright").webkit`.

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

**A phone on the WiFi.** The user runs `local.ps1 start -Lan`, which also serves
the PC's address on the WiFi (it prints it), and photographs boxes with an
iPhone. The WiFi is set to Private and the firewall rule is in (2026-10-01).
- Phones sign in with a password: passkeys need HTTPS or localhost.
- That page is plain HTTP, which is not a secure context. Nothing in the client
  may need one (`npm run laws` refuses `crypto.randomUUID` and `crypto.subtle`),
  WebGPU is unavailable, and the face-finder gets one thread.
- **The server serves `client/dist` from disk, so `npm run build` changes the
  live app at once.** When the client needs a new endpoint or migration,
  rebuild and restart the server first, or the phone gets a client its server
  cannot answer. That happened once on 2026-10-01.
- To swap the release binaries while it runs, rename the running `.exe`s in
  `target/release`, build, then restart: a few seconds down.

---

## What landed

### An item's properties, at the item (D174)

Weigh and Capture are gone as screens. An item's weight, size, photographs and
labels are seen and changed at the item:
- **The item list** filters to *needs weighing*, *needs measuring* or *needs a
  photo*, orders by code, *most ordered first* or *in walking order* (by the
  bin holding most of it), and marks what is recorded per row. A row opens the
  item in a drawer with Previous and Next. `/weigh` and `/capture` redirect to
  those lists.
- **The item's page** (where a scan lands) and the drawer draw the same thing:
  a card per subject (carton, each, family carton, parts) with Weigh, Measure,
  Photograph and Barcodes.
- **The packing bench**: an item code in *To pack* or in a carton opens the
  same drawer.
- `POST /observations` takes `photographs: true` for a look with no figures,
  so a measured item can be photographed on its own.
- `GET /items/{id}` carries `subjects` and `photos`; `GET /items` takes
  `needs=weighing` and `order=demand|walk`, and answers `weight`, `size`,
  `demand` and `bin_code` per row. It picks the page before looking anything
  up for it: 0.2 s on the real item list, from 1.5 s.
- One implementation (DRY): `useItemProperties`, `ItemProperties`,
  `ItemSummary` and `ItemDrawer` in `client/app/items/`. Opening an item's
  properties anywhere is `<ItemDrawer itemId={…} onClose={…} />`.
- Figures are read from `observation_current`, which the scheduler rebuilds a
  few seconds after a write, so the drawer re-reads the item over the next few
  seconds. **Run the scheduler** (`local.ps1 start` does) or recorded figures
  never appear.
- **A box is photographed as a box.** A subject not recorded as having no box
  shape is asked for its six sides and its label, and drawn as a 3D box of its
  own photos: its measured size (a cube until measured), turned by dragging,
  blank sides named. It turns to each side as it is taken. A thing with no box
  shape is asked for one photo and its label. Same faces as before (D132), so
  nothing new is stored. The code is `box.ts` (pure, tested), `box3d.ts`
  (`BoxScene`) and `BoxView.tsx`, which is lazy-loaded.

### Photos go up as WebP (D175)

Every photograph is converted on the phone before it is sent:
- turned the right way up;
- at most 4096 px on its longest side;
- WebP at quality 85;
- no camera metadata, so no location.

One place does it, `client/domain/webp.ts`, called from `api.photograph`.
- iPhones can't make WebP from a canvas (they hand back a PNG). They get
  libwebp in WebAssembly, fetched on first use, at effort 2: about half a
  second for a 12-megapixel photo.
- The server reads a WebP's size from its first chunk.
- On the box, a side shows its name until its photo arrives, and keeps it
  if the photo fails to load, rather than going black.

What followed, as the user chose: each photo is cut to its face, straightened
to the measured proportions, and kept beside the original (D176), with a model
finding the face first (D177).

### A phone on the WiFi can save (31d10a7)

Every act's id came from `crypto.randomUUID`, which browsers offer only on
HTTPS or at localhost, so every press on a phone over the WiFi failed with
"Request failed." before anything was sent. Ids are now made from
`crypto.getRandomValues` (`uuid()` in `client/domain/acts.ts`), and a
thirteenth law refuses `crypto.randomUUID` and `crypto.subtle` in the client.

### A photo cut to its face (D176, migration 100)

The crop screen opens straight after each photo, and from **Crop** on any
photo already taken, on a phone or at a desk:
- four corners to drag onto the face's corners, by how far the pointer moves
  (so a finger does not hide the corner it is moving), or with the arrow keys;
- the face straightened beside them as they move, and **Turn** for a face
  photographed sideways (the thick edge is its top);
- Save straightens the photo at full size, to the face's measured proportions
  when its size is known, as WebP, and keeps it as a cut beside the photo.

The cut shows on the 3D box, on the tile, and as the item's picture. A cut is
its own act, with its own person, so a desk can cut a phone's photos.
- `observation_image_cut`: the photo, the eight corner fractions, the cut's
  bytes, the act and its person. Newest wins.
- `POST /images` keeps bytes (the shared `images::store` that photographs use
  too), and `POST /observation-images/{id}/cuts` is the act.
- `client/app/items/cut.ts` is the maths (tested), `FaceCrop.tsx` the screen.
- The kit's `Dialog` takes a `width`.

### The face-finder (D177, a4d666f)

The crop screen places the corners itself, and the person checks them:
- a photo not cut before is looked at in its middle when the screen opens;
  a tap on the photo asks again at that point;
- an answer never moves a corner somebody has moved since asking, and a
  line under the preview says what the finder is doing.

How it works:
- SlimSAM-77, quantized, through onnxruntime-web (not transformers.js, whose
  version 4 brings `sharp` and `onnxruntime-node` along for nothing).
- `faceFind.ts` is pure and tested: the model's input, its outlines, and
  from outline to corners (the largest outline four corners describe well;
  the largest quadrilateral on its hull).
- `faceWorker.ts` is a module worker of its own. It encodes a photo once,
  so a tap is under a second, and runs one job at a time. `faceModel.ts` is
  the page's side. `desk.findFace` is how the screen asks, so fixtures stub
  it.
- `vite.config.ts` resolves `onnxruntime-web` with the
  `onnxruntime-web-use-extern-wasm` condition. Do not switch to the
  runtime's `proxy` option: in a Vite build its worker loads the app.
- `scripts/fetch-model.mjs` runs in `npm run build`: a pinned Hugging Face
  commit, SHA-256 checked, into `client/public/assets/models/` (gitignored).
- `assets.rs` sends COOP/COEP on the client's files, so localhost and HTTPS
  get several threads.

Measured on a box's real photos: Chrome at localhost, 6 s to the first answer
and 0.9 s for a tap. WebKit 26.5 over plain HTTP, one thread: 16.5 s on this
2019 laptop, 1 s for a tap. The real iPhone is untimed.

The server on this PC was restarted with the isolation headers the same day,
so the desktop at localhost gets threads. Test in WebKit as well as Chromium
(see Before you start).

### The app fills the window

The content area was capped at 1,440 px. It now fills the window; only forms
(`Page narrow`) and running text keep a line length. Side panels (the
dashboard's findings, the pack bench's cartons, the warehouse's places) clamp
to a panel's width on a big screen, the site plan and 3D pane grow with the
window's height, and fact grids no longer split a wide card in half. The render
gate also visits every desk screen at 2560 × 1440.

### Leaving a place out of the draft

The draft's preview has a tick box for each place it would make. The user found
that four families on the real bin list became "racks" that do not exist: lone
codes, and one pair of codes. Unticking a place leaves it out:
- its bins stay in the tray ("Not on the layout");
- it takes no row in the layout;
- the counts and the button ("Make 15 places") follow the ticks.

`POST /layout/draft` takes an optional body, `{ "leave_out": [names] }`
(`DraftRequest`), and the report lists `left_out`.
- A name the draft no longer proposes is refused ("preview it again"), so a bin
  list that changed since the preview cannot make what was left out under
  another name.
- Names are worked out the same way whatever is left out, so the preview's
  names are the apply's.

A left-out family is proposed again by the next draft, because its bins are
still in the tray. Remembering "this is no place" belongs with the editor, or
with marking the bins in NetSuite.

### Racks with two sides (migration 99)

The user's racks have bins on both faces, numbered round the rack: `E-01` to
`E-18` along the front and `E-19` to `E-36` back along the other side, `E-36`
behind `E-01`.

**The model.** A rack is **one place with two sides**:
- `place.sides` is 1 or 2, and `location.slot_side` says which side a cell is
  on; the unique cell index includes it.
- A cell's bay is its **column** from the front's left, the same on both sides,
  so the bin behind another is the same column on the other side.
- The back's labels are numbered round (`Grid::label_number`): column 1's back
  is the last bay.
- J76 also catches a bin on a side its place does not have.

It was first built as two places back to back (*Rack E front*, *Rack E back*).
The user asked whether that was right, and it was not: the back carried copies
of the front's numbering and position that nothing kept true, and nobody calls
half a rack a place. It was replaced before release.

**The screens.**
- The draft's preview has a **Two sides** tick box on each rack, labelled with
  its split ("01–18 front, 19–36 back"), and **Two sides for every rack**.
  `DraftRequest.two_sided` names them, and `DraftedPlace.split` gives the
  labels. The rack is the same codes read on a grid half as long, so its bins
  are found again by name.
- The rack face opens on the side the bin is on, drawn as you would stand
  facing it: the back reads 19 to 36, left to right. **Front · 01–18** and
  **Back · 19–36** tabs switch sides. The plan marks the aisle to stand in, and
  the mark moves when you switch.
- The bins list says "back, bay 36, level 01".
- In 3D a two-sided rack is one block two cells deep, with a spine along its
  top.
- A fixture shows a bin on the back of a rack (`/fixtures/bin/back`).

On a copy of the real data, leaving out the four and making every rack
two-sided gave one place per rack, with every other bin placed. A rack's last
bay opened on its back, lit at the end behind its first.

### The 3D view

On the Warehouse screen, the site plan card has **Show 3D**. It puts the same
site in 3D beside the 2D plan, and they stack when the card is narrow.
- **What it draws.** It draws `GET /layout`'s `plan`, the same shapes the 2D
  plan draws.
  - Solid places are blocks, walk-through places are flat floor, and the
    outermost place is the ground with a solid edge.
  - A place with a grid shows it: a rack's bays and levels on its faces, a
    dock's doors across its floor.
  - The grid comes from `LayoutView.places`, joined by id, so the server did not
    change.
- **One choice.** Clicking a place in 3D chooses it on the plan, on the list and
  in the address. The chosen place is drawn in the accent colour, as on the
  plan. Hovering names a place in a small label, and otherwise the label names
  the chosen one.
- **Controls.** Drag to turn. Drag with the middle or right button, or with
  Ctrl or ⌘ held, to move across the floor; the view slides over the ground rather than up the
  screen, and cannot leave the site. Scroll over it, pinch, or press + and − to
  zoom. "Show the whole site" goes back to the starting view. Nothing is
  edited in 3D.
  - The user asked for these on 2026-10-01: moving, including with the middle
    button (the research said orbit and zoom only), and zooming on a plain
    scroll. The first version needed Ctrl
    to zoom, so the page would scroll past the pane.
- **Shown or hidden** is remembered per browser (`spork.warehouse.3d`). With no
  preference it starts shown at 1100 px wide and above, and hidden below.
- **Colours are the theme's tokens**, read from the page, and re-read when the
  theme changes. Faces are shaded by which way they face rather than lit, so a
  top is exactly its token's colour.
- It draws only when something changes (`frameloop` on demand). It honours
  reduced motion. Without WebGL 2 it says so in its own pane, and the plan
  beside it still works.

How it is built:
- three.js 0.186, **without React Three Fiber**. R3F 9 supports React only below
  19.4, so it would hold back React upgrades. The plan already had a
  hand-written class owning the meshes, and R3F would only have hosted the
  canvas.
- It is lazy-loaded as its own chunk: 586 kB, 150 kB gzipped, fetched the first
  time the pane is shown. The main chunk is unchanged.
- The code:
  - `client/app/layout/blocks.ts` is the pure part: plan to blocks, grid lines
    and bounds, with tests in `blocks.test.ts`;
  - `scene3d.ts` is the three.js class (`SiteScene`);
  - `Site3D.tsx` hosts it;
  - `client/app/common/stage3d.ts` is what every 3D view shares (`Stage`):
    the renderer, the turnable camera, drawing on demand, resizing, theme
    colours, camera moves and disposal. `SiteScene` and the item's `BoxScene`
    each build on one.
- A new fixture, `/fixtures/warehouse/drafted`, is a 19-place site as the draft
  lays one out.

Checked on the business's own bins: on a copy of the database with the draft
applied, 16 long racks in rows read at a glance. Clicking, hovering, zooming,
turning and the remembered toggle all worked, with no console errors.

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
   draft, and read it. On 2026-10-01 the draft was applied to a copy of the
   database, never the working one, and found this:
   - all but a handful of the bins placed;
   - the long racks came out whole;
   - three families of one bin each became racks of their own;
   - one family spans twelve bays for its two bins;
   - one code differs from its family only by an unpadded level, and waits in
     the tray;
   - the other leftovers are named spots (walls, the dock, packing, an office),
     to be placed by hand.

   The user said the four odd families are no racks. **Untick them in the
   preview**, and press **Two sides for every rack**, before making the places
   (above). Until the editor exists,
   nothing moves a bin once the draft has put it in a cell.
2. **Record photos, measurements and weights**, working from the item list
   narrowed to "in stock here" and "needs weighing", "needs measuring" or
   "needs a photo", with the drawer's Next (D174). Each photo opens the crop
   screen with the face already found (D177).
3. **The photo work, still open:**
   - **Time the face-finder on the real iPhone.** It is untimed there. If the
     first answer over the WiFi is too slow, the two ways on are HTTPS
     (threads, and WebGPU with a half-precision model) and starting the
     encoding while the photo uploads.
   - **Who adds a photograph is not checked.** A photo hangs off a look, and
     the look says who took it; the server does not refuse a photo added to
     someone else's look. The app never does that, but a direct call could.
     The user was offered the fix and has not taken it up yet.
   - A photo's `captured_at` is its look's time, not each shot's. The upload
     time, `recorded_at`, is the photo's own.
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

## The editor: the plan

The 3D view is built (above), and the editor comes next.

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
