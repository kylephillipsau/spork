# Handoff: external picks, the bridge, the order page, and the 3D plan

Written 2026-09-28. Read this before continuing on the client's UI work, the
order page, or anything spatial.

The short version: **picks made on the WMS handheld now reach Spork (D172), a
userscript feeds them in, and the UI pass has started.** The per-order page is
built and passes every client gate, but is **not committed**: see "First, do
this". The 3D warehouse view has been researched and has a recommended plan,
which has not been agreed yet.

---

## First, do this

The working tree holds the order page work **and** a crate-wide `cargo fmt`
that reformatted about ninety server files it had no business touching. Strip
the formatting before committing:

```sh
git status --short        # ~96 files
git diff --stat           # the fmt-only files are the large, unrelated ones
```

Keep these, and restore everything else under `crates/` with `git checkout`:

- `crates/server/src/routes.rs`: `ORDER_COLUMNS`, the `order_match` helper,
  `GET /orders/{order_id}` (`order_by_id`, `OrderView`, `OrderLineView`), and
  `FulfilmentSummary.reference`.
- `crates/server/tests/pack_walk_http.rs`: the by-id read and a 404 case, after
  the `?reference=S260041` search.

Both files were fmt-ed whole, so either re-apply the edits to clean copies or
keep the files and check that their diffs are only the intended hunks. Do not
run `cargo fmt` over the crate again. Format only the files you touched, if at
all.

Client files, all intended:

- `client/domain/types.ts`, `client/domain/api.ts` (`api.order`) and
  `client/scripts/check-contract.mjs` (the `OrderView` and `OrderLineView` pairs).
- `client/app/routing/manifest.ts`, `screens.tsx` and `fixtures.tsx`, plus
  `client/app/shell/nav.ts`: the `order` route at `/orders/:order`.
- `client/app/outbound/orders/`: `OrderPage.tsx` and `useOrder.ts` are new.
  `OrdersPage.tsx` now exports `OrderFacts` and `OrderFulfilments`, and the
  drawer has "Open order". The CSS module gains `.short`. `fixture.ts` gains
  `ORDER` and `ORDER_MISSING`.
- `client/app/home/Dashboard.tsx`: order and packing-queue rows are clickable.
- `client/ui/DataTable.tsx`: a row click is ignored when it lands on a link or
  control in a cell, so it does not act twice.

Verified before stopping:

- `npm run verify`, `contract`, `laws` and `render` all pass. **Run
  `npm run build:review` before `render`**, which reads `dist-review/` and
  otherwise renders a stale build.
- `pack_walk_http` against `spork_test`: the new assertions pass. Three later
  tests fail, most likely because `spork_test` now holds bridge data (S900001).
  This is unconfirmed; the fresh-database run was interrupted. Confirm it on a
  fresh database as the README describes.

`bcba4fe` (responsive tables, breadcrumbs) is committed and **not pushed**.

---

## What landed since the last handoff

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
- It is tested end to end against the local test instance.

### A local test instance

Release binaries run against `spork_test` on `127.0.0.1:18080`, with
`SPORK_CLIENT_DIR=client/dist`. See [local.md](./local.md) for the toolchain
environment. The demo sign-in is in the seed. `spork_test` is disposable; the
working `spork` database was left untouched.

---

## Next

1. Commit the order page (above) and push both commits.
2. **Present the 3D and spatial plan for agreement before building any of it.**
   Agreed 2026-09-29. Phase 0 is built: D173, migration 96, `POST /import/racks`
   and `/import/floor` (on the Import screen), J76 and J77. The columns are in
   [layout.md](./layout.md). What phase 0 still needs is the site's survey, as
   a rack file. Phase 1, the read-only plan, is next.
3. Item pages, a bin view and a warehouse view. The order page's item codes
   will link to the item pages; `OrderLineView.item_id` is already on the wire.

Deferred and not forgotten:

- a per-site option to cancel instead of raising a finding when an item
  fulfilment changes;
- a Workspace UI for `site.owner_party_id`;
- NetSuite write-back;
- MachShip.

---

## The 3D warehouse view: the recommendation

Researched on 2026-09-28. Not yet agreed.

**What exists today.** `location` has had `x_mm`, `y_mm`, `z_mm`,
`length_mm`, `width_mm` and `height_mm` since migration 1, but nothing writes
them. The bin importer fills codes, the parsed aisle, bay, level and position,
and `pick_sequence`. [warehouse-data-model.md](./warehouse-data-model.md)
already says coordinates are "a warehouse survey, not a data import". **The
layout data is the real gate, not the renderer.**

**Stack.** three.js with React Three Fiber 9 and a small subset of drei, on
the WebGL renderer, moving to WebGPU when R3F 10 is stable.

- It is MIT-licensed, supports React 19 natively, and has the largest
  ecosystem (three-mesh-bvh, @three.ez/instanced-mesh, troika-three-text).
- Its core is about 185 KB gzipped, **lazy-loaded as its own route chunk**, so
  the rest of the app pays nothing.
- Why not the others: Babylon is heavier and its React binding has one
  maintainer. PlayCanvas's React binding is 0.x. deck.gl is geo-centric and
  its WebGPU is experimental. Needle is commercially licensed. A hand-written
  renderer would only pay off if R3F proves too slow.

**Architecture.**

- An imperative `WarehouseScene` class owns the meshes, and R3F only hosts it.
  This keeps the exit to a hand-written renderer open.
- Scene data is held in typed arrays indexed by location, outside React.
  Updates write the arrays and call `invalidate()` (`frameloop="demand"`), and
  React renders only the chrome.
- One `InstancedMesh` per bin shape, with a per-instance value and a colour
  ramp, so changing the colour-by is a buffer swap.
- Picking is a CPU ray-versus-box test over a BVH or grid built from the bins'
  millimetre boxes.
- Colour-by layers are computed by the server: fill, `pick_sequence`,
  findings, count age, reported-versus-ledger difference, pick frequency.
- One selection store shared by the tables, the detail panels and the 3D view.
- A 2D plan is the same scene through a top-down orthographic camera, and is
  the default on handhelds.

**Data model.** Additive; `location` is left as it is.

- `rack`: site, aisle, origin, rotation, face, bay widths, depth, level
  heights, positions per level, and a code template.
- `location.rack_id` and `location.geometry_source`
  (`template`, `survey` or `manual`). Generated coordinates go into the
  existing columns, and surveyed values beat template values.
- `floor_area`: polygons for docks, staging, walkways, walls and floor stacks,
  optionally linked to a `location`.
- Authoring starts with a CSV rack import. The generator expands each rack and
  diffs its codes against existing bins, reporting mismatches as findings
  rather than refusing them. A 2D plan editor comes later.
- The new tables need `tenant_id`, composite foreign keys, row-level security
  and grants, like `zone` and `location`.

**Phases.**

0. `rack` and `floor_area`, the import, the generator, and laying out
   Melbourne.
1. A read-only 2D/2.5D plan of one site, coloured by fill. Clicking a bin opens
   its panel, and search flies to it.
2. Full 3D, with levels, orbit and plan cameras, the layers, and labels that
   appear by distance.
3. Pick paths and pick-frequency heatmaps.
4. Packages inside bins, staging and docks, live updates.
5. Slotting what-ifs, WebGPU, and a handheld mini-map.

**Risks.**

- The layout survey effort.
- Handheld GPUs and WebView support for WebGPU. Default handhelds to 2D and
  WebGL2.
- Churn moving from R3F 9 to 10.
- Label clutter, and troika's CPU cost past a few hundred labels.
- Drift between template and survey coordinates; `geometry_source` makes the
  precedence explicit.
