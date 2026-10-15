# Handoff: the pack bench, pallets, and the brand

Written 2026-10-12 and brought up to date on 2026-10-15. Read this first. The
earlier handoff, [handoff-capture-and-search.md](./handoff-capture-and-search.md),
still covers capture, crops, search, backups and picking, and its Next list
still stands for those. [handoff-orders-bridge-3d.md](./handoff-orders-bridge-3d.md)
covers the toolchain, tests and layout.

## State

- **Live, 2026-10-15:**
  - `main` is pushed through cde5e10 (the gaps left from M3), then this
    handoff;
  - the live database is at migration 142 since 12:56 on 2026-10-14;
  - the server and client running are D257's (acfb9e0), live since the
    user's restart at 14:39. Asked from the `.local` name, a passkey
    ceremony names `<computer>.local`; the bench's outbox (D256) is kept in
    `client/dist-prev`;
  - the gaps left from M3 and the check's layout (cde5e10) are built and
    not live. They need migrations 143 and 144, the server and the client
    (Deploying, below).
- 2026-10-15, the gaps left from M3 (migrations 143 and 144) and the check's
  layout, on a fresh database (`spork_gaps`):
  - every migration builds and reverses, 144 of them;
  - the server's unit tests pass (321), and its integration tests 166 of 166;
  - the invariant suite passes but for S7 and S10, as before;
  - the client's tests (258), contract and laws pass, and the render gate
    passes over every fixture (150 fixtures, 492 renders).
- 2026-10-15, passkeys at the `.local` name (D257), no migration:
  - the server's unit tests pass (320), and the passkey integration tests
    (8) on `spork_m3run`;
  - the client's tests (254), contract and laws pass, and the render gate
    passes over every fixture (149 fixtures, 488 renders);
  - not tried on a phone yet: no test here holds a real authenticator at the
    `.local` name.
- 2026-10-15, the bench's outbox (D256), client only:
  - the client's tests (254), contract and laws pass;
  - the render gate passes over every fixture (149 fixtures, 488 renders).
- 2026-10-14, the phone pass and the hover fix (fb7c69b), client only:
  - the client's tests (251), contract and laws pass;
  - the render gate passes over every fixture (148 fixtures, 484 renders).
    Its frame check now compares the header and sidebar with the desk's size
    at that width, not with 34px.
- 2026-10-14, M3 of the plan, the board (D245, migration 142), on a fresh
  database (`spork_m3run`):
  - every migration builds and reverses, 142 of them;
  - the server's unit tests pass (316), and its integration tests 164 of 164;
    the two setup tests call `sh`, so they pass run from Git Bash and not
    from a cargo started by PowerShell;
  - the invariant suite passes but for S7 and S10, as before;
  - the client's tests (250), contract and laws pass, and the render gate
    passes over every fixture (148 fixtures, 484 renders).
- 2026-10-14, M2 of the plan, boxes cut to set heights and box photos
  (D252, migration 141), on a fresh database (`spork_m2run`):
  - every migration builds and reverses, 141 of them;
  - the server's unit tests pass (314), and its integration tests 161 of 163;
  - the two failures were `pack_walk_http`'s walkthrough taking a carton's
    cell for a shelf's, since `location_code` names a carton's resolved
    location too; it now takes `holder_location_id`, and both pass on that
    database;
  - the invariant suite passes but for S7 and S10, as before; J82 is new;
  - the client's tests (241), contract and laws pass, and the render gate
    passes over every fixture (144 fixtures, 468 renders).
- 2026-10-14, M1 of [packing-plan.md](./packing-plan.md), the rest of D246
  and D247 on the server (migration 140), on a fresh database (`spork_m1`):
  - every migration builds and reverses, 140 of them;
  - the server's unit tests pass (311), and its integration tests 161 of 162,
    the one failure the trolley pick below again;
  - the invariant suite passes but for `structural_invariants_hold`, S7 and
    S10 as before; J79 to J81 are new and hold;
  - the client's tests (239), contract and laws pass, and the render gate
    passes over every fixture (142 fixtures, 460 renders).
- 2026-10-14, the server side of D246 and the start of D247 (migrations 138
  and 139), on a fresh database (`spork_d246run`):
  - every migration builds and reverses, 139 of them;
  - the server's unit tests pass (311), and its integration tests 161 of 162;
  - the one failure is `picking_http::a_trolley_pick…`, which passes alone on
    another fresh database (`spork_d246p`): the order-dependence noted on
    2026-10-07, when another test has used the walk's fixture line first;
  - the invariant suite passes but for `structural_invariants_hold`, the
    S7 and S10 entries that predate this work.
- The build plan for the rest is [packing-plan.md](./packing-plan.md): seven
  milestones, each with what done means. Feature complete is D244 to D252.
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

**Later on 2026-10-15: the gaps left from M3, and the check laid out as To
pack is**
- **A closed order's plan goes** (settled 4, migration 143). After each fold
  the scheduler calls `pack_plan_drop_closed()`, which drops the plan of
  every fulfilment that is cancelled, closed in the other system, or
  despatched in full. It is a definer owned by `spork_projection_owner`,
  which may read only `fulfilment`'s id, state and `closed_elsewhere`.
- **Undo takes back Make and Ship.** A new board step, `unmake`, is worked out
  at undo time against the bench as read (`writes.ts`). It opens the carton
  if sealed, undoes its puts, takes it off its pallet, discards it, and
  discards the pallet if the same press made it. `making()` in `board.ts`
  plans the parcel again under a new id (`afresh`). A made carton dropped on
  a planned pallet undoes the same way, and a pallet made once per drop.
  A board change's acts are now worked out against the plan as it will be,
  so a made carton dropped for a new pallet makes that pallet.
- **Things that ship as they are can be planned onto a pallet** (migration
  144). A planned parcel is a box type or `as_is` (an item, a level, a
  case). Dragged onto a pallet, they are planned one parcel each; on the
  board they show on the pallet with Ship, which makes them from the plan:
  started with the plan's id, put on the pallet, filled, sealed.
- **The check card is laid out as To pack is**, at every width. One line rule
  (`.checkRow, .boardLine`) is a flex row that wraps. The item has a basis
  of 11rem and the side column (`.lineSide`) sits at the end, so on a phone
  only a row with the count box drops it underneath. The check's own grid,
  with its three breakpoints, is gone.
- Tests:
  - `pack_plan_http`: a closed order's plan is dropped; a thing as it is is
    planned onto a pallet and made from the plan;
  - `pack_plan`: a planned parcel is a box or a thing as it is;
  - `board.test.ts`: Make undone; things as they are planned onto a pallet;
  - `writes.test.ts`: a carton taken apart; a planned thing made as Ship
    makes it.
  - Fixture: `pack/as-is-planned`.

**On 2026-10-15: a paired device's own passkeys (D257)**
- Each address is a relying party (`Parties` in `passkeys.rs`): the
  configured one, and with `SPORK_IDENTITY` the host's name over HTTPS. Each
  ceremony half takes the one whose origin asked (`relying_for` in
  `routes.rs`), and anything else gets the configured one, as before.
- A passkey works where it was added, so a paired device adds its own. The
  pairing page, the Passkeys screen, `start -Lan` and `docs/local.md` say so.
- Test: `the_origin_that_asked_picks_its_relying_party`.

**On 2026-10-15: the bench's presses kept on the device (D256)**
- Every press at the bench is worked out into its writes first, against the
  bench as read (`writes.ts`: `writer`, `writesOf`), then kept on the device
  (`spork.outbox.bench`) and sent. Each write names its part of the press's
  act, and the ids are kept as they are minted (`keptAct` in `acts.ts`), so a
  press sent again, after a reload too, is the same act. `sendOn` carries on
  from the write it got to.
- With no answer the press stays kept. The bench says "Saved on this device:
  …", with Send now, and the board is greyed and takes no new press until it
  has gone (the user's choice). A kept press refused when it goes is shown
  until dismissed; somebody else's waits for them.
- One press counts the goods it has already used, so two cartons filled by
  one press (Ship 3 own cartons) are not filled from the same units. Ship as
  they are refuses up front when there aren't enough, rather than leaving a
  carton open part way.
- Picks and the bench share the outbox: `kept()` and `useOutbox()` in
  `app/outbox.ts`, `Outbox.now()` for a press sent at once, and the notice
  (`app/common/Kept.tsx`).
- A bench read that fails once the bench is showing keeps the board as it
  was, rather than replacing it with an error.
- Tests: `writes.test.ts` (sent on after a reload as the same act; the goods
  counted once), `outbox.test.ts` (`now`); fixture `pack/kept`.

**Last on 2026-10-14: desk screens on a phone, and a row lit whole (fb7c69b)**
- Hover and selection light a table's whole row, not its cells one by one.
  Stacked on a narrow screen, the cells sat apart in the row's padding and
  left patches with gaps round them.
- At 640px or under, or on a coarse pointer, desktop density keeps its text
  and takes 40/44/48px controls (`ui/tokens.css`). On those screens:
  - text boxes take 16px, so a phone does not zoom the page to them;
  - tabs wrap rather than scroll out of sight;
  - a worded ghost button keeps a faint ground;
  - a drawer's close and an alert's Dismiss are tap-sized.
- A scan's answer hangs across the screen under the header, not off its left
  edge.
- Figures (`StatGrid`) draw their lines between each other, never along the
  card's edge. A stacked table value stays inside its column.
- Screens, on a phone:
  - the pack bench's check puts the item across the top of its row;
  - the expected weight runs across the carton card;
  - the bin map's legend sits under the site;
  - Edit layout's plan is shorter, with place names that can be read;
  - Items, Photos to crop, Despatch, To pick and Workspace › Boxes are laid out
    for it.
- The rule is in `interface-design.md`, under the kit's density.

**Later on 2026-10-14: paired devices over HTTPS (D255), and choosing a box**
- `identity.rs`: with `SPORK_IDENTITY=1` the server makes Spork's own
  authority (only `.local` names) and a certificate for the name this
  computer announces by mDNS, renewed by itself, in `.local\state\identity`;
  `/pair` offers the authority and its fingerprint.
- `scripts/local.ps1`: `https` fetches Caddy 2.11.4 (checksum checked);
  `firewall` opens 443 too; `start -Lan` starts Caddy in front once the
  certificate is written. Tried end to end on spare ports: verified with the
  authority, refused without it. Live at 14:00, and an iPhone paired and
  worked. Caddy's process is `$proxy` in the script (`$caddy` was `$Caddy`,
  its path: PowerShell's names ignore case), and the server is stopped with
  the script (b50aff2).
- `client/public/sw.js` and the manifest: installable, the app kept on the
  device over HTTPS; `/api` and the server's documents left to the network.
- The board: a thing nobody measured asks which box in Move to…, which now
  offers every box by name; its fit is said to be by eye.
- Compression (648ca8e) is live.

**Later on 2026-10-14: M3, the board (D245, settled 2, 4 and 5)**
- Migration 142: `pack_plan_parcel` and `pack_plan_line`, the board's plan
  kept per fulfilment; `PUT /fulfilments/{id}/pack-plan` (`pack_plan.rs`)
  replaces it, the bench read carries it, and `POST /packages` with a planned
  parcel's id uses it up.
- The bench read: each line's `on_bench` (its units at the pack location, in
  no carton), each carton row's `line_id`, and the pack location no longer
  offered as a bin.
- The kit's drag (`ui/Drag.tsx`, `ui/choosing.ts`): pointer, choosing
  several, the keyboard, a ghost, and a drag held still for fixtures.
- The board (`PackBenchPage.tsx`, `ToPack.tsx`, `Parcels.tsx`,
  `Chosen.tsx`), worked out in `board.ts`: To pack and the check on the
  left, parcels in the middle with pallets holding cartons, the chosen
  parcel in 3D or layer by layer on the right with the booking; Undo,
  Suggest and the next step above. `fitInto` in `arrange.ts` says how many
  fit. `Suggestion.tsx` and the forms are gone.
- Acts: goods into a made carton from the bench, a bin or handed over; take
  out onto the bench; Make; Open; onto and off a pallet; Undo straight after
  a put as a correction.
- Server responses are compressed (648ca8e), live since the restart that day.
- Tests: `pack_plan_http.rs`, `pack_plan.rs` unit tests, `board.test.ts`,
  `choosing.test.ts`, `arrange.test.ts` (`fitInto`); fixtures `pack/drag`,
  `pack/wont`, `pack/pallet` and `pack/undo`.

**Later on 2026-10-14: M2, boxes cut to set heights and box photos (D252)**
- Migration 141: `package_type.cut_from_id`, its footprint held by the
  reference; `package_type_effective` (a cut's empty weight and limit from its
  full box, whether its height is set, whose photographs it wears); and the
  observable's package-type arm that migration 7 declared.
- `boxes.rs`: `POST /package-types/{id}/heights` adds a height, or links a box
  already on file by the name given (the half box); `DELETE /package-types/{id}`
  takes one away when nothing was packed in it. `GET /package-types` gives each
  box its cut, how many cartons were packed in it, and its photographs.
- `POST /observations` takes `package_type_id`: a look with photographs only.
- The bench's presets and cartons carry their cut and their box's cut faces.
  The suggestion picks the lowest box that takes everything, cuts included,
  and says "Cut a Medium Box down to 28 cm" before it is started and on the
  open carton. A cut is drawn with the bottom of its full box's photographs
  (`coverCut`, `keepOf` in `items/box.ts`), on the bench and in Workspace.
- The booking takes a cut's height from its box.
- Workspace › Boxes (`admin/Boxes.tsx`): each box with its heights; opened,
  its heights with their weights and Suggested, Add height, and its sides
  photographed and cropped with the item page's crop, the box drawn at the
  height chosen.
- J82: a cut is cut from a full box, lower, in its workspace.
- Tests: `boxes_http.rs`, `boxes.rs` unit tests, `box.test.ts` (the crop),
  `arrange.test.ts` (the cut chosen), and fixtures `pack/cut` and
  `workspace/box`.

**Later on 2026-10-14: M1, the rest of D246 and D247 on the server**
- Despatching a pallet: `POST /packages/{id}/despatch` without a line writes
  the package's own `despatched` event (`despatch_itself` in `routes.rs`),
  saying one already gone or still holding units of its own; a void one is
  refused. The despatch screen sends each carton's lines first, then empty
  pallets, then the pallet itself (`despatchCarton` with `inside`).
- The despatch read lists what is on each pallet, carton by carton with
  their lines (`BookedCarton.inside`), and leaves cartons on a pallet that
  goes as one out of Waiting to consign. A booked pallet shows "Pallet · N on
  it".
- `POST /consignments` refuses a breakdown pallet, a carton on a pallet that
  goes as one, and a pallet with something on it already booked.
- Repacks to and from the bench: `POST /repacks` takes `from_package_id` or
  `from_location_id`, and `to_package_id` or `to_location_id`; a location
  must be the site's pack location. Lot, status and owner come from the
  movements in the holder.
- A short at the check is a `short_pick` finding naming the check
  (`discrepancy.pack_check_id`, migration 140), one open finding a line.
- The bench read: each parcel's `kind` (box, pallet or as it is),
  `breaks_down` and `on` (its parent); pallet presets give their `deck`.
  The bench shows "Breakdown pallet" and "On …".
- Invariants J79 (no package inside itself), J80 (three deep at most,
  `nesting_too_deep`) and J81 (what is booked is what is delivered).
- The canvas gains the despatch screen with pallets and the pallet sheet.
- Tests: `pallet_http.rs` (booking refused and allowed, `inside`, despatching
  the carton, the pallet and the wrap), `repack_http.rs` (out onto the bench
  and back in, a shelf refused), `pack_check_http.rs` (the finding), and
  `job_asserted.rs` (a loop, a fourth level, a booked carton on a pallet).

**On 2026-10-14: units between cartons, cartons onto pallets (D246; D247 begun)**
- `POST /repacks` (`repacking.rs`): units of a line from a package-held cell
  into another package, one movement keeping its lot, status and owner.
  Refused into the carton it is in or one despatched or void; out of or into
  a sealed one it is recorded with "open it first" (D5).
- What a package holds is one view, `package_line_content` (migration 138):
  in, less what went on to another holder, never below nought. Packed (the
  fold, J68, the live progress) is what sealed and despatched packages hold,
  so units repacked are counted once. The bench's remaining and contents, the
  packing queue, the packing list, the contents read, despatch and the check
  before a seal read it too.
- Picks and handovers into a sealed carton are recorded with "open it first".
- `POST /packages/{id}/contain` refuses a loop and a fourth level, read from
  the events now (`package_parent_now`, migration 139, in
  `packages::nesting`). It says a sealed pallet, another order's pallet, and
  an open carton on a breakdown pallet. It takes the spot (`at`), kept on the
  event (`at_x_mm`, `at_y_mm`, `at_z_mm`, `turned`).
- `package_status_now` (migration 138) is a package's status from its status
  events. The bench, despatch, void and the act responses had read the newest
  event of any kind, so a sealed carton put on a pallet read as open.
- `package.breaks_down`, said on `POST /packages`; a breakdown pallet that
  names an order is refused.
- Tests: `repack_http.rs` (ten in, four moved on, both sealed: packed ten) and
  `pallet_http.rs` (spot, still sealed on the bench, three deep, the loop, the
  breakdown pallet), and unit tests for each check.

**Since, on 2026-10-13: orders by item, and who packed (D253, D254)**
- The Orders search takes item codes when no order has the number typed:
  the latest 50 orders at the site with every code, a column a code saying
  how many (`OrderMatch.asked_for`). An item's page has "Orders with it".
- Settings › Administration, for administrators: orders packed per person
  today, this week and in 30 days (`GET /packed-by`, `packers.rs`), from
  NetSuite's Packed By. Bridge 0.11.0 sends Packed or Shipped fulfilments of
  the last five weeks every five minutes (`POST /import/packed`,
  `importing/packed.rs`, `reported_packing`, migration 137). Rows are kept
  once seen; when one was packed is when Spork first knew (D254).
- Tests: `pack_walk_http.rs` (search by item) and `packed_http.rs`.

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
  On 2026-10-14 two more: despatch with pallets, and the pallet sheet (A4).

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
- **A check belongs to the line, not to a carton.** A line split over two
  cartons is checked once.
- **The weight explanation is narrow.** It names one thing miscounted, by up
  to 12. An item that has never been weighed gives the carton no expected
  weight, so no verdict.
- **The board plans boxes with `arrange.ts`.** Spots on a pallet, load
  limits and carriers' rules wait for the planner (M4, M5). A pallet's deck
  is read, and nothing fits cartons against it yet.
- **Undo is in memory.** Leaving the bench forgets it, as before. Undoing a
  Make uses the bench as last read, so a carton changed since by someone
  else is taken apart as it is now.
- **The pallet sheet is a mock.** Booking a pallet that goes as one gives it
  no line of its own yet (its weight and height); that is M6.
- **Paired devices (D255):**
  - where each passkey was added is not recorded, so the list of keys does
    not say which address each is for;
  - a server left running a year without a restart keeps a lapsed
    certificate;
  - whether the handhelds let an authority be installed is untried;
  - `firewall` needs an administrator's shell.
- **The bench's outbox (D256):**
  - a kept press that lands later can't be undone with Undo;
  - kept presses are sent from a bench screen. Left on the packing queue,
    they wait until a bench is opened;
  - a press is still worked out against the bench as last read. The board
    waits for a kept press, so nothing new is worked out from a stale board.
- **On a phone:** a finger on Edit layout's plan moves the plan, not the page;
  the page scrolls from above and below it. A long placeholder is cut short
  at 390px.
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
- **The test database** `spork_m3run` is at migration 142. It has been
  written to, so build a fresh one for a clean run.
- **This computer serves the live app.** Run heavy builds, tests and renders
  at below-normal priority, cargo with `-j 4`. Make a hotfix in a clean
  worktree from `HEAD`, never from a tree with half a milestone in it.
- **The render gate** (`npm run build:review && npm run render`) does not
  clear `.render`; delete screenshots of fixtures that no longer exist.
  `RENDER_ONLY=a,b` renders the fixtures whose path holds either.
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

  A change to the client alone needs steps 3 and 4 only, swapped at once,
  with no restart. The previous client stays in `client/dist-prev` (not in
  git) to swap back, unless the user says to remove it, as on 2026-10-15.

## Next

To build, in the agreed order:

1. **Put the M3 gaps live** when the user says so: migrations 143 and 144,
   the server and the client (Deploying, above). Then pack an order on the
   board using all three, as [packing-plan.md](./packing-plan.md) says.
2. **Try D257 on the iPhone** at `https://<computer>.local`: sign in with the
   password, add a passkey under the name menu › Passkeys, sign out, and sign
   in with it.
3. **M4, D249 and D250**, as shaped on 2026-10-15 (settled 9 and 10 in the
   plan): `subject_load`, wall and lid limits, the warehouse's conditions,
   solid or hollow at sealing, Workspace › Carriers with a usual carrier,
   `build_rules`, and the checks that need no positions (heavy, loose, a
   pallet's weight).
4. **M5, D251:** the planner in a worker. It replaces `arrange.ts` once it is
   never worse over a test set of real orders.
5. **M6, D247 and D248:** breakdown pallets through the day, the pallet sheet,
   and the booking by delivered unit.
6. **M7, the finish:** a day's orders packed on the board and a breakdown
   pallet despatched, what that shows fixed, and the speed targets met.

To try on the floor, with the user:

- **D256:** at the bench, turn the WiFi off and drag goods into a carton:
  the bench says it is saved and the board waits. Reload the page, turn the
  WiFi on, and see it go and the board come back.
- Check real orders at the bench, and set the tolerance if 3% is wrong. A
  short lands in the findings queue: see that it names the right picker.
- **M2:** photograph the box types; pack an order that the board puts in a
  cut box. The user's heights are loaded.
- **M3:** pack a real order on the board: drag lines into a carton, Make a
  planned box, put cartons on a pallet, take something out and Undo, and
  despatch the pallet.
- **Pair the handhelds** (D255): `scripts\local.ps1 firewall` once as admin,
  then on each device `http://<computer>.local:18080/pair` and
  `https://<computer>.local`. The iPhone is paired.
