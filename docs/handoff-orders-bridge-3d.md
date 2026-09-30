# Handoff: the bridge, the order page, and places

Written 2026-09-28, updated 2026-09-29. Read this before continuing on the
client's UI work, the order page, or anything spatial.

The short version: **picks made on the WMS handheld reach Spork (D172), a
userscript feeds them in, every order has its own page, and the warehouse
layout exists.** The layout is **places** drawn relative to each other and never
measured (D173). A first layout can be drafted from the bin list, and a scanned
bin now lands on the face of the rack that holds it. The plan editor and the 3D
view are next.

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
psql -h localhost -p 55432 -U postgres -c "DROP DATABASE IF EXISTS spork_fresh" -c "CREATE DATABASE spork_fresh"
export DATABASE_URL=postgres://postgres:spork@localhost:55432/spork_fresh
scripts/verify-migrations.sh && scripts/migrate.sh --baseline
cargo test --workspace --no-fail-fast
```

Use `--no-fail-fast`, because without it cargo stops at the first failing
binary and hides the rest. **Four tests fail in a full run, and they failed the
same way before any of this work:** two in `pack_walk_http` (the packing list's
order reference, and "nothing overdue") and two in `tenancy` (it expects one
site per tenant, and `fulfilment_intake` adds a second). Each passes alone.
They depend on which suites ran first, and nobody has fixed them yet.

**Client gates.** `npm run verify` runs typecheck, tests, contract and laws. Run
`npm run build:review` before `npm run render`, which reads `dist-review/` and
otherwise renders a stale build.

**The local instance.** `scripts/local.ps1 setup` then `start` serves
`http://localhost:18080`. Since 2026-09-30 the working `spork` database holds
the business's own data, read from NetSuite exports and never written back: the
item master, the prepack list, one warehouse's bins and its inventory balance.
The terminal importers load them: `import_prepack`, `import_bins --only` and
`import_stock`. No layout is drafted yet. Real codes stay out of the
repository, as everywhere else.

---

## What landed

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
- **Reads**: `GET /bins/{id}` (a bin, its cell and its place), `GET
  /places/{id}`, `GET /layout`.
- **Client**:
  - a scanned or searched bin code goes to `/bins/:id`, the rack face: the way
    in as chips, bays across and levels up with the labels as the rack prints
    them, the bin's cell lit, and a plan of the building;
  - `/places/:id` shows what is inside a place;
  - Settings › Layout drafts and lists.
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

`GET /orders/{order_id}` (`OrderView`: what the search answers per order, plus
its lines with ordered, committed, picked, picked elsewhere, packed and
despatched). `/orders/:order` in the client, reached from the dashboard, the
orders list's drawer ("Open order") and links. `FulfilmentSummary` gained the
fulfilment's own `reference`.

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

### The Spork Bridge userscript

This lives in the separate `warehouse-scripts` repo as
`public/spork-bridge.user.js`, and is deployed on push to main.

- **Read-only against NetSuite.** It reads Picked item fulfilments at one
  location with SuiteQL through the page's own `require(['N/query'])`, and
  POSTs each one to `/api/import/fulfilment?apply=true` with an import token.
- One tab syncs at a time, under a lease. An item fulfilment is only resent
  when its `lastmodifieddate` changes.
- It uses `anonymous: true`. Spork reads a session cookie before a bearer
  token, so a cookie sent alongside the token gets the import refused.

---

## Next

1. **Try the layout on real data.** Load the bin list, draft, and read the
   preview. Families the draft gets wrong are the first thing to fix.
2. **Test the rack face on the floor** against the bare bin code, as above.
3. **"Something's wrong here"** on the rack face: a worker scans the bin where
   it really is and taps its cell. Trusted roles apply it, and others raise a
   flag for the office.
4. **The plan editor and the 3D view** (below).
5. A history of layout changes (who moved what, and when), and roles for who
   may edit. Until roles exist, anyone signed in can draft.
6. A bin view and a warehouse view. **Item pages are built** (`/items/:id`,
   `GET /items/{id}`): what it is, its photo and family, its carton and
   measurements, and where it is, with NetSuite's report kept apart from
   Spork's own ledger. An item scan lands there, and order lines link to it.

Deferred and not forgotten:

- a per-site option to cancel instead of raising a finding when an item
  fulfilment changes;
- a Workspace UI for `site.owner_party_id`;
- NetSuite write-back;
- MachShip.

---

## The editor and the 3D view: the plan

**Stack.** three.js with React Three Fiber and a small subset of drei, on WebGL,
**lazy-loaded as its own route chunk** so the rest of the app pays nothing.
An imperative scene class owns the meshes and R3F only hosts it, which keeps a
hand-written renderer possible later. Scene data sits in typed arrays outside
React, redrawn on demand (`frameloop="demand"`). Why not the others: Babylon is
heavier and its React binding has one maintainer. PlayCanvas's binding is still
at 0.x. deck.gl is built for maps. Needle is commercially licensed.

**What it draws.** Places, from the same composition the rack face uses:
`PlanShape` already carries each place's footprint on the site, its height and
how far up it starts. Solid places are blocks and walk-through places are floor.
A site-wide geometry read (every place and every cell) is the one new endpoint
it needs. Picking is a ray against the places' boxes, which are few enough to
test directly.

**The editor** is the 2D plan with the 3D pane beside it, following the
research above. Its first commands are drag to move, drag a handle to resize,
rotate with quarter-turn snapping, add a box or an outline, add a grid, and set
the naming pattern. Changes that move bins between cells are previewed, and the
Unplaced tray holds bins nothing names. Writes are a person's act on a session,
recorded with who and when.

**Later.**

- Colour-by layers computed by the server: fill, findings, count age, and the
  difference between reported and ledger stock.
- Pick paths, which treat solid places as obstacles.
- Stock inside places, updated live.
- A handheld mini-map.
