# Handoff: the pack bench, pallets, and the brand

Written 2026-10-12. Read this first. The earlier handoff,
[handoff-capture-and-search.md](./handoff-capture-and-search.md), still covers
capture, crops, search, backups and picking, and its Next list still stands for
those. [handoff-orders-bridge-3d.md](./handoff-orders-bridge-3d.md) covers the
toolchain, tests and layout.

## State

- Spork `main` is pushed through b8f7b59: checking at the bench (D244), the
  design for the rest of packing (D245 to D252), the brand, the theme, page
  titles, and the round-wrap fixes to D240.
- The LAN server has run it since 15:43 on 2026-10-12. The live database is at
  migration 136.
- On 2026-10-12:
  - every migration builds and reverses, from empty and with both fixtures'
    rows (`verify-migrations.sh` against `spork_verify`, 136);
  - the server's unit tests pass (302);
  - the integration tests pass except two, run on `spork_fresh`, a database
    other runs had already used:
    - `pack_walk_http::the_queue_is_the_way_in`: its fixture's promise dates
      were made on 2026-10-01 and are no longer "due today";
    - `reported_stock_import`: that database has the test's file on record
      already, so the import is a replay;
  - the client's tests (239), contract and laws pass, and the render gate
    passes over every fixture (138 fixtures, 444 renders).
  - The invariant suite was not run this session.

## What landed

**Checking at the bench (D244, migration 136)**
- A card above To pack, "Check what came to the bench" (`Checks.tsx`). Each
  line says what should be there in the units on the box ("3 cartons of 10",
  "1 carton of 10 boxes", by `expectedIn` in `check.ts`, in NetSuite's word
  for the unit). Enter confirms; a number typed first says what was seen. The
  next unchecked line takes the cursor.
- A check is a fact: `pack_check`, `POST /pack-checks` (`checking.rs`). The
  bench read gives each line its `unit` (`items::unit_of`, shared with the item
  page) and its newest `check`. A check made against another count than the
  line now commits asks to be made again (`checkOf` says `stale`).
- Short: the plan arranges what is there (`onTheBench`) and the tally adds
  "N short" (`OrderLine.short`). Over is recorded and shown.
- Seal waits on the bench while a line in the carton is unchecked, and so do
  the "ship as they are" presses, which seal as they make. The server still
  records such a seal, with a soft warning naming the lines
  (`SealProblem::Unchecked`, D5).
- The weight check:
  - `ExpectedWeight` gains `tolerance_per_mille` and `agrees`, from the site
    (`site.weight_tolerance_per_mille`, 30 to start; Workspace › Packing,
    `POST /workspace/sites/{id}/weight-tolerance`);
  - `PackedRow.each_g` is what one weighs, as the expectation counted it;
  - off, the bench says the likeliest miscount, fewest first ("2 more of
    APR-PE-CLR-L at 0.180 kg?", `explain`).
- Tests: `pack_check_http.rs` (the check, the seal warning and the
  tolerance setting) and the seal unit test on the server; `check.test.ts`
  on the client. The fixtures show every state:
  `fixtures/pack` (checked and unchecked), `pack/filling` (heavy, seal
  waiting) and `pack/loose` (short).

**The rest of packing, decided and not built (D245 to D252)**
- D245: one board for the whole order, dragged, with planned (dashed) and
  made (solid) parcels; the plan kept as what the packer placed.
- D246: repacks between cartons with packed netted (J68); containment made
  safe (no loops, three deep, no sealed parent) and positioned on the pallet.
- D247: a pallet goes as one or is broken down: what is delivered, booked,
  labelled and despatched follows.
- D248: a breakdown pallet built through the day: the queue heaviest first, a
  spot for each carton round what is on, "Full: wrap it and start another".
- D249: what a box carries, on its walls and on its lid; the standards and
  numbers behind it are in the decision.
- D250: build rules are settings a carrier points at; the planner never reads
  a carrier.
- D251: one planner for boxes and pallets, with a report of every rule.
- D252: boxes cut down to set heights, as boxes of their own cut from a full
  box; box types photographed.
- The order of building closes D252.
- The mockups are the canvas "Pack bench",
  https://claude.ai/artifact/9eMaJSZBNHoRe5KHjTq5TY. It has five boards: the
  board, checking, a breakdown pallet, Workspace › Boxes with heights and
  limits, and Workspace › Build rules. The user approved them on 2026-10-12.

**Round things (D240, amended in place)**
- A lid and base are the largest outline that is an ellipse, straightened to
  a disc (`discOf`, `ellipseFit`).
- The turns between side photos are found, a quarter turn apart included. A
  pair with nothing to match has no say, and Photos to crop says when two are
  more than 100° apart.
- Wrap again… on a round thing's card, and Adjust… under Photos to crop:
  each side photo turned a degree at a time till the seams line up
  (`useWraps`, `restitch`, `turnTo`).

**The look**
- Page titles: the tab names what is open, its order number or item code
  first, then the screen (`usePageTitle`, `routing/title.ts`). No decision
  record; this is the note.
- The brand (`interface-design.md`):
  - the Rack S is a 4 × 5 bin grid in black anodised, lit bins forming the S,
    one in hi-vis (`rackS.ts`);
  - the wordmark is "spork" in B612 Bold, outlined, over a measuring rule
    with a notched "o"; it scans once on hover (`Brand.tsx`);
  - every icon is made from the one mark by `client/scripts/icons.mjs`:
    favicons, Apple and Android, and the Tauri icons under
    `mobile/src-tauri/icons`;
  - the design canvas is https://claude.ai/artifact/KZih9PfZBXEeekAe5kUctZ.
- The theme (`ui/tokens.css`):
  - in light, ink to act and hi-vis to point; in dark, hi-vis to act;
  - `--ui-lit` marks what is chosen;
  - the frame is clear anodised in light and black in dark, and the sidebar
    is light in light mode;
  - codes are set in Atkinson Hyperlegible Mono at body size, medium weight
    (self-hosted in `ui/fonts`).

## What the user said, for the packing work still to build

- **Checking:** confirm, not blind: the expected count is shown.
- **Load bearing:**
  - a thing takes weight unless said otherwise, an item property for what
    isn't solid or sits awkwardly;
  - our own boxes are mostly boxes of other items; a hollow one carries
    lighter boxes, and "10 kg" was only an example. The research in D249
    turned that into a wall limit and a lid limit per box type.
- **Breakdown pallets:**
  - heavy and large orders are packed first, and their cartons go on once
    labelled and finished;
  - full, the pallet is wrapped and another started;
  - one pallet per carrier: the usual carrier, and occasionally a second when
    it is cheaper.
- **The second carrier:**
  - every label faces out and none is hidden;
  - no overhang left or right, which is either side of where the forklift
    tines go in;
  - with the usual carrier, overhang is possible but not ideal;
  - a label on top of the top layer probably doesn't count, but make it a
    setting.
- **Architecture:** no carrier logic in the model; it stays general and
  configurable (D250).
- **Box heights**, as the fast fulfilment script keeps them, in cm:
  - Extra Small Box, a half;
  - Small Box: 30, 25, 20, 15, 10;
  - Medium Box: 41, 34.5, 28, 21;
  - Large Box: 46, 41, 36, 31, 26, 21, 16.
  - A cut box's photo is anchored at the bottom edge and cropped, never
    squashed.
- **What the data showed:**
  - no pack configuration on file records layers per pallet (none of 945);
  - the imported box types include four pallet and skid sizes with heights
    from 600 to 1300 mm; the bench leaves out PAL, SKI and SKD.

## Known limits

- **What should be there is a guess.** Checking guesses it from the line's
  packs, largest first. Goods picked loose can arrive otherwise, and the
  count is one number per line, in the line's unit.
- **A short is not raised as a finding.** It shows on the check card, in the
  plan and in the tally only.
- **A check belongs to the line, not to a carton.** A line split over two
  cartons is checked once.
- **The weight explanation is narrow.** It names one thing miscounted, by up
  to 12. An item that has never been weighed gives the carton no expected
  weight, so no verdict.
- **Packing is still the form bench** with the client-side plan (D195 to
  D202). Nothing calls `POST /packages/{id}/contain` yet, and pallets are box
  types with no size on the bench.
- **Outstanding for the user:**
  - the board grade stamp on the box stock;
  - a floor test of each box's walls and lid;
  - the usual carrier's own pallet rules;
  - whether the second carrier allows front and back overhang (its 2016 fact
    sheet says none on any side).

## Toolchain notes from this session

- **cargo** needs `OPENSSL_DIR`, `OPENSSL_LIB_DIR`
  (`C:\Program Files\OpenSSL-Win64\lib\VC\x64\MD`) and `OPENSSL_INCLUDE_DIR`
  set, and OpenSSL's `bin` on `PATH`, as [local.md](./local.md) says
  `local.ps1` does.
- **`scripts/migrate.sh` needs `psql` on `PATH`** (`C:\Program
  Files\PostgreSQL\18\bin`). Without it, it reads every migration as pending
  and fails at the first, leaving nothing changed.
- **`scripts/verify-migrations.sh` drops the schema of whatever
  `DATABASE_URL` names, and its default is the working `spork` database.**
  Point it at `spork_verify`.
- **The test database** `spork_fresh` is at migration 136. It has been
  written to, so the two failures above are expected there. Build a fresh one
  for a clean run.
- **Deploying while the LAN server runs:**
  1. Rename the running `target\release\spork-server.exe` and
     `spork-scheduler.exe` to `*.running.exe`.
  2. `cargo build --release`.
  3. Build the client into `client/dist-next`:
     `npx tsc --noEmit && node scripts/fetch-model.mjs && npx vite build --outDir dist-next`.
  4. Wait until `schema_migration` has the new migration, then swap
     `dist-next` into `dist`, keeping the old as `dist-prev`. This session ran
     a background loop to do it.
  5. The user restarts `scripts\local.ps1 start -Lan`, which migrates first.

  A client built straight into `dist` goes live at once, ahead of a server
  that lacks its endpoints.

## Next

1. Check real orders at the bench, and set the tolerance if 3% is wrong.
2. **D246 and D247 on the server:**
   - `POST /repacks`, with packed netted in the fold and J68;
   - containment that refuses loops, depth and sealed parents, and records
     the spot;
   - warnings for anything added to a sealed package;
   - `package.breaks_down`;
   - despatching a pallet whole, and consignments of what is delivered;
   - the invariants: no loops, depth, nothing consigned twice;
   - the bench read gains each parcel's parent and kind, and pallet presets
     give their footprint and build height.
3. **D252:** `package_type.cut_from_id` and the heights in Workspace › Boxes,
   with the user's list loaded. Then box types photographed (the observable's
   package-type arm) and the bottom-anchored crop in 3D.
4. **D245:** the kit's pointer drag, then the board on real cartons.
5. **D249, D250, D251:** `subject_load`, wall and lid limits, `build_rules`,
   and the planner in a worker. It replaces `arrange.ts` once it is never
   worse over a test set of real orders.
6. **D247 and D248:** breakdown pallets through the day, the pallet sheet,
   and the booking by delivered unit.
