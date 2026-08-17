# Competitor feature analysis: does the architecture hold up?

Generated 2026-07-30 by a multi-agent research pass over six WMS products
(Oracle NetSuite WMS, Manhattan Active WM, CartonCloud, ShipHero, Odoo
Inventory, Descartes Peoplevox). Each was researched for concrete feature and
data-structure requirements, then a separate adversarial pass tested
[domain-model.md](./domain-model.md) against implementing each feature well.

23 architectural gaps were flagged across the six products before synthesis.

**RF-SMART was not among them, and should have been.** It is the incumbent answer to the problem this project exists to solve — the leading third-party WMS for NetSuite, with a dedicated ANZ team — so it is covered separately in [competitor-rf-smart.md](./competitor-rf-smart.md).

**Provenance caveats.** The text below is the synthesis as produced. Two things
to be aware of when relying on it: counts like "all five" are used loosely and
the study covered six products, so verify a specific claim before betting on it;
and per-product detail lives in the run journal at
`.claude/projects/*/subagents/workflows/wf_bf832739-2e8/journal.jsonl`.
Claims about competitor internals are second-hand from public documentation.

---

## Verdict — does the architecture hold up?

Mostly yes, and better than I expected on the axes it was designed for. The append-only spine, the `measurement` provenance model, the `carrier`/`freight_provider` split and the `location` geometry all survived contact with five competitors and beat most of them outright. Roughly fifteen tables genuinely does buy eleven capabilities.

But the founding claim is wrong. Principle 2 says "two things actually happen in a warehouse: stock moves, and things get measured." Five independent products say there are at least four:

1. Stock moves — `stock_movement`. Modelled.
2. Things get measured — `measurement`. Modelled.
3. **Stock gets promised.** Allocation/reservation. Every one of NetSuite, Manhattan, CartonCloud, ShipHero, Odoo and Peoplevox treats this as first-class. We have no representation of it at all. A reservation is a claim on the future, so it is neither a movement nor an observation, and it cannot be projected from either fact table.
4. **Work gets done.** Tasks, scans, failed picks, skips, idle time. `fulfilment.picked_by_id`/`picked_at` is an after-the-fact stamp, which is precisely the NetSuite shape the project exists to replace.

There is also a fifth, smaller one: stock changes *condition* without moving (quarantine, damage, hold, recall). That fits neither table either.

The model is not wrong so much as under-dimensioned, and the under-dimensioning is concentrated in the outbound execution layer — which is the half of the warehouse the project is actually being built for. Everything inbound (supplier, PO, receipt) is simply absent rather than resisted.

Two things are worse than gaps, because they are unstated assumptions rather than decisions:

- **`stock` has a three-part key.** Adding a status dimension, an owner, or a container to it after the projection is live and every reader assumes three parts is the most expensive migration on this list.
- **Multi-tenancy is not on the deliberately-not-building list.** Serials, multi-currency and the double-entry ledger are recorded as decisions. Tenancy is invisible, which makes it look like an oversight.

Six tables are referenced but never defined: `lot`, `customer`, `contact`, and whatever `actor_id`, `picked_by_id` and `packed_by_id` point at. `lot_id` in particular is load-bearing for a food business and is currently a dangling column on both spine tables.

---

## Architectural gaps — features our model actively fights

Ranked by how expensive the fix gets if deferred, not by size.

### 1. No inventory status dimension on `stock`

**What.** Sellable / damaged / quarantine / on-hold / recall-hold as a dimension of on-hand identity, with an `allocatable` flag driving commitment.

**Who has it.** All five. NetSuite makes it the fourth part of the on-hand key and its assessment calls it "the single most commonly under-modelled thing in home-grown WMS schemas". Manhattan requires holds applicable retroactively at item/lot/LPN/location grain. CartonCloud has seven physical statuses on every POP. Odoo puts it in the quant key alongside owner and package.

**Why we fight it.** `stock` is keyed `(item_id, location_id, lot_id)`. A status change is a transfer between buckets where location is invariant — the current `stock_movement` row shape (`from_location_id`/`to_location_id`) cannot express it. And there is a live correctness bug today: `stock_movement.reason = 'return'` puts returned goods straight back into the same balance row as good stock, immediately pickable, with no inspection state. For food that is not a missing nicety.

**Smallest fix.** `inventory_status(id, name, is_available_for_allocation)`. Add `status_id` to the `stock` key. Add `from_status_id`/`to_status_id` to `stock_movement` mirroring the location pair. Default everything to `available`.

**Cost of deferral.** Highest on the list. It widens the primary key of the one materialised projection everything hangs off, changes every availability query, changes the stated index `stock(item_id, location_id)`, and changes the rebuild-and-assert job. Two columns and a key change today; a system-wide migration in a year.

### 2. No allocation or reservation layer

**What.** A persisted link from demand to specific supply, with a state (soft/hard/picked/short) and a shortage reason.

**Who has it.** All five, unanimously. NetSuite locks order lines to waves and has a `commitment_confirmed` flag distinct from `quantity_committed`. Manhattan's Wave Coordinator agent exists solely to diagnose recorded shortages. ShipHero has allocation as an entity with its own webhooks. Odoo makes `reserved_quantity` a column on the quant and the move line *is* the reservation.

**Why we fight it.** Nothing in the model expresses "promised but not yet moved". Available-to-promise is currently `stock.quantity` and nothing else, which means two fulfilments can be committed against the same units and the conflict surfaces on the floor. Principle 2 blocks the obvious fixes: a reservation cannot be a `stock_movement` (nothing moved) and putting `reserved_quantity` on `stock` voids the stated invariant that `stock` is rebuildable from `stock_movement`. The third option — a `stock_allocation` table with availability computed as a join — puts an aggregate on the hottest read path, which is exactly what materialising `stock` was for.

**Smallest fix.** `stock_allocation(id, fulfilment_line_id, item_id, location_id, lot_id, status_id, quantity, state, allocated_at, released_at)` plus a maintained `stock.allocated_quantity` column, reconciled by the same periodic job — with principle 2 explicitly amended to say `stock.quantity` is a projection of the ledger and `stock.allocated_quantity` is a projection of the allocation table.

**Cost of deferral.** Very high, and it has a deadline: the day we take ownership of the pick path from the Honeywell/NetSuite scanner. Every screen's availability query changes, and FEFO, waves, backorder, zone sortation and short-pick handling are all blocked behind it.

### 3. `package` cannot be a container

**What.** One primitive that is a pallet, a carton inside that pallet, a picking tote, an LPN in racking, and a shipped parcel — locatable, nestable, reusable, barcoded.

**Who has it.** Manhattan is LPN-centric to the point where stock lives *inside* an LPN, not at a location. Odoo 19 added `parent_package_id` plus a parallel planned-containment hierarchy. ShipHero added LPNs with nesting. CartonCloud has a pallet entity distinct from POP and location. NetSuite fudges it (a cart is a bin) and its own assessment calls that out.

**Why we fight it.** `package.fulfilment_id` is mandatory and there is no `location_id` and no `parent_package_id`. So a package is a description of a shipment, not a holder of stock: a reusable tote has no fulfilment, a pallet in racking has no position, and a pallet of forty cartons is one row with no cartons in it. The fulfilment walkthrough is *about* pallets of stacked cartons, so this is not a hypothetical. Meanwhile picking (totes), putaway (LPNs), cluster picking (trolley slots) and put-wall sortation each independently want the same missing primitive — four features, one table, which is principle 1 arguing for building it properly once.

**Smallest fix.** Make `package.fulfilment_id` nullable; add `location_id`, `parent_package_id`, `is_mobile`, `sscc`. Add `package_type.reusable`, `max_payload_g`, `max_cube_mm3`. Constrain nesting to depth 2 (pallet → carton) explicitly so `package_content` queries stay non-recursive.

**Cost of deferral.** High and asymmetric. `package_content` is the model's headline differentiator; changing what it contains after a year of shipped history is the migration nobody wants. `parent_package_id` and `sscc` are one nullable column each today. Note also that Australian grocery (Woolworths, Coles) mandates SSCC pallet-plus-carton labels and ASNs, and the `Alpha` caller value suggests that is not far-fetched.

### 4. No `lot` table, and no lot on `package_content`

**What.** Lot as an entity with expiry, plus lot carried all the way to the physical carton.

**Who has it.** Everyone. Odoo has four separate date columns (`expiration_date`, `use_date`, `removal_date`, `alert_date`) and the assessment is right that collapsing them makes "don't ship under 90 days remaining" unimplementable.

**Why we fight it.** We don't fight it — we just haven't done it. `lot_id` is a dangling FK on `stock_movement` and `stock`. But there is a sharp secondary gap: `package_content(package_id, fulfilment_line_id, quantity)` has no `lot_id`, and neither does `fulfilment_line`. So the recall question that matters — "which cartons on which pallets contain lot L" — is unanswerable by the exact table built to answer "which package was this item in". Open question 1 is correct that FEFO reaches into picking and the router; it understates it, because FEFO is an allocation decision and there is no allocation layer to make it in.

**Smallest fix.** `lot(id, item_id, code, supplier_lot_ref, received_at, manufactured_at, expiry_date, removal_date, status)`. `item.tracking` enum, `item.shelf_life_days`, `item.rotation_type`. `lot_id` on `package_content` and `fulfilment_line`. Index `stock_movement(lot_id, occurred_at)` — it is not in the stated index list and it is the primary trace query. Also index `stock(location_id, item_id)`; the stated index has the wrong leading column for "what is already in this bin", which every putaway commingling check needs.

**Cost of deferral.** The table is cheap whenever. `package_content.lot_id` is cheap now and a backfill-impossible retrofit later.

### 5. Nothing represents work to be done

**What.** A task with a status, an assignee, a source location, a target, a planned sequence and start/end timestamps.

**Who has it.** All five, and Manhattan's is a single unified task table across pick, replen, count, slot-move and yard-move — which is the principle-1-consistent shape.

**Why we fight it.** `stock_movement` is an execution record with no state column and no planned quantity, so a row cannot exist before the work happens without corrupting `stock`. That blocks not just picking but forecasting, putaway capacity checks (which need incoming-but-not-yet-arrived weight per location), reception allocation and cross-dock. Separately, work that moves no stock — a failed scan, a skip, a search that found nothing, idle time — has no home at all, which is why productivity metrics have counts but no denominators.

**Smallest fix.** One `move_task` table with a `purpose` enum (pick, putaway, replenish, transfer, count) rather than four near-identical tables, plus `pick_batch`, plus a second append-only `activity_event` fact table for scans that do not move stock. The activity table is also what the offline handheld queue needs, so it serves two purposes.

**Cost of deferral.** Large but genuinely additive — this is a subsystem you can build later without migrating what exists. It is only urgent because everything else in outbound execution depends on it.

### 6. No idempotency on `stock_movement`

**What.** A client-assigned event id so a replayed offline scan does not double-move stock.

**Why it matters now.** `floor-devices.md` commits to Android handhelds with offline capability. `stock_movement` has `occurred_at` and `actor_id` and nothing else — no `device_id`, no client event id, no server `recorded_at` distinct from the device clock. A replay after a wifi drop silently double-counts inventory and there is no column that could detect it, let alone dedupe it. There is also a live design question the model has not confronted: a movement valid on the device but invalid on arrival either gets accepted and drives `stock` negative, or gets rejected — and rejection means a status on `stock_movement`, at which point "stock on hand is the sum of these" needs a WHERE clause.

**Smallest fix.** `client_event_id` (unique), `device_id`, `recorded_at` on `stock_movement`; a separate `rejected_event` table so the sum stays unconditional.

**Cost of deferral.** Three columns today. After the handhelds ship, you have corrupted inventory you cannot identify retrospectively.

### 7. `stock` and `stock_movement` have no `site_id`

They reach a site through `location_id` — which is NULL on receipts and despatches by design. "All receipts into Melbourne this month" therefore requires inferring the site from the other end of the movement. This is a concrete bug, not a design tension. Denormalise `site_id` onto `stock`, `stock_movement`, `package`, `consignment`, `measurement`.

### 8. Status columns with no history

`order.status`, `fulfilment.status` and `consignment.status` are mutable columns. The model is scrupulously append-only about stock and casually mutable about everything else. The asynchronous carrier label round-trip (requested → received → despatched → manifested, plus failure with a reason) is the thing that will need debugging at 2am, and every transition and timestamp is currently discarded. Peoplevox, Manhattan and ShipHero all treat order/shipment state transitions as emittable events. Add `*_event` tables and derive the status columns, matching the `stock` pattern.

### 9. No UoM layer

`stock_movement.quantity` is always base units. That satisfies principle 5 and is right for arithmetic, but "the customer ordered 5 pallets" is lost at the door and `order_line`, `fulfilment_line` and `package_content` have no unit at all. Odoo's assessment is the sharpest here: our `item_packing_config` Ti/Hi ladder is *better* than Odoo's flat conversion factors for cubing, and *worse* for transacting. Both are needed. Add `entered_quantity` + `entered_unit` alongside the canonical base quantity on every quantity-bearing row, and `stock_movement.item_packing_config_id` so historical conversions are reproducible across config versions. This is also the answer to open question 5.

---

## Confirmed strengths

Only the defensible ones.

**Real travel geometry.** `location.x_mm/y_mm/z_mm` plus `location_edge.distance_mm` is a genuine traversable graph. CartonCloud has a single 1–30 integer doing double duty as travel proxy *and* pick-face flag. Peoplevox has a hand-maintained `sequence`. ShipHero falls back to alphanumeric collation on the bin name, which is why `A-01-01` naming conventions become load-bearing. Odoo's "Closest Location" removal strategy sorts by location name string and its own docs concede it is a proxy. NetSuite has free-text aisle/pallet-position strings and no distance model at all. Only Manhattan is at parity. Caveat: coordinates are nullable pending a survey, so until it happens we are behind everyone, and a nullable `location.sequence` earns its place as a bridge.

**Append-only spine with reconciliation from day one.** "Historic stock at date X" is a replay for us. CartonCloud needs a bolt-on ledger because POP rows mutate in place. ShipHero denormalises `on_hand` onto WarehouseProduct and its own documentation warns you need a reconciliation strategy or it drifts. Odoo has recompute routines for exactly this drift. The invariant that `stock` is rebuildable and a job asserts it is stronger than any count module in the list — Manhattan sells "auditor-approved cycle counting" on much less.

**`carrier` / `freight_provider` separation.** ShipHero's carrier accounts are keyed on (profile, carrier), so it *cannot* rate-shop two accounts of the same carrier on one order — a documented limitation. Odoo's `delivery.carrier` conflates the two. Our split means Swift-via-MachShip and Swift-direct rate-shop against each other, and "what did Swift cost us" spans the migration. This is the best-designed part of the model.

**`measurement` with provenance.** Nobody else has captured-vs-catalogue as a first-class distinction with source and confidence. Manhattan's CubiScan integration overwrites vendor data; we supersede it. Comparing `source = carrier_actual` against a computed prediction makes freight cost validation a query — nothing in the list can do this.

**All the cartonisation inputs, which almost nobody has.** NetSuite states outright it has no cartonisation. CartonCloud's "Optimized Packing" is a marketing page. ShipHero's "Packaging Recommendation" is a historical-frequency heuristic (smallest box used on ≥20% of comparable orders, single-package only, up to 24h stale) and its own docs say so. Odoo's put-in-pack wizard does not even validate against the package type's max weight. Peoplevox has none. Only Manhattan has real 3D cartonisation. We hold `item_packing_config` (Ti/Hi — which Odoo's flat UoM factors structurally cannot express), `measurement` with confidence, `package_type`, and `item.stackable`/`max_stack_height_mm`/`this_way_up`/`temperature_class`. And `package.dimensions_source = computed` means a computed package row *is* a stored pack plan. This is the clearest capability we can hold over four of five competitors without regressing anything.

**Slotting and labour data for free.** Per-(item, location, period) velocity is a GROUP BY over `stock_movement`. ShipHero has nothing. Odoo stores only a product-level 12-month rolling counter with no location dimension. Peoplevox has reports only. CartonCloud has nothing. Same for picks-per-hour per actor. Caveat: only as good as our ownership of the pick path, and `actor_id` is currently populated at crew granularity ("Casual Melbourne"), which makes the columns decorative.

**Dangerous goods as item columns.** `dangerous_goods_class`/`un_number`/`packing_group` feed a declaration directly, rather than Manhattan's and Ship Central's per-consignment hazmat tickbox.

---

## Worth adopting

**Table-stakes — a WMS without these is not credible:**

| Feature | Present in | Shape |
|---|---|---|
| Inventory status dimension | all 5 | See gap 1 |
| Allocation / ATP | all 5 | See gap 2 |
| `lot` with expiry, `item.tracking` | all 5 | See gap 4 |
| Inbound: supplier, PO, PO line, goods receipt | all 5 | Mirror `order`/`fulfilment` shape; derive received qty from the ledger, don't store it |
| `item_barcode` (multiple, with unit level + multiplier) | all 5 | Blocks carton receiving and all scanning; `item.code` alone is not enough |
| `person`/`user`/`role` + person↔site | all 5 | Six dangling FKs today |
| Counts: header + line with frozen snapshot, blind flag, recount generation | all 5 | Snapshot is derivable from the ledger but store it anyway (principle 6) |
| Returns: RMA header/lines, reason code, condition grade, disposition | all 5 | Disposition drives destination status — blocked behind gap 1 |
| Replenishment min/max per (item, location) + task | all 5 | `location.kind = 'pick_face'` already identifies the target |
| Order fields: priority, required-by/promised date, service level | all 5 | Lateness is currently undetectable in our own model |
| `fulfilment_line`: split `quantity` into committed vs packed | all 5 | Editing in place destroys the commitment record |
| Per-package tracking number on `consignment_package` | all 5 | Retrofitting multi-parcel tracking is painful |
| `location`: barcode, pickable/blocked flags, zone membership, `last_counted_at` | all 5 | `kind` is one enum doing three jobs |

**Differentiators worth planning for:**

- **Cartonisation.** Beats four of five. Needs only `package_type.max_payload_g` + `max_cube_mm3` + `package.suggested_package_type_id` on top of what exists.
- **Wave / batch / cluster / put-wall picking.** One ephemeral `receptacle_assignment(batch_id, fulfilment_id, location_id, position, opened_at, released_at)` table with a partial unique index serves trolley slots *and* put-wall cells — first-empty-wins and route-back-to-the-same-slot in one index.
- **Directed putaway as code with typed parameters.** We hold constraints nobody else does (`location` dimensions, `max_weight_g`, `reachable_by`, `item.temperature_class`/`stackable`). Ship a strategy, not a rules editor. Needs a `location_occupancy` projection maintained like `stock`, or the candidate-ranking query is an N+1.
- **Freight quote persistence.** `freight_quote(consignment_id, carrier_service_id, quoted_price_minor, transit_days, selected)` makes "why did we choose this carrier" auditable. Manhattan and ShipHero both persist this; we currently store one settled `price_minor`.
- **SSCC allocator.** `number_range(key, prefix, next_value)` with row-level locking, plus `package.sscc`. Required for any grocery B2B ASN.
- **Outbox / `domain_event` table.** Peoplevox, ShipHero, Manhattan and NetSuite (its EDI 856 staging) all converge on this pattern, and it fits an append-only model naturally.
- **Printer / station / print_job registry.** The ZPL pipeline exists client-side; the server holds no printer registry, no template registry, no job queue. Peoplevox's 5–7 second scan-to-label despatch cycle depends entirely on printer routing resolving from the station, never from a human.

---

## Worth deliberately skipping

**Rules and workflow engines.** NetSuite (~40 rules scoped by rule × location × process type), ShipHero (40+ triggers, 60+ actions, plus a *separate* higher-priority MWA engine), Manhattan (data-driven workflow overrides), Odoo (routes and push/pull rules), Peoplevox (configurable putaway rules editor). Already on the list. Two honest caveats worth writing down. First, `carrier_profile` works because carrier behaviour is a fixed set of scalars — it does not generalise to variable-length routing chains, so "add a column" is not an escape hatch here. Second, the real risk is erosion by accretion: putaway strategy, allocation strategy, rotation strategy, sortation rules and disposition routing each independently want a small ordered rule table. Declining the engine while accepting five rule tables is how you get a rules engine you never designed. Decide now: one deliberate enum-typed strategy primitive, or code.

**Custom-field frameworks.** CartonCloud's ten typed custom fields per client *are* its lot/batch/serial mechanism. NetSuite has two free user fields on the pick task line. Manhattan sells extensibility as a headline. The empirical case against is Peoplevox: it has `Attribute1-15`, and a real carrier integration jammed HS codes into them. Hold `item.hs_code` and `item.country_of_origin` as columns. Note the cost honestly — with fixed columns the stocktake count grain is fixed at (item, location, lot, status); CartonCloud lets each customer choose. Correct for one company, fatal for a 3PL.

**Integration templates as data (Peoplevox).** Per-tenant editable column mappings with per-column transformations is a mapping DSL stored as rows — EAV plus an expression evaluator. Peoplevox's own guidance tells integrators to keep the default field names and ordering, so the flexibility is nominal. Code-defined versioned contracts with generated schema output give the same machine-readable-contract value. Keep only the idempotency ledger (`processed_import` keyed on source system + external id).

**Configurable mobile screens (NetSuite SCM Mobile, Manhattan, Peoplevox).** NetSuite needs this because it has hundreds of screens and cannot ship a change to one customer. We have one team, one site and a deployable codebase. The answer to an odd workflow is a compiled screen and a deploy.

**3PL billing, rate cards, storage periods (CartonCloud, ShipHero, Manhattan).** We ship our own goods and NetSuite is the financial system. CartonCloud's `rate_card_charge` with tier bands, day-of-week predicates and cubic multipliers is a pricing DSL. Steal exactly one instinct: charges should be durable records bound to their source event, not recomputed at invoice time — if internal freight cost allocation ever happens, follow that.

**TMS execution (CartonCloud).** Drivers, run sheets, manifests, ePOD capture, on-forwarders, freight items measured in spaces and cubic rather than SKUs. D1 keeps MachShip as the freight layer, so we deliberately do not execute transport. Currently this reads as a gap rather than a decision — put it on the list.

**Work orders, BOM, kitting, assembly (NetSuite, Manhattan, ShipHero, Odoo).** A kit build is a transformation, not a move: N components consumed, one different item produced. Encoded with our NULL convention it would look like a write-off plus a receipt, permanently corrupting the write-off history the reconciliation story depends on. Fixable with a `transformation` reason and a grouping key — but this is a distribution warehouse and a BOM model drags in a subsystem nothing else wants.

**Yard, appointments, dock scheduling, labour standards, gamification, demand forecasting.** Manhattan and Peoplevox. All downstream of subsystems we do not have.

**Odoo's virtual-location double-entry.** Genuinely elegant — receipts become moves from a supplier location, scrap to an inventory-loss location, and valuation, traceability and adjustment history all fall out of one table. But adopting it means replacing every NULL in `from_location_id`/`to_location_id` with an FK and adding a `usage` predicate to every on-hand query. Our NULL convention is fine; the cost is that "how much of lot X is currently at customers" is answered via `fulfilment` joins instead. Accept it and move on.

**Serials — but reclassify.** The current wording bundles two very different things. Unit-level serialised inventory (serial as a stock-bearing entity with custody and its own movement history) genuinely is heavy and would break `stock` the same way LPNs do. Pack-time serial capture — ShipHero's entire implementation — is one table hanging off `package_content` with no reach into stock, picking or the router. The omission should name the expensive version.

---

## Schema changes recommended

Prioritised. Each is one I would defend.

**Do now — cheap today, migrations later:**

1. **`inventory_status` + `status_id` in the `stock` key + `from_status_id`/`to_status_id` on `stock_movement`.** Enables quarantine, damage, returns-don't-silently-restock, hold-for-recall. Cost: two columns and a PK change on paper; a system-wide migration once `stock` is populated. Fixes a live correctness bug for returns.
2. **`client_event_id` (unique), `device_id`, `recorded_at` on `stock_movement`.** Enables safe offline replay. Cost: three columns. Without it the handhelds will double-count inventory undetectably.
3. **`site_id` denormalised onto `stock`, `stock_movement`, `package`, `consignment`, `measurement`.** Enables per-site rollups that currently cannot be written correctly because `location_id` is NULL on receipts and despatches. Cost: five columns, one backfill rule.
4. **`package.parent_package_id`, `package.sscc`; `package.fulfilment_id` nullable; `package_type.max_payload_g`, `max_cube_mm3`, `reusable`.** Enables pallets-of-cartons, reusable totes, cartonisation weight limits, grocery ASN. Cost: five nullable columns now; changing `package_content`'s meaning after a year of history later.
5. **`lot` table; `lot_id` on `package_content` and `fulfilment_line`; `item.tracking`, `shelf_life_days`, `rotation_type`.** Enables FEFO, expiry blocking, and carton-level recall. Cost: one table, four columns. `package_content.lot_id` is not backfillable.
6. **`person` / `role` / `person_site`.** Resolves six dangling FKs. Cost: three trivial tables. Also fix the data-capture granularity — a crew-level `actor_id` makes the columns decorative.
7. **Indexes: `stock_movement(lot_id, occurred_at)`, `stock(location_id, item_id)`, `stock_movement(reference_type, reference_id)`, `package_content(fulfilment_line_id)`.** The stated index list misses the primary trace query, the commingling check, the source-document lookup, and the packing-station rollup.
8. **`created_at`/`updated_at` on every table; pick one identifier scheme (UUIDv7).** ShipHero kept a translation query purely to convert between legacy numeric ids and UUIDs and calls it a migration scar. Free lesson.

**Do next — additive, but design now:**

9. **`stock_allocation` + maintained `stock.allocated_quantity`, with principle 2 amended in writing.** Enables ATP, FEFO, backorder, waves, short-pick handling. Cost: one table, one projection column, and an honest revision to the model's central invariant.
10. **`move_task` (unified: pick, putaway, replenish, transfer, count) + `pick_batch` + `receptacle_assignment`.** Enables all picking methods, replenishment, directed putaway execution, cluster and put-wall sortation. Cost: three tables serving eight features — the principle-1-consistent version.
11. **`activity_event` (append-only, shares idempotency columns with #2).** Enables labour metrics, accuracy reporting, the offline queue, and exception handling (skip, failed pick, wrong scan). Without it, productivity has counts but no denominators.
12. **Inbound: `supplier`, `purchase_order`, `purchase_order_line`, `goods_receipt`, `goods_receipt_line`, `item_barcode`.** The largest missing subsystem by table count. Derive received quantity from the ledger rather than storing it. Also where `measurement` feed 1 (capture at the dock) finally has a flow to hang off.
13. **`order_event` / `fulfilment_event` / `consignment_event`; status columns become projections.** Enables debugging the async label round-trip and makes downstream events meaningful.
14. **`entered_quantity` + `entered_unit` on `order_line`, `fulfilment_line`, receipt lines and `stock_movement`; `stock_movement.item_packing_config_id`.** Enables ordering fidelity and reproducible historical conversions. Answers open question 5.
15. **`stock_movement.adjustment_reason_id` (orthogonal to `reason`), `note`, `reverses_movement_id`.** `reason` is a movement type, not a business why — "adjustment, damaged" and "adjustment, found" are currently indistinguishable, and an unpick has no link to what it undoes.
16. **`freight_quote`; `consignment_package.tracking_number`; `number_range` allocator; `printer`/`station`/`print_job`.** Freight and label completeness.
17. **Nullable `stock_movement.unit_cost_minor`.** Not accounting — insurance. If valuation ever comes back in-house, the history starts from the migration date otherwise.

**Declined, but record the reasoning:** `owner_id` on stock (decide: nullable column now, or an explicit "we never hold third-party stock" entry). Generic putaway/allocation/rotation/disposition rule tables. Report SQL as data. Dashboard tile JSON without an explicit principle-3 exemption.

---

## Open questions raised

1. **Is `stock` allowed to hold state that is not derivable from `stock_movement`?** Allocation forces the question and inventory-status counting reopens it. Principle 2's "two things happen" framing needs restating either way — better done deliberately now than discovered during implementation.
2. **How many fact tables are there?** Status changes, quantity counts, task events and scans are all facts that fit neither `stock_movement` nor `measurement`. `measurement.subject_type` is (item, package_type, package) and `metric` is (length, width, height, weight, cube), so a cycle count observation cannot go there. Three tables, four, or does `stock_movement` widen?
3. **Multi-tenancy: decide and write it down.** Currently invisible. One sentence — "one company owns all stock; `site_id` is the only partition" — costs nothing and prevents an unexamined assumption from becoming an expensive one.
4. **Do we own the pick path, or does the Honeywell/NetSuite scanner?** This determines whether allocation, FEFO, per-item velocity and labour metrics are even our data. Risk 2 (two sources of truth) proposes mirroring NetSuite inventory read-only at first, in which case `stock_movement` holds coarse mirrored adjustments and half the "what emerges" table is aspirational until the pick path moves.
5. **Is food/FEFO real?** The `Alpha` caller value suggests yes. It blocks the shape of `lot`, the allocation query and the router's objective function — and there is currently no stated tiebreaker between "minimise travel" and "take the oldest stock".
6. **Is grocery B2B (Woolworths/Coles ASN, SSCC pallet + carton labels) in the future?** Decide before `package_content` has history, because it is the one requirement that forces package nesting.
7. **Open questions 2 and 3 need settling together.** Waves push `fulfilment.order_id` toward a join table; load-building and consolidated freight push `consignment.fulfilment_id` the same way. Every competitor with wave picking has both as many-to-many.
8. **Does the "package dimensions are frozen historical facts" rule survive reusable totes?** A shipped parcel's dimensions must not change; a reusable tote's come from a live preset. One row cannot be both.
9. **Serials: split the decision.** Pack-time capture is one table; unit-level serialised inventory is a subsystem. The current omission bundles them and may be excluding the cheap one by accident.
10. **Does the outbox payload get a principle-3 exemption?** An event payload is JSON we generate. Defensible under never-queried-structurally, but it should be an explicit second exemption alongside `provider_exchange.payload`, not a quiet precedent.
11. **When does the location survey happen?** Route optimisation, slotting, putaway capacity and the 3D map all block on it, and until it lands we are behind Peoplevox's hand-maintained integer. A nullable `location.sequence` is the bridge.
