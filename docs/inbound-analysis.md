# Inbound scope: research and analysis

Generated 2026-07-31 by a multi-agent research pass over six streams: enterprise
inbound (Manhattan, Blue Yonder), Odoo's open-source receipt schema, Australian
and mid-market peers (CartonCloud, NetSuite WMS, ShipHero), the GS1 physical
identification layer, ASN/EDI messaging, and receiving operations and returns.
Each stream was then adversarially tested against [domain-model.md](./domain-model.md)
and its decisions D1-D20.

Follows [competitor-analysis.md](./competitor-analysis.md), whose gaps have
largely been addressed by D1-D20.

**Provenance caveats.** Claims about competitor internals, standards and
Australian retailer requirements are second-hand from public documentation and
should be verified before anything contractual depends on them. Per-stream detail
is in the run journal at
`.claude/projects/*/subagents/workflows/wf_281d4478-813/journal.jsonl`.

---

# Inbound scope — conclusions

## Verdict — how well does the existing architecture extend to inbound?

Better than I expected, and the places it breaks are specific rather than structural.

The spine is right and inbound makes it *more* valuable, not less. `stock_movement` being append-only, uniquely-identified and never rewritten (D5, D8) is exactly the evidence chain a supplier dispute needs, and the Auburn ASN study is empirical proof of D8's thesis: most retailers cannot break receiving accuracy down by vendor or error type, and many discard it at 60 days, which is precisely why chargebacks are disputable. Our scorecards are reproducible by construction. D4's `inventory_status` in the `stock` key beats every competitor's quarantine-as-a-location fudge, and D14's `lot_hold` composition beats D365's per-status blocking checkbox. D20's capability-as-data is, word for word, the right answer to supplier ASN capability — a supplier who has never sent an SSCC should not be shown a scan-the-pallet-label screen, and that is a column on the party, not a mode.

Five things are actually wrong.

1. **`goods_receipt`'s CHECK forbids blind receipt.** D16 wrote `num_nonnulls(purchase_order_id, transfer_order_id) = 1`. A truck arriving with no order has neither. This is the same error the published "Correction to D10" already fixed once, for the same reason.
2. **`package_content` is outbound-only.** Its single demand FK is `fulfilment_line_id`. D6 claims "one primitive serves shipped parcels, pallets of cartons, picking totes, putaway LPNs and put-wall cells". That claim is currently false for putaway LPNs — an inbound pallet's contents are unrepresentable in the table the doc calls "the capability NetSuite does not have".
3. **Principle 2 is missing a category.** A supplier's ASN, a CHEP transfer docket, a carrier ETA, a declared carton weight: none are our facts (we did not observe them), our intentions (we did not plan them, and cannot cancel them), or findings. All five research streams hit this independently. The model already has the seed — `measurement.source = supplier` — and it is welded to dimensions.
4. **D20 broke D12's invariant in passing.** `owner_id` joined the `stock` key; `stock_movement` never got it. "Each column is rebuildable from its own source" is false today, and ownership transfer without physical movement is inexpressible. D4 solved the identical problem for status by adding a from/to pair. This is the sharpest technical defect found and it is a one-decision fix.
5. **Containment is mutable state with no fact behind it.** `package.parent_package_id` and `package.location_id` are columns that overwrite. Split, merge, re-palletise and relabel leave no evidence, and nothing reconciles `stock` against `package_content`. Inbound is LPN-centric end to end, so this is where it bites.

Everything else is additive.

---

## The recommended inbound model

### The shape: four groupings, not one document

D15's insight run inbound. Outbound separates demand / work / freight; inbound has a fourth, because a counterparty tells us things.

| Grouping | Question | Entity | Principle 2 category |
|---|---|---|---|
| Demand | What did we ask for? | `purchase_order`, `transfer_order`, `return_authorisation` | Intention |
| **Assertion** | What does the counterparty say is coming? | `inbound_shipment`, `asserted_unit` | **Assertion (new)** |
| Work | What did we do with this arrival? | `goods_receipt`, `work_task` | Grouping of facts |
| Freight | What turned up at the door? | `vehicle_arrival` | Fact |

Collapsing any two is how every bloated inbound model got that way. Hanging the ASN off the PO — the single most common schema mistake in this area — is structurally identical to `consignment.fulfilment_id`, which D15 deleted.

### Principle 2, amended

**Facts** are what happened. **Intentions** are what *we* plan. **Assertions** are what a counterparty claims. **Findings** are where any two disagree.

An assertion is immutable like a fact (we cannot edit a supplier's message; they can only send another), unverified like an intention (wrong ~8% of the time by the Auburn median), and evidentiary. The rules: an assertion is **never projected into `stock`**, it always names its author party, and it exists only to be compared. This is not a fourth mechanism — it is a category label on tables that already have to exist, and it is the difference between an advised quantity living in something stock-shaped and living somewhere it can be argued with.

Everything below is tagged.

### Party and capability *(reference / operational)*

Settles question 57 in favour of one table plus roles, and the argument is external: the same GLN is ship-to on one label and bill-to on another. Role belongs on the relationship.

```
party                       -- intrinsic; tenant_id NULL = shared (D19)
  id, tenant_id, name, abn, gln, active
  gs1_company_prefix, gs1_prefix_length     -- licensed to a legal entity

party_role
  id, party_id, role        -- legal_entity | customer | supplier | carrier | pool_provider
  owner_id                  -- nullable: a 3PL client's supplier, not ours

party_profile               -- operational; tenant_id NOT NULL, always (D19)
  party_id, tenant_id
  sends_asn, asn_channel, applies_sscc_labels, asn_hierarchy_depth
  lead_time_days, sscc_reuse_window_days
  min_shelf_life_days       -- what WE require of THEM (mirror of D14's customer rule)
  b2b_compliance_status
  labels_per_pallet         -- moved off carrier_profile; it is a partner rule too
```

The D19 split lands cleanly and is a third piece of evidence it is real rather than fitted to items: a GLN and a company prefix are intrinsic; `asn_timeliness` and `label_quality` are our observations of them and are tenant-scoped.

### Purchase order *(intention)*

```
purchase_order
  id, tenant_id, site_id, supplier_party_id, owner_id
  reference, ordered_at
  promised_from, promised_to, required_by       -- MABD window; early is also late
  status

purchase_order_line
  id, purchase_order_id, item_id
  quantity                                       -- base units
  entered_quantity, entered_unit, item_packing_config_id
  promised_from, promised_to
  closed_at, closed_reason                       -- short-closing is amending an intention
```

No `quantity_received` column, ever. Received is `SUM(stock_movement.quantity)` grouped by `goods_receipt_line_id` — which is a Diesel `belonging_to` batch load only because D10 made the cause a typed FK. ShipHero's documented accumulator double-count bug is structurally impossible here; that is worth stating so nobody adds a counter "for performance" later.

`promised_from/to` is overdue independently of inbound: `order` has no required-by date either, so lateness is currently undetectable in both directions.

### Inbound shipment and its hierarchy *(assertion)*

```
inbound_shipment
  id, tenant_id, site_id, supplier_party_id
  ship_from_gln, ship_to_gln
  vendor_shipment_ref                  -- their ASN number
  gsin, ginc                           -- nullable
  carrier_party_id, conveyance_ref, container_ref, seal_number
  despatched_at, estimated_arrival_at
  split_shipment, completes_order      -- asserted flags; drive PO closure
  ingestion_channel  -- edi | portal | csv | email | api | keyed | derived_from_po
  granularity        -- derived: header | line | unit_ids | unit_contents
                     --                    | unit_contents_with_lot
  status             -- accepted | rejected | superseded
  supersedes_id, inbound_message_id
  UNIQUE (supplier_party_id, vendor_shipment_ref) WHERE status <> 'rejected'

asserted_unit                          -- the declared logistic hierarchy
  id, inbound_shipment_id, parent_asserted_unit_id
  level_code                           -- raw, as received ('3','1','P','T')
  sscc, sequence
  package_type_id, gross_weight_g, net_weight_g, ti, hi
  package_id                           -- nullable; set at receipt

asserted_unit_content
  id, asserted_unit_id
  item_id                              -- nullable: unresolved GTIN
  raw_gtin, purchase_order_line_id
  quantity, entered_quantity, entered_unit
  lot_code, expiry_date, best_before_date, catch_weight_g
```

**This is how D6's depth-2 constraint survives.** The X12 HL levels S-O-T-P-I are not five containers: S and O are documents. Physical depth is pallet → carton, which is exactly D6. But SSCCs genuinely nest without bound (shrink-wrap two pallets and the result gets its own SSCC), so the *asserted* tree is unbounded in `asserted_unit` — cold path, rare, recursion is fine — and collapses to depth 2 in `package` at receipt. D6's stated reason (keeping `package_content` queries non-recursive) is preserved rather than quietly broken.

It is also why advised units are not `package` rows. A `package` is a physical object; an ASN that is later rejected must not leave orphaned physical objects behind. And the ASN-to-PO relationship is many-to-many at line level with no join table needed at header level, because a shipment holds units and units name PO lines.

Retain the raw message separately:

```
inbound_message             -- a fact: a communication happened
  id, tenant_id, party_id, direction, channel
  interchange_ref, control_reference, syntax, syntax_version, test_indicator
  payload bytea, byte_count, content_hash, received_at, parse_status

inbound_message_error(inbound_message_id, source_line, rule_code, message)
```

Raw EDIFACT is bytes, not JSON, so principle 3 needs no exemption — but its wording ("`provider_exchange.payload` is the *only* JSONB") should become a rule rather than a census, because this is the third case.

### Goods receipt *(grouping of facts; status derived, never stored)*

```
goods_receipt
  id, tenant_id, site_id, owner_id
  purchase_order_id, transfer_order_id, inbound_shipment_id, return_authorisation_id
  CHECK (num_nonnulls(purchase_order_id, transfer_order_id,
                      inbound_shipment_id, return_authorisation_id) <= 1)
  vehicle_arrival_id                   -- orthogonal, always permitted
  supplier_party_id, delivering_party_id, manufactured_by_party_id
  mode                                 -- asn | po_line | blind | transfer | return
  blind                                -- captured, not inferred from role config
  reference
  started_at, completed_at

goods_receipt_line
  id, goods_receipt_id, item_id
  purchase_order_line_id               -- nullable: unexpected item
  asserted_unit_content_id             -- nullable: what this confirms
  expected_quantity                    -- SNAPSHOT at receipt time, not read live
  entered_quantity, entered_unit, item_packing_config_id
  lot_id, package_id
  accepted_at, rejected_at             -- facts, not a status
  matched_at, matched_by_id            -- a blind receipt reconciled later
```

Four things about this.

**`<= 1`, not `= 1`.** Precedent is the model's own Correction to D10: "an internal move has no demand-side cause at all". An unsolicited delivery has no demand-side cause for the identical reason. Today the ledger already permits a blind receipt (a `reason = 'receipt'` movement with no cause FK is legal) while the document cannot exist — the worst of both.

**The demand FK moved to the line.** One truck, three POs, one physical arrival. Header-level FK is the `consignment.fulfilment_id` mistake again: a second, weaker representation that silently forbids the consolidation it appears to model.

**`expected_quantity` is a snapshot.** Precedent is `stock_count.challenge_context` — model state frozen onto the fact at capture. Read it live and variance becomes unreproducible once the PO is amended.

**`accepted_at` is a fact, not a status.** Acceptance extinguishes the right to reject, and under the Food and Grocery Code that right expires 24 hours after delivery. A mutable status column cannot say "accepted at 14:32 by this person".

### `package_content` — the change that must happen first

```
package_content
  id, package_id
  item_id                              -- promoted: contents readable without a demand join
  fulfilment_line_id                   -- nullable
  goods_receipt_line_id                -- nullable
  CHECK (num_nonnulls(fulfilment_line_id, goods_receipt_line_id) <= 1)
  lot_id, quantity, catch_weight_g
```

`<= 1` because an ad-hoc container — a tote, a rebuilt pallet — has neither. This is the D10 idiom applied to the model's most load-bearing table, and the competitor analysis already warned that "changing what `package_content` contains after a year of shipped history is the migration nobody wants". It has no history yet. Do it now.

### Containment becomes a fact

```
package_event               -- a fact; append-only, D5 idempotency columns
  id, occurred_at, recorded_at, client_event_id, device_id, recorded_by_id
  package_id
  kind          -- created | contained | uncontained | moved | sealed
                -- | emptied | relabelled | superseded
  parent_package_id, location_id       -- both nullable
  source        -- label | asn | operator | derived
```

`package.parent_package_id`, `package.location_id` and a new `package.status` become **projections**, maintained by the same rebuild-and-assert job that guards `stock`. Nothing writes them directly.

Without this: "where was pallet P on 15 June" is unanswerable, per-pallet storage billing has no interval to query, an orphan SSCC scanned at the dock cannot be re-parented when the ASN arrives, and split/merge is a mutable column plus a delete. It is the only physical relationship in the model whose history is destroyed on update, and it is the one an inbound investigation has to walk.

### `item_barcode` — referenced twice in D19, never defined

```
item_barcode
  id, tenant_id             -- NULL = shared (a GTIN); set = internal or tenant override
  item_id
  barcode                   -- fixed-width text, zero-padded. Never numeric.
  kind                      -- gtin13 | gtin14 | itf14 | internal | supplier_ref
  unit_level                -- each | inner | carton | layer | pallet
  quantity                  -- base units per scan of this barcode
  active
```

This is the single most load-bearing undefined table for inbound — it blocks carton receiving and all scanning. Two things to get right.

**One unit vocabulary, four consumers.** `item.base_unit`, `order_line.entered_unit`, `item_packing_config`'s Ti/Hi rungs and `item_barcode.unit_level` are all reaching for an enum that does not exist. `entered_unit` is currently an unconstrained string.

**D19 needs one more sentence.** A carton GTIN's *level* is intrinsic (the brand owner assigns it), but the *count it implies* depends on the case pack a given tenant receives — which is D19's own reason `item_packing_config` is tenant-scoped. So: a shared row carries the brand owner's declared level and count; a tenant receiving a different case pack writes a tenant-scoped row that wins. Without that, one tenant's case pack silently rewrites another's scan arithmetic, which is exactly the poisoning D19 exists to prevent.

The GS1-128 parser is **code** — vendor `gs1-syntax-engine` (Apache-2.0, fuzzed, maintained), with the GS1 Syntax Dictionary imported as read-only shared seed data with a `release` column. That stays the right side of D13's line: an external standards body's table is not our logic as rows. Unrecognised AIs are stored opaquely and never rejected — rejecting one turns a future GS1 release into a dock outage, which D5 forbids.

### Put-away *(intention, scored in code)*

D13 pre-answered this by name: "when putaway, replenishment and disposition need the same treatment, they get the same shape: code that scores, configuration that weights."

```
putaway_policy              -- same shape as allocation_policy (D13)
  id, scope_kind, scope_id
  weight_travel, weight_consolidation, weight_access, weight_fit
  avoid_earlier_expiry_commingle
  allow_mixed_items, allow_mixed_lots
  max_equipment_class_id

location                    -- additions
  zone_id, sequence, pickable, blocked, active, storage_category_id

location_occupancy          -- a projection, maintained like `stock`
  location_id
  used_volume_mm3, used_weight_g
  distinct_item_count, distinct_lot_count
  inbound_committed_weight_g          -- from open work_task
```

We are better equipped than D365 on inputs: we hold `location.length_mm/width_mm/height_mm`, `max_weight_g`, `reachable_by`, coordinates and `location_edge`, plus `item.stackable`, `max_stack_height_mm`, `temperature_class`, `this_way_up` and `equipment_class.relative_cost`. Location directives can only match on attributes someone configured; a scoring function computes fit, travel and access cost directly. Oracle's `location_size_type` is a classification standing in for measurements they do not hold.

Work is `work_task(purpose = 'putaway')` plus a new `work_task.package_id` — inbound putaway is "scan the target LP and execute", and today the task has no way to name a container even though D6 promised `package` would serve putaway LPNs. Add `work_task.split_from_work_task_id` for lineage.

Two more, both small and both stolen from the reference set:

```
work_creation_outcome       -- why work was or was not created
  id, tenant_id, site_id, occurred_at
  trigger, inputs_summary, work_task_id, outcome, reason
```

"No work was created for wave X, see the work creation history log" is a routine, expected outcome in Manhattan's own support taxonomy. The model records what workers did (`activity_event`) and nothing about what the planner decided or declined to do. That is pure D8 — the non-event is a finding.

And the missing index: the stated list has `stock(item_id, location_id)`. Every putaway commingling check asks "what is already in this bin", which needs `stock(location_id, item_id)`. It is currently the wrong way round.

### Returns *(intention, then facts)*

```
return_authorisation        -- inbound from a customer
  id, tenant_id, site_id, customer_party_id
  order_id                  -- nullable: bought through a reseller
  reference
  claim_basis               -- change_of_mind | consumer_guarantee | warranty
                            -- | dead_on_arrival | recall
  requested_resolution, authorised_at, authorised_by_id, expires_at

return_authorisation_line
  id, return_authorisation_id, order_line_id, item_id, quantity
  customer_stated_reason

supplier_return             -- outbound to a supplier
  id, tenant_id, site_id, supplier_party_id
  goods_receipt_id, rma_reference, raised_at, status

fulfilment
  CHECK (num_nonnulls(order_id, transfer_order_id, supplier_return_id) = 1)
```

D16's own reasoning gives this for free: a supplier return differs from a customer order on the demand side only — the work and freight sides are identical. Third typed FK, nothing downstream duplicated.

Four independent axes on the return, and conflating them is the commonest returns-schema mistake: customer-stated reason, verified inspection finding, condition grade, and disposition action. The first two must both survive — a mismatch between "wrong item sent" and "used and damaged" is a fraud signal, and D8's whole complaint about auto-adjustment is that it "throws that information away at the exact moment it is most recoverable". Condition is `inventory_status` (D4). Disposition is an *action*, not a state, so it belongs on `stock_movement.adjustment_reason_id` — competitor recommendation 15, still undecided, and returns is where it bites hardest.

The main fraud control needs no new column: "did this lot come from us" is D14's stated recall query run backwards.

`reason = 'return'` currently means inbound customer return. Disambiguate it before a supplier return needs the enum value.

### Discrepancy, amended *(finding)*

```
discrepancy                 -- additions
  counterparty_party_id     -- whose finding is this against
  goods_receipt_line_id, stock_count_id, stock_movement_id, work_task_id
                            -- typed FKs replacing source_type/source_id (D10)
  package_id                -- a pallet advised and absent is not about an item
  expected_text, observed_text        -- non-numeric findings
  discovered_at_stage       -- at_gate | at_unload | at_count | at_inspection
                            -- | at_putaway | post_putaway
  disposition               -- accept | accept_and_notify | reject | quarantine
                            -- | return_to_vendor | destroy
  respond_by                -- NULL for internal findings; a clock for commercial ones
  kind += receipt_variance, unexpected_item, expiry_mismatch, batch_mismatch,
          identifier_suspect, label_non_conformance, unresolved_barcode
```

Plus the table D8 already promised and does not have:

```
attachment
  id, tenant_id, kind       -- photo | docket | pod | consignment_note | certificate
                            -- | weighbridge | temperature_log | label_payload
  goods_receipt_id, goods_receipt_line_id, discrepancy_id, package_id
  CHECK (num_nonnulls(...) = 1)
  uri, captured_at, captured_by_id, device_id
```

D8's prose says a discrepancy has "an owner, evidence and a resolution". There is no attachment, blob or image table anywhere in the model. A damaged-pallet claim without a photo is unwinnable, and photo capture at receipt-line grain is table-stakes in all three mid-market products.

### Vehicle arrival *(fact)* — small, and not the yard

```
vehicle_arrival
  id, tenant_id, site_id
  carrier_party_id, vehicle_registration, trailer_ref, container_ref
  seal_number, seal_intact, driver_name
  consignment_note_ref
  arrived_at, dock_location_id, departed_at
  pallet_count_declared, pallet_count_counted    -- the gate count
```

Many-to-many with `goods_receipt`. This is *not* the yard (see the skip list). It is here because Coles' first proof of delivery is a stamped driver's consignment note counted at the door before any electronic scan, Metcash requires a signed paper POD with a total pallet count on every delivery, and one truck legitimately carries several receipts. `location.kind` already has `dock`.

### Pallet pooling *(assertion + a second ledger)*

```
pallet_account
  id, tenant_id, party_id, provider    -- chep | loscam | vps
  account_number, site_id              -- accounts are per party AND per DC/commodity

equipment_movement                     -- signed deltas; D5's shape exactly
  id, occurred_at, recorded_at, client_event_id, device_id, recorded_by_id
  provider, equipment_code, quantity   -- signed
  from_account_id, to_account_id
  docket_number, effective_date, resolution  -- transfer | exchange | iou | no_account
  goods_receipt_id, consignment_id     -- nullable
  claim_deadline
  CHECK (from and to accounts share a provider)
```

Balance per (account, equipment_code) is a projection. This must **not** ride on `stock_movement` — D17 already made the argument for `activity_event`: "mixing them would put a `WHERE` clause on the sum that defines stock, and D5 exists to keep that sum unconditional." Hire equipment is not goods, and the pallet can leave under a different client's stock than it arrived under.

The docket is a counterparty assertion ("sender declares"), signed and dual-copy. `effective_date` is computed at receipt from `dc_delay_days(site, provider)` — CHEP and VPS use the receipt date, Loscam adds the DC's delay — and must be **frozen as a fact**, not recomputed when the reference data changes. Same reasoning that froze `package` dimensions.

### The full principle 2 mapping

| Entity | Category | Rule |
|---|---|---|
| `stock_movement`, `activity_event`, `package_event`, `equipment_movement`, `inbound_message`, `stock_count` | Fact | Append-only, immutable, projects to state |
| `purchase_order`, `transfer_order`, `return_authorisation`, `supplier_return`, `work_task`, `stock_allocation`, `lot_hold` | Intention | Mutable, cancellable, reconciled against facts |
| `inbound_shipment`, `asserted_unit`, `asserted_unit_content`, `equipment_transfer_docket` | **Assertion** | Immutable, never projected into `stock`, always names its author, exists to be compared |
| `discrepancy` | Finding | Where any two of the above disagree |
| `goods_receipt` | Grouping | Status derived from the ledger, never stored |
| `party_profile`, `putaway_policy`, `allocation_policy`, `tolerance`, `carrier_profile` | Policy | Scalars, scoped, resolved most-specific-wins; the logic is code |

---

## Where inbound stresses existing decisions

Ranked by how much it costs to discover late.

**1. D16's `goods_receipt` CHECK is wrong and needs correcting in writing.** `= 1` forbids blind receipt, unsolicited delivery, ASN-only receipt and RMA. The model already made and published this exact correction for `stock_movement`. Change to `<= 1`, widen to four sources, move the demand FK to the line. Also worth noting: D16's typed-FK idiom was argued against *one* alternative; at four or five arms it is visibly straining, and if the honest answer is a `kind` enum then that reverses D16 and should be written down rather than drifted into.

**2. D6 delivered the container and never widened the contents.** `package_content.fulfilment_line_id` is mandatory and outbound. This is a change to the model's most load-bearing table, on the migration path the competitor analysis explicitly flagged as the one nobody wants. It has no history. Fix before it does.

**3. Principle 2 needs a fourth category.** Every stream found it independently, from a different direction: the ASN, the supplier-asserted carton weight, the carrier ETA, the CHEP docket, the retailer's compliance requirement. The alternative reading — "intentions have an author, and a counterparty's intention is still an intention" — is cheaper but wrong: an ASN is a claim about goods that have *already been despatched*, not a plan. Name the category.

**4. D20 broke D12's invariant.** `stock` has `owner_id` in its key; `stock_movement` has no owner column. D12's "each column is rebuildable from its own source, and the existing reconciliation job asserts both" is false today. Add `from_owner_id`/`to_owner_id` mirroring D4's status pair — a pair, not one column, because title transfer without physical movement (a 3PL client's stock sold in situ, consignment converting on sale) is structurally identical to a status change. `stock_allocation` needs `owner_id` in its cell key too, or `allocated_quantity` cannot be maintained per cell.

**5. D8 survives a commercial counterparty structurally, and is under-dimensioned in three ways.** The principle gets *stronger* — a per-supplier error taxonomy is a commercial lever where an internal count variance is housekeeping. But: there is no `counterparty_party_id` (whose fault is unrecordable); `resolving_movement_id` presumes every resolution moves stock, when a supplier variance resolves as a credit or a chargeback; and **"non-blocking by default, raised asynchronously" is safe internally and lossy commercially**. An internal finding loses nothing by waiting. A commercial finding expires — Coles' 24-hour rejection window, 48-hour written reasons, 30-day claim limit under the Food and Grocery Code; Primary Connect disclaiming all pallet liability past 180 days; Walmart's quarterly chargeback dispute deadline. D8 has no representation of a finding that becomes worthless on a date. Amend it, and state that non-blocking still holds for internal findings.

Also: `discrepancy.source_type/source_id` is a polymorphic pair in the table D10 was written to de-polymorphise, and inbound quadruples the source set.

**6. Containment has no history (D6), and nothing reconciles `stock` against `package_content`.** Two independent representations of where stock is, no assertion job, and moving one pallet of 40 mixed cartons either writes 40 movements or the projection is wrong. Decide explicitly: does `package_id` join the `stock` key (a seventh column — expensive, and D20 already called six wide), or is `package_content` authoritative for containment and `stock` for balance with a reconciliation job? The silent third option — they drift — is what happens by default.

**7. D12/question 40 comes due, and the fix narrows a stated invariant rather than widening it.** Cross-dock, pre-receipt allocation and allocate-against-inbound all require a supply reference that has no `stock` cell. Either those allocations do not project onto `stock.allocated_quantity` (and availability stops being a single indexed read, which was D12's stated justification), or `stock` acquires rows for goods that are not there (and `quantity` stops being rebuildable). Q40 called this "a genuine generalisation of D12, not a patch" and deferred it to inbound. It is inbound. The cost was never stated.

**8. D13's line gets crossed by accretion, not by decision — and inbound is where the fifth chain arrives.** The model now wants most-specific-wins policy resolution for: allocation (D13), tolerance (D20 q55), min shelf life (D14 + q33/39), putaway, default receiving status, quality sampling, audit tiering, over-receipt. The competitor analysis warned in these words: "declining the engine while accepting five small rule tables is how you get a rules engine you never designed." Write **one** decision governing all policy resolution — one shared resolver, scopes as data, logic in code — rather than an eighth bespoke chain.

Separately, refuse D365's work templates and location directives outright. "A general saved-query structure (table, derived table, field, criteria, joins, sorting) as data" is a query DSL stored as rows: principle 1, 3 (in spirit), 4 and 6 simultaneously, plus the no-rules-engine refusal. But note the honest cost has changed since D13 was written: D13's premise was "one team, one site, a deployable codebase", and D18 made multi-tenancy non-negotiable and pointed at a commercial product. A tenant who wants "vendor X's goods go to zone 3" cannot have it without our deploy. That is defensible — opinionated software, fast defaults — but it is now a *product* decision, not an engineering convenience, and D13's reasoning does not address it.

**9. D5 makes "prevent over-receipt" unbuildable as stated, and the requirement is real anyway.** If 120 cartons are on the dock, 120 arrived; rejecting the scan discards a true observation to protect a database invariant. But the commercial requirement is not to block the *count* — it is to block the *acceptance*, and the model has no acceptance act distinct from the movement. Add `accepted_at`/`rejected_at` on the receipt line and let disposition write a D4 status. Goods refused at the gate and returned on the truck are a harder case: they never entered custody, so there is no movement, and `activity_event.kind` does not cover it. One of the two must widen, deliberately.

**10. Dangling references.** `item_class` is referenced by D13 (`scope_kind`) and D20 q55 (tolerance resolution) and defined nowhere. `item_barcode` is named twice in D19 with one column. Both are now blocking. Also: `device_id` appears on `stock_movement`, `activity_event` and `stock_count` and there is no `device` table — which is where an NMI approval number and a calibration date live, and a billing dispute becomes a question about the instrument.

**11. `measurement` is welded to dimensions, and three independent requirements want its idiom.** `metric` is (length, width, height, weight, cube) and `subject_type` is (item, package_type, package). It cannot hold: a supplier-asserted *quantity*, a carrier ETA with confidence, a receipt temperature against a spec, a quality measurement, a gross-versus-net-versus-tare distinction, or a measurement of a *packaging level* (the carton of item X is neither the each nor a generic preset). The provenance-with-confidence idiom is the model's most defensible original idea and it is trapped in one table. Worse, question 20's likely answer (typed subject FKs) closes the subject set at the moment inbound opens it. Decide whether it generalises before implementing q20.

**12. Principle 3's wording is a slow leak.** "JSONB is permitted for exactly one thing" and "`provider_exchange.payload` is the *only* JSONB in the model" are already contradicted in intent by the outbox payload (competitor q10, unanswered) and now by inbound message retention. Restate as a rule: opaque counterparty payloads retained for audit, never queried structurally, with queryable parts promoted to columns. An enumerated exemption erodes by exception, which is the accretion the principle exists to stop.

**13. Principle 4 versus regulatory forms.** Biosecurity discrepancy forms, imported pig meat notifications and baitfish forms have externally-defined field sets we do not control. Principle 4's bet — "adding a column is cheap and migrations are routine" — was made when this was one company's tool; D18 changed that premise. The defensible position is still no custom fields: one typed table per scheme, shipped by us, a new scheme is a release. But say so with the cost named, rather than discovering it when a tenant asks.

**14. Status columns with no history (competitor gap 8), never elevated to a decision.** `order.status`, `fulfilment.status`, `consignment.status`, `work_task.state`, `discrepancy.state` — the model is scrupulously append-only about stock and casually mutable about everything else. Inbound adds `goods_receipt`, `inbound_shipment` and `package.status`, and for `inbound_shipment` the transition history is the audit (a rejected-then-resent ASN). Settle it once, uniformly, before building three more.

**15. `external_reference(system, entity_type, entity_id, external_id)`** (open-decisions #4) is a polymorphic pair, which D10 rejected. D10's cold-path exception does not save it here: party-code and item-code resolution during ASN validation runs per message against a 3-hour acknowledgement SLA, so D10's argument 3 (batch loading cannot be expressed over a polymorphic reference) applies directly.

---

## The graceful degradation ladder

It works, cleanly, and D20 is the mechanism unchanged. The rungs:

| Rung | What arrives | Receiving flow | Enables |
|---|---|---|---|
| 0 | Nothing | Blind receipt: scan item, count, LPN generated | Ledger, findings |
| 1 | Paper docket, PO known | PO-line receipt, expected quantities available | Variance vs PO |
| 2 | Spreadsheet / portal keying | Line-level assertion, single root `asserted_unit` | Variance vs advised |
| 3 | ASN with SSCCs, no contents | Unit IDs known, contents unknown | Scan-to-identify, gate reconciliation |
| 4 | ASN with unit contents | One scan receives a pallet | Cartonised receipt, cross-dock |
| 5 | + lot, expiry, catch weight | Traceability captured in the same scan | FEFO putaway, recall from receipt |

What makes it one model rather than five:

- **`goods_receipt` never requires an `inbound_shipment`.** That is the whole ladder in one constraint, and it is the constraint D16 currently violates.
- **Blind is a flag, not a workflow.** Oracle exposes three receiving modes as screen parameters; ours is `goods_receipt.blind` plus `mode`, on one set of tables.
- **A flat CSV produces a one-level tree, not a different shape.** `asserted_unit` with a single root and contents attached is exactly the EDIFACT `CPS+1++4` "no packaging hierarchy" case the standard itself defines. The hierarchy tables must permit it without null-hostile constraints.
- **`granularity` is derived and drives what is offered.** Which receiving screen appears, whether SSCC scanning is enabled, whether cross-dock is feasible, how the discrepancy report is worded.
- **`party_profile` capability columns choose the flow per supplier** — D20's sentence with a different noun: "the dimension only exists on the records that need it." An operation whose suppliers all send paper never encounters any of the ASN machinery, and nothing is switched off in settings.

Two honest caveats.

**Capability degrades, not just UI.** Pre-receipt allocation and planned cross-dock genuinely require rung 4. That is not a defect of the model — it is a fact about information — but it should be said plainly rather than implied away, because it means the top-of-market capabilities sit on three unbuilt layers and are correctly sequenced as differentiators, not table stakes.

**The contracted rung and the delivered rung are different numbers, and the gap is a finding.** Store the contracted granularity on `party_profile`, the delivered granularity on each `inbound_shipment`, and raise the difference as a D8 finding. That is the compliance conversation, produced by three existing patterns rather than a new subsystem.

---

## Australian specifics

**What actually binds.**

- **GS1 identification is mandatory; the ASN is not.** GS1 Australia's retailer matrix marks GTIN barcodes and SSCC pallet labels M (mandatory) and puts EDI/despatch advice, National Product Catalogue and GS1 Recall in the *preferred* tier. Woolworths, Coles and Metcash all require SSCC labels on inbound and their DCs are heavily automated, so placement and print quality are rejection risks, not paperwork. Coles mandates GTIN-14 in ITF-14 on every master carton — a class identifier with no AIs, which is a different capture event from an SSCC scan and must not be collapsed into "barcode scanned".
- **DESADV, not 856.** Australian grocery uses EANCOM DESADV with nested CPS packaging sequences. Cardinality is *narrower* than the standard: Metcash and Coles both forbid an ASN spanning more than one PO, more than one destination or more than one truck. That fits `goods_receipt`'s singular demand FK comfortably. It is US/3PL 856 traffic, with its ORDER hierarchy level, that would break it — decide whether that is in scope or a recorded omission.
- **Realistic adoption is thin and portal-mediated.** Supplier-to-3PL DESADV in the AU mid-market is rare. The dominant channel for small suppliers is hosted web-EDI (MessageXchange, ~$99/month per retailer community) where the same web form produces the ASN *and* prints the SSCC labels — which is why they agree, and why they stop agreeing the moment anyone re-pallets. Build channel-agnostic ingestion with a DESADV-shaped canonical model; do not build an EDIFACT parser until a real DESADV exists.
- **Paper does not go away.** Metcash requires a signed paper POD with every delivery — two copies, per PO, carrying carton counts per line and total pallets — from fully EDI-compliant suppliers. Coles stamps the driver's consignment note as the initial proof of delivery before any electronic receipt. Three parallel channels (ASN, consignment note, delivery docket) that can and do disagree. This is why `vehicle_arrival` with a gate count is in scope while the yard is not.
- **CHEP and Loscam are a parallel ledger with hard clocks.** Woolworths Primary Connect's Equipment Control Policy (v2.0, Nov 2024) operates a sender-declares model with signed dual-copy transfer dockets, per-DC equipment acceptance matrices, and claim windows of 60 / 180 days beyond which liability is disclaimed. CHEP cannot net against Loscam. No competitor models this — CartonCloud users type pallet counts into ad hoc charge fields by hand, and the feature request has one vote. It is an uncontrolled loss line that can exceed the storage revenue on an account, and our core primitive (signed deltas projecting to a balance) fits it exactly. Best differentiator in the whole set.
- **Food and Grocery Code of Conduct, mandatory since 1 April 2025.** Binds Woolworths, Coles, ALDI and Metcash; ACCC-enforced. Fresh produce may be rejected only for written specification failure, only within 24 hours of delivery, only if not already accepted, with written reasons within 48 hours; damage and shortfall claims within 30 days. This is the concrete source of D8's missing clock, and of `accepted_at` needing to be a fact.
- **Biosecurity at an Approved Arrangement site.** For imports, DAFF licences by class (1.1–3.0), goods arrive under biosecurity control and cannot be released until a direction is satisfied — which is a D4 `inventory_status`, not a location, and `party`/`person` accreditation with an expiry. Commodity-specific discrepancy forms are the principle-4 pressure point noted above.
- **Chain of Responsibility (HVNL).** A consignee operating a dock is a CoR party, which turns queue time, dock delay attribution and appointment adherence into an evidentiary record. The device-clock/server-clock split (D5) and non-editable `recorded_by_id` (D11) already make our timestamps evidentiary rather than assertable — by accident, but correctly. This is the one reason to re-examine the yard skip, and it should be re-argued on this basis rather than inherited.

**What does not apply, despite the noise.**

- **Sunrise 2027 is about retail POS.** GS1-128 remains the mandatory carrier on logistic labels; 2D is strictly additive and must duplicate the linear content. Do not scope a 2D-only receiving path.
- **Prefix 930–939 says where the number was licensed, nothing about origin.** Goods made in China and registered by an Australian company carry 93x. Country of origin needs its own column on `lot` (it varies lot to lot for food, so `item` is the wrong home), sourced from the catalogue or the label's AI 422–427. Never derive supplier, country or ownership from prefix digits; GEPIR lookups are an advisory cache with a freshness timestamp, barred from operational joins.
- **No pharmaceutical serialisation mandate.** TGA recalls at batch level. Serialisation demand here is warranty and high-value goods, not regulation — which is exactly D14's "when a customer asks for it" trigger.
- **Peppol in Australia is invoice-only.** The ATO is the Peppol Authority and the mandate covers e-invoicing (PINT A-NZ); the despatch advice transaction is not in scope. Worth parsing eventually; not a channel to expect volume on. The one idea worth stealing is `OutstandingReason` on the line — "4 are coming" versus "forget the rest" is a counterparty intention no arithmetic over our facts can produce.
- **No mandated goods-movement document.** Unlike Italy's DDT or India's e-way bill, Australia imposes zero localisation burden on the receipt itself.

---

## Worth deliberately skipping

- **Yard management, appointment scheduling, dock slot booking, detention and demurrage.** Already on the skip list; re-record it against inbound rather than inheriting the "downstream of subsystems we do not have" argument, which was written before inbound existed as scope. Keep `vehicle_arrival` and the gate count — they are contractually required in AU and are not the yard. Note if it is ever built: `location.kind` already has `dock` and `staging` and `location_edge` is a real graph, so yard positions are location rows, not a second location model; and `load` must be self-referencing on day one because B-doubles and road trains mean 2–3 trailers per prime mover.
- **Work templates and location directives (D365), and putaway priority tables (Oracle).** The rules engine, fully assembled. Ship a scoring function, not a rules editor.
- **Planned cross-dock templates** with ordered supply-source lists and demand-requirement enums. The time windows and FEFO expiration range are `allocation_policy`-shaped scalars and are fine; the template is not.
- **Three-way match, GR/IR accrual, purchase price variance, landed cost, valuation layers.** NetSuite remains the financial system. Our obligation is an accurate, timestamped, non-rewritable receipt quantity, which we do better than most ERPs.
- **3PL billing — but say so explicitly, because the premise changed.** The competitor analysis skipped rate cards and storage periods on the stated grounds that "we ship our own goods". D20 voided that. Either write "we hold third-party stock but do not bill for it — the client's own system does", or accept that rate cards with stepped bands, effective dating, charge triggers and product-profile scope are *logic as rows* and the largest missing subsystem is revenue, not documents. Do not leave it as an omission inherited from a dead premise.
- **Unit-level serialised inventory at receipt.** D14's boundary holds, and the AU regulatory driver is absent. But note the asymmetry it creates: D14 blesses pack-time serial capture as "one table hanging off `package_content`", while a supplier sending AI 21 on a logistics label has nowhere to put it at receipt. Decide the cheap inbound equivalent explicitly rather than excluding it by silence.
- **Scan4Transport AIs 4300–4333.** Parcel/courier scope, thin adoption, and the natural shape is an `ai_code`/value child table principle 4 refuses. Accept them into the unrecognised-AI overflow so they never cause a dock outage; model none of them.
- **Structured discrepancy back-channels (X12 861 / EANCOM RECADV).** The live AU 861 implementation in production carries line number, quantity and UOM — no condition code, no damaged quantity, no rejected quantity. Do not build a discrepancy pipeline that depends on inbound EDI populating condition codes. Build the manual capture path with photos.
- **Backorder documents, split-PO chains, sibling receipts.** D12 already eliminated backorder as an entity; the inbound arithmetic is identical. CartonCloud needs its `PO12345 - 2` machinery only because its billing keys off the order.
- **Per-tenant CSV column mappings.** Already refused as "integration templates as data". Ship one or two code-defined versioned formats with generated schema docs.
- **Returns P&L.** Keep the linkage (return line → fulfilment line → order line → customer, plus the disposition that determined recovery); leave the accounting in NetSuite.
- **Cross-tenant supplier benchmarking.** The most valuable thing a multi-tenant WMS could sell back to its customers, and D19 as written forbids it — supplier performance is observed data, therefore tenant-scoped, no NULL option. Almost certainly the right call on privacy grounds, but it is a product decision currently being made silently by a data-scoping rule.

---

## Prioritised recommendations

**Tier 0 — cheap now, ruinous later. Do before any inbound code.**

1. `package_content` gains `item_id` and typed demand FKs with `CHECK <= 1`. The migration nobody wants, and it has no history yet.
2. `stock_movement.from_owner_id` / `to_owner_id`. Restores D12's invariant, which is currently false.
3. `goods_receipt` CHECK to `<= 1`, widened to four sources; demand FK on the line.
4. `entered_quantity` / `entered_unit` / `item_packing_config_id` on `goods_receipt_line` and `stock_movement`. Without the config FK, correcting a packing config silently rewrites the meaning of every historical receipt.
5. Define `item_barcode` and the shared unit vocabulary (one enum, four consumers).
6. Define `item_class`. Two decisions reference it; nothing defines it.
7. `lot.country_of_origin` and `lot.production_date`. Not backfillable — same argument that got `package_content.lot_id` in early.
8. `number_range(key, prefix, next_value)` scoped to `legal_entity`, row-locked, with the GS1 12-month reuse guard. `package.sscc` has no issuer today.
9. Index `stock(location_id, item_id)`. Currently the wrong way round for every putaway and commingling check.
10. Decide `stock_movement.unit_cost_minor` (competitor rec 15's sibling, rec 17). Insurance, not accounting — but history before the decision date is unrecoverable.
11. Add `promised_from` / `promised_to` / `required_by` to `order` and `order_line` while you are in there. Lateness is currently undetectable on both sides.

**Tier 1 — build with inbound.**

`party` + `party_role` + `gln` + `party_profile`; `purchase_order` + lines; `inbound_shipment` + `asserted_unit` + `asserted_unit_content`; `goods_receipt` + lines; `attachment`; the `discrepancy` amendments (counterparty, `respond_by`, typed source FKs, non-numeric fields, `package_id`); `package_event` with `parent_package_id`/`location_id`/`status` demoted to projections; `vehicle_arrival` with the gate count; `work_task.package_id`; `inbound_message` + `processed_import` idempotency.

**Tier 2 — soon after.**

Putaway scoring + `putaway_policy` + `location_occupancy` + `location.zone_id/sequence/pickable/blocked/active`; `work_creation_outcome`; the CHEP/Loscam equipment ledger; `return_authorisation` + `supplier_return` + the four returns axes + `adjustment_reason_id`; supplier scorecards as a maintained projection; the shared policy resolver decision.

**Tier 3 — wait, deliberately.**

Cross-dock and pre-receipt allocation (blocked on question 40 being decided, not on effort); quality orders and inspection facts (blocked on the observation-table decision); SSCC issuance and outbound DESADV (blocked on whether grocery B2B is in the roadmap); EDIFACT and X12 parsers (blocked on a real trading partner existing).

---

## Open questions

> **Questions raised in this document have moved to [open-questions.md](./open-questions.md)**, the single register and the canonical numbering. The entries below are retained as written; the register is authoritative on status.


Continuing the doc's numbering.

58. **Is `purchase_order` ours, or a NetSuite mirror?** Same fork as question 4, and it decides the size of the entire build. If purchasing stays in NetSuite, `supplier_item` catalogues, lead times, reorder logic and most of the demand side are out of scope and `purchase_order` should be as deliberately thin as `order`.
59. **Does `package_id` join the `stock` key, or is `package_content` authoritative for containment with `stock` authoritative for balance and a job asserting agreement?** The silent third option is drift, and it is what happens by default.
60. **Question 40, now due.** Does `stock_allocation` gain typed supply-side FKs (cell | `purchase_order_line` | `asserted_unit_content` | `goods_receipt_line` | `transfer_order_line`), and what happens to `stock.allocated_quantity` and `available_quantity` for allocations with no cell? This narrows a stated invariant. Decide it knowing that.
61. **Principle 2: fourth category, or "intentions have an author"?** Recommendation is the fourth category. Either way it must be written before `inbound_shipment` is built, because it decides whether an ASN is one table or two.
62. **Does D8's non-blocking rule get a counterparty carve-out, and where does the clock live** — `discrepancy.respond_by`, or a Nosdesk SLA per D7? A finding that becomes worthless on a date is not a category D8 has.
63. **One shared policy resolver, or N bespoke chains?** Allocation, tolerance, shelf life, putaway, default receiving status, quality sampling, audit tiering. The eighth is where D13's line gets crossed by accretion.
64. **Does the `measurement` provenance idiom generalise into a typed observation table** (temperature, quality measures, counterparty-asserted quantities, ETAs, packaging-level dimensions), or stay dimension-only with each new case getting its own columns? Three-times-requested. Decide before question 20 closes the subject set.
65. **Is a 3PL client an owner scope *inside* a tenant?** If so, D19's "two policy shapes, applied by category" becomes three, `person` needs a relationship to `party`, and `item`/`item_barcode`/`measurement`/`item_packing_config` need owner scoping — because D19's own argument for why measurements cannot be shared across tenants applies verbatim one level down.
66. **Do we bill for 3PL storage and handling?** D20 admitted the stock; the skip list still declines the billing. Holding third-party stock without billing for it is not a business.
67. **Cross-tenant supplier benchmarking: forbidden, or a product capability needing an aggregation carve-out?** D19 as written says forbidden. Make it a decision, not a side effect.
68. **Multi-PO ASN and the US 856 ORDER hierarchy level — in scope, or an explicit omission?** AU cardinality fits `goods_receipt` unchanged. US and 3PL traffic does not.
69. **Is grocery B2B in the roadmap?** Competitor question 6, still unanswered after twenty decisions. It decides whether SSCC issuance, GS1 prefix management and outbound DESADV are urgent. `package_content` still has no history, which is the good case.
70. **Statutory retention versus tenant data deletion** (question 47). HVNL, food traceability and biosecurity all require retention past any tenant's right to be forgotten. Needs a written position before the first tenant asks.
71. **Serial capture at receipt.** D14 excludes it by silence while blessing the pack-time equivalent. A supplier sending AI 21 has nowhere to put it. Decide explicitly.
72. **Does a mandatory dimension block putaway?** Question 31's enforcement answer extends to expiry and country of origin, but "blocks putaway" is a coordination stance D5 would reject — the stock is physically on the shelf whether or not we captured the date. Recommendation: accept and raise a finding. State it, so it does not get re-litigated at the dock.
