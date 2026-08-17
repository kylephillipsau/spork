# Domain model

Companion to [warehouse-data-model.md](./warehouse-data-model.md) (which covers
*what data we need and where it comes from*). This covers *how it is shaped*.

Working document. Table sketches are indicative — enough to argue with, not a
migration.

## Design principles

These are the rules we hold ourselves to. They exist because the failure mode is
known: NetSuite is capable and awful, and it got that way one reasonable
compromise at a time.

**1. Few primitives, composed.** The target is a small set of orthogonal
concepts that combine, not a concept per business noun. If a new feature needs a
new table, that is a signal worth examining — most should be new *queries*.

**2. Facts, intentions and findings are three different kinds of thing.**
*(Restated 2026-07-30 — see D12. The original claimed only two things happen in a
warehouse: stock moves and things get measured. That was wrong, and the
competitor analysis was right to break it.)*

- **Facts** are what happened. Append-only, immutable, never edited. They project
  to current state. `stock_movement`, `measurement`, `stock_count`,
  `activity_event`.
- **Intentions** are what we plan. Mutable, cancellable, and reconciled against
  facts as reality arrives. `stock_allocation`, `move_task`, `purchase_order`.
- **Findings** are where the two disagree. `discrepancy` (D8).

The rules differ by category, and keeping them apart is what stops an intention
being quietly recorded as a fact. Facts give us audit, reconciliation and history
for free. Intentions are allowed to be wrong — that is what makes them plans
rather than lies. Findings are the most valuable output of the system.

**Assertions are a fourth provenance value** *(D21)* — a statement of record
exchanged with another party, stored exactly as exchanged, which neither side may
unilaterally revise.

**There is a second, orthogonal axis: role** *(lifted in with D25)*.

| Axis | Values | Decides |
|---|---|---|
| **Provenance** | fact \| intention \| assertion \| finding | mutability, who may author, what may project from it |
| **Role** | reference \| projection \| policy \| grouping | how it is read, indexed, rebuilt, and who may write it |

Every table registers exactly one value on each axis. This is what stops the
category list growing: `goods_receipt` is a **grouping**, `stock` is a
**projection**, `allocation_policy` is **policy** — none of them is a new kind of
thing, and each kept looking like one only because the role axis was missing.

**The provenance axis is not declared closed — it has an admission test.** The
count has gone two, three, four, and each time completeness was asserted rather
than argued. Four is defensible (what we observed, what we plan, what someone else
stated, where those disagree) but it is not proved, and findings sit awkwardly:
ours and mutable, like intentions. So, as with every other limit here:

> A fifth provenance value is admitted only if it changes **who may write the row,
> whether the row may be revised, and what may project from it** — all three. If
> it changes none of those, it is a role value or a `kind` column. If it changes
> only one, argue it explicitly rather than adding a category.

That is the test the role axis passes and `goods_receipt` fails.

**3. Nothing we query is opaque, and the model contains no `jsonb` column.**
*(Restated with D21.)* Payloads that crossed a party boundary are retained
verbatim for audit and are never queried structurally — but they are retained as
**`bytea`**, not JSONB, with `content_type` saying what they are. EDIFACT is
bytes; a photographed docket is bytes; JSON is bytes. Anything queryable is
promoted to a column.

The point of the type change is that it converts a judgement into an assertion:
**CI checks the schema contains no `jsonb` column at all**, rather than a reviewer
deciding per column whether this one is "really" opaque. "We'll make it flexible
with JSON" is the first step toward the mess we are replacing, and an enumerated
exception list erodes by exception.

**4. No entity-attribute-value, and no untyped attribute soup — but a tenant may
declare a typed scheme that compiles to real columns.** *(Amended by D26.)* The
original wording refused custom fields because "adding a column is cheap and
migrations are routine". That sentence has a hidden subject: cheap **for us**, and
D18 made the subject non-universal. The refusal was never really about columns; it
was about deferring *type* decisions to runtime. So the boundary is **what a
column may be**, not who may add one: real types, real CHECKs, real foreign keys,
real indexes, declared up front and compiled to DDL. A generic attribute system,
a spare-column sidecar, or JSON-with-a-schema remains refused.

The original reasoning, which still holds for us: adding a column is
cheap and migrations are routine. A generic attribute system is how you get a
schema that cannot be read and queries that cannot be optimised.

**5. Canonical units, integers.** Lengths in **millimetres**, mass in **grams**,
money in **minor units**, all as integers. Conversion happens at the edges.
Floating-point dimensions and money are a permanent source of drift and
off-by-a-cent bugs. (The label maker already resolves everything to millimetres —
same convention.)

**6. Queries are designed with the schema.** The quality bar is one screen,
sub-second. That means for each screen we know its query. If a screen needs a
dozen joins, the model is wrong — we find that out now, on paper, not after the
N+1 shows up in production.

**7. Deliberate omission is a feature.** Every capability we do not build is
complexity we do not carry. See "What we are deliberately not building".

## One word that meant three things

*(Split 2026-08-04. Nothing about the design changed; one noun was doing three
jobs and the schema comments were the worst place for it.)*

**Operator** is the customer side: the people who run a warehouse with this
system, from the picker holding the scanner to the manager deciding what counts
as a variance worth raising. D11 built the attribution vocabulary on it,
separating the operator from the workers and the accountable; D24's
`package_event.source` carries `operator_scan` as a value; D9 gives the operator
the tolerance call. The outward documents use it the same way. That sense keeps
the word.

**Platform** is us: whoever runs this system for the tenants on it. It owns the
rows no tenant may write, which are the ones carrying `tenant_id IS NULL` in a
table that otherwise has one. `number_range` and `extension_slot` are
platform-owned. A policy binding with a NULL tenant is platform-shipped, and a
tenant's own binding always beats it. Issuing a tenant more extension slots is a
platform act with a row and a timestamp behind it.

Those two were both called operator until now, including in the same file, and
`OPERATOR-OWNED (tenant_id IS NULL)` read as though a picker owned the row. The
collision would have got worse rather than better, because the platform sense is
the one that grows: every question about who may approve a schema extension, who
holds a company prefix, or who sees across tenants is a question about the
platform.

The third sense is an operator such as "greater than", which appears in D13 and
D22's rule that a setting may never name a database field or an operator. That is
standard usage and the sentence disambiguates itself, so it stays.

## The spine: two fact tables

Everything else hangs off these.

### `stock_movement` — every change in where stock is

```
stock_movement
  id
  occurred_at
  item_id
  quantity                -- signed, in the item's base unit
  from_location_id        -- null = entered the system (receipt)
  to_location_id          -- null = left the system (despatch, write-off)
  lot_id                  -- nullable
  reason                  -- enum: receipt, putaway, pick, pack, despatch,
                          --       adjustment, stocktake, transfer, return
  reference_type          -- what caused it (fulfilment, receipt, adjustment...)
  reference_id
  actor_id
```

**Stock on hand is the sum of these.** We keep a materialised `stock` table
(item × location × lot → quantity) updated in the same transaction as the
movement, because summing history on every read does not stay fast. The
invariant is that `stock` is always rebuildable from `stock_movement`, and a
periodic job asserts it. That is the reconciliation story, and it exists from
day one rather than being bolted on when the numbers first disagree.

Nothing writes to `stock` directly. Ever. That single rule is what keeps
inventory explicable.

### `measurement` — every physical observation

```
measurement
  id
  observed_at
  subject_type            -- enum: item, package_type, package
  subject_id
  metric                  -- enum: length, width, height, weight, cube
  value                   -- integer, canonical unit
  source                  -- enum: measured, supplier, derived, carrier_actual,
                          --       operator_correction, estimated
  confidence              -- enum or small int
  actor_id
  note
```

This is the "dimensional data is a living asset" model made concrete. The four
feeds from the data model doc are just `source` values. Nothing is overwritten —
a new observation supersedes an old one, and the history explains itself.

Current dimensions are a projection: the most recent, highest-confidence
measurement per (subject, metric). Materialised the same way as `stock`, for the
same reason.

**Why this matters:** when a carrier re-weighs a consignment and bills us for the
difference, that is a `measurement` with `source = carrier_actual`. Comparing it
to our prediction is then a query, not an integration project — and it both
finds bad records and quantifies what they cost.

## Catalogue

```
item
  id, code, description, base_unit, active
  dangerous_goods_class, un_number, packing_group   -- nullable
  temperature_class, stackable, max_stack_height_mm
  this_way_up

item_packing_config           -- how an item is presented for shipping
  id, item_id
  units_per_inner
  inners_per_carton
  cartons_per_layer           -- the classic Ti
  layers_per_pallet           -- the classic Hi
  package_type_id             -- what it ships as
  effective_from
```

Dimensions live in `measurement`, not here. That is deliberate: an item's weight
is an observation with a provenance, not an attribute someone typed once.

`item_packing_config` is versioned by `effective_from` so a consignment shipped
last year can still explain its own dimensions.

## Packaging

```
package_type                  -- the preset catalogue
  id, name                    -- 'PALLET', 'SKID', 'small box', or an item code
  carrier_package_code        -- 'PAL' etc.
  dimensions_fixed            -- boolean: a box's are, a pallet's height is not
  tare_weight_g
  effective_from
```

Default dimensions are `measurement` rows with `subject_type = package_type`.
Same provenance model, one mechanism.

## Space

```
site
  id, name, timezone

location
  id, site_id, code
  aisle, bay, level, position        -- parsed, not just a code string
  x_mm, y_mm, z_mm                   -- for the map and the router
  length_mm, width_mm, height_mm
  kind                               -- pick_face, bulk, staging, dock, overflow
  max_weight_g
  reachable_by                       -- equipment class

location_edge                        -- the traversable graph
  from_location_id, to_location_id, distance_mm, bidirectional
```

Coordinates and the graph are needed only by the map and the route optimiser.
They are nullable and can stay empty until the survey happens — which is what
stops the survey blocking fulfilment work.

## Fulfilment

This is where NetSuite spreads one flow across four record types and loses the
package count entirely. The chain here is **order → fulfilment → package →
consignment**, with one join table that NetSuite has no equivalent of.

```
order
  id, customer_id, confirmation_number, contact_id, site_id, placed_at, status

order_line
  id, order_id, item_id, quantity_ordered

fulfilment                     -- a commitment to ship part of an order
  id, order_id, site_id, status
  picked_by_id, packed_by_id, picked_at, packed_at

fulfilment_line
  id, fulfilment_id, order_line_id, quantity

package                        -- a physical parcel. First-class.
  id, fulfilment_id, package_type_id
  length_mm, width_mm, height_mm, gross_weight_g
  dimensions_source            -- computed | confirmed | corrected
  barcode
  sequence                     -- 1 of 3, 2 of 3

package_content                -- WHAT IS IN THE BOX
  id, package_id, fulfilment_line_id, quantity
```

**`package_content` is the capability NetSuite does not have.** Because packages
are real rows rather than a count typed into MachShip at the last moment, and
because contents join back to fulfilment lines, we can answer:

- what is physically in this carton (packing list, per package)
- which package a damaged or missing item was in (carrier claim)
- what actually arrived when a delivery is partial
- whether a package's declared weight matches its contents (before the carrier
  tells us, expensively)

That last one is a query against `measurement` and `package_content`. In NetSuite
it is not answerable at all.

Note that `package` carries denormalised dimensions rather than reading them from
`measurement`. That is intentional: a shipped package's dimensions are a
historical fact about that consignment and must never change when a preset is
later corrected. `dimensions_source` records whether they were computed from
packing config, confirmed by an operator, or corrected — which is also feed 2.

## Freight

The D1 decision — MachShip now, Swift and Direct direct later — is a schema
question, and this shape makes the migration a data change rather than a rewrite.

```
carrier                        -- Swift, Direct, and the rest
  id, name, code

freight_provider               -- HOW we reach a carrier
  id, name, kind               -- machship | direct_api
  
carrier_service
  id, carrier_id, name, code

consignment
  id, fulfilment_id
  carrier_id                   -- who is carrying it
  freight_provider_id          -- who we booked it through
  carrier_service_id
  provider_consignment_id      -- MachShip's id, or the carrier's
  carrier_consignment_number
  despatch_at, eta
  price_minor, currency
  status

consignment_package            -- packages on this consignment
  consignment_id, package_id

```

*(`provider_exchange` was replaced by `party_message` in D21 — one table for every
payload that crossed a party boundary, inbound or outbound, stored as `bytea`.
`consignment.status`, `.eta` and `.price_minor` became projections of the in-force
carrier advice at the same time.)*

**Separating `carrier` from `freight_provider` is the whole trick.** Swift is a
carrier today reached via MachShip; tomorrow it is the same carrier reached
directly. Consignment history stays coherent across the switch, and "what did
Swift cost us this quarter" is one query that spans both eras.

### Carrier rules, without a rules engine

Swift needs 07:00 next-business-day despatch and a caller value of `Alpha`.
Direct needs two labels when palletised. The temptation is a config blob or a
rules engine. For a handful of carriers, both are bloat.

```
carrier_profile
  carrier_id
  default_despatch_time, default_despatch_day_offset
  caller_value
  labels_per_pallet, labels_per_carton
  requires_manifest
  requires_dg_declaration
```

Explicit columns. When a carrier needs something genuinely new, add a column.
If this table ever reaches thirty columns of one-carrier-only flags, *then*
reconsider — but design for the five carriers we have, not the fifty we imagine.

## What emerges

The point of principle 1. These are features we do not build so much as query:

| Capability | Falls out of |
|---|---|
| Stock history, traceability, audit | `stock_movement` existing at all |
| Reconciliation against NetSuite | `stock` rebuildable from movements |
| WMS dimension autofill | `item_packing_config` + `measurement` |
| Packing calculator | the same two, used to decide rather than describe |
| Freight cost validation | `measurement(carrier_actual)` vs our prediction |
| Per-package packing lists | `package_content` |
| Carrier damage claims | `package_content` + `consignment_package` |
| 3D map occupancy | `location` coords + `stock` |
| Pick routing | `location` coords + `location_edge` + `stock` |
| Dimensional data quality reporting | `measurement.source` + `confidence` |
| "What did Swift cost us" across the MachShip/direct switch | `carrier` vs `freight_provider` |

Eleven capabilities, roughly fifteen tables, no rules engine and no custom-field
framework.

## Query discipline

Principle 6, made specific — this is the answer to "no bandaid N+1s".

**Know the screen's query before building the screen.** The packing station is
the test case: scan a confirmation number, and one round trip should return the
order, its fulfilment, its lines, the items, their packing configs, current
dimension projections, and the available package presets. That is a handful of
joins against indexed foreign keys, and it should be written and explained before
any UI exists.

**Batch, never loop.** With Diesel that means `belonging_to` plus `grouped_by`
for children, not a query inside a `for`. Worth an explicit review rule: any
database call inside a loop is a defect unless argued for.

**Projections are tables, not views over views.** `stock` and current dimensions
are materialised and maintained transactionally. Stacked views are where query
plans go to die.

**Indexes designed with the query.** At minimum: `stock_movement(item_id,
occurred_at)`, `stock(item_id, location_id)`, `measurement(subject_type,
subject_id, metric, observed_at desc)`, `package_content(package_id)`,
`consignment(carrier_id, despatch_at)`.

## What we are deliberately not building

Principle 7. Named so they are decisions rather than oversights:

- **A custom-field framework.** Add columns.
- **A workflow/rules engine.** `carrier_profile` columns until proven insufficient.
- **A generic document model.** Orders, fulfilments and consignments are
  different things and benefit from being different tables.
- **Multi-currency**, until a second currency exists. `currency` is recorded;
  nothing converts.
- **A full double-entry inventory ledger.** `stock_movement` is a movement log,
  not accounting. NetSuite remains the financial system for now.
- **Serial-number tracking**, unless it turns out to be required. Lot/batch is
  modelled because food likely needs it; serials are a much heavier commitment.

## Decisions

Taken 2026-07-30 in response to [competitor-analysis.md](./competitor-analysis.md).

### D4 — Inventory status joins the `stock` key

Accepted as flagged. `inventory_status(id, name, is_available_for_allocation)`;
`status_id` in the `stock` key; `from_status_id`/`to_status_id` on
`stock_movement` mirroring the location pair. Everything defaults to `available`.

Also fixes a live correctness bug: `reason = 'return'` currently restocks
straight into pickable inventory with no inspection state.

### D5 — The ledger is a CRDT. There is no offline mode.

**Decision.** Real-time synchronisation, with the movement ledger given
**operation-CRDT semantics** so that a dropout degrades gracefully instead of
triggering a separate offline code path. We do **not** build an offline-first
handheld app with a queue-and-replay design.

**The reframe.** The scanner is observing physical reality; the database is a
model of it. **When they disagree the scanner is usually right.** A scan is
therefore not a request to be validated against system state — it is a
*delta asserted at the place and time the physical event happened*.

**`stock_movement` with `client_event_id` already is the right CRDT.** An
append-only set of uniquely-identified signed deltas, projecting to a counter,
is an op-based PN-Counter: commutative, associative, idempotent. Order of arrival
does not matter and replay is a no-op. So gap 6 is not an idempotency bandaid
bolted onto a ledger — **it is the column that makes the ledger convergent.**
That reframing is why it is non-negotiable rather than nice-to-have.

Columns: `client_event_id` (unique), `device_id`, `recorded_at` (server clock,
distinct from `occurred_at` device clock).

**Yjs/`yrs` is the wrong tool for stock, and we should not force it.** Nosdesk
uses Yjs for collaborative documents, which is what Yjs is excellent at. But
`Y.Map` fields are last-writer-wins registers: if two pickers concurrently pick
the last unit, LWW **silently discards one pick**. That is the exact failure the
ledger exists to prevent. Document CRDTs are for shared mutable state; a warehouse
ledger is accumulated immutable facts. Different primitive, deliberately.

What *is* reusable from Nosdesk is the layer underneath: the WebSocket sync
transport, presence, reconnection and backpressure handling in `backend/src/sync`
and `handlers/collaboration.rs`. That is transport, and it is CRDT-type-agnostic.

**Convergence is not invariant preservation.** This is the honest limitation and
it must be designed for, not discovered. A CRDT guarantees all replicas agree; it
cannot guarantee `stock.quantity >= 0`, because enforcing that requires
coordination and coordination is what we are giving up. Two pickers taking the
last unit will converge on **-1**.

**We allow it.** Negative stock is not corruption — it is a *discovered
discrepancy*, and it is information: someone physically picked stock the model did
not know about, so the model was wrong. Negative balances surface as exceptions
for resolution rather than being rejected at write time. Rejecting the write would
mean discarding a true observation about the physical world to protect a database
invariant, which is precisely backwards.

**Counts are assertions, not deltas** — a stocktake says "there are 47 here",
which is an absolute claim about state, a different CRDT class (a register, not a
counter). Mixing assertions with deltas naively is where this design breaks: any
movement landing mid-count silently corrupts a computed adjustment.

**Resolved by D8**: a count does not adjust anything. It is recorded as an
observation, and the variance against the ledger becomes a *finding* that a human
resolves. The adjusting movement is then written by the resolution, carrying a
reason. See D8.

### D6 — `package` becomes a container

Accepted as flagged. `fulfilment_id` nullable; add `location_id`,
`parent_package_id`, `is_mobile`, `sscc`; `package_type` gains `reusable`,
`max_payload_g`, `max_cube_mm3`. Nesting constrained to depth 2 (pallet → carton)
so `package_content` queries stay non-recursive.

One primitive serves shipped parcels, pallets of cartons, picking totes, putaway
LPNs and put-wall cells — principle 1 arguing for building it once, properly.

### D7 — Warehouse tasks are not Nosdesk tickets, but exceptions are

**Decision.** `move_task` is its own table in this system. We do **not** model
warehouse work as Nosdesk tickets. But **Nosdesk is the escalation target**: when
a task fails — short pick, damage found, location empty, count mismatch — that
becomes a ticket, with the task as its origin.

**Why not conflate.** They differ on every axis that matters:

| | Warehouse task | Nosdesk ticket |
|---|---|---|
| Origin | Machine-generated | Human-authored |
| Volume | Thousands/day | Tens/day |
| Lifetime | Seconds to minutes | Hours to days |
| Completion | Defined condition | Negotiated |
| Shape | (item, from, to, qty) | Conversation |

Forcing one table to be both means a ticket schema carrying warehouse columns and
a warehouse hot path carrying comment threads. That is the accreted-complexity
failure this project exists to avoid.

**Why the seam is genuinely valuable.** Exceptions are exactly the warehouse
events that *do* need conversation, assignment, history and SLA — which is what
Nosdesk already is. A short pick that becomes a ticket, routed to the right
person, with the pick task linked, is better than anything in the competitor set.
None of them have a real exception-management surface; they have status codes.

**What we share.** The stack, not the entity: Rust/Actix/Diesel/Postgres, auth,
the sync transport (D5), and the plugin SDK. Whether that means a shared workspace
or a service boundary is open — see question 14.

### D8 — Discrepancy is a designed output, not a failure mode

**Decision.** The system does not try to prevent physical reality from diverging
from the model. It **surfaces divergence with enough context to investigate**,
while the warehouse keeps running. Discrepancy is a first-class entity with an
owner, evidence and a resolution — not an error state, not a silent correction.

**The reasoning.** D5 framed negative stock as an unfortunate consequence of
choosing convergence over coordination. That framing was too defensive. In a
system that records **what was actually done**, a discrepancy is not a flaw in
the design — it is the most valuable thing the system produces. A mismatch means
something physical happened that nobody recorded: stock damaged and not reported,
stock never delivered, a mis-pick, a mislabelled pallet. Every competitor treats
these as adjustments to be reconciled away. Making them *findings to be
investigated* is a capability none of them offer.

**Three consequences.**

**1. The work event is the invariant.** What stock was taken, from where, by
whom, for which order is created by the person doing the work and is **never
rewritten**. Reconciliation does not edit or delete movements. A correction is a
*new* movement carrying `reverses_movement_id` and a reason. If a balance goes
negative we do not undo the pick — the pick happened; the model was wrong.

**2. Accountability is a schema requirement, not a nice-to-have.** Investigation
needs to reach a person, so every movement and every scan carries an individual
`actor_id`, plus `device_id`, `occurred_at` (device) and `recorded_at` (server).

This makes the current practice a data-quality defect to fix rather than mirror:
NetSuite's `Picked By: Casual Melbourne` is a **crew**, not a person, and against
a crew-level actor every accountability and labour column is decorative. Owning
the pick path (per floor-devices.md) is what makes individual attribution
possible, so it is a prerequisite for this decision paying off.

**3. Counts create findings, not adjustments.** A stocktake asserts "this much
exists here". Comparing it to the ledger produces a **variance**, and a variance
has candidate explanations worth surfacing — damaged and unreported, not yet
delivered, mis-picked, put away in the wrong bin. Auto-adjusting throws that
information away at the exact moment it is most recoverable.

```
stock_count            -- the assertion, preserved forever
  id, location_id, item_id, lot_id, status_id
  counted_quantity, counted_at, actor_id, device_id, blind

discrepancy            -- the finding
  id, kind              -- negative_balance | count_variance | short_pick
                        -- | damage | unexpected_stock | receipt_variance
  item_id, location_id, lot_id, status_id
  expected_quantity, observed_quantity, variance
  detected_at, detected_by_id
  source_type, source_id            -- the count, movement or task that raised it
  state                 -- open | investigating | resolved | accepted
  resolution_reason_id, resolved_at, resolved_by_id
  resolving_movement_id             -- the adjustment, if one was needed
  ticket_id                         -- escalation, per D7
```

The ledger stays pure: the count never writes to it. The resolution does, with an
explanation attached and a link back to the finding that caused it.

**Why this is the payoff of the whole design.** The append-only spine means a
finding can be investigated against the exact state at the moment it was
observed. `measurement` already does this for dimensions; `discrepancy` does it
for quantities. And D7 gives it somewhere to go — a discrepancy escalates to a
Nosdesk ticket with the originating movement, actor, device and timestamp
attached, so a warehouse manager can follow it up without stopping the floor.

**Non-blocking by default.** A discrepancy never halts operations. Negative
balances are allowed, picks against them succeed, and the finding is raised
asynchronously. That is what lets the warehouse proceed as intended while the
investigation happens separately.

### D9 — Discrepancies are caught at the point of capture, and the operator defines signal

**Decision.** Detect variance **at the moment of recording**, while the observer
is standing at the location and can look again. A fixed global tolerance is not
the mechanism; the operator decides what is signal.

**Why point-of-capture changes the value of the data.** The ledger already knows,
cheaply, whether a location has had any movement since it was last counted. So
when someone records a quantity for a location that **has not moved and has had
nothing picked from it**, and the number disagrees, that is knowable instantly —
and the person who can resolve it is right there.

Prompting then — *"this location has not moved since the last count of 50, and
you have entered 47. Are you sure?"* — produces one of two outcomes, and both are
better than silence:

- They look again and correct it. The bad data never enters the system.
- They confirm. **That is now a much stronger signal than an unexplained
  variance found in a batch reconciliation days later**, because a human was
  challenged with the contradiction, at the location, and stood by the number.

So confirmation-against-challenge is recorded as its own fact, not collapsed into
an ordinary count:

```
stock_count
  ...
  challenged            -- did we contradict them at capture time
  challenge_context     -- what we told them (expected qty, last movement at)
  confirmed             -- did they stand by it after being challenged
```

A confirmed-against-challenge variance should sort to the top of any
investigation queue. An unchallenged one is unremarkable by comparison. This is
the `measurement.confidence` idea applied to quantities: **how an observation was
obtained changes what it is worth.**

**Tolerance is the operator's call.** A one-unit variance on a 10,000-unit line
may be noise to one operator and a red flag to another, and only the data can
tell them which. So we do not hard-code thresholds — we give managers the means
to set, tune and change them, and we surface the distribution so those choices
are informed rather than guessed. The engineering requirement is to make signal
*attainable*, not to decide it.

**Corollary for fulfilment.** Discrepancies raised against the work actually
happening — a short pick, a location empty during a pick — are more directly
actionable than variances surfaced by an unrelated stocktake later. They should
be first-class in the same way, not a lesser kind.

### D10 — Movements reference their cause with typed FKs, not a polymorphic pair

**Decision.** Replace `reference_type`/`reference_id` on `stock_movement` with
nullable typed foreign keys plus a CHECK that exactly one is set:

```
stock_movement
  fulfilment_line_id      -- a pick
  goods_receipt_line_id   -- a receipt
  discrepancy_id          -- an adjustment from a finding (D8)
  move_task_id            -- an internal transfer
  CHECK (num_nonnulls(fulfilment_line_id, goods_receipt_line_id,
                      discrepancy_id, move_task_id) = 1)
```

**The reasoning, since this one is not obvious.**

A polymorphic pair is genuinely more convenient in one respect: adding a new cause
never needs a migration. Against that, four costs — and the last two are decisive
*for this model specifically*, rather than being general database advice.

1. **No referential integrity.** The database cannot check a polymorphic
   reference, so links can dangle. D8 makes investigation *follow* these links,
   which turns a dangling reference from untidy into a dead end during exactly
   the task the system exists to support.
2. **No query-planner statistics**, and a `WHERE reference_type = …` on every
   join. Minor, but it compounds on the hot path.
3. **It silently defeats principle 6.** The stated N+1 defence is Diesel's
   `belonging_to` + `grouped_by` batch loading. **That cannot be expressed over a
   polymorphic reference** — there is no association to declare, so loading
   movements for a set of fulfilment lines becomes either raw SQL or a loop. The
   convenient choice quietly removes the mechanism we said would prevent the
   problem you explicitly do not want.
4. **Partial indexes get awkward.** With typed columns,
   `CREATE INDEX … WHERE fulfilment_line_id IS NOT NULL` is natural and small.

**What it costs.** A migration when a genuinely new cause appears, and a wider
CHECK. The cause set is small and closed — pick, receipt, adjustment, transfer —
and grows roughly never. Four nullable `bigint`s is ~32 bytes a row.

**The same argument applies to `measurement`.** `subject_type`/`subject_id` is
the identical pattern over (item, package_type, package). For consistency it
should go the same way — see question 20, since there is a real counter-argument
that `measurement` is append-only reference data on a cold path, where the
batch-loading concern does not bite.

### D11 — Attribution separates the operator, the workers, and the accountable

**Decision.** Three distinct things, never collapsed into one `actor_id`:

```
stock_movement
  recorded_by_id     -- the authenticated session. NEVER null, NEVER editable.
  device_id
  work_session_id    -- nullable: the crew this work belonged to
  authorised_by_id   -- nullable: a supervisor standing behind an override

work_session          -- declared at sign-on, not inferred
  id, site_id, kind, started_at, ended_at

work_session_member   -- append-only; joins and leaves are timestamped
  work_session_id, person_id, role, joined_at, left_at
```

**The problem this solves.** In the simple case the person doing the work and the
person recording it are the same, and one field would do. But shared and
collaborative work is normal — someone managing counts across containers, a team
on a large task without a scanner each — and that is an accountability question
before it is an engineering one. The requirement is to let those situations be
**expressed declaratively**, without letting anyone's accountability be
misrepresented.

**How misrepresentation is prevented.** `recorded_by_id` comes from the
authenticated session, is never client-supplied, and is never editable. That is
the non-repudiable floor: whatever else is claimed, we always know which person,
on which device, recorded this. Everything else is a *claim made by that
operator* and is stored as such.

Crew membership is **declared at sign-on and append-only**, with `joined_at` and
`left_at`. So "who was on this team when that movement happened" is answerable
from history, and nobody can be retroactively added to a past window without it
being visible as a later row.

**Why a session rather than participants per movement.** At warehouse volumes a
participant row per movement is heavy and mostly repeated. A session is declared
once, is cheap to join, and expresses the real-world unit — a crew working
together for a shift or a task.

**`authorised_by_id` is deliberately separate.** A supervisor correcting a
casual's recorded work should be visible *as* a supervisor override, not by
overwriting who did the original work. The original stands (D8: the work event is
the invariant); the authorisation sits beside it.

### D12 — Allocation is an intention, and it is advisory

**Decision.** Add `stock_allocation` as an **intention** (principle 2), not a
lock. It expresses "we mean these units for this demand". It does **not** gate
picking, and it is allowed to be wrong.

**Why advisory, when every competitor enforces.** D5 traded coordination for
convergence, and allocation-as-enforcement is coordination. More importantly, an
enforcing allocation contradicts the founding observation: *the scanner is more
authoritative than the database*. If the plan says pick lot A from bin 12 and the
picker finds lot B, an enforcing system rejects the scan and stops the floor. Ours
accepts it — the pick is a fact — and raises a finding that the plan was wrong.

This is not a weaker allocation. It is allocation that cannot lie about physical
reality, and it makes every plan-versus-actual divergence *visible* rather than
suppressed at the point where the truth was available.

**How this resolves the principle 2 tension.** The analysis said allocation
cannot be a movement (nothing moved) and cannot live on `stock` without voiding
the rebuildable invariant. Both true — because allocation is not a fact at all.
It is a different category. `stock` therefore becomes a projection of **two
sources**, each column named for its own:

```
stock
  item_id, location_id, lot_id, status_id     -- the key (status per D4)
  quantity            -- projection of stock_movement   (facts)
  allocated_quantity  -- projection of stock_allocation  (intentions)
  available_quantity  -- generated: quantity - allocated_quantity
```

The invariant is **widened, not broken**: each column is rebuildable from its own
source, and the existing reconciliation job asserts both. Availability stays a
single indexed read rather than an aggregate on the hot path.

```
stock_allocation
  id
  fulfilment_line_id                          -- the demand
  item_id, location_id, lot_id, status_id     -- the supply cell
  quantity
  state          -- allocated | picking | fulfilled | short | released
  allocated_at, released_at
  allocated_by_id                             -- person, or null for the allocator
  move_task_id                                -- nullable, the work carrying it out
```

**Backorder is not an entity — it emerges.** Demand that could not be allocated
is `fulfilment_line.quantity - SUM(allocated)`, greater than zero. No backorder
table, no backorder status to keep in sync, and partial backorder falls out for
free. This is principle 1 doing its job.

**No soft/hard split.** Most WMS have order-level soft allocation and then
cell-level hard allocation. We allocate to a cell directly. Available-to-promise
still works — it sums `available_quantity`, which does not care how specific the
allocations were — and skipping the split removes a state, a transition, and a
class of stuck-in-between bugs. The usual argument for soft allocation is that
committing to a cell early is risky if stock moves; under D5 a moved cell simply
means the plan was wrong, which we already handle.

**FEFO lives here.** Rotation is an allocation decision, which is why it had
nowhere to go before. `item.rotation_type` (fifo | fefo | lifo | none) selects
which cells the allocator prefers. This is the real answer to open question 5.

**Short picks are already handled.** D8 did the work: the allocation goes to
`short`, a `discrepancy` of kind `short_pick` is raised with the picker, device
and location attached, and the unfulfilled quantity returns to the pool — where
it is either re-allocated or shows up as backorder by the same arithmetic. No
special machinery.

**Over-allocation is allowed**, consistently with D5. `available_quantity` may go
negative. That is a finding, not a rejected write.

### D13 — The model holds the inputs; the policy belongs to a manager

**Decision.** The allocator does **not** encode a rotation policy. The model holds
the facts that make *any* policy computable, the allocator is a scoring function
over those facts, and the weights and thresholds are **configuration a manager
owns**. We ship defaults, not hard-coded behaviour.

**Why this way round.** A flexible model with a policy applied on top can always
become strict. A strict model built around one policy cannot be made flexible
without surgery. Baking FEFO into the allocator would mean travel-awareness could
not be added later without reopening the core of the system — so the asymmetry
decides it, independently of which policy is currently right.

And the policy genuinely varies by product and by situation. Grabbing the next
best thing from another bay at ground level is often correct. The same
substitution is a different decision when the alternative needs a forklift to
bring stock down — which may mean waiting for equipment, a second person, or a
safety consideration. **The software should not make that trade on a manager's
behalf.** It should make the trade *visible* and let them set the rule.

This is D9's principle applied to planning rather than to findings: the
engineering job is to make the information attainable, not to decide what it
means.

**Access cost becomes first-class.** Distance alone does not capture the
difference the example describes, because the cost is a step change in *method*,
not a longer walk:

```
equipment_class
  id, name                       -- ground, ladder, order_picker, reach_truck, forklift
  relative_cost                  -- a ground pick is 1; a forklift retrieval is not
  requires_second_person
  is_shared_resource             -- can this become a queue

location
  ...
  reachable_by                   -- FK to equipment_class (already present)
```

Height is already derivable from `location.level` and `z_mm`, so the continuous
part of the cost is computable. `equipment_class` supplies the discontinuity.

**The policy surface, kept small.** Scalar configuration, scoped — the same shape
as `carrier_profile`, deliberately not a rules engine:

```
allocation_policy
  id, scope_kind                 -- site | item_class | item
  scope_id
  weight_rotation, weight_travel, weight_access
  rotation_tolerance_days        -- "oldest first, but treat within N days as equal"
  max_equipment_class_id         -- beyond this, prefer an alternative or ask
```

`rotation_tolerance_days` is the field that does most of the work: it turns strict
FEFO into "take the oldest, unless a much cheaper pick is within tolerance", which
is what most operations actually mean.

**Ranked candidates, not a single answer.** Scoring produces an ordered list, so
the allocator commits the top candidate but the `move_task` can carry alternates.
A picker who finds the bin empty gets the next option immediately instead of a
dead stop — and, per D8, the empty bin is still recorded as a finding. This
capability falls out of ranking rather than being built.

**What keeps this from becoming a rules engine.** The competitor analysis warned
that declining an engine while accepting five small rule tables is how you get an
engine you never designed. The discipline here: **the scoring function is code —
one implementation, testable, versioned.** Only its weights are data. When
putaway, replenishment and disposition need the same treatment, they get the same
shape: code that scores, configuration that weights. If we ever find ourselves
adding a table where *the logic itself* is rows, that is the line, and we should
notice we are crossing it.

### D14 — Lot, expiry and rotation

**Decision.** Add `lot` as a real entity, carry `lot_id` to `package_content`,
keep it *off* `fulfilment_line`, and store shelf-life **facts** on the lot while
shelf-life **requirements** live with the customer.

```
lot
  id
  item_id                   -- a lot is always of exactly one item
  code                      -- ours
  supplier_lot_ref          -- theirs, often different; both are needed for a recall
  supplier_id               -- nullable until inbound exists
  manufactured_at
  received_at
  expiry_date               -- the hard date
  best_before_date          -- nullable; differs from expiry for a lot of food

item
  ...
  tracking                  -- none | lot | serial
  shelf_life_days           -- expected, for validating a received expiry date
  rotation_type             -- fifo | fefo | lifo | none
```

**Two dates, not Odoo's four — because the fourth is not a lot property.**
Odoo carries `expiration_date`, `use_date`, `removal_date` and `alert_date`. The
analysis is right that collapsing them naively makes *"don't ship with under 90
days remaining"* unimplementable. But storing `removal_date` on the lot is the
wrong fix, because **minimum remaining shelf life is a customer requirement, not
a fact about the goods.** Grocery chains commonly demand a fixed proportion of
shelf life remaining on delivery, and different customers demand different
amounts for the same lot.

So the requirement goes where it belongs:

```
customer
  ...
  min_shelf_life_days       -- or
  min_shelf_life_pct        -- proportion of total shelf life that must remain
```

and shippability becomes a computation — `expiry_date - ship_date` against the
customer's rule — rather than a date frozen at receipt against one customer's
assumption. This is strictly more capable than Odoo's single `removal_date`, and
it is the same shape as D13: **the model holds facts, the policy sits beside it.**

`alert_date` is derivable from `expiry_date` and a lead time, so it is not
stored. A lot pulled early for a quality reason is not a date problem at all —
that is `inventory_status` (D4).

**`lot_id` goes on `package_content`. It does not go on `fulfilment_line`.**
Here I disagree with the competitor analysis, which recommended both.

`fulfilment_line` is **demand** — "ship 100 of item X". The lot is chosen later,
at allocation and pick time, and a line may legitimately ship from several lots.
Putting a lot on the demand row either over-specifies the order or forces a
second fulfilment line per lot, conflating what was asked for with what was sent.
`stock_allocation` already carries `lot_id` (D12) and so does `stock_movement`,
which is where supply decisions belong.

`package_content` is different: it is a **fact about a physical carton**. Without
`lot_id` there, a package holding two lots of the same item is unrepresentable,
and the recall question dead-ends at exactly the table built to answer "what is
in this box".

Both recall directions then work:

- *Which customers received lot L?* `stock_movement WHERE lot_id = L AND
  fulfilment_line_id IS NOT NULL` → line → fulfilment → order → customer. This
  query only exists because D10 made the cause a typed FK.
- *Which cartons on which pallets hold lot L?* `package_content.lot_id`, walking
  `parent_package_id` (D6).

**Rotation slots into D13 with no new machinery.** `item.rotation_type` selects
which date the scoring function uses as its rotation key — `expiry_date` for
FEFO, `received_at` for FIFO — and `allocation_policy` supplies the weight and
`rotation_tolerance_days`. Nothing about rotation is hard-coded; FEFO is a
configuration of a general allocator, not a mode.

**A recall writes movements; it is not a flag.** Holding a lot means writing
status-change movements (`from_status = available`, `to_status = hold`, per D4)
across its cells, referencing a `lot_hold` record that carries the reason and the
decision-maker. It is a bulk write, and recalls are rare enough that this is
fine. The alternative — a `lot.on_hold` boolean overriding cell status — creates
a second answer to "is this available", which is how availability logic starts
disagreeing with itself.

The retroactive part falls out: stock of a held lot that is already allocated
produces allocations against unavailable stock, which is a **finding** under D8
rather than a special case anyone has to code.

**Holds are intentions; the movements are the facts that carry them out.** This
is principle 2's three categories doing real work, and it buys precision that a
flag cannot.

```
lot_hold                    -- an intention (principle 2)
  id, lot_id
  reason, reference          -- recall notice number, test result, customer complaint
  scope_note
  raised_at, raised_by_id
  lifted_at, lifted_by_id, lift_reason
```

Every status movement written by a hold references its `lot_hold`. Three things
follow, and the third is the one that matters:

**Release is precisely scoped.** Lifting hold H returns only what H held. A flag,
or a naive "release everything held for this lot", would also release stock held
for an *unrelated* reason — damage found separately, a quality quarantine, a
customer complaint on the same lot. Because the movements name their cause, the
reversal cannot overreach.

**Overlapping holds compose correctly.** Two active holds on one lot mean stock
returns to `available` only when the last is lifted. That is a count of unlifted
`lot_hold` rows, not a boolean anyone has to keep consistent.

**Stock that moves while held stays held.** A transfer of held stock is a
movement with the status unchanged on both sides, so the hold follows the goods
to their new location without anything tracking it. Release then acts on wherever
the stock actually is now — which a snapshot of affected cells, taken at hold
time, would have got wrong.

**Amendment keeps the mistake visible.** If a hold was scoped too widely, the
over-held stock is released with its own reason, and the record still shows what
was held, by whom, and why it changed. Nothing is rewritten, so the correction is
as auditable as the original decision — which is D8's invariant applied to
directives rather than to work.

**Serial stays split, per the analysis.** `tracking = serial` reserves the enum
value, but **unit-level serialised inventory remains out of scope** — a serial as
a stock-bearing entity with its own custody chain would reshape `stock` the way
containers do. Pack-time serial capture is a different, much cheaper thing: one
table hanging off `package_content`, with no reach into stock, allocation or
routing. That version is worth having when a customer asks for it.

**Indexes.** `stock_movement(lot_id, occurred_at)` — the primary trace query, and
missing from the original list. `lot(item_id, expiry_date)` for FEFO candidate
selection. `package_content(lot_id)` for recall-to-carton.

### D15 — Three groupings, kept separate; `consignment.fulfilment_id` is dropped

**Decision.** `fulfilment.order_id` stays singular. `consignment.fulfilment_id`
is **removed** — the relationship already exists, through packages. No new join
tables.

**The mistake worth naming.** The competitor analysis said waves push
`fulfilment.order_id` toward many-to-many and consolidated freight pushes
`consignment.fulfilment_id` the same way, so both should become join tables. That
reasoning conflates **three independent groupings** that happen to overlap in
other systems:

| Grouping | Question it answers | Where it belongs |
|---|---|---|
| **Demand** | What did a customer ask for? | `order` |
| **Work** | How do we organise the picking? | `pick_batch` / `move_task` (intentions) |
| **Freight** | How does it travel? | `consignment` |

Every WMS with a bloated shipment model got there by making one table serve two
of these. Keep them apart and each stays simple — principle 1.

**Waves do not touch `fulfilment.order_id`.** A wave is a grouping of *work*, not
of demand. It belongs in the intentions layer, where `pick_batch` spans
fulfilments freely and nothing about the order structure has to bend. The
pressure the analysis detected is real; it just lands on a different table.

So a fulfilment stays "one order's commitment to ship from one site". One order
can already have many fulfilments — partial shipment, multi-site, backorder
release — because the FK is many-to-one. That was never the constraint.

**Consolidated freight needs no change either, because packages already carry
it.** The path exists today:

```
consignment → consignment_package → package → fulfilment → order
```

A consignment carries **physical things**, not abstract fulfilments. Two orders
consolidating onto one truck is two fulfilments putting their packages on one
consignment — which is exactly what physically happens. `consignment.fulfilment_id`
was a second, weaker representation of a relationship the package path already
expressed correctly, and it silently forbade the consolidation it looked like it
was modelling.

Dropping it also removes a consistency hazard: with both present, nothing stopped
`consignment.fulfilment_id` disagreeing with the fulfilments reachable through
its packages.

**Both directions stay cheap**, with `package(fulfilment_id)` and
`consignment_package(package_id)` indexed:

- *Which fulfilments are on this consignment?* → two indexed joins.
- *Which consignment carries this fulfilment?* → the same path, reversed.

**What this buys, concretely.** Consolidating two orders for one customer onto a
single pallet run — normal practice on the Swift and Direct pallet freight that
is our highest volume — is now expressible. Under the old shape it was not, and
the walkthrough's one-fulfilment-one-consignment flow would have hardened into a
constraint rather than being simply what happens most of the time.

**One loose end.** D6 made `package.fulfilment_id` nullable so a package can be a
reusable tote or an LPN in racking. A package on a consignment should have a
fulfilment — except possibly for an inter-site transfer, which is a shipment that
fulfils no customer order. Worth deciding whether a transfer is a kind of
fulfilment or its own thing (question 37).

### D16 — A transfer is its own demand, sharing the fulfilment machinery

**Decision.** Inter-site transfers get their own demand entity. `fulfilment`
references demand through **typed FKs** (D10), not through a widened `order`.

```
transfer_order
  id, from_site_id, to_site_id
  requested_at, required_by, status

transfer_order_line
  id, transfer_order_id, item_id, quantity

fulfilment
  order_id             -- customer demand
  transfer_order_id    -- internal demand
  CHECK (num_nonnulls(order_id, transfer_order_id) = 1)
```

`fulfilment_line` gains the same treatment against `order_line` /
`transfer_order_line`.

**Why not just add a `kind` and a nullable customer to `order`.** That is the
generic document model already on the deliberately-not-building list. It would
mean `customer_id` nullable on every customer order, shelf-life rules (D14)
reaching for a customer that may not exist, and every query carrying a `WHERE
kind = …` that the database cannot help with. Two honest tables beat one
apologetic one.

**Why this is the same insight as D15.** A transfer differs from a customer order
on the **demand** side only — no customer, a destination site, no revenue. The
**work** and **freight** sides are identical: it is allocated, picked, packed,
consigned, labelled and tracked exactly like anything else. So the demand entity
forks and everything downstream is shared. Splitting `fulfilment` too would
duplicate the entire outbound machinery for no gain.

**D15's loose end dissolves.** A transfer's packages *do* have a fulfilment — one
pointing at a `transfer_order` instead of an `order`. So the rule holds without
exception: **every package on a consignment has a fulfilment.** No nullable
special case, and D6's nullable `package.fulfilment_id` goes back to meaning only
what it was introduced for — totes and LPNs that are not shipping anywhere.

**The inbound side is symmetric.** A transfer arriving at the destination is a
goods receipt, so `goods_receipt` takes the same typed pair:

```
goods_receipt
  purchase_order_id     -- from a supplier
  transfer_order_id     -- from another site
  CHECK (num_nonnulls(purchase_order_id, transfer_order_id) = 1)
```

One entity, two demand sources, on both ends. A transfer is simply the case where
our own outbound feeds our own inbound.

**Stock in transit is derivable — no virtual location needed.** Between despatch
from A and receipt at B, the goods are in neither site's stock: the despatch
movement has `to_location_id = NULL` and the receipt has `from_location_id =
NULL`, per the existing convention. That makes in-transit stock invisible in
`stock` — but it is not unanswerable:

> transfer orders despatched and not yet fully received → their fulfilment lines
> minus their receipt lines.

The intention (the transfer order) plus the facts at each end give the answer
without inventing a location to park it in. This is the concrete reason the
analysis's admiration for Odoo's virtual-location model does not translate into
a reason to adopt it: we get the same answer from principle 2's categories
instead of from a NULL-elimination trick that would add a `usage` predicate to
every on-hand query.

### D17 — `work_task`: one table for directed work, and a second fact table for work that moves nothing

**Decision.** One `work_task` table across every kind of directed work, plus
`activity_event` as a second append-only fact table. Tasks are **intentions**
(principle 2); the movements and counts they produce are the facts.

```
work_task
  id, site_id
  purpose            -- pick | putaway | replenish | transfer | count | inspect
  pick_batch_id      -- nullable; the work grouping (D15)
  item_id            -- nullable: a count task is told a location, not an item
  from_location_id
  to_location_id     -- nullable: a count moves nothing
  planned_quantity   -- nullable: a count's quantity is the question, not the input
  sequence           -- travel order within the batch
  state              -- pending | claimed | started | completed | failed | cancelled
  claimed_by_id, claimed_at
  started_at, completed_at
  work_session_id    -- D11
```

**Why one table, when D15 warned about tables serving two masters.** The test is
whether the concepts are genuinely one thing. Pick, putaway, replenish, transfer,
count and inspect are all *directed work assigned to a person at a location, with
a state*. That is one concept. What differs is the **fact they produce** — a
movement, or a count — and those already live in separate tables. Manhattan
reaches the same shape with a single unified task table, and the analysis was
right to call it the principle-1-consistent answer.

**Named `work_task`, not `move_task`, because a count moves nothing.** Calling it
`move_task` and then filing counts and inspections in it is exactly the small
dishonesty that accretes into a schema nobody can read. It also pairs with
`work_session` (D11). Two nullable columns — `to_location_id` and
`planned_quantity` — are the honest cost of covering non-moving work, and they
are nullable *for a stated reason* rather than by drift.

**Claiming is advisory, but coordination here is cheap.** D5 traded coordination
away where it would block physical work. A task claim is the opposite case: it is
low-stakes, and losing a race costs a moment rather than a pick. So the server
takes a first-claim-wins lock and tells the loser immediately over the existing
real-time channel.

**The distinction worth stating: we avoid coordination where it would stop the
floor, not everywhere.** Blocking a scan is unacceptable because the physical
event already happened. Blocking a claim is fine because nothing has happened
yet. If two people do the same task anyway — offline, or by ignoring the
warning — both sets of movements are still facts, and the duplication is a
finding under D8. The claim is a courtesy, not a guarantee.

**Cluster picking needs no extra table.** One visit to a cell can serve several
orders. `stock_allocation.work_task_id` (D12) links N allocations to one task, and
each allocation already knows its `fulfilment_line` and therefore its destination
receptacle. The task's `planned_quantity` is just the sum of its allocations. Pick
once, distribute by allocation.

```
pick_batch                    -- the work grouping (D15)
  id, site_id
  kind                        -- wave | cluster | zone | single
  state, created_at, released_at, completed_at, created_by_id

receptacle_assignment         -- trolley slots and put-wall cells, one mechanism
  id, pick_batch_id, fulfilment_id
  package_id                  -- the tote, which is a container (D6)
  position
  opened_at, released_at
```

A partial unique index on `(pick_batch_id, position) WHERE released_at IS NULL`
gives first-empty-wins allocation of slots and route-back-to-the-same-slot in one
index. The tote being a `package` is D6 paying off — no parallel container
concept was needed.

**`activity_event`: the denominator.** Work that moves no stock currently has
nowhere to live — a failed scan, a skip, a location found empty, a search that
turned up nothing, time between tasks. Without it, productivity has counts but no
denominators, and exception patterns are invisible.

```
activity_event                -- a fact (principle 2), append-only
  id, occurred_at, recorded_at
  client_event_id (unique), device_id      -- shares D5's idempotency columns
  recorded_by_id, work_session_id
  work_task_id, location_id                -- both nullable
  kind          -- scan_ok | scan_mismatch | location_empty | skip
                -- | search_failed | task_paused | idle
  detail
```

Kept separate from `stock_movement` deliberately: mixing them would put a
`WHERE` clause on the sum that defines stock, and D5 exists to keep that sum
unconditional. It is also the natural home for the handheld's event stream, so
the same idempotency machinery serves both.

**A `location_empty` event is worth more than its size suggests** — it is D9's
point-of-capture principle applied to picking. Somebody stood at a bin the system
believed had stock and found none, which is a strong signal recorded at the
moment it was cheapest to catch.

**Not every movement has a task.** Ad-hoc work is real — a pallet moved because
it was in the way. `work_task_id` is nullable on `stock_movement`.

### Correction to D10

D10 listed `move_task_id` as one of four **mutually exclusive** causes on
`stock_movement`, with a CHECK that exactly one is set. That is wrong: a pick
movement has *both* a `fulfilment_line_id` (why the stock moved) and a task (how
the work was organised). They are orthogonal dimensions, not alternatives.

Corrected:

```
stock_movement
  fulfilment_line_id      -- cause
  goods_receipt_line_id   -- cause
  discrepancy_id          -- cause
  CHECK (num_nonnulls(fulfilment_line_id, goods_receipt_line_id,
                      discrepancy_id) <= 1)     -- AT MOST one, not exactly one

  work_task_id            -- orthogonal: nullable, always permitted
```

**At most one**, because an internal move — a replenishment, or an ad-hoc
relocation — has no demand-side cause at all. The existing `reason` enum already
distinguishes what kind of movement it is; the cause FK says which document
demanded it, when one did.

### D18 — Multi-tenant and multi-site are different axes, and we build both

**Decision.** `tenant` owns `site`. Multi-tenancy is a stated, non-negotiable
requirement; multi-site already existed. They are orthogonal and cheap together,
expensive apart.

```
tenant
  id, name, slug, active

site
  id, tenant_id, name, timezone      -- tenant_id is new
```

`tenant_id` is denormalised onto every major table alongside the `site_id` the
competitor analysis already recommended, and isolation is enforced by Postgres
row-level security rather than by remembering a `WHERE` clause.

**Why the distinction is worth stating.** Multiple warehouses in multiple
Australian states is **multi-site** — one organisation, many locations, and the
model already handled it. **Multi-tenant** is a different boundary: separate
organisations whose data must never meet.

They behave in opposite ways on exactly the thing we just designed:

| | Across sites | Across tenants |
|---|---|---|
| Stock transfers (D16) | Normal | Must be impossible |
| Reporting | Rolls up | Never joins |
| Users | May span, with permissions | Never span |
| Queries | A feature | A leak |

**D16 is the proof.** A `transfer_order` moves stock between sites. You do not
transfer stock between tenants — that is a sale, or a 3PL movement, not a
transfer. So the Australian states are sites of one tenant, and building hard
isolation around them would break the inter-state transfers just designed.

**Why build tenancy now anyway.** It is the most expensive thing on the list to
retrofit — every table, query, index and cache key — and Nosdesk's shape (BUSL
licence, licence keypair, licensing module, hosted deployment targets) says this
codebase is heading somewhere commercial. A `tenant_id` column added now costs
about what the `site_id` denormalisation already costs. Added later it is a
migration touching everything.

**Deployment stays open.** Row-level security supports both shared-database and
database-per-tenant, and a BUSL product that self-hosts usually wants the latter
available. Deciding the schema now does not commit the deployment model.

**Explicitly deferred: `stock.owner_id`.** Holding *another* organisation's
stock at our site — 3PL — is a third axis again, and it would put owner in the
`stock` key alongside status (D4), making it the fifth key column. We ship our
own goods, so this is not needed. Recorded rather than assumed, because if 3PL
ever becomes a product direction this is the migration nobody wants: see
question 48.

### D19 — People span tenants; reference data can be shared, observations cannot

**Decision.** `person` has no `tenant_id` — membership is a join table. Reference
data takes a **nullable** `tenant_id` where NULL means shared. Observed and
operational data is **always** tenant-scoped, even when it describes a shared
item.

```
person                      -- global identity
  id, name, email, active

person_tenant               -- membership; append-only, timestamped
  person_id, tenant_id, role, joined_at, left_at

person_site                 -- site access within a tenant
  person_id, site_id, joined_at, left_at
```

**The split that is not obvious.** "Companies with multiple tenants share similar
product" is true, but it does not follow that an *item row* can simply be shared.
An item carries two very different kinds of information:

| | Example | Scope |
|---|---|---|
| **Intrinsic** — what the thing *is* | code, description, GTIN, DG class, UN number, packing group | Shareable |
| **Operational** — how *we* handle it | measurements, packing config, internal barcodes, rotation type, shelf life | Never shareable |

Two tenants stocking the same product may receive it from different suppliers, in
different case packs, on different pallet configurations. **Their measurements
legitimately differ, and neither is wrong.** So D14's `measurement` and
`item_packing_config` stay tenant-scoped even when the `item` they describe is
shared — otherwise one tenant's cubing corrections silently rewrite another
tenant's autofill, which is the D9 confidence model quietly poisoned across an
isolation boundary.

The pleasant consequence: a shared item is **thin** — identity and intrinsic
facts only. That is exactly the part that genuinely is the same everywhere, and
it is also the part that is expensive to key in and easy to get wrong (UN numbers
in particular). The valuable sharing happens without any of the risky sharing.

```
item
  tenant_id            -- NULL = shared catalogue
  code, description
  dangerous_goods_class, un_number, packing_group
  tracking

item_barcode
  tenant_id            -- NULL for a GTIN; set for an internal barcode

measurement            -- tenant_id NOT NULL, always
item_packing_config    -- tenant_id NOT NULL, always
```

**RLS follows the same shape.** Reference tables get
`tenant_id IS NULL OR tenant_id = current_tenant()`; everything else gets
`tenant_id = current_tenant()`. Two policy shapes, applied by category, not
per-table judgement.

**Accountability still works across the boundary.** `recorded_by_id` (D11) points
at a global `person`, but the facts it appears on are tenant-scoped. So someone
working in two tenants has one identity and two separate histories, and a manager
in one cannot see their activity in the other. That falls out of RLS rather than
needing a rule.

**Membership is append-only**, matching `work_session_member` (D11): who had
access to what, when, is answerable historically. Access that was removed leaves
evidence.

### D20 — Capability is a property of the data, not a configuration mode

**The principle.** Support as many operating models as possible without imposing
any of them. An organisation with a streamlined process should never encounter
the machinery that serves a complicated one — not because it is switched off in
settings, but because **the dimension only exists on the records that need it.**

This is the difference between "enable lot tracking" as a global mode that
changes how the whole system behaves, and `item.tracking = lot` on the forty
items that need it. The second is discoverable, per-item, reversible, and
invisible to everyone else. It is also already how D14 works, so this decision
generalises an existing pattern rather than inventing one.

Four capabilities, one shape:

| Capability | Carried by | Default | Cost to an org that does not need it |
|---|---|---|---|
| Lot / expiry / FEFO | `item.tracking`, `item.rotation_type` | `none` | Nothing |
| Catch weight | `item.quantity_mode` | `count` | Nothing |
| Third-party stock (3PL) | `stock.owner_id` | the site's own entity | Nothing |
| Multiple legal entities | `site.legal_entity_id` | the tenant's own entity | Nothing |

Every one defaults to the simple case. Nothing needs configuring to get the
streamlined behaviour, and nothing needs "going deep into context options" to get
the complicated one — you set a property on the product or the site.

### `party` and `legal_entity` — the third axis

3PL is confirmed as in scope, which requires an owner concept for stock. Once
that exists, the corporate-group case comes free, so these are one decision:

```
party                        -- anyone we transact with or on behalf of
  id, tenant_id, kind        -- legal_entity | customer | supplier | carrier
  name, abn, active

site
  legal_entity_id            -- which of our entities operates this site

stock
  owner_id                   -- party; defaults to the site's legal entity
```

**Three axes, not one.** This is the distinction D18 started and did not finish:

| Axis | Question it answers | Crossing it means |
|---|---|---|
| `tenant` | Who may see this? | Impossible |
| `legal_entity` | Who owns it and who invoices? | An inter-company sale |
| `site` | Where is it? | A transfer (D16) |

**This de-risks question 49.** Whether the Australian states are one legal entity
or several changes the *deployment* — one tenant or several — but **not the
schema**, because `legal_entity` expresses a group either way. If they are
several entities under one operational group, the right answer is almost
certainly one tenant with several legal entities: operationally one warehouse
network, legally several companies. That keeps D16's transfers working while
letting them generate the inter-company paperwork they legally require.

**`owner_id` joins the `stock` key**, making it (item, location, lot, status,
owner). Six columns is wide, and it is the same shape Odoo reached. For an
organisation that never holds third-party stock the column is a constant, so the
index behaves as though it were not there. This is the migration D4 warned about,
which is precisely why it is being done now rather than discovered later.

### Catch weight, contained

A catch-weight item is sold by actual weight but handled as discrete units — six
cartons of beef weighing 47.3 kg in total. The trap is letting that turn
`quantity` into two numbers everywhere.

It does not have to. **The count stays primary**; the weight rides alongside as
an observation captured at the same moment:

```
item.quantity_mode          -- count | catch_weight
stock_movement.catch_weight_g   -- nullable; required when quantity_mode = catch_weight
package_content.catch_weight_g
```

Picking, allocation, cubing and the ledger all keep working on counts unchanged.
Weight is captured at pick and pack time, flows to `package_content`, and drives
invoicing and freight. Enforcement follows the same pattern as `tracking = lot`
(question 31): application-level, plus a periodic assertion.

### Ordering by weight is unit conversion, not a different allocator

*(Revised — I first called this "a genuinely different allocation problem". It is
not, and the simpler reading is better.)*

Weight is a **property of the stock**, and a kilogram is a unit that converts
through the weight per unit we already hold. So ordering in kg is the same
mechanism as ordering in pallets: `entered_quantity` + `entered_unit`, converted
to the base unit for everything downstream.

```
order_line
  entered_quantity, entered_unit     -- 240, 'kg'
  quantity                           -- 12, base units (derived)
  quantity_tolerance_pct             -- how close is close enough
```

Two cases, one mechanism:

- **Fixed weight** — a box of screws is always 12 kg. `240 kg → 20 boxes`
  exactly. Pure unit conversion; catch-weight machinery never engages.
- **True catch weight** — a box of beef is *about* 20 kg. `240 kg → ~12 boxes` is
  a **planning estimate**, which is all allocation ever needed. The real numbers
  arrive when the boxes are actually picked and weighed.

**Allocation stays on counts, unchanged.** It plans against nominal weight because
at planning time nominal weight is the only weight that exists — the actual boxes
have not been chosen yet. Trying to solve closest-fit in the allocator means
optimising against numbers we have not observed, which is exactly the mistake D5
exists to avoid.

**Closest-fit belongs at pick time, where the scale is.** The picker has real
weights, so the handheld can guide: *"241.3 kg picked, target 240 ± 2%, within
tolerance."* Under or over tolerance is a decision made with actual numbers in
hand, and if it ships outside tolerance that is a finding (D8), not a rejected
pick.

**Stock in kilograms becomes a projection, not a calculation.** Because actual
weights land on movements, weight-on-hand sums the same way count does:

```
stock
  quantity        -- projection of stock_movement.quantity
  weight_g        -- projection of stock_movement.catch_weight_g
```

So *"how many kilograms of beef do we have"* is a single indexed read, exactly
like the count — and it is **actual** weight rather than an estimate. For
non-catch-weight items the column is null and nominal weight is derived from
`measurement` on demand.

**What still needs differentiating** is what the customer *asked for*, which
`entered_unit` records. An order for 240 kg and an order for 12 boxes are
satisfied differently even when they pick the same stock: one is judged against a
weight tolerance, the other against a count.

### D21 — Assertions: statements of record neither side may revise

*Adopted 2026-08-01 from [mechanism-design.md](./mechanism-design.md), with the
provenance closure test replacing the closure claim, rule 3 stated as its
negative half only, and principle 3 restated to `bytea`. D26 remains proposed.*

**Decision.** An **assertion** is a statement of record exchanged with another
party, stored exactly as exchanged, which neither side may unilaterally revise.

**The cut is control, not authorship.** The property that generates every rule is
not *who wrote it* — it is that **a copy exists outside our control**. Our own
outbound despatch advice is as unrevisable as a supplier's inbound one, because
they hold a copy and will quote it back. Taking the symmetric version costs one
`direction` column and buys outbound EDI, proof of delivery and quotations on
machinery we build once.

**Why "intentions have an author" fails.** Three decisive reasons: mutability is
the intention category's *defining* rule and must be disabled for every
counterparty claim; assertions arrive in the author's vocabulary and are normally
unresolvable, where an intention with dangling FKs is a defect; and intentions
project into `stock.allocated_quantity` while an ASN must not.

**The five rules.**

1. **Immutable.** No UPDATE, no DELETE, ever. A revision is a new assertion.
2. **Always names its author party.** `author_party_id NOT NULL` — an
   access-control boundary, not metadata.
3. **Never projects into `stock`, and never into a commitment that survives
   withdrawal of the claim.** *(Narrowed by D24 (supply side) — assertions project
   into `expected_supply`, and demand may bind to it. This is a policy change, not
   a schema one: a counterparty's claim can now reach a customer promise. See
   D24's rule-3 section.)*
4. **Exists to be compared.** A claim never checked is itself a finding.
5. **Recorded in the author's vocabulary.** Resolution into ours is a separate,
   fallible, recorded step.

```
party_message                 -- FACT. Replaces provider_exchange.
  id, tenant_id, party_id
  direction                   -- inbound | outbound
  channel                     -- edi | portal | csv | email | api | webhook | print
  transport_ref, content_type
  payload bytea               -- verbatim. NEVER jsonb (principle 3).
  byte_count, content_hash
  occurred_at, recorded_at, client_event_id
  parse_status                -- pending | parsed | partial | failed | unsupported
  parser_version
  UNIQUE (tenant_id, party_id, content_hash, transport_ref)

assertion                     -- ASSERTION. Envelope.
  id, tenant_id, kind         -- despatch_advice | carrier_status | equipment_docket
                              -- | delivery_receipt | order_response | price_advice
  direction, author_party_id (NOT NULL), transmitted_by_party_id
  owner_party_id, site_id
  author_reference, author_version, message_function
  asserted_at                 -- their clock
  received_at                 -- ours. NEVER null.
  party_message_id            -- the artefact, when one exists
  captured_by_id              -- the person, when keyed from paper
  client_event_id
  supersedes_assertion_id     -- THEIR claim that this replaces that
  correction_of_assertion_id  -- OUR transcription fix
  CHECK (party_message_id IS NOT NULL OR captured_by_id IS NOT NULL)
  CHECK (correction_of_assertion_id IS NULL OR party_message_id IS NULL)
  UNIQUE (id, kind)                              -- composite FK target for bodies
  UNIQUE (id, tenant_id, author_party_id, kind)  -- target for supersession
  -- NO unique on (author_reference, author_version): a duplicate resend must be
  --   STORABLE and raise a finding (D5), not be refused at the write.
  -- NO status column: our position is assertion_stance (D25).

assertion_stance              -- FACT: our position on a claim
  id, tenant_id, assertion_id
  stance                      -- pending | in_force | rejected | superseded
                              -- | withdrawn_by_author | expired
  reason_code, note, successor_assertion_id
  CHECK (stance <> 'superseded' OR successor_assertion_id IS NOT NULL)
  occurred_at, recorded_at, client_event_id
  recorded_by_id / automation_key, authorised_by_id

assertion_check               -- FACT: a claim was checked against reality
  id, tenant_id, assertion_id
  asserted_unit_id, asserted_unit_content_id
  metric_id                   -- FK metric (D23). NOT a second vocabulary.
  outcome                     -- agreed | disagreed | unverifiable
                              -- | unchecked_at_close
  asserted_numeric, observed_numeric        -- canonical units (D23)
  asserted_text, observed_text
  variance_numeric GENERATED
  discrepancy_id
  CHECK (outcome <> 'disagreed' OR discrepancy_id IS NOT NULL)
  checked_at, recorded_at, client_event_id, recorded_by_id / automation_key
```

**Typed bodies, one per kind**, joined by a composite FK on `(assertion_id, kind)`
with the body's `kind` a stored generated constant — so "this body belongs to an
assertion of the matching kind" is declarative rather than a trigger.

```
despatch_advice               -- body for kind = 'despatch_advice'
  assertion_id PK, kind (GENERATED, + composite FK)
  inbound_shipment_id         -- the SUBJECT this claim is about (our resolution)
  ship_from_gln, ship_to_gln, gsin, ginc
  carrier_party_id, conveyance_ref, container_ref, seal_number
  despatched_at, estimated_arrival_at
  split_shipment, completes_order, granularity
  resolved_purchase_order_id, resolved_at, resolved_by_id, resolution_method

asserted_unit                 -- the declared logistic hierarchy, per claim
  id, assertion_id, parent_asserted_unit_id
  level_code, sscc, sequence
  raw_package_type_code, resolved_package_type_id
  -- NO weights, NO ti/hi. Those are OBSERVATIONS whose observable is this
  --   asserted_unit and whose asserted_by is the author (D23).
  -- Nesting is unbounded here (cold path); it collapses to D24's cap at receipt.

asserted_unit_content
  id, asserted_unit_id
  raw_gtin, raw_item_code, resolved_item_id
  raw_po_reference, raw_po_line_number, resolved_purchase_order_line_id
  quantity, entered_quantity, entered_unit_id   -- structural: the receipt
  lot_code, expiry_date, best_before_date       --   compares these line by line
  resolved_at, resolved_by_id, resolution_method
```

**The boundary with observations.** An assertion body holds **identifiers,
structure, and the values the receipt compares line by line**. Every other number
with a unit is an `observation` (D23) whose observable is the asserted unit. A
supplier-declared carton weight is therefore an observation with
`asserted_by_party_id = supplier`, and comparing it to our scale is one query
against one vocabulary rather than two parallel ones.

**Two column classes, and immutability follows the class.** `raw_*` and every
transcribed value are immutable. `resolved_*` are *our annotation* and may be
written when resolution later succeeds — a GTIN unresolvable today becomes
resolvable when the item is created tomorrow, and refusing that would discard a
claim because our catalogue was behind. But a re-resolution **freezes on first
use**: once an `assertion_check` or a `goods_receipt_line` references it, it may
not be rewritten, and a correction writes a new assertion. Same rule as
`goods_receipt_line.expected_quantity`.

**`inbound_shipment` is a subject, not an assertion.** Filing it as an assertion
means a resend mints a second row and orphans every FK pointing at the first —
the `consignment.fulfilment_id` defect D15 already deleted once.

```
inbound_shipment              -- PROJECTION (subject)
  id, tenant_id, site_id, supplier_party_id, owner_party_id
  vendor_shipment_ref
  in_force_assertion_id       -- @projection: the currently effective claim
  granularity, estimated_arrival_at
  asserted_unit_count, asserted_base_quantity      -- for the gate check
  first_asserted_at, superseded_count
  vehicle_arrival_id
  UNIQUE (tenant_id, supplier_party_id, vendor_shipment_ref)
```

**Nothing on it is NOT NULL that requires an assertion**, so blind receipt — rung
zero of the degradation ladder — is a **schema property**, not a workflow branch.

#### Amendments to earlier decisions

- **Principle 2** — the fourth provenance value, with the admission test.
- **Principle 3** — restated to `bytea`; the model contains no `jsonb` column.
- **D1** — `provider_exchange` becomes `party_message`; `consignment.eta`,
  `.status` and `.price_minor` become projections of the in-force carrier advice.
- **D5** — terminology: "counts are assertions, not deltas" becomes "counts are
  **absolute claims** — register semantics". `stock_count` is a fact, and ours.
  "Assertion" now means a statement of record exchanged with a party.
- **D8** — `discrepancy` gains `assertion_check_id`; kinds `+= expiry_mismatch`,
  `identity_mismatch`, `assertion_unresolvable`, `asserted_unit_absent`,
  `asserted_unit_unexpected`.
- **D11** — machine actors: `automation_key` XOR `recorded_by_id`, on
  assertion-ingestion facts **only**. `stock_movement` keeps its NOT NULL person.
- **D14** — `lot.expiry_date` remains the *accepted operational value*; a
  supplier-asserted expiry is an assertion, and disagreement is `expiry_mismatch`.

**Rejects.** "A counterparty's intention is still an intention". One table per
assertion kind with author, artefact and clocks repeated. A single assertion table
with a JSONB payload. An EAV bag of asserted attributes. Correcting an assertion
in place where an artefact exists. Making the category asymmetric — inbound only.
Treating adopted rate cards and customer shelf-life requirements as assertions:
those are **policy**, carrying `adopted_from_assertion_id`, because we may change
them unilaterally.

### D22 — Policy resolves against a scope lattice

*Adopted 2026-08-01 from [mechanism-design.md](./mechanism-design.md), with
taxonomy changes as facts added. D21, D23, D25 and D26 remain proposed.*

**Decision.** One `policy_binding` table. A policy is a **typed value row bound to
a point in a lattice of six ordered, tree-shaped scope dimensions, effective over
a period**. Resolution matches every binding whose non-null dimensions are
at-or-above the request's node on each axis, orders the matches by their **depth
vector** compared lexicographically in a precedence order declared per kind in
code, takes the winner's value row, then clamps any field the value type declares
as clamped.

| Dimension | Nodes, least to most specific | Columns |
|---|---|---|
| **Tenancy** | platform (NULL) → tenant | `tenant_id` |
| **Product** | any → `item_class` ancestors → `item_class` → `item` | `item_class_id`, `item_id` |
| **Counterparty** | any → `party_class` → `party` | `party_class_id`, `party_id` |
| **Space** | any → `site` → `zone` | `site_id`, `zone_id` |
| **Ownership** | any → `owner_party` | `owner_party_id` |
| **Metric** | any → `metric` (flat) | `metric_id` |

**Tenancy is not a declarable dimension** — it is the mandatory first component of
every depth vector, so a tenant's binding always beats a platform-shipped one.
Without this, a per-kind order ranking Product above Tenancy would let our default
outrank a tenant's own configuration: a correctness hole, not a support surface.

**Specificity is a vector, not a number.** Collapsing a componentwise comparison
to one integer lets a large count in a low-weight component beat a small count in
a high-weight one. CSS is twenty years of proof; there is no `specificity` column.

**A scope is a conjunction.** The columns are independent nullable axes, NULL
meaning "any". There is deliberately **no `num_nonnulls` CHECK** — one would
reverse the semantics and forbid the all-NULL platform default that shipped
defaults and clamping require.

**The entire matching language has cardinality one:** *is this node an
ancestor-or-self of that node*, over closure tables. No `<`, no `LIKE`, no `IN`,
no boolean connectives anywhere in the data. That is why this is not a rules
engine, and it is greppable.

**Ties are prevented, not broken.** Within a tree dimension a request node has
exactly one ancestor at each depth, so two matching bindings with identical depth
vectors must name identical scopes — forbidden by the unique index. **The
load-bearing dependency is that every scope dimension is single-parent.** If
`item_class` ever becomes many-to-many tags, resolution becomes non-deterministic
and this design is unsound. Defence in depth: equal vectors take the lower binding
id (deterministic — never stop the floor) and raise
`discrepancy.kind = 'policy_ambiguous'`.

```
policy_binding                -- WHERE a policy applies. Scope is IMMUTABLE.
  id, tenant_id               -- NULL = platform-shipped default
  kind
  item_class_id, item_id, party_class_id, party_id
  site_id, zone_id, owner_party_id, metric_id
  supersedes_id, note, created_at, created_by_id
  UNIQUE NULLS NOT DISTINCT (tenant_id, kind, item_class_id, item_id,
                             party_class_id, party_id, site_id, zone_id,
                             owner_party_id, metric_id)
  UNIQUE (id, kind)           -- composite FK target for value tables
  INDEX (tenant_id, kind)     -- the resolver's only scan

policy_change                 -- FACT. Append-only. Mandatory reason.
  id, tenant_id, occurred_at, recorded_at
  policy_binding_id, kind
  action                      -- created | revalued | retired | reinstated
  reason (NOT NULL)           -- a weight change with no reason is how tuning
                              --   becomes superstition
  recorded_by_id, authorised_by_id

<kind>_policy                 -- WHAT applies and WHEN. Append-only versions.
  id, policy_binding_id, kind
  CHECK (kind = '<its kind>')
  FOREIGN KEY (policy_binding_id, kind) REFERENCES policy_binding (id, kind)
  effective tstzrange
  EXCLUDE USING gist (policy_binding_id WITH =, effective WITH &&)
  ... typed scalars ...
```

**Eleven kinds:** `allocation`, `putaway`, `receiving`, `order_tolerance`,
`count_tolerance`, `shelf_life`, `sampling`, `cycle_count`, `specification`,
`observation_precedence`, `observation_acceptance`. The enum, the `%_policy` table
set and the compiled Rust registry are asserted equal in CI.

**Combination is most-specific-wins over the whole value row**, not per field —
you must not take `weight_rotation` from a customer binding and `weight_travel`
from a site binding, because weights are only meaningful relative to each other.
The one exception is **per-field clamping declared on the Rust value type**: the
winner's value, clamped against every less-specific match. That is what "customer
× item class, plus a site floor" (q33, q39) actually asks for, and it gives a
commercial product a platform-shipped ceiling no tenant can exceed.

**No per-row override flag.** That is `!important`, and it exists precisely to
escape the precedence order it was supposed to live in. If an operation needs both
a floor customers cannot undercut and a default they can, those are two fields
with two declarations.

**Instance agreements sit outside the resolver and win outright.** An explicitly
agreed value on an instance — `order_line.tolerance_under_pct` from an EDI order
or typed by a salesperson — is a different category of thing (what was *agreed*,
not what we do by default), and the resolver is not consulted.

**Reproducing a past decision is a foreign key, not a replay.** Value rows are
append-only, so `stock_allocation.allocation_policy_id` points at the exact
immutable row that scored it. One naming convention, asserted:
`<kind>_policy_id`, always an FK to the value row, never a version integer.

**Scope is immutable.** Editing a binding's scope would silently change the
meaning of every value row a past decision already references. Rescoping is
retire-and-create, linked by `supersedes_id`.

**The resolver returns an explanation, not a value** — winner, what clamped it,
and in explain mode the candidates considered and the near-misses with the
dimension that failed. *"Why is the 60 I configured not applying?"* is only
answerable by evaluating bindings that did not match. `resolve_batch` is the
primary interface; single-request is a wrapper.

#### Where the line sits

D13 said *"if we ever find ourselves adding a table where the logic itself is
rows, that is the line"* — true, and unfalsifiable as written. Sharpened:

> **Data may say where a number applies and how big it is. Only code may say what
> to do with it.**
> A row may contain: a scope node identifier, a period, and a typed scalar.
> A row may never contain: the name of a field, the name of an operator, a
> comparison, a boolean connective, the target of an action, or an ordering of
> steps. **The moment a table has a column whose *value* is a *column name*, we
> have crossed.**

That is a grep, and it caught its own author: the first draft of this design
failed it on a `band_axis` column.

**One bounded extension, fenced now rather than smuggled in later.** A value table
may have **at most one child, keyed on a single numeric axis declared in code**,
with `[lower, upper)` bands and a no-overlap exclusion constraint, containing only
bounds and typed scalars. `order_tolerance_band` is the only instance, and the
axis is named in the Rust type — there is no `band_axis` column.

#### Taxonomy changes are facts

*(Added on adoption.)* Depth vectors make **tree shape semantically
load-bearing**, and the failure mode is subtler than it looks. Membership changes
are intuitive — move a class out of `dairy` and dairy's rules stop applying. But
**cross-dimension flips are possible**: a binding at Product-3/Space-0 beats one
at Product-2/Space-2; re-parent so the first is depth 2 and the second now wins,
with nothing about either binding changed.

Retire-and-create for taxonomy nodes was rejected as too heavy for a structure
that legitimately evolves. Instead the change is recorded, and its blast radius is
computed **before** it is committed:

```
taxonomy_change               -- FACT. Append-only.
  id, tenant_id, occurred_at, recorded_at
  item_class_id, party_class_id            -- exactly one
  CHECK (num_nonnulls(item_class_id, party_class_id) = 1)
  action                      -- created | reparented | renamed | retired
  from_parent_id, to_parent_id
  reason (NOT NULL)
  affected_resolution_count   -- computed before the move, frozen on the fact
  recorded_by_id, authorised_by_id
```

> **Superseded by D72 and D73, and left here because the differences are the
> record.** No `taxonomy_change` table was built. The arms went onto
> `policy_change` instead — a taxonomy edit and a policy edit have identical
> consequences, and this paragraph's own argument is that they should therefore
> leave identical evidence, which is better served by one table than by two that
> must be read together.
>
> - `from_parent_id` is not stored. The previous parent is the previous
>   `reparented` row for that class, or the class's original parent if there is
>   none — storing it would be a second copy of a fact the fold already holds.
> - `action` is `policy_change_kind`, which gained `reparented` alone. **`renamed`
>   was declined**: nothing matches on `code`, so a rename cannot change which
>   policy wins. `retired` is question 149, and is undesigned rather than deferred.
> - `affected_resolution_count` is `affected_binding_count`, and **the rename is
>   the substance of D73** rather than a tidy-up: resolutions are unbounded and
>   bindings are not.
> - `recorded_by_id, authorised_by_id` were already on `policy_change` and needed
>   nothing.

Two things follow. The manager sees *"this move changes N active resolutions"*
before confirming, so the blast radius is a decision rather than a discovery. And
*"why did this item's shelf-life rule change last March"* becomes answerable by
the same anti-join that answers it for `policy_change` — which is the point: a
taxonomy edit and a policy edit have identical consequences and should leave
identical evidence.

#### Prerequisites

`item_class` and `party_class` are per-tenant rooted trees with closure-table
projections. `zone` becomes a real table (`zone(id, site_id, code, …)`,
`location.zone_id`), not a bare column — the Space dimension needs something to
FK to and a depth to read.

**`item.item_class_id NOT NULL` would break D19** and is not used: a shared item
(`tenant_id IS NULL`) cannot carry a mandatory FK into one tenant's private
taxonomy, since every other tenant reads it as an unresolvable link. Classification
is an association:

```
item_classification
  tenant_id, item_id, item_class_id
  PRIMARY KEY (tenant_id, item_id)      -- exactly one class per item per tenant
```

Single parentage is preserved, so the tie-freedom proof holds, and D19's thin
shared item survives.

#### Amendments to earlier decisions

- **D13** — `allocation_policy(scope_kind, scope_id)` → `policy_binding_id`. The
  scoring function stays code, unchanged and reaffirmed; the line becomes a grep.
  "We ship defaults, not hard-coded behaviour" becomes literally true: our
  defaults ship as `tenant_id IS NULL` bindings.
- **D14** — `customer.min_shelf_life_days`/`_pct` are **removed**; they cannot
  express "different requirements by category". Replaced by `shelf_life_policy`
  with clamping, so a site floor raises a customer rule.
- **D9 / q21** — `count_tolerance_policy`, distinct from order tolerance:
  different numbers, different screens.
- **D20 q55** — `order_line.quantity_tolerance_pct` becomes an instance
  agreement; the policy value moves to `order_tolerance_policy` plus its band
  child. **`order_line` is not a scope** — it would put instance-cardinality rows
  in a config table.
- **D8** — `discrepancy.kind += policy_ambiguous`; `respond_by` populated from
  `receiving_policy.respond_by_hours`.
- **D19** — a third RLS shape, for platform-shipped shared bindings.

**Rejects.** A polymorphic `policy_scope(scope_kind, scope_id, precedence)` pair,
structurally unable to express *(customer AND item class)*. A scalar or packed
specificity. Tie-breaking by entry order, `created_at`, or document order. A
per-row `is_override` flag. A `combine` column chosen per kind as data — combining
is semantics and belongs in the Rust type. Predicate rows.
`policy_value(policy_id, field_name, value)`. JSONB value blobs. Many-to-many item
tags. Per-tenant policy kinds or tenant-defined dimensions. A `policy_resolution`
audit table — volume is per-scan, and the value-row FK carries the same
information.

**Not a projection, deliberately.** `policy_change` holds no values, so a claim
that policy state is rebuildable from it reduces to rebuilding the value rows from
the value rows. The guarantee is carried entirely by a bidirectional anti-join:
every value version has a matching `policy_change` and vice versa.

### D23 — Observations generalise; the subject set opens on a registry

*Adopted 2026-08-01 from [mechanism-design.md](./mechanism-design.md), with the
`stock_count` boundary stated and the projection-under-policy rule lifted in.
D21, D25 and D26 remain proposed; references to them are marked.*

**Decision.** `measurement` is replaced by a two-level fact pair —
`observation_event` (the act) and `observation` (one result of it) — over three
reference primitives: an `observable` subject registry, a data-defined `metric`
vocabulary whose *result kinds* are code-defined, and a `dimension`/`unit` pair
carrying exact rational conversion to one canonical integer per dimension.

**Three welds, not one.** `metric` is closed by an enum — widening it alone fails
on the first temperature, because an affine unit cannot be an integer in an
implied canonical unit, an ETA is not an integer at all, and a quality grade is an
ordinal term. `subject_type` is closed by an enum. And `source` conflates three
orthogonal things: `carrier_actual` bundles *a carrier asserted it* with *an
instrument produced it*; `operator_correction` is not a source at all but a
lifecycle event about a different row.

#### The subject is typed once, in a registry

```
observable                    -- REFERENCE. The ONLY place the subject set widens.
  id, tenant_id (NOT NULL)
  kind                        -- GENERATED: which arm is set
  item_id, packaging_level, item_packing_config_id   -- each|inner|carton|layer|pallet
  package_type_id, package_id, lot_id, location_id, consignment_id
  device_id, vehicle_arrival_id
  asserted_unit_id, asserted_unit_content_id         -- [D21, proposed]
  CHECK (num_nonnulls(<arms>) = 1)
  CHECK ((item_id IS NOT NULL) = (packaging_level IS NOT NULL))
  CHECK (item_id IS NULL OR packaging_level = 'each'
         OR item_packing_config_id IS NOT NULL)
  UNIQUE (id, tenant_id)
  ... one partial unique index per arm ...
```

Every arm is a real FK, so an investigation cannot dead-end, and batch loading is
two hops with no N+1. **The fact tables are permanently stable in shape**: adding
"we now observe pallet-pooling accounts" is one column on a table of ~10⁵ rows and
zero change to anything holding 10⁷. This is the answer to question 20 — typed
subject FKs, but on the registry rather than on the fact, which is what stops the
subject set being frozen at the moment inbound opens it.

**`item` carries a `packaging_level`**, so "the carton, not the each" is
expressible. The identity is `(item_id, packaging_level, item_packing_config_id)`,
with the config NULL only at `each` — a carton is only a definite physical object
relative to a case pack, and because `item_packing_config` is versioned, a
corrected case pack cannot silently rewrite the dimensions of cartons shipped last
year. `packaging_level` is a **subject qualifier, never a unit**: a carton is not
commensurable with a millimetre.

#### The discriminated-union boundary rule

*(Amends the four-arm limit from the cross-cutting review.)*

> Typed nullable FKs with a mutual-exclusion CHECK are correct when the arms are
> **alternative identities of one referent** — a discriminated union where exactly
> one is structurally required and "none" is meaningless. They strain when the
> arms are **distinct relationships that merely happen to be exclusive today** —
> causes, demands, sources — because there exclusivity is a *policy*, and policies
> turn out to be wrong.

That explains both prior failures: `stock_movement`'s cause CHECK and
`goods_receipt`'s demand CHECK both had to relax to `<= 1`. Nobody will ever
discover an observation about no thing, or about two things at once. The rule
licenses `observable`'s arms and **constrains** cause sets, which stay at `<= 1`.

#### The boundary with `stock_count`

*(Stated on adoption. A `stock` cell is deliberately not an `observable`.)*

Admitting it would drag D4, D12, D20 and D24 into this decision for a case
`stock_count` already serves, and D24 gives `stock` a surrogate id that would make
it tempting. The line:

> **Quantities of cells are `stock_count`. Quantities of identified things are
> `observation`.**

Counting bin A3 is a `stock_count` — a cell is a coordinate, not a thing. A
supplier claiming "this pallet holds 40 cartons" is an `observation` about a
package, because a pallet is a thing with an identity. Counterparty-asserted
quantities therefore work without exception, because they are always properties of
an identified object.

#### Units, dimensions and metrics

```
dimension        id, code, canonical_unit_id      -- length|mass|volume|temperature
                 UNIQUE (id, canonical_unit_id)   -- |count|ratio|time_interval
                 -- shipped by us; no tenant_id, ever

unit             id, dimension_id, code, ucum_code, uncefact_code
                 factor_num, factor_den           -- EXACT rational. in = 254/10 mm
                 offset_num, offset_den           -- affine; degC = +273150 mK
                 display_decimals
                 UNIQUE (id, dimension_id), UNIQUE (dimension_id, code)

metric           id, tenant_id                    -- NULL = shipped (D19)
                 code, label
                 result_kind                      -- quantity|instant|code|boolean|text
                 dimension_id                     -- NOT NULL iff quantity
                 reserved                         -- only code may name these
                 applies_to                       -- which observable arms are legal
                 higher_is_better
                 UNIQUE (id, result_kind), UNIQUE (id, dimension_id)
                 UNIQUE NULLS NOT DISTINCT (tenant_id, code)
                 CHECK ((result_kind='quantity') = (dimension_id IS NOT NULL))
                 CHECK (NOT reserved OR tenant_id IS NULL)

metric_code      id, metric_id, code, label, ordinal
```

**`metric.aggregation` is removed; `higher_is_better` stays.** Aggregation
(last|min|max|mean) is a per-row data value selecting which fold a projection
performs — that is semantics, and semantics belong in the Rust type, the same
argument that refused a `combine` column in D22. `higher_is_better` is a display
and scorecard hint, not a fold.

**Why this is not a custom-field framework.** EAV is deferring *type* decisions to
runtime; its signature is one `value text` column, an arbitrary attribute name, an
untyped subject, and a schema that cannot be read. This has none of them: the
subject is an FK; the value is one of five typed columns chosen by the metric's
declared `result_kind` and enforced per row by a composite FK plus a CHECK; the
unit is enforced commensurable by a second composite FK, so recording a length in
grams is a constraint violation rather than a code-review finding; nothing
queryable is in JSONB; and a metric cannot add a column elsewhere or make the
system branch. **The result types are code; only the vocabulary is data** — D13
one level up.

Enforced two ways: reserved metrics ship with `tenant_id IS NULL, reserved = true`,
and **application code may name only reserved codes**, which is a grep in CI. The
day someone writes `if metric.code == "customer_special_thing"`, the build fails.

**Gross, net and tare are three metrics, not one with a modifier.** GS1 settled
this (AI 310n net, AI 330n gross). The single `weight` metric is genuinely
ambiguous today, and gross-versus-net is exactly what a carrier re-weigh surfaces.

#### The facts

```
observation_event             -- THE ACT. Append-only.
  id, tenant_id, observable_id
  observed_at                 -- VALID time (device clock, D5)
  recorded_at                 -- TRANSACTION time (server clock, D5)
  device_id                   -- the RECORDING device. Unconditional (D11).
  instrument_device_id        -- the MEASURING instrument; set only when
                              --   method IN ('instrument','scan')
  recorded_by_id / automation_key      -- CHECK num_nonnulls(...) = 1
  work_session_id, authorised_by_id, work_task_id, goods_receipt_id
  asserted_by_party_id        -- WHO claims it. NULL = us.
  method                      -- HOW: instrument|scan|keyed|derived|estimated
                              --      |transcribed|asserted
  ingestion_channel           -- THROUGH WHAT: edi|portal|csv|email|api|keyed
                              --               |scale|scanner|derived
  derived_from_event_id
  party_message_id, attachment_id       -- [D21 / inbound, proposed]
  -- NOTE: challenge fields are NOT here. See the correction below: a challenge
  --   is a property of a captured VALUE, and an event may carry several.
  UNIQUE (id, observable_id), UNIQUE (id, observed_at), UNIQUE (id, tenant_id)

observation                   -- ONE RESULT. Append-only. Never UPDATEd.
  id, tenant_id, observation_event_id
  observable_id, observed_at             -- denormalised, composite-FK'd
  metric_id, result_kind, dimension_id   -- denormalised, composite-FK'd
  value_numeric bigint        -- ALWAYS the dimension's canonical unit.
                              --   NO unit column: non-canonical storage is
                              --   structurally unrepresentable.
  value_instant, value_code_id, value_boolean, value_text
  uncertainty_dimension_id, uncertainty_numeric   -- half-width
  absent_reason               -- not_measured|not_applicable|unreadable|retracted
  entered_value numeric, entered_unit_id          -- as the counterparty gave it
  confidence smallint
  corrects_observation_id     -- the target was NEVER true (retroactive)
  retracts_observation_id     -- the target should not exist
  FK (metric_id, result_kind) -> metric(id, result_kind)
  FK (metric_id, dimension_id) -> metric(id, dimension_id)
  FK (entered_unit_id, dimension_id) -> unit(id, dimension_id)
  FK (value_code_id, metric_id) -> metric_code(id, metric_id)
```

`uncertainty_dimension_id` is separate from `dimension_id` because an ETA has no
dimension but "± 2 hours" is a real answer.

**Provenance is three deliberately uncorrelated columns.** A carrier re-weigh is
`(carrier, instrument)`. A supplier ASN is `(supplier, asserted)`. Our own eyeball
is `(NULL, estimated)`. The old enum could express the first and third only by
having a value per combination, which is why it ran out.

**The five-channel test.** The same fact — pallet SSCC 393123… weighs 412.5 kg —
arriving over EDI, a portal, a CSV, a dock scale and a keyboard produces five rows
that are **byte-identical in `(observable_id, metric_id, value_numeric,
entered_value, entered_unit_id)`**, differing only in provenance. One CI fixture
per channel, one assertion. If a new adapter ever needs a content column the
others do not have, the test fails on the day it is introduced. **That is the
interoperability requirement made testable instead of asserted.**

**Supersession, correction and retraction are three different things.**
Supersession needs no mechanism — a pallet weighed 400 kg Monday and 380 kg
Tuesday because a carton came off; both are true at their own times. Correction is
an explicit link because the old row was *never* true and must stop influencing
the projection **retroactively**. Retraction is an explicit link with no
replacement. All three are observations, so the table stays append-only with no
mutable status on a fact.

With both clocks and corrections distinguished from supersessions, two genuinely
different questions get different answers: *what did the pallet actually weigh on
Monday* (`observed_at <= Monday`, excluding corrected and retracted) and *what did
we believe on Monday* (`recorded_at <= Monday`, including rows later corrected).
The second is what a chargeback dispute needs, and it is unanswerable if
correction and supersession are the same thing.

#### The projection, and a general rule

```
observation_current           -- PROJECTION, keyed (observable_id, metric_id)
  observation_id, value_*, observed_at, recorded_at
  method, confidence, uncertainty_numeric, asserted_by_party_id
  observation_precedence_policy_id      -- WHICH policy row chose this (D22)
  in_breach                             -- against the resolved specification_policy
  cube_numeric                          -- COMPUTED here; never stored
```

**Precedence is a policy, not a number** — `observation_precedence`, one of D22's
eleven kinds. That makes *"trust supplier dimensions for items we have never
measured, but never trust their weight over our scale"* a configuration a manager
owns rather than a branch in our code.

> **General rule, lifted in on adoption: any projection maintained under a policy
> must record the policy row that produced it.** Otherwise the rebuild-and-assert
> job reports every policy change as drift — the projection was correct under the
> old policy and correct under the new one, and a rebuild cannot tell the
> difference without knowing which applied.

This is not specific to observations. It binds every projection D22 governs, and
it is why `observation_current` carries `observation_precedence_policy_id` and
`stock_allocation` carries `allocation_policy_id` (D22).

**A derived value is stored only when the derivation was a captured act with its
own provenance; otherwise it is computed.** `cube` is computed. A supplier
*asserting* a cube is an assertion and stays expressible.

**`package` dimensions are frozen at seal, not live projections.** Making them
projections of `observation_current` would break the stated invariant that *a
shipped package's dimensions are a historical fact about that consignment and must
never change* — a retroactive correction would rewrite the number a freight
invoice was computed against. Same argument as `goods_receipt_line.expected_quantity`.
The projection assertion applies to unsealed packages only.

#### Amendments to earlier decisions

- **Principle 5** — restated from a census of three conventions to a rule: every
  dimension has exactly one canonical unit; that unit is a **ratio scale**; stored
  values are integers in it; conversion is **exact rational**, never a float
  factor; affine units carry an offset that never reaches storage, so canonical
  temperature is **millikelvin** and `AVG`, differences and ranges are meaningful
  while cold-chain values never go negative. `numeric` is permitted for the
  preserved entered value — the prohibition is on floating point, not on exact
  decimal.
- **Principle 3** — the observation family is JSONB-free, asserted.
- **D5** — extended, not amended: both clocks carry their stated meanings.
- **D8** — `discrepancy.observation_id`; kinds `specification_breach`,
  `uncalibrated_instrument`. We do not reject the reading; we record it and raise
  the finding.
- **D9** — the policy deciding *when* to challenge is `count_tolerance_policy`
  (D22). *(This amendment originally promoted `challenged`/`challenge_context`/
  `confirmed` to `observation_event` and stripped `stock_count`'s copies. That was
  wrong — see the correction below.)*

#### Correction — a challenge belongs with the value, not the act

*(2026-08-02.)* The amendment above created a live defect. It moved the challenge
fields to `observation_event` **and** this decision's own boundary rule says a
`stock` cell is deliberately not an `observable` — *"quantities of cells are
`stock_count`"*. So a bin count has no `observation_event` to carry the challenge,
and **D9's founding worked example — "this location has not moved since the last
count of 50, and you have entered 47" — became unrecordable.**

The error was putting a **per-value** property on a **per-act** table. A challenge
contradicts a *number*, and one act may carry several: a cubing scan produces four
observations from one capture, and two of them may each be challenged. A single
flag on the envelope cannot say which.

> **The challenge lives with the captured value:** `challenged`,
> `challenge_context` and `confirmed` sit on **`observation`** (the result, not the
> event) and on **`stock_count`** (which is both act and result — a count is one
> number).

That is one *rule* applied to two value tables, not two mechanisms. It is the same
shape as the cell-key columns appearing on both `stock_movement` and `stock_count`,
which the model already accepts for the same reason: the value is where the
property belongs.
- **D13** — extended one level up: result types are code, vocabulary is data.
- **D19** — `dimension`/`unit` are global; `metric`/`metric_code` are shared
  reference; everything else tenant-scoped.
- **D20** — `measurement` **replaced**; `package.dimensions_source` deleted as a
  weaker private copy of `method`; `package_type.tare_weight_g` becomes an
  observation.

**Rejects.** Widening the enums in place. A polymorphic `(subject_type,
subject_id)`. Typed subject FKs on the observation row itself — correct goal,
wrong location: a ten-arm CHECK on the second-largest table plus ~80 bytes of
nulls per row forever. A join table between observation and subject (permits zero
and two subjects, both meaningless). One `value text` column or a JSONB result.
`metric` as an enum — it must carry attributes, unlike `activity_event.kind` where
code branches and an enum is honest. Statistical aggregates on the row. FHIR-style
comparators. Folding in `stock_count`. Money as a dimension. Storing `cube`.
`confidence` as the sole ranker. A float `factor` column. Milli-degrees-Celsius as
canonical. A separate lifecycle table for corrections.

### D24 — Containment joins the `stock` key; a package's placement is a fact

*Adopted 2026-08-01 from [mechanism-design.md](./mechanism-design.md), with the
three amendments from [containment-review.md](./containment-review.md) applied.
Numbering is kept stable across both documents: **D21, D22, D23, D25 and D26
remain proposed and are not adopted here.** Where this decision references them,
that is marked.*

**Scope.** The containment half of the proposed D24 is adopted. The supply-side
half (`expected_supply`, `stock_allocation`'s two supply arms) is **not** — it
depends on D21 and D23 and was not part of the review.

**Decision.** `package_id` joins the `stock` key as an **exclusive alternative to
`location_id``. `package_content` becomes a view over `stock`. A package's own
placement is a projection of a new append-only `package_event`.

```
stock                         -- PROJECTION
  id                          -- surrogate; stable
  tenant_id, item_id
  holder_location_id          -- \ exactly one
  holder_package_id           -- /
  lot_id, status_id, owner_id
  CHECK (num_nonnulls(holder_location_id, holder_package_id) = 1)
  UNIQUE NULLS NOT DISTINCT (tenant_id, item_id, holder_location_id,
                             holder_package_id, lot_id, status_id, owner_id)
  quantity, weight_g, allocated_quantity
  available_quantity          GENERATED (quantity - allocated_quantity) STORED
  resolved_location_id        -- @projection: holder location, or the holder's
  site_id                     -- @projection from resolved_location_id
  INDEX (tenant_id, item_id, site_id) INCLUDE (available_quantity)
        WHERE quantity <> 0

CREATE VIEW package_content AS
  SELECT id, holder_package_id AS package_id, item_id, lot_id,
         quantity, weight_g AS catch_weight_g
    FROM stock WHERE holder_package_id IS NOT NULL;
```

**Why an exclusive arm rather than a seventh dimension.** `location_id` and
`package_id` answer the same question at two resolutions, and a package's location
is a property of the package. Carrying both on a stock row is two independently
writable representations of one fact — the drift question 59 named as the default
outcome. As an exclusive arm the drift becomes **unrepresentable** rather than
detected, which is the stronger form. The key is six dimensions in seven columns.

Every capability D6 claimed for `package_content` survives, and two improve: it
gains `item_id`, `status_id` and `owner_id`, which it never had; and a sealed
carton's manifest acquires history, because it is the movements that put stock
into it up to `sealed_at`.

```
package_event                 -- FACT. Append-only.
  id, tenant_id, site_id
  occurred_at                 -- device clock; orders the register
  recorded_at                 -- server clock; first tiebreak
  recorded_by_id, work_session_id, authorised_by_id, device_id, work_task_id
  package_id                  -- THE SUBJECT. Always exactly one.
  kind        -- created | placed | contained | observed | identified
              -- | sealed | opened | relabelled | despatched | voided
  parent_package_id, location_id
  sscc, barcode
  source      -- operator_scan | label | asn | derived | correction
  asserts_placement           -- GENERATED: kind IN (created, placed, contained)
  CHECK (parent_package_id IS NULL OR location_id IS NULL)
  CHECK (kind <> 'contained' OR parent_package_id IS NOT NULL)
  CHECK (kind <> 'placed'    OR location_id IS NOT NULL)
```

Idempotency columns follow D5 until the `client_event` registry (D25, proposed)
is adopted.

**Placement is a register, not a counter — a different CRDT class from stock.**
D5 ruled out last-writer-wins because it silently discards a pick. That is right,
and it is about *quantities*. Two concurrent picks are both true; two concurrent
claims that a carton is on P1 and on P2 cannot both be true, and choosing a winner
is not data loss. **Quantities are counters; relationships are registers**, and we
implement the register over the same append-only log. Yjs stays ruled out — we
need the loser retained, ordered by device clock, and raised as a finding, none of
which `Y.Map` does.

Maintenance is **compare-and-set**, ordered by `(occurred_at, recorded_at, id)`,
so a late event with an earlier `occurred_at` loses without touching the current
value. The projection update is therefore commutative and idempotent, and
shuffling arrival order is a property test. A losing placement raises
`discrepancy.kind = 'containment_conflict'`.

`package.parent_package_id`, `location_id`, `resolved_location_id`, `status` and
`depth` become projections of `package_event`, maintained by the same
rebuild-and-assert job that guards `stock`. Nothing writes them directly.

**D6's nesting cap goes from two levels to three** (depth 0 = root). Overwrap →
pallet → carton is physically real.

**It is not a CHECK, and that matters.** `depth` is a *projection* column. A CHECK
on it would mean a `contained` event creating a four-level chain is a valid fact
the projection cannot represent — and since the projection must be rebuildable
from the log in any arrival order (D5), rebuild fails too. The result is not a
rejected event but an **unprojectable log and a wedged projection**, which is
worse than either alternative.

So the cap is enforced the way every other physical impossibility in this model
is — as a **finding**:

- `depth` records whatever the log implies, without limit.
- Depth greater than 2 raises `discrepancy.kind = 'nesting_too_deep'`.
- The resolution fold stays a **fixed three-hop join** — which is what D6's cap
  was actually protecting — and returns NULL beyond it.
- A NULL `resolved_location_id` on stock with quantity then raises
  `stock_without_location` through the existing invariant, so an over-deep chain
  surfaces twice rather than silently resolving to the wrong place.

The event is always accepted (D5), the fold stays non-recursive, and the
impossible state is visible rather than prevented.

#### The rule that decides which fact gets written

*(Amendment 1. The proposal said "custody changes" — not decidable, because a
carton is both a holder and a thing with a holder.)*

> - **`stock_movement`** — a stock cell's **key** changed: holder, lot, status or
>   owner; or quantity entered or left the system.
> - **`package_event`** — a package's **placement** changed: its parent, or its
>   location.

The two are disjoint **by subject**: a movement is about a quantity of an item, an
event is about a container. No act qualifies for both; none falls between.

| Physical act | What changed | Fact |
|---|---|---|
| Pick loose units from a bin into a tote | stock's holder | `stock_movement` |
| Pick 6 of 12 units out of a carton | stock's holder | `stock_movement` |
| Pick a whole carton onto an outbound pallet | the carton's parent | `package_event` |
| Move a pallet, bay A → bay B | the pallet's location | `package_event` |
| Goods arrive / leave | stock enters / leaves | `stock_movement` |
| Quarantine a pallet's contents | stock's status | `stock_movement` |
| Re-key a mis-recorded lot | stock's lot | `stock_movement` |
| Count a bin | nothing observed to change | `stock_count` |

**A whole-carton pick writes no movement, and that is the point.** Nothing
happened to the goods — they never left the container, nobody counted them,
nobody saw them. A movement would assert an inspection that did not occur. The
alternative writes forty movements for a pallet move, each a claim about goods
nobody touched, which is the lie D8 exists to prevent. **The fact recorded is the
fact observed.**

Work questions are asked at the **act** layer, not by unioning consequence
tables: one physical act inserts one act row plus all its facts in one
transaction, so *"what did this person do today"* joins out to whichever facts
resulted. That is D8's work-event invariant implemented properly rather than
scattered across the tables recording its effects.

**Fan-out is confined to the system boundary.** Receiving and despatching an
ASN'd pallet genuinely writes per-line movements, because goods entered or left
custody and both PO variance and D14's recall trace require it. Internal moves
are O(1) facts regardless of contents.

#### Package minting is a policy

*(Amendment 2.)* A `package` row exists **when something identifies it** — an
SSCC, a licence plate, a scan. Cartons sitting in bulk on a pallet are not
individually identified and do not become packages until labelled or picked.

This matters because LPN grain is **not** inherently larger than location grain —
five SKUs on a mixed pallet is five `stock` rows, exactly as five SKUs in a bin
is. Cardinality multiplies only if we mint a package per carton. The failure mode
to guard against is minting per-carton at receipt "for completeness" and
discovering the cost later. Minting granularity is configurable, and the default
is never per-carton.

#### Dead cells are reapable

*(Amendment 3. The proposal said `stock` rows are never deleted.)* `stock` is a
**projection** and rebuildable from the ledger by definition. A zero-quantity cell
with no allocation referencing it holds nothing the ledger does not. Rows are
**not deleted while referenced**; unreferenced dead cells are reaped by a
maintenance job. This removes the unbounded-growth concern rather than indexing
around it.

#### Movements become two-sided in space

The spine said `quantity` is "signed" *and* gave a from/to pair. Those are not
compatible, and "stock on hand is the sum of these" is then not a well-defined
fold. Corrected: **`quantity` is strictly positive and every movement folds into
two cells** — `−quantity` at the from-cell, `+quantity` at the to-cell. A receipt
has an empty from-side; a despatch an empty to-side. D5's CRDT property is
untouched. This is two-sidedness in *space*, not double-entry in *value*; the
financial ledger stays on the not-building list.

#### The cell key travels — and two dimensions were missing their pair

| Dimension | On `stock_movement` | Verdict |
|---|---|---|
| `item_id` | single | Correct. Changing item is a transformation: two movements. |
| `location_id` | pair | ✓ |
| `status_id` | pair (D4) | ✓ |
| `owner_id` | **absent** | The known D20 breakage. Pair added. |
| `lot_id` | **single** | **The same breakage, undetected.** Pair added. |
| `package_id` | new | pair by construction |
| `tenant_id` | single | Correct (D18); both sides must resolve to one tenant. |

```
stock_movement                -- amended
  item_id, quantity (> 0), catch_weight_g
  from_location_id, from_package_id, from_lot_id, from_status_id, from_owner_id
  to_location_id,   to_package_id,   to_lot_id,   to_status_id,   to_owner_id
  lot_id GENERATED ALWAYS AS (COALESCE(to_lot_id, from_lot_id)) STORED
  CHECK (num_nonnulls(from_location_id, from_package_id) <= 1)
  CHECK (num_nonnulls(to_location_id,   to_package_id)   <= 1)
  -- a populated side must carry the WHOLE key, not just a holder:
  CHECK (num_nonnulls(from_location_id, from_package_id) = 0
         OR (from_status_id IS NOT NULL AND from_owner_id IS NOT NULL))
  CHECK (num_nonnulls(to_location_id, to_package_id) = 0
         OR (to_status_id IS NOT NULL AND to_owner_id IS NOT NULL))
  CHECK (ROW(from_*) IS DISTINCT FROM ROW(to_*))
```

The whole-key CHECKs matter more than they look: under `NULLS NOT DISTINCT` a
NULL owner is a *different cell* from the site's entity, so an omitted column
would not error — it would silently fork the balance.

#### Re-lotting is a correction only

The generated `lot_id` preserves D14's recall index and query verbatim, but it
preserves the *index* while changing the *trace*: lot A re-lotted to B and then
shipped means the despatch carries `from_lot_id = B`, so *"which customers
received lot A"* returns nothing.

**Whether that is right depends on why the re-lot happened, so the answer is to
permit only one reason.**

- **Correction** — "we keyed A, it was always B". Permitted. The trace must
  **not** follow: those goods were never lot A, and following would produce a
  false recall.
- **Transformation** — a genuine merge or split. **Forbidden.** Lot merging is
  already unacceptable in food traceability, and forbidding it keeps D14's recall
  query exactly as written with no chain-walking to remember. A genuine split, if
  ever needed, is two movements through a transformation — the same treatment
  `item_id` already gets.

`adjustment_reason_id` records the correction on the row, and the partial index on
`WHERE from_lot_id IS DISTINCT FROM to_lot_id` becomes a correction audit rather
than a required leg of every recall.

#### Amendments to earlier decisions

- **D5** — registers clause added (relationships are registers, quantities are
  counters); `quantity` is positive with a two-sided fold.
- **D6** — `package_content` retired as a base table; containment columns demoted
  to projections; nesting cap raised to three levels and enforced.
- **D12** — `owner_id` and `lot_id` pairs restore the broken invariant.
  `stock_allocation` references `stock_id`.
- **D14** — `lot_id` pair; re-lotting is correction-only; recall query unchanged.
- **D20** — `owner_id` pair; the key re-framed as six dimensions in seven columns.

#### Index correction

Inbound Tier-0 asked for `stock(location_id, item_id)`. It must be
**`stock(resolved_location_id, item_id)`** — on `holder_location_id` it would make
container-held stock invisible to every commingling and putaway check.

### D24 (supply side) — expected supply, netting and pre-receipt allocation

> **This register carries two decisions numbered D24**, and the qualifier is how
> they are told apart: unqualified `D24` is containment above, and this one is
> always cited as *D24 (supply side)*. It is a collision, it is recorded as a
> known debt in [handover.md](./handover.md), and it is not being renumbered
> because more than a hundred citations name it.

*Adopted 2026-08-01 from [supply-side-design.md](./supply-side-design.md), which
corrected four defects and two false claims in the sketch deferred at D24's
adoption. Four further amendments applied on adoption, marked below.*

**Decision.** Supply that has not arrived is a projection, `expected_supply`, over
purchase order lines, transfer order lines, advised ASN content and return
authorisation lines. `stock_allocation` gains a second supply arm so demand can be
bound to a promise. Netting between a promise and its refinement is a **transient
suppression released as the refinement is consumed**, not a subtraction.

#### The bug the sketch had, and the invariant that encoded it

`quantity_available GENERATED (expected − refined − received − allocated)`
**double-subtracts**. A PO promising 100, an ASN advising 60, 58 arriving reads
`100 − 60 − 58 = −18`: the same 58 units subtracted once as suppression and once
as consumption.

`J8` in the invariant register — *"`quantity_refined` = the sum of refining
rows"* — **was the bug**, not the check for it. The replacement is a partition
identity that holds by construction, including under over-refinement
(`100 = 120 + 0 + 0 + (−20)`):

> `quantity_expected = quantity_refined + quantity_received + quantity_closed_short + quantity_outstanding`

**A wrong invariant is worse than a missing one**, because it confers confidence.
That is now the register's own first lesson.

```
expected_supply               -- role: PROJECTION. Folds intentions, assertions
  id, tenant_id, site_id      --   and facts.
  item_id (NOT NULL)          -- unresolvable content produces a finding, not a row
  owner_id, status_id         -- @projection from the source line: what the goods
                              --   will be ON ARRIVAL. Not a mid-flight claim.

  purchase_order_line_id      \
  transfer_order_line_id       |  exactly one — D23's discriminated-union rule:
  asserted_unit_content_id     |  these are alternative identities of one promise's
  return_authorisation_line_id/  origin, and "none" is meaningless for a projection
  CHECK (num_nonnulls(<the four>) = 1)

  refines_expected_supply_id  -- an ASN row refining a PO row. ONE LEVEL ONLY.
  CHECK (refines_expected_supply_id IS NULL OR asserted_unit_content_id IS NOT NULL)

  advised_lot_code            -- RAW supplier string. Never resolved to lot_id.
  advised_expiry_date         -- RAW. What FEFO cross-dock sorts on.
  expected_from, expected_to  -- a window: a dock appointment has two ends
  date_confidence             -- advised | ordered | inferred | none

  quantity_expected           -- @projection, per arm
  quantity_refined            -- @projection: SUM of OPEN children's OUTSTANDING
  quantity_received           -- @projection: receipts naming this row or a child
  quantity_closed_short       -- @projection: the source line's agreed release
  quantity_allocated          -- @projection: active allocations naming this row
  quantity_outstanding  GENERATED (expected - refined - received - closed_short)
  quantity_promisable   GENERATED (outstanding - allocated)

  closed_at, closed_reason    -- received_in_full | short_closed | superseded
                              -- | cancelled | expired | withdrawn
  derived_from_assertion_id, receiving_policy_id, allocation_policy_id
  UNIQUE (tenant_id, purchase_order_line_id)      -- one partial unique per arm;
  UNIQUE (tenant_id, transfer_order_line_id)      --   the idempotency guard for
  UNIQUE (tenant_id, asserted_unit_content_id)    --   message reprocessing
  UNIQUE (tenant_id, return_authorisation_line_id)
```

**The arm determines which rules apply to the row.** *(Amendment 3.)* This table
is a discriminated union whose branches carry different obligations: rule 3 (D21)
binds `asserted_unit_content_id` rows and not the others; the transfer arm derives
from our own despatch movements and has **zero exposure to rule 3**, which is why
it is the arm to build first. Anyone querying `expected_supply` needs to know
that, so it is stated here rather than implied across three sections.

`stock` was always multi-provenance too — `quantity` from facts,
`allocated_quantity` from intentions. The real distinction is **column grain
versus row grain**: D12 separated by column, and `expected_supply` cannot.

#### Ownership in transit is out of scope, and the boundary is stated

*(Settling question 107.)* `owner_id` is a **projection of the source line**
describing the arrival state, so it stays correct when a PO is amended to change
the receiving entity — which is the case that would actually have gone stale. It
is not, and does not attempt to be, a statement about who held title mid-flight.

> **We model custody — who holds the goods — and allocatable ownership — whose
> goods we may promise. Legal title timing, meaning when an asset moved between
> entities for tax, insurance and revenue recognition, is the finance system's
> record.** NetSuite remains the financial system (D20, q56).

Incoterms allocate risk and cost and explicitly do **not** transfer title; title
passes per the sales contract. That is a contract fact, not a warehouse fact, and
a warehouse system that models it will be wrong in a way nobody notices until an
audit.

The cases that look like they need it do not:

- **Consignment stock and VMI** work at rest (`stock.owner_id` = the supplier) and
  at consumption (a movement carrying `from_owner_id`/`to_owner_id`, D24).
- **Loss in transit** is a `discrepancy` with `counterparty_party_id`. We record
  who to pursue without recording who held title at the moment it vanished.
- **Inter-company transfer** crosses `legal_entity`, which D20 already says is a
  sale. The sale's documents and their timing belong to whoever raises them.

If this is ever needed it is a `supply_custody_change` fact with owner and
custodian pairs, and `owner_id` becomes its projection. Recorded as a **refusal
with a mechanism**, not an open question.

*(Amendment 2: the proposed scoping of S3 — "`<= 1` on grouping tables, `= 1` on
projections" — is **dropped**. D23's discriminated-union rule already gives `= 1`
here, and a second test reaching the same answer is the accretion this model
exists to refuse.)*

#### Multi-PO ASNs are supported, and the scalar FK is why

*(Settling question 109, which was recorded as an omission on a misreading.)*

The concern was that one `asserted_unit_content` line cannot draw on two purchase
order lines, and that widening it to an association table would invalidate J8's
partition identity. Both halves are true. Neither is needed.

**The X12 ORDER hierarchy level exists precisely to partition advised content by
purchase order.** As the inbound analysis established, `S-O-T-P-I` is not five
containers — S and O are *documents*, and the physical depth is pallet → carton.
So an ORDER-level split does not need a new structure: it becomes content lines
each naming one PO line, which is exactly what
`asserted_unit_content.resolved_purchase_order_line_id` holds.

*(Narrowed by D43. S remains the assertion; **O becomes a node** in
`asserted_unit`, because X12 states the purchase order at the order node and not
on the line, so filling the line's `raw_po_reference` from an 856 would write an
inherited value into an as-exchanged column. The settlement below is unchanged: a
content line still names exactly one PO line and the scalar FK is still correct.)*

**A content line never legitimately spans two POs, because the ORDER level
separates them.** Therefore:

- Multi-PO ASNs are **in scope and supported**. The scalar FK is correct rather
  than a limitation, and `refines_expected_supply_id` stays scalar.
- **J8 is safe**: each content line produces one `expected_supply` row refining
  one parent, so the partition identity holds per row.
- A content line that genuinely names two POs is **malformed** under GS1 and X12
  semantics and raises `assertion_unresolvable` — D8 behaving as designed.

The residual case is a loose channel — a CSV or portal with no ORDER grouping and
no per-line PO reference — where a shipment spans POs and nothing states the
split. That is unresolvable, and correctly so: **we cannot invent an allocation
the supplier did not state.** Which is also why the Australian grocery mandate
(Metcash and Coles both forbidding multi-PO ASNs) is the sane position rather
than a constraint we are working around.

#### Allocation gains a second supply arm

```
stock_allocation              -- INTENTION (amended)
  fulfilment_line_id          -- the demand. Unchanged, single arm.
  stock_id                    -- \ exactly one
  expected_supply_id          -- /
  CHECK (num_nonnulls(stock_id, expected_supply_id) = 1)
  origin_expected_supply_id   -- set once at binding, never cleared
  binding_kind    GENERATED   -- on_hand | pre_receipt | in_transit
  bound_at, expires_at
  rebind_count, last_rebound_at   -- volatility, not a path (q108)
  firm, firmed_at, firmed_by_id, firmed_reason   -- what the re-allocator may not steal
  allocation_policy_id
  state -- allocated | picking | picked | packed | fulfilled | short | released
```

**Allocations are never migrated by an ingestion event.** Every vendor rebinds a
PO-bound allocation when the ASN lands; we do not, and the reason is rule 3 — *an
ASN arriving must not rewrite one of our commitment rows*, because that is the
supplier's message authoring our intention. The ASN is an **input to the
allocator**, and the allocator's output is our own act with a fresh `bound_at` and
`allocation_policy_id`. No rebinding mechanism, no event log for intentions.

**Partial receipt splits the allocation, and the movements commit regardless.** An
allocation of 100 against an ASN row where 60 arrives becomes two rows — 60
cell-bound, 40 still expectation-bound — with `origin_expected_supply_id` on both.
**Rolling back a receipt because an intention could not be rewritten would be the
clearest D5 violation in the model.** A failed re-point raises a finding; the goods
are on the dock either way.

#### Re-point history: first, last, and a volatility signal

*(Settling question 108.)* An allocation records where it was **first** bound
(`origin_expected_supply_id`) and where it is **now** (the live arm). The path
between is not stored, and most of what it was wanted for is answerable elsewhere.

**"Which PO did this unit come from?"** is answered by **containment, not by
allocation history.** Goods arrive in identified packages, and D24 makes the
package part of the `stock` key — so PO X's pallet and PO Y's pallet are
*different cells*, and receipt movements carry `goods_receipt_line_id` → PO line.
Physical provenance lives in the fact ledger. Once a pallet is broken down and
cartons are mixed into one tote, provenance blurs — but that is **physically
true, not a modelling gap**, and a schema claiming otherwise would be lying.

**"Why did the promise slip?"** is answered by findings: `expected_supply` rows
close with `closed_reason`, assertions supersede via `assertion_stance`, and
`supply_withdrawn` / `commitment_unbacked` carry timestamps and a counterparty.

**What is genuinely unanswerable is auditing an automated re-allocator** — did it
release a binding it should not have, and why? A per-allocation event log is the
wrong shape for that: it records *"the row changed from A to B"*, which is the
changelog D25 refuses and which fails D25's own falsifier (bounded by our code
paths, not by things that happened in the world).

**A planner decision, by contrast, is a thing that happened in the world.** The
right mechanism generalises the proposed `work_creation_outcome` — trigger,
inputs considered, outcome, reason, policy version — which are columns a state
transition does not have. It is the same category as `policy_change` and
`taxonomy_change`: a **fact about a decision**, not a history of a row. It passes
the falsifier rather than needing an exception to it.

> **Deferred with a trigger: build `planner_decision` when the re-allocator is
> built.** Not before — there is nothing to audit until something automated is
> making these choices, and building the audit table first means guessing what it
> needs to record.

What ships now is `rebind_count` and `last_rebound_at` on the allocation: a
counter and a timestamp on an intention, which D25 permits, and which make *"this
promise has moved four times"* visible without a log. **An unstable promise is the
one worth looking at**, and that is the operationally useful half of the question.

#### Demand-side coverage — the symmetric fold

*(Amendment 4, settling question 106.)*

```
fulfilment_line               -- amended
  quantity
  allocated_quantity          -- @projection: active allocations, either arm
  uncovered_quantity    GENERATED (quantity - allocated_quantity) STORED
  INDEX (tenant_id, site_id, uncovered_quantity) WHERE uncovered_quantity > 0
```

`stock.allocated_quantity` and `fulfilment_line.allocated_quantity` are **the same
sum folded two ways** — one groups allocations by supply, the other by demand.
D12 already accepted the supply-side fold; having one materialised and not the
other means every *"is this demand covered"* question takes a different shape from
every *"is this supply committed"* question, for no reason but which we needed
first. **The asymmetry was the anomaly, not the column.**

D12's *"backorder is not an entity"* stands untouched: backorder is still derived,
now from a materialised sum rather than a live aggregate — exactly as
`stock.allocated_quantity` already was.

The payoff is the generated companion, not the column. *"Find every line not fully
covered"* stops being a join across all open lines and all allocations and becomes
a partial-index scan — the same move D24 made with `WHERE quantity <> 0`.

**The cost, named:** a `stock_allocation.state` transition now moves **two**
projections (one supply-side, one demand-side — not three, because the supply arms
are mutually exclusive), making it the model's hottest projection trigger.

#### Cross-dock adds no policy kinds

The capability needs a supply window, a shelf-life tolerance and a statement of
which arms may be allocated against. All are typed scalars folding into the
existing `allocation_policy` — `consider_expected_supply`, one
`allow_supply_*` boolean per arm, `window_before`/`window_after`,
`crossdock_min_window`/`max_window`, `crossdock_expiry_tolerance_days`,
`revalidate_on_receipt`, `rotation_key_expected`.

**Not an ordered supply-source child table.** D365's cross-dock template has one
and it fails S11 twice: `sequence` is an ordering of steps and `supply_source` is
a column whose value names an arm. The contrast worth recording:
`fill_sequence ∈ {inventory_only, crossdock_only, prioritize_inventory,
prioritize_crossdock}` names four **code-implemented strategies** and passes
cleanly. An enum naming columns does not.

**Revalidation at receipt may refuse, and refusal never blocks.** Window missed,
goods still land: the cross-dock allocation releases, putaway proceeds, a finding
is raised.

#### Supersession, short arrival, and never arriving

| Event | Mechanism |
|---|---|
| **ASN replaced or cancelled** | Derived rows close (`superseded`/`cancelled`); the parent's `quantity_refined` releases by the closed rows' outstanding in the **same transaction**. Replacement rows are created by **identity-preserving upsert** keyed on `(tenant_id, asserted_unit_content_id)` — never truncate-and-regenerate, because live allocations hold these ids. Allocations are **not** auto-released: `supply_withdrawn` is raised and a human decides. A counterparty's retraction must not silently un-promise a customer order. |
| **Arrives short** | The residual stays outstanding until the row closes or the fence passes. |
| **Arrives over** | `quantity_promisable` goes negative; `over_receipt`. Tolerance is an instance agreement (D22). |
| **Blind receipt** | Names no `expected_supply` row, so it nets **nothing**. The PO stays open and the overdue sweep raises it, rather than a matching heuristic quietly closing the wrong row. `receipt_unmatched` makes it countable. |
| **Never arrives** | The gap the whole comparison set gets wrong — D365's scheduled supply simply stops appearing, projected on-hand drops with no event, and commitments become unbacked invisibly. Here a sweep past `expected_to + receiving_policy.supply_overdue_hours` closes the row `expired` and raises `supply_overdue`, plus `commitment_unbacked` if allocations reference it. Both carry `counterparty_party_id` so they aggregate into the supplier scorecard. |

`expected_supply` rows are **retained after `closed_at`**. D24's dead-cell reaper
applies to `stock` only.

#### The availability read path

**Two index-only range scans, and D12's guarantee needs restating because the
version in the document was never true.** Availability was never a single row
read — it is a fold over the cells of one item at one site across lot × status ×
owner × holder. What was actually load-bearing survives intact:

> **No join, and no aggregate over a fact table, on the availability path.**

Two corrections to the adopted schema make that true:

- **`owner_id` and `status_id` join the index key.** D24's adopted index is
  `(tenant_id, item_id, site_id) INCLUDE (available_quantity) WHERE quantity <> 0`
  — neither owner nor status is in it, so every candidate needs a heap fetch and
  the `INCLUDE` buys nothing. **The failure mode is silence, not slowness:** a
  single-owner tenant never notices, and a 3PL tenant promises a vendor's units.
  D20 argued the column is a constant for single-owner tenants; D24 then wrote the
  index without it. Both go in, and `owner_id` becomes a **mandatory** argument of
  the availability function so the defaulting bug is unrepresentable.
- **`inventory_status.is_available_for_allocation` is never joined.** The
  allocatable status set is resolved once per request in code and passed as
  `= ANY`.

**Date-qualified ATP is deliberately not built.** The sketch claimed *"ATP over
future supply is one indexed read"*, which is false: ATP is a **running minimum
over a forward horizon**, and a PO for 100 landing day 30 against demand for 100
due day 5 nets to zero per row and the promise gets made. No vendor computes it
from row-per-supply storage. Building it needs a third projection hop (breaking
J24's two-hop cap) **and** the async escape D25 refuses by name — two exceptions
argued, not one. Until then the floor never asks *"when can I promise"*.

The column is therefore `quantity_promisable`, not `quantity_available`. Two
near-identical names with different meanings on two same-shaped tables is how
availability logic starts disagreeing with itself.

#### Rule 3 is narrowed, and that is a policy decision

*(Amendment 1. Stated here as what it is, rather than as a schema detail.)*

D21 rule 3 as adopted: *"Never projects into `stock` or into commitment."*
Restated:

> **Never projects into `stock`, and never into a commitment that survives
> withdrawal of the claim.**

**In plain terms: we now let a counterparty's claim reach a customer promise.**
Allocate against advised supply, the supplier retracts, and we hold a commitment
with nothing behind it. That is the price of cross-dock, and cross-dock is worth
it — but it is a change in what we allow, not a change in how we store it.

The compensating guarantees are real and both are asserted. Nothing silently
changes a balance, because an assertion-sourced allocation never touches a cell —
the test is **J19**: truncate every assertion table, and `stock` plus
`stock.allocated_quantity` rebuild byte-identical. And a withdrawal never silently
un-promises: it raises `supply_withdrawn` and leaves the commitment standing for a
human.

#### Invariants created

| # | Invariant |
|---|---|
| J3 *(restated)* | `stock.allocated_quantity` = active allocations **with `stock_id` set**, state ∈ `{allocated, picking, picked, packed}`. Enumerated, not adjectival. |
| J4 | `expected_supply.quantity_allocated` = the same fold over `expected_supply_id` allocations |
| J8 *(replaced)* | `quantity_expected = refined + received + closed_short + outstanding`. **The check that catches the double-subtraction; the old formulation was the bug.** |
| J9 *(restated)* | `parent.quantity_refined = SUM(child.quantity_outstanding)` over **open** children — never over children's `quantity_expected` |
| J25 | `refines_expected_supply_id` is acyclic and of depth exactly 1; violations raise `refinement_too_deep`. **Asserted by a job, never a CHECK** — a CHECK on a projection column wedges the rebuild (D24's own q92 ruling) |
| J26 | `expected_supply.quantity_received` = the fold of `goods_receipt_line` rows naming this row or any row refining it |
| J27 | Transfer arm: **no unit is simultaneously counted in origin `stock.available_quantity` and destination `quantity_promisable`** |
| J28 | No open row with `quantity_outstanding > 0` past `expected_to + grace` → `supply_overdue` |
| J29 | No active allocation references a closed or overdue row → `supply_withdrawn` / `commitment_unbacked`. The allocation is **not** released by the job |
| J30 | Rebuilding `expected_supply` preserves row identity. Truncate-and-regenerate is forbidden while any allocation holds an `expected_supply_id` |
| J31 | `fulfilment_line.allocated_quantity` = active allocations against that line, **either arm** |
| J19 *(scoped)* | `expected_supply` and `stock_allocation.expected_supply_id` are exempt and named — that is what rule 3's narrowing permits |
| S25 | Availability indexes on `stock` and `expected_supply` both carry `owner_id` and `status_id` in the key; no availability query joins `inventory_status` |
| S26 | `expected_supply` carries at most five maintained quantity columns; a sixth requires a recorded decision |

#### Amendments to earlier decisions

- **D21** — rule 3 narrowed, above. **Freeze-on-first-use gains a fourth
  referencer**: `expected_supply` references `asserted_unit_content.resolved_*`,
  and a late re-resolution must not re-key a projection row a live allocation is
  bound to.
- **D12** — `stock.allocated_quantity` narrowed by an enumerated predicate; the
  allocation lifecycle gains `picked` and `packed`, without which picked-but-not-
  despatched stock reads as available again — the `usage`-predicate cost D16
  refused, reintroduced by the back door. `fulfilment_line.allocated_quantity`
  added as the symmetric fold.
- **D16** — **q40 answered: yes**, a transfer can be allocated from despatch,
  against a destination-site row. In-transit stays off `stock`, reinforced.
- **D22** — no new policy kinds. Cross-dock scalars fold into `allocation_policy`.
  Incoterms and title-transfer triggers, if ever needed, are **instance
  agreements** on the order, not a policy kind.
- **D25** — `discrepancy` gains its **sixth** source arm, `expected_supply_id`,
  which is the recorded decision the cap demanded. Knowingly a *subject standing
  in for an absent cause* — the cause of "nothing arrived" is the absence of a
  movement — so D23's boundary rule is stretched deliberately rather than quietly.
- **D8** — `discrepancy.kind +=` `supply_withdrawn` (reinstated),
  `supply_overdue`, `supply_over_refined`, `commitment_unbacked`,
  `advised_lot_mismatch`, `refinement_too_deep`, `receipt_unmatched`,
  `over_receipt`. `discrepancy` gains `counterparty_party_id`, without which none
  of the supplier-facing kinds aggregate.
- **D24 (containment)** — the availability index gains `owner_id, status_id`. And
  `stock_movement` gains `CHECK (num_nonnulls(from_location_id, from_package_id) +
  num_nonnulls(to_location_id, to_package_id) >= 1)`: today a movement with both
  sides empty **passes every CHECK and is insertable**, but folds into two cells
  with no holder, which `stock`'s own CHECK forbids. Insertable but unprojectable
  — the wedged-rebuild failure D24 refused for `depth`.
- **D14 / q32 answered: no.** A lot is not created before its goods arrive. The
  advised code and expiry ride on `expected_supply` as raw, non-authoritative
  strings.

**Rejects.** `expected_supply` as a view or UNION — netting computed once beats a
rule every query remembers, *and* the FK target is the gate row Postgres needs to
serialise concurrent allocations without gap locks. A signed-adjustment ledger
with caller-supplied compensation (D365's, where the vendor documents that
avoiding double-count is the integrator's job). Depleting the parent (NetSuite,
Oracle) — it conflates *"the supplier promised 100"* with *"the supplier has told
me about 60"*, and supplier-promise accuracy becomes unanswerable. The movement
graph as the supply binding (Odoo's chained moves) — `stock_movement` has no
UPDATE grant, and a status predicate on the fold is a WHERE clause on the sum D5
keeps unconditional. **A transit location or transit warehouse** — no `transit`
value on `location.kind`, ever, recorded as a named refusal rather than left
protected by reasoning. A reservation as a ledger row with a zero physical delta
(OFBiz) — attractive, refused three ways independently, recorded so it is refused
once rather than re-proposed by everyone who has seen OFBiz. A `quantity_short`
column on `stock_allocation`. A stepped `stock_by_item_site` rollup (ERPNext's
`Bin`) — its `projected_qty` has no date, so a PO due in six months and a pallet
on the floor contribute identically. Quota / allocated ATP as a column —
entitlement is not supply, and consumption windows make it many-to-many.

### D25 — Status is derived, declared or forbidden; projections are enforced by the database

*Adopted 2026-08-01 from [mechanism-design.md](./mechanism-design.md), with the
role axis lifted into principle 2. D21 and D26 remain proposed.*

**Decision.** Every status column is exactly one of three things, determined by
the **provenance and role** of the row it sits on. Projections are enforced by
column-level grants, not by convention. One `client_event` registry owns
idempotency. One `acceptance` table owns assent.

**Why the cross-cutting review's own test is rejected.** *"Derive status where the
transition is evidence, store it where it is only state"* is not decidable —
there is no way to lose the argument that a transition is evidence. It also
answers the wrong question (the evidence in a consignment delivery is the
carrier's message, not a status log), and it has two branches where the model
needs four. The largest class here is statuses that are neither evidence nor state
but **arithmetic over facts that already exist**, for which an event table is
strictly worse than the column it replaces.

**The rule, and it is total:**

| Provenance / role | Treatment |
|---|---|
| **Fact** | **No status.** Facts do not have lifecycles. |
| **Intention** | **Declared** — the row owns it, one timestamp per state reached, no history table. Principle 2 defines an intention as mutable; giving it an immutable event log contradicts its own category. |
| **Finding** | **Declared.** `discrepancy.state` is ours, with `resolved_at`/`resolved_by_id` as its timestamps. |
| **Grouping / projection** *(role)* | **Derived** — materialised, maintained in the same transaction, never written by the application, rebuildable, asserted. |
| **A time-varying relationship** | **Forbidden as a column.** It lives in a fact table; the current value is a projection (D24's containment). |
| **Assertion** | **Forbidden.** The claim is immutable; *our position* on it is a separate fact (`assertion_stance`, D21). |

**The falsifier that stops event tables breeding:**

> An event table earns its place only if it has columns a state transition does
> not. If it would be `(entity_id, from_state, to_state, changed_at, changed_by)`
> and nothing more, it is a changelog. **An event table's row count is bounded by
> things that happened in the world; a changelog's is bounded by our own code
> paths.**

`policy_change` (D22) passes: mandatory `reason`, `authorised_by_id`, and a
valid/transaction time split. `work_task_status_history` fails, and is refused.

#### One column per source — D12's move, applied to status

`purchase_order.status ∈ {draft, issued, partially_received, closed, cancelled}`
is **two facts about two different things in one enum**: `draft/issued/cancelled`
is *our intention*, `partially_received/closed` is *arithmetic over the ledger*.
Storing both in one column means either the arithmetic overwrites the intention or
a human overwrites the arithmetic — and NetSuite is what happens when you do that
for fifteen years.

```
purchase_order   state          -- DECLARED: draft | issued | cancelled
                 issued_at, cancelled_at
                 receipt_status -- @projection: none|partial|complete|over
transfer_order   state / receipt_status               -- same split
order            state          -- DECLARED: placed | on_hold | cancelled
fulfilment       state          -- DECLARED: planned | released | cancelled
                 progress       -- @projection
goods_receipt    status         -- @projection: lines + movements
consignment      status         -- @projection: in-force carrier advice
package          status         -- @projection: package_event + consignment
work_task        state          -- DECLARED
stock_allocation state          -- DECLARED
discrepancy      state          -- DECLARED (finding)
```

**`order.fulfilment_status` is dropped.** It would be a third hop
(`stock_movement → fulfilment.progress → order.fulfilment_status`), breaking the
two-hop cascade cap. An order has few fulfilments; compute it on read.

**`work_task.state` stays declared**, for three stated reasons — not "a pick lasts
ninety seconds", which would not survive a week-long task. A monotone lifecycle's
timestamps **are** its event log transposed (`claimed_at`, `started_at`,
`completed_at` — zero extra rows, no greatest-n-per-group). The non-monotone parts
have a home already: `activity_event` gains `task_claimed | task_released |
task_reassigned`. And the hot read is the current state, which principle 6
decides. A status changelog here would be ~20,000 rows/day/site recording
transitions three columns already imply.

#### Enforcement is the database, not a sentence in a document

The model already says *"Nothing writes to `stock` directly. Ever."* Postgres can
make that a fact rather than a hope.

- **Column-level grants, issued column-wise from the first migration.** The trap:
  granting table-wide and then revoking one column **does not work**. A single
  table-wide `GRANT UPDATE` would silently disarm every projection guard in the
  schema, and nothing would fail loudly.
- **Fact tables get no `UPDATE` and no `DELETE` grant at all.** Append-only stops
  being a convention. One mechanism and one catalogue assertion replaces four
  separate immutability-trigger families.
- **Triggers exist for projection maintenance and nothing else.** One function per
  projection, named for it. Never business rules, defaults, validation or
  cascades. Unbounded trigger logic is Postgres's own accretion failure mode.
- **`SECURITY DEFINER` maintainers require `FORCE ROW LEVEL SECURITY`** on every
  RLS-protected table involved. Table owners bypass RLS by default, so without
  this the mechanism guarding the projections punches a hole through D18.
- **Registration is checkable.** Each projection column carries
  `COMMENT '@projection <source>'`, and CI diffs the commented set against the
  code registry of rebuild functions, bidirectionally.

#### Idempotency has one registry

```
client_event                  -- Unpartitioned, by necessity.
  tenant_id, client_event_id  -- PRIMARY KEY (tenant_id, client_event_id)
  site_id, device_id, work_session_id
  recorded_by_id / automation_key      -- CHECK num_nonnulls(...) = 1
  app_version, submitted_at, received_at
```

**Every fact table carries `client_event_id` as a plain FK**, not a unique one.
Two reasons the per-table `UNIQUE` had to go, both of which void D5's stated
non-negotiable property:

1. **One physical act produces many facts** — a receipt writes a `package_event`
   and N movements; a cubing scan writes an event and four observations.
2. **Postgres requires the partition key in any unique constraint.** On a table
   range-partitioned by `occurred_at`, a per-table `UNIQUE(client_event_id)`
   silently degrades to *per-partition* uniqueness, and a replay landing in a
   different month is accepted.

One submission inserts one `client_event` row plus all its facts in one
transaction; a replay aborts on the primary key and rolls back. The
`(tenant_id, …)` key also closes a cross-tenant unique-constraint oracle.

`recorded_by_id` stays denormalised on every fact row — D11 is emphatic that it is
the non-repudiable floor and a join is the wrong shape for an accountability query
— and CI asserts the denormalisation agrees with the envelope.

#### Acceptance, and what happens when it is contradicted

```
acceptance                    -- FACT: a person took responsibility for a state
  id, tenant_id, occurred_at, recorded_at, client_event_id
  accepted_by_person_id       -- ours (D19's global person)
  accepted_by_party_id        -- the counterparty, when external
  authorised_by_id
  goods_receipt_id, consignment_id, discrepancy_id, observation_id
  assertion_id                -- [D21, proposed]
  CHECK (num_nonnulls(<arms>) = 1)   -- a subject union, per D23's rule
  decision                    -- accepted | rejected | accepted_with_exception
  accepted_state              -- the derived value AS AT acceptance. FROZEN.
  basis                       -- manual | policy_auto
  observation_acceptance_policy_id   -- which policy auto-accepted (D22)
  reason_id, note
  rejection_window_expires_at -- statutory clock, computed once and frozen
```

The five arms are legitimate under D23's discriminated-union rule: these are
alternative identities of the thing being accepted, not a cause set.

**The frozen `accepted_state` answers open question 13.** When a derived status
recomputes to a value that contradicts an `acceptance`, the recomputation does
**not** lose — but it raises `discrepancy.kind = 'accepted_state_contradicted'`.

> The projection is the truth about the facts; the acceptance is the truth about
> what a person committed to; their disagreement is the finding.

```
projection_check              -- FACT: we checked a projection against its source
  id, tenant_id, projection_name, scope_kind, scope_id
  checked_at, rows_checked, rows_mismatched, duration_ms
```

Mismatches raise `discrepancy.kind = 'projection_drift'`, so the model's
self-consistency lands in the same queue as every other finding.

#### `row_audit` is not built

A generic `(table_name, row_id, before jsonb, after jsonb)` changelog is a
polymorphic reference plus an untyped payload, defended only by "we never read
it". It is also mostly dead weight: fact tables have no `UPDATE` or `DELETE`
grant, so their audit rows could only ever be inserts — doubling the write volume
of the largest tables in the system to record nothing. The tables that *are*
mutable are intentions and policy, and policy already has `policy_change`. If
compliance later demands row-level audit for intentions, that is an
infrastructure decision with its own justification, not a domain table smuggled in
on a principle-3 exception.

#### Amendments to earlier decisions

- **Principle 2** — the role axis, lifted in above.
- **D5** — the `client_event` registry replaces per-table idempotency uniqueness.
- **D8** — `discrepancy.kind +=` `containment_conflict`, `projection_drift`,
  `clock_skew`, `accepted_state_contradicted`, `policy_ambiguous`,
  `stock_without_location`, `specification_breach`, `uncalibrated_instrument`,
  `nesting_too_deep`. **Source arms are capped at five** with `CHECK <= 1`; a
  sixth requires a recorded decision, because these are causes, not a subject
  union.
- **D12** — one-column-per-source, generalised to status.
- **D17** — `work_task.state` confirmed with the reason replaced; `activity_event`
  gains three kinds.

**Rejects.** A global `events` table. Per-entity status-history tables. The
evidence test. CQRS with async projectors — the model already chose transactional
projections for `stock`, and async would make availability and task queues stale
on exactly the paths D5 spent its coordination budget to get right.
Event-sourcing ceremony: aggregates, upcasting, snapshots. Postgres tables *are*
the snapshots; upcasting is what you build when you cannot migrate.
System-versioned temporal shadow tables. `row_audit`. An event log for intentions.

### D26 — Extensibility: a schema compiler, an outbox, and no document store

*Adopted 2026-08-01 from [mechanism-design.md](./mechanism-design.md), with four
amendments applied. **This completed D1–D26; D27 onward followed.***

**Decision.** Extensibility decomposes into exactly three things a tenant can
want, and each gets one primitive:

| Want | Primitive |
|---|---|
| **Data** the product does not model | `record_scheme` — a schema compiler |
| **Decisions** the product makes | D22's scope lattice — nothing new |
| **Reactions** to things that happen | `event_subscription` + `outbox` |

A fourth — presentation — is not extensibility and never touches the schema.
**Three is the budget. A fourth request means one of the three is wrong.**

#### The premise that changed

Principle 4's refusal rested on *"adding a column is cheap and migrations are
routine"*. That has a hidden subject — cheap **for us** — and D18 made it
non-universal. But the refusal was never about columns; it was about **untyped
attribute soup**. So the boundary moves from *who may add a column* to **what a
column may be**.

```
record_scheme                 -- REFERENCE. tenant_id NULL = shipped by us (D19)
  id, tenant_id, key, version
  provenance                  -- fact | intention | assertion | finding
  role                        -- reference | grouping
  attaches_to                 -- a core entity (see "one registry" below)
  cardinality                 -- one | many
  physical_table              -- 'ext_daff_biosec_discrepancy_v1'; immutable
  manifest_source bytea, manifest_hash          -- retained; never queried
  source                      -- shipped | tenant | plugin
  plugin_id, state, materialised_at, created_by_id
  UNIQUE (tenant_id, key, version)

record_scheme_field           -- the COMPILED SYMBOL TABLE. Rows, not JSON.
  id, record_scheme_id, ordinal, column_name, label
  field_type                  -- integer|text|boolean|date|timestamptz
                              -- |quantity|money_minor|enum|ref|attachment
  unit_id                     -- FK unit (D23). NOT NULL iff quantity.
  currency                    -- NOT NULL iff money_minor
  enum_values text[]          -- non-empty iff enum   -> generates a CHECK
  ref_entity                  -- NOT NULL iff ref     -> generates a real FK
  required, min_value, max_value
  CHECK (parameter presence matches field_type)
  UNIQUE (record_scheme_id, column_name)
```

**This is a code generator whose input happens to live in a row**, not an
attribute store. The generated table gets a real `tenant_id` FK, a real parent FK
with **`ON DELETE RESTRICT`** (not CASCADE — a fact scheme's evidence must not be
destroyed when a receipt is deleted), `client_event_id` where
`provenance = fact`, RLS with `FORCE`, column-wise grants **derived from
`provenance`**, and real columns with real types, CHECKs and indexes.

Against six tests: referential integrity — real FKs. Database enforces invariants
— NOT NULL, CHECK, UNIQUE, RLS, and no UPDATE grant for facts, which is the
**first time principle 2's categories are mechanical rather than a naming
convention**. Survives migrations — a scheme version *is* a migration. Queryable —
`contamination_g` is an integer with planner statistics. Debuggable — `\d` tells
you everything; there is no interpreter. Not a language — nine type constructors,
no nesting, no `any`. EAV fails all six; spare-column sidecars fail all six;
JSONB-with-a-schema fails all six.

**Evolution is additive in place, otherwise a new version.** Adding a nullable
column is metadata-only in Postgres and permitted; narrowing, dropping or adding
NOT NULL mints version N+1 with a new table and a generated backfill, and the old
table stays. **A scheme is never rewritten** — D8's invariant applied to schema.

**Ceilings are declared numbers**: 50 schemes per tenant, 60 fields per scheme,
100 tenant-defined metrics. Not because 51 breaks anything, but because the
ceiling is what keeps this a schema *extension* rather than a schema *escape*.
Salesforce's flex-column pivot is what happens when it comes off.

**The boundary against D23:** a single unit-carrying number with provenance is a
`metric`; a coherent multi-field record is a `record_scheme`. Most "we need a
field" requests are actually observations, which is why D23 absorbs the larger
share.

#### Ownership, not detection

*(Amendments 3 and 4.)* The compiler runs DDL from tenant-supplied declarations,
so it is the same deliberately-elevated path question 102 identified for the
projection maintainer. It gets the same treatment, and one mechanism does the work
of two:

> **The compiler role owns every generated table. The application role receives
> DML grants only.**

`ALTER TABLE` requires ownership in Postgres, so manual drift is not *detected*,
it is **unrepresentable** — D25's move applied one level down. That collapses the
drift job from *"has anything diverged?"* to *"did a materialisation complete?"*,
which is a far smaller surface and is checkable at materialisation rather than by
polling.

What remains is checked in **one set-based query** over `pg_attribute` and
`pg_constraint`, aggregated to a hash per table and joined to `record_scheme` —
not one query per table. At any plausible scale that is a catalogue read of a few
hundred thousand rows.

The compiler role requires `FORCE ROW LEVEL SECURITY`, its own audit, and a
stated rule on who may invoke materialisation.

**Scale is a ceiling, not an expectation:** shipped schemes have
`tenant_id IS NULL` and are **shared**, so only tenant-declared schemes multiply.
**Escape hatch, recorded not built:** if generated tables ever grow large enough
to matter, they move into a per-tenant Postgres namespace — which also turns
"everything belonging to tenant T" into a schema listing rather than a filtered
scan, helping export and deletion. The trigger is catalogue-query latency; the
cost is a dimension on every generated FK and grant.

#### Reactions

```
event_subscription            -- INTENTION
  id, tenant_id, name
  source_table                -- a core entity (see below)
  site_id, party_id, item_class_id      -- scope filter, D22's dimensions
  delivery                    -- webhook | plugin | outbox_only
  endpoint_id, plugin_id, format_version, state

outbox                        -- FACT. Written in the SAME TRANSACTION as its fact.
  id, tenant_id, occurred_at, enqueued_at
  source_table, source_id     -- audit only; NEVER dereferenced (see below)
  subscription_id
  party_message_id            -- THE PAYLOAD, rendered at enqueue (D21)
  attempt_count, next_attempt_at, delivered_at, last_error
  INDEX (next_attempt_at) WHERE delivered_at IS NULL

registered_endpoint           -- SSRF guard: targets are registered, not free-form
  id, tenant_id, url, host, auth_kind, secret_ref, verified_at, active
```

**The outbox's polymorphic pair is permitted, and the reason is an invariant, not
a comment.** *(Amendment 2.)*

> **S27 — the outbox source reference is never dereferenced. The rendered bytes in
> `party_message_id` are the payload.**

D10's decisive argument against polymorphic references was that batch loading
cannot be expressed over one. That argument does not apply to a queue *only
because* the payload is rendered at enqueue. Without S27 asserted, a delivery
worker that looks up the source puts polymorphic joins back on a hot path, and
nothing would fail.

**Subscription filtering uses D22's scope dimensions, not a predicate language.**
*"Webhook me for discrepancies against supplier X"* is a scope, not an expression
— one addressing mechanism across policy and subscriptions.

#### One registry, not three lists

*(Amendment 1.)* `record_scheme.attaches_to` and `event_subscription.source_table`
were both specified as hand-maintained closed enums naming core tables. Two lists
tracking the same moving target drift — and had already begun to: three members of
`source_table` were renamed by decisions written in the same document.

Both are instead **derived from the code-side table registry** that S4 already
requires and CI already diffs against `information_schema`. Attachability and
subscribability become capability flags on a registration, not lists someone
remembers to update.

`observable`'s arms (D23) stay as they are and are legitimately different: those
are typed FKs to *instances*, not an enum naming *types*.

#### Plugins, with one divergence from Nosdesk

The plugin surface reuses Nosdesk's sandbox wholesale — opaque-origin iframe on a
separate registrable domain, `connect-src 'none'`, Comlink over a transferred port
authenticated by holding the port, manifest-declared permissions enforced from
trusted DB state, signed bundles with trust tiers, an egress proxy injecting
credentials the plugin never sees.

**One deliberate divergence: no document store.** Nosdesk's
`plugin_collection_rows.data jsonb` is right for a helpdesk, where a plugin's
saved addresses are nobody's business but the plugin's. It is wrong here, because
**warehouse plugin data is almost never private to the plugin** — a biosecurity
form is evidence in a dispute, a quality result gates a release. In a JSONB
collection it cannot be joined to a receipt, reported on, exported into a claim,
or given a foreign key: D23's trapped-in-`measurement` failure, one level down. A
warehouse plugin declares a `record_scheme` and gets a real table.

#### Two deferrals, both with thresholds

**`decision_rule`** — a compiled, type-checked, total predicate (Cedar or
equivalent) is a genuinely better answer than an interpreted rule table, and if
ever built it should be adopted rather than written. But it crosses D22's
sharpened line on every clause, and **both its motivating examples have
evaporated**: *"vendor X's goods go to zone 3"* is a `putaway` binding, and
subscription filtering is a scope. **Revisit when two tenants want different
behaviour at the same decision point** — one tenant is a shipped vertical.

**Server-side WASM** — nothing in the three axes needs arbitrary computation
inside a warehouse transaction, and for the one job it might serve WASM is
strictly worse: Wasmtime bounds guests with fuel or epoch interruption, and
neither is a **type checker**, which is the property we actually want. Adopting it
would buy generality by deferring *what a plugin computes* to runtime — the exact
failure the standing direction names.

#### Amendments to earlier decisions

- **Principle 1** — corollary: an extension mechanism is itself a primitive.
  Three is the budget.
- **Principle 2** — the categories become mechanical: a scheme's `provenance`
  drives its grants.
- **Principle 4** — amended above.
- **D7 / q14** — Nosdesk is shared as **library crates** (sandbox, bridge,
  consent, signing), not as a deployment. **Not** shared: the plugin collection
  store.
- **D19** — the nullable-tenant reference shape covers `record_scheme`.
- **D20** — cited as the consistency check: an org with no schemes and no
  subscriptions encounters none of this, because there are no rows.
- **D25 / q102** — answered for the compiler: ownership, `FORCE ROW LEVEL
  SECURITY`, its own audit.
- **Tenant export and deletion are materially de-risked** — because tenant
  extension data lives in enumerable named tables rather than shared blobs,
  "export everything for tenant T" and "delete T except statutorily retained
  facts" are generated queries over `record_scheme.physical_table`, not a hunt.
  That is an argument *for* the compiler that has nothing to do with
  extensibility.

**Rejects.** EAV. Spare-column sidecars (`ext_int_1..20`) — EAV with extra steps,
and the planner's statistics land on `ext_int_7` rather than on `contamination_g`.
JSONB plus JSON Schema. Schema-per-tenant with **tenant-owned DDL** — it fails on
operations, not capability: if the tenant owns the DDL, *we* cannot upgrade, and
the rebuild-and-assert jobs holding this model together cannot be written once.
(Database-per-tenant as a *deployment topology* stays available per D18, and
namespacing without tenant DDL stays available as the escape hatch above.)
Interpreted rule tables. Writing our own condition language. Server-side WASM
(deferred, seam open). Nosdesk's plugin collection store. Shipped verticals as the
*only* answer. A polymorphic `(entity_type, entity_id)` for attaching extension
records — the genericity lives in the compiler, not in the row.

### D27 — `device`: one table, two roles, calibration as facts

*Adopted 2026-08-02, settling question 99. `device_id` is referenced by D5, D11,
D23, D24 and D25 and was defined by none of them.*

**Decision.** One `device` table covering both roles a device plays, with
commissioning as timestamps and calibration as an append-only fact.

#### Two roles, one table

D23 already distinguishes them on `observation_event`:

- **`device_id`** — the **recording** device. The handheld that submitted the
  fact. Unconditional under D11: every fact names the hardware it came from.
- **`instrument_device_id`** — the **measuring** instrument. The scale, the cubing
  station, the thermometer. Set only when `method ∈ {instrument, scan}`.

A handheld with a built-in scanner is both, on different facts. That is two roles
of one thing, not two things, so it is one table with a `kind` — the same
reasoning D17 used for `work_task`.

```
device                        -- role: REFERENCE
  id, tenant_id
  home_site_id                -- nullable; a handheld moves, a dock scale does not
  kind                        -- handheld | scanner | scale | cubing_station
                              -- | printer | workstation | gateway
  serial_number, model, manufacturer, asset_tag
  commissioned_at, decommissioned_at        -- see below

  -- measurement capability: NULL for a device that measures nothing
  measures_dimension_id       -- FK dimension (D23)
  resolution_numeric          -- least count, canonical unit. D23's
                              --   observation.uncertainty default comes from here.
  range_min_numeric, range_max_numeric

  -- trade-legal metrology
  approval_authority          -- nmi | oiml | none
  approval_number             -- e.g. an NMI pattern approval
  verification_class
  calibration_valid_until     -- @projection from device_calibration

  CHECK ((measures_dimension_id IS NULL) = (resolution_numeric IS NULL))
```

#### Calibration is a fact, not a column

A device is calibrated repeatedly, and *"was this scale in calibration when it
produced that weight"* is the question a billing dispute turns on.

```
device_calibration            -- FACT. Append-only.
  id, tenant_id, device_id
  calibrated_at, valid_until
  performed_by_party_id       -- the external calibrator
  certificate_ref, attachment_id
  outcome                     -- passed | adjusted | failed
  recorded_by_id, client_event_id
```

This gives D23's `uncalibrated_instrument` finding an actual source: an
`observation` whose `instrument_device_id` had
`calibration_valid_until < observed_at` raises it. Per D23 and D5 we **do not
reject the reading** — the goods really did weigh that much — we record it and
raise the finding, because the reading may be perfectly good and the certificate
merely lapsed.

#### Why this is not bureaucracy: catch weight

D20 admitted catch-weight items, which are **sold by actual weight**. Under the
National Measurement Act a weight used for trade must come from an instrument with
pattern approval and current verification. So the chain
`observation → instrument_device → approval_number + calibration_valid_until` is
what makes a catch-weight consignment legally invoiceable. Without it we can weigh
things but cannot defend the number.

#### `active` is derived, not stored

`commissioned_at` and `decommissioned_at` are a **monotone lifecycle**, so their
timestamps *are* the event log transposed — D25's `work_task` reasoning applied
verbatim, and `active` is derived rather than a mutable boolean. Decommissioning
matters: a dispute may turn on whether a scale was still in service, and a boolean
cannot say when it stopped being.

#### A gap this exposed in principle 2

**Reference tables have a role but no provenance.** `device`, `dimension`, `unit`,
`metric` and `observable` are all role `reference`, and none of them is a fact, an
intention, an assertion or a finding — we neither observed them nor plan them nor
were told them.

This is not a fifth provenance value: under D21's admission test, reference data
changes who may write it (us) but not what may project from it or whether it may
be revised in any distinctive way. The honest statement is:

> **Role `reference` tables do not register a provenance.** Their mutable state is
> treated as an intention would be — declared, with timestamps, no history table
> unless a transition is itself a fact worth keeping (`device_calibration` is).

Recorded here rather than left implicit, because D25's rule table is indexed by
provenance and would otherwise have no row for the tables it most obviously
applies to.

#### `device_id` and `automation_key` are different questions

D21 admits machine actors via `automation_key XOR recorded_by_id`, and q105 asks
what an automation key is. `device` answers it by contrast rather than directly:
**`device_id` is *how* a fact was captured; `automation_key` is *who* captured
it.** An EDI parser has an automation key and no device; a handheld has a device
and a person. They are orthogonal and both may be present.

#### Amendments to earlier decisions

- **D5, D11, D24, D25** — `device_id` becomes a real FK to `device`. It was a bare
  column on `stock_movement`, `package_event`, `stock_count` and `client_event`.
- **D23** — `observation.uncertainty_numeric` defaults from
  `device.resolution_numeric`, which is now a real column;
  `uncalibrated_instrument` gains its source.
- **D19** — `device` is tenant-scoped, not global. Unlike `person`, hardware
  belongs to an organisation.

### D28 — Failed scans: record resolution failures, never decode failures

*Adopted 2026-08-02 from [d24-open-questions.md](./d24-open-questions.md),
settling question 89. Two amendments applied: the canonical length unit becomes
micrometres, and `discrepancy`'s source-arm cap is retired rather than spent.*

**Decision.** `activity_event` gains four **resolution**-failure kinds and typed
columns for the identifier that failed to resolve. `scan_ok` is **deleted** from
D17's kind list. Scan-rate denominators come from `client_event` (D25), which
already takes one row per fact-producing act.

#### The obvious answer is wrong in both directions

**A decode failure is not observable, so a `no_read` kind would read zero forever
and be believed.** On the dominant handheld platform a no-read produces *nothing*:
the scan intent bundle carries `source, label_type, data_string, decode_data,
decoded_mode` — no status field and no failure variant — and the scanner-status
enum has no "failed decode" value. Trigger-driven readers emit events on success
only.

Making it observable would mean owning the decode session via soft trigger, which
makes **scanning depend on our process being healthy** — D5's central trade run
backwards. Refused with the reason.

**The population we *can* record is the benign one, and the decision must say so.**
A misread returns a valid-looking wrong value and is indistinguishable from a
correct scan; it surfaces only as a contradiction against an expectation
(`scan_mismatch`, `containment_conflict`). Recording unresolvable identifiers does
nothing to find misreads, and a failure kind must not imply otherwise.

**`activity_event` is the only table that can hold a subjectless fact.** Not
`observation` — `observable` has typed arms under `= 1`, and D23's licensing
argument is that *"nobody will ever discover an observation about no thing"*. Not
`package_event` — `package_id` is the subject, always exactly one. So a failed
scan is **keyed on context, never on a subject**: adding a `package_id` that is
NULL for the entire failure population is the always-NULL column D23 refused.

#### The volume argument, made rather than assumed

A site at 5,000 picks/day takes 20,000–50,000 application-level identifier
captures/day across picking, receiving, putaway, replenishment and counting.

| | rows/year/site | storage |
|---|---|---|
| `scan_ok` in `activity_event` | 7.3M – 18M | 2.2 – 5.5 GB |
| its `client_event` companion (S19) | 7.3M – 18M | **and this one cannot be partitioned** |

The second row is the cost nobody had computed. S19 requires every fact to carry a
`client_event`, and D25 states that `client_event` can **never** be partitioned.
Failures only, at 0.1–2% of captures, are 20–1,000 rows/day/site — **a ~100×
ratio, and it is the whole decision.**

**The denominator survives without `scan_ok`.** `client_event` already carries
`device_id`, `recorded_by_id`, `work_session_id`, `site_id` and `submitted_at`,
one row per act, so scan rate is a `GROUP BY` over an existing table at zero
marginal cost. Its only bias is scans producing no fact at all — exactly the
population that should not mint rows. D17's denominator claim is narrowed:
`activity_event` supplies **labour-time** denominators (`idle`, `task_paused`,
`skip`, `search_failed`); `client_event` supplies **scan-rate** denominators.

```
activity_event                 -- FACT (D17), amended. Range-partitioned on occurred_at.
  id, tenant_id NOT NULL, site_id NOT NULL      -- both absent from D17's sketch
  occurred_at, recorded_at
  client_event_id              -- PLAIN FK (D25); D17's "(unique)" was stale
  device_id                    -- FK device (D27): the RECORDING device
  instrument_device_id         -- FK device: the scan engine, when separately mounted
  recorded_by_id, work_session_id, authorised_by_id
  work_task_id, location_id                     -- CONTEXT, not subject; both nullable
  location_provenance          -- observed | context
  kind
  scanned_value    text        -- the decoded string, capped and truncated
  symbology_id                 -- FK symbology (shared reference)
  parsed_ai        text        -- FIXED-WIDTH TEXT, never smallint: '00' keeps its
                               --   leading zero and 310n has a variable final digit
  expected_entity_kind         -- package | location | item | lot | none
  attempt_ordinal  smallint    -- within the client_event
  detail                       -- retained; nothing queryable may live here

symbology                      -- REFERENCE, tenant_id NULL (D19 shape)
  id, aim_code, device_label_type, label
  -- a table, not an enum: scanner platforms ship ~60 label types and they grow
  -- with firmware. An enum turns a vendor release into a migration.
```

**`location_provenance` earns its column.** A failed scan's location is the app's
*belief*, not an observation. Mixing the two makes a per-bin failure heatmap
confidently blame the last bin that scanned correctly.

**Grain is one row per operator-initiated capture attempt.** This belongs in the
decision, not in an implementation note: camera decoders run per preview frame, so
a three-second aim is 45–90 failed decodes, and at decode-attempt grain a
20,000-scan site generates ~1.3M rows/day. **Four orders of magnitude turn on that
sentence.**

**Retries coalesce by `client_event_id`, server-side.** An operator scanning a
smudged label six times in four seconds is one world event. Six rows would measure
label quality × operator persistence, and a patient operator would score worse
than one who gives up. Never coalesce on the client, where the discard has no
audit and depends on app version.

#### Amendment 1 — the canonical length unit becomes micrometres

Principle 5 as restated by D23 makes canonical length **millimetres as integers**.
A verifier aperture of ten thousandths of an inch is 0.254 mm and **rounds to
zero**. The design used this to argue barcode grading is out of scope; the real
problem is that the failure is *silent truncation* rather than a stated boundary.

> **Canonical length is micrometres.** 0.254 mm is 254 µm, a standard pallet is
> 1,165,000 µm, and a `bigint` covers nine orders of magnitude beyond anything a
> warehouse holds.

It costs nothing today because nothing is built. **Instrument specifications —
aperture, wavelength — are not observations of goods and do not use the canon.**
They are `device` attributes (D27) with their own units. That is the clean split:
*the canon measures things we handle; specs describe the instruments that measure
them.*

#### Amendment 2 — `discrepancy`'s source-arm cap is retired

D25 capped `discrepancy` source arms at five and required a recorded decision for a
sixth; D24 (supply side) spent the sixth on `expected_supply_id` while noting it
was "a subject standing in for an absent cause". Scan-failure aggregates want a
seventh.

**The cap was imported from the wrong rule.** `stock_movement`'s cause CHECK is
capped because causes are *distinct relationships that merely happen to be
exclusive* — D23's straining case. `discrepancy.source_*` is not that: it is **the
row this finding is most closely associated with**, seen through different types.
Two of six arms already being subjects is the symptom, not an anomaly.

Under D23's own test that is a **subject union with an optional none** — "none" is
meaningful (a negative balance has no single associated row), so `<= 1` stays, and
the arms grow with the subject set exactly as `observable` does.

> **The cap is removed. `discrepancy.source_*` grows with the subject set under
> D23's discriminated-union rule; `<= 1` is retained because a finding may be
> associated with no single row.**

`activity_event_id` joins as an arm. **Findings are raised per pattern, not per
attempt** — one `activity_event` per capture, and N failures at one supplier,
device or location within a window is what a human sees.

#### Amendments to earlier decisions

- **D17** — `activity_event` gains `tenant_id`, `site_id`, `instrument_device_id`
  and the identification columns; four resolution-failure kinds; `scan_ok`
  deleted; `client_event_id` corrected to a plain FK; the denominator claim
  narrowed.
- **D23 / principle 5** — canonical length is micrometres (amendment 1).
- **D24** — `package_event.source` gains **`keyed`**. Today a hand-keyed SSCC must
  be recorded as `operator_scan`, which is a false fact under D24's own *"the fact
  recorded is the fact observed"*. Manual keying is the only reliable proxy for an
  unreadable label, which is why GS1 mandates human-readable interpretation on
  logistic labels.
- **D25** — `activity_event` is range-partitioned on `occurred_at` from the first
  migration, with local indexes. **Retention on a fact table is partition DDL by
  the owning role, not a DELETE grant** — compatible with S6 rather than an
  exception to it, and written down before someone requests a grant or quietly
  stops recording. Source-arm cap retired (amendment 2).
- **D19** — `symbology` joins the shared reference set.
- **D27** — instrument specifications (aperture, wavelength) are device attributes
  with their own units, outside the observation canon.

**Rejects.** `scan_ok`, refused with the number. `no_read` as a kind —
structurally unpopulatable. Per-decode-attempt grain — bounded by our frame rate,
not by the world. A `scan_stat` counter table — it has **no role value** under
D25's axis: not reference, not policy, not grouping, and not a projection, because
a counter over scans not otherwise recorded has no source to rebuild from. It
would be the first unrebuildable maintained table in the model. A derived supplier
label-quality score — GS1's own verification template disclaims the inference in
both directions, and an inferred score has no author, which D21 rule 2 makes an
access-control boundary rather than metadata. Failure aggregates may **trigger** a
verification; the verification is the assertion of record. Barcode print-quality
grading as a by-product of picking — D27 instrument territory. `detail` as the
home for the scanned string: "which barcodes are failing, on which device" is the
entire point of the row, so it is queryable, so principle 3 promotes it to a
column.

### D29 — Nothing mints a package at receipt

*Adopted 2026-08-02 from [d24-open-questions.md](./d24-open-questions.md),
settling question 90.*

**Decision.** Goods arriving unlabelled land at a dock `location` —
`holder_location_id` set, `holder_package_id` NULL, zero `package` rows. A package
is minted on exactly **three triggers, all of them our acts**, and the identifier
for an unlabelled pallet is an **internal licence plate, never an SSCC**.

#### D24's minting rule contradicted its own default, and J19 could not see it

D24 says a package exists *"when something identifies it — an SSCC, a licence
plate, a scan"*, and four lines later that *"the default is never per-carton"*.
Those collide, and they collide on the shape Australian grocery **mandates**: an
ASN carrying SSCCs at the carton level, which is required whenever a pallet holds
more than one SKU. At a reference site that is 1,200 cartons/day — and D24's rule
as written fires per carton.

That is the cheap half. The expensive half:

> If a `package` is minted from `asserted_unit.sscc`, and stock is received into
> it, then `stock.holder_package_id` — **a component of the six-dimension `stock`
> key** — was determined by a counterparty's message.

D21 rule 3 forbids exactly that. **And J19 passes anyway.** Truncate every
assertion table: `assertion`, `asserted_unit` and `asserted_unit_content` go;
`package` is not an assertion table and survives; `package_event` is a *fact* with
`source = 'asn'` and survives; `stock` rebuilds byte-identical.

**This is the J8 pattern again.** The invariant tests the *values* after a
truncation, and the laundering happened in the *keys*, through an intermediate
table the truncation does not reach. A wrong invariant is worse than a missing
one — and this one was the register's confidence in rule 3.

**The fix is one word:**

> A `package` row exists when something **we observe** identifies it. A
> counterparty's claim about a logistic unit lives in `asserted_unit` and becomes
> a `package` only when someone scans it or we build it.

#### The argument stronger than cardinality, and the one that was wrong

**Per-carton identification at receipt is physically unobservable.** GS1 General
Specifications 4.4.2: on a nested pallet *"only the SSCC barcode of the higher
logistic unit SHOULD be readable. The SSCC barcodes of the lower level logistic
units should be obscured."* A receiver in front of a wrapped pallet **cannot** scan
the cartons, because the standard says the labels are covered. Minting per carton
would be minting packages nobody identified.

**A tenant may have no company prefix at all, and then the grain question does not
arise.** GS1 Australia's subscription entitlements mark *"Allocation & use of GS1
Company Prefix (eg: so you can create SSCC)"* as **not included** in the
Individual Barcode Number tier: turnover under $1M, one to ten GTINs, up to nine
more on application. Such a tenant **cannot form an SSCC at any grain** while
Woolworths, Coles and Metcash all mandate SSCC pallet labels.

That is why the internal-LP fallback below is a daily path rather than an edge
case, and it is a categorical limit rather than a budget: not a small serial
allowance, but none.

*(**Retracted on 2026-08-04, and the retraction is the point of question 123.**
This argument previously ran on serial capacity: a 12-digit prefix leaving four
serial digits, 100,000 lifetime SSCCs, a sustained ceiling of 274/day, carton
grain exhausting a namespace in four months. **GS1 Australia does not issue
12-digit prefixes.** Its SSCC fact sheet tabulates seven, eight, nine and ten
digits, and its membership FAQ sets the allocation by turnover: eight digits under
$50M, seven over. The smallest prefix a full member holds is ten digits, leaving
six serial digits, which is 1,000,000 per extension digit and 10,000,000 across
the ten, or roughly 27,400/day sustained under the twelve-month rule. Carton grain
at 1,200/day is 4.4% of that a year. The figure was wrong by two orders of
magnitude in the worst Australian case and four in the normal one, and it was the
claim this decision told the reader to rely on. The same arithmetic appears in
[d24-open-questions.md](./d24-open-questions.md), which this record supersedes.)*

**The conclusion is unchanged, and it never rested on this.** Per-carton minting
is refused because the cartons on a wrapped pallet cannot be observed, and because
minting from an assertion launders a counterparty's claim into the `stock` key,
which is the expensive half above. Serial capacity is not the constraint in
Australia. **Do not argue this on storage grounds, and do not argue it on serial
budget either.**

#### The three triggers

1. **We scanned a real label** — a supplier SSCC read off the pallet at the dock.
2. **We built the logistic unit** — re-palletising, consolidating loose cartons,
   rebuilding a broken pallet. GS1 4.4.1.2: *"the physical builder of the logistic
   unit or the brand owner is responsible for the allocation of the SSCC."*
3. **Putaway to a location that requires a holder** — the licence plate is a
   property of *where the goods land*, not of the goods or the receipt. The dock
   needs no package; a bulk rack gets one at putaway, which is the moment the
   pallet first becomes a thing anyone must address later.

**The identifier is an internal LP, not an SSCC.** Every property that makes an
SSCC expensive — a licensed prefix, a finite serial budget, a 12-month obligation,
an implicit assertion of authorship — exists to make it meaningful to *other
parties*. A pallet broken down into putaway locations before anything leaves the
site pays every cost and gets no benefit. **An SSCC is minted at the boundary:**
despatch, or re-palletisation into something that will ship.

The LP format must be **mechanically distinguishable from an SSCC in one regex** —
alpha-prefixed, never an 18-digit numeric. An internal plate with a coincidentally
valid check digit will eventually be transmitted on a despatch advice, and that
class of error is undetectable afterwards.

#### The issuing machinery, which did not exist

`package.sscc` (D6) had no issuer; `party` had no company prefix; there was no
number range.

```
number_range              -- REFERENCE. PLATFORM-OWNED (tenant_id IS NULL).
  id
  issuer_party_id         -- the LEGAL ENTITY holding the prefix (D20), not the tenant
  key                     -- 'sscc' | 'internal_lp'
  extension_digit         -- explicit row per digit; NO automatic rollover
  next_value, block_size  -- claimed under FOR UPDATE, handed out from process memory
  issued_through          -- high-water mark: serials consumed but never applied are
                          --   still evidenced against the reuse window
  exhausted_at            -- exhaustion raises a finding and FALLS BACK to internal
                          --   LPs. It never blocks a print. Same code path as
                          --   "tenant has no prefix" — one fallback, exercised daily.

sscc_allocation           -- FACT. Append-only.
  id, tenant_id, client_event_id
  issuer_party_id, extension_digit, gcp, gcp_length, serial_reference
  sscc CHAR(18) GENERATED -- CHAR, never bigint: leading zeros are significant
  issued_at
  UNIQUE (issuer_party_id, extension_digit, serial_reference)
```

#### D24's fan-out guarantee, stated honestly

Without a package, a pallet move is **not** the O(1) `package_event` D24 promises
— it is N `stock_movement` rows. So D24's guarantee does **not** hold at the dock
for unlabelled goods, which is the common case.

Trigger 3 is what recovers it: the pallet acquires an LP **at putaway**, so every
move after putaway is O(1). Only the dock→putaway move fans out, and that move is
a receipt, where fan-out at the system boundary is expected and correct (D24).

#### Amendments to earlier decisions

- **D24** — the minting rule gains "we observe"; the fan-out guarantee is scoped
  to post-putaway.
- **D21 / J19** — widened: truncate every assertion table, rebuild `stock`, assert
  byte-identical **and** assert that no column of the `stock` key is reachable
  from an assertion table by any path that survives truncation.
- **J34** *(new)* — no `package` row's earliest `package_event` has
  `source = 'asn'`. Minting from an assertion is structurally absent.
- **J6** *(extended)* — the `package_event` fold covers `sscc`, `barcode` and
  `identifier_kind`. Its enumerated fold omitted them, so relabelling drift passed
  the check written to catch it — a third bad invariant of the J8 shape.
- **D20** — `party` gains `gs1_company_prefix` and `gs1_prefix_length`.
- **J35** *(new)* — every `sscc_allocation` serial lies within its range's issued
  span, and no serial is reissued within the reuse window.

**Rejects.** Minting per carton from an ASN. Minting an SSCC for internal use.
An 18-digit numeric internal plate. Automatic extension-digit rollover on
exhaustion — it changes the first character of every SSCC we issue and downstream
systems pattern-match it. Blocking a print on range exhaustion.

### D30 — The reaper: one reference, and the predicate belongs to the rebuild

*Adopted 2026-08-02, settling question 91.*

**Decision.** "Unreferenced" is a closed, CI-asserted list of **exactly one
foreign key** — `stock_allocation.stock_id`, **in every state**, not the
enumerated live set. `stock.id` is a handle for the life of the cell, not an
archival key. The reaper runs weekly, off-peak, under the projection-maintainer
role, batched, with a grace period and a kill switch.

#### J3 is a quantity fold that reads as a reference test

`stock.allocated_quantity` folds only `{allocated, picking, picked, packed}`.
**Terminal allocations contribute nothing to it and still hold the foreign key.**
So the obvious cheap predicate — `quantity = 0 AND allocated_quantity = 0`, which
J3 makes look authoritative — **deletes rows that live foreign keys point at.**

Write it as an `EXISTS` over `stock_allocation` in **any** state, and record that
**J3 must never be used as a reference test.** Fourth bad invariant of the J8
shape.

**The reaper as adopted was also a near no-op.** A `fulfilled` allocation against
a now-zero cell **is the normal end of every pick**, so under a literal reading of
"not deleted while referenced" every cell that ever served a pick is pinned
forever.

There is no referential action that both reaps and keeps the allocation: RESTRICT
blocks, CASCADE destroys fulfilment history, and SET NULL violates
`CHECK (num_nonnulls(stock_id, expected_supply_id) = 1)`. Relaxing that CHECK is
refused twice over — by D23's rule and by D24 (supply side) amendment 2, which
dropped exactly that scoping. So: **RESTRICT, declared, with the reaper narrowed
to match.**

#### `stock.id` is a handle, not an archival key

**The model has already answered this three times without writing it down.**
`stock_count` and `discrepancy` carry the full cell key *column set* rather than an
FK. `observable` (D23) deliberately excludes stock cells and says why —
*"D24 gives `stock` a surrogate id that would make it tempting."* `stock_movement`
carries `from_*`/`to_*` pairs, never a `stock_id`.

> **`stock.id` is a current-state handle, not an archival key. History is
> `stock_movement`.**

That collapses the "historical reporting joins `stock.id`" worry into a rule the
model already obeys — and it is why `outbox.source_id` is safe: **S27 is doing
load-bearing work for the reaper that nobody wrote it for.**

**Three referencers the register did not cover:**

1. **D26's schema compiler.** `record_scheme_field.field_type = 'ref'` generates a
   real FK with `ON DELETE RESTRICT`, and D26 derives `attaches_to` from the code
   registry — which contains `stock`. So **a tenant could declare a scheme that
   creates a durable FK to `stock.id` at runtime**, disabling a platform
   invariant with no privilege required, surfacing as a maintenance job erroring.
2. **`package_content` is a view exposing `stock.id`.** Any export or `ref_entity`
   naming it persists a reapable surrogate.
3. **`projection_check.scope_kind`/`scope_id`** (D25) is a polymorphic pair on a
   fact with no DELETE grant — scope a check to a cell and it holds a `stock_id`
   forever, uncatchable by any FK.

**S2 licensed the bug.** *"Every table naming a stock cell carries the whole key —
FK to `stock.id` **or** the complete column set."* Under a reapable `stock` those
are not equivalent: the column set survives the row's death and the FK does not.
**The disjunction is removed.**

#### The rebuild collision

J1 asserts `stock.quantity` equals the fold of `stock_movement` over the cell key.
A reaped cell folds to zero and has no row — so unless the rebuild's definition of
*which cells exist* excludes them, **every reap cycle emits `projection_drift`**
and D8's queue fills with noise the model generates about itself.

> A cell exists iff `quantity <> 0 OR weight_g <> 0 OR allocated_quantity <> 0 OR
> referenced`.

`weight_g` is easy to miss and matters: J2 folds `catch_weight_g` independently, so
a cell can reach `quantity = 0` with `weight_g <> 0`. That is a catch-weight
capture bug, and it is exactly the evidence a quantity-only reaper destroys while
the rebuild resurrects the row.

#### The honest benefit

D24 amendment 3 claimed reaping *"removes the unbounded-growth concern"*. **That
is wrong.**

- The availability index is already partial (`WHERE quantity <> 0`), so dead cells
  are already invisible to the read path.
- The `UNIQUE NULLS NOT DISTINCT` arbiter index **cannot** be partial — it must
  find a zero cell to resurrect it — so it carries every cell that ever existed.
  Reaping trims about **one B-tree level**: roughly one page access per upsert.
- At 5,000 picks/day, twelve months unreaped is ~730k dead rows, ~330 MB/year/site.
  The ratio is the argument, not the megabytes.
- **The real cost on `stock` is non-HOT UPDATE churn, and reaping does not touch
  it.** `available_quantity` is `GENERATED STORED` and sits in the availability
  index's `INCLUDE`, so every quantity change and every allocation state
  transition writes new index tuples.

Restated: **reaping bounds the arbiter index's page count.** It does not solve
growth.

#### Mechanics the natural implementation gets wrong

- **The `DELETE` repeats the full predicate.** Under READ COMMITTED,
  `DELETE ... WHERE id = ANY($1)` deletes a cell resurrected between the SELECT
  and the DELETE. Batch by id **and** predicate.
- **`stock_allocation(stock_id)` plain btree must exist first.** Postgres does not
  index the referencing side of a foreign key; without it each delete fires an RI
  trigger that sequentially scans the allocation table.
- **Grace period.** `stock` gains `last_movement_at` (`@projection`) with a
  candidate index. A cycle-count wave revisits a bin a week later; measure grace
  in days.
- **Per-table autovacuum** in the migration that creates `stock`, justified by
  UPDATE churn rather than by the reaper. A self-hosted deployment (D18) has no
  DBA to set it.
- The reap-versus-resurrect race is **not** a problem: `ON CONFLICT DO UPDATE`
  guarantees insert-or-update against a concurrent delete. The reaper cannot make
  a receipt fail.

#### Amendments to earlier decisions

- **D24** — amendment 3's premise corrected; predicate stated as an `EXISTS` over
  every allocation state; benefit restated. `stock` gains `last_movement_at`.
  `stock.id` is documented as *"a handle for the life of the cell. NOT durable:
  reissued if the cell is reaped and returns."*
- **D12 / D24** — `stock_allocation.stock_id` declared `ON DELETE RESTRICT` with a
  plain btree.
- **D25** — **DELETE revoked on projection tables** from the app role. S5 covered
  UPDATE only and S6 covered fact tables only, so *"nothing writes to `stock`
  directly, ever"* was unenforced against DELETE. `projection_check.scope_kind`
  may not name a stock cell.
- **D26** — `stock` and `package_content` carry neither the attachable nor the
  subscribable capability flag.
- **S2** *(corrected)* — the disjunction removed.
- **J33** *(new)* — rebuilding `stock` preserves row identity; truncate-and-
  regenerate is forbidden while any allocation holds a `stock_id`. **This is
  J30's missing analogue** — D24 (supply side) forbade it for `expected_supply`
  because live allocations hold those ids, and `stock` has the same exposure under
  the same CHECK on the same table.
- **J32** *(new)* — the reap predicate is the complement of the rebuild's
  existence predicate: reap, rebuild, assert produces zero `projection_drift`.
- **q102** — the reaper runs as the projection maintainer, so q102 blocks it.

**Rejects.** Reaping on `allocated_quantity = 0`. Relaxing the `= 1` CHECK to
permit SET NULL; CASCADE. Loose foreign keys with a deletion queue and worker — it
deletes the guard that stops a caller naming a cell that never existed. A
`stock_reaped` tombstone — storage to record having freed storage, recreating the
unbounded table the reap exists to prevent; extractors get the natural key, never
the surrogate. `deleted_at` soft delete, which is not a reap. Never-reap plus
periodic `REINDEX CONCURRENTLY` — attractive on the numbers, refused on D5,
because a concurrent reindex of a unique index can make `INSERT ... ON CONFLICT`
fail with a spurious unique violation, and on `stock`'s arbiter index that is a
receipt scan being rejected. Deterministic `stock.id` (UUIDv5 over the key) —
considered seriously, unnecessary once nothing durable holds the id, and it puts a
random-key B-tree on the hottest table in the system.

### D31 — Retention: floors are declared, and nothing that folds is ever deleted

*Adopted 2026-08-03, settling questions 101, 113 and 115 together.*

**Decision.** Retention has exactly two drivers: **verifiability**, which sets a
hard floor nothing may cross, and **claim windows**, which set the minimum age for
anything that can still be argued about. **Value decay is rejected as a driver.**
Ageing data out is done by **archiving, never deleting**. Every floor is a
declared row with a named authority, and CI asserts the data actually reaches it.

#### Why value decay is rejected

The intuitive model is that detail matters while something is in motion and fades
into summary afterwards, the way memory does. It is seductive and it is wrong for
an audit trail, for one reason:

> **Significance is determined retrospectively.** Nothing is knowable as noise at
> the time it is written. An event becomes evidence when a dispute surfaces, and
> that can be months later.

The memory analogy fails precisely where it matters. Human memory is lossy and we
accept that. A chargeback dispute needs exactly the detail that looked like noise
when it was recorded.

#### The verifiability floor

Facts divide into two classes by whether anything folds from them.

**Facts that fold to a projection** — the movement ledger, observations, container
placement — **are never deleted.** The check that makes stock trustworthy is that
the stored total equals the fold of the whole ledger, and deleting any of it means
that check can no longer run. This is not a retention policy so much as a
consequence of the model: the log is the only truth, and projections are caches
that may be discarded and refolded at any time.

**Facts that fold to nothing** — scan failures, activity, work that moved no stock
— may age out, but see the archive rule below.

#### Archive, never drop

*(Adopted from the substrate work in the `timespace` project, whose law 8 states
compaction as freeze-with-tested-unfreeze and archives a segment only once
everything in it is snapshot-covered.)*

D28 range-partitions `activity_event` on time, with the implication that old
partitions are dropped. **Dropping is deletion, and question 115 named the failure
mode: it is silent.** A fold invariant detects source deletion because the
projection stops matching. An **existence** predicate — has this identifier been
used in the last twelve months, has this submission been seen before — has no
projection to compare against, so after a truncation both sides agree and nothing
fails.

So a partition is **detached and archived**, not dropped. It reopens cold and
transparently. The failure mode of archiving is slowness; the failure mode of
dropping is a check that quietly starts passing.

#### Retention floors are declared facts with an authority

*(Settling question 115.)*

```
retention_floor               -- REFERENCE
  id, tenant_id               -- NULL = applies to all
  subject                     -- the table or check the floor protects
  minimum_age                 -- interval
  basis                       -- statutory | standard | contractual | operational
  authority                   -- the instrument it comes from, named
  established_at, established_by_id, note
```

The floors known from the inbound research, each with the instrument behind it:

| Subject | Minimum age | Basis |
|---|---|---|
| Identifier reuse guard | 12 months | GS1 General Specifications, SSCC non-reallocation |
| Receipt and discrepancy evidence | 30 days minimum | Food and Grocery Code of Conduct, claim window |
| Pallet account movements | 180 days | Carrier equipment control policy, liability window |
| Carrier charge disputes | 12 months | Quarterly dispute cycles, four quarters of cover |

**[Unverified and probably the binding one: Australian business record retention
under tax and corporations law, commonly five years. Needs legal confirmation
before any floor is set below it.]**

**The assertion that makes a floor real**: for every declared floor, either the
oldest live row in the subject reaches `minimum_age`, or the archive manifest
covers back to it. A partition detached below the floor fails the check loudly
rather than degrading a guard nobody is watching.

#### `client_event` retention is derived, not chosen

*(Settling questions 101 and 113.)*

Every fact carries `client_event_id`. The row it points at holds the person, the
device, the session, the app version and both clocks, which is exactly the material
D8 and D11 depend on for an investigation. **Deleting it while keeping the fact
would leave the record intact and gut the ability to investigate it.**

So the retention of `client_event` is not an independent decision. **It lives as
long as the longest-lived fact that references it**, which for the ledger is
indefinitely.

#### The premise of question 101 was wrong

D25 asserted that `client_event` can never be partitioned, because Postgres
requires the partition key inside any unique constraint, so partitioning by time
would degrade a global uniqueness guarantee into a per-partition one and let a
replay landing in a different month through.

That holds only when the partition key is **independent** of the identifier. It is
not a property of the table.

> **Derive the partition key from the identifier and a replay routes to the
> partition its original landed in, so uniqueness within the partition is globally
> sufficient.**

With a time-ordered identifier (UUIDv7, already the recommended scheme), the
server computes the bucket from the id itself and stores it as a plain column. The
key becomes `(tenant_id, bucket, client_event_id)`. A replay carries the same id,
therefore the same bucket, therefore the same partition, and the conflict is
caught. Two distinct submissions can never collide because their identifiers
differ.

The bucket is computed server-side from the identifier, never supplied by the
client.

#### The volume does not justify urgency

With `scan_ok` deleted (D28), `client_event` takes one row per fact-producing
submission rather than per capture: on the order of 10,000 a day per site. The row
is narrow.

| | |
|---|---|
| Rows per year per site | ~3.7M |
| Storage per year per site, with indexes | ~500 MB |
| Ten years, twenty sites | ~100 GB |

That is unremarkable for Postgres. **The honest answer to question 101 is that
`client_event` is retained indefinitely, it is partitionable if that ever becomes
useful, and the question was more urgent in the asking than in the answering.**

#### Snapshot membership is by identity, never by time

*(Adopted pre-emptively from the same substrate work, whose law 3 is marked as a
correction, implying it was learned the hard way.)*

Nothing is snapshotted today, so this costs nothing to write down now and would be
expensive to discover later.

> When a segment of the ledger is folded into a snapshot, the remainder is defined
> as **the operations the snapshot does not contain**, never as *"everything after
> time T"*.

A time-keyed cut loses any movement that arrives late but is dated before the cut.
The model is built to make exactly that case correct: D5 orders the ledger by
device clock so a late scan lands in its true position, and D9 computes count
variance against ledger state at the moment of counting rather than at write time.
The first time-keyed snapshot would quietly undo both.

**And when a snapshot exists, it joins the trusted base.** Today nothing is trusted
except the log. Afterwards a corrupted snapshot is invisible to the mechanism built
to detect corruption, which is an acceptable trade if it is chosen rather than
arrived at.

#### Amendments to earlier decisions

- **D25** — the claim that `client_event` can never be partitioned is withdrawn;
  it holds only for a partition key independent of the identifier.
- **D28** — `activity_event` partitions are **archived, not dropped**.
- **J-series** — a new assertion per declared `retention_floor`, checking the
  oldest live row or the archive manifest reaches it.

**Rejects.** Value decay as a retention driver. Dropping partitions on a table any
existence check reads. Deleting `client_event` rows while referencing facts
survive. Choosing an arbitrary retention window in place of a derived one.
Time-keyed snapshot cuts.

### D32 — One `party` for identity, roles as relationships

*Adopted 2026-08-03, settling question 57 and answering question 53.*

**Decision.** One `party` table holding a company's identity, and a `party_role`
table saying what that company is **to us**. Role-specific operational data hangs
off the role, not the party. Every foreign key meaning "a company" points at
`party.id`, a single column with no arms.

#### The `kind` column is a shape this model already refused

`party(id, kind, …)` with `kind ∈ {customer, supplier, carrier, legal_entity}` is
the same construction D16 rejected for `order`: *"adding a kind and a nullable
customer to `order` would be the generic document model already on the
deliberately-not-building list."* The objection was that a discriminator column
forces every consumer to carry a predicate the database cannot help with, and that
two honest tables beat one apologetic one.

Here it fails for a sharper reason.

#### Roles are not exclusive, so neither a discriminator nor separate tables works

D23's rule says typed alternatives are correct when the arms are **alternative
identities of one referent**, where exactly one applies and "none" is meaningless.
A party's roles are not that. **A company can hold several at once**, so this is a
set, not a union.

Both of the obvious shapes break on it in the same way. A `kind` column means a
company that is both customer and supplier gets two rows, therefore two
identities, and "are these the same company" becomes unanswerable. Separate
`customer` / `supplier` / `carrier` tables have the identical problem with more
DDL, and additionally force four typed arms onto every table that references a
counterparty.

**The overlap is not hypothetical, and the clearest case is already in the
model.** A carrier invoices us. D31's freight-cost goal is to compare what a
carrier charged against what was predicted, which makes a carrier's invoice a
supplier document from a company that, under a discriminator, is not a supplier.
Swift is a carrier when it moves a pallet and a supplier when it bills for it, and
it is one company throughout.

```
party                         -- REFERENCE. Identity. Intrinsic (D19).
  id, tenant_id               -- NULL = shared across tenants
  name, trading_name
  abn                         -- Australian Business Number
  gln                         -- GS1 Global Location Number
  gs1_company_prefix, gs1_prefix_length      -- D29, for identifier issuance
  active

party_role                    -- REFERENCE. What they are TO US. Operational.
  id, tenant_id (NOT NULL), party_id
  role                        -- customer | supplier | carrier | freight_provider
                              -- | legal_entity | pool_provider | calibrator
  owner_party_id              -- whose supplier is this? NULL = ours (D20's 3PL)
  account_reference           -- our account with them, or theirs with us
  established_at, ended_at    -- a monotone lifecycle; timestamps are its log (D25)
  UNIQUE (tenant_id, party_id, role, owner_party_id)
```

**Role-specific data hangs off `party_role`, never off `party`.** A company that is
both supplier and carrier has two role rows, each carrying its own operational
detail: `carrier_profile` (D22's despatch scalars) attaches to the carrier role,
supplier capability such as whether they send advices attaches to the supplier
role. Hanging both off the party would produce one sparse table where most columns
are null for most rows, which is the attribute-soup shape principle 4 refuses.

**`owner_party_id` is what makes the role relative.** D20 admitted third-party
stock, and a 3PL client has their own suppliers whose goods arrive at our dock.
That party is a supplier *to the client*, not to us. One nullable column expresses
it, defaults to NULL meaning ours, and an operation that never holds another
company's stock never encounters it. D20's own pattern.

#### D19's split falls out for the third time, which answers question 53

Question 53 asked whether carriers and package types are shared reference data
too, and noted that if the intrinsic-versus-operational split applied again it
would be *"a good sign the split is real rather than fitted to items"*.

It applies exactly. A company's name, ABN, GLN and GS1 prefix are facts about the
company that are true for everyone, so `party` is shareable with `tenant_id NULL`.
What that company is to a given tenant, the account number, the despatch rules,
whether they send advices, is observed and operational, so `party_role` is
tenant-scoped and never shared.

**Third independent case, none of them fitted to the others.** The split is real.

#### What this changes in the freight model

D1 introduced `carrier` and `freight_provider` as separate tables. A carrier is a
company, so `carrier` collapses into `party` plus a carrier role. `carrier_service`
survives unchanged, because a service offering is not a company.

`freight_provider` survives as a **route**, which was always D1's point, and now
names the company at the other end of it:

```
freight_provider              -- HOW a carrier is reached
  id, tenant_id
  party_id                    -- the intermediary; NULL when reached directly
  kind                        -- aggregator | direct
  ... credentials, endpoint ...
```

D1's whole argument is preserved and gets sharper. Swift reached through an
aggregator and Swift reached directly are the same carrier because they are the
same `party`, and the aggregator is itself a party we hold a relationship with.

#### The consequence that matters

Every reference to a company is one column. `discrepancy.counterparty_party_id`,
`observation_event.asserted_by_party_id`, `assertion.author_party_id`,
`stock.owner_id`, `site.legal_entity_id`, `purchase_order.supplier_party_id` and
`consignment.carrier_party_id` all point at `party.id`.

Under separate tables each of those would need four typed arms and a CHECK, on
seven tables. Under a discriminator each would point at a row whose meaning
depends on a column the database cannot constrain. **One identity per company is
what makes a supplier scorecard, a carrier cost history and a counterparty finding
join to each other at all.**

#### Amendments to earlier decisions

- **D1** — `carrier` collapses into `party` plus a carrier role;
  `freight_provider` gains `party_id` and keeps its routing meaning.
- **D20** — `party` loses its `kind` column; roles move to `party_role`. The
  `legal_entity` axis is unchanged, and `site.legal_entity_id` now points at a
  party holding that role.
- **D19** — extended to parties, which is its third independent application.
- **D22** — `carrier_profile` attaches to the carrier `party_role`, not to a
  separate carrier table.
- **D29** — the GS1 company prefix sits on `party`, where identifier issuance
  already assumed it.

**Rejects.** A `kind` discriminator on `party`, refused on D16's reasoning and on
the fact that roles are not exclusive. Separate `customer` / `supplier` /
`carrier` tables, which multiply the identity problem and force four typed arms
onto seven tables. A single `party_profile` carrying every role's operational
columns, which is sparse by construction. Deriving a role from the existence of
related rows, such as treating any party with a purchase order as a supplier,
which cannot express a supplier we have not yet ordered from.

### D33 — Two confirmations before the first migration

*Adopted 2026-08-03, settling questions 31 and 44. Both had a stated lean; both
leans hold, and one holds for a different reason than the one given.*

#### Lot tracking is enforced by a finding, never by a rejection

A CHECK cannot reach from `stock_movement.lot_id` to `item.tracking`, so a
movement for a lot-tracked item that carries no lot cannot be forbidden by the
database in the ordinary way. D23 leaned toward application validation plus a
periodic assertion on the grounds that it matches how `stock` is already
reconciled. That is true and it is not the reason.

**The reason is D5.** A trigger that rejects the insert blocks the floor to
protect a data rule. Picture the case: a picker scans an item whose lot label is
damaged or missing. The goods are real, the pick happened, and a trigger would
refuse to record it. That is the exact trade D5 exists to refuse, and refusing it
here is worse than usual because the discarded record is the one a recall would
have needed.

So enforcement runs the same way every other impossible state does:

1. **Challenged at capture** (D9). The handheld knows the item is lot-tracked and
   asks for the lot while the operator is standing at the shelf and can look
   again. This is where almost all of them get caught.
2. **Accepted if confirmed.** An operator who cannot read the label records the
   movement without one. The goods moved.
3. **Raised as a finding.** `discrepancy.kind = 'lot_missing'`, carrying the
   person, the device and the timestamp, so somebody can go and look at the pallet
   while it is still where it was put.

A trigger would have produced a rejection, which loses the event. A finding
produces an investigation, which is the thing that recovers the lot.

**One detail this exposed.** `item.tracking` is mutable, because an item can start
being lot-tracked. Historical movements made before that change are still valid
and must not fail the assertion. So `item` gains `tracking_effective_from`, and the
assertion only considers movements at or after it. Without that column, switching
an item to lot-tracked would retroactively flag every movement it ever had.

#### `activity_event.kind` is an enum, and the model already has the test

D28 leaned enum. Confirmed, and the general rule is worth stating because there
are now three instances pointing at it.

> A value set is a **table** when it carries attributes and grows independently of
> the code that reads it. It is an **enum** when code branches on it, because then
> the set is closed by the code that handles it, and adding a value without adding
> handling is a bug rather than a configuration.

| Set | Shape | Why |
|---|---|---|
| `metric` (D23) | table | Carries a result kind, a dimension, a unit; tenants may define their own |
| `symbology` (D28) | table | Scanner platforms ship around sixty label types and add more with firmware |
| `activity_event.kind` | **enum** | Every value exists because code does something different with it |
| `discrepancy.kind` | **enum** | Same: each kind routes differently |

D23 had already drawn the line in passing, when it refused a `metric` enum
*"contrast question 44's `activity_event.kind`, where code branches and an enum is
honest"*. This confirms it and makes the test explicit rather than a remark.

**The escape hatch already exists**, which is what makes the refusal safe. D28
rejected a kind table on the grounds that it *"invites per-site custom kinds,
which is a small step toward the rules-engine-by-accretion D13 warned about"*. A
tenant that needs to record something the enum does not cover declares a
`record_scheme` (D26) and gets a real typed table for it. That is the sanctioned
path, and it does not require loosening a discriminator the application branches
on.

#### Amendments to earlier decisions

- **D14 / D23** — `item` gains `tracking_effective_from`; the lot-tracking
  assertion is scoped to movements at or after it.
- **D8** — `discrepancy.kind += lot_missing`.
- **D23** — the table-versus-enum test is stated as a rule rather than left as an
  aside.

**Rejects.** A trigger enforcing lot presence, refused on D5. Denormalising
`item.tracking` onto the movement with a composite foreign key: the idiom is
sound and D23 uses it, but the foreign key would forbid ever changing an item's
tracking, which is a legitimate operation. A kind table for `activity_event`.
Deriving tracking obligations from the presence of related rows.

---

### D34 — `item_barcode`, and one identifier is not the way it was printed

*Adopted 2026-08-03, settling question 116. Referenced twice by D19 and defined
by no decision until now, which is the position `device` was in before D27. It
blocks the first migration and every scan.*

#### The scheme is what the identifier is, not how it reached the scanner

The inbound sketch proposed `kind` as `gtin13 | gtin14 | itf14 | internal |
supplier_ref`. Two axes are collapsed there and both collapses cause a defect.

ITF-14 is a **symbology**, one of several ways to print a GTIN-14, and
`symbology` is already a table (D24, from Q89). Putting it in the identity column
is `package.sscc` again in a different place: a carton read from an ITF-14 and the
same carton read from the GS1-128 beside it would resolve through different rows.

GTIN-13 and GTIN-14 are not two schemes either. A GTIN-13 right-justified to 14
with indicator digit 0 **is** the same trade item, which is why GS1's own storage
guidance is to hold every GTIN in 14 characters. Indicators 1 to 8 denote higher
packaging levels and are genuinely different trade items; indicator 0 is the
same one. Store the unnormalised form and `9312345678907` scanned from an EAN-13
and `09312345678907` scanned from AI 01 on the carton are two rows for one
product, so the carton scan misses. That is the most common integration defect in
this area and it is silent.

So `scheme` is `gtin | internal | supplier_reference`, GTINs are normalised to 14
on write, and how a barcode was printed is recorded on the scan rather than on
the identity. The raw string as read already has a home on `activity_event`
(D24).

#### One constraint does three jobs, and the obvious spelling of it does none

```
item_barcode                  -- REFERENCE (D19 shape)
  id, tenant_id               -- NULL = shared catalogue
  item_id
  issuer_party_id             -- NULL = the brand owner or us
  barcode                     -- fixed-width text, GTINs normalised to 14.
                              --   Never numeric: leading zeros are significant.
  scheme                      -- gtin | internal | supplier_reference
  unit_id                     -- D23's unit vocabulary
  quantity                    -- base units per scan of this barcode.
                              --   NULL = variable measure; the AI carries it.
  effective  daterange        -- NOT NULL, DEFAULT daterange(CURRENT_DATE, NULL)

  CHECK (quantity IS NOT NULL OR scheme = 'gtin')
  EXCLUDE USING gist (
    COALESCE(tenant_id, '00000000-0000-0000-0000-000000000000'::uuid) WITH =,
    COALESCE(issuer_party_id, '00000000-0000-0000-0000-000000000000'::uuid) WITH =,
    barcode WITH =,
    effective WITH &&
  )
  INDEX (barcode)             -- the scan-resolution path
```

`active boolean` with `UNIQUE (tenant_id, barcode) WHERE active` is the shape
this wants to be written in, and it enforces nothing across the shared
catalogue. `tenant_id` is NULL for every shared row, a NULL comparison yields
NULL rather than true, and a unique index therefore permits any number of
duplicate shared barcodes. **`UNIQUE NULLS NOT DISTINCT` fixes that for a unique
index and has no equivalent for an exclusion constraint**, so the sentinel
`COALESCE` is load-bearing rather than stylistic. It carries a schema comment
saying so, because the first reviewer to read it will try to simplify it back
into the hole.

Written as an exclusion over a range instead, one constraint gives three things:

1. A barcode resolves to at most one item per scope **at any instant**.
2. Deactivation closes the range rather than flipping a boolean, so a scan
   recorded in March is still explainable in September. Under D31 nothing here is
   deleted anyway, and this is the shape that makes the retained row useful
   rather than merely present.
3. A barcode that legitimately rebinds does so as a later non-overlapping range,
   which the same constraint permits while continuing to forbid the overlap. A
   supplier reusing its own carton code for a different product, a tenant-scoped
   row superseding a shared one, an internal code retired and later reissued: all
   three are real, and none is expressible by a boolean.

*(**Corrected on 2026-08-04.** Point 3 previously read "GTIN reuse after GS1's
waiting period". **There has been no general GTIN reuse waiting period since 1
January 2019**, when the GTIN Management Standard eliminated reuse in every
sector: a GTIN allocated to a trade item is not reallocated to another. The only
surviving exception is narrow, for an item that was allocated a GTIN and never
produced, reusable twelve months after deletion from the catalogue. The
constraint shape is unchanged and the range is still right, for the reasons now
stated; the example leading it was the one case that no longer happens. It has one
consequence still to be picked up: whenever a GTIN issuer is built, its non-reuse
obligation is **permanent**, so the `retention_floor` it needs has no expiry and
S34's history-depth companion cannot be satisfied by any finite window. D31's
twelve-month identifier floor is the SSCC's and stays as written.)*

The idiom is already named in the model for `%_policy.effective`,
`package_containment.valid` and `order_tolerance_band.quantity_range`, so this is
a fourth line in an existing CI assertion template rather than a new mechanism.

#### Supplier carton codes collide, and the scoping is one nullable column

A supplier's own code on a carton is not globally unique and two suppliers will
eventually use the same string for different products. As a bare row in a table
keyed on the barcode alone, `supplier_reference` is wrong the first time that
happens, and wrong by resolving confidently.

`issuer_party_id` scopes it. NULL means the brand owner issued it (a GTIN) or we
did (an internal code); set means it is this party's code and means nothing
outside that. Receiving passes the expected supplier from the advice. Without
one, the scan is ambiguous by construction, which is already a recorded outcome:
D24's `identifier_ambiguous`. This is the one addition beyond the inbound sketch,
and it is a nullable foreign key in an exclusion key that already exists.

#### Assertions may not write reference data

`item_barcode` is reference, and D19's two RLS policy shapes apply unchanged.
A GS1 National Product Catalogue feed, a supplier price file or a wholesaler's
catalogue export is an **assertion** under D21, not reference data. It lands as
an assertion and something with a name promotes it.

If a feed writes here directly, a supplier silently rewrites what a scan means.
That is the poisoning D19 exists to prevent, one level below measurements and
with a worse blast radius, because a wrong measurement produces a bad autofill
and a wrong barcode binding produces stock movements against the wrong item. The
pressure to auto-ingest the National Product Catalogue will arrive early and this
refuses it once.

#### D19's missing sentence

A carton GTIN's **level** is intrinsic, assigned by the brand owner. The **count
it implies** depends on the case pack a given tenant receives, which is D19's own
reason `item_packing_config` is tenant-scoped.

A shared row therefore carries the brand owner's declared unit and quantity. A
tenant receiving a different case pack writes a tenant-scoped row for the same
barcode and it wins, resolved by `ORDER BY tenant_id NULLS LAST LIMIT 1`.
Deterministic, and never ambiguous between the two scopes. A tenant row pointing
at a **different item** is also legitimate, for a private-label variant or a
supplier reusing a code, and resolves the same way.

#### Resolution is a function with three arms in a fixed order

The three identifier surfaces are `package_identifier` (D24, a projection of
`package_event`), `item_barcode` (reference, this decision) and `location`. They
stay three tables because they sit in different provenance categories: a
package's identity is something that happened at a place and time, and an item's
barcode is a durable fact about a product class. One table for both would put a
projection and a reference row under one primary key.

Resolution is nonetheless **one function**, because the scanner does not know
which it is holding:

1. **Parse.** A GS1-128 or DataMatrix string yields application identifiers: 00
   is an SSCC, 01 is a GTIN, 10 is a batch, 17 an expiry, 21 a serial, 310n a
   weight. A bare numeric string with a valid mod-10 check digit is a GTIN by
   length. Anything else is passed through raw. The parser is the vendor
   `gs1-syntax-engine`, per the inbound finding, and unrecognised AIs are stored
   opaquely and never rejected.
2. **Dispatch.** AI 00 to `package_identifier`, AI 01 to `item_barcode`, a raw
   string to all three.
3. **Narrow.** The screen supplies `expected_entity_kind`, which already exists
   on `activity_event`. Zero hits is `identifier_unknown` when well-formed and
   `identifier_unrecognised` otherwise. More than one surviving hit is
   `identifier_ambiguous`. All four kinds are D24's and none is new.

**One scan yields item, lot and expiry.** A GS1-128 carton label carries AI 01,
AI 10 and AI 17 in a single string, so resolution returns the binding and the lot
attributes together. Receiving stops keying batch numbers and expiry dates, which
is most of the operational value of doing this properly, and it is what makes
D33's lot-tracking assertion cheap to satisfy rather than a burden on the dock.

**Variable measure is why `quantity` is nullable.** A variable-measure GTIN
carries the weight in the barcode under AI 310n rather than in a table, so the
table cannot hold a fixed count. A NULL `quantity` means the scan supplies it,
and the CHECK confines that to GTINs because no other scheme has a place to carry
one.

#### Amendments to earlier decisions

- **D19** — the missing sentence stated: shared rows carry the brand owner's
  declared unit and quantity, tenant-scoped rows win, resolution is
  `tenant_id NULLS LAST LIMIT 1`. `item_barcode` joins the reference set formally
  rather than by mention.
- **D21** — reference data may not be written by an assertion. Stated as a rule
  here because the National Product Catalogue is the first thing that will try.
- **D23** — `item_barcode.unit_id` is the named consumer, replacing the
  `unit_level` enum in the inbound sketch. The unit vocabulary carries packaging
  levels and measures in one table, as already recorded.
- **D24** — the four identification kinds are reused unchanged. `expected_entity_kind`
  gains no values. The resolver is named as the single consumer of all three
  identifier surfaces.
- **D25** — `item_barcode` is reference, so it takes UPDATE and DELETE grants on
  the application role like the rest of the reference set. `effective` closing is
  an UPDATE, not a DELETE, and D31's retention floors apply to the closed row.
- **D31** — `item_barcode` is retained indefinitely. A closed range is the
  evidence for what a historical scan meant, and truncating it makes historical
  resolution quietly start returning nothing.
- **J42** *(new)* — every GTIN in `item_barcode` is 14 characters and passes the
  mod-10 check digit. Asserted over the table, not at the call site, because the
  normalisation happens on write and a second write path will be added.
- **J43** *(new)* — the exclusion constraint's `COALESCE` sentinels are present.
  Asserted from `pg_constraint`, because the failure mode of their removal is
  that duplicate shared barcodes become insertable and nothing complains.

**Rejects.** One identifier table for packages, items and locations: it puts a
projection and a reference row under one key and would make `package_identifier`
directly writable, which D24 refused by name. `active boolean`: it cannot express
a rebinding as a later range, it cannot explain a historical scan, and its unique
index does not constrain the shared catalogue. A `kind` enum carrying symbologies.
Storing the raw scanned string here rather than on `activity_event`, which would
duplicate D24's home for it and split the diagnostic query in two. Auto-promotion
of National Product Catalogue rows into reference data. A `gtin13` column beside
a `gtin14` column, which is the unnormalised defect written down deliberately.

---

### D35 — The projection maintainer is a set of named functions, not a role

*Adopted 2026-08-03, settling questions 102 and 117. D26 answered this for the
schema compiler through ownership; the projection maintainer was left open, and
D30's reaper has been blocked on it since.*

#### A role is ambient privilege, a function is privilege attached to code

The model says *"nothing writes to `stock` directly, ever"* and D25 makes that a
grant rather than a sentence. Something must still write it, so the guard is
deliberately open in one place, and the question is what shape that opening has.

Granting `UPDATE` on `stock` to a `projection_maintainer` **role** reduces the
rule to *"nothing writes to `stock` except this role, which does five jobs."*
Role privilege is ambient: anything running in a session with that role can write
any value to any projection column, for any reason, and the audit can only ever
say that the role did it. "Who may write a projection" has no better answer than
a role name.

Attach the privilege to code instead and the answer becomes a list of names:

> **No login role holds a write grant on any projection column. The grants are
> held by `projection_owner`, a `NOLOGIN` role with no members, and every write
> happens inside a `SECURITY DEFINER` function it owns.**

Nobody can `SET ROLE` to it and no connection string can be it, so the only way
to exercise the privilege is to call one of the functions, and each function body
is the entire surface of what that privilege can do. `stock` is written by
`projection_stock_apply()` and `projection_stock_rebuild()` and by nothing else,
and that sentence is verifiable from `pg_proc` rather than asserted.

This is D26's move one level across. Ownership made schema drift
*unrepresentable* rather than detected; function-scoped privilege makes *"someone
with maintainer access ran an UPDATE"* unrepresentable rather than audited.

#### Three escalations Postgres enables by default, all one line to close

The opening is only as narrow as the function definitions, and Postgres's
defaults widen all three of them silently.

**`EXECUTE` is granted to `PUBLIC` on every new function.** A `SECURITY DEFINER`
projection writer is therefore callable by the application role, directly, with
hand-made arguments. That is precisely *"the way around every other rule here"*
this question was raised about, and it is on by default. Every function in the
set carries `REVOKE EXECUTE ... FROM PUBLIC` in the same migration that creates
it, and `EXECUTE` is then granted only to the scheduler role and, for the rebuild
entry points, a platform role.

**A mutable `search_path` in a `SECURITY DEFINER` function is remote code
execution as the owner.** A caller who can create objects in any schema earlier
in the path shadows an unqualified name, and their function body runs with the
owner's grants. Every function in the set is defined with
`SET search_path = pg_catalog, <schema>`.

**Table owners bypass row-level security.** `projection_owner` owns nothing, but
the functions run as it, so without `FORCE ROW LEVEL SECURITY` on every
projection table a rebuild is unscoped by tenant. D25 already stated this
requirement; it is restated here because it is the third member of the same
family and the three are worth reading together.

All three are catalogue assertions, not review items: `rolcanlogin = false` and
zero rows in `pg_auth_members` for `projection_owner`; no `SECURITY DEFINER`
function in the set with `EXECUTE` to `PUBLIC`; no `SECURITY DEFINER` function
in the set without `search_path` in `proconfig`; `relforcerowsecurity` true on
every table carrying an `@projection` column.

#### Forcing RLS makes the rebuild per-tenant, which is the better design anyway

With RLS forced, a rebuild can only touch rows its tenant predicate admits, so
**there is no global rebuild**. The scheduled job iterates tenants and rebuilds
each in its own transaction. That is a constraint accepted rather than worked
around, and four things improve:

- A rebuild defect cannot cross a tenant boundary, so the largest blast radius in
  the maintenance path is one tenant.
- Drift in one tenant no longer stalls the check for every other tenant.
- `projection_check.scope_kind`/`scope_id` gains a natural value instead of
  usually being null, and stays clear of the cell-naming defect D30 found.
- A full rebuild of `stock` at scale stops being one long transaction holding
  back vacuum on the busiest table in the schema.

**The honest limit.** This bounds a *bug*, not an attacker holding the
connection. `current_tenant()` is a session setting and any session can set it,
so tenant isolation still rests on the application's authentication layer, as it
already does everywhere else. What the database enforces here is that a rebuild
predicate which forgot its tenant filter writes nothing rather than writing
everything.

#### Every invocation writes a `projection_check` row, drift or not

The audit q102 asked for is a table the model already has. `projection_check` is
a fact meaning *"we checked a projection against its source"*, and every
invocation of a maintainer function writes one whether or not it found anything.

That answers three questions with one query. What was rebuilt and when. Who
asked, since a scheduled run and the platform forcing a rebuild after a suspected
drift are different acts and `invoked_by_id` distinguishes them. And, most
usefully, **what has not been rebuilt**, because a scope with no recent row is a
scheduler that stopped, which is otherwise the quietest failure in the system.

A separate audit table was the alternative and it would record the same facts in
a second place, against D25's one-mechanism rule.

#### The reaper is the rebuild's delete phase, not a job

D30 is titled *"the predicate belongs to the rebuild"* and then gives the reaper
its own schedule, its own role and its own execution. Completing the decision in
its own direction removes the gap: **the reap is a phase inside
`projection_stock_rebuild()`**, running before the fold, under the same scope, in
the same transaction, writing `rows_removed` on the same `projection_check` row.

J32 said reap, rebuild and assert together produce zero `projection_drift`. As
two jobs that is a property to test and a window where it is false. As one
function it is structurally true, because nothing can observe the intermediate
state.

The weekly cadence D30 gave the reaper is preserved as a parameter: the reap
phase runs when the schedule says so and is skipped otherwise. One function, one
scope, one audit row, and the reaper is unblocked.

#### Amendments to earlier decisions

- **D25** — the maintainer is defined: `projection_owner` is `NOLOGIN` with no
  members, holds every projection write grant, and owns one `SECURITY DEFINER`
  function per projection per operation. The bidirectional CI diff already
  required for `@projection` comments extends to the function set, so a
  projection column with no maintainer function and a maintainer function with no
  projection column both fail.
- **D25** — `projection_check` gains `invoked_by_id` (nullable; null means the
  scheduler), `rows_removed`, and a row on every invocation rather than only on a
  check that ran.
- **D26** — the compiler role is the same pattern named differently, and the two
  are stated as one rule with two instances rather than two similar decisions.
- **D30** — the reaper is a phase of the rebuild rather than a job. Its role
  question is answered by there being no role. J32 is restated as structural.
- **D18** — the tenant-isolation claim for the maintenance path is stated with
  its limit, so nothing downstream reads it as stronger than it is.
- **J36** *(new)* — no login role holds `INSERT`, `UPDATE` or `DELETE` on any
  column commented `@projection`. Asserted from
  `information_schema.column_privileges` joined to `pg_roles` on `rolcanlogin`.
- **J37** *(new)* — every `SECURITY DEFINER` function in the maintainer set has
  `search_path` set in `proconfig` and no `EXECUTE` grant to `PUBLIC`. Asserted
  from `pg_proc` and `pg_default_acl`. **This is the invariant that catches a new
  maintainer function written correctly and deployed with Postgres defaults**,
  which is the likely failure rather than a deliberate one.
- **J38** *(new)* — `relforcerowsecurity` is true on every table carrying an
  `@projection` column.

**Rejects.** A `projection_maintainer` login role, refused as ambient privilege
with a five-job surface. A separate maintainer audit table, refused against
`projection_check`. A global cross-tenant rebuild, refused because forcing RLS
makes the per-tenant form mandatory and it is better on four counts. Exempting
the maintainer from RLS instead of forcing it, which is the shape of the hole
D18 exists to prevent. Granting the application role `EXECUTE` on the rebuild
functions so a screen can trigger one: a rebuild is a platform act, and a screen
that can start one at will is a denial-of-service surface on the busiest table in
the schema.

---

### D36 — Extension ceilings are claimed slots, enforced before the DDL runs

*Adopted 2026-08-03, settling question 111. D26 declared three numbers (50
schemes per tenant, 60 fields per scheme, 100 tenant metrics) and left them as
figures a job would check.*

#### A ceiling checked tomorrow has no remedy, which is why it must be a constraint

The usual argument against deferred enforcement is that undefined behaviour is
bad. The argument here is narrower and stronger: **by the time the job runs there
is no action left that is not worse than the violation.**

The tenant declared, the compiler ran DDL, the table exists and rows are landing
in it. Tomorrow's job can drop the table, destroying tenant data in a model whose
generated tables carry `ON DELETE RESTRICT` precisely so evidence is never
destroyed as a side effect. Or it can raise a finding and change nothing, which
makes the ceiling a note. There is no third option, so the ceiling has to hold
**before** the DDL, or it does not hold.

The ceiling is also not a resource limit. D26 says so directly: *"not because 51
breaks anything, but because the ceiling is what keeps this a schema extension
rather than a schema escape."* Resource limits can be soft, because exceeding one
degrades gradually and the platform wants headroom. A design boundary cannot be
soft, because a negotiable design boundary is not a boundary. Salesforce's
flex-column pivot, which D26 names, is what a soft one looks like after five
years.

`CREATE TABLE` is transactional in Postgres, so this works: a failed claim inside
the materialisation transaction rolls back the DDL with it. The tenant gets a
named error and an unchanged schema, and "mid-declaration" is not a state that
exists.

#### The ceiling is data, so there is no number to disagree with

D25 forbids validation triggers by name, so `BEFORE INSERT ... SELECT count(*)`
is not available and should not be smuggled in. A counter column is a projection
with no fact behind it. A `CHECK` cannot hold a subquery, so a per-tenant limit
cannot be read from the tenant row.

Slots are issued instead, and the ceiling **is** the number of slot rows.

```
extension_slot                -- REFERENCE. PLATFORM-OWNED, like number_range.
  tenant_id, kind, ordinal    -- kind: record_scheme | metric
  claimed_key                 -- NULL = free
  claimed_at, withdrawn_at
  PRIMARY KEY (tenant_id, kind, ordinal)
  UNIQUE (tenant_id, kind, claimed_key)
```

Provisioning a tenant issues 50 `record_scheme` slots and 100 `metric` slots.
Declaring a scheme claims one:

```sql
UPDATE extension_slot SET claimed_key = $key, claimed_at = now()
 WHERE tenant_id = $t AND kind = 'record_scheme'
   AND claimed_key IS NULL AND withdrawn_at IS NULL
 ORDER BY ordinal LIMIT 1 RETURNING ordinal;
```

Zero rows returned is the ceiling, and it is a defined, testable outcome rather
than an error class. `record_scheme` carries a foreign key to the claiming slot,
so a scheme without one cannot exist.

There is no ceiling constant anywhere. **A number in a `CHECK` and a number in a
plan description are two representations that will eventually disagree**, and
this design has one. Raising a tenant's ceiling is inserting slot rows, which is
a platform act with a row and a timestamp behind it rather than a migration.
That is the same posture as `number_range`, and it is what the model means by
capability determined by data.

**The concurrent-declaration race resolves correctly and is worth stating.** Two
transactions claiming simultaneously both target the lowest free ordinal; one
blocks on the row lock, then re-evaluates its predicate under READ COMMITTED,
finds `claimed_key` no longer null, updates zero rows and moves to the next
ordinal or hits the ceiling honestly. No lost update, no double claim, no
advisory lock.

#### Fields need no new machinery

`record_scheme_field.ordinal` already exists. `CHECK (ordinal BETWEEN 1 AND 60)`
with `UNIQUE (record_scheme_id, ordinal)` gives the 61st field nowhere to go,
declaratively and with no trigger.

A fixed constant is right here where it was wrong above, and the test between the
two shapes is stated rather than left to taste:

> **A ceiling that is per-tenant and commercially variable is issued slots. A
> ceiling that is per-parent and fixed by design is an ordinal with a `CHECK`.**

Sixty fields is a statement about what one table should hold before it wants to
be two, which is the same for every tenant on every plan.

#### Two ceilings were conflated, and the release rule separates them

*"50 schemes per tenant"* is ambiguous between 50 rows in `record_scheme` and 50
distinct keys, and D26's own evolution rule drives those apart quickly: version
N+1 mints a new table and **the old table stays**. Under the row reading a tenant
with ten well-maintained schemes is punished for maintaining them, having spent
50 on five revisions each.

So the slot is claimed by the **key**, and every version of that key shares it.
The design boundary counts distinct schemes, which is what it was always about.

That leaves the resource question the row reading was accidentally answering:
live tables in the catalogue are now unbounded even though keys are capped. It is
answered by the release condition rather than by a second number:

> **A slot is released only when every table its scheme ever materialised has
> been archived under D31.**

Retiring a scheme does not free the slot while its data is still queryable. One
ceiling counts keys, and the thing it was silently also bounding is bounded by
what releasing costs.

#### Lowering a ceiling never destroys a schema

The platform downgrading a tenant cannot delete claimed slots, because the foreign
key from `record_scheme` would either refuse or cascade, and cascading here means
deleting tenant tables to enforce a billing change.

`withdrawn_at` is set instead. A withdrawn slot keeps its scheme, is excluded
from the claim predicate, and cannot be reclaimed when released. The tenant
descends to the new ceiling by attrition and nothing is destroyed, which is the
same posture D31 takes everywhere else.

#### Amendments to earlier decisions

- **D26** — the three ceilings are enforced at declaration, not asserted by a
  job. The scheme and metric ceilings become issued slots; the field ceiling
  becomes an ordinal `CHECK`. The stated numbers become provisioning defaults.
- **D26** — the slot is claimed by `key`, not by `record_scheme` row, so versions
  do not consume the budget.
- **D26 / D31** — a slot is released only when every table its scheme
  materialised has been archived. Archival of a superseded scheme version's table
  is named as the mechanism bounding catalogue growth.
- **D19** — `extension_slot` is platform-owned reference data with `tenant_id NOT
  NULL`, the same shape as `number_range`. A tenant may read its own slots, which
  is how a screen shows what is left, and may not write them.
- **D25** — this is a validation, and it is enforced with keys and a `CHECK`
  rather than a trigger, which is the rule holding rather than an exception to it.
- **J39** *(new)* — every `record_scheme` row has a claiming `extension_slot`, and
  no slot is claimed by two keys. A plain foreign key and a unique constraint, so
  the invariant is the schema rather than a job.
- **J40** *(new)* — no `record_scheme` whose slot is released has an unarchived
  materialised table.

**Rejects.** A job that checks ceilings after materialisation, refused because it
has no remedy that is not worse than the violation. A `BEFORE INSERT` counting
trigger, refused by D25. A ceiling constant in a `CHECK` alongside a plan
description, refused as two representations of one number. A counter column on
`tenant`, refused as a projection with no source. Deleting claimed slots on
downgrade. Freeing a slot on retirement while the scheme's tables are still live,
which would cap keys while leaving the catalogue unbounded and is the conflation
this decision separates.

**Raises question 121.** `event_subscription` is the third of D26's three
extension primitives and it has no ceiling, while the other two now have enforced
ones. An unbounded subscription set is a real load surface on the outbox, but the
number depends on outbox throughput that has not been measured, so this is a gap
named rather than a number invented.

---

### D37 — The receiving path, and the one column keeping the cold chain hot

*Adopted 2026-08-03, settling the buildable half of question 75 and raising 122
for the half that needs a database. Question 75 was raised before the merge and
three of the four costs it named have since been removed by decisions that were
not written to answer it.*

#### The chain in the question is the cold path, and the screen must not walk it

Question 75 named
`goods_receipt → inbound_shipment → in_force_assertion → asserted_unit →
asserted_unit_content`, plus observation resolution, plus a policy resolve per
line, plus `client_event`. Checking each against what was actually adopted:

| Cost named | Where it is now |
|---|---|
| The five-table assertion walk | Collapsed by `expected_supply`, **except for one hop** |
| A policy resolve per line | Gone. `expected_supply.receiving_policy_id` is resolved at projection time |
| Observation resolution | Never on the list. It is a second query, opened per unit |
| `client_event` | Write path, not read path. One point lookup per submitted scan |

The assertion body is the archive of what a supplier claimed. It is written once,
read during investigation, and quoted in a dispute months later. **It is a cold
table and the dock is the hottest screen in the building**, so D24's availability
rule applies here verbatim:

> **No join through an assertion body, and no aggregate over a fact table, on the
> receiving path.**

`expected_supply` exists precisely to be the warm projection between them, and it
already carries the advised lot, the advised expiry, the expected window and the
resolved policy. That is three of the four costs gone.

#### The remaining hop, and why it costs exactly one column

`expected_supply`'s four arms are `purchase_order_line_id`,
`transfer_order_line_id`, `asserted_unit_content_id` and
`return_authorisation_line_id`. **None of them is a shipment**, so scoping the
screen to a delivery still requires
`asserted_unit_content → asserted_unit → assertion → despatch_advice`, which is
the exact chain the question names, three joins deep, per line.

The asymmetry that decides this: a purchase order is **one hop** from its lines,
so the unadvised arm scopes with an ordinary join and needs nothing. A shipment
is **three hops** from its content lines, because the hierarchy in between is the
supplier's declared packaging structure rather than ours.

So one column, not four:

```
expected_supply
  inbound_shipment_id     -- @projection from the assertion arm's chain.
                          --   NULL for the other three arms.
```

D24 (supply side) set this precedent by denormalising `owner_id` and `status_id`
onto the same table as projections of the source line. This is the same move for
the same reason, and it stays a projection maintained by D35's function rather
than a column anyone writes.

**The refinement case works out correctly and is worth checking rather than
assuming.** An ASN row refining a purchase order row carries
`asserted_unit_content_id`, so the child gets the shipment and the parent does
not. Receiving against a delivery therefore lists the refined children, which is
what the operator should see, and the unrefined parent stays out of the list
until the goods actually arrive unadvised.

#### The screen is three queries, and merging them is what made it unmeasurable

Six designs could not measure the aggregate because they were describing one join
graph. It is not one. Separating them is most of the answer, because each has a
different frequency and therefore a different budget.

**A. The line list, advised.** Once per delivery.

```sql
SELECT es.id, es.item_id, i.code, i.description,
       es.quantity_expected, es.quantity_outstanding,
       es.advised_lot_code, es.advised_expiry_date, es.receiving_policy_id
  FROM expected_supply es
  JOIN item i ON i.id = es.item_id
 WHERE es.tenant_id = current_tenant()
   AND es.inbound_shipment_id = $1
   AND es.closed_at IS NULL;
```

One partial index range scan returning N rows, then N primary-key lookups on
`item`. N is lines on a delivery: five to sixty typically, a few hundred at
worst. Bounded by N with every access an index hit, and no table in the plan is a
fact table.

**B. The line list, unadvised.** Once per delivery, and the common case here,
because GS1 Australia's retailer matrix puts despatch advice in the preferred
tier rather than the mandatory one.

```sql
  FROM expected_supply es
  JOIN purchase_order_line pol ON pol.id = es.purchase_order_line_id
  JOIN item i ON i.id = es.item_id
 WHERE es.tenant_id = current_tenant()
   AND pol.purchase_order_id = $1
   AND es.closed_at IS NULL;
```

Driven from `purchase_order_line (purchase_order_id)`, then the existing
`UNIQUE (tenant_id, purchase_order_line_id)` on `expected_supply`. No new index.

**C. The scan resolve.** Once per carton, which makes it the only one in this set
that is genuinely hot. D34's resolver, one hit on `item_barcode (barcode)`,
matched against the already-loaded list in the client. **The naive design queries
the line list again on every scan**, which is how a screen that measured fine
becomes unusable at a hundred cartons.

#### The index set, and the one that is only correct if it is partial

```
expected_supply (tenant_id, inbound_shipment_id) WHERE closed_at IS NULL
item_barcode (barcode)                                        -- D34
purchase_order_line (purchase_order_id)                       -- ordinary
```

The partial predicate is the whole point. Inbound at this scale is roughly thirty
deliveries a day at thirty lines, so about 230,000 `expected_supply` rows a year
a site, of which a few thousand are open at any moment. **A year in, the live set
is about one percent of the table**, and an unpartial index makes the planner
read the other ninety-nine.

That is also the specific way a naive acceptance test passes and production
fails: seed a week of data and every row is open, so the index looks fine and the
predicate does nothing.

#### A hazard on the reference join, recorded because it bites the next screen

D19's reference-table policy is `tenant_id IS NULL OR tenant_id =
current_tenant()`. **A row-level security policy with an `OR` over a nullable
column is a known planner hazard**, because it can turn an index scan into a
sequential scan on the shared catalogue.

Query A is safe: the join is a primary-key lookup, so the policy is a filter on
one row rather than the thing driving access. **A screen that searches items by
description or code is not safe**, and that is a different screen with a
different index, most likely on the same `COALESCE(tenant_id, sentinel)`
expression D34 needed for the same underlying reason. Recorded here because the
next person to write a picker will hit it and the failure is a slow page rather
than an error.

#### Which policy the receipt is judged against

`expected_supply.receiving_policy_id` is resolved when the row is projected,
which can be weeks before the goods arrive. D22's rule that a projection under a
policy records the policy that produced it is satisfied by that column, and it is
what stops a tolerance change reading as drift.

It is **advisory for the screen and not the judgement**. The judgement is the
`acceptance` fact (D25), which resolves the policy at receipt and records the row
it used. So a manager who changes a tolerance the morning of a delivery gets the
new tolerance applied, and the stale advisory value never silently decides
anything.

#### Amendments to earlier decisions

- **D24 (supply side)** — `expected_supply` gains `inbound_shipment_id` as
  `@projection`, NULL for the three non-assertion arms, with the partial index
  above.
- **D24 (supply side)** — the availability rule is restated to cover receiving:
  no join through an assertion body and no aggregate over a fact table on either
  path.
- **D22 / D25** — the split is stated: `expected_supply.receiving_policy_id` is
  advisory pre-population; `acceptance` resolves and records the policy that
  judges the receipt.
- **D34** — the resolver is named as the receiving screen's per-scan path, and
  re-querying the line list per scan is refused.
- **J41** *(new)* — `expected_supply.inbound_shipment_id` equals the walk
  `asserted_unit_content → asserted_unit → assertion → despatch_advice` for every
  assertion-arm row, and is NULL for every other arm. Rebuild-and-assert, like
  every other projection column.

**Rejects.** Denormalising `purchase_order_id` onto `expected_supply` alongside
the shipment: the unadvised arm is one hop and does not need it, and two
denormalisations where one is justified is how a projection turns into a
duplicate of its sources. Resolving policy per line at read time. Loading
supplier-declared measurements with the line list, when they are wanted for one
unit at a time. One merged query for the whole screen, which is what made the
aggregate unmeasurable in six designs.

**Raises question 122**, and question 75 is retired into it. The queries above
are written and their plans are reasoned about; they are not measured, because
measuring needs a database with a year of seeded history that does not exist yet.
The acceptance test is named now so it is written with the first migration rather
than remembered later:

> Query A at p95 under 50 ms with 200 lines against a year of history at least
> ninety percent closed. Query C at p95 under 10 ms, because it is the only one
> in an operator's hand per carton. `EXPLAIN (ANALYZE, BUFFERS)` on both, with the
> plans committed beside the test so a later regression is a diff rather than a
> memory.

---

### D38 — The invariant register has one home, and an absence is only as strong as its population

*Adopted 2026-08-03, settling question 88. The register was 77 entries across
four documents. Consolidating it found a numbering collision created hours
earlier, one invariant that had never been numbered at all, and four entries
whose current text could not be read from a single document.*

#### The evidence arrived while the question was being answered

Question 88 predicted the register would erode as principle 3's census did. It
had already begun, and the sharpest instance is the most recent one.

**D34 allocated J34 and J35 on the day this decision was adopted, and D28 and D29
already held them.** That happened in the same document that had been
consolidated hours earlier to stop exactly this failure for questions, and it
happened for exactly the same reason: numbers hand-allocated in prose across
several files, with no mechanism that could notice. The earlier claim wins, so
D34's two entries are renumbered J42 and J43.

Two more, both quieter:

**D31's retention-floor coverage assertion had no number.** The decision states
that for every declared floor either the oldest live partition or the archive
must reach back to it, and that assertion entered no register. It is S34.

**Four entries could not be stated from one document.** J3 was annotated in one
place and defined in another; J6 was extended twice, in two files; J19 was
widened in one and scoped in a second; S2 was corrected in a third. J6 is one of
the five invariants previously found encoding its own bug, and reconstructing its
current text required reading two documents. **An invariant nobody can quote is
not one.**

#### An anti-join is only as strong as the assertion that its population exists

The most useful finding is not the collision. It is a property that cuts across
the whole register and had been noticed for exactly one entry.

`stock_movement` can lose half its rows and J1 fails immediately, because a fold
compares a projection against its source and the two stop matching. That is
self-guarding.

An **absence** is not. *"No serial is reissued within twelve months"* passes
perfectly when the twelve months of history have been truncated. So does *"the
model contains no `jsonb` column"* before any tables exist, and *"no resolver
call appears inside a loop"* before any resolver does, and *"no login role holds
UPDATE on a projection column"* before any projections exist.

> **Every invariant asserted as an absence passes when its population is empty,
> and therefore reports success on the day it stops being checked.**

This was recorded once, for the SSCC reuse guard, as a class the register could
not check. It is not a class. **Twenty-nine of the seventy-seven entries have
this property**, and each needs a companion assertion that its population is
non-empty and reaches back far enough. S34 is the general one where the bound is
history depth. The rest are named per entry as they are implemented, and an entry
that reaches `implemented` without one is a lie the suite tells itself.

That number, 29 of 77, is the one worth watching. It is invisible while the
register is prose.

#### The register is generated from the suite, not maintained beside it

Question 88 stated the direction: *"it should be the CI suite, with this table
generated from it, not the other way round."* Made concrete:

Each invariant is a test carrying its own metadata: identifier, class, statement,
the decisions that own it, the assertion method, whether it asserts an absence,
and its companion population assertion if so. `docs/invariants.md` is generated
from that set, and CI fails when the generated file differs from the committed
one.

**The identifier is then a constant in code**, so allocating one twice is a
compile error rather than something a careful reader might notice. That is the
same move as D26's ownership and D35's function-scoped privilege: the failure
becomes unrepresentable rather than detected. It is also the mechanism D25
already uses for `@projection` columns, applied to the register itself.

**The register has no owner, and that is the answer to the half of question 88
that asked for one.** An artefact generated from the thing it describes does not
need a person to keep it true. What needs an owner is *adding* an entry, and that
is the same rule the question register already runs on: an invariant stated in a
decision's amendments is added in the same commit that adopts the decision. A
decision may state an invariant; it does not own the numbering.

#### Until the backend exists

The file is hand-written today and the numbers are allocated in it, which is the
stopgap that already failed once. What makes the stopgap safer than the previous
arrangement is that there is one file rather than four, so a collision is visible
to anyone adding an entry.

Every entry carries `status`, and all 77 are `specified`, meaning stated and
reasoned about with nothing executing. **The count of `specified` entries can
only go down**, which makes it a better measure of progress than the count of
decisions, since decisions can be added indefinitely and invariants without tests
cannot.

#### Amendments to earlier decisions

- **D34** — J34 and J35 renumbered to J42 and J43. Nothing outside D34's own text
  cited them.
- **D31** — the retention-floor coverage assertion is numbered S34 and is named
  as the companion population assertion for every absence-asserting entry bounded
  by history depth.
- **D36** — J21 is superseded as a job. The ceiling is enforced at declaration by
  claimed slots, so the job form narrows to asserting that no tenant holds more
  claimed slots than issued.
- **D35** — J32 is noted as structural rather than job-asserted, because the reap
  is a phase inside the rebuild function and the intermediate state cannot be
  observed.
- **D25** — the bidirectional registry diff already required for `@projection`
  columns is the same mechanism this register uses, stated once rather than
  described twice.
- **mechanism-design.md, supply-side-design.md, d24-open-questions.md** — their
  invariant tables are retained as written and annotated to say that
  [invariants.md](./invariants.md) is authoritative on text, numbering and status.

**Rejects.** A third invariant class for retention floors, refused because the
property is not a class: it is a per-entry attribute that twenty-nine entries
share. Leaving the numbering in prose with a convention about checking first,
which is what was in place when the collision happened. Renumbering the older
claim rather than the newer, which would break existing citations to preserve a
decision hours old. Generating the tests from the document, which inverts the
dependency and produces a suite nobody can debug.

---

### D39 — Every integration is a capability, never a dependency

*Adopted 2026-08-03, settling questions 4/58, 56 and 49. Three questions that
looked separate turn out to be one principle asked three times.*

> **The system is complete on its own. Every external system is one
> implementation of a seam that has a working default behind it.**

The design previously read as though NetSuite stays the financial system, which
quietly made it required. It is not required. It is what this deployment happens
to have, and a deployment without it must lose no necessary function.

#### Orders are ours, and NetSuite is a channel (question 4/58)

`order` and `purchase_order` are **ours**. They are created here, they are
complete here, and an operation running nothing else works.

An order arriving from NetSuite is **not an assertion**, and getting this wrong
would be expensive. D21's assertion category exists for claims by parties outside
our control, whose defining property is that somebody outside holds a copy and
will quote it back. NetSuite is *us*. An order arriving from it is **our own
intention reaching us through a channel**, which is the vocabulary D23 already
established for observations. Filing it as an assertion would make our own orders
unretractable and require an acceptance before they could be acted on.

**One system of record per order, declared at creation and recorded on the row.**
`order.source_channel` names where it came from and `order.external_ref` holds
the identifier there. An order whose record of authority is external is amended
through that system, not here.

**Bidirectional merge is refused outright.** Two systems that both accept edits to
one order need a conflict resolution nobody can explain to a person on a dock,
and every product that has tried this ships a reconciliation screen as the
apology. One authority per row, recorded, is the whole mechanism.

#### Inter-company documents generate by default (question 56)

A movement between two of our own legal entities is a sale, and something must
raise the paperwork.

**The system generates it.** A deployment that has a finance system may configure
the movement to raise a flag for that system instead. Both paths exist and
neither is required, which is the principle applied rather than a preference
expressed.

Alpha will use the flag path, because NetSuite is here and doing it twice is
worse than doing it once. That is a configuration of this deployment, not a
property of the design, and the distinction is exactly what the previous framing
lost.

#### Multiple legal entities are already supported and cost nothing (question 49)

The answer is that the schema has held this since D20 and nobody checked.
`site.legal_entity_id` exists, D32 made a legal entity a `party` carrying the
`legal_entity` role, and stock ownership already points at a party. An
inter-company movement is a movement whose owners are two parties both holding
that role. **No new table, no new column, no migration.**

So the default is one entity with several sites, and several entities is
available to any deployment that needs it, at no cost to the one that does not.

**Why one is the right default here, and why that is a fact rather than a
choice.** Australia does not incorporate at state level: companies register
federally with ASIC under the Corporations Act 2001, and one ACN operates
nationally. What is state-based is registration rather than incorporation, being
payroll tax, land tax and workers' compensation, none of which requires a
separate company. Separate entities in Australia come from acquisitions never
merged, liability ring-fencing or joint ventures, not from geography. This is the
opposite of the United States, where state incorporation is the norm.

**The competitor set converges from the other direction.** NetSuite sells
multi-subsidiary as OneWorld, a paid edition, and a vendor only does that when
the common case does not need it. Manhattan assumes multi-entity because it sells
to enterprises that are. ShipHero and Peoplevox are single-entity and
multi-warehouse. CartonCloud, the Australian one, organises around the client
boundary rather than the legal-entity boundary. **Every product treats
multi-warehouse as universal and multi-entity as an upsell**, and the Australian
ones do not model it at all.

An Australian company operating overseas needs it, which is the case that makes
availability the right call rather than merely a harmless one.

*Marked for confirmation rather than decision: whether any Alpha state
operation sits in its own ACN for historical reasons. That is a lookup with the
accountant, and either answer leaves the schema unchanged.*

#### Amendments to earlier decisions

- **D15** — `order` gains `source_channel` and `external_ref`; exactly one system
  of record per order, recorded.
- **D20** — the multi-entity capability is confirmed as already present, with the
  Australian default stated so nobody adds a table for it later.
- **D23** — the ingestion-channel vocabulary covers inbound orders, not only
  observations.
- **Non-goals** — "NetSuite remains the financial system" is restated as a
  property of this deployment rather than of the design.
- **J44** *(new)* — no `order` row is written by a path that does not set
  `source_channel`, and no externally-authoritative order is amended locally.

**Rejects.** Orders as mirrors of NetSuite's, which makes the whole inbound build
a synchronisation project. Bidirectional merge on any shared entity. Filing
internally-sourced orders as assertions. A `legal_entity` table separate from
`party`, refused because D32 already covers it and a second identity table is how
the party question got asked in the first place.

---

### D40 — Third-party stock is billable, and the mechanism already exists

*Adopted 2026-08-03, settling question 66. D20 admitted the stock and the
exclusions still declined the billing, which is not a business.*

Storage and handling for another company's stock are **billable, off by default,
and built from what is already there**.

**Nothing new is needed to measure it.** Pallet-days fall out of
`package_containment`'s intervals, which exist because D24 made placement a fold
over events with a validity range. Handling in and out are `stock_movement`
facts. The two questions a storage invoice asks are already answerable, and this
is what the containment decision bought without being justified on it.

**Rates are two new kinds under D22's lattice, not a new mechanism.** A rate card
is most-specific-wins over client, item class and site, which is precisely what
the scope lattice does. `storage_rate` and `handling_rate` join the eight
existing kinds and inherit the resolver, the explain view and the
policy-change record.

**Capability follows data.** A deployment with no rate rows bills nothing and
never encounters the feature, which is the same pattern as batch tracking, catch
weight and multiple entities.

**The invoice is out of scope and stays out.** This produces the charge lines and
what they were computed from. Rendering and sending an invoice is a finance
system's job, and D39's rule applies: a deployment without one gets the lines and
can export them.

#### Amendments to earlier decisions

- **D22** — `policy_kind` gains `storage_rate` and `handling_rate`. S13's
  three-way diff covers them with no change.
- **D24** — pallet-day derivation from `package_containment` is named as a
  consumer, so a later change to that table knows it has one.
- **Non-goals** — third-party billing moves out of the exclusions.

**Rejects.** A separate rate-card table with its own precedence, which would be
D22 built twice. Storing computed charges as facts before an invoice exists, when
they are derivable and D25 forbids unrebuildable maintained tables. Invoice
rendering.

---

### D41 — Telemetry and benchmarking are two products, and only one is buildable now

*Adopted 2026-08-03, settling question 67, which had been forbidden as a side
effect of a data-scoping rule rather than by anyone deciding it.*

The question bundled two things with different subjects, and separating them is
most of the answer.

**Product telemetry is about our software.** Which screens are used, how long a
pack takes, error and crash rates. It is collected with **per-tenant opt-out**,
and what is collected is a **declared enumerated list** rather than whatever
happens to be logged. A list nobody wrote is how a telemetry feature becomes a
privacy incident.

**Benchmarking is about other companies' commercial performance**, and calling it
anonymised does not make it so.

> With a small tenant set, an aggregate plus your own contribution recovers
> everyone else's. At three contributors it recovers them exactly.

**Opt-out makes this worse rather than better**, which is the counter-intuitive
part worth writing down: withdrawing a tenant shrinks the cohort, and small
cohorts are the re-identifiable ones. A feature whose privacy control degrades
privacy is not a control.

The honest version needs a **minimum cohort floor with suppression below it**,
conventionally five contributors, so a supplier used by four tenants returns
nothing at all. That is a real and defensible product. It is not buildable now,
because there is one tenant.

**So the general rule, stated as a decision rather than inherited from D19:**
cross-tenant reads are forbidden. Any exception is a named decision carrying a
cohort floor and a suppression rule. D19's scoping continues to enforce it; what
changes is that switching it off now requires overturning a decision rather than
noticing a rule was inconvenient.

#### Amendments to earlier decisions

- **D19** — cross-tenant isolation is a decision with its own reasoning, not a
  consequence of RLS shape.
- **D18** — telemetry is named as the one egress path and is bound by the
  declared list.
- **J45** *(new)* — no query in the register reads across tenants except those on
  a declared exception list, and every entry on that list carries a cohort floor.

**Rejects.** Benchmarking without a cohort floor. Treating opt-out as sufficient
privacy control for aggregates. Collecting telemetry against an undeclared list.

---

### D42 — An amendment to an intention is a fact, and the model has done this four times

*Adopted 2026-08-03, settling question 77. `row_audit` was refused and the
requirement behind it was never answered.*

"Do we need an audit log" imports a mechanism from systems where mutable rows are
normal. **This system mostly does not have mutable rows, and everywhere it
removed them the audit question dissolved rather than being answered:**

| Was mutable | Became |
|---|---|
| `stock` balances | A fold of `stock_movement` |
| Pallet placement and identity | A fold of `package_event` (D24) |
| Policy values | Paired with `policy_change` |
| Expected quantities | A projection with its sources kept |

Four instances of one move, never named as one. **The order is the significant
mutable entity left, which is exactly why the question kept coming back about the
ship-to address specifically.**

So there is no audit requirement. **There is a missing fact.** *"On Tuesday, Kyle
changed the ship-to address from A to B"* has an author, a time, a device and a
reason, which is this model's definition of a fact, and there is already a table
shape for it and a rebuild-and-assert job that checks it for free.

**It is cheaper than what was rejected, not more expensive.** `row_audit` was
refused partly because a generic changelog doubles the write volume of the
largest tables to record nothing, since fact tables cannot be edited. That
argument reverses here: orders are hundreds a day and amendments to them are
rare. **The expensive version was refused on the tables where it was worthless,
and the cheap version was never considered on the table where it is valuable.**

```
intention_amendment           -- FACT
  id, tenant_id
  order_id | purchase_order_id | transfer_order_id    -- CHECK num_nonnulls = 1
  kind        -- ship_to_changed | requested_date_changed | quantity_changed
              -- | line_added | line_removed | cancelled | reopened
  <typed payload columns per kind>
  reason_code, note
  occurred_at, recorded_at, client_event_id
  recorded_by_id / automation_key, authorised_by_id
```

Two rules keep it from becoming the generic changelog wearing a different label.

**No `field` column.** S11's denylist already forbids that name for policy
tables, and for the same reason here: `field`, `before` and `after` is untyped
soup relabelled. The `kind` enum is closed and adding a value is a decision,
which is what keeps the set honest.

**No `before` column.** The previous value is the previous amendment, or the
original. You fold, exactly as `stock_movement` does.

**What belongs in the enum has a test you already have.** If nobody would ever
investigate a change to a field, it is not an event and the column stays mutable.
That is principle 3's queryability test pointed at changes rather than at values.

The covered columns on `order` become `@projection` of the original plus the
amendments, under D35's maintainer. The rest stay ordinary mutable intention
columns, which is the same hybrid D24 applied to `package`.

#### Amendments to earlier decisions

- **D25** — `row_audit` stays refused, and the requirement behind it is now
  answered by a typed fact rather than left open.
- **D15** — `order`'s covered columns are demoted to `@projection`.
- **D35** — `intention_amendment` folds are maintained by the same function set.
- **J46** *(new)* — every covered `order` column equals the fold of
  `intention_amendment` over that order, in `(occurred_at, recorded_at, id)`
  order, and replay in any arrival order is identical. Same shape as J6.

**Rejects.** A generic `(table, row_id, before, after)` changelog, refused twice
now and on stronger grounds. A `field` column. Storing the previous value. One
amendment table per intention type, which is the polymorphic problem inverted
into three near-identical tables. Demoting every `order` column, which would make
routine pre-release editing an event stream nobody reads.

---

### D43 — The order level is a node, and a delivery's receipts are one per demand document

*Adopted 2026-08-04, refining D24 (supply side)'s settlement of questions 68 and
109 and raising 125 and 126. Neither question is reopened: both were settled
correctly, and one of the two reasons given holds for EDIFACT and not for X12.*

#### The settled answer was right, and half its reason was not

D24 (supply side) settled multi-PO advice with this:

> An ORDER-level split does not need a new structure: it becomes content lines
> each naming one PO line, which is exactly what
> `asserted_unit_content.resolved_purchase_order_line_id` holds.

The conclusion is correct and stays. The step under it is true of one standard
and false of the other, and the difference is invisible until an 856 arrives.

**EDIFACT states the purchase order on the line.** A DESADV nests `CPS`
packaging sequences with `LIN` under the innermost level, and `RFF+ON` is
available at line level. `asserted_unit_content.raw_po_reference` is filled from
the message.

**X12 states it above the line.** `PRF` carries the purchase order number in the
order-level `HL` loop. The item level carries `LIN` and `SN1` — identifier,
quantity, unit of measure — and no purchase order at all. A shipment covering
several orders has one order loop with its own `PRF` per purchase order number,
which is the documented structure for mixed-PO shipments rather than an edge of
it.

So ingesting an 856 at content-line grain means walking up from each item node
and writing an ancestor's `PRF` into every descendant's `raw_po_reference`.

#### An inherited raw value is not an exchanged one

D21 splits an assertion body's columns into two classes and hangs immutability
off the split: `raw_*` and every transcribed value are **as exchanged**,
`resolved_*` are our annotation. Rule 5 says an assertion is recorded in the
author's vocabulary.

After that walk, `raw_po_reference` holds a string that **appears nowhere on that
line in the original message**. The value is correct and the claim the column
makes about itself is not.

It fails silently, in the shape this model has now caught five times: a check
that reads `raw_*` to prove fidelity to the received bytes passes, because the
value it finds really was in the message, just not there. The failure is in the
provenance, and the provenance is what the column class exists to carry.

#### The order level is a node in `asserted_unit`

**Decision.** An X12 order loop is stored as an `asserted_unit` node with
`level_code = 'order'`, no SSCC, and physical children. The tree stored is the
tree the author sent, and resolution to a purchase order line walks up at
ingestion.

**This costs no schema change.** `asserted_unit` already carries `level_code`,
already permits unbounded nesting on the stated grounds that it is a cold path,
already has a nullable `sscc`, and already collapses to D24's depth cap at
receipt. The walk happens once per message at ingestion, never once per scan at
the dock, which is the boundary D37 drew when it put the assertion body off the
receiving screen entirely.

**Question 109's settlement is untouched.** Content lines still resolve to
exactly one purchase order line, `refines_expected_supply_id` stays scalar, and
J8's partition identity holds, because refinement is an ASN row refining a PO row
and has nothing to do with where the order reference was stated.

What changes is one sentence. The inbound analysis established that `S-O-T-P-I`
is not five containers because *"S and O are documents"*, and D24 (supply side)
reasoned from it. **S remains the assertion. O becomes a node.**

#### One order per logistic unit, which is the rule both markets already enforce

The 856's structures in production use are SOI, SOTI, SOPI and SOTPI, and in all
four the order level sits directly under the shipment and **above every physical
level**. The consequence is rarely stated and it is the useful half:

> A logistic unit belongs to exactly one purchase order. There is no well-formed
> 856 in which a pallet or carton spans two orders, because the tare and pack
> nodes are descendants of a single order node.

Metcash states the identical rule as prose: *"An ASN can only relate to a single
PO. This means that goods from different orders cannot be mixed within a
logistics unit."* The second sentence is the universal rule and the first is
Australia's addition.

Australia and the United States therefore **agree on the pallet** and differ only
on the message. That is one level narrower than the question assumed, and it is
why this is worth an invariant rather than a policy row.

#### Cardinality restrictions belong to the counterparty, not to the schema

Metcash forbidding an ASN that spans two purchase orders, Coles requiring pallets
homogeneous by expiry date, a retailer accepting only a SOPI structure: all three
are rules of a trading relationship, not properties of a despatch advice.

They belong in D22's lattice with the party as a scope, and they are checked as
D8 findings rather than enforced as constraints, because a constraint encoding
one customer's rule silently applies it to another customer's goods. Coles and
Metcash contradict each other outright on pallet composition, so this is not
hypothetical.

Stated as a rule here because it has now been reached twice from opposite
directions — outbound pallet composition and inbound message cardinality — and
two independent routes to the same rule is the reason to write it down once.

#### A delivery's receipts are one per demand document

One truck, one ASN, three purchase orders has two legal representations today.
Three receipts each naming a purchase order in the header, or one receipt with
the header demand null and lines carrying their own demand through
`expected_supply`. Both satisfy S3, both fold correctly under J26, and **nothing
chooses**, so both will appear and every query grouping by receipt will be right
for one and wrong for the other. The silent third option is drift, which is what
happens by default.

**Decision: one `goods_receipt` per delivery per demand document.** The 856's own
hierarchy then lands on tables that already exist:

| 856 level | Ours |
|---|---|
| Shipment | `inbound_shipment`; the truck is `vehicle_arrival` |
| Order | `goods_receipt` |
| Tare, Pack | `asserted_unit`, collapsing to `package` at receipt |
| Item | `asserted_unit_content`, becoming `goods_receipt_line` |

Australia is the degenerate case with one receipt per delivery, by contract
rather than by schema. The international case is the same structure with N
greater than one. No branch, no mode, no per-market column.

**`goods_receipt.inbound_shipment_id` is adopted.** D37 names the chain
`goods_receipt → inbound_shipment → in_force_assertion → asserted_unit` as the
cold path it designs around, and the column appears in no adopted DDL block. It
is what makes "the three receipts off one delivery" a query rather than an
inference.

**The header source set widens to mirror `expected_supply`.** Purchase order,
transfer order, return authorisation, inbound shipment, and none — at `<= 1` per
S3, which already caught this CHECK once as D16-repeats-D10.

#### Amendments to earlier decisions

- **D16** — `goods_receipt`'s header demand set widens to four sources at `<= 1`,
  and it gains `inbound_shipment_id`. The grain is one receipt per delivery per
  demand document.
- **D21** — `asserted_unit.level_code` admits non-physical levels. `raw_*` may
  not hold a value inherited from another node; where a standard states an
  attribute above the line, the node is stored.
- **D24 (supply side)** — the quoted *"S and O are documents"* is narrowed to S.
  Question 109's settlement stands unchanged and its reasoning gains the X12
  case.
- **D37** — the `goods_receipt → inbound_shipment` hop it designs around now has
  a declared column rather than an implied one.
- **S35** *(new)* — every `asserted_unit` node with a non-physical `level_code`
  has a NULL `sscc` and contributes no `package` row at receipt.
- **J47** *(new)* — no `package`, and no `asserted_unit` subtree, resolves to
  content lines naming more than one purchase order.

**Rejects.** Redefining `raw_po_reference` as "as exchanged at or above this
line" with a companion column naming the level it came from, which adds a
provenance column whose only job is to carry an apology and weakens a rule that
is currently absolute. An `asserted_order` sibling table, refused on D32's
grounds: a new table for something an existing tree already models. A CHECK
enforcing one purchase order per despatch advice, which encodes one
counterparty's rule for every counterparty. Per-market ingestion modes, which is
the branch this decision exists to avoid.

---

### D44 — Acknowledgement is two layers, and a counterparty's order is both

*Adopted 2026-08-04 from [outbound-edi-analysis.md](./outbound-edi-analysis.md),
answering three questions the GS1 pass raised and raising 127 and 128. The fourth
is refused in writing below.*

#### An acknowledgement is two things arriving by different mechanisms

Conflating them is how this gets built wrong, because they have different
subjects and different lifetimes.

**Transport and syntax.** EDIFACT `CONTRL`, X12 `997` or `999`. The interchange
arrived and parsed. Metcash expects *"an automated Functional Acknowledgement
(FA) at interchange level ... for all B2B documents"*, in both directions, for
every message type, within three hours.

This is a property of **the message**, not of the claim inside it.
`party_message` already carries `parse_status` and `parser_version`, which are our
parsing of an inbound message; their parsing of our outbound one is the mirror,
and the acknowledgement arrives as its own inbound `party_message`. So the link is
message to message: one nullable self-referencing `acknowledges_party_message_id`,
plus the acknowledgement's outcome. No new table.

**Application disposition.** EDIFACT `APERAK`, X12 `824`, or a proprietary
exception process. The business accepted or rejected the claim, with reasons, at
header and line level. Metcash rejects a whole despatch advice for spanning two
purchase orders, spanning two deliveries, reusing an ASN number inside 24 months
or naming a closed PO, and excepts a line for a GTIN not on the PO, an SSCC used
in the past twelve months, a quantity over the PO, a Ti/Hi mismatch or shelf life
outside agreed limits.

That is **a counterparty's statement of record about our claim**: authored by
them, held by them, quoted back in a chargeback. D21's definition unedited. It is
an inbound `assertion` whose body names the assertion it responds to.

**The symmetry is load-bearing and the model already had both halves.**
`assertion_stance` is *our* position on *their* claim, and it is a fact because it
is ours. Their position on *our* claim is *their* assertion, and it is immutable
because it is theirs. Only one half has ever been used.

#### D5 survives, and the rule is D8's

> *"If the supplier does not receive a FA from Metcash after sending an ASN, the
> supplier should not despatch stock against that ASN."*

A counterparty-imposed block on our own physical act. D5 forbids the ledger
blocking on coordination, and the two do not collide: D5 governs what the ledger
refuses to record, and this governs what the floor should do.

**Despatching against an unacknowledged advice raises a finding, not a lock.**

Two reasons beyond consistency with D8. A lock is unenforceable — the truck leaves
whether or not the software agrees — so it would produce an *unrecorded* despatch,
which is strictly worse than a recorded one carrying a finding. And the
requirement is per-counterparty: Metcash states it, and it has no force for a
customer who sends no acknowledgement at all.

That makes it the same shape as question 126, whose wording widens from
message-cardinality rules to **counterparty message rules** generally, so
acknowledgement-required and structure-required resolve wherever 126 lands.

#### A retailer's purchase order is an assertion and an order

D39 settled that orders are ours. D21's cut is that a copy exists outside our
control. For a retailer-issued purchase order both are true and neither displaces
the other: the `ORDERS` message is a document the retailer authored, holds and
will quote back, and the `order` row is our own intention.

**This is the `inbound_shipment` pattern one direction over**, and D21 already
reasoned it: *"`inbound_shipment` is a subject, not an assertion. Filing it as an
assertion means a resend mints a second row and orphans every FK pointing at the
first."* A customer purchase order has the identical failure mode and takes the
identical fix. The document is an assertion, the `order` is the subject, and the
body carries `resolved_order_id` the way `despatch_advice` carries
`inbound_shipment_id`.

**The kind set already held the reply and not the message.** `assertion.kind` ran
`despatch_advice | carrier_status | equipment_docket | delivery_receipt |
order_response | price_advice`. `order_response` is the outbound ORDRSP and
nothing held the ORDERS it answers, which is a gap the enumeration pointed at.
It gains `purchase_order` and `document_response`.

**An externally-authoritative order is amended by succession, not by amendment.**
Metcash *"will NOT be implementing an EDI Purchase Order Change (POC) message"*,
and instead *"there may be a need for the Stock Controller to cancel the PO and
re-raise a new PO in its place."* D42 made an amendment to an intention a fact,
which is right for orders we author; D39 already said an externally-authoritative
order is amended through its own system. What was missing is the link, so a
cancel-and-reraise pair was two unrelated rows and the second one's history
started from nothing. `order` gains a nullable `supersedes_order_id`, the same
shape as `assertion.supersedes_assertion_id`.

#### Price is named, and deferred as its own question

The Metcash purchase order acknowledgement confirms **GTIN, PO line number,
price, quantity, pack size, Ti/Hi, unit of measure and shelf life**, and *"the
amount payable is calculated by reference to POA confirmed price and the quantity
received."* Every one of those has a home except price: `order_line` is
`id, order_id, item_id, quantity_ordered`.

**This is not the scope D40 declined.** D40 kept invoice rendering out and had the
system produce charge lines and their inputs. A price on an order line is upstream
of that — a term of the intention, which is what the counterparty asks us to
confirm.

It is **not settled here**, because it touches D40's boundary, the deferred
`stock_movement.unit_cost_minor` and whatever a rate card becomes, and settling a
commercial question by implication inside an EDI decision is how boundaries get
moved without anyone deciding to move them. Question 127 carries it, with the
cheap half stated: `order_line` wants a price and a currency before the first
grocery order, and history before the column exists is not recoverable.

#### The GTIN issuer: a namespace is not always a sequence

Recorded as a **refusal with a mechanism** rather than built, on the pattern D24
(supply side) used for `supply_custody_change`.

`number_range` is `next_value` plus `block_size` claimed under `FOR UPDATE`, which
assumes a computable serial space. Australian GTIN namespaces are not all that
shape. A GTIN-13 from our company prefix is a sequence; a GTIN-8 is individually
allocated by GS1 Australia; the Individual Barcode Number tier is one to ten
granted numbers with nine more on application; variable measure numbers come from
restricted-circulation prefixes under member-organisation rules; a UPC company
prefix is a separate entitlement.

**A pool is not a range with a next value.** Modelling granted numbers as a
sequence means inventing a `next_value` over numbers we were handed, and the
first tenant on the Individual Barcode Number tier breaks it. The mechanism, when
it is built: granted numbers are **pre-created allocation rows marked unissued**,
so the fact table is the pool and `number_range` covers only the namespaces that
genuinely are sequences. D29's row-locked claim path for SSCCs is untouched.

Two constraints on whoever builds it. Indicator digits 1 to 8 produce higher
packaging levels from the same item reference, so allocating a fresh reference for
a carton burns the scarce namespace eight times faster, which D34 already implies
and the issuer must respect. And the non-reuse obligation is **permanent**, which
`retention_floor.minimum_age` cannot express.

#### The National Product Catalogue is out of scope, and here is why

The logistics half is derivable from the model today. `observable`'s
`(item_id, packaging_level, item_packing_config_id)` triple is the per-level
subject NPC needs, versioned so a corrected case pack cannot rewrite the
dimensions of cartons shipped last year; gross, net and tare are three metrics
because GS1 settled it; Ti and Hi are `item_packing_config`. **This is a scope
decision, not a capability gap**, and saying so is the point of writing it down.

What makes it the wrong subsystem to own: it carries price, which the model does
not hold; a published value is not a fold but the value we told a retailer plus
the history of what we told them and when, which is a projection and a publication
log serving an external catalogue; and its attribute surface is classification and
marketing copy, which under D26 would take several extension schemes past D36's
sixty-field ceiling. A warehouse model growing several schemes to hold marketing
copy is the shape of a mistake, and D36's ceiling exists to make that visible
rather than gradual.

**The precedent is D40.** Hold and export the logistics attributes we observe; let
a product-information system publish. The obligation accepted here is the export,
not the catalogue.

#### Amendments to earlier decisions

- **D21** — `assertion.kind` gains `purchase_order` and `document_response`. A
  `document_response` body names the assertion it disposes of. The direction rule
  is stated: a counterparty's disposition is always of a claim of the opposite
  direction.
- **D25** — `assertion_stance` is unchanged and is explicitly *ours*. A
  counterparty's stance on our claim is their assertion, never a stance row.
- **D26 / principle 3** — `party_message` gains `acknowledges_party_message_id`,
  nullable and self-referencing. No payload interpretation moves out of `bytea`.
- **D31** — `retention_floor` admits a floor with **no expiry**. GTIN non-reuse is
  permanent and an interval cannot say so. S34's history-depth companion is
  unsatisfiable for such a floor by any finite archive window, which is stated
  rather than discovered.
- **D39** — an externally-authoritative order is amended by succession;
  `order` gains `supersedes_order_id`.
- **D40** — the National Product Catalogue joins the exclusions, with the
  logistics export named as in scope.
- **D42** — the amendment fold is for orders we author. It does not apply to an
  order whose record of authority is external.
- **J48** *(new)* — every outbound `party_message` on a channel whose party
  profile requires acknowledgement has one, or a finding naming it.
- **J49** *(new)* — no `document_response` assertion names a subject assertion of
  the same `direction`.

**Rejects.** One acknowledgement mechanism covering both layers, which would put a
syntax error and a rejected line in the same column and make "was it accepted" two
different questions with one answer. A status column on the outbound assertion,
refused by D21 for the same reason it was refused inbound. Filing a counterparty's
disposition as an `assertion_stance` row, which would make our fact table hold
their claim. Modelling granted identifier pools as sequences. Building the GTIN
issuer now, when nothing issues one. Settling price inside an EDI decision.
Publishing to the National Product Catalogue.

---

### D45 — `goods_receipt_line`, and the invariant that folds a column which does not exist

*Adopted 2026-08-04. Three decisions depend on this table's columns and none
defines it, which is where `item_barcode` sat before D34 and `device` before D27.
Defining it corrects J26.*

#### What depends on it today

D23 cites `goods_receipt_line.expected_quantity` as the precedent for freezing a
resolution on first use. D24 (supply side) makes it a cause arm on
`stock_movement`. J26 folds `expected_supply.quantity_received` across it. The
inbound analysis sketched it before `expected_supply` existed. Nothing adopted
says what it holds.

```
goods_receipt_line            -- GROUPING
  id, tenant_id, goods_receipt_id
  item_id (NOT NULL)          -- unresolvable content is a finding, not a row

  expected_supply_id          -- nullable: NULL is a blind or unexpected line
  expected_quantity           -- SNAPSHOT at capture, never read live

  entered_quantity, entered_unit_id, item_packing_config_id
  lot_id                      -- nullable; absence is a finding, never a refusal

  accepted_at, accepted_by_id
  rejected_at, rejected_by_id, rejected_reason_id
  matched_at, matched_by_id   -- a blind line reconciled to supply afterwards
  recorded_at, client_event_id, recorded_by_id / automation_key
```

#### One supply arm, because the sketch predates `expected_supply`

The inbound sketch gave the line two nullable arms, `purchase_order_line_id` and
`asserted_unit_content_id`. Those are two of the four arms that D24 (supply side)
later unified into `expected_supply`, whose whole purpose is that a promise of
goods arriving has one identity regardless of which document produced it.

Carrying the old pair now would rebuild the union one level down, and it would
break on the two arms the sketch never had. A transfer receipt and a return
receipt would each need a third and fourth column, and D37's shipment-scoped
screen would have four join paths instead of one.

**So the line names `expected_supply` and nothing else.** Blind receipt sets it
NULL, which is the same `<= 1` argument the model has now made three times: an
internal move has no demand-side cause, an unsolicited delivery has no demand-side
cause, and S3 already forbids writing that as `= 1`.

**The refinement case works out.** An ASN row refining a purchase order row is the
child; the receipt names the child; J26's fold reaches the parent through
`refines_expected_supply_id`. The operator receives against the delivery and the
purchase order's outstanding quantity falls, with no second write.

#### There is no received quantity on this table, and that is the point

Received is `SUM(stock_movement.quantity)` grouped by `goods_receipt_line_id`,
which is a batch load rather than an N+1 because D10 made the cause a typed FK.

A stored accumulator is the defect the inbound analysis found shipped in a
competitor, where a documented double-count bug follows directly from keeping a
running total beside the ledger that produces it. Here the total is structurally
impossible to disagree with the movements, because there is nowhere to write it.

**This is what corrects J26.** As adopted it reads *"`expected_supply.
quantity_received` = the fold of `goods_receipt_line` rows naming this row or any
row refining it"*. There is nothing on a `goods_receipt_line` to fold. The
invariant names the right relationship and the wrong table, and it would have been
written as a query that sums a column somebody then had to add.

> **J26 (corrected).** `expected_supply.quantity_received` = the fold of
> `stock_movement` rows whose `goods_receipt_line_id` names this supply row or any
> row refining it.

That is the J8 shape once more, and worth counting: an invariant that cannot fail
because it cannot run. It was caught by defining a table rather than by reviewing
the invariant, which is an argument for defining tables before writing checks over
them.

#### `expected_quantity` is a snapshot, and so is the packing config

`expected_quantity` is copied onto the line at capture and never read live.
Reading it live makes variance unreproducible the moment a purchase order is
amended, and the variance is the output D8 exists to produce. Precedent is
`stock_count.challenge_context` and `package` dimensions frozen at seal.

`item_packing_config_id` is the same idea for units and it is the tier-0 item that
has no other home. `entered_quantity` and `entered_unit_id` record what the
operator actually keyed, forty-eight of something, and the config says what that
meant on the day. D23 already versions `item_packing_config` by `effective_from`
for exactly this reason; without the FK on the receipt, correcting a case pack
silently rewrites the meaning of every historical receipt, which is the half of
the mechanism that was missing.

#### Acceptance is a fact with a time on it, never a status column

`accepted_at` and `rejected_at` are timestamps with people attached, not a status
enum, because acceptance extinguishes a right and the moment it happened is the
thing in dispute. Under the Food and Grocery Code of Conduct fresh produce may be
rejected only within 24 hours of delivery and only if not already accepted, so
"accepted at 14:32 by this person" is the record that matters and a mutable
status column cannot hold it.

Where the clock lives is question 62's, unchanged. This decision only insists the
timestamps exist to hang it on.

`goods_receipt.status` stays a projection over lines and movements, per D25.

#### What is deliberately not on it

**No `package_id`.** D24 made containment a fold over `package_event` and retired
`package_content` as a base table. A package reference here would be a second
representation of where the goods went, and the receiving LPN is already on the
`stock` cell.

**No `quantity_received`, no `variance`.** Both are folds. A variance column would
additionally freeze a comparison whose inputs move.

**No `blind` flag.** It is on the header, captured rather than inferred.

**No status.** See above.

#### Amendments to earlier decisions

- **D16** — `goods_receipt_line` is defined. The demand link is
  `expected_supply_id`, singular and nullable, replacing the inbound sketch's two
  arms.
- **D23** — the freezing precedent it cites now points at a defined column.
- **D24 (supply side)** — the cause arm on `stock_movement` names a defined table;
  **J26 is corrected to fold `stock_movement` rather than the receipt line.**
- **D37** — the receiving screen's line list scopes through `expected_supply`, and
  the receipt line's single supply FK is what keeps that one join rather than four.
- **D44** — one `goods_receipt` per delivery per demand document, so a line's
  supply row always resolves within its header's document.
- **S36** *(new)* — no table in the receipt set carries a stored received quantity,
  variance or accumulator column. Received is a fold over `stock_movement`.

**Rejects.** `purchase_order_line_id` and `asserted_unit_content_id` as separate
arms, which rebuilds `expected_supply`'s union one level down and breaks on the
transfer and return arms. A stored `quantity_received`, which is the competitor
bug written into the schema. A `variance` column. A status enum in place of
acceptance timestamps. `package_id`, refused because containment is a fold.
Reading `expected_quantity` live from the purchase order, which makes historical
variance unreproducible.

---

### D46 — A zone is a name for a set of locations, and everything else about it is a policy

*Adopted 2026-08-04, completing D22's prerequisites and raising 129. The last
thing standing between the decision record and the first migration.*

D22 asked for it in one line: *"`zone` becomes a real table (`zone(id, site_id,
code, …)`, `location.zone_id`), not a bare column, the Space dimension needs
something to FK to and a depth to read."* The interesting part is the `…`, and the
answer is that almost nothing goes in it.

```
zone                          -- REFERENCE. Tenant-scoped (D19 shape 3).
  id
  tenant_id (NOT NULL)
  site_id (NOT NULL)
  code, name
  active
  FK (site_id, tenant_id) -> site(id, tenant_id)
  UNIQUE (tenant_id, site_id, code)
  UNIQUE (id, site_id)        -- composite target for location's FK below

location                      -- amended
  zone_id                     -- nullable: a dock belongs to no zone
  FK (zone_id, site_id) -> zone(id, site_id)
```

#### It is thin because the lattice is the place to say things

The temptation is a temperature class, a pick sequence, an equipment restriction,
a pickable flag. Every one of those belongs somewhere else and putting it here
would be the accretion this model exists to refuse.

Temperature, priority and putaway preference are **policies bound to the zone**,
which is what D22 built the Space dimension for: *"vendor X's goods go to zone 3"*
is a `putaway` binding, and D22 already names it as the example of a rule that
evaporated once the lattice existed. A `zone.temperature_class` column would be a
second way to say the same thing, and the two would disagree.

Pickable, blocked, active, sequence and reachability are **properties of a
location**, which already carries `kind`, `reachable_by` and its coordinates. A
zone that repeated them would be a grouping pretending to be a thing.

So a zone is an identity, a membership and a name. **Everything you want to say
about a zone is said by binding a policy to it**, and that is the whole reason the
Space dimension exists.

#### Flat within a site, which is what the lattice already says

D22's Space dimension is `any → site → zone`, two nodes, no ancestors. Product is
the only dimension carrying an ancestors level, because item taxonomies genuinely
nest and the closure table earns its cost there. Counterparty is flat for the same
reason zones are: the set is small and the nesting is usually a naming convention
rather than a structure.

Real warehouses do sometimes nest, a chilled area holding a chilled pick face and
a chilled bulk run. **If that arrives it costs a closure table and one lattice
row**, exactly as Product has, and the precedent is written rather than invented.
Question 129 carries it, deferred against a tenant that actually has sub-zones,
because building the tree now means maintaining a closure table over a handful of
rows for a shape nobody has asked for.

#### `location.zone_id` is nullable, and the composite FK is load-bearing

A dock belongs to no zone. Neither does a staging lane, in most layouts. Making
the column NOT NULL forces every site to invent a catch-all zone, which is a
fiction that then has policies bound to it.

A NULL simply means zone-scoped bindings do not match that location, which is the
correct answer rather than a special case.

**The composite FK is the part a reviewer will try to simplify.**
`FK (zone_id, site_id) -> zone(id, site_id)` stops a location joining a zone that
belongs to a different site. Written as a plain `zone_id` FK it permits exactly
that, and the failure is silent: every zone-scoped policy resolution for that
location returns another site's answer, and nothing complains because both rows
exist and both are valid on their own. It is declarative, so D25's ban on
validation triggers is untouched, and it needs the `UNIQUE (id, site_id)` on
`zone` to have something to point at.

J15 already forbids a *binding* naming a zone outside its site. This forbids the
same disagreement one level down, in the data the binding resolves against, which
J15 cannot see.

#### `tenant_id` is denormalised on purpose

A zone reaches its tenant through its site, so the column is redundant. It is
there anyway for two reasons: D19's third RLS shape wants `tenant_id NOT NULL` on
tenant-scoped reference data, and J14 asserts that every scope FK resolves within
the binding's tenant, which is a local join with the column and a two-hop join
without it. The composite FK to `site(id, tenant_id)` makes disagreement
unrepresentable rather than merely unlikely.

#### Amendments to earlier decisions

- **D22** — the Space dimension has a real table behind it. Its prerequisites are
  complete.
- **D19** — `zone` joins the tenant-scoped reference set, RLS shape 3.
- **D25** — the site and zone agreement checks are foreign keys, not triggers.
- **S37** *(new)* — `location.zone_id` is a composite foreign key including
  `site_id`, and `zone` carries the matching unique key. Asserted from
  `pg_constraint`, because replacing it with a simple FK silently allows a
  location to sit in another site's zone.

**Rejects.** `zone.temperature_class`, `zone.pick_sequence`, `zone.pickable` and
the rest, all of which are either a location's property or a policy bound to the
zone. A nested zone tree with a closure table, deferred to 129 rather than built
for a shape nobody has. `location.zone_id NOT NULL`, which forces a fictional
catch-all zone per site. A plain `zone_id` foreign key, which permits a
cross-site zone silently. A `zone` without `tenant_id`, which would make J14 a
two-hop join and D19's RLS shape non-uniform.

### D47 — A correction carries the moment it is about, not the moment it was found

*Adopted 2026-08-05, building the clause D8 wrote and nine migrations did not.
Migration 10. Raises 130 and 131.*

D8 settled the principle in one sentence: *"Reconciliation does not edit or
delete movements. A correction is a **new** movement carrying
`reverses_movement_id` and a reason."* The column did not exist. The competitor
analysis had also been carrying it as gap 15 since before there was a schema, so
the sentence had been read twice as a design and was still a promise.

Building it forces the question D8 did not ask: **which `occurred_at` does a
correction carry?**

**Two kinds of wrong, and only one of them is a correction.**

Four units are damaged after receipt. Something happened; a new movement records
it; nothing was wrong. Against that, an operator types 36 when there were 32.
Nothing moved and nothing happened. There were never 36.

Recording the second as an ordinary movement asserts that four units physically
moved at the moment somebody noticed. Handling volume and labour reports then
count it, and "we damage a lot of stock" becomes indistinguishable from "we
miscount a lot", which is the difference between a packaging problem and a
training problem. The presence of `reverses_movement_id` is therefore the whole
discriminator: **set means we were wrong, null means the world changed.**

**The answer: the target's `occurred_at`, with its own `recorded_at`.**

A receipt at 09:14 corrected at 11:30 produces a correction stamped
`occurred_at` 09:14, `recorded_at` 11:30. Two questions stay separately
answerable off one ledger:

| | filter | answer |
|---|---|---|
| What did we believe at 10:00? | `recorded_at <= 10:00` | 100 |
| What was true at 10:00? | `occurred_at <= 10:00` | 96 |

Stamping the correction 11:30 answers only the first, permanently. No query
recovers what was actually in the bin, because the system is asserting a
physical movement at the moment of discovery.

None of this is new machinery. Both columns already existed, D24 already folds in
`(occurred_at, recorded_at, id)` order, and J6 and J46 already assert that
arrival order does not change the result, so a correction is one more
out-of-order arrival.

**The payoff is a metric that would otherwise need instrumenting.**
`recorded_at - occurred_at` on a correction is the time the wrong number stood.
Time to detection is the measure the whole verification argument rests on, and it
falls out of the two timestamps without a counter anywhere.

**The mechanism is the fourth, and that is correct.** The record now corrects
four different things four different ways, which reads as inconsistency until the
cases are named. A `measurement` does not point backward, because a newer
weighing does not make the old one false, only stale. An `assertion` points
backward through `supersedes`, because superseding a claim does make the old one
false. An `intention_amendment` folds, because an order is amended in parts. A
movement points backward at one row, because a correction is about one fact. Four
kinds of wrong, four shapes.

**Why the vocabulary is split rather than listed.** `reason` is a movement type,
so "adjustment, damaged" and "adjustment, miscounted" are the same row today.
`adjustment_reason` is therefore keyed by an `adjustment_class` of exactly
`record_error` or `world_event`, and a reason of the wrong class is inadmissible
in either direction. A single flat list would rebuild the ambiguity inside the
fix.

**The trigger that S7 rejected.** Four rules govern a correction, all four
compare it to the row it corrects, and Postgres has no cross-row CHECK. This
migration was written with a `BEFORE INSERT` guard holding them. S7 failed on it
immediately, and S7 is right: D25 forbids triggers that implement rules,
validation, defaults or cascades, and validation is the named case.

The letter of the rule was the weaker argument. `stock_movement` is the highest
volume table in the schema and a guard costs it a `SELECT` per insert on the one
path that has to stay fast. Worse, a trigger is the most quietly disarmable
object in Postgres, since `DISABLE TRIGGER` leaves the row in `pg_trigger`, and
the first draft of S38 was written to watch `tgenabled` for exactly that reason.
Needing a second mechanism to watch the first is the argument against the first.

The rules moved to `crates/server/src/correction.rs`, as a pure function over the
target row and the proposed correction, with ten tests and no database. The write
path reads the target anyway to mirror its sides, so it costs nothing that was
not already being paid. It returns every problem rather than the first, because a
person fixing one and resubmitting to find another is how people learn to stop
correcting.

**Rejection is admissible here, and only here.** D5 refuses to reject the floor's
observations, because the scanner is more authoritative than the database and a
refused scan stops work. A correction is not an observation of the floor. It is a
claim about the record, and refusing a malformed one blocks nothing: the operator
can still record what they see as an ordinary movement, which is the path D5
protects.

**Correcting must not need approval.** Contradiction has to stay as cheap as
confirmation, or people stop correcting and the ledger rots while looking
healthy. Anyone who can record can correct, and the correction carries who, when
and why. A high correction rate against one item or one station is a finding
about that item or station, never a disciplinary input, on the same grounds that
rule out a per-operator accuracy leaderboard.

**Amendments.**

- `adjustment_class`, an enum of `record_error` and `world_event`.
- `adjustment_reason`, a shared-reference table in D19 shape 2, seeded with ten
  entries and a guard that fails the migration if it seeds none. Migration 7
  seeded zero rows through a join to an unpopulated table and said nothing.
- `stock_movement.reverses_movement_id`, a self-referencing FK, and
  `stock_movement.adjustment_reason_id`.
- Two in-row CHECKs: a correction states a reason, and nothing corrects itself.
- A partial index on `reverses_movement_id`.
- **S38.** The FK is self-referencing and both CHECKs are present.
- **J50.** Every correction carries its target's `occurred_at`, mirrors its
  target's sides, names the same item and tenant, and gives a `record_error`
  reason.
- **J51.** Corrections against a movement never exceed it in total quantity. A
  finding rather than enforcement, because two concurrent corrections each see a
  pre-image without the other, and the lock that would prevent it is the
  coordination D5 exists to refuse.
- **J52.** `reverses_movement_id` is acyclic.
- The fixture gains a correction, so all three examine a row rather than passing
  on an empty population.

**Rejects.** A compensating movement with no back-reference, which nets to the
right number while asserting a physical event that did not happen. A correction
stamped at its discovery time, which is the same information loss dressed as a
timestamp. Full-reverse-and-restate as the only shape, which forbids the partial
correction that is the common case. A single flat reason list. A validation
trigger. Approval on corrections. A `corrected` flag on the original row, which
is an UPDATE to a fact and the thing this whole model refuses.

### D48 — The order side was answering the wrong question, and getting the wrong answer

*Adopted 2026-08-05, settling question 130 raised by D47. Migration 11. Raises
132, and narrows J44.*

D47 split a movement by whether the world changed or the record was wrong.
Question 130 asked whether `intention_amendment` needs the same split. It does,
and the argument is stronger, because this is not a loss of history. **It is a
wrong current value.**

**The failure.** An order is placed at 01:00 promising Thursday, which is a typo:
the customer said Friday. At 02:00 they genuinely move it out to Saturday. At
03:00 the typo is found. The only shape expressible today is an amendment stamped
03:00, and J46 folds in `(occurred_at, recorded_at, id)` order, so it sorts last
and wins. The order now promises Friday. The customer is expecting Saturday.
Every invariant passes.

Run against the schema before this decision, that is exactly what happens:
`promised_to` comes out `2026-08-07` when the customer asked for `2026-08-08`.

**The fix is already in the ordering key.** Carrying the corrected moment's
`occurred_at` puts the fix at 01:00, where the fold reads it *before* the 02:00
change, and the answer comes out Saturday. D24 chose
`(occurred_at, recorded_at, id)` for exactly this reason. What was missing was
any way to say that an amendment is about an earlier moment than the one it was
written in, and any way to tell that such an amendment had been written.

Both readings then work, off one table:

| | filter | answer |
|---|---|---|
| What did we believe at 01:30? | `recorded_at <= 01:30` | Thursday, the typo, still standing |
| What was true at 01:30? | `occurred_at <= 01:30` | Friday |

And `recorded_at - occurred_at` is again the time the wrong value stood.

**Why it cannot be left to convention.** Both amendments are legal rows with
legal timestamps and no check can tell them apart after the fact. The difference
between them decides what the warehouse ships.

**The discriminator is a column, not a foreign key.** On the movement side,
`reverses_movement_id` names the row being corrected and the class falls out of
its presence. An amendment has no such row: the value it revises may be the
order's own original, which is not an amendment at all. So `revision_class` is
explicit and `NOT NULL`, and migration 11 adds it with a default only long enough
to backfill and then drops it. **The absence of the default is the part S39
asserts**, because restoring it is a one-line change that makes every unthinking
correction a `world_event` again.

**One question, one type.** Migration 10 named this `adjustment_class`, which is
movement vocabulary for a distinction that is not about movements. *Was the world
different, or was the record wrong* is the same question wherever a fact can be
revised, and it has now come up twice in two migrations. The type is renamed
`revision_class` and shared. Two enums with identical values and different names
is the near-duplicate this record keeps refusing.

**J44 was too wide by exactly one case.** D39 allows one system of record per
order, and J44 forbids amending an externally-authoritative order locally. That
was written before this split existed. If we mis-parse a quantity out of an
inbound ORDERS message, the counterparty's document is not wrong and their
intention has not changed. **Our transcription of it is wrong**, and forbidding
the fix leaves knowingly wrong data in place with no alternative, since
succession is their mechanism and we cannot cancel their purchase order to fix
our own parse bug. J44 narrows to `world_event`. A local amendment claiming the
intention changed is still forbidden, because that is the bidirectional merge D39
refused outright.

**What this leaves open.** J44 cannot be implemented either way, because nothing
declares which `source_channel` values are externally authoritative. It is a free
text column and the property is currently read out of it by convention. That is
question 132, and it now blocks an invariant rather than sitting as tidiness.

#### Amendments to earlier decisions

- **D47** — `adjustment_class` is renamed `revision_class` and is no longer
  movement-specific. Values are unchanged.
- **D42** — `intention_amendment` gains `revision_class NOT NULL` and a CHECK
  that a `record_error` states a reason. The fold is untouched: the class is
  metadata for people and for findings, and J46 sorts by time as before.
- **J44** *(narrowed)* — no externally-authoritative order carries a
  `world_event` amendment. A `record_error` amendment is permitted.
- **S39** *(new)* — `revision_class` is `NOT NULL`, carries no default, and is of
  the shared enum type; the correction-reason CHECK is present.
- **J53** *(new)* — every `record_error` amendment carries an `occurred_at`
  matching its order's `placed_at` or another amendment's `occurred_at` on the
  same order. A correction has to name a moment at which something was set.

**Rejects.** Leaving the distinction to the `reason` free text, which is the
`field`/`before`/`after` soup D42 refused wearing a third label. A second enum
duplicating D47's values. A `corrects_amendment_id` foreign key, which cannot
name the order's original value and so fails on the commonest case. Enforcing the
`occurred_at` rule in a trigger, refused for the same reasons as D47. Widening
J44 to forbid nothing, which would permit the bidirectional merge D39 refused.

### D49 — A guard is only as good as its marking, and nothing was checking the marking

*Adopted 2026-08-05, found while researching question 127. Migration 12. Raises
134. 127 itself is not settled here.*

Question 127 wants a price on `order_line` and a currency on `order`. Deciding
the grant for a new column on `order` meant reading the existing one, and the
existing one was wrong in a way three separate guards were built to prevent and
none of them could see.

**The defect.** D42 demoted four `order` columns to `@projection` of
`intention_amendment`, and `projection_order_rebuild` maintains them. Migration 9
did none of the three things that make that real: no registry rows, no column
comments, and a table-wide `UPDATE` grant to the application. It wrote the
comment explaining the danger four lines above the grant that caused it.

**Why nothing caught it.**

**S5** was `Pending("the rebuild function registry does not exist yet")` while the
registry existed and held eighteen rows. A check whose stated precondition has
since been met is a third failure mode for this suite, after a check that was
wrong and a check that examined nothing. This one reads as deliberate.

**S5 as stated would not have caught it anyway.** The register described two
directions, comment to registry and registry to column. Both held for `order` by
both sides being empty. The rebuild function was the only witness, so the diff is
now three-way, and a `projection_%_rebuild` function with no registry rows is a
projection nobody declared.

**J36 could not fire at all.** It read `information_schema.role_table_grants`,
which is wrong here twice over: the view restricts to grants the current user is
grantor, grantee or a member of, so a superuser running the suite sees none of
the application role's; and it carries table-level grants only, so a column-level
`GRANT UPDATE (quantity) ON stock`, the exact shape D25 prescribes everywhere,
never appears. What it actually asserted was that no table-level grant existed on
a table containing a projection column, which is why it caught the `package` case
once and nothing since. Granting the application `UPDATE (quantity)` on `stock`
was verified to pass before the rewrite and to fail after it.

**Two claims were wearing one marker.** Running the three-way diff for the first
time found `consignment.status`, `.eta` and `.price_minor` commented against a
carrier advice that does not exist, and `fulfilment.progress` and
`fulfilment_line.allocated_quantity` registered against
`projection_fulfilment_rebuild`, **a function migration 9 never wrote**.
`@projection` was asserting both that a rebuild owns the column and that the
application must never write it. The second is true of all five now; the first is
true of none. `@projection(pending)` carries the second alone, and the guards
keep working because they match on the prefix.

Nothing here invents a rebuild function to make a check pass. That is question
134.

#### Amendments to earlier decisions

- **D25** — the projection diff is three-way, and `@projection(pending)` is a
  declared state rather than an omission.
- **D42** — `order`'s four covered columns are registered, marked and granted by
  column, which is what the decision said and migration 9 did not do.
- **S5** *(implemented, widened)* — the third leg, function to registry.
- **J36** *(rewritten)* — `has_column_privilege` rather than a view with a
  visibility predicate, `UPDATE` and `DELETE` rather than `INSERT`.

**Rejects.** Writing `projection_fulfilment_rebuild` inside a migration named for
something else, which is how the registry came to name it in the first place.
Deleting the marker from the consignment columns, which would drop the protection
that is genuinely wanted. Keeping `rolcanlogin` in J36, which made the check
depend on deployment state the migrated database never has. Checking `INSERT` on
a projection column, which would stop an order carrying an initial promised
window at all.

### D50 — A price is a term of one order, and everything around it is a different thing

*Adopted 2026-08-05, settling question 127 raised by D44. Migration 13. Raises
135.*

Metcash's purchase order acknowledgement confirms **GTIN, PO line number, price,
quantity, pack size, Ti/Hi, unit of measure and shelf life**, and *"the amount
payable is calculated by reference to POA confirmed price and the quantity
received."* Every one of those had a home except price.

D44 deferred it rather than adding a column, on the grounds that a price column
looks like it settles a commercial question and settling one by implication is
how boundaries move without anyone deciding to move them. So the boundary comes
first.

**What a price is not.**

| | |
|---|---|
| A **cost** | `stock_movement.unit_cost_minor` stays deferred. What we pay for goods and what a customer pays us are different numbers reached by different means, sharing only a data type. |
| A **rate** | D40 put `storage_rate` and `handling_rate` under D22's lattice, most-specific-wins over client, item class and site. A rate is a policy that produces a charge; a price is a term of one order agreed with one counterparty, and in the lattice it would resolve for orders that never agreed it. |
| A **total** | D40 keeps invoice rendering out. An extended amount is quantity times price, and a stored one can disagree with its own inputs. S40 asserts the absence, the same way S36 does for received quantity. |
| **Tax** | `unit_price_minor` is tax-exclusive, stated rather than assumed. A column silently inclusive in one place and exclusive in another is a commercial bug with no symptom until an invoice is wrong. |

**Currency is on the order.** One order, one currency. A line priced differently
from its order is not a thing anyone sells, and the alternative is a per-line
column that only ever agrees with itself. The existing non-goal stands: currency
is recorded and nothing converts. J54 carries the pairing across the row
boundary, since no CHECK can reach both tables.

**Price is an integer, and the basis quantity is what makes that survive
grocery.** Minor units as `bigint`, matching `consignment.price_minor` and
`record_scheme_field`'s `money_minor`. A price of 3.45 per 100 is exact as
`(345, 100)`; as a per-each price it is 0.0345, which cents cannot express and
which would force either a decimal type or a silent rounding. It also matches how
the commercial document reads, since a counterparty quotes per some quantity
rather than per each, and EDIFACT's PRI segment carries exactly this pair.

**One column, not an ordered/confirmed pair.** Metcash's wording separates the
price on their purchase order from the confirmed price on our acknowledgement,
and payment references the second. An acknowledgement that could not differ from
the order would not be worth sending, so both numbers are real. Only the term in
force belongs on the line. **The price as received is a statement of record
exchanged with another party, stored exactly as exchanged, which neither side may
unilaterally revise**, which is D21's definition of an assertion, and it lands
there when the inbound migration exists with J18's shape applying. A second
column here would be the assertion layer built badly, in the wrong place, with no
writer until inbound EDI exists.

**What this leaves.** `intention_amendment` is `order_id`-only with four
order-level covered columns, so a price change to a line has nowhere to go and
moves by UPDATE like any other intention column. D42's own sketch listed
`quantity_changed`, `line_added` and `line_removed` in a `kind` enum the built
table does not have, so the schema is narrower than the decision that specified
it, and this is the first thing to run into that. Question 135. Settling it makes
`unit_price_minor` a `@projection` under D42 and lets D48's `revision_class`
separate a renegotiated price from a mis-keyed one.

#### Amendments to earlier decisions

- **D15** — `order` gains `currency`; `order_line` gains `unit_price_minor` and
  `price_basis_quantity`.
- **D40** — the boundary is asserted rather than described. Charge lines and
  their inputs stay out of the order set.
- **D44** — the price question it named is closed.
- **S40** *(new)* — no stored line total, extended amount or tax column in the
  order set.
- **J54** *(new)* — no priced `order_line` on an `order` naming no currency.
- `consignment.currency` gains the format CHECK it has lacked since migration 9,
  because one validated currency column beside an unvalidated one is the
  inconsistency this record keeps refusing.

**Rejects.** A per-line currency. A decimal or floating price type, when the
basis quantity makes integers exact. An `ordered_price`/`confirmed_price` pair,
which is D21's assertion table built badly in the wrong place. A stored extended
amount or line total. A `tax_minor` column, which would make the tax-exclusivity
of the price ambiguous to anyone reading the schema. Putting price in D22's
lattice, which would resolve a price for orders that never agreed one. Settling
`stock_movement.unit_cost_minor` here, which is the same mistake D44 declined to
make in the other direction.

---

### D51 — An amendment names one subject, and until now it could not name a line

*Adopted 2026-08-06, settling question 135 raised by D50. Migration 14. Raises
136 and 137.*

D42 established that an amendment to an intention is a fact and gave the sketch:
`kind` values of `quantity_changed`, `line_added` and `line_removed` alongside the
order-level ones. Migration 9 built the order-level half. **The schema has been
narrower than the decision that specified it ever since, and price was simply the
first thing to walk into it.** D50 could add `unit_price_minor` and could not make
it a `@projection`, because no amendment could name the column.

**What was actually at stake.** A line's quantity and its price are the two
numbers a counterparty disputes. *"We ordered 40, not 60"* and *"we agreed 3.20,
not 3.45"* are the arguments that happen, and under the previous schema the answer
to both was an `UPDATE` with no author, no moment and no reason — on a table whose
whole design premise is that the order is the significant mutable entity left.

**One amendment names one subject.** `order_line_id` is nullable and the covered
columns split by it: an amendment carries the order-level four or the line-level
four, never both. Letting one row carry both was refused because the fold groups
by subject, so a row in two groups has to be folded twice under two keys to mean
what it says. Two facts from one keystroke share a `client_event_id`, which is
what that column is for.

**The foreign key is composite through `order_id`.** Without it an amendment can
name order A and a line belonging to order C: the fold writes C's line while every
report about A reads consistent. Same idiom as `location.zone_id` through
`site_id`, which S37 already asserts.

**A row-level rule is what makes the per-column fold safe.** D50 made a price two
columns, because `(345, 100)` is 3.45 per 100 and `345` alone is ambiguous by
exactly the factor that matters. D42's fold is per column, so an amendment setting
the price and not the basis would fold onto an unpriced line as `(320, NULL)` —
which the line's own CHECK rejects, making the projection unrebuildable from data
that was legal when written. `intention_amendment_price_pair_ck` requires an
amendment touching the price to state both halves. Then the per-column fold can
never separate them, because no row ever carried one without the other.
**The constraint is not a special case bolted onto the mechanism; it is what lets
the general mechanism stay general.**

**Removal is a state, not a deletion.** `order_line_quantity_ck` forbids zero and
the application has never had DELETE, so a cancelled line was inexpressible: no
value it could take and no verb that removed it. `line_state` folds like everything
else. The row survives because `fulfilment_line` references it and J31 folds
allocations across it — **a commitment that was made is a fact about what the floor
was told to do, and it stays true after the customer changes their mind.**

`line_state` keeps its default, unlike `revision_class` in D48, and the difference
is worth stating because the two look alike. S39 asserts the *absence* of a default
there because the common value is the dangerous one. Here there is exactly one
legal value at insert: a line is created active and becomes removed only by an
amendment naming who and when. The default is the base of the fold, not a guess
standing in for a decision.

**What the application may still write on `order_line` is `line_number`.**
Everything else is either the subject of an amendment or the identity of the line:
changing `item_id` is not an edit, it is a different line, and saying so keeps the
history honest.

#### Amendments to earlier decisions

- **D42** — the sketch's line arm is built. `intention_amendment` reaches an order
  or one of its lines.
- **D50** — `order_line.unit_price_minor` and `price_basis_quantity` become
  `@projection`, which is what question 135 was holding open.
- **D49** — mark, register, grant applied on the way in this time. `order_line`
  had carried a table-wide INSERT and UPDATE since migration 9, on the same
  statement as `order`, which is why migration 12 found one and not the other.
- **D35** — one maintainer, not two. `projection_order_rebuild` folds both
  subjects, because it is one fold of one source under one ordering and splitting
  it would leave two functions that must agree about
  `(occurred_at, recorded_at, id)` forever.
- **J46** *(implemented, widened)* — it had been `Pending("order and
  intention_amendment arrive with the outbound migration")` since before migration
  9 delivered both. Now covers `order` and `order_line`, with the replay property
  as its own test.
- **S41** *(new)* — every covered column is named by both the subject CHECK and
  the changes-something CHECK, and the line FK is composite. The covered set is
  read from the catalogue rather than listed, so a ninth covered column added
  without wiring it in fails on the next run.
- **J55** *(new)* — no fulfilment that is not cancelled commits to a removed line.

**Rejects.** A `kind` enum, which D42's sketch showed and which the built table
was right to omit: the covered columns already say what changed, and a `kind`
beside them is a second representation that can disagree with the first. A
separate `order_line_amendment` table, which is D42's rejected one-table-per-type
inverted one level down. An amendment carrying both subjects. Deleting a removed
line. A `projection_order_line_rebuild` beside the existing maintainer. Letting an
amendment set half a price and special-casing it in the fold.

---

### D52 — A blocker is an object, not a sentence

*Adopted 2026-08-06, settling question 136 raised by D51. Raises 138.*

D49 named three ways a guard fails: **the check was wrong, the check examined
nothing, and the check's stated precondition has since been met while it goes on
reading as deliberate.** Migration 12 fixed the third for S5 and swept nothing
else. By migration 14 the suite held **fourteen** entries waiting on things that
had already been built.

| Waiting on | Arrived | Entries |
|---|---|---|
| `observation` | Migration 7 | J10, J11, J12, J23 |
| `policy_binding` and the policy value tables | Migration 8 | J13, J14, J15, J16 |
| `fulfilment` | Migration 9 | J31 |
| `order` and `intention_amendment` | Migration 9 | J46, swept by D51 |
| `party` | Migration 2 | S33 |
| `metric` | Migration 7 | S21 |
| `stock` | Migration 2 | S25 |
| The application crates | The server | S23 |

**The question undercounted its own subject, which is the finding in miniature.**
It was raised naming nine entries, all job-asserted, because the sweep that found
them read one of the two classes. Four more sat in the structural list the whole
time.

**Why prose was the defect rather than carelessness.** *"observation arrives with
the measurement migration"* is a claim about the schema stored in the one place
the schema cannot reach. Nothing could contradict it, so it stayed true-looking
for as long as nobody re-read it, and re-reading is exactly what does not happen
to a line that says a gap is deliberate. This is the same failure as prose
describing a projection, as a hand-maintained invariant count, and as a question
total carried forward instead of derived. **Four instances of one move, and the
fix has been the same every time: derive the claim from the fact.**

So a blocker becomes a value:

```rust
enum Blocker {
    Absent(&'static str),     // a table, function, or table.column. S42 checks it
    Elsewhere(&'static str),  // a code registry, an AST pass, an open question
}
```

and the reason string is rendered from it rather than stored beside it. **`Absent`
is the arm that carries the property**: S42 asserts every named object is still
missing, so a check waiting on something that has since been built now fails
instead of dozing. `Elsewhere` is what SQL cannot see, and it is counted and
reported rather than checked, because the size of the unverifiable set should be
a number rather than a silence. It is eleven.

**Every named object must be absent, not merely one of them.** S33 waited on
*"party and number_range"* long after `party` arrived, and a rule satisfied by
`number_range` still being missing would have let that half rot for as long as the
other half held.

**Five checks could simply run, and running them found real defects** in data that
had sat unexamined since migration 8. A platform-shipped `policy_binding` was
scoped to one tenant's item — and `policy_binding` is under the shared-reference
RLS shape, so that is a default every tenant can read which resolves for one of
them. A `shelf_life_policy` version was in force from January against the
`policy_change` that created it in August. Another had no change record at all.
**The checks were not merely unwritten; the thing they were meant to guard had
already drifted.**

#### Amendments to earlier decisions

- **D49** — the third failure mode gets a mechanism instead of a name. What
  migration 12 did once by hand, S42 now does on every run.
- **D22** — J13, J14, J15 and J16 are implemented. J14's second clause is the one
  with teeth and it caught a live instance on its first run.
- **D24 supply side** — J31 implemented. It runs before its maintainer exists,
  which is deliberate: it asserts an unmaintained column agrees with a fold
  producing nothing, and starts failing the moment the first allocation is made
  against a line.
- **S42** *(new)* — every database object a pending check names is actually
  absent, across **both** classes. The cross-class scope is not incidental: a
  sweep that covered one of them is the mistake this exists to stop repeating.

**Rejects.** Correcting the fourteen reasons and leaving them as prose, which is
migration 12's fix applied thirteen more times and would rot again on the same
schedule. Deleting the reason and letting `Pending` be bare, which is the silent
gap the reason was introduced to close. Making `Elsewhere` checkable by inventing
a code-side registry to point at, which is a second registry to keep true. Marking
J13, J14, J15, J16 and J31 as blocked on something plausible, which would have
been writing a false blocker to avoid writing a check.

---

### D53 — Progress is three fractions, and one name was doing two jobs

*Adopted 2026-08-06, settling question 134 raised by D49. Migration 15. Raises 139.*

The question was what `projection_fulfilment_rebuild` computes. Answering it meant
settling what `progress` means, which D25 sketched and never decided:

```
fulfilment  state     -- DECLARED: planned | released | cancelled
            progress  -- @projection: allocations, movements,
                      --   packages, consignment
```

**One of those four sources cannot be reached.** `stock_movement` carries no
reference to a fulfilment or an order line, so there is no path from a movement to
the commitment it served. Migration 9's own column comment — *"folded from the
movements against this fulfilment's lines"* — describes a fold nobody could have
written. That is why nobody did.

**A commitment does not have one progress.** It is picked, packed and despatched
to three different extents at the same instant, and every one of those is
something a person asks about. Any single value has to choose one and discard the
other two, which is what made this a D15 question rather than a fold.

So the answer is four quantities on `fulfilment_line`, each a sum over a nested
set of allocation states, taking the lifecycle mechanism-design already states —
*an allocation holds through `allocated | picking | picked | packed`, releasing
only at despatch*:

| Column | States |
|---|---|
| `covered_quantity` | allocated, picking, picked, packed, fulfilled |
| `picked_quantity` | picked, packed, fulfilled |
| `packed_quantity` | packed, fulfilled |
| `despatched_quantity` | fulfilled |

`short` and `released` appear in none. Both are terminal ways for an allocation to
stop covering anything, and counting either reports a commitment as met by supply
that went elsewhere.

**`fulfilment.progress` is dropped rather than defined.** D25 dropped
`order.fulfilment_status` because it would be a third hop and *"an order has few
fulfilments; compute it on read."* **The same argument applies one level down and
D25 did not apply it there.** A fulfilment has few lines, any label `progress`
could hold is a function of the four, and storing it creates a value that can
disagree with its own inputs — which is what S40 refuses for a line total and S36
for a received quantity. S44 keeps it gone, because a rollup that looks obviously
useful is exactly the kind that comes back.

#### One name, two questions

`stock.allocated_quantity` and `fulfilment_line.allocated_quantity` asked
different things and needed different state sets. On the cell: how much of this
stock is claimed — a despatched allocation must **not** count, because the stock
has left. On the commitment: how much of what we promised is covered — a
despatched allocation **must**, because it is the most covered a line can be.

Under one name the second borrowed the first's answer. `uncovered_quantity` is
`quantity - allocated_quantity` and `fulfilment_line_uncovered_idx` indexes
`WHERE uncovered_quantity > 0` to find lines still needing supply, so with
`fulfilled` excluded **a fully despatched line drops to zero covered and reappears
in the index of outstanding work forever.**

D48 renamed `adjustment_class` to `revision_class` because two names for one
question is a near-duplicate. This is the inverse and the more dangerous shape:
one name for two questions does not look like a duplicate at all. It had already
cost something — J31 was implemented from J3's state set in the previous commit,
because the column name said they were the same thing. The commitment side is now
`covered_quantity`.

#### A registered maintainer that maintained nothing

Found on the way. Migration 3 registered five columns to
`projection_stock_rebuild`, and the function body never mentioned
`allocated_quantity`. It has been registered to a maintainer that does not
maintain it since migration 3.

**S5's three legs all pass on it.** The table exists, the column exists, the
function exists, the column is commented and registered. What none of them checks
is whether the function named actually writes the column claimed. It was harmless
only because no allocation had ever been written; the first one would have made J3
find drift on every run with nothing anywhere to fix it.

S43 is the fourth leg, and writing it sharpened the rule. Its first draft demanded
the named function itself and reported `package.placement_event_id` and
`package.placement_occurred_at`, which `projection_package_stamp` does write.
S5 already records that steps are registered under the rebuild they belong to, so
**the unit is the family** — every `projection_<subject>%` function — not the one
name in the registry. `stock.allocated_quantity` was written by nothing in its
family at all, which is the case that matters and the one that survives the
correction.

#### Amendments to earlier decisions

- **D25** — `fulfilment.progress` is withdrawn. The argument D25 used against
  `order.fulfilment_status` applies to it.
- **D24 supply side** — `projection_stock_rebuild` maintains
  `stock.allocated_quantity`, which it was registered for and never did.
- **D12** — the cell-bound predicate is unchanged and now actually executed.
- **J31** *(widened)* — all four coverage quantities, on nested state sets.
- **S43** *(new)* — every registry row names a function family that writes the
  column. S5's fourth leg.
- **S44** *(new)* — no stored progress, completion or rollup status on the
  fulfilment set.
- **J56** *(new)* — no line covered beyond its own quantity, and
  despatched ≤ packed ≤ picked ≤ covered.

**Rejects.** Defining `progress` as a label over the four quantities, which is a
fifth representation that can disagree with them. A numeric percentage, which
picks one of three fractions and hides the choice. Keeping one
`allocated_quantity` name across both tables with a comment explaining the
difference, which is what the comment already tried. A
`projection_fulfilment_rebuild` with a `fulfilment` arm, when the header has
nothing left to maintain. Adding `fulfilment_line_id` to `stock_movement` to make
D25's sketch reachable, which would put a demand reference on the ledger to serve
a status column.

---

### D54 — Authority is declared, not spelled, and J44 was narrowed on the wrong axis

*Adopted 2026-08-06, settling question 132 raised by D48. Migration 16. Raises 140.*

D39 put exactly one system of record on every order and said an order whose
record of authority is external is amended **through that system, not here**. J44
is that sentence as an invariant, and it has never been implementable, because the
only thing separating an authoritative channel from an ordinary one was **how its
name happened to be spelled** in `source_channel text NOT NULL`.

A check wanting to know whether an order was externally authoritative had to carry
a list of magic strings, and a list of magic strings inside a check is the
convention it was meant to replace, written down twice.

So `source_channel` becomes a table, shared-or-tenant like `carrier`, with an
`authority` column of a two-value enum. Two because D39 fixed it at two:
bidirectional merge was refused outright, so the record of authority is ours or
theirs and a third value would be that merge wearing a label.

**Backfilling every existing channel as `local` is a claim, not a default.** An
external channel is one where a counterparty holds the record of authority, which
under D21 makes their document an assertion, and the assertion tables do not
exist. Nothing that would make a channel external has been built, so no external
channel can have written an order. Defaulting the other way would have made J44
fire on every historical order at once, and the first thing anyone did would have
been to silence it.

`source_channel_id` is granted on INSERT and not on UPDATE, because D39 says the
system of record is declared at creation. Moving an order to another authority
afterwards is the refused merge performed one row at a time.

#### J44 was narrowed on the wrong axis

D48 read D39 as being about a class of change and narrowed J44 to forbid
`world_event` amendments, permitting `record_error` because fixing our own
mis-transcription is not amending their document. That was right about
`record_error` and wrong about what the rule is.

**A counterparty cancels their own order.** Their message arrives on the channel.
Recording it sets `order.state`, a covered column, so it is an
`intention_amendment` carrying `new_state`, and the world genuinely did change so
its class is `world_event`. Under D48's statement that amendment is forbidden, and
nothing else can express something that unambiguously happened.

**What D39 forbids is a place, not a class.** A counterparty's event arriving
through the channel is their amendment reaching us. A person here typing the same
thing is us editing their order. `intention_amendment_actor_ck` already carries
the distinction — exactly one of `recorded_by_id` and `automation_key` — so an
amendment with a person on it was made here.

**The proxy is imperfect and the imperfection has a number.** `automation_key` is
undefined: D27 narrowed it by contrast, *a device is how, a key is who*, without
saying what one is. That is question 105, and it has stopped being tidiness — a
local nightly job holding an automation key would pass a check a person fails. J44
now carries the strongest rule the schema can state, and 105 is load-bearing.

#### A column granted to nobody

D50 added `order.currency` in migration 13 and granted it to no one. The
application could price an order line and could never record the currency that
price is denominated in — which J54 requires before a line may be priced at all.

**Every guard in this suite watches for too much privilege.** S6 and S30 assert
that facts and projections have none; J36 asserts projections are not writable by
the application. A column writable by nothing is the opposite mistake and just as
silent, because everything that could have noticed was pointed the other way.

Finding the rule that matched the design took two wrong drafts, and both are worth
recording. *Can any role write it* reported eleven columns and not `currency`,
because `nylonite_projection_owner` holds table-wide UPDATE on `order` and a
table-wide grant covers columns added later. *Can `nylonite_app` write it*
reported thirty-eight the application is deliberately forbidden. **The rule is per
table: on a table the application may write, every non-projection column is in one
of its grant lists.** That is the state a column-level grant is meant to leave a
table in, and it breaks the same way every time — a column added to a table whose
grants were already enumerated, and enumerations do not update themselves.

#### Two closure tables nothing could ever have written

A draft of S45 reported `item_class_closure` and `party_class_closure`. Their
table comment says *"maintained by a named function under D35, never written by
the application role"*. The second half is enforced; the first names a function
that does not exist.

In a deployment those tables can only be written by a superuser. The fixture
populates them because it runs as one. **D22's entire matching language is
ancestor-or-self over these tables**, so a resolver in production would read an
empty closure and every scoped policy would quietly fail to match. Marked
`@projection(pending)` rather than given an invented maintainer, which is what
migration 12 did for `consignment` and refused to do for
`projection_fulfilment_rebuild`. Question 140.

#### Amendments to earlier decisions

- **D39** — `source_channel` is a table with a declared authority.
- **D48** — J44's narrowing is corrected. `record_error` stays permitted; the
  discriminator is the actor, not the class.
- **D50** — `order.currency` becomes writable, having never been.
- **D27, Q105** — `automation_key` is now load-bearing.
- **J44** *(implemented, corrected)* — no `world_event` amendment by a person on
  an externally-authoritative order. Its first clause became structural: NOT NULL
  with a foreign key, so no path can omit it.
- **S45** *(new)* — column grant lists are complete.

**Rejects.** An enum of channel names, which would make adding a channel a
migration. Authority as a column on `order`, which would let two orders on one
channel disagree about whose system of record it is. Authority in D22's lattice,
which would resolve it for orders that never arrived on a channel at all. A
`counterparty_party_id` on `source_channel`, which is the obvious next column and
is not needed to settle 132 — and cannot sit on a shared row, because `party` is
tenant-scoped and J14 forbids it. Keeping J44 as D48 stated it, which leaves a
counterparty's own cancellation inexpressible.

---

### D55 — A policy with no `WITH CHECK` authorises writing what it meant to allow reading

*Adopted 2026-08-06. Migration 17. Raises 141, 142, 143. Found by an external
audit of the schema against PostgreSQL practice rather than by the suite.*

Migration 1 states the rule, in its own grants section, on the day the schema was
created:

> *"Shared rows belong to the platform. The application may read the shared
> catalogue and write only its own rows, **which RLS enforces per row**; the
> platform role is what writes `tenant_id IS NULL`."*

**RLS did not enforce it.** Shape 2 was one policy carrying one expression:

```sql
CREATE POLICY item_shared_reference ON item
    USING (tenant_id IS NULL OR tenant_id = current_tenant());
```

Postgres uses the `USING` expression as the `WITH CHECK` when none is given, so
that clause authorised writes as well as reads. Verified against the live schema
as `nylonite_app` with a tenant context set: an `INSERT` naming
`tenant_id = NULL` succeeded, and an `UPDATE` renamed a platform-shipped row.

**The reach is the shared catalogue itself.** `item` is D19's thin shared item.
`metric` is D23's vocabulary, whose platform codes S21 reserves. `policy_binding`
is worst: a tenant could author a platform-shipped binding that resolves for every
other tenant — which is exactly the shape J14 was written to catch after the fact,
and which the fixture was found to contain when J14 first ran.

**Why nothing caught it.** Every RLS check in the suite reads `polqual`, the
`USING` expression, because that is where tenancy visibly lives. S8 checks FORCE
is on. S9 checks the shape is permitted. Both were correct and both were looking
at the read side. **The write side was never written down, so nothing could
disagree with it** — the same structure as a projection marked in prose, a pending
reason stored as a sentence, and a count carried forward instead of derived. This
is the fifth instance and the first found from outside.

#### An added `WITH CHECK` is not enough

`DELETE` has no `WITH CHECK`. It is governed entirely by `USING`, so a policy
admitting shared rows for reading admits them for deletion however carefully its
`WITH CHECK` is written. The application holds `DELETE` on these tables and needs
it, for its own rows.

So the shape splits into the pair it always meant:

| Policy | Command | Predicate |
|---|---|---|
| `<t>_shared_read` | `FOR SELECT` | shared rows and ours |
| `<t>_own_write` | `FOR ALL` | ours, or shared if we are the platform |

Permissive policies are OR'd per command, so `SELECT` sees both and gets the
union, while `INSERT`, `UPDATE` and `DELETE` see only the write policy.

`is_platform()` exists because `nylonite_platform` carries neither superuser nor
`BYPASSRLS`, so it is subject to these policies and needs an arm that admits it.
Migrations run as the owner, which is a superuser and bypasses RLS entirely, so
seeding shared rows from a migration is unaffected.

**The split is derived from the catalogue rather than from a list.** A list would
be the thing this migration exists to stop trusting: correct today, and silently
short by one the next time somebody adds a shared table.

#### What the audit also found, and what it did not change

Three findings are recorded as questions rather than acted on, because each is a
trade-off rather than a defect: **141** on UUIDv4 primary keys, **142** on foreign
keys without covering indexes, **143** on D33's enum test applied inconsistently.

The audit confirmed what is already right, which is worth recording because the
next audit should not re-derive it: `timestamptz` throughout with no naive
timestamps, money as integer minor units, no `jsonb` and no polymorphic
`(type, id)` pairs, `text` rather than `varchar(n)`, consistent constraint naming,
column-level grants, and migrations verified reversible from empty on every
commit. The tenant-scoped RLS shape was confirmed correct on writes before this
migration was written, because a fix aimed at the wrong shape is worse than none.

#### Amendments to earlier decisions

- **D18, D19** — shape 2 is a read policy and a write policy. The rule migration 1
  stated is now the rule the database applies.
- **S9** *(widened)* — three permitted expressions rather than two. It still reads
  `polqual` and still cannot see the write side, which is why S46 exists.
- **S46** *(new)* — the read half is `FOR SELECT`, never appears without its write
  half, and the write half declares an explicit `WITH CHECK`.

**Rejects.** Adding `WITH CHECK` to the existing single policy, which leaves
`DELETE` governed by a `USING` that admits shared rows. Giving `nylonite_platform`
`BYPASSRLS`, which buys the platform arm by removing tenant isolation from the
role that most needs it. Revoking `DELETE` from the application, which would take
its own rows with it. Listing the affected tables in the migration instead of
deriving them. Treating this as a grant problem: the grant decides which verbs and
the policy decides which rows, and the mistake was asking the grant to carry a
distinction only the policy can make.

---

### D56 — Reversible from empty is the weaker property, and it is the one that was checked

*Adopted 2026-08-06. No migration; a protocol and one corrected `down.sql`.*

Every commit in this repository has claimed some form of *"seventeen migrations up
and down from empty leaving no tables and no types"*. Each was true. **It is also
the weaker of the two things "reversible" can mean, and it is the one that does not
matter.** A down migration is only ever run against a database with rows in it.

The workflow made the gap invisible: migrations are applied, then the fixture is
applied, then the suite runs. The reverse chain was tested before the fixture
existed in the run, so **the seeded reverse had never been executed once** in
seventeen migrations.

Running it found exactly one failure, which is worth stating precisely because the
alarming number would have been the interesting one and this is not it. Sixteen
downs reverse cleanly with data. Migration 14's does not.

#### What a down owes, and what it cannot promise

`intention_amendment_changes_something_ck` requires at least one covered column to
be set. Before D51 the covered columns are four, all order-level. A line-level
amendment sets none of them, so reversing D51 leaves a row asserting that nothing
changed — which the older schema forbids by construction. **It is not a column
being lost; the row is unrepresentable**, and reordering the statements does not
help.

So the down deletes those rows, and says how many out loud.

That sits in tension with a principle worth naming rather than stepping around:
facts are appended and never deleted, which is why `stock_movement` has no DELETE
grant and why nothing may remove a `discrepancy`. **The resolution is that a down
migration is not an application path.** It is an operator withdrawing a
capability, and facts recorded through that capability go with it, exactly as the
columns do. What must not happen is that it goes quietly, so the deletion counts
its rows and raises a notice naming the decision being reversed.

The general rule this sets: **a down migration must run, and where reversal
destroys facts it must say so.** Failing is not a safe default — a down that
aborts halfway leaves a schema in neither state, which is worse than one that
reverses honestly.

#### The protocol

`scripts/verify-migrations.sh` runs four phases: up from empty, down from empty,
up and seed, down with data. The fourth is new and is the only one that would have
caught this. It prints the sentence a commit message quotes, so the claim and the
check are the same artefact rather than two things that agree until they stop.

That is the same move as every other correction in this record — derive the claim
from the fact — applied to the one claim every commit makes about itself.

#### Postgres 18

`compose.yaml` moves from 17 to 18, verified by applying all seventeen migrations,
the fixture and the full suite against 18.4: identical results, no changes
required. The things that could have broken did not — all seven generated columns
name `STORED` explicitly, so 18's new `VIRTUAL` default is inert; `btree_gist` is
the only extension; and S9 and S46 compare `pg_get_expr` output as exact strings,
which turns out to be stable across the versions. **That last one is luck rather
than design** and is worth knowing about before it is not.

The reason to move is question 141: 18 has native `uuidv7()`, so a time-ordered
default is `DEFAULT uuidv7()` rather than an extension. It settles half of 141 and
no more — D5 has handhelds minting identifiers offline, so the client generates
UUIDs too and needs the same ordering from the `uuid` crate's `v7` feature. A
server-side default alone would leave the fact tables getting random keys from the
writer that produces most of them.

**Rejects.** Leaving migration 14's down failing and documenting it, which makes
the whole chain irreversible past that point. Making the down abort with a clear
error instead of deleting, which is the same irreversibility with better wording.
Testing the seeded reverse by hand each time, which is what has been happening to
the other three phases and is why this went unnoticed. Staying on 17 until 141 is
decided, when the upgrade is a one-line change today and a migration later.

---

### D57 — Identifiers are time-ordered, and half of that is not the database's

*Adopted 2026-08-06, settling half of question 141. Migration 18.*

Forty-three columns defaulted to `gen_random_uuid()`, which is version 4 and
therefore random, so every insert landed at an arbitrary point in the primary
key's B-tree rather than at its right edge. The published cost at scale is roughly
a quarter more index, substantial write amplification, and bulk loads an order of
magnitude slower.

**UUIDv7 keeps the property this design actually depends on.** D5 has a handheld
observing reality and minting an identifier for what it saw, offline, with no
round trip. A sequence cannot do that, which is why the answer is a time-ordered
UUID rather than a `bigint`. Postgres 18 has `uuidv7()` natively, which is why the
deployment moved to it in D56.

**Half of this is not settled and saying so is the point.** The server generates
identifiers for rows it creates; the floor generates them for everything it
observes, and that half lives in a handheld that does not exist yet. A server-side
default alone leaves the fact tables — the tables this is for — taking random keys
from the writer that produces most of their rows. Question 141 carries the
remainder rather than being closed.

**S47 asserts the absence of `gen_random_uuid()`, not the presence of
`uuidv7()`**, and the asymmetry is deliberate. Nothing will revert a column that
already defaults to v7. What will happen is a new table written from the pattern
of the forty-three before it, because that is what copying an existing
`CREATE TABLE` gives you. Phrased the other way the check would also be wrong: a
column may legitimately have no default at all.

**Rejects.** `bigint` identity columns, which cannot be minted offline. Keeping v4
until there is data to measure, which is the argument for never doing it — the
measurement requires the cost to have already been paid. An extension on 17
instead of moving to 18, when the move is a one-line change today. Closing 141 on
the strength of the server-side default.

---

### D58 — The columns whose absence destroys history

*Adopted 2026-08-06. Migration 18. Closes three of `migration-1.md`'s four
outstanding items.*

What these have in common is not that they are useful. It is that **the
information they would carry is unrecoverable once the moment passes.** A column
added later starts empty, and every row written before it is permanently silent on
the subject. That is the test `migration-1.md` applied to its tier-0 list and then
left three items sitting under for two weeks.

**Where a lot came from, and when it was made.** Zero mentions in the decision
record before now. Nobody can reconstruct where a lot came from after the pallet
has gone, and no supplier answers that question a year later. It sits on `lot`
rather than `item` because it varies lot to lot for food — the same item arrives
from Malaysia in March and Vietnam in June — and Coles requires country of origin
on every master carton, so it is a labelling input rather than a nicety. Two
letters checked by shape, in the same idiom as D50's currency: a country table
would be a reference list to maintain for a value that is validated by format and
consumed as a label.

**What the goods cost.** `migration-1.md` item 10, unblocked by D50, which drew
the boundary: a cost is not a price. The shape mirrors D50's exactly, because a
supplier quotes per some quantity as readily as a customer is quoted per some
quantity, so minor units alone are ambiguous by the factor that matters.

**The currency sits in the row, and that is the interesting difference.** On an
order the money is on the line and the currency is on the order, so no CHECK
reaches both and J54 has to be a job. A movement has no parent carrying a
currency, so the same rule is expressible as a constraint that cannot be violated
rather than a check that reports afterwards. The pairing rule is identical; where
it can live is not.

What a cost is not, kept true by S48: **not a landed cost**, because freight and
duty apportioned across lines is a computation over facts living elsewhere and a
stored one disagrees with its own inputs the moment a freight invoice is
corrected; **not a valuation**, which depends on a costing method nobody has
chosen and which D40 puts outside this system; **not tax-inclusive**, for D50's
reason exactly.

**What was actually keyed, and what turned it into a number.** `migration-1.md`
item 4, and the half that bites is the packing config. Somebody scans three
cartons and the ledger stores thirty, because `quantity` is in the item's base
unit and everything downstream depends on that. Correct a case pack next year and
every historical thirty silently becomes a different number of cartons. **D23
already versions `item_packing_config` by `effective_from` for exactly this
reason, and without the foreign key that versioning protects nothing**, because
nothing records which version applied.

The list called the middle column `entered_unit`, and **it is not a unit**. A
carton is not a dimensional unit: its factor varies by item and by date, which is
the whole reason the config exists and is versioned. `packaging_level` is the
vocabulary D43 established for precisely this, so it is reused rather than
reinvented. The foreign key is composite through `tenant_id`, per J14's lesson —
without it a movement can name another tenant's config and convert its quantity by
somebody else's carton size.

#### Amendments to earlier decisions

- **D14** — `lot` gains `country_of_origin` and `production_date`.
- **D50** — the cost half of the boundary it drew is now built.
- **D23** — `item_packing_config`'s `effective_from` becomes load-bearing rather
  than merely present.
- **S48** *(new)* — no landed cost, valuation or cost-tax column in the movement
  set.
- **J57** *(new)* — every movement naming a config names one for its own item that
  was already effective when it occurred. **The foreign key catches neither
  failure**: a config created next March is a valid row for a movement that
  happened last August, and a config for another item is equally valid as a
  reference while describing something else's carton size.

**Rejects.** Country of origin on `item`, which cannot express the same product
arriving from two countries. A country reference table for a two-letter code
validated by shape. A landed cost column. Storing entered quantity without the
config that converted it, which is the version of this item that looks complete
and protects nothing. `entered_unit_id` as a foreign key to `unit`, which would
make a carton a dimensional unit with a fixed factor and contradict the reason
`item_packing_config` is versioned. The fourth outstanding item, `goods_receipt_line`'s
half of the entered-quantity trio, which waits on the table existing.

---

### D59 — The purchase order, adopted rather than assumed

*Adopted 2026-08-06. Migration 19. Raises 145. The first of three that close
`migration-1.md`'s tier-0 list.*

Nineteen places in the decision record name `purchase_order` or
`purchase_order_line`. D25 gives its status columns. D24 gives an index over it
and makes it the first arm of `expected_supply`. D39 declares it ours in the same
sentence as `order`. D43 makes a receipt one per demand document. Question 107 was
settled by calling `expected_supply.owner_id` a projection of its source line.

**None of them defines it, and that is the fourth time.** `item_barcode` sat here
before D34, `device` before D27, `goods_receipt_line` before D45. Each was found
the same way: something downstream needed a column nobody had written down.

**How this one was found is worth recording**, because it says something about the
list rather than the table. `migration-1.md`'s last outstanding item is three
columns on `goods_receipt_line`. That table names `expected_supply` and nothing
else for its demand arm, per D45. `expected_supply` requires **exactly one** of
four provenance arms — `= 1` rather than `<= 1`, because "none" is meaningless for
a projection of a promise — and not one of the four exists. **The list's last item
was three tables deep the whole time it was described as a column set.**

#### A purchase order is an order we place

The shape mirrors `order` deliberately rather than by coincidence. D39 puts both
in one category in one sentence: *"`order` and `purchase_order` are ours. They are
created here, they are complete here, and an operation running nothing else
works."* One is what a customer asked us for, the other what we asked a supplier
for, and everything that follows from being our own intention reaching us through
a channel follows for both.

So `source_channel_id` carries D54's meaning unchanged, INSERT-only for D54's
reason. D50's price shape applies unchanged, currency on the document and money on
the line — which makes **J54 one rule over two tables** rather than a second rule
that happens to look alike.

D25's status split is built as specified: `state` is DECLARED and holds
`draft | issued | cancelled`, with one timestamp per state reached and a CHECK
tying each to its state. `partially_received` and `closed` are absent because they
are arithmetic over the ledger, which is the split D25 drew and named NetSuite for
not drawing.

**`receipt_status` is deliberately absent although D25 adopted it.** Its source is
`expected_supply`, which does not exist yet. Adding it now would be a projection
with no maintainer — precisely what migration 12 spent a commit undoing on `order`
and what question 134 spent another answering on `fulfilment`. It arrives with the
thing that computes it.

**The line carries the arrival state, which is what question 107 settled.**
`owner_party_id` and `status_id` describe what the goods will be when they land,
not custody in transit, which is why `expected_supply` can project them and stay
correct when the order is amended. Both nullable: a NULL owner is our own stock,
and a NULL status means the receiving policy decides, which is what D37 put
`default_status_id` on `receiving_policy` for.

#### Amendments do not reach here, and D42 said they would

D42's sketch gave `intention_amendment` three subject arms —
`order_id | purchase_order_id | transfer_order_id`, exactly one. Migration 9 built
the order arm alone and D51 then settled that one amendment names one subject.

Widening to purchase orders is the same shape a third time and it is not free:
`order_id` is NOT NULL, so a second subject means making it nullable, extending
the subject CHECK that S41 derives from the catalogue, and deciding whether a
purchase order line takes amendments the way an order line now does. Doing that
inside a migration named for something else is what D44 and D50 both refused.
Question 145.

#### Amendments to earlier decisions

- **D25** — the status split is built. `receipt_status` waits for its source.
- **D39** — the second half of "ours" now exists.
- **D24** — the first provenance arm of `expected_supply` has something to point at.
- **D54** — `source_channel` covers both order kinds.
- **J54** *(widened)* — one currency rule over both.

**Rejects.** A `received_quantity` or `receipt_status` column with no source, which
is the projection-without-a-maintainer pattern this record has now corrected three
times. `partially_received` in the state enum, per D25. A per-line currency. Free
text for the state. Widening `intention_amendment` in passing. Defining the
transfer or return arms of `expected_supply` here, which are their own documents
and their own decisions.

---

### D60 — One promise identity, whatever document produced it

*Adopted 2026-08-06, building D24's supply side. Migration 20. The second of
three closing `migration-1.md`.*

`expected_supply` is the table that gives a promise of goods arriving **one
identity regardless of which document produced it**, so the allocator learns two
supply kinds forever and a new source is a new arm on a projection rather than a
branch in the hot path. Thirteen pending checks named it, more than any other
absent object in the suite.

**One arm exists, and the precedent for that is `discrepancy`.** D24 gives four
provenance arms with `CHECK (num_nonnulls(<the four>) = 1)` — `= 1` rather than
`<= 1`, because "none" is meaningless for a projection of a promise. Three of
those targets do not exist, and migration 6 already settled what to do: *"Only the
arms whose targets exist are present. The rest arrive in the migration that
creates what they point at, because a nullable FK to a table that does not exist
is not a placeholder."* So the ordered arm is NOT NULL and becomes the four-way
CHECK when it has company. S24 asserts one partial unique index per arm, reads the
arm set from the catalogue, and therefore widens by itself.

Eight further columns are absent for the same reason and are listed in the
migration rather than left to be noticed: the other three arms,
`refines_expected_supply_id` (which D24 pairs to the asserted arm by CHECK, so it
could never be set), the two raw advised fields, and the two that J41 walks. **A
nullable column nothing can populate is the `@projection(pending)` mistake in a
different costume**: it reads as capability and delivers none.

**The generated pair is written out rather than layered**, because Postgres will
not let a generated column read another generated column. D24 states promisable as
outstanding minus allocated; the schema states both from the five inputs.

**The maintainer upserts on the arm and never regenerates.** J30 states it and
`stock_allocation.expected_supply_id ON DELETE RESTRICT` is why: live allocations
hold these ids, so truncate-and-regenerate would either fail outright or orphan a
commitment. Identity surviving a rebuild is what lets an allocation outlive one,
and the property test rebuilds three times to say so.

Three of the five quantities have no source yet and the function says which and
why, rather than leaving them to a default nobody re-reads. `quantity_refined`
needs the asserted arm to have children, `quantity_received` needs
`goods_receipt_line`, and `quantity_closed_short` needs a supplier conversation
nothing models.

**Only an issued order promises anything**, which is the DECLARED half of D25's
split doing exactly the work it was split for. A cancelled order's promise closes
with a reason rather than vanishing, because D24's `closed_reason` set exists so
that "why is this not promisable" always has an answer.

#### S42 earned its place here

Applying this migration made **ten** job-asserted checks fail S42 at once: each
was pending on `expected_supply`, which had just been built. That is D52's guard
working on its first real opportunity, and it is the difference between this
migration and migration 9 — which delivered `order` and left J46 dozing for five
commits with nobody the wiser.

Six could then run. The rest now name what is actually missing:
`expected_supply.refines_expected_supply_id`, `goods_receipt_line`,
`expected_supply.transfer_order_line_id`, `expected_supply.inbound_shipment_id`,
and for J28 and J29 the resolver, which is what turns a receiving policy into an
overdue grace for a row.

#### Amendments to earlier decisions

- **D24 supply side** — built, in its ordered arm.
- **D12** — `stock_allocation` gains its second supply arm, promised by migration
  3 in as many words, with `= 1` across the two.
- **S24, S25, S26** *(implemented)* — the arm indexes, the availability key, and
  the five-quantity cap.
- **J4, J9, J30** *(implemented)* — the allocation fold, the withdrawn-supply
  finding, and identity across a rebuild.

**Rejects.** A unique index over `(item_id, owner_id, status_id)`: two purchase
orders may promise the same goods to the same place and they are two promises.
Truncate-and-regenerate. Nullable columns for arms whose targets do not exist.
Creating `stock` rows for goods that are not there, which is what the second
allocation arm exists to avoid. Inventing an overdue grace so J28 could run, when
what a grace is resolves from a receiving policy and the resolver does not exist.

---

### D61 — The receipt, and the last item on a list written seventeen migrations ago

*Adopted 2026-08-06, building D45. Migration 21. Raises 146. **Closes
`migration-1.md`.***

`migration-1.md` was written on 2026-08-04 as an audit of the inbound analysis's
tier-0 list — *"cheap now, ruinous later, do before any inbound code"*. This
closes the last item on it.

The item is three columns. Reaching them took three tables, because the line names
`expected_supply`, which needs an origin, which needed a purchase order nobody had
defined. **The list described its own last entry as a column set for two weeks
while it was three tables deep**, which is the strongest argument yet for the
banner it carries: re-derive it rather than trust it.

**The demand CHECK is `<= 1` and S3 exists because of this table.** An unsolicited
delivery has no demand-side cause and is still a receipt: goods on the dock are
goods on the dock. Writing `= 1` is the mistake D16 made by repeating D10, and S3
was written to catch it by name.

**One supply arm on the line.** The inbound sketch gave it two, which are two of
the four D24 later unified into `expected_supply`; carrying the pair would rebuild
that union one level down and break on the two arms the sketch never had. NULL is
a blind or unexpected line, which is the `<= 1` argument for a third time in the
same migration.

**The line carries no received quantity, and S36 has asserted that since before
the table existed.** Received is a fold over `stock_movement` grouped by the typed
cause arm, which is a batch load rather than an N+1 precisely because D10 made the
cause a foreign key. The stored accumulator is a documented double-count bug in a
competitor, and the fold excludes putaway movements for the same reason: a move
afterwards names the same receipt line and is not a second arrival.

#### S3 was wrong, and this is the first constraint it ever examined

S3 tests `pg_get_constraintdef(...)` for the substring `= 1`. **`<= 1` contains
`= 1`**, so it flagged both of the constraints this migration wrote — exactly the
form it exists to require.

It has been wrong since it was written and could not show it, because no
constraint named `%_cause_ck` or `%_demand_ck` existed until now. **A vacuous check
cannot be wrong out loud.** The register's own first lesson is that a wrong
invariant is worse than a missing one because it confers confidence; this was
wrong in the cheaper direction and no more correct for it.

#### The fixture produced a negative promisable, and that became J58

With the receipt folded, `quantity_promisable` came out at **-30**. D24 specifies
the handover precisely: *"At receipt, in the same transaction as the movements,
allocations are re-pointed at the new `stock_id`."* Nothing performs that, because
it is the receiving path rather than the schema — so a fully received promise went
on carrying the claims made against it.

The fixture was corrected to a partially received promise, which is coherent, and
**J58 is the check that would have said so**: promisable is never negative, and
when it is, either the re-point did not happen or more was promised against a row
than the row ever had. Neither is expressible as a CHECK, because the generated
column derives from five maintained ones a rebuild sets independently.

#### Amendments to earlier decisions

- **D45** — built, including the correction it made to J26.
- **D24** — `quantity_received` has a source; the ordered arm is complete end to
  end from order to receipt.
- **D10** — the typed cause arm on `stock_movement`, which is what makes the fold
  a batch load.
- **S3** *(corrected)* — the substring test that flagged the form it requires.
- **J26** *(implemented)* — folding the ledger rather than a column that does not
  exist.
- **J58** *(new)* — claims that outlive the promise they were made against.

**Rejects.** A received quantity on the line. `= 1` on the demand CHECK. Two
supply arms. A `status` column on `goods_receipt`, for the reason
`purchase_order.receipt_status` is also absent: what partial means for a receipt
whose lines were accepted, rejected and matched in different proportions is a D15
question rather than a fold, and question 146 carries both. Counting putaway
movements as arrivals. Making promisable a CHECK.

---

### D62 — The receipt handover, and the third clause nobody built

*Adopted 2026-08-06. Migration 22 and `crates/server/src/receiving.rs`.*

D24 states the handover in one sentence:

> *"At receipt, in the same transaction as the movements, allocations are
> re-pointed at the new `stock_id`, `bound_at` is stamped, and
> `origin_expected_supply_id` is retained so 'this unit was cross-docked against
> Coles PO 88421' stays answerable."*

**Two of those three existed and the third did not.** `stock_id` and `bound_at`
were there from migration 3. `origin_expected_supply_id` was not, so the moment a
claim moved from a promise to the stock that promise became, the promise it came
from was gone — and which purchase order a cross-docked unit was committed against
stopped being answerable at exactly the point somebody starts asking.

Nothing performed the handover at all, which is what migration 21's fixture said
out loud: a fully received promise went on carrying its claims and
`quantity_promisable` came out negative. J58 catches the state. This is the code
that stops producing it.

#### Why the rule lives in Rust

The same reason D47's four correction rules do. The decision spans rows — which
claims, against which promise, covered by how much arrived — and Postgres has no
cross-row CHECK. S7 rejects triggers and D25 forbids the validation kind by name.
The write path reads the claims anyway in order to update them, so a pure function
over what it already holds costs nothing it was not paying. J4, J9 and J58 assert
the same properties against stored data for rows that arrive by a restore, a
backfill, or a path nobody has written.

#### Two rules, and both are refusals to be clever

**First bound, first served.** When an arrival cannot cover every claim, the claim
bound earliest is served. Deterministic, explainable to the person whose order did
not get the stock, and deliberately dull: anything weighing urgency or customer
against each other would be the allocator making a decision at receipt time, which
is question 26 and not this.

**A claim larger than the arrival is not split.** Splitting an allocation is an act
with a decision behind it — questions 26 and 34 both circle it — and **a receiver
quietly halving somebody's commitment is not a receipt, it is a re-allocation
performed by the wrong person.** So the claim stays against the promise and the
shortfall is reported.

#### Problems do not veto the handover

A partial delivery produces both a list of claims that moved and a note about the
ones that did not, and both are true. **D5 is why**: this is on the path of an
observation of the floor, and goods on the dock are goods on the dock. A closed
promise still hands over what physically arrived; what it must not do is pretend
the rest is coming, so the stranded claims are raised at the dock rather than
waiting for J58 to find them later — because at the dock somebody is standing in
front of the pallet and can still do something.

The only arrival that yields nothing is an arrival of nothing, which is not an
observation of anything.

#### Amendments to earlier decisions

- **D24** — the handover's third clause exists and the handover runs.
- **D12** — `origin_expected_supply_id` is provenance, not a claim, and is
  deliberately **not** `ON DELETE RESTRICT` unlike `expected_supply_id`: a live
  claim makes a promise un-removable, while the memory of a handover should not
  be more binding than the handover.
- **J58** — now has code that prevents the state it finds, rather than only a job
  that reports it.

**Rejects.** Splitting a claim at receipt. Weighing claims by anything but binding
order. Refusing a receipt because its promise closed, which would discard an
observation of the floor to keep a projection tidy. A trigger. Putting the rule in
the database as a CHECK it cannot express.

---

### D63 — The closure gets the maintainer its comment promised

*Adopted 2026-08-06, settling question 140. Migration 23.*

`item_class_closure` has carried this table comment since migration 1:

> *"PROJECTION of `item_class`. Maintained by a named function under D35, never
> written by the application role."*

The second half was enforced — the application holds SELECT and nothing else. The
first named a function that did not exist. **In a deployment these tables could
only be written by a superuser**, and the fixture populated them by hand because
it runs as one, which proved nothing about deployment.

**D22's entire matching language is ancestor-or-self over these two tables.** A
resolver in production would have read an empty closure and every scoped policy
would have failed to match — not error, not warn, just never apply.

#### Total rebuild, and why that is safe here and forbidden next door

J30 forbids truncate-and-regenerate on `expected_supply`, because
`stock_allocation.expected_supply_id` is `ON DELETE RESTRICT` and live claims hold
those ids.

**The closure is the opposite case and the difference is identity.** Its primary
key is `(ancestor_id, descendant_id)` — the fact itself, not a surrogate — and
nothing in the schema references a closure row. A rebuilt row is not a new row
replacing the old one; it is the same fact recomputed. So a total rebuild per
tenant is correct, and it is chosen over an incremental one for the reason
question 122 exists: **an incremental rebuild optimises a cost nobody has
measured**, and re-parenting is question 78's problem before it is a performance
one.

#### A cycle was insertable, and the maintainer had to survive it

`item_class_not_own_parent_ck` forbids a node being its own parent. **Nothing
forbids A → B → A**, which was verified insertable before the maintainer was
written. A naive recursive walk over that does not terminate.

The rebuild uses SQL's `CYCLE` clause, so a corrupted taxonomy degrades to a
partial closure rather than hanging. Verified against a live two-node cycle: the
function returns, and produces the reachable pairs.

**That makes the maintainer safe and the data still wrong**, which is exactly the
division between the two invariant classes. So J59 is two claims: no cycle, and
every node reaches itself. The second matters because the dropped branch is a
subtree with no closure rows, and the depth-zero row is what makes ancestor-or-self
one lookup — a node missing its own is invisible to a binding naming it directly.

#### Amendments to earlier decisions

- **D22** — the matching language has something to match over in a deployment.
- **D35** — two more named maintainers, tenant-scoped, `SECURITY DEFINER` with a
  pinned `search_path`.
- **D55** — the `@projection(pending)` markers it added become `@projection`, with
  registry rows and the three-way diff S5 asserts.
- **J59** *(new)* — parentage is acyclic and the closure covers every node.

**Rejects.** An incremental rebuild, which optimises an unmeasured cost. A CHECK
or trigger enforcing acyclicity: S7 rejects the trigger and no CHECK can walk a
chain. Granting the application any write on a closure. Leaving the table comment
claiming a maintainer that does not exist, which is the failure this record has
now corrected in five places.

---

### D64 — The order the maintainers run in, written down

*Adopted 2026-08-06, settling question 139. Migration 24.*

A rebuild family is several functions. `projection_package_rebuild` folds the log
and `projection_package_stamp` writes `placement_event_id` afterwards;
`projection_stock_rebuild` folds the ledger and
`projection_stock_resolve_locations` resolves the container arm once packages have
been placed.

**Nothing said so.** `projection_rebuild` records which rebuild owns a column.
S5's third leg requires every `projection_%_rebuild` function to have a registry
row — and neither step matches that pattern, so nothing required them to be
registered, and nothing anywhere said they had to run at all, let alone in what
order.

The order lived in exactly one place: fifteen hand-written calls scattered through
`fixtures/seed.sql`. **A scheduler written from the decision record rather than
from the fixture would have called eight functions and skipped two.**

That is not a hypothetical, and it was measured rather than argued. Clearing
`package.placement_event_id` and running every function in `projection_rebuild` —
which is everything S5 requires to exist — leaves **nought of two packages
stamped**. The declared run stamps both.

#### One entry point, driven by a table

`projection_step` holds the function name, an ordinal and a note saying what the
step needs to have happened first. `projection_run_all` reads it and calls each in
turn, so **skipping a step stops being something a caller can do by omission.**

The order is read rather than compiled in, for the reason every other correction
in this record reaches: a list in code and a list in a table agree until they
stop, and the one nobody re-reads is the one that rots.

Ordinals are sparse. Inserting a step between two others should not mean
renumbering the ones after it, because renumbering is how an ordering silently
changes while looking like it was tidied.

The dependency note is prose rather than a graph. Ten functions in one order do
not need one, and a dependency graph nobody can read is worse than a sentence
somebody can.

#### What S49 adds that S5 could not

S5's third leg reads `projection_%_rebuild` and is right to: a rebuild with no
registry row is a projection nobody declared. **It cannot see a step**, because a
step is not a rebuild and owns no column — its whole job is to finish what another
function started, and S43 passes throughout because the *family* does write the
column.

S49 is the same bidirectional diff over the wider set: every `projection_%`
function is a step exactly once, and every step names a live function.
`projection_run_all` is the only exclusion, because an orchestrator that ran
itself would not terminate.

#### The fixture stops being the specification

It now calls `projection_run_all` and never names an individual maintainer or an
order. **That is the test of whether the knowledge actually moved**: if the
ordering still lived only in the fixture, removing it from the fixture would have
broken something.

#### Amendments to earlier decisions

- **D35** — the maintainer set has a declared order and one entry point.
- **D53** — question 139, raised when S43 was written, is answered.
- **S49** *(new)* — every maintainer is a declared step, and every step is a live
  function.

**Rejects.** A dependency graph, for ten functions in one order. An orchestrator
with the order compiled into it. Renumbering ordinals densely. Requiring steps to
match `%_rebuild` so S5 would catch them, which would rename two functions to fit
a check rather than fixing the check. Leaving the order in the fixture, where no
scheduler reads.

---

### D65 — A year of history, and the first thing it said

*Adopted 2026-08-06, settling question 122. Migration 25, `fixtures/history.sql`,
`scripts/measure.sh`.*

Question 122 has been open since the register was written: *"The receiving queries
are written and reasoned about, not measured."* It was deferred against a seeded
year of history that nobody had built, and 142 and 76 were deferred behind it.

`fixtures/history.sql` is that year. It loads **after** `seed.sql` and into its
**own tenant**, so the small fixture's numbers are undisturbed — a check that
examined six things still examines those six, plus whatever this adds.

| | |
|---|---|
| 365 days, 500 items, 200 locations | the proposal's timed baseline is 300 orders a day |
| 289,083 stock movements | receipts, putaways and despatches |
| 35,040 promises, receipts and order lines | 8 purchase orders a day at 12 lines |
| **the whole suite holds** | 2.3 seconds against a year |

**Item choice is skewed rather than uniform.** A flat draw over 500 items makes
every index look equally good, which would answer question 142 wrongly. Most of
the mass sits on a few items, as it does on a real catalogue.

#### Determinism, and a check that passed by not running

`seed.sql` is deterministic by writing every value down, which does not scale to a
quarter of a million rows. A fixture that differs per run makes every measurement
a description of the afternoon, which is the criticism that produced `seed.sql`.
So `setseed` once, and the rest follows.

**The first version of that check was worthless and is worth recording.** It
computed a checksum, deleted the tenant, reloaded, and compared. `person_tenant`
references `tenant`, so the delete failed; the reload never happened; and the
comparison was the data against itself. It reported success. **A check that passes
by not running** is the failure this record has now found in a projection marker,
a pending reason, a hand-maintained count, an RLS policy and a substring match —
and this time in the verification of the thing built to find such failures.

It now builds the year twice from an empty schema and compares. It takes four
minutes and it is worth four minutes.

#### What the measurement said

**D24's query B was right.** The line list for one delivery runs in **0.36 ms**,
driven from `purchase_order_line (purchase_order_id)` and then
`expected_supply`'s per-arm unique index — exactly the two D24 named, with no new
index. A prediction confirmed is worth as much as one refuted and is rarer.

**D24's index reasoning rested on a premise that was false.** It justifies the
partial predicate on the receiving index:

> *"about 230,000 `expected_supply` rows a year a site, of which a few thousand
> are open at any moment. A year in, the live set is about one percent of the
> table, and an unpartial index makes the planner do the wrong thing."*

Measured: **32,963 of 35,040 promises were fully received and still open.** The
live set was a hundred percent. `projection_expected_supply_rebuild` closed a row
when its purchase order was cancelled and on no other occasion, so the partial
index would have indexed the whole table while its reasoning read as sound.

D65 closes a promise delivered in full, using the `received_in_full` reason D24
already had waiting. The live set falls to **2,077 of 35,040, 5.9%** — and those
2,077 are exactly the six per cent of lines the generator makes arrive short.
D24's order of magnitude was right; the mechanism was missing.

**A short receipt deliberately stays open.** Closing it is `short_closed`, which
is a supplier agreeing to release the remainder — a conversation rather than
arithmetic. D24 lists the two reasons separately for that reason, and a rebuild
collapsing them would decide a commercial question by rounding.

#### Amendments to earlier decisions

- **D24** — the close on full receipt, and the partial index its reasoning
  described, now that the predicate selects something.
- **D60** — `projection_expected_supply_rebuild` gains its fourth close arm.
- **Question 122** — answered, and `scripts/measure.sh` makes the answer
  reproducible rather than a number in a commit message.
- **Question 142** — now answerable. Its trigger was this fixture.

**Rejects.** Closing a short receipt automatically. A fixture that is not
reproducible. Folding the year into `seed.sql`, which would make the small
fixture's deterministic story impossible to read. A uniform item distribution.
Adding indexes from the measurement in the same commit that first takes it, which
is 142's decision and deserves its own.

---

### D66 — Three indexes out of a hundred and thirty-two, and the measurement said which

*Adopted 2026-08-06, settling question 142. Migration 26.*

142 asked which foreign keys need covering indexes and said the answer should be
**a considered subset chosen from measurement rather than from taste**. D65 built
the year that made measuring possible.

**Selectivity decides, not foreign-key-ness.** Postgres indexes the target of a
foreign key and never the source, and the standard advice is to index every source
column. Against 289,083 movements that advice is wrong more often than it is
right:

| candidate | rows matched | unindexed | indexed |
|---|---|---|---|
| `stock_movement` lot pair | 3 of 289,083 | 21.96 ms | **0.27 ms** |
| `stock_movement` location pair | 70,080 of 289,083 | 12.41 ms | **24.78 ms** |
| `goods_receipt_line.item_id` | 1 of 35,041 | 22.40 ms | **0.044 ms** |

**The location index makes its query twice as slow.** Twenty-four per cent of a
table is not an index lookup, it is a sequential scan with extra steps, and the
planner is right to say so. That candidate was chosen because it looked exactly as
plausible as the two that were kept.

#### What was taken

The **lot pair**, partial. Three rows out of two hundred and eighty-nine thousand,
and the question behind it is a recall: this lot is contaminated or mis-dated,
where did it go and what is left. D14 built lot tracking for that and D31 sets a
retention floor so the answer survives. Partial because D33 makes tracking a
property of a product rather than a mode, so the index stays proportional to the
tracked subset — 864 kB against a 63 MB table. Two indexes rather than a composite,
because the recall question is "either side" and a bitmap OR answers it.

**`goods_receipt_line.item_id`.** One row in thirty-five thousand, five hundred
times faster. The supplier scorecard D8 wants and the shortage investigation the
receiving screen needs.

#### What was rejected, which is most of it

**The actor columns.** The fixture has three people, so one of them matches
thirty-eight per cent of the ledger. **This is not a decision, it is an admission
that the instrument cannot read it**, and question 147 carries it rather than
letting a guess pass as a result.

**The parent-delete cost.** Deleting one `location` costs 67.6 ms, of which
60.8 ms is two sequential scans of `stock_movement`. That is the cost of
*discovering the delete is illegal* — a location with movements against it cannot
be removed without destroying what the ledger means, and the foreign key correctly
refuses. Locations are deleted approximately never, so 60 ms of rare admin work
does not buy 2.3 MB of index and a write cost on the hottest table in the schema.

**The reaper, which 142 named as the case making this live rather than
theoretical.** It is already covered: `stock_allocation.stock_id` carries an index
and deleting an empty cell costs 0.669 ms. **The question was wrong about its own
strongest argument** — it was written from the same reading of the same standard
advice that this decision is refusing, which is worth recording rather than
quietly correcting.

**The remaining hundred and twenty-odd.** Validation foreign keys on small
reference tables, never joined from, on tables the planner reads in a page.

#### Amendments to earlier decisions

- **D14, D31** — the recall question has an index behind it.
- **Question 142** — settled by measurement. **Question 147** *(new)* carries the
  part the fixture cannot answer.

**Rejects.** Indexing every foreign key. Indexing the location pair, which was
measured slower. Indexing the actor columns on a fixture that cannot distinguish
three people from thirty. Indexing to make an illegitimate delete fast. A
composite over the lot pair.

---

### D67 — The partitioning plan, and why none of it happens yet

*Adopted 2026-08-06, settling question 76. No migration; a plan and a trigger.*

76 asked for the partitioning plan for the large fact tables and was deferred
against the first large tenant. D65 built a tenant-year, so the question can be
answered from size rather than from intuition.

| | one tenant-year | dead tuples |
|---|---|---|
| `stock_movement` | 100 MB, 289,083 rows | **1** |
| `expected_supply` | 66 MB, 35,041 rows | **35,040** |

**The ledger does not need partitioning and the projection did not need
partitioning either** — it needed D68. That is the first thing the measurement
said, and it is worth separating: `stock_movement` is append-only and behaves
exactly as an append-only table should.

#### What partitioning would actually cost here

A partitioned table's primary key must contain the partition key. Partitioning
`stock_movement` on `occurred_at` therefore makes its key `(id, occurred_at)`, and
**every foreign key referencing it becomes composite**: `reverses_movement_id` on
itself, and `stock_movement_id` and `resolving_movement_id` on `discrepancy`.

UUIDv7 does not help. D57 made identifiers time-ordered, but the order is
*generation* time and the partition key is `occurred_at`, the device clock — D24
separated those deliberately and D47 built a whole correction vocabulary on the
difference. They cannot be the same column.

#### The plan, and the trigger is retention rather than size

**`activity_event`** is partitioned from birth. S32 already requires it — *"range-partitioned
on `occurred_at` with local indexes from the first migration"* — so the decision
predates this one and the table does not exist yet.

**`stock_movement` and `observation`** are not partitioned now, and the trigger is
**not** a row count. It is D31's retention floor: the first time a floor requires
a year of ledger to be removed, a mass `DELETE` of several million rows is exactly
what partitioning exists to avoid, and `DROP TABLE` on a partition is the whole
operation. Size alone does not justify it — ten tenants at five years is about
fourteen million rows, which Postgres serves unpartitioned without complaint.

**This does not meet D58's bar and that is the argument for waiting.** D58 acts on
things that are unrecoverable later: history not written cannot be written
afterwards. A primary key is expensive to change later and not impossible, so the
"cheap now, ruinous later" test does not fire, and building partitioning before
anything needs it would buy planning overhead and composite foreign keys in
exchange for nothing measurable.

**Rejects.** Partitioning `stock_movement` now. Partitioning on `id` rather than
`occurred_at`, which would partition by when a row was written rather than by when
the thing happened. Treating row count as the trigger. Deferring `activity_event`'s
partitioning to match, when S32 already decided it and a table partitioned from
birth costs nothing to partition.

---

### D68 — A rebuild that changes nothing should write nothing

*Adopted 2026-08-06. Migration 27. Found while measuring for question 76.*

Every maintainer here was idempotent in the sense that mattered before there was
data: run it twice and the values are the same. **That is not the same as writing
nothing**, and the difference is invisible until there is a year to see it
against.

Measured: one `projection_run_all` **with nothing changed rewrote 39,565 rows.**
`expected_supply` carried 243,214 updates against 35,041 rows and 35,040 dead
tuples against 35,041 live ones. Only 8,427 of those updates were HOT, so most
wrote index entries too.

Under MVCC each of those is a new row version. **A projection rebuilt hourly
churns its whole table hourly**, and the cost lands on autovacuum, on the buffer
cache, and on every index over the table.

#### It was a class, and the fixed ones hid it

An `ON CONFLICT DO UPDATE` with no `WHERE`, and an `UPDATE ... FROM` with no change
predicate, both rewrite a row to the value it already holds. **Seven of the ten
maintainers did one or the other.** The two that did not were the two whose guards
were written after J4 and J31 made the comparison obvious — so the correct ones
looked like a stylistic preference rather than a rule, which is why nobody read
the others as wrong.

`projection_package_containment_rebuild` was a third shape: delete every row and
re-insert it, which is the truncate-and-regenerate J30 forbids next door. It is
*safe* here — nothing holds a foreign key to a containment interval, which is why
J30's argument does not reach it — but replacing an identical set is pure churn.
It now compares a hash of the computed set against the stored one and returns
early when they match.

After the guards, on a freshly built year: **zero dead tuples**, and
`expected_supply` falls from 66 MB to 36 MB.

#### The property is a test, not a comment

`a_rebuild_that_changes_nothing_writes_nothing` runs the whole set twice and
asserts the second run writes nothing. It is a property test rather than a check
for J6's and J46's reason: observing it requires running the maintainers.

**It was written before the fix and failed at 39,589**, which is the only way to
know a test can fail. Then 7, which found the two the first pass missed. Then zero.

One of those intermediate failures is worth recording: the first guard on
`projection_package_rebuild` compared `wp.status` when the SET clause reads
`ws.status`, and Postgres rejected it outright. **A guard that compares the wrong
columns either never fires or always fires**, and both look like working code.

#### Amendments to earlier decisions

- **D35** — a maintainer writes only what changed.
- **D25** — "rebuildable" now means rebuildable without cost.
- **J30** — its argument is cited where it does not reach, rather than assumed to.

**Rejects.** Tuning autovacuum to keep up with churn nothing needed to produce.
Leaving the containment rebuild replacing an identical set. A row-by-row diff for
containment, when the only question is whether to replace the set at all.
Asserting the property in a comment.

---

### D69 — Two actor indexes, once the instrument could read them

*Adopted 2026-08-06, settling question 147. Migration 28.*

D66 measured every index candidate except the actor columns and could not measure
those. `history.sql` had three people, so one of them matched thirty-eight per
cent of the ledger and the number described the fixture rather than the domain.
147 recorded that as **an admission the instrument could not read it**, rather
than as a decision.

So the instrument changed first. Forty people over the year with turnover, five of
whom leave — which also exercises `person_tenant.left_at`, a column nothing had
touched since migration 1, and which question 70's retention work will want.

| | before | after |
|---|---|---|
| distinct actors on the ledger | 2 | **40** |
| median rows per person | 109,500 | **7,610** |
| median selectivity | 38% | **2.63%** |

| query | rows | unindexed | indexed |
|---|---|---|---|
| everything one person recorded | 7,610 of 289,083 | 18.28 ms | 1.94 ms |
| one person, one day | 20 | 52.47 ms | **0.031 ms** |
| their work sessions | — | 1.90 ms | 0.074 ms |

**The day-scoped query is the shape that matters and was the worst.** *"What did
this person do on that shift"* is the question D11 split the operator, the workers
and the accountable to make answerable, and it is the architecture's own argument
for naming a person rather than a crew: *"nobody can follow up a question with
'Casual Melbourne'."* Unindexed it scans the whole ledger to return twenty rows.

**One composite, not two indexes.** Equality on the actor, range on the moment.
Measured with the composite alone it serves both shapes, so the plain
`(recorded_by_id)` index it would duplicate is not taken — 9.2 MB rather than
11.3 MB, and one index to maintain on the hottest table rather than two.

#### The other actor columns answer themselves

**`stock_movement.authorised_by_id`: zero non-null rows in a year.** Not a gap in
the fixture — D11's shape. The accountable is named on the exceptional movement,
not the ordinary one, so the column is sparse by design. A partial index over an
empty set would be a guess dressed as a decision.

**`goods_receipt_line.accepted_by_id`: fully populated and not selective.** Three
receivers cover eight deliveries a day, so one matches a third of the table.
**That is a property of receiving rather than of the fixture** — goods inward is
done by few people because there are few deliveries — so 147's criticism does not
apply to a number that is small for a real reason. This is the distinction the
question was really about: not "is the number small" but "is it small for a reason
that will still hold in production".

**`discrepancy.detected_by_id`.** One row after a year. The findings queue is
small by design, which is D8 working rather than a population waiting to grow.

#### Amendments to earlier decisions

- **D65** — the generated year gains a workforce, and `left_at` is exercised.
- **D66** — the measurement it deferred is taken.
- **D11** — the question its role split exists to answer now has an index behind
  it.

**Rejects.** A second plain index on `recorded_by_id`, which the composite covers.
Indexing `authorised_by_id` over an empty population. Indexing
`accepted_by_id` because three looked like too few, when three is what receiving
has. Assuming that any low-cardinality actor column is a fixture artefact — the
lesson of 147 is to ask why the number is small, not to distrust every small
number.

---

### D70 — What makes a cached resolution stale, and the half nobody writes

*Adopted 2026-08-06, settling question 80. Migration 29.*

80's full text lives in `mechanism-design.md` rather than in the register, which
is worth noticing on the way past: **the register carries the one-line title and
the reasoning stayed behind**, which is precisely the failure the register exists
to stop. It reads:

> *"Resolver cache invalidation. A missed invalidation means the floor runs on
> stale weights and nothing detects it. `policy_change.id` as a monotonic epoch
> per (tenant, kind), checked on every resolution rather than a TTL — but it needs
> designing, not assuming."*

The instinct is right — an epoch checked per resolution rather than a TTL — and
the mechanism is short by three things.

#### What actually changes a resolution

| | seen by |
|---|---|
| a value version arrives or retires | `policy_change` |
| a binding is added or superseded | `policy_binding` |
| the taxonomy is re-parented | the closures |
| **an effective range opens or closes** | **nothing at all** |

`policy_change.id` sees the first. It does not see the second: D22 makes a
binding's scope immutable and a change a *new* binding superseding the old, and a
binding is created without a `policy_change` — measured, one of three in the
fixture has no change record and is right not to. It does not see the third, which
is question 78's territory.

**And nothing sees the fourth.** `%_policy.effective` is a `tstzrange`. A
resolution taken at 09:00 can be wrong at 09:01 because a version's range ended,
and no row was written when it did. **An epoch is a fact about writes, so it is
insensitive to this by construction**, and a cache keyed on one alone serves that
answer forever — which is exactly the failure the question names.

So validity is two conditions, both exact:

    the epoch is unchanged     covers everything that is written
    now < expires_at           covers the one thing that is not

Neither is a TTL. **A TTL is a guess about how wrong you are willing to be**, and
the thing being guessed about is which stock ships.

#### The epoch is derived, and the taxonomy arm comes from the maintainer

Every stored counter in this repository has gone wrong — the invariant count, the
question total, the `@projection` markers. This one is a `greatest()` over three
indexed reads and is correct by not existing.

The closures have no timestamp and S7 forbids the trigger that would give them
one. But D64 already has every maintainer reporting rows touched and D68 made that
zero when nothing changed, so the stamp is free: `projection_run_all` records when
a step last did something, **and only when it did**, which keeps D68's property
true.

That arm is deliberately coarse. `projection_step` is global, so one tenant's
closure rebuild bumps every tenant's epoch. **Over-invalidating is safe and the
opposite mistake serves a wrong answer**, which D22 already ranks: a resolution
nobody can explain is worse than a slow one.

#### What is not built

The cache. There is no resolver, so there is nothing to cache and where it lives
is the resolver's decision. What is built is the two questions a cache has to be
able to ask, and **S50 is what keeps them answerable**: every table a resolution
reads must reach the epoch by some route, read from the catalogue rather than
listed — so the fourth `%_policy` table is covered the day it exists rather than
the day somebody remembers. Seven of D22's ten named kinds have no value table
yet, so that day is coming.

Re-parenting still does not announce itself as an act. The epoch sees it only
because a maintainer noticed the closure moved, which is a consequence rather than
a record. That is question 78 and it stays open.

#### Amendments to earlier decisions

- **D22** — the resolver has a defined invalidation contract before it has a
  resolver.
- **D64, D68** — the step report becomes a signal, and the stamp is conditional so
  a no-op run stays one.
- **S50** *(new)* — the epoch sees every resolver input.

**Rejects.** A TTL. `policy_change.id` alone, which misses scope, taxonomy and
time. A stored epoch counter. Per-tenant precision on the taxonomy arm, which
would buy sharper invalidation at the risk of the one error that matters.
Inventing a cache to go with the contract, when the resolver that would own it
does not exist.

---

### D71 — The write budget, measured on a row that is a lock on purpose

*Adopted 2026-08-07, settling question 120. No migration; a harness and a rule.*

120 was 111 in the supply-side design before consolidation renumbered it, and —
like 80 — its reasoning stayed in the research document while the register kept
the title:

> *"ASN ingestion performs one parent update per child row, so a 200-line DESADV
> touches one parent 200 times while the allocator wants it. Batching the
> decrement per parent inside the ingestion transaction turns 200 updates into one
> and needs no schema change — but the model states a read budget and no write
> budget, for this table or for `stock`, and D5 only decides the floor-blocking
> half."*

#### The row is a lock on purpose

The supply-side design keeps `expected_supply` a table rather than a view for two
reasons, and the second is this: *"the FK target is the gate row Postgres needs to
serialise concurrent allocations, in the absence of the gap locks ERPNext's
availability check depends on."*

Two allocators asking whether a promise still has room must not both say yes.
`SELECT ... FOR UPDATE` on the promise is what makes them take turns. **So every
needless write to that row is a needless queue**, and the 200-versus-1 question is
not about throughput — it is about somebody else's latency.

#### What it costs, measured

| | |
|---|---|
| one update per line, 200 lines | **64.33 ms**, 200 row versions |
| one update per message | **0.71 ms**, 1 row version |

Ninety times the duration and two hundred times the garbage, for arithmetic that
is the same either way.

And the waiting side pays it in full: with the gate held for 200 ms, an allocator
arriving behind it waited **178 ms**. There is no queue-jumping and no partial
progress, so **shortening the hold is the only lever there is.**

The gate itself serialises cleanly — one allocator 34 ms, eight 55 ms,
thirty-two 174 ms — which is roughly a fixed cost plus a few milliseconds a turn.
**That is the budget: one promise row sustains on the order of a couple of hundred
turns a second, and an unbatched 200-line advice spends 64 ms of it — about a
dozen allocator turns — on arithmetic worth one.**

#### The rule

**One update per parent per transaction, never one per child.** It needs no schema
change, which is what the question already suspected; what it needed was a number,
and the number is ninety.

This is a code-path rule rather than a schema one, so it has no invariant — the
same position S23 is in, waiting on an AST pass over the application crates that
does not exist. The three measurements are the check, and they live in the suite
rather than in a comment.

**When the gate stops being enough**, the remedy is the delta side table the
supply-side design already named and priced — D365's `WHSInventReserveDelta` — and
it costs the generated column and the index-only read on whichever table it is
applied to. The trigger is a measured one now rather than a feeling: sustained
demand for more than a couple of hundred allocations a second against a single
promise.

#### The instrument had to change again

`history.sql` produces a year of volume from one connection, and **volume is not
contention** — nothing in it ever waits for a lock. This is the third time a
question could not be read until the instrument changed: D65 built the year, D69
gave it a workforce, and this gives it two hands. Worth noticing as a pattern,
because the three questions were deferred for years of project time against
triggers that turned out to describe the measuring rather than the thing measured.

#### `stock` has no write budget because it has no gate

The question asks for one for `stock` too. It does not need one: nothing takes a
row lock on `stock`, because nothing writes to it except its maintainer. The floor
appends to `stock_movement`, which is append-only and contends with nobody — which
is D5's design working, and the reason the floor-blocking half was the only half
D5 had to decide.

**Rejects.** A delta side table now, before anything is contended. A write budget
for `stock`, which has no gate to budget. Asserting a throughput threshold in the
suite, where thirty-two threads measure the laptop as much as the schema and a
number chosen to pass would be one nobody could defend.

---

### D72 — Re-parenting is an act, and `parent_id` is what the acts add up to

*Adopted 2026-08-07, settling question 78. Migration 30.*

> *"Taxonomy re-parenting silently changing which settings win."*

Every word of that was true. `item_class.parent_id` and `party_class.parent_id`
were ordinary columns the application could UPDATE, and moving a class changes
which binding wins for everything underneath it — so the most consequential edit
in the policy system was the one with no author, no moment and no reason. D70 met
it three weeks earlier and had to route around it: the policy epoch sees a
re-parent only because a maintainer noticed the closure moved, *which is a
consequence rather than a record*.

#### The shape

A move becomes a `policy_change` row with `change_kind = 'reparented'`, and
`parent_id` becomes a `@projection` folded by `projection_taxonomy_rebuild` at
`projection_step` ordinal 5 — **before** the closure rebuilds at 10 and 20, so one
`projection_run_all` moves the class and repairs its closure in the right order.

The fold takes the destination **verbatim** rather than coalescing onto the
current row, which is the difference between this fold and D42's: a `reparented`
row always states the parentage, and NULL means the root. A COALESCE would make
"moved to the root" inexpressible.

`policy_change` gains taxonomy subject arms, so `policy_binding_id` and `kind` stop
being NOT NULL and a CHECK requires exactly one subject. The application keeps
`INSERT (id, tenant_id, parent_id, code, name)` — a class is still *created*
somewhere — and `UPDATE (code, name)`. It loses UPDATE on `parent_id`, and it
loses DELETE, which **J36 is what said so**: deleting the row takes the projection
with it, the same objection whether the table is wholly maintained like `stock` or
carries one folded column like this.

#### Two typed arms, twice, and the second time it failed silently

The destination column was written once as `new_parent_id REFERENCES item_class`.
The fixture found it on its first party-class move: one untyped column has to point
somewhere, and pointing it at one taxonomy makes the other unrepresentable while
pointing it at neither is the polymorphic pair D10 refused. Two columns, two
foreign keys.

**The same mistake in the impact function did not raise an error — it returned a
number.** Asked about a party class, a function computing from the item closure
answered **0**, because no item class has that id and the count of nothing is
zero. A screen reading "this move changes 0 resolutions" before a move that
changes several is worse than no screen. This is the recurring failure of this
record wearing a new coat: *a claim that was true of the case it was written for
and silently false everywhere else.*

And the definition was wrong underneath the typing. It counted bindings scoped
*into* the subtree, but **a binding scoped to GLOVES is exactly as specific
wherever GLOVES hangs.** What changes is which *ancestor* bindings reach the
subtree's members — the symmetric difference of the old ancestor chain and the
new one — and computing that needs the destination. So the function takes the pair
the caller is about to record.

#### The fixture was asserting an effect it did not produce

Its comment said the move "changed which shelf-life policy wins for every supplier
underneath". All three bindings sat on the Product dimension: **the party closure
was exercised structurally and no policy ever resolved through it**, so the move
changed nothing. Migration 30 adds a counterparty-scoped binding, and the number
is now real.

#### What is detected rather than prevented

A cycle. Two individually-legal `reparented` rows can produce one, and the fold
writes it — measured, and J59 named both nodes. **Prevention is a cross-row
constraint with no declarative form, and the trigger that would carry it is
forbidden by S7** ("no trigger implements rules, validation, defaults or
cascades"). The honest position is the one the record already holds for this
class of constraint: the write succeeds, the rebuild degrades to a partial closure
via SQL's `CYCLE` clause rather than hanging, and J59 is the detector. What
changed is the route — the application can no longer reach `parent_id` at all, so
a cycle now requires two recorded acts with an author and a reason attached to
each.

#### The suite was green partly by accident

Verifying this migration ran the suite in parallel, and D68's property — *a
rebuild that changes nothing writes nothing* — failed. It was not the taxonomy
fold. **Measured on the commit before this one: five parallel runs, three red;
the same six tests green every time on a single thread.**

The test binaries share one database and several of them write to it, while cargo
runs tests within a binary on several threads and runs the binaries in parallel.
A rebuild is only a no-op if nobody else is writing, so the property was racing
its own siblings. It has presumably been intermittent since D68 added it, which
means **every "the suite is green" in this record has been partly a scheduling
accident** — the same class of error as a check that passes because it examines
nothing, which is what the vacuity column exists to expose.

Every database-touching test now takes one session-level advisory lock. It is
database-wide, so it serialises across binaries as well as threads, which
`--test-threads=1` would not. Six consecutive parallel runs, all green.

Counted while there: the register said 53 invariants carry the vacuity mark and
the table carries 54. **That is the fourth stored count in this repository to
drift from the thing it counts**, after the invariant total, the question total
and the `@projection` markers. **Correcting it by hand immediately produced the
next one**: the question register's own "the deferred section holds 35" was stale
against the table above it, and settling 78 first bumped it to 37 rather than
re-deriving it — adjusting a stale number in the paragraph that exists to warn
against adjusting stale numbers. Both are corrected by counting. The pattern is
question 150, because a fifth found by hand predicts a sixth.

#### Amendments to earlier decisions

- **D70** — the epoch's taxonomy arm now has a record behind it, not just a
  consequence. The arm itself is unchanged and stays coarse.
- **D10** — restated a fourth time, and the first time the untyped form failed by
  returning a plausible number instead of an error.
- **J59** — its text said a cycle is insertable by UPDATE. That route is closed;
  the wording now describes the one that remains.

**Rejects.** A trigger to refuse cycles, which S7 forbids. Having the fold skip a
cycling move, which silently discards a recorded act. Having the fold raise, which
lets one bad row wedge every projection for a tenant. An `active` flag to replace
the DELETE this removes — **retiring a class is an act nobody has designed**, and
inventing one here to close a gap this migration opened is how undesigned things
get built. That is question 149. Temporal closure is question 148.

---

### D73 — The blast radius counts bindings, because resolutions are unbounded

*Adopted 2026-08-08, settling question 95. Migration 31.*

D23 raised it on the day the taxonomy design was adopted:

> *"`affected_resolution_count` is computed before a taxonomy move — against what?
> Active bindings is cheap; actual future resolutions is unbounded. It needs a
> defined denominator or the number is theatre."*

D72 built the number two days ago and did not answer this. It shipped a definition
that is *nearly* right, and the gap is the one D22 warned about in the same
paragraph that asked for the number.

#### What D72 counted, and what it missed

D72 counted the symmetric difference of the old and new ancestor chains: what a
subtree stops inheriting from and starts inheriting from. Bindings scoped there
plainly change.

But resolution orders matches by **depth vector, compared lexicographically**, and
D22 says exactly what that costs:

> *"Cross-dimension flips are possible: a binding at Product-3/Space-0 beats one at
> Product-2/Space-2; re-parent so the first is depth 2 and the second now wins,
> with nothing about either binding changed."*

    A > B > X       move X directly under A
    A > X

A is an ancestor before and after, so **D72's function reports zero** — measured,
against the corrected one reporting one. A's depth went from 2 to 1, so every
binding scoped to A moved a place up the Product axis and may now outrank a Space
binding it previously lost to. A number that reports zero for that is precisely
the theatre 95 named.

#### The denominator, stated once

**Active bindings whose depth vector for the moved subtree changes.**

- **Bindings, not resolutions.** A resolution is per item, per kind, per instant,
  per request node — 95 is right that it is unbounded, and it is the wrong unit
  regardless. Every resolution that changes involves a binding whose vector
  changed, so **counting the cause rather than the effect is what makes the number
  finite and true**.
- **Active**: it has a value version whose `effective` covers the moment asked,
  and nothing supersedes it.
- **Depth vector changes**: gained an ancestor, lost one, *or kept one at a
  different depth*.

The depth change is identical for every member of the subtree — a member k below
the moved class sees each ancestor at k plus its depth from the class, and k
cancels — which is what makes this computable at the class instead of per item.

Bindings naming an `item_id` or `party_id` directly are excluded: they sit at the
most specific node of their dimension and no re-parenting moves them. **D72
counted them, which was the same error pointing the other way** — inflating
instead of understating.

#### Frozen, and why this is not the sixth counter to drift

D22 asked for the number "computed before the move, frozen on the fact", and that
survives. It deserves defending, because five stored counts in this repository
have now drifted.

**Every one of those was a count of the present**, re-derivable at any moment;
storing it was redundancy, and redundancy rots. This one cannot be re-derived. It
is a fact about a past instant — how many bindings *were* active and *would have*
changed, measured against a taxonomy shape the move itself replaced. Recomputing
it tomorrow answers a different question. That is the same reason D58 stores a
lot's country of origin and D50 makes a price a term of one order: **it is
evidence of what somebody was told before they agreed to something, not a cache.**

Nullable, with **J60** rather than NOT NULL. A NULL means the move predates this
migration, and backfilling one would be inventing evidence of a conversation that
did not happen. J60 reports them; it does not fabricate them.

#### What this does not solve

**Nothing checks the stored number against the function.** It cannot — verifying
it needs the pre-move taxonomy, which the move destroyed, and the only mechanism
that could catch it at write time is a trigger, which S7 forbids. It rests on the
application calling the function it was given, which is where S23 already sits.

Noticed on the way past: `policy_binding_scope_key` is unique over the whole
scope, so **two bindings can never share one scope** and `supersedes_id` can only
ever record a scope *change*. A policy that changes at the same scope does it
through value versions, which is D22 working as designed — but it means the
supersession arm of "active" is narrower than it sounds, and it took a negative
control to find out.

#### Amendments to earlier decisions

- **D22** — its `taxonomy_change` sketch is annotated in place with what was built
  instead. `affected_resolution_count` becomes `affected_binding_count`, and
  `renamed` is declined outright.
- **D72** — its impact functions are replaced, not extended. The definition was
  wrong in two directions at once.
- **J60** *(new)* — a move that recorded no blast radius.

**Rejects.** Counting resolutions, which is unbounded and the wrong unit.
Counting all bindings in the subtree, which is D72's error. Backfilling NULLs with
a recomputed number, which manufactures evidence. A trigger to verify the stored
count, which S7 forbids. Storing *which* bindings were affected as well as how
many — the SETOF function answers it live, and the list at the time is a bigger
claim than any screen has asked for.

---

### D74 — Retiring a class governs the editor, not the resolver

*Adopted 2026-08-08, settling question 149. Migration 32.*

D72 raised this by closing the door it names: it took DELETE on the class tables
away from the application, because the row carries a projection column and
deleting it takes the projection with it. The capability was already nearly gone,
but *"nearly is not a design: a class created by mistake now has no exit at all.
An `active` flag is the obvious answer and the obvious answer is what needs the
thought."*

#### The acts already had names

`policy_change_kind` has held `retired` and `reinstated` since migration 8, where
they describe a policy *version* leaving and returning to force. A class leaves
and returns the same way. **No enum values were added and no vocabulary invented**
— the two that exist now point at a second subject, exactly as D72 pointed
`reparented` at one. `retired_at` becomes a `@projection` on both class tables,
folded by the same maintainer, and `reinstated` folds it back to NULL so a class
can return without a special case.

#### The decision is what retirement means, and the tidier answer is wrong

**Retirement does not change resolution.** A retired class keeps its closure rows,
its bindings keep matching, and every member still classified into it keeps the
rules it had this morning.

The alternative — withdraw it from the closure — looks like housekeeping and is
the same failure D72 and D73 exist to prevent, arriving through a door nobody
inspects. Every item under that class would silently lose the policies scoped to
it. **A taxonomy edit that changes which stock ships must be a decision with a
blast radius in front of it, and this would be one that changed everything while
displaying nothing.**

So retirement speaks to the editor: do not offer this class for new
classifications, do not scope new bindings to it, do not show it in the picker.
It says nothing to the resolver.

Two consequences worth stating rather than discovering later:

- **A retirement has no blast radius**, and that is now a claim rather than an
  omission. D73's CHECK ties `affected_binding_count` to `reparented` rows, which
  reads as an accident until this migration; it is correct, because the number a
  retirement would carry is zero by construction.
- **Retiring is not deleting, and there is still no delete.** A class created by
  mistake is retired with a reason and the row stays — the same position this
  record takes on every other fact, and D51 does not delete an order line either.

#### What cannot be enforced, named rather than implied

*"No new members after retirement"* is **not checkable from the data**.
`item_classification` is a current-state table keyed on `(tenant_id, item_id)`
with no timestamp, so there is no moment to compare against `retired_at`.

A declarative form was looked for and rejected: a foreign key onto a partial
unique index over the unretired classes would forbid new members, and would also
forbid *retiring a class that still has any* — the ordinary case, and the one the
editor exists to warn about rather than prevent.

So **J61** checks the two things that are visible: a live child under a retired
parent, and a binding created after the class it names was retired. Neither is an
outage, because retirement changes no resolution; what they mean is that somebody
carried on as though the retirement had not happened, and the taxonomy now says
two things. The third case is invisible until classification becomes an act with
a moment, which no question has yet asked for.

#### Phase 4 found a down migration that had quietly become wrong

Migration 30's reversal deleted `change_kind = 'reparented'` rows before restoring
`NOT NULL` on `policy_binding_id`. That was right while re-parenting was the only
taxonomy act. **D74 made it wrong** — a `retired` row also has a taxonomy subject
and no binding — and the down failed on the fixture.

It is keyed on the subject now rather than on the kind, which is what migration 30
made possible and therefore what its reversal must remove. Worth recording because
the failure was not in the new migration: **an old reversal stopped being correct
because something new became expressible**, and only phase 4 of
`verify-migrations.sh` — the phase D56 added, running down *with data* — could see
it. D56 has now paid for itself twice.

#### Amendments to earlier decisions

- **D72** — its down migration is keyed on the subject rather than the change
  kind, and the taxonomy fold gains a second column.
- **D73** — `affected_binding_count` being reparent-only becomes deliberate.
- **J61** *(new)* — a retired class still being built on.

**Rejects.** An `active` boolean, which cannot say when or why. Withdrawing a
retired class from the closure, which silently restrips every member of its
policies. Deleting the row, which the application cannot do and should not.
A foreign key that would forbid retiring a class that still has members. Cascading
retirement to children, which would make one act mean an unbounded number of
others — the child is retired by its own act or it is not retired.

---

### D75 — The registers count themselves, because five times they did not

*Adopted 2026-08-08, settling question 150. No migration; a test.*

D72 raised it after correcting the fifth drift by hand and immediately producing
the sixth in the same edit:

> *"The counts are all derivable from the tables that follow them. The cost is a
> test that parses Markdown, which is why it has not been written."*

| the count | who found it | how wrong |
|---|---|---|
| the invariant total | D43 | one short of its own sections |
| the question total | D51 | understating by five |
| the `@projection` markers | D49 | — |
| the vacuity marks | D72 | 53 against a table of 54 |
| "the deferred section holds 35" | D72, correcting the one above | five out, then bumped to 37 |

**Every one was found by whoever next needed to touch the file, never by the file
itself.** The paragraph asserting the totals are derived rather than carried
forward was itself the fifth thing to drift — *a note asserting a property is not
a mechanism enforcing one*, which is what D49 concluded about `@projection` and
S5 learned the hard way.

#### What it checks

Four tests, no database, so unlike the rest of the suite they can never skip.

- The invariant register against its own tables: structural rows, job-asserted
  rows, the total, `specified`, and the vacuity marks — then the sentence that
  spells two of them out again in words.
- **The same register against the Rust that implements it.** This is the half a
  Markdown parser cannot see: an invariant in `ALL` with no row, or a row with
  nothing behind it. The counts now have to agree three ways.
- The question register: live questions across all five sections against the
  stated total, settled questions against theirs, and **no question listed as
  both** — being both means one table was edited and the other was not.
- The README, whose numbers are spelled out in English and should stay that way.
  Parsing words back into integers is the price of prose, and it is about forty
  lines.

Aliases are read rather than hardcoded: the duplicates table says 79 was retired
in favour of 93, so a row naming both is one question. The rule comes from the
table that records it.

#### It found a sixth on its first run, and my own script agreed with the wrong answer

The settled count read `~78`. The tilde was doing real work — it meant *nobody
has counted these* — and the true figure is **86**.

Worth recording precisely because of how it went. A throwaway script written to
check the number said 76, and 76 is also wrong: it matched rows beginning with a
digit, and ten settled questions live in rows whose first column is empty and
whose numbers sit in the third — `| | | 4, 58 | D39 |`. **Two hand-rolled counts,
two different wrong answers, and the stated one wrong in the other direction.**
That is the entire argument for 150 in one observation: the counting is not hard,
it is just never the thing anyone is actually doing at the time.

The tilde is gone with the number. An approximation mark on a derivable figure is
a licence for it to drift.

#### What it does not check

Per-section question counts. The five section rows are labelled differently from
the headings above them — "Live, business answer" against "Live, needing a
business answer" — and matching them needs either a lookup table that is itself
untested prose, or renaming headings this decision has no business renaming. The
total is derived from all five, so a section that gains a question without the
total moving is still caught.

Numbers 6 to 12 and 144 appear in neither register. Checked rather than assumed:
6 to 12 were absorbed by the consolidation the duplicates section describes, and
**144 has never appeared anywhere in the repository's history** — a skipped
number, not a lost question. No check asserts the sequence is dense, because it
is not, and asserting it would fail forever on a true fact.

**Rejects.** Generating the registers from the code, which would make the prose an
output and the prose is the point. Asserting the sequence has no gaps. A tolerance
on the settled count, which is what `~` was. Moving the counts into the test as
constants, which is the same hand-typing one directory across.

---

### D76 — The taxonomy could not be replayed, and the audit does not want a replay

*Adopted 2026-08-08, settling question 148. Migration 33.*

> *"Whether the closures become temporal. D72 makes a re-parent a dated fact, so
> `policy_change` now knows the taxonomy's shape at any past instant — but the
> closures hold only the present one [...] The cost is not small — a temporal
> closure is a range per edge, and D22's resolver walks it on every lookup."*

Four premises, checked before anything was built. **The first is false and the
other three argue against the thing the question asks for.**

#### `policy_change` did not know the shape at any past instant

A `reparented` row states where a class went. **Nothing states where it was.**
D74 declined `from_parent_id` reasoning that the previous parent is the previous
act, *"or the class's original parent if there is none"* — and the original parent
is exactly what D72 turned into a projection and overwrote.

Measured on the fixture before this migration: `GLOVE_SUPPLIER` was created at the
root and moved under `SUPPLIER` at 00:12, and **its parent at 00:11 was
unrecoverable**. One act naming a destination, over a base that no longer existed.

So a temporal closure would have been a precise index over a history with a hole
at the beginning of every class's life. **The first thing 148 needs is not ranges.
It is a base state**, and D22 asked for one: its sketch listed four actions, D72
built `reparented`, D74 built `retired` and declined `renamed`. This is the
fourth, and with it that sketch is finished.

A column would have been cheaper and is the wrong shape. Creating a class is a
governance act with an author, a moment and a reason, and `item_class` has no
timestamp of any kind to hang the other two off.

#### The shape at an instant, derived rather than stored

With a base, the closure at any instant is a fold and a walk — **no range, no
maintainer, nothing on the floor's path**, and the cost paid by the audit that
asks, which is the only caller there is.

**Both time axes are mandatory arguments**, because question 84 is about this
exact trap: *"what did the pallet weigh on Monday" and "what did we believe on
Monday" differ by one predicate, and getting it wrong in a dispute is worse than
not having the capability.* A single-timestamp function picks one silently. This
one cannot be called without saying which question is being asked, and the fixture
shows them disagreeing: the move valid at 00:13 is **one edge** as known now and
**none** as known at 00:11.

#### Why the temporal closure is not built

- **There is no resolver.** D70 says so in as many words. 148 prices the change as
  *"the resolver walks it on every lookup"* — a retrofit cost against code that
  does not exist. That is a reason to settle the shape now, not to build storage
  now.
- **The scale is unmeasured.** `history.sql` builds a year of movements and no
  taxonomy at all. D65, D69 and D71 each refused to answer a cost question without
  an instrument; this one has none, so any number here would be invented.
- **It would not answer the question 148 is triggered by.** *"The first audit that
  asks why a lot was accepted"* — `goods_receipt_line` records `accepted_at` and
  `accepted_by_id` and **nothing about which policy version governed the
  decision**. A replay would *reconstruct* an answer, not *report* one, and a
  reconstruction is a claim about the past rather than evidence of it.

#### The audit is served by recording the decision

That is already how this schema answers the same question elsewhere.
`stock_movement.item_packing_config_id` names the version that converted it (D57,
D58), `goods_receipt_line.item_packing_config_id` the same, and D73 froze a count
precisely because it could not be recomputed. **The missing half is policy**, and
it is missing because the resolver that would produce the answer does not exist.

So the commitment is **J63**, and it is `Pending` rather than absent: every act a
policy governed names the version that governed it. It blocks on a column that
does not exist, S42 asserts the column really is absent, and **the day a resolver
adds it the invariant wakes on its own**. The stamp arrives with the resolver, not
before — the position D70 took when it declined to invent a cache for a resolver
that does not exist.

#### Amendments to earlier decisions

- **D22** — the fourth of its four taxonomy actions is built, and the sketch is
  now fully accounted for.
- **D72, D74** — the parentage fold reads `created` as its base. D74's reasoning
  for declining `from_parent_id` was sound and rested on a base that was not
  there; it is there now, so the decision stands and its premise is repaired.
- **J62** *(new)* — a class that records no origin.
- **J63** *(new, pending)* — the decision names the version that made it.

**Rejects.** A temporal closure table, deferred to question 151 against a trigger
that names a resolver and a measurement rather than a feeling. `from_parent_id`,
which stores a value the previous act already implies once a base exists.
Backfilling creation acts for classes that predate this migration — J62 reports
them instead, because a synthetic act needs a synthetic `client_event` and that is
manufacturing evidence of a conversation nobody had, which D73 refused for the
same reason. Stamp columns on acts now, ahead of the resolver that would fill
them.

---

### D77 — D21, built: nine invariants had been waiting on one missing table

*Adopted 2026-08-08. Migration 34. No question settled — this is an adopted
decision that was never implemented.*

D21 was adopted on 2026-08-01 and nothing was built. **Nine invariants blocked on
it** — S17, S18, S35, J17, J18, J19, J47, J48, J49 — the largest blocked set in
the register, and no question deferred it. That is the failure mode D52 named:
*the check's stated precondition has since been met while it goes on reading as
deliberate*, except here the precondition was never met and nobody noticed either.

#### What was built

`party_message`, `assertion`, `assertion_stance`, `assertion_check`,
`despatch_advice`, `document_response`, `asserted_unit`, `asserted_unit_content`,
`inbound_shipment`, and a maintainer for the last one's five projections. D44's
two amendments are folded in where they touch the same tables: the two extra
kinds, and the message-to-message acknowledgement link.

**Rule 1 lives in the grants.** *"No UPDATE, no DELETE, ever. A revision is a new
assertion."* S7 forbids the trigger that would say so, so the application is
simply never given the privilege — the same mechanism D72 used for a projection.
Verified: the app cannot revise or delete a claim, and **can** write the
`resolved_*` columns, which is D21's stated exception and not an oversight.

**The typed bodies are declarative.** A body's `kind` is a stored generated
constant and the foreign key is composite, so attaching a despatch advice body to
an assertion of another kind fails on the key rather than on a trigger. Tested
before the migration was written, and again after.

#### Four invariants could not be woken, and say why

S42 caught all nine the moment the tables existed — *"the gap reads as deliberate
and is not"* — which is precisely what it is for. Five now run. The other four
were re-blocked on things that are genuinely absent:

| | blocked on |
|---|---|
| J18 | `observation_current` |
| J48 | `party_profile` |
| S35, J47 | the `asserted_unit` collapse at receipt, which turns a declared tree into packages |

**J19 was rewritten rather than implemented literally.** Its statement says
*"truncate every assertion table, rebuild `stock`, assert byte-identical"* — but a
job-asserted check runs against a live database and must not destroy it. It checks
the same property in the catalogue instead: no foreign key from `stock` or
`stock_movement` reaches an assertion table. D24's supply-side narrowing is
listed by name, because **an exemption that is written down is a decision and an
exemption that is silent is a hole.**

#### Two gaps the building surfaced

**Nothing represents us.** D21 makes the category symmetric — our outbound
despatch advice is as unrevisable as theirs — and `author_party_id` is NOT NULL,
so an outbound claim needs a `party` row for the operating company. There is no
such row and nothing would mark it as us if there were. The fixture adds one to
exercise the outbound arm; question 153 carries the real problem.

**`discrepancy` had no `(id, tenant_id)` key**, so the first composite tenant
foreign key into it could not be written. Added rather than dropping to a
tenant-blind reference — that shape is what D55 found open since migration 1.
`item` is the deliberate exception, referenced by id alone in twelve places, where
tenant agreement is J20's job.

#### What this does not build, stated rather than left to be discovered

Six of the eight kinds have no body table. The two that exist are the two with
invariants waiting and consumers in the record. **Inventing five more bodies with
no reader is how a schema grows columns nobody fills**, and the composite-FK
pattern is identical for each when one is needed.

Nothing parses an EDI message into these tables. J41 was pending on the ingestion
adapters before this migration and still is.

**The freeze on re-resolution is stated and unenforced.** D21: *"once an
`assertion_check` or a `goods_receipt_line` references it, it may not be
rewritten."* Nothing records when a `resolved_*` column changed, so it cannot be
checked — question 152.

**Rejects.** A trigger to enforce immutability. Bodies for kinds nothing reads. A
self-party flag invented here to close the gap this migration opened — that is
question 153, and D74 already refused the same shortcut for retirement.
Implementing J19 by truncating a live database.

---

### D78 — The current value of an observed thing, and who is allowed to set it

*Adopted 2026-08-08. Migration 35. No question settled — a second adopted decision
that was never implemented.*

D23 was adopted on 2026-08-01, the same day as D21, and its projection was never
built. Four invariants had been waiting: **J10, J11, J12, J18**. All four run now.

D77 built D21 and leaned on the boundary between the two decisions — an assertion
body holds identifiers, structure and the values a receipt compares line by line,
and *every other number with a unit is an observation whose observable is the
asserted unit*. **That boundary was asserted and unexercised.** The observation
side had no current value and no way to name an asserted unit as a subject.

#### The arms migration 34 should have added

`observable` states how its own union grows, in a comment written in migration 7:
*"each arrives as one column in the migration that creates its target."* Migration
34 created `asserted_unit` and `asserted_unit_content` and added neither column,
so D77 shipped a boundary it could not express. Corrected here.

It is also the argument for D23's registry paying off: widening the subject set is
two columns on a reference table of ~10⁵ rows, and nothing holding 10⁷ changes.

#### Acceptance, which is D21's cut applied to measurement

J11 — *"a counterparty-asserted observation enters `observation_current` only with
an acceptance"* — had nothing to check because nothing expressed an acceptance.
`observation_acceptance` is that act.

**A supplier's declared weight is theirs and unrevisable; adopting it as the number
we compute freight against is ours, and it is separate.** Without it, a message
sets the value an invoice is checked against by arriving. Measured on the fixture:
their pallet height is accepted and is current; their gross weight is not accepted
and is not, while our own scale reading stands.

#### The default precedence, written where it can be read

**Precedence is a policy D22 has not built** — the kind is not in `policy_kind`,
the value table does not exist, and D70 established there is no resolver. So the
maintainer applies a default, stated in the migration rather than left to be
inferred from an `ORDER BY`: retractions and corrections lose, a counterparty's
value needs an acceptance, **ours beats theirs**, then the most recent.

Rule four is the one a policy will most likely overturn — D23's own example wants
supplier dimensions where we have never measured — and until it can be configured
the safe direction is the one that never lets a message overwrite a measurement.

#### Two invariants threw out a column I had written

`observation_precedence_policy_id` went in nullable, because J10's statement names
it. **S16 refused it**: a `<kind>_policy_id` column must be a foreign key to its
value table, and there is no `observation_precedence_policy`. **S43 refused it**
independently: it was registered to a maintainer whose body never writes it, so
*"the column keeps whatever it was last set to while every existence check
passes."*

Both are right, and they reached by machine the conclusion D70 reached by
argument when it declined to invent a cache for a resolver that does not exist.
The column arrives with the policy. J10's register statement is amended to name
the default rule instead of a column that does not exist, which is the register
being corrected by the suite rather than by somebody noticing.

#### `observation` had no tenant key either

The second table in two migrations found carrying `PRIMARY KEY (id)` alone, after
`discrepancy`. Added for the same reason — but the audit that followed found
**thirty-two tenant-scoped tables without one**, so this is a mixed convention
rather than two oversights, and J20 is currently the only thing between it and a
cross-tenant reference. Question 155.

#### Amendments to earlier decisions

- **D77** — the `observable` arms it owed D23.
- **J10** — its statement drops the precedence policy row it can no longer name.
- **J11, J12, J18** — implemented as written.

**Rejects.** `cube_numeric` and `in_breach`, which D23 lists: the first is computed
by its own rule and is three metrics multiplied anyway; the second needs a
`specification_policy` that does not exist. Adding `observation_precedence` to
`policy_kind` without a value table, a binding or a resolver — vocabulary with
nothing behind it, which D77 declined for the same reason. Feeding `package`
dimensions from this projection, which D23 forbids in writing because a shipped
package's dimensions are a historical fact a freight invoice was computed against.

---

### D79 — A reference to another tenant's row stops being writable

*Adopted 2026-08-08, settling question 155. Migration 36.*

D78 raised it after two migrations running found a table with `PRIMARY KEY (id)`
alone when a composite tenant foreign key wanted a target — `discrepancy`, then
`observation`. The audit that followed found **114 foreign keys between
tenant-scoped tables carrying no tenant column, against 69 that did.**

And the justification written into migration 34 for following the weaker form —
*"tenant agreement on an item reference is J20's job"* — was far more generous
than J20 is. **J20 checks two joins**, `stock_movement.item_id` and
`package_event.package_id`. Not the class. That sentence is a committed migration
comment, which is why this decision names it rather than quietly fixing it.

RLS is not the answer either: it filters what a tenant **reads**, and says nothing
about what a row may **point at**. D55 found exactly that hole open since
migration 1 and closed it by hand.

#### The convention was only half wrong

The split turned out to be two genuinely different kinds of table, and the audit
only became actionable once they were separated.

| | |
|---|---|
| **Strictly owned** — `tenant_id NOT NULL` | every row belongs to one tenant; a cross-tenant reference has no legitimate reading, so **the key must refuse it** |
| **Shared reference** — `tenant_id` nullable, D55's policy pair | platform rows belong to nobody, so a composite key is *impossible*: the child's tenant is real and the parent's is NULL |

`item` is the second kind. Twelve tables reference it by id alone and **that was
right rather than lazy** — which is why the mixed convention survived so long
without looking like a defect.

So the rule is: every foreign key to a strictly-owned table carries `tenant_id`.
Sixty-four constraints were rewritten, nine tables gained the `(id, tenant_id)`
key to be referenced by, and the body was generated from the catalogue rather than
typed, which is the only honest way to touch that many.

Two constraints are exempt and named, both already tenant-determined through a
column another invariant requires: `location_zone_fk` carries `site_id` for S37,
`intention_amendment_line_fk` carries `order_id` for S41.

#### S51 found something worse on its first run

It reported `consignment` as referenced and strictly owned with no key to be
referenced by. The reason was not a missing key.

**`consignment_package` had no `tenant_id` and no row-level security at all.** Two
columns, both foreign keys, linking a tenant's packages to a tenant's consignments
— and nothing required them to be the *same* tenant, nothing filtered reads, and
the application held INSERT, UPDATE and DELETE across every row of it. It is D55's
finding one step further along: not a policy missing its `WITH CHECK`, but a
table with no policy at all.

It is the only table of that shape. The others without `tenant_id` are global
reference, policy value tables that reach tenancy through `policy_binding`, or the
projection registries. It now carries a tenant, composite keys both ways, and a
tenant-scoped policy.

#### Both halves are enforced, and neither is remembered

- **S51** *(new)* — every foreign key to a strictly-owned table carries its
  tenant, and every such table offers the key. Structural, so a new tenant-blind
  foreign key fails the suite the day it is written.
- **J64** *(new)* — every reference to a shared-reference row resolves to the
  referencing row's own tenant or to a platform row. It cannot be a key, so it is
  data, walked from the catalogue so a new shared-reference table is covered the
  day it exists. Twenty-eight edges today.

**S38 had to be widened**, not because it was wrong but because it was literal: it
required `REFERENCES stock_movement(id)` and the composite form is the same
self-referencing link with a tenant carried alongside. Reading it literally would
have made hardening the tenancy boundary look like a regression.

**Rejects.** Composite keys to shared-reference tables, which cannot exist.
Narrowing S51 so `consignment_package` would pass — the check was right and the
table was wrong. Leaving the two exempt constraints unnamed. Trusting RLS, which
is a read filter. Trusting J20, which checks two joins.

---

### D80 — The ceiling is data, so there is no number to disagree with

*Adopted 2026-08-08. Migration 37. A third adopted decision that was never built.*

D26 was adopted on 2026-08-01 and D36 settled its ceilings on 2026-08-03. Neither
was built, and three invariants had been waiting: **J21, J39, J40.** This is D36's
declaration layer — the tables a schema compiler must claim against before it runs
any DDL.

#### Why the ceiling is rows

D36's argument is narrower and stronger than *"undefined behaviour is bad"*:
**by the time a job runs there is no action left that is not worse than the
violation.** The tenant declared, the compiler ran DDL, the table exists and rows
are landing in it. Tomorrow's job can drop the table — destroying tenant data in a
model whose generated tables carry `ON DELETE RESTRICT` precisely so evidence is
never destroyed as a side effect — or it can raise a finding and change nothing,
which makes the ceiling a note.

So it holds before the DDL or it does not hold, and `CREATE TABLE` being
transactional is what makes that possible: a failed claim inside the
materialisation transaction rolls the DDL back with it.

It also cannot be a constant. D25 forbids validation triggers by name, a `CHECK`
cannot hold a subquery, and a counter column is a projection with no fact behind
it. **A number in a `CHECK` and a number in a plan description are two
representations that will eventually disagree**, and this design has one: issued
rows. Raising a ceiling is inserting slots — a platform act with a row and a
timestamp, not a migration.

Measured on the fixture: three slots issued, three claims succeed, the fourth
returns NULL. **The ceiling is a defined outcome rather than an error class**, and
re-claiming a held key returns the slot it already has, so declaring version N+1
costs nothing.

#### The two shapes, and the test between them

The 60-field limit is a constant, and D36 says why that is not a contradiction:

> **A ceiling that is per-tenant and commercially variable is issued slots. A
> ceiling that is per-parent and fixed by design is an ordinal with a `CHECK`.**

Sixty fields is a statement about what one table should hold before it wants to be
two, and that is the same for every tenant on every plan. The 61st has nowhere to
go, declaratively and with no trigger.

#### What the invariants add that the constraints cannot

The foreign key gets a scheme to *a* slot. It cannot say the slot is claimed by
*this* scheme's key, nor that two keys have not pointed at the same ordinal — and
both would break the ceiling while every existence check passed.

- **J39** checks that a scheme holds the slot it names, and that no ordinal is
  claimed by two keys. Shipped schemes are exempt and it is not an oversight:
  `tenant_id NULL` consumes no tenant's ceiling.
- **J21** is mostly structural now — a claim is an UPDATE of an issued row, so
  claimed can never exceed issued. **What is not structural is withdrawal.** D36
  is explicit that lowering a ceiling must never destroy a schema, and a withdrawn
  slot still holding a key is that failure in progress: the platform has taken the
  slot out of issue while a tenant's table is still standing on it.

#### Slot release is not built, and a function that always said yes would be worse

D36: *"a slot is released only when every table its scheme ever materialised has
been archived under D31."* D31's archival is not built — nothing records that a
table was archived — so a release function could not check its own precondition.
**J40 stays pending on `retention_floor`** rather than being answered by a
function that cannot fail.

`plugin_id` is absent for the same reason S16 and S43 removed a column from D78
an hour earlier: there is no plugin registry, so it would be unfillable.

**Rejects.** A ceiling constant anywhere. A validation trigger, which D25 forbids
by name. A counter column, which is a projection with no fact behind it. A release
function that cannot check the condition it exists to enforce. Building the
compiler, the outbox or tenant metrics — the claim is the half that cannot be
added afterwards, and the rest is application code with its own decisions.

---

### D81 — The eleven precedence orderings, and the argument for each

*Adopted 2026-08-08, settling questions 93 and 79. No migration; a crate.*

> *"Eleven orderings of six dimensions, declared in a Rust const. Counterparty
> over product for shelf life, product over space for putaway — both defensible,
> neither obvious, and a manager who assumes wrong misconfigures confidently.
> Each needs a written justification, not just a declaration."*

The research note calls it **"the highest-leverage undocumented number in the
system"**, and it is right for a reason worth naming: **the ordering is invisible
in the UI and decides the answer.** Two managers configure the same intent, get
different stock shipped, and neither sees why.

So the deliverable is the argument. The `const` was always going to exist.

#### The rule that generates them

An ordering is not a ranking of how important the dimensions are. It answers one
question: **when two people have configured this kind at different depths on
different axes, whose statement was more likely meant as an override?**

- **Commercial terms belong to the counterparty.** Shelf life on arrival, delivery
  tolerance, receiving discipline — a promise to a named party beats a default.
- **Physical handling belongs to the product.** Where a thing may be stored, how
  it is sampled, how tightly counted, follow from what it is.
- **Measurement rules belong to the metric.** A statement about temperature is
  about temperature, whatever carries it.
- **Space and Ownership are filters far more often than authors.** A site rarely
  means "and I intend to override the customer's contract".

#### The orderings

| kind | after Tenancy | the short reason |
|---|---|---|
| `allocation` | Counterparty, Product, Space, Ownership, Metric | which stock may go to a customer is a promise |
| `putaway` | Product, Space, Counterparty, Ownership, Metric | **stated by the record already** |
| `receiving` | Counterparty, Product, Space, Ownership, Metric | a site cannot know a supplier is unreliable |
| `order_tolerance` | Counterparty, Product, Space, Ownership, Metric | a commercial term, quoted back in a dispute |
| `count_tolerance` | Product, Space, Counterparty, Ownership, Metric | stock being counted is ours whoever sold it |
| `shelf_life` | Counterparty, Product, Space, Ownership, Metric | **stated by the record already** |
| `sampling` | Product, Counterparty, Space, Ownership, Metric | the regime follows the hazard |
| `cycle_count` | Product, Space, Counterparty, Ownership, Metric | no counterparty has a view on our cadence |
| `specification` | **Metric**, Product, Counterparty, Space, Ownership | a spec is a statement about one attribute |
| `observation_precedence` | **Metric**, Counterparty, Product, Space, Ownership | D23's own example is metric-shaped |
| `observation_acceptance` | Counterparty, Metric, Product, Space, Ownership | trust in a party, not in a number |

Three are worth reading twice.

**`specification` leads on Metric**, which is the ordering most likely to
surprise. A specification is a statement about one attribute — two to eight
degrees, under fifteen percent moisture — so a binding naming the attribute is
speaking about the thing being specified, and one that does not is a default for
everything measurable. **A statutory limit a customer must not be able to loosen
is not modelled by precedence at all**: it is a platform-shipped clamp, which D22
already provides.

**`observation_precedence` leads on Metric** straight from D23's example —
*"trust supplier dimensions for items we have never measured, but never trust
their weight over our scale"* — which is a rule about *which metric*, and is
inexpressible if Counterparty outranks Metric. D78's maintainer applies a default
in place of this until the kind is built.

**`observation_acceptance` inverts it**, and the inversion is the point. Whether a
declared value needs a human to accept it is a question about trust in a *party*;
which value wins once accepted is a question about the *metric*.

**`sampling` is the one genuinely arguable case.** Product first because the
regime follows the hazard, Counterparty a close second because supplier risk is
the standard modifier. Recorded as arguable rather than settled by taste.

#### Tenancy, and the hole it closes

Tenancy is not declarable and is index 0 of all eleven. D22: without it, an order
ranking Product above Tenancy would let a platform-shipped default outrank a
tenant's own configuration — *"a correctness hole, not a support surface"*.
**S15 now runs**, having been pending on a registry that did not exist, and
asserts both that and well-formedness: an ordering naming five dimensions would
leave pairs unorderable, and the failure would present as a tie rather than as a
missing declaration. Measured: moving Tenancy off index 0 for `shelf_life` fails
the crate's own test and S15, with the sentence naming the consequence.

#### Where it lives, and what it is for

`crates/policy`, which the resolver will share — the registry is domain code, not
a checking artefact. The justification is a **method** rather than a comment so
the explain screen can show a manager the reasoning beside the answer, which is
what 79 means by *"the explain UI must ship with the resolver, not after it."*

S13 needed reconciling: the registry declares eleven and the database has three
value tables, so a registry entry with no enum value is **not** a violation — it
is a kind that is designed and unbuilt. The reverse still is: an enum value the
registry has never heard of.

**Rejects.** Leaving the orderings as a bare const, which is the state this
settles. A single global ordering, which would make `specification` and
`shelf_life` answer to the same axis. Ownership above Space anywhere — for a 3PL
client the owner may well be an author rather than a filter, and **that is
questions 65 and 81**, which are open; the entry for `allocation` names it as the
first candidate to move.

---

### D82 — The resolver: matching in SQL, ordering in code

*Adopted 2026-08-08. Migration 38 and `crates/policy`. No question settled — D22's
resolver, which had been designed since 2026-08-01 and never built.*

Everything has been waiting on this. D70 built the cache-invalidation contract for
a resolver that did not exist. D73 derived impact numbers for a screen that would
call it. **D78 hard-coded a precedence rule into a maintainer because there was
nothing to ask.** J22, J28, J29, J63 and S23 all block on it.

#### The split is D22's, not a convenience

Matching is *ancestor-or-self over closures*, which is where the data is.
Ordering is *lexicographic over a per-kind precedence order*, which D22 puts
**"in code, where the precedence order is a visible `const DIMENSIONS` on the
kind's Rust type"** and D81 justified one kind at a time.

So `policy_candidate` returns every binding whose declared axes are at-or-above
the request, each with its depth vector, and `nylonite_policy::resolve` decides
which wins. **Neither half is the resolver**, and a test of either alone passes
while the join is wrong — which is the only way this fails in practice, so the
end-to-end test is the one that matters.

#### Specificity, and the collision the first draft had

A component is how specific a binding is on that axis, zero meaning it declared
nothing. A class's depth is its **ancestor count including itself**, so a root
class scores 1 and "any" keeps 0.

Counting ancestors *excluding* itself was the first draft, and it collided:
measured on the fixture, a binding on `PPE` scored 0 — **the same as a binding
declaring no product at all**. "All protective equipment" and "anything
whatsoever" would have been equally specific, and D22 makes equal vectors a
`policy_ambiguous` discrepancy, so the failure would have arrived as a tie rather
than as a wrong answer.

An item scores 1000 rather than "deepest class plus one", because computing that
per request makes the answer depend on the shape of the taxonomy *elsewhere*. A
constant no class depth can reach says the same thing and cannot drift.

#### What the tests establish

- **A tenant binding beats a platform one on all eleven kinds**, even when the
  platform row names the exact item and the tenant row names nothing. That is
  S15's property observed rather than asserted, and D22's *"correctness hole, not
  a support surface."*
- **The same candidate rows answer differently under different kinds.** On the
  fixture, `shelf_life` picks the counterparty-class binding and `count_tolerance`
  picks the one naming the item — the same four rows, two answers. **That is the
  whole reason 93 had to be settled first**, and it is now a test rather than an
  argument.
- **A vector is not a number.** A candidate deep on three low-weight axes loses to
  one shallow on a high-weight axis; summing would reverse it.
- **A version effective from August does not match in February.** The half D70
  said no epoch can see, because nothing is written when a range opens.
- **Equal vectors take the lower binding id and say so.** The floor never stops —
  D5 — and the caller raises `policy_ambiguous` rather than the resolver deciding
  quietly.

#### The explanation ships with it, because 79 said so

*"The explain UI must ship with the resolver, not after it."* `explain` names the
dimension that decided and carries **D81's argument for that kind verbatim**, so a
manager reads why counterparty outranks product beside the answer it produced.
Shipping the resolver without it is precisely how the ordering stays invisible,
which is the failure 93 describes.

#### What is not built, and the blockers that were reworded

The **value row** is not fetched: each kind's `%_policy` table has its own columns
and its own clamped fields, so that belongs with the kind, and three of eleven
have a table at all. **Clamping** — D22's per-field floor against every less
specific match — is therefore not built either.

Three invariants named "the resolver" as their blocker and it now exists, so
saying so would be false in the direction S42 exists to catch. **J22** blocks on a
golden snapshot and a `RESOLVER_VERSION`; **J28** and **J29** block on the
supply-side job that applies a resolved receiving policy to a promise — *D82 built
the resolution, not the caller*.

**Rejects.** Ordering in SQL, which would bury the precedence order in an ORDER BY
nobody reads and make D81's justification decorative. A `specificity` column,
which D22 refuses and CSS spent twenty years proving wrong. Fetching value rows
generically, which would mean a JSON bag or eight unused columns. Breaking ties by
anything other than the lower id — the floor must never stop, and the discrepancy
is the record that it happened.

---

### D83 — The winner's value, and the floors it cannot go under

*Adopted 2026-08-08. Migration 39 and `crates/policy`. Finishes D22's resolver.*

D82 named a winning binding and stopped there, so nothing could use it: `resolve`
returned an id and the value was still in a table nobody read. D22 asks for two
things here and the second is the interesting one — *"takes the winner's value
row, then clamps any field the value type declares as clamped"*, and on D14, **"so
a site floor raises a customer rule."**

#### One function in long format, not three in row format

The obvious shape is a function per kind returning that kind's row, and it is
wrong for the reason clamping is generic: **the clamp is per field and needs every
candidate's value for that field, not just the winner's.** In row format that is
three code paths that must each remember to fetch the losers. In long format —
`(binding, field, value)` — it is one, and a fourth kind adds a `UNION ALL` rather
than a code path.

Only degrees are returned. `receiving_policy.default_status_id` is a *choice*
rather than a degree, so it comes from the winner whole and is not clampable at
all. Booleans are `0` and `1` so `false < true`, and a floor on one means "no less
strict" — the same sentence as a shelf-life floor rather than a special case.

#### What clamps, and the one that clamps nothing

| kind | field | direction |
|---|---|---|
| `shelf_life` | `min_shelf_life_days`, `min_shelf_life_pct` | **floor** |
| `receiving` | `respond_by_hours`, `tolerance_over_pct`, `tolerance_under_pct` | **ceiling** |
| `receiving` | `require_lot` | **floor** |
| `allocation` | — | **nothing** |

Tolerances are permissions, so the less specific rule is the outer bound: a tenant
cannot accept a larger over-delivery than the platform allows, nor take longer to
respond. That is D22's *"a commercial product a platform-shipped ceiling no tenant
can exceed."*

**`allocation` declares no clamped fields, and that is the decision rather than an
omission.** D22, in its own words: *"you must not take `weight_rotation` from a
customer binding and `weight_travel` from a site binding, because weights are only
meaningful relative to each other."* The whole row wins or none of it does.

Kinds with no value table declare nothing, because **a clamp direction invented
before the field exists is a guess that will be obeyed.**

#### The assertion the four migrations were for

Measured end to end on the fixture: a customer negotiated **sixty** days of shelf
life, and a rule on the item itself requires **a hundred and twenty**. The
customer's binding *wins* — D81's ordering puts Counterparty above Product for
`shelf_life` — and the goods still ship at a hundred and twenty.

Both halves matter. The customer's binding is the one that decided, and the
customer did not get to decide the number. **A manager reading only the winner
would see sixty and be wrong**, which is why D22 says the resolver returns *"an
explanation, not a value — winner, what clamped it"*, and why `Clamped` names the
field, the direction, what the winner said and which binding bound it.

The floor takes the strictest of the less specific, not the least specific: the
platform's thirty is also a match and does not win the floor. Asserted, because
"clamp against the less specific" reads as though the least specific should
dominate and it must not.

#### Where the direction lives

Beside the precedence orderings, for D81's reason restated: **a clamp direction is
as invisible and as consequential as a precedence order.** A manager who believes
a customer can shorten a shelf-life floor is wrong in exactly the way one who
believes product outranks counterparty is wrong, and neither is visible in the
answer without being told.

**Rejects.** A function per kind, which hides the losers. Clamping a choice — a
default status has no direction to be clamped in. Clamping `allocation`'s weights,
which D22 forbids by name. Declaring directions for the eight kinds with no value
table. Returning the clamped value without saying what moved it, which is the
failure D22 names when it insists the resolver returns an explanation.

---

### D84 — The golden snapshot: the only guard that watches meaning

*Adopted 2026-08-09. `crates/policy`, `crates/invariants`, and a recorded file.
J22, which had been pending since D22 was adopted.*

> *"Golden snapshot: adding a scope dimension changes no existing resolution
> without an explicit `RESOLVER_VERSION` bump."*

Every other check in this suite asserts something about **structure** — a column
exists, a grant is absent, a projection agrees with its fold. This one asserts
that **the answers have not moved**, and it is the only guard that would notice a
change to what ships.

It could not be built before D83. Snapshotting a resolver that named a winner and
no value would have frozen an interim answer, and the interesting cases are the
ones where the winner and the value disagree.

#### What it caught, which is the argument for it

Three controls, and the third is the one worth keeping.

**A precedence ordering re-argued.** Flipping `shelf_life` to Product-first
changed the answer from the counterparty binding to the item binding, and J22 said
so with both answers in the message.

**A version bump with a stale file.** The snapshot is refused rather than
accepted: *"regenerate it and read the diff, which is what the bump is for."* The
bump does not make the check pass — it makes somebody read what changed.

**A re-parent, with no code change whatsoever.** Moving `GLOVES` out from under
`PPE` is one recorded act under D72, entirely legitimate, and it changed two
answers:

    shelf_life/glove+gloveco
      was: counterparty class wins on counterparty, clamped 60 -> 120
      now: counterparty class wins on tenancy

**The floor vanished.** The binding that required a hundred and twenty days named
`PPE` as well as the item, so it stopped matching when the item left that class —
and the customer's sixty-day figure, which D83 exists to prevent shipping, would
have shipped. Nothing in the schema changed. No constraint fired. Every structural
check still passed.

That is the whole case for J22 in one move: **the taxonomy is an input to what
ships, D72 made editing it an act precisely because of that, and until now nothing
compared the before to the after.**

#### The shape

`RESOLVER_VERSION` is a version of *what resolution means* — not a release
number. It moves when an answer legitimately changes: an ordering re-argued, a
clamp direction flipped, a specificity constant redefined, a dimension added.

The cases are chosen so that **each one would move if a different thing broke**
rather than to cover rows: a decision on Counterparty, the same rows under a kind
ordered the other way, a winner overridden by a floor it did not set, three clamps
at once against a platform ceiling, a request before any tenant version was
effective, and a request naming no counterparty.

The file is text, sorted, one `case = answer` line, and the answer names the
winning binding by its **note** rather than its id — so a diff reads *"the
platform default now wins"* instead of two uuids. **Its whole value is in being
read when it changes**, which is also why regeneration is a separate opt-in test
rather than something the suite does when it notices a difference. A snapshot that
rewrites itself watches nothing.

#### Amendments

- **J22** — implemented, having been pending on a resolver since 2026-08-01.
- **D72, D73** — the blast radius they made visible before a move is now checkable
  after one. D73 derives what a move *would* change; this notices what a move
  *did* change, including changes nobody asked for.

**Rejects.** JSON, which nobody reviews. Automatic regeneration, which converts a
failing guard into a silent one. Snapshotting binding ids. Recording every
resolution the fixture can produce — six cases that each fail for a different
reason are worth more than sixty that fail together.

---

### D85 — Which observation wins, configured rather than compiled

*Adopted 2026-08-09, settling question 154. Migration 40.*

D78 hard-coded a precedence rule into `projection_observation_current_rebuild` —
*ours beats theirs, then the most recent* — and said why: the kind was not in
`policy_kind`, the value table did not exist, and D70 had established there was no
resolver. All three stopped being true, and this is the debt paid.

#### A correction to D78

D78 said *"trusting theirs where we have nothing is a policy decision and is not
made here."* That reads as though the default refuses it, and it does no such
thing: the fold orders ours before theirs and takes what is left, so where we have
measured nothing an accepted counterparty value **already** becomes current — the
fixture's pallet height is exactly that. What the default refuses is theirs
beating **ours**. The distinction is the whole of D23's sentence and D78 blurred
it.

#### Two fields, both read straight out of D23

> *"Trust supplier dimensions for items we have never measured, but never trust
> their weight over our scale."*

The first clause is `accept_counterparty`, the second is `prefer_own`. **Nothing
else is invented** — an age limit, a confidence threshold and a device-class rule
are all plausible and none is in D23.

Demonstrated end to end: our scale reads 500 g, the supplier declares 505 g and it
is accepted and fresher. Under the resolved policy ours is current; with
`prefer_own` off, theirs is; with `accept_counterparty` off, no counterparty value
is current at all. The observations never change.

#### The boundary this had to cross

**The maintainer cannot resolve the policy itself.** D22 puts the precedence order
in Rust *"where it is a visible const"*, D81 justified it there, and a second
ordering written in PL/pgSQL is the two representations that eventually disagree.
So the maintainer takes the answer as arguments and the caller resolves. The
defaults are D78's rule, so `projection_run_all` behaves exactly as before and the
change is opt-in per caller rather than a silent flip.

#### Three findings on the way, and they are the substance

**A hole in a whole table class.** The first draft gave the new value table a
row-level policy and S9 rejected the shape. Checking why turned up the real
problem: `allocation_policy`, `receiving_policy` and `shelf_life_policy` had
**row-level security disabled entirely**, with `nylonite_app` holding SELECT,
INSERT and UPDATE. Any tenant's connection could read and rewrite another tenant's
policy values. **D79 missed it**: value tables carry no `tenant_id` column, so an
audit that classified every table as strictly-owned or shared-reference skipped
them as neither. Their tenancy is real and *indirect*, and indirect is what an
audit keyed on a column name cannot see. All four now carry a policy, and S9 gains
the shape as a fourth permitted expression.

**The same coupling in three places.** Migration 38 predicted it — *"a fourth
would have to be added here, which is exactly the coupling S50 exists to catch"* —
and misattributed the guard: S50 is about what the *epoch* sees, this is about
what the *resolver* sees. Within the hour a fourth kind resolved to **nothing at
all**, because `policy_candidate` did not know its table existed. Then J13
reported its perfectly good version as missing, from a hardcoded list of three.
**Resolving to nothing is the worst available failure** — not an error, not a
default, but a legitimate answer the caller cannot distinguish from a missing
table. **S52** now checks both SQL functions from the catalogue, and J13's list is
read from the catalogue rather than written down.

**A parameter list is not a replacement.** `CREATE OR REPLACE` with new arguments
makes an *overload*, and `projection_run_all` calls maintainers by name with one
argument, so both matched and the call was ambiguous. The fixture found it on the
first run.

#### The golden snapshot gains a case

`observation_precedence` decides what a freight invoice is checked against, so it
is watched like the rest. Regenerated **without** a `RESOLVER_VERSION` bump, and
the diff is why: it is a pure addition and no recorded answer moved. **Adding
coverage is not changing meaning**, and J22 already distinguishes them — a case
that is new reports differently from an answer that differs.

**Rejects.** Fields D23 does not ask for. A precedence ordering written in
PL/pgSQL so the maintainer could resolve for itself. Letting the value tables keep
no row-level security for consistency with each other, when what they were
consistent about was a hole. Bumping the resolver version for an added case.

---

### D86 — One answer per metric, and the row that says which policy gave it

*Adopted 2026-08-09, settling question 156. Migration 41.*

Two halves that are one change.

#### D23's sentence, finally obeyed

D85 handed the maintainer **one answer for the whole run** — right while every
binding is tenant-wide, wrong the moment one names a metric, and naming a metric
is exactly what D81 ordered this kind for:

> *"Trust supplier dimensions for items we have never measured, but never trust
> their weight over our scale."*

**That is two answers for two metrics, and until now it could be stored and not
obeyed.** Measured on the fixture, with one supplier and one pallet: under a
single answer their declared gross weight becomes current; under the resolved
per-metric decisions their dimensions are accepted, their weight is refused
outright, and our own scale reading stands.

The maintainer still does not resolve — a second precedence ordering in PL/pgSQL
is the two representations that eventually disagree — so a decision is a **value**
it can be passed. A composite type rather than parallel arrays, and certainly
rather than `jsonb`, which principle 3 forbids. `metric_id IS NULL` is the
fallback, which is the same shape as a NULL axis meaning "any" everywhere else in
D22's lattice, and the default is D78's rule expressed as one such row — so
"nobody resolved anything" and "the resolver said exactly this" travel one code
path rather than two.

#### The column S16 and S43 threw out, back for the right reason

D78 added `observation_precedence_policy_id` because J10's statement named it, and
both invariants refused it: S16 because a `<kind>_policy_id` column must be a
foreign key to its value table and there was none, S43 because it was registered
to a maintainer whose body never wrote it. **Both objections are gone** — D85
built the table, and this writes the column.

D23's general rule is what it is for: *"any projection maintained under a policy
must record the policy row that produced it. Otherwise the rebuild-and-assert job
reports every policy change as drift — the projection was correct under the old
policy and correct under the new one, and a rebuild cannot tell the difference
without knowing which applied."*

**J10 is that job**, and it now recomputes each row under the policy that row
records rather than under one global rule. Without this change it would have
reported the fixture's per-metric run as drift the moment an orchestrator used it
— the policy working, reported as the projection broken.

#### Amendments

- **D78** — the column it lost is restored, with the table and the writer that
  were missing.
- **D85** — its single decision becomes the `metric_id IS NULL` fallback rather
  than being replaced.
- **J10** — recomputes under the recorded policy.

**Rejects.** `jsonb` for the decisions. Parallel arrays, which are a composite
type with the type removed. Resolving inside the maintainer. Leaving J10 on one
global rule, which would have made the first real use of a per-metric policy look
like a defect.

---

### D87 — The receipt names the policy that governed it

*Adopted 2026-08-09. Migration 42. J63, pending since D76.*

**This is the invariant that makes D76's answer true rather than aspirational.**

Question 148 asked whether the closures should become temporal so a past
resolution could be replayed. D76 answered that the audit is served by **recording
the decision, not by replaying the world** — and then `goods_receipt_line` went on
recording who accepted a line and when, and nothing about which tolerances it was
measured against. *"Why was this lot accepted"*, which is the question 148 was
triggered by, stayed unanswerable for three more days.

#### Why it cannot be reconstructed, in three legitimate ways

- The binding may have been **superseded** — D22 makes scope immutable and a
  change a new binding.
- Its effective range may have **closed with nothing written**, which D70
  established no epoch can see.
- The taxonomy it resolved through may have been **re-parented**, and D84 measured
  exactly that silently removing a shelf-life floor.

Each is a normal act by someone doing their job. Together they mean a
reconstruction is a claim about the past rather than evidence of it, which is
D76's sentence and now has a column behind it.

#### The blocker's name was mine, and it was wrong

D76 wrote it as `goods_receipt_line.decided_by_receiving_policy_id`, and that
name would have **failed S16**: it derives the value table by trimming
`_policy_id`, so it would look for a `decided_by_receiving_policy` table. The
convention is already set by `expected_supply.receiving_policy_id`, and a second
spelling for one relationship is how a rule becomes unenforceable. The column
takes the conventional name; **the invariant's statement was right and only its
blocker was wrong**, which is the distinction S42 cannot draw and a reader has to.

#### What is deliberately not enforced

Not NOT NULL: a line dispositioned before this legitimately has none, and
backfilling would invent evidence of a decision nobody recorded — D73's objection
to a backfilled blast radius and D76's to a backfilled class origin, a third time.
J63 reports them, so the set is visible and shrinking.

Not frozen: once a receipt has been compared against a version that reference
should not move, and nothing records when it changed. Same gap as question 152
carries for a `resolved_*` annotation, for the same reason.

Not yet written by anything: no receiving path resolves a policy and stamps it,
because D62 built that path before there was a resolver to call. **J63 is what
will notice the first line dispositioned without one** — which is the point of
implementing it before the caller exists rather than after.

**Rejects.** A second spelling of a column that already has a convention.
NOT NULL. Backfilling. Waiting for the receiving path, which would leave the
check pending against a column that exists — the state S42 exists to refuse.

---

### D88 — What a resolved receiving policy decides, and what is still missing

*Adopted 2026-08-09. `crates/server/src/receiving.rs`. No migration.*

D87 gave `goods_receipt_line` a column for the policy that governed it and left
J63 watching a column nothing fills. This is the decision that fills it — and the
scope needs stating plainly, because it is smaller than it sounds and the reason
is the interesting part.

#### There is no acceptance path, and there never was

`receiving.rs` plans the **handover**: which claims move from a promise to the
stock it became. It does not accept or reject a line, and nothing else does
either. `goods_receipt_line.accepted_at` and `accepted_by_id` have been written
only by the fixture since migration 21.

So "wire the receiving path to resolve and stamp" had no path to wire. What is
built instead is the piece that was genuinely missing and can be built correctly
now: **the decision a resolved policy makes about a line**, as a pure function in
the same shape as `plan`, with the version it decided under carried through it.

#### What it decides, and what it refuses to invent

The rules are the policy's own fields and nothing else. **No threshold, grace or
rounding is invented here**: D83 declared which fields clamp and in which
direction, and a rule this function made up would be a fourth place the answer
lives, after the value table, the registry and the clamp.

- **Over-delivery is recorded, not refused.** The pallet is on the dock and D5
  will not have a receiver blocked to protect a number. What the tolerance decides
  is whether it is a discrepancy.
- **A missing lot the policy requires is refused**, because accepting it puts
  stock on the floor that cannot be recalled by lot and D14 makes expiry a
  property of the lot.
- **Under-delivery is a fact rather than a refusal.** Short closes the promise
  short and is visible in `quantity_closed_short`; `tolerance_under_pct` governs
  when that becomes a conversation with the supplier, which is a claim against
  them rather than a reason to turn goods away.

**The version is stamped on a refusal too.** J63 is about *governed acts*, not
accepted ones: *"why was this rejected"* has the same answer missing if nothing
records it.

#### The assertion that matters

The tenant asked for twenty-five percent over-delivery and D83's ceiling brought
it to ten before this function ever saw it. A hundred and twenty-five against a
hundred is inside twenty-five and outside ten, and the test asserts both — which
is **the clamp reaching the floor rather than stopping at the resolver**. That is
the first time a platform-shipped ceiling changes what happens to a pallet, and it
is the whole point of the chain from D81 through D87.

#### What is still missing, named rather than implied

The write path. Nothing calls this, nothing resolves a policy at a dock, and
nothing sets `accepted_at`. **J63 stays green because no line is dispositioned
outside the fixture**, and it will raise the moment one is — which is what it is
for, and why implementing it before the caller was the right order.

**Rejects.** Inventing tolerances or rounding. Refusing an over-delivery, which
D5 forbids. Refusing a shortfall. Writing an acceptance path here, which needs a
transaction boundary, an actor and a screen, and would be three decisions taken
inside a function that is meant to make one.

---

### D89 — The transaction that disposes of a receipt line

*Adopted 2026-08-09. Migration 43, and a library target for `crates/server`.*

Three pieces were built and none was connected: D82 and D83 resolve and clamp,
D88 decides, D87 has the column. This is the act that joins them, and until now
`goods_receipt_line.accepted_at` had been written by nothing but the fixture since
migration 21.

#### In SQL, for the reason D85 already settled

The caller resolves, because D22 puts the precedence order in code. The effect is
a database function it hands the answer to — the same shape as a projection
maintainer taking a resolved decision, and D24 requires the transaction to be one:
*"at receipt, in the same transaction as the movements."*

#### A disposition happens once

A line already accepted or rejected is refused a second disposition. Changing it
would silently rewrite what the first one meant, and **a change of mind is a
correction** — D51's shape, and not this function's job. That is D21's rule for a
resolved annotation and D87's note about freezing, arriving here first because
this is the first place the value is actually written.

#### `crates/server` gains a library target

The disposition rules D88 wrote **could not be tested against the database at
all**: a binary crate exposes nothing, and the suite that owns the fixture lives
in another crate. A rule nothing outside its own file can reach is a rule with one
reader, and D88's tests were all pure — none of them had ever seen a row.

#### What the end-to-end run establishes

On a real line, with the fixture's own policies: the tenant asked for twenty-five
percent over-delivery, **the platform ceiling brought it to ten before the dock
saw it**, and a hundred and twenty-five against a hundred was recorded as an
`over_receipt` discrepancy rather than refused. The line carries the version that
governed it, and a second disposition is refused.

That is the first time the chain from D81 through D88 does work on a row somebody
could ship — and the first time a platform-shipped ceiling changes what happens to
a pallet rather than what a test asserts.

**Rejects.** An async write path in the server crate, which would need a
transaction boundary, an actor and a screen — three decisions taken inside one
that is meant to make none of them. Allowing a second disposition. Raising the
discrepancy outside the transaction that wrote the disposition, which is how an
over-receipt becomes a log line nobody chases.

---

### D90 — A resolution freezes on first use

*Adopted 2026-08-09, settling question 152. Migration 44.*

D21 states the rule and D77 built the columns without it:

> *"A re-resolution **freezes on first use**: once an `assertion_check` or a
> `goods_receipt_line` references it, it may not be rewritten, and a correction
> writes a new assertion."*

**Both halves matter and they pull against each other**, which is why it needs
machinery rather than a note. Re-resolution must stay possible — D21 is explicit
that *"a GTIN unresolvable today becomes resolvable when the item is created
tomorrow, and refusing that would discard a claim because our catalogue was
behind"*. And it must stop the moment something has compared against it, because
after that a rewrite changes what the comparison concluded.

#### The pattern, on its fourth use

S7 forbids the trigger and D25 forbids the validation kind by name, so this is not
a constraint on the table. D89 established a day ago that it does not have to be:
**take the privilege away and mediate the write through a function that refuses.**

| | |
|---|---|
| D72 | `parent_id` — the app cannot UPDATE it; a move is an act |
| D77 | a claim — no UPDATE, no DELETE; a revision is a new assertion |
| D89 | a disposition — refused a second time; a change of mind is a correction |
| **D90** | a resolution — refused once compared against |

Four decisions reaching the same shape independently is worth naming as a pattern:
**where a cross-row rule cannot be a constraint, it becomes a grant plus a
function.** The application keeps INSERT — resolution at ingestion is a normal
write — and loses UPDATE on exactly the five columns that are our annotation
rather than the counterparty's words.

Measured on the fixture: the app cannot rewrite an annotation directly; the line
an `assertion_check` has compared against is refused with the count of checks that
froze it; and an unchecked line still re-resolves, which is the half D21 insists
on.

#### The half that cannot be expressed, named rather than implied

D21 names two things that freeze a resolution and **only one is checkable**:
nothing links a `goods_receipt_line` to the asserted content it was counted
against. D62 built the receiving path before the assertion tables existed and D77
built the assertion tables without going back, so **the two halves of a receipt —
what they said would arrive and what we counted — have never been joined by a
foreign key.**

That is a larger gap than this migration and it is question 157, not a column
invented here to make one sentence checkable. `despatch_advice` keeps its
`resolved_*` columns writable for the same reason: what would freeze them is a
receipt against the shipment, and that is the same missing link.

**Rejects.** A trigger. Recording every change to a `resolved_*` column so a
rewrite could be detected afterwards — detection after the fact is what D21 is
avoiding, and the value would already have been used. Freezing on *any* reference
rather than the two D21 names. Inventing the receipt-to-content link here.

### D91 — The two halves of a receipt, joined

*Adopted 2026-08-09, settling question 157. Migration 45.*

D90 could enforce half of D21's freeze and said so. This is the other half, and
the column it needs turns out to be the smaller part of what was missing.

**The gap was chronological rather than considered.** D62 built the receiving path
in migration 21, when the assertion tables did not exist. D77 built the assertion
tables in migration 34 without going back. So `goods_receipt_line` and
`asserted_unit_content` have described the same pallet for eleven migrations with
no relationship between them — and D43's own inbound mapping had already written
down that they are the same thing seen twice:

| 856 level | Ours |
|---|---|
| Item | `asserted_unit_content`, **becoming `goods_receipt_line`** |

`goods_receipt_line.asserted_unit_content_id` is **nullable**, which is the whole
of D21's rung zero: *"nothing on it is NOT NULL that requires an assertion, so
blind receipt is a schema property, not a workflow branch."* A truck with no
paperwork still gets received. S17 already examines five foreign keys into the
assertion set and this is the sixth, so the rule that keeps it optional was in
place before the column was.

#### The comparison D21 describes, and the half of it that cannot be done

D21 says the receipt compares the declared values **line by line** — that is the
stated reason `quantity`, `lot_code` and `expiry_date` sit on the claim rather
than being observations. Until this migration no query could perform that
comparison at all. `goods_receipt_variance` is it.

**Lot and expiry compare outright, and expiry is the one that earns the function.**
A supplier declaring a date the goods do not carry is not a quantity problem: the
counts agree, the lot codes agree, and every count check in the system passes. For
a distributor whose stock has a shelf life that is the failure most worth catching
and the one nothing else looks at.

**Quantity does not compare outright, and the first draft of this function got it
wrong.** It subtracted `goods_receipt_line.entered_quantity` from
`asserted_unit_content.quantity` — ten cartons from four hundred units. The two
columns are not in the same vocabulary and D58 already explains why they cannot
be: a packaging level *is not a unit*, because its factor varies by item and by
date, which is the entire reason `item_packing_config` exists and is versioned.
The fixture makes the error concrete at a factor of ten.

So the base comparison **reads the ledger instead of converting anything.** D45
defines what arrived as `SUM(stock_movement.quantity)` grouped by
`goods_receipt_line_id` — the ledger is in base units, which is D58's point
restated — and the two projections that fold
it both exclude internal moves the same way — a putaway names the same receipt
line and is not a second arrival. This is that predicate's third use rather than a
fourth statement of it. The entered pair is reported beside it in its own
vocabulary, labelled with the unit and the packaging level, and never subtracted.

The consequence is that `counted_base_quantity` is NULL for a line nothing has
landed for, and the variance with it. That is correct rather than unfortunate:
a rejected line has no arrival, and a number invented for it would be a claim
about goods that never entered the ledger.

Not a stored variance, for D23's reason and D73's: both sides are facts and the
difference is arithmetic over them, so a column holding it disagrees with its own
inputs the moment either side is corrected.

Measured on the fixture, which now holds both shapes. The short delivery — four
hundred declared, one hundred arrived, lot and expiry agreeing — reports −300. The
divergence — quantities equal, lot codes equal, June declared against September on
the goods — reports zero variance and two different dates, which is the line that
would otherwise pass silently. And a claim referenced by a receipt line and by no
check is refused re-resolution with `0 check(s) and 1 receipt line(s)`, which is
exactly the half D90 could not demonstrate.

#### What this raises

Fixing the subtraction did not answer the question underneath it. **The conversion
from an entered packaging level to base units lives nowhere in the database**, and
two places already assume it away: this function's first draft, and
`goods_receipt_line.expected_quantity` against `entered_quantity` — 130 base
against 10 cartons in the fixture — which `disposition()` compares directly to
decide over-receipt. The inputs are all present and versioned; what is absent is
one definition of the arithmetic. That is question 158.

**Rejects.** Converting cartons to base units inside this function, which would
put the case-pack arithmetic in a second place and is what 158 is for. Comparing
`asserted_unit_content.entered_quantity` against the line's entered quantity —
both are entered vocabularies and they are different ones, so that subtraction is
wrong in the same way with more steps. Making the column NOT NULL. Storing the
variance. Widening the freeze to `despatch_advice`, whose `resolved_*` columns
stay writable until something receives against a shipment.

### D92 — The receipt line counts in base units, like everything else

*Adopted 2026-08-09, settling question 158. Migration 46.*

158 was filed as *"where the conversion from an entered packaging level to base
units lives"*, and that framing is a symptom rather than the problem. **The
conversion already lives exactly where Principle 5 puts it** — in the writer, with
the canonical value stored and the entered form preserved beside it. There is no
SQL conversion function for dimensional units either, and that is not an omission.
What was missing is the column to store the result in.

| | entered form | canonical |
|---|---|---|
| `observation` | `entered_value` + `entered_unit_id` | `value_numeric` |
| `asserted_unit_content` | `entered_quantity` + `entered_unit_id` | `quantity` |
| `stock_movement` | `entered_quantity` + level + config | `quantity` |
| `goods_receipt_line` | `entered_quantity` + level + config | **nothing** |

One member of the family stored the vocabulary and not the meaning. Every symptom
follows from that single hole, and there were four of them.

**The absence had been noticed once and worked around.** D45, on rewriting J26:
*"The original folded `goods_receipt_line.quantity`, which does not exist and never
did — the invariant named the right relationship over the wrong table and could
not have run."* The invariant author reached for the column instinctively. The
invariant changed instead.

#### The column, and why it is not a duplicate

`goods_receipt_line.quantity`, bigint, base units, with **no vocabulary column
beside it** — Principle 5's structural trick borrowed verbatim from `observation`,
whose comment is the clearest statement of it in the schema: *"there is no unit
column, which makes non-canonical storage structurally unrepresentable rather than
merely discouraged."*

It does not duplicate `stock_movement.quantity`. The line says **what we counted**;
the ledger says **what entered the world**. They agree on an accepted line and
differ on every rejected one, and the schema could previously express only the
second — so a line refused for over-delivery could not state the over-delivery it
was refused for. D91's variance had to read the ledger for want of this column,
which is why it returned NULL on exactly the lines a variance report exists for.

#### The defect this was hiding, in shipped code

`disposition()` decides over-receipt with `entered_quantity > expected_quantity`.
`expected_quantity` is a snapshot of the promise and has always been base units.
`entered_quantity` is a count of cartons. **So the over-receipt tolerance has been
inoperative for every line not entered in eaches**, and not marginally: at the
fixture's case pack of ten, twelve cartons against a hundred units is a twenty-unit
over-delivery that reads as an eighty-eight-unit shortfall, and nothing is raised
until the delivery exceeds ten times the promise.

The end-to-end test asserted this behaviour and passed. It entered 125 against an
expectation of 100 and called it twenty-five percent over; the row it actually
wrote was a 1250-unit delivery against a promise of 100. **A test can encode the
misreading as firmly as the code does**, and both did.

#### The check that could have caught all of it

Principle 5 has the writer convert and the database store the result, and J57
checks that a row names the *right* config — correct item, already effective.
**Nothing has ever checked that the stored number is what that factor produces.**
A miscalibrated client writes a wrong quantity with a perfectly valid config beside
it and all 116 rules pass.

J65 closes that, across all three tables, with two factor sources: `packing_factor`
for the packaging arm and `unit.factor_num/factor_den` for the unit arm. It is not
a second source of truth — the writer still converts. It is independent
re-derivation, which is what the whole suite is: J1 re-folds the ledger to check
`stock.quantity` rather than trusting the projection that wrote it.

It found something immediately. The fixture's declared claim asserted **400 base
units against 40 entered in `ea`**, a unit whose factor is 1/1 and the only member
of the `count` dimension. That pair is impossible, and it had been in the fixture
since the assertion tables existed. Either the row was wrong or `entered_unit_id`
was being used to mean cartons — which it structurally cannot, because a packaging
level is not a unit. That second reading is question 159.

`packing_factor` exists **for the check rather than for the write path**, and
returns NULL where the cascade cannot answer: a config with no `cartons_per_layer`
cannot convert a layer, and a made-up 1 there is a wrong answer rather than a
missing one. J65 reports that case as `factor_unavailable` rather than skipping it,
because a row naming a config that cannot convert the level it also names is a row
whose quantity nobody can ever verify.

**J57 is widened rather than duplicated.** Nothing in it was ever about movements
specifically; the receipt line carries the identical triple and was uncovered only
because D58 put the columns on the ledger first. It examines four rows now instead
of two.

#### What storing the canonical settles for free

158's trigger noted that the factor is versioned by `effective_from`, so the
conversion is as-of the receipt rather than as-of now, and worried that this needed
machinery. It does not. **Storing the canonical at capture is what makes a
versioned factor safe** — correct a case pack next year and no historical quantity
moves. That is D58's own argument for recording which config converted a movement,
applied one table further than D58 applied it.

**Rejects.** A SQL conversion function on the write path, which is against
Principle 5 and would make history rewritable by re-deriving from a factor that
moves. Making `packaging_level` a unit — D58 settled that, and its factor varying
by item and date is precisely why it is not a dimension member. Backfilling a
counted quantity for rejected lines from anything: there is no ledger row and an
invented number would be a claim about goods that never arrived. A second J-number
for the receipt-line half of J57. Deriving the variance's counted quantity in the
query instead of storing it, which is where D91 already went wrong once.

### D93 — A claim can say cartons, and we keep the word they used

*Adopted 2026-08-09, settling question 159. Migration 47.*

159 asked whether a claim can state a carton count. Researching it turned up a
better question and a smaller defect.

#### The standards keep the split this schema already keeps

**UN/ECE Recommendation 20 codes units of measurement. Recommendation 21 codes
types of cargo, packages and packaging materials.** Two lists, and Rec 21 says its
codes *"may be used in combination with a data element specifying unit of
measurement"* — complementary rather than alternative. `unit` and
`packaging_level` are that division, and D58 arrived at it independently by
reasoning about case packs. That is worth writing down, because the next reviewer
will wonder why we did not simply add `carton` to the unit table.

Both message families put a code from those lists beside the quantity. X12 856's
SN1 pairs SN102 with SN103 — `CA` for case, `EA` for each. EDIFACT DESADV's QTY is
`qualifier:quantity:measureUnitCode` — `PCE`, `CT`, `PK`, defaulting to `PCE`. A
supplier saying "40 CT" is ordinary traffic.

**In Australian grocery the level more often rides on the GTIN than on the code.**
GS1's trade item hierarchy gives every packaging level its own GTIN tagged with
`tradeItemUnitDescriptor` — BASE_UNIT_OR_EACH, PACK_OR_INNER_PACK, CASE, PALLET,
which is nearly this schema's enum, standardised. Coles mandates GTIN-14 in ITF-14
on every master carton; Woolworths requires pricing against the CASE GTIN when a
product is ordered by carton. Both routes exist and a receiver meets both.

#### The defect underneath, which is smaller and worse than the question

D58's correction — *"The list called the middle column `entered_unit`, and **it is
not a unit**"* — was applied to `stock_movement` in migration 18 and
`goods_receipt_line` in migration 21. `asserted_unit_content` did not exist yet.
Migration 34 built it from D45's **pre-D58** sketch, and nobody swept back.

That left a **Rule 5 violation in the one table category that exists to prevent
it.** D21: *"Recorded in the author's vocabulary. Resolution into ours is a
separate, fallible, recorded step."* Every other counterparty value on the row has
a raw twin — `raw_gtin`, `raw_item_code`, `raw_po_reference`, `raw_po_line_number`
— and the sibling table keeps `level_code` raw as received. The unit code was the
only counterparty code in the assertion tables converted on write with the
original thrown away.

So the answer to "can a claim say cartons" is that it could not, and worse, it
could not say what it *did* say either.

#### One raw code, two readings

`raw_unit_code` holds the author's word verbatim. `resolved_unit_id` — renamed
from `entered_unit_id`, because it was never an entered value — is our reading
when that word names a dimension. `resolved_packaging_level` with
`item_packing_config_id` is our reading when it names a package type. Exactly one
of the two, by `num_nonnulls(...) <= 1`, which is S3's convention on its third
outing.

The rename matters more than it looks. **Calling a resolution `entered_` is what
made 400 base units against "40 ea" read as a legible row** for as long as the
assertion tables existed. A column named for what it is would have invited the
question earlier.

The config arm brings D58's argument to its third table: a corrected case pack
cannot rewrite what a claim meant, because the claim names the version that sized
it. **Both new checks cover it with no new code** — J65 gained a second arm on the
claim and J57 a third table — which is the strongest evidence available that the
shape is right rather than merely convenient.

**J57's clock differs per table and the difference carries meaning.** A movement
has its own `occurred_at`. A receipt line takes its receipt's `received_at`. A
claim line takes its assertion's `asserted_at` — *their* clock — because the case
pack that applied is the one in force when they packed, not when we read it.
`received_at` stands in when they stated no time, which is D5's both-clocks rule
doing exactly what it was written for.

#### What is deliberately not here

**`item_barcode` stays unbuilt.** It is the GS1-correct resolution path — a case
GTIN resolving to item, level and factor is the brand owner's own declaration
rather than our mapping of a two-letter code — and D24 designed it with
`unit_level` and *"base units per scan of this barcode"* plus D19's tenant
override for a differing case pack. J42 and J43 already wait on it. But it is a
shared-catalogue reference table that blocks all scanning work, and it **feeds**
these columns rather than replacing them. When it exists, the question is whether
GTIN resolution supersedes a hand-set level, which is narrower than 159 was.

**No code-list column beside `raw_unit_code`.** X12 and Rec 21 overlap in spelling
without agreeing in meaning, so which list a code came from is load-bearing. It is
knowable from `party_message`, whose artefact records its own standard, and a
second copy here is a second place for it to disagree. Question 160.

**Rejects.** Adding package types to `unit` — Rec 21 existing separately is the
standards body making D58's argument. Requiring the structural form, a child
`asserted_unit` per carton: GS1 permits it, Australian suppliers state carton
counts over pallets whose cartons are not SSCC-marked, and D24 caps the nesting at
receipt regardless. Widening `asserted_unit_content_resolve` to cover the new
resolution columns, which would have smuggled a fix for question 161's defect
into a rename.

### D94 — The mediated write, actually mediated

*Adopted 2026-08-09, settling question 161. Migration 48.*

D90 named a pattern on its fourth use: *"where a cross-row rule cannot be a
constraint, it becomes a grant plus a function."* Take UPDATE away from the
application, hand it a function that refuses when the rule says refuse.

**Both halves were stated and neither was built, in opposite directions.**

| | stated | built | effect |
|---|---|---|---|
| D90 | the app loses UPDATE, the function performs it | the function is not a definer | mediation **impossible** |
| D89 | the function refuses a second disposition | the app kept UPDATE on the columns | mediation **optional** |

`asserted_unit_content_resolve` ran with the caller's rights, and the caller is
the role whose rights had just been removed. As `nylonite_app` it failed on its
own first statement — `SELECT 1 ... FOR UPDATE`, which needs UPDATE privilege —
before reaching the freeze it exists to enforce. D90's measured refusals were
genuine, because the refusal path raises before touching a row. **The success
path, the half D21 insists on, had never once been executed by the role that
would execute it.**

`goods_receipt_line_dispose` had the mirror failure. Migration 21 granted the app
UPDATE on the disposition columns, so D89's *"a change of mind is a correction,
not a second disposition"* bound only callers who chose to be bound. Measured
before this migration: the direct UPDATE succeeded.

D72 and D77 are unaffected — `item_class` grants the app `code` and `name` only,
and D77 removed UPDATE on the claim entirely. So the pattern's two *function*
instances were both wrong and its two *grant-only* instances were both right,
which is a fair summary of where the attention went.

#### Why the obvious fix would have been a tenancy hole

`SECURITY DEFINER` alone is the dangerous answer. Owned by `postgres` the body
runs as a superuser, and **a superuser bypasses row-level security
unconditionally** — so every mediated write becomes a tenancy escape, reached
through a function the application is handed on purpose. That is D55's write hole
reopened through a door built for safety.

The safe shape was already in the schema before anything checked it: fifteen
`SECURITY DEFINER` projection functions owned by `nylonite_projection_owner`,
which has neither SUPERUSER nor BYPASSRLS, against tables with FORCE ROW LEVEL
SECURITY so RLS applies to the owner too. `nylonite_mediation_owner` is that
shape for a different concern, and separate from the projection owner because the
grants should be: a projection owner rewrites whole projection columns, a
mediation owner writes exactly the columns its functions govern.

Measured as `nylonite_app` after the change: the unfrozen claim re-resolves; the
frozen one is refused *with the freeze message rather than a privilege error*; the
direct UPDATE on either table is denied; and **another tenant asking to resolve
this tenant's claim is told the row does not exist**, which is RLS still holding
through the definer.

#### The register catches its own additions

`mediated_write` is `projection_rebuild`'s shape for `projection_rebuild`'s
reason. Without a declared side, S54 would have to guess which columns a function
is responsible for, and guessing is how both halves went unbuilt. Four properties
per column, because there are four ways to get it wrong and two of them were live.

**J37 caught the change on its first run**, which is worth recording as the suite
working rather than as a stumble: Postgres grants EXECUTE to PUBLIC by default,
harmless on an invoker function and a privilege escalation on a definer, and J37
has been asserting exactly that since it was written. The revoke is in the
migration because the check said so.

#### The harness that existed and was switched off

`crates/server/tests/tenancy.rs` holds three tests of the application role and
**every one of them skipped on every run**, printing `DATABASE_URL_APP unset` and
reporting success — because every role here is NOLOGIN and that variable names a
connection nobody had made. A suite whose entire premise is that a vacuous pass is
not a pass had three tests passing vacuously for as long as they had existed.

They now fall back to `DATABASE_URL` and `SET ROLE nylonite_app`, which is
faithful for everything they assert: RLS and column grants are decided by the
current role rather than the session role. All three pass. `mediated_write.rs`
is new and tests the behaviour S53 and S54 test structurally, because the
structure was right in the register and wrong in the database for four
migrations.

**Rejects.** `SECURITY DEFINER` owned by `postgres`. Reusing
`nylonite_projection_owner`, which would hand a resolution function every
projection privilege. Granting the app UPDATE on the five resolution columns and
calling the function advisory, which is D89's failure adopted deliberately.
Dropping the role in the down migration: a role is cluster-wide and another
database may hold objects owned by it. A `BEFORE UPDATE` trigger, which S7 and D25
forbid and which this pattern exists to avoid.

### D95 — A projection says how stale it is allowed to be

*Adopted 2026-08-09, settling question 163. Migration 49.*

The inbound walk raised this by reaching the end of a delivery and being unable to
fold it. Migration 24 decided the **order** the maintainers run in and put it in
`projection_step`. Nothing had ever decided their **cadence**.

`projection_run_all` is granted to the scheduler, the platform and the projection
owner, and deliberately not to the app — D25 keeps the writer away from the
maintainer — and S7 examines zero triggers. So nothing folds a write until
something external runs, and a receiver finishes a delivery while `stock.quantity`
does not move.

**That is not a defect and this decision does not fix it.** It is the design
working: the ledger is the truth and `stock` is a cache. What was missing is that
nobody could tell how old the cache was, and nothing said how old it was allowed
to get.

#### Why synchronous folding is not available, measured rather than argued

Every maintainer is a **full-tenant fold**. `projection_stock_rebuild` reads
`stock_movement WHERE tenant_id = p_tenant` in its entirety and recomputes every
cell, so its cost is O(history) rather than O(the write that prompted it), and
it grows forever.

| population | full `projection_run_all` |
|---|---|
| 289,080 movements, one year | 1954 / 2021 / 2073 ms |
| the small fixture tenant | 27 ms |

Folding on the write path would therefore put two seconds and rising on every
scan. The sharper consequence is that **the cadence has a ceiling which is a
property of the fold rather than of the scheduler**: N tenants cost N × 2 s per
cycle, so "run it every minute" stops being available at a tenant count nobody has
computed. Making the fold O(delta) is a different decision and question 164.

The README claimed this fold took *"under a second"*. It takes two. That line was
true when written and stopped being true across nine migrations of new
projections, which is the drift this project keeps finding — this time in the file
that warns about it.

#### What was added

`projection_step.freshness_bound` is NOT NULL with **no default**, so a step added
later cannot avoid the question. The bounds differ by what reads them, which is the
only defensible way to set them: five minutes for the stock and package families,
which is what a picker looks at while standing in front of the goods; fifteen for
planning and the receiving screen; an hour for taxonomy closures, which change when
somebody reorganises a catalogue.

`projection_freshness` records when each maintainer last ran for each tenant, and
`projection_age()` is what a screen asks so it can say *"as at 14:03"* rather than
presenting a cached number as though it were the ledger. NULL means the fold has
never run for that tenant, which is a different answer from a large interval and
reads as one.

**The stamp is written on every run, including a run that found no work**, and
that is the entire point: a fold that ran and found nothing is *fresh*, a fold that
has not run is *stale*, and the two are indistinguishable if the stamp only moves
on work. `projection_step.last_changed_at` already answers the other question —
when the projection last *changed* — which D70 uses for the policy epoch, so the
two columns are deliberately different facts.

#### The tension with D68, and why the carve-out is narrow

D68 asserts that **a rebuild that changes nothing writes nothing**, measured after
one run rewrote 39,565 unchanged rows, 35,040 of them `expected_supply`. Its test
counts `n_tup_upd + n_tup_ins` across every user table, so a stamp written on every
run breaks it by construction.

`projection_freshness` is excluded by name, in the test, with the reason beside it.
The exclusion is defensible on the thing D68 is actually about: it measured a table
churning **in proportion to its own size**, and this one writes one row per step
per tenant, bounded by the register rather than by the data. It is also the
orchestrator recording itself rather than a maintainer rewriting a fold. S17
carries the same shape of scope for the same reason and says so in the same place.

J66 is what notices staleness, and it has two arms because the second one hides: a
projection that ran too long ago is visibly stale, while one that has never run for
a tenant has **no row at all**, and an anti-join written the obvious way passes
over it in silence. A finding rather than a block, per D8 — a stale cache never
stops the floor.

**J66 found something on its first run against a fresh database, through the arm
that hides.** The fixture ran the maintainers for tenant alpha eleven times and for
beta **not once**. It was invisible while nothing recorded that a fold had
happened: beta's projections were empty because beta has little to project, so
*correctly empty* and *never computed* looked identical. The fixture now folds
both, because a second tenant that exists to prove tenant isolation should be
maintained the way the first one is, or it proves isolation of a state no
deployment would ever be in.

**S49 also caught this migration.** It reads every `projection_%` function and
asserts each is a declared step, and `projection_age` matched the pattern while
being a reader. The scope is now `provolatile = 'v'` rather than a name exception,
because Postgres enforces the distinction the check is actually about: a STABLE
function cannot write, so it cannot be a maintainer whatever it is called, and the
next reader in this family is covered without anyone remembering to add it.

#### A fixture that had been broken for three migrations

`history.sql` would not load: D92 added `goods_receipt_line.quantity` with a CHECK
pairing it to `entered_quantity`, and the generated year still wrote only the
entered form. Nothing noticed because **`verify-migrations.sh` loads `seed.sql`
only** — the large fixture is reached exclusively by `scripts/measure.sh`, which no
test invokes. So the schema can break the fixture that exists to measure it, and
the failure waits for somebody to run a script by hand. Fixed here because D95
needed the year to measure against; that it stayed broken for three migrations is
worth more attention than the fix.

**It got that attention immediately afterwards.** `history.sql` now takes its
length as a parameter and `verify-migrations.sh` loads a single day of it as a
fifth phase, which exercises every INSERT in the file against the live schema for
0.15s rather than the two minutes a year costs. `measure.sh` still builds the
year, because a smaller population would answer question 142 about a database
nobody runs. One file, two jobs, no second copy to drift.

**Rejects.** Making the bound a `policy_kind` under D22 — a projection's freshness
is a platform engineering property, not a tenant's business rule, and D22 is for
rules that vary by scope. A run *log* rather than a current-state row: every answer
but the latest is noise, and an unbounded table to hold it is the churn D68 exists
to prevent. Stamping only on work, which makes "ran and found nothing" look
identical to "never ran". Granting the app `projection_run_all`, which reverses
D25's separation and would put a two-second fold on the write path. Blocking a read
on a stale projection, which is D8's non-blocking rule inverted.

### D96 — The pallet they declared meets the pallet we scanned

*Adopted 2026-08-09, settling question 162. Migration 50.*

**This is 157 one level up.** D91 joined the two halves of a receipt at the content
level — what they said was in the carton against what we counted — and the unit
level stayed open. A despatch advice declaring three SSCCs and three pallets
arriving on the dock were two sets of rows with nothing between them. D24's own
sketch gave `asserted_unit` a `package_id`, *"nullable; set at receipt"*, and
migration 34 built the table without it.

S35 had been pending on this for eleven migrations, and its blocker read *"the
collapse at receipt does not exist yet"* — true of the code, and it hid that the
column the sentence needed was missing too. **A blocker naming the larger absence
concealed the smaller one inside it.**

#### Their level vocabulary is theirs, so physicality is our reading

`level_code` is raw and stays raw: an 856 says `S O T P I`, EDIFACT says something
else, and D43 stores the order level as a node. So *non-physical* is not derivable
from the code, and S35's sentence — a non-physical node carries no SSCC and
contributes no package — had nothing to be true of.

`resolved_physical` is that reading, in the same raw-to-resolved shape the table
already carried for `raw_package_type_code → resolved_package_type_id` and that
D93 formalised on the content line. With it, **both halves of S35 become row-level
CHECKs**, which is stronger than the register asked for: the rows cannot be written
rather than being found afterwards. S35 moves from pending to passing, and the
structural suite's pending count drops for the first time in this run of decisions.

#### The collapse is a mediated write, and it is the first one built on a working pattern

The application has no UPDATE on `asserted_unit` and must not get one — a claim is
immutable (D77) and our annotation on it freezes on first use (D21). So the link is
set through `asserted_unit_collapse`, a definer owned by `nylonite_mediation_owner`
with UPDATE on exactly the three columns it writes, registered in `mediated_write`
so S54 checks the arrangement rather than trusting it.

**D94 built that machinery two decisions ago and this is its first use.** That the
next mediated write needed no new mechanism, only three registry rows, is the
evidence D94 was the right shape.

Four refusals, each a rule with nowhere else to live: a second collapse (D21's
freeze); a document node (S35, said in words a receiver can read rather than as a
constraint violation); a package with no observed event, because collapsing onto
one minted from the claim would make the claim its own evidence, which is J34's
rule at the moment a receiver could still act on it; and an SSCC that disagrees,
because a pallet scanned under one licence plate is not the pallet declared under
another.

J67 carries the last of those forward. `package.sscc` is a fold of `package_event`,
so a relabel recorded afterwards moves one side of a comparison already made —
exactly J57's shape, where a config can be edited after a movement named it.

#### A guard of mine that said nothing for a whole decision

The inbound walk was written with an assertion designed to fail the moment this
column appeared, so the walk would have to be extended through it. It did not fail.
It watched for a column named `package_id`, because that is what D24's sketch
called it, and D96 built `resolved_package_id` to match the convention the table
already had.

**A check that names the wrong object cannot fail.** That is S42's entire subject,
arriving in a test that S42 does not cover, written by the person who had just
finished writing about it. The guard now asks by shape rather than by name — is
there a foreign key from the declared tree to the physical one — which a rename
cannot silence and a third spelling cannot evade.

**Rejects.** Naming the column `package_id` after the sketch, when every other
annotation on these tables is `resolved_*`. Deriving physicality from the raw
`level_code` with a list of known document codes, which is a second vocabulary to
maintain per standard. Letting the app UPDATE the link directly. Minting the
package from the claim, which J34 forbids and which would make the despatch advice
evidence for itself. A CHECK for the SSCC agreement, which cannot span rows and
would in any case be checking a projection that moves.

### D97 — An event that asserts a placement says where

*Adopted 2026-08-09. Migration 51. Found by the outbound walk on its first run.*

Two constraints disagreed and the maintainer is where they met.

| | |
|---|---|
| `package_event` | `num_nonnulls(parent_package_id, location_id) <= 1` |
| `package_containment` | `num_nonnulls(parent_package_id, location_id) = 1` |

`asserts_placement` is generated as `kind IN ('created','placed','contained')`, and
of those three only `placed` and `contained` carry a CHECK requiring a holder.
**`created` requires neither** — so a shipping carton brought into existence at a
packing bench, which is the ordinary first act of packing an order, is a legal
event the containment fold cannot represent.

**The consequence is worse than a bad row.**
`projection_package_containment_rebuild` *raises*, so `projection_run_all` aborts
at ordinal 60 and the transaction rolls back. Every projection after it never runs
— `stock.resolved_location_id`, the order fold, the fulfilment fold, the inbound
shipment fold, expected supply — and the ones before it are rolled back with it.
One carton created without a holder stops every projection for that tenant until
somebody finds and deletes the row.

That is D8 inverted. The rule is that a disagreement becomes a finding and never
stops the floor; here a disagreement stops the floor and produces no finding at
all.

#### Derived rather than listed

The fix is not a third kind-specific CHECK. The property is not about `created`: it
is that **anything asserting a placement has to say what the thing is placed in**,
and `asserts_placement` is already the column that decides which kinds do. Deriving
the constraint from it means the next placement-asserting kind is covered on the
day it is added, by whoever adds it, without their noticing — which is the only
kind of coverage that survives this project'"'"'s own history.

`package_event_placed_ck` and `package_event_contained_ck` stay. They pin *which*
holder each kind names, which is a different sentence from *whether* it names one.

The migration reports offending rows before adding the constraint rather than
failing with a constraint violation, because in a deployment where this has been
happening the rows are the diagnosis: they are exactly the packages whose tenant
has had no working projection.

**Rejects.** Making `created` not assert a placement, which trades a crash for a
carton that holds stock at no location and a J7 finding — quieter, and it leaves
the goods genuinely unlocatable. A defensive maintainer that skips malformed rows,
which hides the disagreement rather than settling it; the general question of a
maintainer that raises taking every other projection down with it is 166. A CHECK
naming the three kinds, which is the same rule written where the next kind will not
look.

### D98 — One maintainer that raises costs one projection

*Adopted 2026-08-10, settling question 166. Migration 52.*

D97 closed the one reachable way to make a maintainer raise. **The blast radius
belonged somewhere else**, and 166 was the question of where.

`projection_run_all` walked `projection_step` in one transaction, so a maintainer
that raised aborted the run *and rolled back the steps that had already succeeded*.
The tenant was left with no fold at all rather than a partial one, and nothing
recorded that it happened. That is D8 inverted, and D8 is the rule this system is
built on: a disagreement becomes a finding and never stops the floor.

#### Three changes, and the third is a refusal

A PL/pgSQL block with an EXCEPTION clause is an implicit savepoint, so a step that
raises now undoes its own work and nothing else. **What already succeeded is kept.**

The failure is recorded twice, because two different readers need it.
`projection_freshness.last_error` says why, beside the `last_run_at` that D95 made
the answer to *how old is this number* — and that column deliberately **does not
advance on a failure**, so a projection that keeps raising goes stale and J66
reports it without needing to know about D98 at all. A `projection_failed`
discrepancy puts the same fact in the queue a human reads. Not `projection_drift`,
which is a projection holding a *wrong* value: this is one holding *no* value, and
the two want different responses — drift is fixed by rebuilding, this by repairing
whatever the maintainer choked on.

**The sequence still stops, and that is deliberate.** The obvious reading of 166
was that one failure should cost exactly one projection, and it cannot: the ordinal
is a dependency order and `projection_step` says so in its own notes — *"Needs
cells from 30 and placements from 40"*. Running the rest would fold over inputs
that were never written, turning one broken projection into several wrong ones,
which is worse than the thing being fixed. What changes is that the steps which
never ran now go stale visibly instead of silently holding yesterday's values.

Measured on a step registered to raise on purpose: the orchestrator returns rather
than propagating, the closure fold before it keeps its fresh stamp, the stock fold
after it does not run, the freshness row carries the error with no run time, and
the discrepancy is in the queue.

#### D95 caught this migration's own test

`projection_step.freshness_bound` is NOT NULL with no default, *"so a step added
later cannot avoid the question"*. The test that registers a deliberately failing
step is a step added later, and it was refused until it stated a bound. A
constraint written three decisions ago catching the person who wrote it is the
cheapest possible confirmation that it was worth writing.

**Rejects.** Continuing past a failed step, for the dependency reason above.
Declaring per-step dependencies so independent steps could still run, which is a
larger change and would be the right answer to a question nobody has asked yet.
Reusing `projection_drift`. Retrying the failed step, which repeats the raise and
delays the finding. Leaving `last_run_at` advanced on failure, which would make a
broken projection look fresh — the exact inversion this decision exists to remove.

### D99 — A pick says which line it served

*Adopted 2026-08-10, settling the first half of question 165. Migration 53.
Narrows 165 to its second half. Raises 167.*

The outbound walk asserts the gap rather than describing it: ten units picked into
the carton, the ledger says ten, `fulfilment_line.picked_quantity` says zero.

**The first half of 165 was never actually open.** The Correction to D10 already
states the corrected cause set, and `fulfilment_line_id` is the first line of it.
Migration 21 built the inbound arm because D45 needed it, wrote a one-argument
CHECK, and left the rest. Two further passages assume the column exists: D53 names
its absence as the *reason* progress folds the allocation — *"there is no path from
a movement to the commitment it served"*, a constraint reported rather than a source
preferred — and `mechanism-design.md` retires `package_content` on the sentence
*"the demand cause lives on the movement that put the stock there, which is where
D10 says causes live"*, which is not yet true of the outbound path.

So this adopts nothing new. It builds what D10 as corrected already specified.

The `discrepancy_id` arm is not added and is not missing: migration 6 resolved that
cause in the other direction, with `discrepancy.stock_movement_id` and
`resolving_movement_id`. **The cause set on this table is two arms, not three.**

#### The cause CHECK has been vacuous since it was written

`CHECK (num_nonnulls(goods_receipt_line_id) <= 1)` takes one argument, returns 0 or
1, and is therefore always true. It has never excluded a row.

S3 reads it — `%_cause_ck`, tested for the substring `<= 1` — and passes, because S3
checks the *form* of the rule and not whether the rule has anything to say. That is
D45's own sentence about S3's first outing arriving one table later: *"a vacuous
check cannot be wrong out loud."* Adding the second arm is what makes it a rule.
Nothing about S3 changes. **167 carries the general version**: a mutual-exclusion
CHECK with fewer than two arms is vacuous by construction, and nothing detects it.

#### The discriminator, which is the part that took a decision

An outbound cause arm makes `SUM(quantity) GROUP BY fulfilment_line_id` expressible,
and immediately wrong: a pick movement and a despatch movement both name the same
line, so the ungrouped sum counts ten units as twenty. **This is D45's putaway
double-count arriving on the outbound side**, and it takes the same answer — D45
separates an arrival from a later putaway *by shape*, never by `reason`:

```sql
AND m.from_location_id IS NULL AND m.from_package_id IS NULL   -- goods entering
```

The rule adopted here is the mirror, and it is shape-only:

| Quantity | Movements naming the line, where |
|---|---|
| `picked_quantity` | `from_location_id IS NOT NULL` — units left a storage location |
| `despatched_quantity` | `to_location_id IS NULL AND to_package_id IS NULL` — units left the building |

`packed_quantity` is not a movement at all, and `covered_quantity` does not move off
`stock_allocation`; both are below.

**Why the from side and not the to side.** The obvious reading is that a pick is a
movement *into* an order's carton — `to_package_id` naming a package whose
`fulfilment_id` matches the line. It fails twice. Batch picking puts units for
several orders into one tote, which has no single `fulfilment_id`, so a real pick
would not count. And a consolidation from that tote into the carton names the same
line and *does* match, so it counts a second time. The to-side reading undercounts
the picking model most warehouses actually run and double-counts the step after it.

The from side has neither problem, because **re-handling after a pick is
holder-to-holder**. Working the cases:

| | | |
|---|---|---|
| bin → carton | `from_location_id` set | picked ✓ |
| bin → tote (batch pick) | `from_location_id` set | picked ✓ |
| tote → carton (consolidation) | `from_package_id` only | not counted again ✓ |
| carton → carton (repack) | `from_package_id` only | not counted again ✓ |
| bin → staging location | `from_location_id` set | picked ✓ |
| carton → out | `to` both null | despatched ✓ |
| bin → out (pallet straight to the truck) | both | picked **and** despatched ✓ |

The last one is not a double-count. D53's sets are nested — `despatched ⊆ picked`,
because despatched units were picked — so a movement that does both belongs in both.

A replenishment into a pick face is excluded by the cause arm rather than by the
shape rule: an internal move has no demand-side cause, which is the Correction to
D10's entire subject and the reason the CHECK is `<= 1`.

#### The limit, named rather than discovered

`stock.holder_location_id` and `holder_package_id` are exclusive by CHECK, so **a
cell can be package-held**: a pallet in a rack is storage. Picking from it is
`from_package_id`, and the rule above does not count it.

The tempting patch — count a from-package whose `fulfilment_id` is null — restores
the consolidation double-count from the other side, because a batch tote is not any
one line's carton either. The complete rule is recursive: a holder is in a line's
service once stock has entered it under that line's cause, which is the same shape
`mechanism-design.md` already uses for a sealed carton's manifest — *"the movements
that put stock into it up to `sealed_at`"*. That is a real design and it is not
built here.

So the limit is stated, and the fold migration owes it a finding rather than a
silently low number. This is D8's shape: the gap is detectable, not papered over.

**A second obligation arrives with the arm.** A movement may now name a fulfilment
line while carrying a different `item_id` from it, and a fold would credit the
commitment with the wrong goods. It cannot be a CHECK — it spans two rows — so it is
an invariant raising a finding, which is also the mechanism that makes D12's
substituted pick *visible*: the picker who finds lot B where the plan said lot A is
D12's founding case, and an item that disagrees with its line is the same event one
category up. The fold migration owes this check; nothing reads the column until then.

#### `packed` is not in the ledger, and that is the sharper half

**Sealing a carton moves no stock.** There is no `stock_movement` for it, so no cause
arm on this table can ever produce `packed_quantity` by grouping — which means the
question as 165 poses it, ledger versus state machine, has no answer that covers all
three fractions.

The derivation exists and is already written down: the movements into a carton,
bounded by its `sealed` event. It is a fold of two fact tables rather than one, and
it is still evidence rather than intention.

#### What stays, and what stops being guaranteed

`covered_quantity` keeps folding `stock_allocation`. Coverage asks how much of this
commitment is *spoken for*, which is a question about intentions, and D12 makes an
allocation exactly that. **D53's insight repeats one level in**: the cell and the
commitment asked different questions of one word, and here coverage and progress are
different questions of which only one is about what physically happened.

One consequence has to be stated rather than found. Under the allocation fold,
`picked ⊆ covered` is *structural* — four nested FILTER clauses over one state set
cannot violate it. Folding three of the four from the ledger gives that up: a pick
with no allocation, or an over-pick, produces `picked > covered`, and no constraint
prevents it.

That is not a defect. D12 permits both explicitly — an allocation *"is allowed to be
wrong"* — and the allocation fold does not make them safe, it makes them **invisible**,
discarding a real pick because no intention row existed to carry it. The guarantee
becomes a finding, which is the trade this system makes everywhere else. But
whoever builds the picking screen will expect the four numbers to nest, so it is
said here.

**Rejects.** Folding `picked` from `reason = 'pick'`: `stock_movement.reason` is
`text NOT NULL` with **no CHECK at all**, which is a worse instance of what 143
raises about `stock_allocation.state` — that one at least has a CHECK listing seven
values. The to-side discriminator, for the two failures above. Moving the fold in
the same migration, which would put a change needing no discriminator behind a
question that does. A `discrepancy_id` arm, resolved in the other direction by
migration 6. `= 1` on the cause CHECK, which is D16-repeats-D10 and what S3 exists to
catch. Inventing a movement for the act of sealing a carton, so `packed` could fold
like the others — a movement that moves nothing is what
`stock_movement_distinct_sides_ck` refuses. Any UPDATE or DELETE grant on this table,
which would make the cause rewritable and void the whole argument for preferring it.

### D100 — Outbound progress folds the facts that produced it

*Adopted 2026-08-10, settling question 165. Migration 54. Raises 168.*

D99 gave the ledger its outbound cause and settled the discriminator. This moves the
fold, and the three things it needed are the three D99 could not supply: a source for
`packed_quantity`, a rule for what a correction does to progress, and J31 split so
one source stops answering two kinds of question.

| Column | Source | Rule |
|---|---|---|
| `covered_quantity` | `stock_allocation` | unchanged — the covering state set |
| `picked_quantity` | `stock_movement` | the movement left a storage location |
| `packed_quantity` | `stock_movement` + `package.status` | into a carton whose winning status is `sealed` or `despatched` |
| `despatched_quantity` | `stock_movement` | no `to` side at all |

#### `packed` reads the carton's status, and `sealed_at` would have undone the decision

D99 named the derivation — the movements into a carton, bounded by its seal — and
left the column to read. **`package.sealed_at` is the wrong one.** Migration 9 grants
the application both INSERT and UPDATE on it, so it is a timestamp the app can
rewrite. Folding progress from it would reintroduce the exact defect that made this
question worth asking, one column over from the state machine being abandoned: a
physical claim resting on a mutable value.

`package.status` is a **projection of `package_event`** — the winning row of
`('sealed','opened','despatched','voided')` — maintained at ordinal 40, and this fold
runs at 90.

**The status also makes the time bound unnecessary**, which is the part worth
recording because it is not obvious. The bound existed to stop a repack counting
twice: stock into carton A, A sealed, A opened, stock moved to carton B, B sealed —
both movements are movements into a sealed carton. But opening A moves its winning
status to `opened`, so A stops contributing on its own and only B counts. The
carton's current status already carries the history the bound was reaching for.

| | |
|---|---|
| into a tote, then tote into a sealed carton | packed once, at the carton |
| into a carton later opened and not resealed | packed zero — it is not packed |
| into a carton sealed and then despatched | packed, and stays packed |

The last is D53's nesting: despatched units are still packed. `despatched` is a
*subset* of `packed`, not its successor.

#### A movement recorded in error did not happen, and the pair leaves together

D47 splits a movement by whether the world changed or the record was wrong, and
migration 10 states the rule as *"reason class: `record_error` exactly when
`reverses_movement_id` is set"*. A reversal is not an adjustment to a real event; it
is the statement that the event was never real. So the erroneous movement leaves the
fold.

**Missing the second half is the subtle part.** The reversal must leave too, and not
for symmetry — for shape. Reversing a despatch means running it backwards, and a
despatch is `carton → nothing`, so its reversal is `nothing → carton`: a
`to_package_id`, no `from` side, and therefore a movement *into* a sealed carton. A
fold that dropped only the original would read the correction as ten more units
packed. **A correction would inflate the number it was recorded to fix.**

#### The same gap exists inbound, and this decision does not reach into it

D45's `quantity_received` counts movements with no `from` side and excludes neither a
reversed arrival nor its reversal, so a receipt recorded in error stays received.
Nothing has hit it because the single reversal in the fixture names no receipt line.

It is the same defect and it is not fixed here. Widening a decision by editing its
fold from another decision's migration is how two decisions drift into disagreeing
about one table, which is the failure this record exists to prevent. **168 carries
it**, with the demonstration this migration's own reasoning provides.

#### What the register had to absorb

- **J31 narrowed** to `covered_quantity`. Coverage asks how much of a commitment is
  *spoken for*, which is a question about intentions and D12 makes an allocation
  exactly that. The other three moved to J68. Leaving all four would have kept one
  source answering two kinds of question, which is D53's own catch one level on.
- **J68 flipped without its query changing.** It compared the columns to the ledger
  while the columns were folded from allocations, so it reported the gap; the same
  three predicates are now what the maintainer computes, and a finding means the
  projection has drifted rather than that the source is wrong. A check written to
  report a known gap and then to guard its closure is cheaper than two checks, and it
  cannot be quietly satisfied by deleting the thing it watched.
- **J56 became load-bearing.** `despatched ≤ packed ≤ picked ≤ covered` held by
  construction while one fold produced all four from nested state sets. With three
  from the ledger and one from allocations, nothing ties the sources together: a pick
  with no allocation, or an over-pick, produces `picked > covered`. D99 recorded that
  this would happen; J56 is where it surfaces.
- **J69 and J70**, the two obligations D99 booked against this migration. J69: a
  movement naming a line must move that line's item, reached through `order_line`. A
  finding rather than a CHECK, and not only because it spans rows — D12's founding
  case is the picker who finds lot B where the plan said lot A, so refusing the write
  would stop the floor over exactly the disagreement the system exists to record.
  J70: the package-held storage limit, reported rather than hidden — a movement
  leaving a package that never received stock for that line is a pick the fold does
  not count, and knowing a number is incomplete is different from it being wrong.

**Rejects.** `package.sealed_at` as the packed source, for the reason above. Bounding
packed movements by a seal timestamp, which the carton's current status makes
redundant. Counting net contents of a sealed carton, which reads zero after despatch
and breaks D53's nesting in the one case that matters most. Dropping only the
reversed movement and not its reversal. Fixing the inbound reversal gap here. Moving
`covered_quantity` onto the ledger, which would leave the model with no number for
what a commitment is *meant* to have and make backorder — `quantity - covered` —
unanswerable. A CHECK for the nesting, which now spans two sources and could not hold
anyway. Reordering `projection_step` to state the new dependency, which is already
satisfied at 90; the note is corrected instead, because the ordinal was right by luck
and saying so is what stops the next reordering finding out.

### D101 — A cell held in a carton settles

*Adopted 2026-08-10. Migration 55. Found by the fixture on D100's first
package-held stock cell. Raises 169.*

**`stock` never converged for a package-held cell, and had not since migration 3.**
Two maintainers owned one column between them and neither knew it:

| | | |
|---|---|---|
| 30 | `projection_stock_rebuild` | writes `resolved_location_id = holder_location_id` |
| 70 | `projection_stock_resolve_locations` | writes it from the package's placement |

For a cell held in a location the two agree. For a cell held in a **package**,
`holder_location_id` is NULL by the key CHECK, so 30 wrote NULL over what 70 had
resolved and 70 resolved it again on the next run. `site_id` went the same way,
because 30 reads it off a location it does not have.

Three writes per cell per run, forever. D68 states the property this breaks — *a
rebuild that changes nothing must still write nothing* — because under MVCC each
rewrite is a dead tuple, and a projection rebuilt hourly churns its whole table
hourly.

#### The values were never wrong, which is why nothing caught it

Every run ended with the correct answer. J1 and J7 compare values, and the values
were right at the moment they were read. What was wrong was that arriving at them
cost three writes per run and the fold oscillated instead of settling, which only a
check that counts *writes* can see. D68's test is that check, and it had been passing
because there was nothing of this shape to fail on.

**It went unseen for fifty-two migrations because the fixture had no package-held
stock cell.** Not an untested branch — an untested *shape*, reachable only by putting
stock into a carton and leaving it there, which nothing did until an order was picked
into one. D100 added that cell for an unrelated reason, to stop J68, J69 and J70
being vacuous, and it surfaced on the next run.

#### Two fixes, and only one of them is about ownership

**Step 30 stops writing what step 70 owns.** It cannot resolve a container's location
and should not pretend to: a package's placement is folded at 40, ten steps later,
which is the entire reason 70 exists. The ordinal is the dependency order, so the fix
is not to reorder anything but to stop the earlier step asserting a value it has no
source for. The INSERT arm still writes NULL for both columns, which is correct — a
cell in a carton is created with its container arm unresolved and 70 fills it in the
same run. One write on creation, none after.

**Step 70 stops writing when it has nothing to say.** The second half is a plain D68
omission rather than a shared-ownership problem: the container-arm UPDATE carried no
distinctness guard at all, so it rewrote every package-held cell every run even when
the package had not moved. Its sibling immediately below it, the `site_id` update,
has had one since it was written. `touched` now counts cells whose container arm
actually moved, which matters beyond tidiness — it is what the orchestrator reports
and what D95's freshness stamp is measured against, and a step that reports work it
did not do is one nobody can use to find a stuck fold.

#### What this says about the register

S43 checks that a registered function actually writes the column it claims. **Nothing
checks the converse**: that no *other* function writes it too. Shared ownership is
what this was, it is invisible to every check in the register, and it produced a fold
that was correct and non-convergent at the same time. 169 carries it.

**Rejects.** Reordering the steps, which cannot help — 30 has no source for the
container arm at any ordinal before 40, and moving it after would break the cells 70
reads. Dropping `resolved_location_id` from 30 entirely, which would leave the
location-held case — every cell in the fixture but one — with no writer. Making 70
recompute the whole column for every cell rather than the package-held ones, which
trades a correctness bug for a full-table write. Letting 30 join `package` to resolve
it itself, which is 70 written twice and puts a dependency on 40 into a step that
runs at 30.

### D102 — A correction nets against what it corrects

*Adopted 2026-08-10, settling question 168 and **correcting D100**. Migration 56.
Raises 170.*

168 asked whether a reversed movement leaves the inbound fold. It does not:
`expected_supply.quantity_received` read **100 against a receipt where 96 arrived**,
from the migration that built it. The fixture has contained the evidence since
migration 21 — a receipt of a hundred and a correction of four, thirty lines apart in
the same file — and nothing looked, because D45's fold counts arrivals and a
correction to an arrival is not one.

Answering it produced the rule the outbound fold needs, and applying that rule showed
**D100 had it wrong**.

#### What D100 got wrong

D100 excluded a corrected movement and its correction from the fold entirely. That is
right only when a correction reverses the *whole* of its target, **and it does not
have to**. Migration 10 states the rule as *"quantity: not larger than the target, or
it invents stock"* — not larger, not equal — and the fixture's own correction is
partial: four of a hundred, discovered half an hour later.

So a pick of twenty with five reversed did not read fifteen. It read **zero**, because
the whole movement left the fold, while `despatched` stayed at twenty because its own
movement was untouched. The three stopped nesting, and J56 — made load-bearing by
D100 one migration earlier — would have reported a commitment despatched beyond what
it had picked.

The rule is arithmetic rather than exclusion. **A movement contributes what is left of
it:**

```
effective quantity  =  quantity  -  the corrections against it
```

The correction rows still never contribute directly, and that half of D100 was right
for the reason it gave: reversing a despatch produces a movement *into* a sealed
carton, which reads as packing and would make a correction inflate the number it was
recorded to fix.

#### Why the fixture could not have caught it

The partial correction it contains is on the **inbound** side, where the bug was the
other one. There has never been a corrected outbound movement, because until D99
there was no outbound cause to correct against. So D100 shipped a rule that was
wrong for a case the fixture could express everywhere except where the rule applied.

That is 169's subject arriving from a different direction: the fixture reaches every
table and not every *shape*, and the shapes it misses are the ones no check knows to
ask for.

#### The limit, named rather than discovered later

The netting is one level deep. A correction to a correction — meaning the units were
right after all — should return the original to its full quantity and does not: the
second correction is subtracted from the first, which is not in the fold, so it
contributes nothing anywhere. Resolving it is recursive, because J52 permits a chain
of any depth, and that is not built. **J71 reports the shape instead**, the same
treatment as J70 and for the same reason. 170 carries the build.

Note what this does not claim. The chain is legal, `stock.quantity` folds it correctly
because that fold is a signed sum over every movement, and D47 intends corrections to
be correctable. Only the cause-grouped folds cannot see past the first level.

#### A near-miss worth recording

Migration 56 was first written against **migration 21's** body of
`projection_expected_supply_rebuild`, because that is the migration D45 and J26 name.
Two later migrations had replaced it — D65's `received_in_full` close at 25, and
D68's idempotency guard at 27 — so rebuilding from 21 silently reverted both.

D68's own write-counting test caught it within a minute: `expected_supply` churned one
row per run and the test that exists for exactly that said so. **A `CREATE OR REPLACE`
carries no record of what it replaced**, and the decision record names the migration
that *introduced* a function rather than the one that last changed it, so the obvious
place to look is reliably the wrong one. The down migration is rebuilt from 27 and
says so in a comment, because the same trap is set for whoever reverses this.

**Rejects.** Netting only when the correction is total, which is the same bug with a
narrower blast radius. Subtracting corrections as negative rows in the fold, which
reads a `nothing → carton` reversal as packing — the failure D100 avoided and this
keeps avoiding. Recursive netting, deferred to 170 rather than built under a decision
about something else. Fixing the inbound fold without correcting the outbound one,
which would leave two folds disagreeing about what a correction means. Leaving
`quantity_received` at 100 and calling the four units a receiving-screen concern,
which is precisely the stored-accumulator drift S36 exists to prevent, arrived at by
a fold instead of a column.

### D103 — A correction to a correction

*Adopted 2026-08-10, settling question 170. Migration 57. Retires J71.*

D102 made a movement contribute what is left of it after the corrections against it,
one level deep, and named the rest as a limit. This builds the rest.

```
effective(m)  =  m.quantity  −  Σ effective(r) over the corrections r to m
```

#### The recursion has a closed form, and that is the whole decision

Expanding the definition collapses it:

```
effective(P) = P − effective(R1)
             = P − (R1 − effective(R2))
             = P − R1 + R2 − effective(R3) = …
```

**Every level alternates sign.** So the effective quantity is a single signed sum over
a movement's whole correction subtree — positive at even depth, negative at odd — and
it holds when the tree branches, because a movement corrected twice in parts has two
children at depth one and both are subtracted.

That matters beyond elegance. The definition is *bottom-up* and SQL's recursion is
*top-down*, so a per-level walk with an aggregate at each step is not expressible as
one `WITH RECURSIVE`. The closed form turns it into a top-down walk plus a `GROUP BY`:
one query, one pass.

It is also exactly how `stock.quantity` has always worked, which is why that fold
never had this bug — it is a signed sum over every movement. 170 said the clue was
there, and it was.

#### One definition, five readers

`quantity_received`, the three progress quantities, J26, J51 and J68 all need this
number. **D102 duplicated a simpler version into four places and nearly paid for it**:
rebuilding one copy from the wrong migration silently reverted two decisions, and only
D68's write-counting test noticed.

So it is a view, `stock_movement_effective`, and a reader that drifts from it has to
do so visibly by not using it. `security_invoker` because RLS should apply as the role
asking rather than as the view's owner — the two views this schema already has predate
that option and read as their owner, which for a superuser-owned view means no tenant
filtering at all. They are saved by every caller filtering explicitly. This one does
not rely on that.

The `CYCLE` clause is not decoration. **J52 asserts acyclicity and reports a loop as a
finding; it does not prevent one**, and a maintainer that hangs is worse than one that
raises — which is D98's subject one class up. The view stops rather than trusting the
check.

#### What the register absorbed

- **J51 widened to the chain.** The rule is unchanged and its arithmetic is not: a
  movement is over-reversed exactly when what is left of it goes negative. The
  difference is a real case rather than a refactor — a correction of twenty-five that
  was itself corrected by ten takes back fifteen, and against a target of twenty that
  is legitimate. The face-value form reported it as overreach, which is a check firing
  on its own arithmetic rather than on the data.
- **J71 is retired.** D102 created it to report the shape these folds could not
  represent, and they can now. Its number is not reused.
- **J26 and J68** read the view, so all five readers share one definition.

#### A citation error, corrected

D102, J71, migration 56 and question 170 all named **J53** as the invariant permitting
a chain of any depth. J53 is about `record_error` amendments carrying a moment that
exists; **J52** is the acyclicity check. Four places, one wrong number, propagated by
being copied from the first. Corrected here rather than left, because a reader
following it lands on an unrelated rule and the register's whole value is that its
numbers resolve.

**Rejects.** A depth cap instead of the `CYCLE` clause, which trades a hang for a
silently truncated sum. Materialising the effective quantity as a column on
`stock_movement`, which is a stored accumulator over an append-only table and S36's
subject exactly. Repurposing J71's number for the new overreach rule, since a number
that has meant two things is worse than a retired one and J51 already owned the rule.
Leaving J51 on face values, which would have it report a legal chain as invented
stock. Forbidding chains, which D47 permits deliberately: a correction is a fact and
a fact about a fact is still a fact.

### D104 — A projected column has one writer, and a function says when it last changed

*Adopted 2026-08-10, settling question 169. Migration 58. Raises nothing.*

D101 fixed the dual-writer bug and raised 169: nothing in the register would have
noticed it, and nothing records which migration last changed a function. This is
both, as structural checks rather than as another fold.

#### S55 — the converse of S43

S43 checks that a registered family writes the column it claims. **Nothing checked
that nobody else does.** The shared write of `stock.resolved_location_id` by steps
30 and 70 was invisible to S43 because both functions are in the same
`projection_stock%` family, and S43's unit is the family.

S55 reads every VOLATILE `projection_%` body for table-scoped writes — `UPDATE … SET
col =`, or `INSERT INTO table` with a `DO UPDATE SET col =` — and requires:

1. Every writer is in the registered family.
2. A column with more than one writer is on a **declared multi-writer allowlist**,
   with exactly those co-owners.

The allowlist has two entries today, both the D101 pair: `stock.resolved_location_id`
and `stock.site_id`, written by `projection_stock_rebuild` (location-held arm) and
`projection_stock_resolve_locations` (package-held arm). Adding a row is a decision,
not a silence. A dual write of `stock.quantity`, or a third writer of either, fails
without waiting for a fixture shape — which is what kept D101 latent for fifty-two
migrations.

#### S56 — last changed

Migration 56 was first written against **migration 21's** body of
`projection_expected_supply_rebuild`, because that is the migration D45 and J26 name.
Two later migrations had replaced it, so rebuilding from 21 silently nearly reverted
D65 and D68. **A `CREATE OR REPLACE` carries no record of what it replaced.**

Every VOLATILE `projection_%` function now carries a body comment:

```
-- last changed: migration NNN (Dxxx)
```

Body comment rather than `COMMENT ON FUNCTION`, so pasting an older body keeps the
older marker and an intentional edit bumps it. Migration 58 backfills the marker on
every current maintainer from the live definitions — only the comment line is new —
because rewriting a fold from the migration that introduced it is exactly the trap.

#### What this does not settle

The deeper half of 169 is that the fixture reaches every *table* and not every
*shape*. That is a discipline for every later fixture edit rather than a question
with a yes-or-no answer, so it is not re-raised. J68/J69/J70 examining few rows and
J51's chain living only in a rolled-back test remain carry-forward.

**Rejects.** A registry column for the owner per projection row (option B of 169),
which is the right long-term shape but heavier than the near-miss warrants before
more folds: the allowlist is the same information for the one pair that needs it.
Relying on D68 alone, which failed for fifty-two migrations. A catalogue table of
function revisions, which is more ceremony than a body comment for the same signal.
Leaving last-changed only in the decision record, which is what failed last time.

### D105 — D33's test, applied once, and a movement says what kind it was

*Adopted 2026-08-10, settling question 143. Migration 59. Raises nothing.*

143 asked whether D33's enum test is applied consistently, and named a list of
columns that look fixed but are not enums. The outbound walk then found something
worse than any of them: `stock_movement.reason` is `text NOT NULL` with **no CHECK
at all**, and nothing folds on it only because D99 refused to. This states the
classification once, and closes the unconstrained column.

#### The test, restated

D33:

> A value set is a **table** when it carries attributes and grows independently of
> the code that reads it. It is an **enum** when code branches on it, because then
> the set is closed by the code that handles it, and adding a value without adding
> handling is a bug rather than a configuration.

What 143 found is a third shape the test does not forbid: **text with a CHECK**.
That is the right form when the set is fixed for operators and reports, but no
application branch depends on every value being handled — adding a synonym is a
vocabulary change, not a missed `match` arm. It is weaker than an enum (Postgres
will not force the Rust side to exhaust) and stronger than free text (a typo is a
constraint violation, not a silent new kind).

#### Classification

| Column | Shape | Why |
|---|---|---|
| `order.state`, `fulfilment.state`, `discrepancy.state`, `discrepancy.kind`, `policy_kind`, `channel_authority`, `revision_class` | **enum** | Code branches; a new value without a handler is a bug |
| `stock_allocation.state` | **text + CHECK** (seven values) | Progress once folded on it; coverage still does (`projection_fulfilment_rebuild` FILTER). **D33's test says enum**, and converting is the right move the day a value is added — the trigger 143 already named. Not converted now: the CHECK already refuses typos, and the cost of an enum change is what the trigger is waiting to pay for |
| `package.status`, `package_event.kind` | **text**, no CHECK on status | Folds of an event log; the event kinds are the vocabulary, and status is derived. A CHECK on the fold would freeze a projection |
| `location.kind`, `observable.kind`, `item.tracking`, `consignment.status`, `freight_provider.kind` | **text + CHECK** where one already exists; text otherwise | Fixed operational vocabularies, not match arms. `freight_provider.kind` already has a CHECK of three values |
| `stock_movement.reason` | **text + CHECK** (migration 59) | Fixed labels for what kind of movement a row is. **Not a fold discriminator** — D99 and D100 fold by shape, and J68 asserts that. Distinct from `adjustment_reason_id`, which says *why* an adjustment was recorded |

#### The only schema change

```
reason IN ('receipt', 'putaway', 'pick', 'despatch', 'move', 'adjustment')
```

Every value the fixtures and walks already write. `putaway` is in the year
generator; the rest are in seed and the suite. Adding a seventh is a migration,
same as any other CHECK vocabulary.

**Folds still do not read it.** That is deliberate and unchanged: a pick is bin →
carton naming a fulfilment line, a despatch has no `to` side, packing is
`package.status`. A CHECK makes the trap safe to walk past rather than inviting
the next fold to step on it.

**Rejects.** Making `reason` an enum, which D33 would want only if code branched on
every value — nothing correct does. Folding progress on `reason = 'pick'`, refused
again for D99's reasons (batch picking undercounts, consolidation double-counts).
Converting `stock_allocation.state` to an enum in this migration, which is the
right shape but the wrong moment: 143's own trigger is "before the state machine
gains a value", and no value is being added. Leaving `reason` unconstrained
because folds do not read it, which is how free-text NOT NULL columns fill with
`"update"` (question 87) and how the next fold author reaches for it.

### D106 — A fold joins only the roots it needs

*Adopted 2026-08-10, settling question 164. Migration 60. Raises nothing.*

D95 measured `projection_run_all` at about two seconds against a year of history and
raised 164: every maintainer is a full-tenant fold, cost is O(history), and the
cadence has a ceiling that belongs to the fold rather than the scheduler. The
candidates were watermarks, dirty-marking cells, or accepting the ceiling.

#### What remeasurement found

Against the same year (289,080 movements, gamma tenant), after D99–D105:

| | median `projection_run_all` | `projection_expected_supply_rebuild` |
|---|---|---|
| D95 (ten maintainers) | ~2.0 s | not broken out |
| Before this decision (thirteen maintainers) | **~12.8 s** | **~12.2 s** |
| After this decision | **~0.55 s** | **~0.33 s** |

Almost the entire regression was one join. `quantity_received` joined
`stock_movement_effective` — a recursive view over every uncorrected movement in the
tenant — into a nested loop over every receipt line. The plan materialised 289,080
effective rows and joined them to 35,040 arrivals by filter, removing about **ten
billion** intermediate rows. `EXPLAIN ANALYZE` of that join alone took six minutes.
The same arithmetic, rooted only at arrival movements that name a supply row,
finishes in about fifty milliseconds.

`projection_stock_rebuild` was never the problem: ~170 ms, under a tenth of a percent
of its five-minute freshness bound. Fulfilment progress was ~270 ms before and is
~1 ms after the same scoping, against a year that has almost no outbound movements.

#### What was decided

**Full-tenant folds remain.** Watermarks and dirty-marking are not built. They are
the right tools when a maintainer that already reads only what it needs still cannot
meet its freshness bound, or when sequential-tenant cadence math fails. Neither is
true after the join fix:

| | |
|---|---|
| Median year fold | ~550 ms |
| Tightest bound (stock family) | 5 minutes = 300,000 ms |
| Fold as fraction of that bound | ~0.2% |
| Sequential tenants per 5-minute cycle | ~500 |

**Maintainers restating D103's closed form over the roots they need is mandatory.**
`stock_movement_effective` stays the single *definition* — jobs and documentation
read it; a second arithmetic must not invent itself. What is forbidden is joining
that view over the whole ledger against a filtered movement set. The planner will
nested-loop, and the cost is O(roots × filters) rather than O(roots). Migration 60
scopes the arrival fold and the fulfilment fold that way; the view comment says so.

**D68 is unchanged.** A no-op rebuild still writes nothing. The year fold reports
zero rows touched on every warm run.

#### Cadence is arithmetic, not a surprise

N tenants cost N × fold-time per sequential cycle. At half a second that is about
five hundred tenants inside a five-minute stock bound. This product is not near that
line. When it approaches — or when any step's `last_run_ms` is a material fraction
of its `freshness_bound` — O(delta) is the next decision, not a hope. J66 already
notices age; `projection_freshness.last_run_ms` is what a measurement script reads
for cost. `scripts/measure.sh` prints the per-step breakdown so the next regression
is not "the fold got slower" without a name.

**Rejects.** Building watermarks now, which optimises a cost that was a join plan
rather than the fold shape. Dropping `stock_movement_effective`, which would
reintroduce the duplication D103 paid to remove. Leaving the global join because
"the view is the definition", which confuses a definition with a scan plan.
Raising freshness bounds to hide a twelve-second fold, which is D95 inverted.
Granting the app `projection_run_all` so screens force a rebuild — still a
half-second on the write path and still D25's separation.

### D107 — A tenant may ask for a rebuild; the floor never waits for one

*Adopted 2026-08-11. Migration 61. Raises nothing.*

Outbound write paths (pick, package create, seal, despatch) insert ledger facts
and deliberately do not call the maintainers (D25, D95). That left a product gap:
the floor needs the *consequence* of the write it just made, and occasionally the
whole cache must be forced to converge, without the app becoming the maintainer
or holding `EXECUTE` on `projection_run_all` (refused by D95 and restated under
D106).

Three mechanisms, in the order the product needs them. Migration 61 builds B and
C; A is application code with no schema.

#### A. Live ledger views (HTTP, O(entity))

Write responses and single-entity GETs return a second view of the same facts:
`ledger` progress for a fulfilment line, and `status_ledger` for a package. Both
restate D99/D103 (and J6's event order) over **one** line or package — D106's
scoped form — so cost is O(that entity), not O(tenant history). They never UPDATE
`@projection` columns. List endpoints stay projection-only so the index stays
cheap.

The dual view is the floor's primary answer. A pick response can show 20/20/15
before the next scheduler run moves the cache.

#### B. Dirty set for the scheduler

`projection_dirty` is one row per tenant (upsert). After a floor write the app
calls `projection_mark_dirty(tenant, reason)`. The scheduler drains with
`projection_run_dirty`, which runs `projection_run_all` for each dirty tenant
oldest-first and clears on success. Owned by `nylonite_projection_owner`; only
scheduler/platform may execute the drain. Full-tenant folds remain (D106);
this only prioritises who runs next.

#### C. Rate-limited on-demand refresh

`projection_refresh_tenant` is a SECURITY DEFINER wrapper the app *may* call:

| Rule | How |
|---|---|
| Current tenant only | Requires `nylonite.tenant_id`; rejects any other `p_tenant` |
| Not `projection_run_all` | App still has no EXECUTE on the orchestrator (D95) |
| One accepted rebuild per 5 s | `projection_freshness.last_on_demand_at`, independent of scheduler stamps |
| Rate-limit ≠ warm rebuild | Returns `NULL` when limited; `0` when accepted and D68 touched nothing |
| Clears dirty | Accepted rebuild removes the tenant from `projection_dirty` |

HTTP surface: `POST /projections/refresh` → `{ rows_touched, rate_limited }`.

#### What this is not

It is not O(delta) watermarks (still deferred under D106's trigger). It is not
the app writing progress columns. It is not a grant that lets one tenant rebuild
another. S49's non-step list now includes the three D107 helpers beside
`projection_run_all`; S56 requires their last-changed markers like every other
volatile maintainer.

**Rejects.** Granting the app `EXECUTE` on `projection_run_all`, which D95
refused. Blocking the write path until maintainers finish, which makes latency
the fold's problem. Skipping live ledger views and relying only on refresh, which
puts half a second (or a rate-limit wait) between a pick and the number the
screen shows. A per-cell dirty bitmap before any step's cost is a material
fraction of its freshness bound (D106's own trigger, still not met).

#### Follow-up: drain sets the tenant (migration 62)

Migration 61's `projection_run_dirty` selected from `projection_dirty` without a
tenant setting. FORCE RLS applies to the projection owner, so the tenant-scoped
policy hid every row and the drain was a no-op. Migration 62 loops tenants,
`SET LOCAL` each id, then sees and clears that tenant's dirty row under the same
RLS shape S9 already permits — no `USING (true)`. The `nylonite-scheduler`
binary is the process that calls the drain on an interval.

## Open questions



**These have moved.** [open-questions.md](./open-questions.md) is the single
register and the canonical numbering. It consolidates the questions that lived
here with those left behind in `mechanism-design.md` (73 to 88),
`inbound-analysis.md` (58 to 72), `supply-side-design.md` and
`d24-open-questions.md` (116 to 119), and resolves three numbering collisions.

The list below is retained because each entry sits with the decision that raised
it, which is useful when reading a decision. **The register is authoritative on
status.**



1. ~~Lot/batch and expiry~~ — settled by D14, and the scope question is answered:
   the business distributes **food safety products** (gloves, hair nets,
   protective equipment), which are largely non-perishable. So `tracking = lot` is
   the **exception, not the default**, and rotation applies to a small part of the
   catalogue. The capability is built for breadth, not for current need — see D20.
2. ~~Does a fulfilment ever span multiple orders?~~ Settled by D15: no. Waves are
   a work grouping and belong in `pick_batch`, not in the order structure.
3. ~~Does a consignment ever span multiple fulfilments?~~ Settled by D15: yes,
   and it always could — via `consignment_package → package → fulfilment`.
   `consignment.fulfilment_id` is dropped.
4. **Is `order` ours, or a mirror of NetSuite's?** During coexistence it is a
   mirror. The field list above is deliberately thin so that the mirror is cheap
   and the eventual ownership is not painful.
5. *(Vocabulary supplied by D23's `dimension`/`unit`; the catch-weight case is
   settled by D20.)* **Item base units.** `base_unit` assumes each item has one sensible base.
   Anything sold by weight or length breaks that assumption. *(Likely answered by
   the `entered_quantity`/`entered_unit` change — see the competitor analysis.)*

Raised by D5–D7:

13. ~~Does the count-as-assertion approach hold?~~ Resolved by D8, and the
    late-arrival case is settled by D25: a recomputation that contradicts an
    `acceptance` does not lose, it raises `accepted_state_contradicted`.
14. ~~Nosdesk: shared workspace, or service boundary?~~ Settled by D26: shared as
    **library crates** (sandbox, bridge, consent, signing), not as a deployment;
    the plugin collection store is explicitly not shared. ~~Sharing the platform
    could mean one Cargo workspace with shared crates, or two services with an
    API between them. Affects deployment, migrations and blast radius (D7).
15. **What is the reconciliation UI for negative stock?** D5 accepts negative
    balances as discovered discrepancies rather than errors. That is only
    defensible if there is a real surface where they get resolved — otherwise it
    is just tolerated corruption with a nicer name.
16. **Does anything here genuinely need a document CRDT?** D5 rules Yjs out for
    stock. If nothing else needs it, the Nosdesk reuse is transport only, which
    simplifies question 14 considerably.

Raised by D8:

17. ~~How does a movement reference its cause?~~ Settled by D10: typed FKs.
18. ~~Does `actor_id` mean who did it or who is accountable?~~ Settled by D11:
    `recorded_by_id`, `work_session_id` and `authorised_by_id` are separate.
19. ~~What is the tolerance policy?~~ Settled by D9: operator-configurable, not
    hard-coded, with point-of-capture challenge as the primary mechanism.

Raised by D9–D11:

20. ~~Does `measurement` get typed subject FKs too (D10)?~~ Settled by D23: yes,
    but on the `observable` registry rather than on the fact, which is what keeps
    the subject set open. ~~Consistency says yes.
    The counter-argument is that `measurement` is append-only reference data on a
    cold path, where the batch-loading argument does not apply — so this may be
    a case where the polymorphic pair genuinely costs nothing.
21. ~~Who configures tolerances, and at what grain?~~ Settled by D22: the scope
    lattice, with `count_tolerance` and `order_tolerance` as separate kinds. ~~Per item, item class, site,
    or location kind? D9 says the operator decides, but not yet at what
    resolution they express it.
22. **What is a `work_session` in practice?** A shift, a task, a wave, or an
    ad-hoc grouping someone opens and closes? The schema does not care; the floor
    process does, and it determines whether sign-on is a habit or a chore.
23. **Can a movement have no `work_session`?** Modelled as nullable, so solo work
    just has `recorded_by_id`. Worth confirming that is right rather than forcing
    everyone into a session of one.

Raised by D12:

24. ~~FEFO versus travel — which wins?~~ Settled by D13: neither, in code. The
    model holds expiry, travel and access cost; a manager sets the weights.
25. ~~When are allocations released?~~ Settled by D22:
    `allocation_policy.allocation_expiry_hours`. ~~Explicit release on cancellation is
    obvious. Less obvious: does an allocation expire? One held for a week is a
    lie that suppresses availability for everything else. A sweep needs a rule.
26. **Who allocates, and when?** At order import, on a schedule, at wave
    creation, or on demand when picking starts? Allocating late reduces churn;
    allocating early makes ATP meaningful. Probably late plus an explicit
    "commit this order" action, but it is a real choice.
27. **Does allocation cross sites?** Modelled as not — a cell belongs to one
    location and therefore one site. Multi-site fulfilment of a single order
    would change this, and relates to question 2.

Raised by D13:

28. **Does the allocator run before the location survey exists?** Travel and
    access scoring need coordinates and `reachable_by`, and neither is populated
    yet. Until then the weights collapse to rotation only — which is fine, but it
    means the survey gates allocation *quality*, not just the map. A nullable
    `location.sequence` as an interim travel proxy would soften this.
29. **Is `relative_cost` on `equipment_class` enough**, or does access cost need
    to account for equipment *availability* (one forklift, three people wanting
    it)? Queueing is a scheduling problem, and modelling it properly is a much
    larger commitment than a scalar.
30. ~~Who may change an `allocation_policy`, and is the change audited?~~ Settled
    by D22: `policy_change` is a fact with a mandatory reason. ~~These
    weights directly affect spoilage and labour cost. Per D8's spirit, a policy
    change is exactly the kind of thing you want to correlate against a later
    change in findings — which argues for policy edits being facts too.

Raised by D14:

31. ~~How is `tracking = lot` enforced?~~ Settled by D33: challenged at capture,
    accepted if confirmed, raised as a finding. A trigger would block the floor,
    which D5 refuses. ~~A CHECK cannot reach from
    `stock_movement.lot_id` to `item.tracking`. Options are application-level
    validation plus a periodic assertion (consistent with how `stock` is already
    reconciled), or a trigger. The first fits the existing pattern; the second is
    stricter. Worth deciding once, since putaway, receiving and adjustment all
    need the same rule.
32. ~~Can a lot exist before its goods arrive?~~ Settled by D24 (supply side):
    **no**. The advised code and expiry ride on `expected_supply` as raw,
    non-authoritative strings. ~~Supplier ASNs name lots ahead of
    delivery. If yes, `lot` is created by inbound rather than by the first
    movement, and `received_at` becomes nullable — which is fine, but it means
    lots can exist with no stock, and expiry reporting must not count them.
33. ~~Is `min_shelf_life` per customer, or per customer and item?~~ Settled by
    D22: `shelf_life_policy` on the lattice, any combination of dimensions. ~~Modelled on
    the customer. A single retailer often has different requirements by category,
    which would push it to a customer-item-class pair.
34. **What happens to allocations when a lot is held?** D14 says they become
    findings. But should the system also auto-release them so the demand
    re-allocates to good stock, or wait for a human? Auto-release is convenient
    and quietly discards the evidence of what the plan had been.

Raised by D15:

35. **What identifies a delivery to the customer?** With two orders consolidated
    onto one consignment, the customer receives one delivery containing two
    orders. Tracking is per-consignment and per-package, which works — but
    packing lists, ASNs and customer notifications need an explicit answer about
    whether they are per-order or per-consignment.
36. **Can packages from different customers share a consignment?** Physically yes
    for a milk run; commercially it depends on the carrier and the rate. Nothing
    in the schema forbids it, which is correct, but the allocator and any
    consolidation logic need a rule.
37. ~~Is an inter-site transfer a fulfilment?~~ Settled by D16: it is a fulfilment
    against a `transfer_order` rather than an `order`. Every package on a
    consignment has a fulfilment, with no exception.

Raised by D16:

38. ~~Does a transfer's receipt reconcile against its despatch automatically?~~
    Settled by D24 (supply side): yes, through the destination-site
    `expected_supply` row. The **destination owns the variance**, and it is
    suppressed as a timing difference until `expected_to + supply_overdue_hours`.
    ~~Original:~~
    Shipped 100, received 98 is a discrepancy (D8) — but which site owns it, and
    at what point does in-transit shrinkage become someone's finding rather than
    a timing difference? Needs a rule, since transfers will otherwise generate
    noise every time a truck is mid-journey at a reporting boundary.
39. ~~Do shelf-life rules apply to transfers?~~ Settled by D22:
    `shelf_life_policy.min_remaining_days_transfer`, with a site floor via
    clamping. ~~D14 puts `min_shelf_life` on the
    customer, and a transfer has none. If site B serves a customer who demands 90
    days, sending them stock with 30 days left is a real failure that the current
    model would not catch.
40. ~~Can a transfer be allocated before it arrives?~~ Settled by D24 (supply
    side): **yes**, from the moment of despatch, against a destination-site
    `expected_supply` row. The transfer arm has zero exposure to rule 3 and is the
    arm to build first. ~~Committing inbound stock to
    outbound demand is normal practice, but our allocation is against a specific
    `stock` cell (D12), and in-transit stock is in no cell at all.

Raised by D17:

41. **Does a count task lock its location?** D8 computes variance against ledger
    state at `counted_at`, which works without a freeze. But a picker taking
    stock from a cell mid-count produces a variance that is a timing artefact,
    not a finding. Either counts tolerate it (and D8's tolerance settings absorb
    the noise), or count tasks block picking on that cell — which is
    coordination, and needs justifying against D17's stated line.
42. **How does `sequence` get computed before the survey?** Travel order within a
    batch needs the same coordinates D13's scoring needs, and they do not exist
    yet. The interim `location.sequence` proxy would serve both, which
    strengthens the case for adding it now rather than waiting.
43. **What closes a `pick_batch`?** All tasks terminal is the obvious rule, but a
    batch with one permanently failed task would never close. Probably needs an
    explicit abandon, which is itself a decision worth recording.
44. ~~Are `activity_event` kinds an enum or a table?~~ Settled by D33: enum, and
    the table-versus-enum test is now stated as a rule. ~~An enum is honest and
    typed; a table invites per-site custom kinds, which is a small step toward
    the rules-engine-by-accretion D13 warned about. Leaning enum.

Raised by D18:

48. ~~Will we ever hold third-party stock (3PL)?~~ Yes — settled by D20.
    `owner_id` joins the `stock` key now rather than as a later migration.
49. *(De-risked by D20: this changes the deployment, not the schema.)*
    **Are the Australian states one legal entity or several?** If several, they
    may be separate tenants that nonetheless move stock between each other —
    which D18 says is impossible, and would need an inter-tenant transfer concept
    (effectively an internal sale). This is the one thing that could invalidate
    the site-not-tenant reading.
50. ~~Is reference data per-tenant or shared?~~ Settled by D19: shareable when
    intrinsic, tenant-scoped when observed or operational.
51. ~~Does `person` span tenants?~~ Settled by D19: yes, via `person_tenant`.

Raised by D19:

52. **Who governs the shared catalogue?** If tenant A edits a shared item's
    description, tenant B sees it. Either shared reference data is
    platform-managed and read-only to tenants, or a tenant needing a change forks
    it into a tenant-scoped copy. The fork is more flexible and quietly
    reintroduces the duplication that sharing was meant to avoid.
53. ~~Are carriers and `package_type` shared too?~~ Answered for carriers by D32:
    yes, and the intrinsic/operational split applied a third time without being
    fitted, which was the stated test. `package_type` still wants confirming.
    ~~A carrier looks intrinsic —
    Swift is Swift — but `carrier_profile` (despatch times, caller values) and
    account credentials emphatically are not. Probably the same intrinsic /
    operational split applied again, which would be a good sign the split is real
    rather than fitted to items.
54. **Can a `work_session` span tenants?** A person may belong to two, but one
    shift crossing tenants would make `work_session_id` ambiguous on movements.
    Simplest answer is no — a session belongs to one site, therefore one tenant —
    but it should be stated rather than assumed.

Raised by D20:

55. ~~Can a customer order by weight rather than by count?~~ Settled by D20's
    revision: yes, as unit conversion. Allocation plans on nominal weight;
    closest-fit happens at pick time where the scale is.

    **Tolerance is a policy object, not a scalar** *(2026-07-31)*. It resolves
    like every other policy here — most specific wins across site, item class,
    item, customer and order line — but the *shape* stays open too, because
    `± 2%` is only one of the models an operation might need:

    - symmetric percentage or absolute
    - asymmetric (`never under, up to 5% over` is common in food)
    - stepped by order size, where small orders need looser proportional limits

    So `tolerance` is its own small typed entity rather than a column on
    `order_line`, and which one applies is resolved the same way `allocation_policy`
    is (D13). This keeps the environment and the product each able to express what
    they actually need, without a tolerance column sprouting on five tables.
    Consistent with D20: the capability exists where it is needed and is invisible
    where it is not.
56. **Does an inter-company movement generate documents automatically?** D20 says
    crossing `legal_entity` is a sale. Whether we raise the corresponding order,
    purchase order and invoice, or merely flag it for the finance system, decides
    how far this project reaches into accounting.
57. ~~Is `party` one table or several?~~ Settled by D32: one `party` for
    identity, `party_role` for what a company is to us. Roles are not exclusive,
    so neither a discriminator nor separate tables can represent a carrier that
    also invoices. ~~Modelled as one with a `kind`, which is
    the generic-document-model smell the project has otherwise avoided. The
    counter-argument is that a customer can also be a supplier and the same
    carrier can be both — real overlap that separate tables handle badly.
    Worth revisiting before it is built.

Raised by D24 (adopted 2026-08-01):

89. ~~Does a failed container scan need an `activity_event`?~~ Settled by D28:
    **yes for resolution failures, never for decode failures** — the latter are
    not observable on the hardware. `scan_ok` deleted.
90. ~~What mints a package at receipt when the supplier sends no SSCC?~~ Settled
    by D29: **nothing**. Goods land at a dock location; three minting triggers,
    all ours; internal licence plates, never SSCCs.
91. ~~When does the reaper run, and what is "unreferenced"?~~ Settled by D30:
    exactly one FK in every state, and `stock.id` is a handle rather than an
    archival key — a rule the model already obeyed in four places.
92. ~~Is `depth <= 2` enforced on write or on projection?~~ Settled 2026-08-01:
    neither. It cannot be a CHECK on a projection without making the log
    unprojectable, so it is a finding (`nesting_too_deep`) with a fixed three-hop
    fold returning NULL beyond. See D24.

Raised by D22 (adopted 2026-08-01):

93. **The eleven per-kind precedence orderings are undocumented.** Eleven
    orderings of six dimensions, declared in a Rust const. Counterparty over
    product for shelf life, product over space for putaway — both defensible,
    neither obvious, and a manager who assumes wrong misconfigures confidently.
    Each needs a written justification, not just a declaration.
94. **S23 — "no resolver call inside a loop" — is probably not enforceable** as an
    AST check in Rust, with closures, iterator chains and helper functions in
    play. Worth having, but the batch-first interface shape is doing the real
    work and should not be assumed redundant.
95. **`affected_resolution_count` is computed before a taxonomy move — against
    what?** Active bindings is cheap; actual future resolutions is unbounded. It
    needs a defined denominator or the number is theatre.
96. **Does `metric` want a hierarchy?** It is the only flat scope dimension. "All
    temperature metrics" is a plausible near-term ask, and adding a tree later
    changes existing depth vectors — the same class of hazard as q78.

Raised by D23 (adopted 2026-08-01):

97. **`observable` has ten arms and one partial unique index each, and it grows
    monotonically with the domain.** The discriminated-union rule licenses it, but
    the growth path should be acknowledged: at what count does the CHECK and the
    index set stop being reasonable? Probably never in practice, but it should be
    a noticed threshold rather than a surprise.
98. **`observation` denormalises five columns** (`observable_id`, `observed_at`,
    `metric_id`, `result_kind`, `dimension_id`) from its event and metric, with
    composite FKs enforcing agreement. That is the price of database-enforced type
    safety on the typed value columns, and it is roughly 40 bytes a row on the
    second-largest table. Deliberate, but worth measuring before it is 10⁷ rows.
99. ~~`device` is referenced by adopted decisions and defined by none.~~ Settled
    by D27: one table, two roles, calibration as an append-only fact.
100. **Does `metric.applies_to` belong in data?** It constrains which `observable`
    arms a metric is legal against — arguably a type rule, which D23's own
    reasoning would put in code. It reads as the one place the vocabulary/type
    line is blurred.

Raised by D25 (adopted 2026-08-01):

101. ~~What is the idempotency retention window?~~ Settled by D31: derived, not
    chosen. `client_event` lives as long as the facts referencing it, and the
    "never partitioned" premise was wrong. ~~`client_event` is the one
    table that can never be partitioned — partitioning it would reintroduce
    the exact per-partition-uniqueness bug it exists to prevent. It takes a row
    per fact-producing act, forever, and nothing says when rows may go. If a
    handheld can be offline for a week the window is a week; if the answer is
    "forever", that is an unbounded unpartitionable table and it should be a
    decision rather than a discovery.
102. *(Answered for the compiler by D26: ownership, `FORCE ROW LEVEL SECURITY`,
    its own audit. Still open for the projection maintainer itself.)*
    **Who may write a `@projection` column during a rebuild?** The maintainer
    role has grants the application role does not, so the rebuild path is the one
    place the guard is deliberately open. It needs the same `FORCE ROW LEVEL
    SECURITY` treatment and its own audit, or it becomes the way around every
    other rule here.

Raised by D21 (adopted 2026-08-01):

103. ~~Rule 3's positive half has no target.~~ Settled by D24 (supply side):
    assertions project into `expected_supply`, and rule 3 is narrowed in writing
    rather than stretched. ~~Assertions must never project into
    `stock` or commitment, which is settled — but where they *do* project depends
    on D24's supply-side (`expected_supply`), which is not adopted. Until it is,
    an ASN informs nothing downstream, which makes cross-dock and pre-receipt
    allocation unreachable rather than merely unbuilt.
104. **`assertion_check` holds a third copy of both values.** The asserted value
    is an observation, the observed value is an observation, and the check
    denormalises both. Justified — a comparison must be immutable and
    self-contained for a dispute, same argument as
    `goods_receipt_line.expected_quantity` — but it is a third copy and should be
    a noticed cost.
105. **What is `automation_key`?** D11 is extended to machine actors on
    assertion-ingestion facts, but nothing says whether an automation key is a
    row in a table, a config value, or a service identity. Accountability under
    D8 reaches a person; it needs to reach *something* auditable here. *(D27
    narrows it: an automation key is **who**, a device is **how**, and they are
    orthogonal — so it is not solved by pointing it at `device`.)*

Raised by D24 (supply side), adopted 2026-08-01:

106. ~~Does `fulfilment_line` get a maintained `allocated_quantity`?~~ Settled:
    **yes**, with a generated `uncovered_quantity` and a partial index. Justified
    by symmetry with D12's supply-side fold rather than as an exception to it.
107. ~~Does title change while in transit, and do we need to record it?~~
    Settled: **out of scope, with the boundary stated** — we model custody and
    allocatable ownership; legal title timing belongs to the finance system. The
    rebuildability concern was misplaced: `owner_id` is a projection of the source
    line describing the arrival state, now marked as such. See D24 (supply side).
108. ~~Are intermediate re-points reconstructable?~~ Settled: **first, last and a
    volatility counter now; the full path deferred with a trigger.** PO
    provenance comes from containment rather than allocation history, promise
    slippage from findings, and auditing an automated re-allocator wants a
    `planner_decision` fact — not an event log for intentions. See D24 (supply
    side).
109. ~~Multi-PO ASN — in or out?~~ Settled: **in, and already supported.** The
    X12 ORDER hierarchy level partitions advised content by PO, so a content line
    names exactly one PO line and the scalar FK is correct. Recorded as an
    omission on a misreading. See D24 (supply side).

Raised by D26 (adopted 2026-08-01):

110. **What is the materialisation authority?** The compiler role owns generated
    tables and runs DDL from tenant-supplied declarations. Who may *invoke* it —
    the tenant directly, a platform approval step, or a signed plugin bundle —
    is a product decision with a privilege-escalation surface behind it.
111. **Do the ceilings need enforcement, or only assertion?** 50 schemes and 60
    fields are declared numbers checked by a job. A tenant hitting the ceiling
    mid-declaration needs a defined behaviour, and "the job complains tomorrow"
    is not one.

Raised by D28 (adopted 2026-08-02):

114. ~~`client_event` retention is now the binding constraint.~~ Settled by D31:
    the volume is unremarkable and the retention is derived. ~~Original:~~ D28 avoided
    7–18M rows/year/site by deleting `scan_ok`, but migration imports still put
    30–60k rows per tenant in on day one, and `client_event` is the only table
    with no range-drop exit. Question 101 is promoted: answer it with the
    partitioning plan, not separately.
115. ~~Retention floors are a class the invariant register cannot check.~~
    Settled by D31: a floor is a declared row with a named authority, and the
    assertion is that live data or the archive manifest reaches it. ~~Original:~~ A
    duplicate-identifier guard needs N months of history; drop a partition and
    the check *silently starts passing*. Fold invariants detect source deletion
    because the projection stops matching; an **existence predicate** has no
    projection to compare against, so after a truncation both sides agree and
    nothing fails. Retention floors must be declared and asserted separately.

*All decisions D1–D46 are now adopted. The proposals in
[mechanism-design.md](./mechanism-design.md), [inbound-analysis.md](./inbound-analysis.md)
and [supply-side-design.md](./supply-side-design.md) are superseded by this
document where they disagree. Their open questions are carried by
[open-questions.md](./open-questions.md) and their invariant tables by
[invariants.md](./invariants.md); both registers are authoritative over the text
left behind in those documents.*

### D108 — A style is what gets measured, and a SKU inherits it

*Adopted 2026-08-13. Migration 73. Raises nothing.*

The real prepack list measures `SKU-0180`. No such item exists. The catalogue
sells `SKU-0180-S`, `-M`, `-L` and `-XL`, and one carton spec covers all four —
58 measured rows standing for 252 sellable codes. `STY-7720` is the same in
thirteen sizes.

*The counts are the real file's; the codes are stand-ins. The originals are a
customer's catalogue and were swept out on 2026-08-31.*

**Writing the style's numbers onto every variant is a lie about how many
measurements exist.** Four rows claiming to be measured when one carton went on
a scale, no way afterwards to tell which, and no way to record that size 13
turned out not to fit. That is the failure this project keeps finding: a fact
copied to where it is convenient, then indistinguishable from a fact observed
there.

So `item_style` is a subject, `observable` gains a fifth arm, and `item.style_id`
is a nullable pointer. An item with no style is the ordinary case.

**Resolution is per fact, not per subject.** D22 resolves policy by taking the
most specific match; this is that rule one level down, applied to each metric
independently. A code weighed here but never measured keeps its style's
dimensions. Resolving by subject would let one weight against the SKU hide three
dimensions the style holds, which is worse than never having recorded the
weight. `GET /items/{id}/measurements` reports `own`, `style` or `mixed` so a
screen can say which numbers were taken against the code in front of it.

**Not `item_class`.** The taxonomy exists, with a closure table and a resolver
over it, and a style is not a class. `item_class` answers *what kind of thing is
this* and feeds D22's policy lattice; a style answers *which codes share a
carton*. Filing 58 styles as classes would put packaging facts in the structure
that decides receiving tolerance and shelf life, and every future binding would
have to be written to avoid them.

**Nothing infers a style from a code prefix.** `SKU-0180-L` looking like a
variant of `SKU-0180` is a convention this database has never been told about,
and asserting it in a migration would make it true by fiat. The loader proposes
and a person decides.

### D132 — A photograph hangs off the look that produced it, and its bytes are addressed rather than stored

*Adopted 2026-08-16, with migration 78.*

**Decision.** A photograph taken while examining an object is another artefact
of that observation, so `observation_image` references the
`observation_event` that already carries the who, the when, the device and the
method. It gets no event of its own. The bytes live behind a content address —
a SHA-256 the row holds and `crate::images` resolves — rather than in the
database.

**Why the event is shared.** One capture session is one look at one box, and the
figures and the pictures both come out of it. Sharing the event makes *"these
photographs and these measurements are of the same object at the same moment by
the same person"* a join rather than an inference. Giving images their own event
would have left two rows with two timestamps and nothing connecting them but
proximity — the shape D10 rejected for movements and D44 recognised again later
in a different place.

**Why the bytes are not in Postgres.** D39: every integration is a capability
with a working default. A filesystem volume needs nothing external, so a
deployment running no object store can still photograph a carton, and an S3 or
R2 implementation is the same seam later rather than a migration.

Content addressing then pays for itself three ways without anything being built
for it. Writing is idempotent, so a retry after a timeout cannot produce a
duplicate and a retake of an unchanged carton costs no storage. The name is the
checksum, so rot and truncation are detectable by reading the file. And deletion
becomes a reference question — bytes are removable when no row addresses them —
which is the form D30's reaper already answers rather than a second mechanism.

The practical argument is smaller and still real: multi-megabyte blobs in a
table are multi-megabyte blobs in the WAL, in every replica and in every dump.
The figures in this database are small and the photographs are not.

**A retake is a new row.** The first draft made it replace its predecessor,
which needs `ON CONFLICT DO UPDATE`, which needs UPDATE on a table granting
INSERT and SELECT — and the grant was right. This is a fact table, facts are
only ever added to, and a blurred first attempt is a thing that happened. Which
picture of a face is current is a fold on `package_event`'s own winning-row
rule, and reaching for the existing rule rather than inventing a second one is
the point.

**What it costs.** A volume to back up separately from the database, and the
window where the two disagree: a restore of one without the other leaves rows
addressing absent files, or files nothing addresses. The read reports the first
as a 404 rather than a 500, because it is a real state and not a fault.

**What it settles.** Every later "where do the bytes go" question — signed
documents, despatch paperwork, a picture attached to a finding. Same seam, same
address, same reference rule.

### D133 — A capture session is one act, and the figures go in one call

*Adopted 2026-08-16. No migration: the write path already permitted this and the
client was about to be built against the other reading.*

**Decision.** Weighing a carton, measuring it and photographing it is one look at
one box, so it is one `observation_event`. The operator's figures — gross weight,
length, width, height — travel in a single `POST /observations` with one
`client_event_id`, and the photographs follow against the `observation_event_id`
that call returns. A capture session sends one write for the numbers and up to
seven for the faces.

**Why this was a decision at all.** D132 shares the event so that *"these
photographs and these measurements are of the same object at the same moment by
the same person"* is a join. Nothing enforced it. `POST /observations` mints an
event per call, so a screen that posted the weight, then the dimensions, then the
pictures would have produced two events with the images hanging off whichever one
happened to be last — D132's join quietly answering the wrong question while
every row involved looked correct. The endpoint takes `[{metric, entered_value,
unit}]` and always did; the failure was available only because nothing had yet
driven it.

**Why not let the client name an existing event.** The alternative was an
`observation_event_id` field on the request, so a session could be opened and
added to. It costs more than it looks. D25 settled the idempotency registry on
exactly this case — *"one physical act produces many facts … a cubing scan writes
an event and four observations"* — and `client_event_id` claims that one act.
The replay branch answers a repeat by returning the facts the act produced, so a
second call attaching to a prior event has to say what a replay of *itself*
means, and one act stops standing for one event. `require_observation_facts` is
written against the rule that it does.

Appending to an act already recorded also asks a fact to grow after it happened,
which Principle 2 does not allow: facts are append-only and never edited. An
event that accumulates over four requests has no single moment it describes, and
`observed_at` would name the first of them while the last is what the operator
would call the time.

**What it costs, and it is real.** The figures are not durable until the one
call. An operator who weighs a carton and loses the handheld before typing the
height has recorded nothing, where three calls would have kept the weight. This
is accepted rather than mitigated: the session is seconds long with the box in
hand, a partial capture is re-enterable by picking the box up again, and the
alternative buys durability with an event that means less. What the interface
must not do is imply otherwise — nothing is confirmed on the screen until the
call returns.

**One event carries one method, and that is the real constraint this creates.**
`observation_event` records how the figures were come by once, for all of them.
A capture session reads a scale and a tape measure, and both are instruments —
`MEASURED_METHODS` classes them together, so one `instrument` covers the four
figures honestly. It stops covering them the moment a session mixes a measured
figure with an asserted one: a weight off the scale beside a height the packer
knows because they cut the box. That is what the pack bench does, and it is why
`api.measure` deliberately sends two events rather than one.

So the rule is narrower than "a session is an event": **where the means
genuinely differ, that is two looks, and two events is the correct record.** The
capture session is one event because its four figures are come by one means, not
because a session is always one event. A screen that acquires a figure some
other way sends it as its own act and says so in `method`.

**A photograph still needs its event first**, which is why the order is scan →
weigh → measure → photograph and not the reverse. This costs nothing: the
operator has the numbers before they have the camera open, and a client holding
seven images in memory waiting for an event id would be holding the one thing on
the handheld that is expensive to hold.

**What it settles.** Every later capture surface — a finding with a picture
attached, a receiving inspection, a despatch check — sends its figures once and
its bytes afterwards. The event is the session, and the session is an act.

### D136 — The locator resolves, and does not yet record that it failed

*Adopted 2026-08-16, with migration 79. Builds D34, which had been adopted and
unimplemented since 2026-08-03.*

**Decision.** `GET /resolve` is D34's resolution function: parse the scanned
string, dispatch a GTIN to `item_barcode` and an SSCC to the package surface and
a raw string to all three, narrow by what the screen expects, and answer with one
of D24's four outcome words. It is a read. **A failed resolution is not
recorded, and that is a deferral rather than an omission.**

**What D111 asks for and does not get.** *"A scan resolving to nothing writes a
resolution-failure record and tells the operator it has."* It tells them. It
writes nothing, because the place to write it is `activity_event` — D28 specifies
the four kinds, the typed columns for the identifier, and a `symbology` reference
table beside it — and neither table exists. That is a range-partitioned fact
table and a reference set, and building it inside the work that built D34 would
have doubled a migration in order to produce a table with no reader yet.

So the screen says what happened and keeps nothing, and says *that* too. The cost
is the one D28 names: the benign population — unresolvable identifiers, at 0.1 to
2% of captures — goes unmeasured until the table lands, so nobody can yet answer
*"which labels are failing, and where."* Nothing else is lost, because D28 is
already emphatic that a decode failure is unobservable and a misread is
indistinguishable from a correct scan: the record was never going to find those.

**A `GET`, and the verb is a claim.** While resolution is a pure read, a `POST`
would assert a write that does not happen. When the record lands, a failed
resolution becomes an act, it takes a `client_event_id` like every other act, and
the verb changes with it. Written down here so the change reads as the decision
arriving rather than as an API break.

**The parser is narrow, and D34 names the one that is not.** D34 says the parser
is the vendor `gs1-syntax-engine`. `crate::barcodes` handles the five
application identifiers a warehouse label actually carries — SSCC, GTIN, batch,
expiry, serial — recognises the variable-measure weights by prefix, and passes
everything else through opaquely rather than rejecting it, which is the behaviour
D34 requires of the real one. What the deviation costs is that an unrecognised
variable-length AI cannot be length-delimited without an FNC1, so it is kept
whole rather than split: an honest partial read instead of a confident wrong one.
Vendoring the engine is a dependency decision and is not made here.

**A correction to D34, found by building it.** D34 names `item_barcode.unit_id`
as the consumer replacing an `unit_level` enum, on the grounds that *"the unit
vocabulary carries packaging levels and measures in one table, as already
recorded."* **It does not.** `unit` holds `ea` in the count dimension and nothing
above it — no inner, no carton, no layer, no pallet — so a carton GTIN cannot say
it is a carton. The column is built as specified and the resolver does not pretend
to read a level out of it: a scan returns the item, and the operator names the
level. Putting packaging levels into `unit` is D23's to decide and is not assumed
here.

**A scan lands on the worklist's own subjects.** This is the part with a trap in
it. A scan resolves to an *item*, and the obvious next move is to open a capture
session against that item at carton level — which walks straight back into the
trip D108 exists to prevent, because for a styled variant the worklist
deliberately offers the *style's* carton instead. Two enumerations would disagree
the first time somebody scanned a variant, and **the scan would win silently**,
writing observations against a subject the worklist would never list. So
`crate::capture` has one classifier and one enumeration, and the resolver returns
`subjects_for_item`, which is the worklist query with a filter on it.

**What it settles.** Every later scanning surface — receiving, picking, counting —
resolves through this one function against these three tables, and gains the
failure record when D28 is built rather than each inventing one.

### D137 — A database says which migrations it has, and the schema goes first

*Adopted 2026-08-16, with migration 80.*

**Decision.** `schema_migration` records every applied migration by directory
name, written only by `scripts/migrate.sh`, which applies what is pending and
nothing else. The deploy runs it to completion **before** the server starts.

**Why a ledger, when eighty migrations managed without one.** It managed because
every deployment was one laptop and one person who remembered. That stopped
being true twice in one session: a stack was rebuilt and restarted against a
database missing migration 78, the server logged a clean start, and the only
reason anyone noticed is that somebody went looking. `/observations/{id}/images`
would have answered 500 behind a health check reading `ok`. The second time the
order was got right deliberately — and *getting it right was a thing a person
had to remember* rather than a thing the deploy could not get wrong.

**Ordering, not assertion.** The obvious fix is a boot-time check: the server
reads the ledger, compares it to the migrations it was built with, and refuses
to start when it is behind. That needs the migration list inside the binary — a
build script, a generated manifest, a second copy of the directory — and it
turns the failure into a server that refuses to serve, which still has to be
noticed.

A migrator that must exit 0 before the server starts removes the state instead
of detecting it. **A failed migration becomes a failed deploy**: `migrate` exits
1, `server` and `scheduler` are never created, and `docker compose up` exits
non-zero, so the thing watching the deploy sees a failure rather than a green
health check over a broken endpoint. Demonstrated with a deliberately broken
migration rather than assumed.

**Why the name and not a number.** The number is a prefix of the directory name,
and the name is what the filesystem orders by — the ordering every script here
already relies on. A separate integer is a second source for the same fact.

**Why not infer the version from the schema.** That is what was done by hand, it
took several rounds, and one of the probes was for a table that has never
existed in any migration — which answers *false* exactly like a missing one, and
produced a confident wrong claim about how far behind the deployment was. A list
of names cannot be wrong in that way.

**What it costs, and this is the interesting half.** The obvious implementation
applies each migration and writes its ledger row in one transaction, so a crash
between them is impossible. **That does not work here.** Six migrations run
`ALTER TYPE … ADD VALUE` and some then *use* the value, which Postgres refuses
inside a transaction — *"New enum values must be committed before they can be
used."* So the choice is made per file: a migration that adds an enum value runs
without a transaction and can leave part of itself behind if it fails, and every
other one is atomic. The migrator says which of the two happened when it fails,
because the recovery differs.

Worth recording how that was found. `CONCURRENTLY` was grepped for first — the
usual reason a migration cannot be wrapped — none was found, and the set was
declared transaction-safe on that basis. It was the wrong check. Running it
found the right one on migration 30.

**`--baseline` is explicit and never inferred.** A database that predates the
ledger is recorded as current by an operator saying so, once. A heuristic that
guessed wrong would mark a pending migration as applied, which is the single lie
this whole mechanism exists to prevent.

**What it settles.** Deploys stop needing somebody who remembers. `compose.deploy.yaml`
is a stack a machine can bring up on its own, which is what makes hosting this
anywhere other than the laptop it was written on possible at all.

### D138 — A measurement names the state the thing was in, and some things have no dimensions

*Adopted 2026-08-17, with migration 81. Raises J72.*

**Decision.** `observation_event` carries a `presentation_id` — the arrangement
the subject was in when it was measured, from a shipped vocabulary of seven
words. And `observation.absent_reason = 'not_applicable'`, which the schema has
held since migration 7 and no write path could reach, becomes an answer a
capture session can give: *this thing has no such measurement.*

**The assumption that broke, and it took two shapes to see it.** An apron is
loose in its carton. Folded twice it is 250×180×30; in a heap it is something
else. A lobby pan set is a pan and a 1200mm handle, and their bounding box is
mostly air arranged however they happened to lie on the bench. Both look like
measurement problems and neither is: the model assumes **an item has
dimensions**, and it does not. **A presentation has dimensions.** A rigid carton
has exactly one, which is why nobody noticed — the numbers reproduced because
there was never a choice to record.

**On the event rather than as a metric, and the reason is D108's.** The cheaper
build is a code-valued `metric`: the machinery exists, the vocabulary stays
data, no fact table changes shape. It fails on resolution. D108 resolves **per
fact**, independently per metric, so a presentation metric lands its own row in
`observation_current` and a reader joining it to current length describes
dimensions with an arrangement they were never taken under. The correct read —
reach the presentation through the winning length's event — and the wrong one
are a single join each and look identical on the page. That is a trap laid for
code nobody has written yet, and D108's own trap was closed structurally rather
than with a note.

`observation_event.observable_id` means one event is about one subject, so **one
presentation per event is not a convention but a restatement of what the table
already says.** On the event, the wrong read cannot be written. It also lands in
the right company: `method`, `ingestion_channel`, `device_id` and
`asserted_by_party_id` all describe the act rather than any one result, and what
state the thing was in is the fifth of that kind.

**This does not split the event, and D133 is not being contradicted.** D133 says
that where the *means* genuinely differ that is two looks and two events.
`method` is how the figures were come by; presentation is what was in front of
the person. A photograph does not get its own event under D132 for the same
reason, and this is that argument about a different artefact of the same look.

**The rule with teeth: a length at `each` requires it.** A carton is rigid and
asking for the word would be ceremony, so the requirement is narrow — three
metrics, one packaging level — and it is checked before the event row is
written, because a rejection arriving after four observations have gone in is a
rejection that has already half-happened. J72 asserts the same thing about the
data, because a rule that lives only in the writer is a rule the second writer
breaks, and the prepack loader is already a second writer.

**An absence is an answer, and the projection had nowhere to put it.** The
rebuild already lets an absence win — it filters retractions and corrections,
not absences — but `observation_current` had no `absent_reason`, so a declared
*not applicable* arrived as NULLs and was indistinguishable from nobody having
looked. That is the difference a capture worklist exists to act on, so the
column is the whole of the fix. A declared absence supersedes an earlier
measurement, because the winner is the most recent eligible row and somebody who
has looked at the thing knows more than the reading that preceded them.

**A declared absence does not age.** Revalidation exists because a figure drifts
from the thing it describes; *this set has no bounding box* is not a reading that
goes stale and there is nothing to put back on a scale. So absences are excluded
from the worklist's staleness and method aggregation — without that, a set whose
weight came off a scale and whose dimensions were declared absent would resolve
its newest method to the `keyed` act that declared them and sit on `unconfirmed`
for ever — and a subject whose every figure is an absence is settled.

**Only two of the four absence words are writable.** `not_measured` is what a
subject with no row already says, and recording it makes silence
indistinguishable from an answer, which is the confusion this decision exists to
end. `retracted` belongs to `retracts_observation_id`, and a second way to spell
a retraction is a second answer to which rows count. That leaves
`not_applicable` and `unreadable`.

**What it costs.** One more thing to ask the operator at `each`, and a
vocabulary that is ours rather than the tenant's — a warehouse needing a word
this list cannot express is the trigger for the tenant column, and it is not paid
for before then. The seven words are a guess at the world made from two products.

**What it settles.** Every later question of the form *these two measurements of
the same thing disagree*. They now disagree with a reason attached, or they were
taken the same way and the disagreement is real.

### D139 — A product's parts are what get measured when the product has no shape

*Adopted 2026-08-17, with migration 82. The sixth `observable` arm.*

**Decision.** `item_part` is a physical constituent of a sellable item that is
measured on its own, and `observable` gains an arm for it. The pan and the
handle carry the sizes; the set carries `not_applicable`.

**Why this is not solved by D138 alone.** D138 gives the each an honest way to
say it has no bounding box. It does not give the sizes anywhere to live, and
every measurable subject in this database was an item, a style, a package, a
lot, a location or a unit somebody asserted. A handle is none of those.

**The registry is exactly what this is for.** D23 put the subject union on
`observable` rather than on the fact tables so that widening it is one column on
a table of about 10⁵ rows and no change to anything holding 10⁷, and said the
remaining arms would each arrive with the migration that creates their target.
D108 was the first. This is the second, and it is the same shape for the same
reason: **a style is measured and not sold, and a part is measured and not
sold.**

**Why a part is not an item, stated because it departs from how the case was
described.** These were called two separate items, and they are two separate
*things*. An `item` is stocked, counted, allocated, picked and ordered, and
minting two of them for one product creates stock cells nobody counts, catalogue
codes that cannot be sold, and a question with no good answer: does receiving ten
sets put ten sets on hand, or ten pans and ten handles? *A measurable
constituent of a sellable thing* is a smaller claim and it is the one that is
true today.

**The promotion trigger, so nobody has to guess it.** The day a part is stocked,
picked or sold on its own — a replacement handle going out alone, a pan counted
separately in a stocktake — it has stopped being a part and become an item, and
this table stops being its home. That is a migration, and it should be, because
it is a change in what the thing is. The same event is the trigger for parts
having packaging levels: handles arriving in a carton of handles is a carton of
an item.

**No packaging level.** `observable`'s item arm carries one because a carton of
boots and a pair of boots are different subjects. There is one handle. The
existing `observable_item_level_ck` ties the level to the item and style arms
and therefore already says the right thing about an arm written after it, which
is the second time that constraint has been correct about a future it did not
know.

**Identical parts are one row.** A set with two matching brackets is one
`item_part` with `quantity_per = 2`, not two rows: they are the same object
measured once, and two rows would claim two measurements where one bracket went
on the scale — D108's founding error one level down.

**Nothing creates a part, and that is the same answer the catalogue already
gives.** There is no `POST /item_parts`, because there is no `POST /items`
either — an item arrives through the prepack import or through `psql`, and a
part arrives the same way, beside the item it belongs to. Building a write path
for parts before there is one for items would be building the second half of a
door.

The trigger for an editing path is the first person who is not me needing to
declare a part, which is the same trigger the catalogue has been waiting on
since migration 1. Recorded rather than left silent, because *how does one of
these come to exist* is the question a decision that introduces a table owes an
answer to, and the answer here is a real one rather than an omission.

**The worklist lists parts and does not suppress their parent.** An item with
parts still offers its `each`, because a two-part thing *could* have a
meaningful assembled box and inferring otherwise would decide by fiat what the
operator is there to determine. What the screen gets is the part count, so it
can say *measure the parts, and answer not applicable here* — the loader
proposes and a person decides, which is D108's rule about styles applied to the
same kind of guess.

**What is deliberately not built.** Nothing arranges parts into a box. That is
cartonisation, it has no caller, and building a nesting model against no caller
is what D67 declined for partitioning on the same ground. What this provides is
the input: real sizes for the things that actually go in the carton, and a
parent that says honestly it is not one of them.

### D140 — Evidence is a look, and a record points at it

*Adopted 2026-08-17, with migration 83.*

**Decision.** `evidence` links a record to the `observation_event` that supports
it. Findings, receipt lines, counts, adjustments and despatch events can each
carry evidence; none of them is obliged to. `POST /evidence` mints the look and
the link in one act and answers with the event id the photographs then hang off.

**There is no new kind of thing here.** An `observation_event` already records
who looked, when, at which subject, by what means and on whose authority — which
is the whole content of *supporting evidence*. `observation_image` already holds
the bytes behind a content address. So this needed no table for the picture, the
provenance or the moment, and building one would have given the system two
places a photograph can live and no rule about which.

What was missing was one sentence the schema could not say: **this look is
offered in support of that record.**

**Why the arms are the licensed kind.** D23 draws the line — typed nullable FKs
with a mutual-exclusion CHECK are right when the arms are *alternative
identities of one referent*, and wrong when they are *distinct relationships
that merely happen to be exclusive today*, because there exclusivity is a policy
and policies turn out to be wrong. One evidence row is about one record; that is
not a claim about the world, it is what the row means. And **a look that
supports two records is two rows**, which is the escape `discrepancy`'s source
arms never had and the reason they degenerated into a subject union (question
112). A photograph of a crushed carton really is evidence for both the receipt
line and the finding it raised, and it says so twice.

**Optional everywhere, required nowhere.** Nothing gained a NOT NULL, no
existing write path changed, and a record with no evidence is the ordinary case.
The sixth record type is one column and one line in the writer, which is why the
arms live on this table rather than a nullable `evidence_id` being sprinkled
across five record tables.

**Why not `POST /observations` with no measurements.** The obvious build relaxes
the observation writer to accept an empty `measurements` array. It breaks
idempotency in a way that loses data: an accidentally empty request would
**claim the `client_event_id`**, and the client's retry carrying the real
figures would hit D25's replay branch and be handed back the empty act's
nothing. *"An observation of nothing is not an observation"* is protecting that
rather than being tidy. So a look that produces a picture instead of a figure is
its own act with its own route — the shape `POST /weighings` already has over
the same two tables.

**The look names the subject; the link makes it evidence.** A finding carries an
item, a location and a package without those being exclusive, so which one the
operator held up to the camera is theirs to say and nothing infers it from the
record.

**A new method, because none of the seven fitted.** `observation_method`
describes how a figure was arrived at. Calling a camera an `instrument` would
have a consequence rather than being merely loose: `MEASURED_METHODS` treats
`instrument` as confirmed, and revalidation would eventually read a photograph
as evidence that something had been weighed. So `photographed`, and the
migration is therefore one that cannot run in a transaction — which is the first
time `scripts/migrate.sh`'s enum detection has mattered outside its own test.

**No invariant, and that is a decision rather than an omission.** The obvious
one — every `evidence` row's look carries at least one photograph — is false by
construction: the act and the picture are two calls, and the window between them
is a legitimate state rather than a defect. Nor is there anything to assert
about which subject was photographed, because the whole point is that only the
person holding the camera knows. The register stays where it is.

**What it costs.** A link table that grows with the record types, and no way to
unlink: `evidence` grants INSERT and SELECT and nothing else, on Principle 2's
terms, so a mis-attached picture is answered by attaching the right one. If
mis-attachment turns out to be common rather than hypothetical, the answer is a
retraction row naming what it retracts, not a DELETE grant.

### D141 — A picture inherits for recognition and never for evidence

*Adopted 2026-08-17. No migration: this is a read.*

**Decision.** `crate::pictures` resolves the front-face photograph for an item —
the item's own looks first, then its style's — and **the answer says whose
picture it is**. `crate::capture`'s rule that photographs do not inherit stays
exactly as written.

**Two reads, two questions.** The capture worklist asks *has anyone looked at
this subject*, and inheritance would answer it falsely: showing a style's
picture against a variant claims a photograph of this carton that is a
photograph of a different one. A picker asks *what am I looking for on this
shelf*, and there the style's carton is a perfectly good answer for thirteen
sizes of one boot — refusing it leaves somebody hunting a bay with nothing to go
on.

So the same bytes resolve differently, and what keeps that honest is D108's
rule one more time: **`own` or `style`, on every answer, and the screen draws
the label.** An inherited picture shown unlabelled is the pixel version of a
number nobody took against this code reported as though somebody had.

**Front, and nothing else.** Seven faces are recorded and one is offered. A
`label` is a barcode close-up and a `bottom` is a box lid; shown under the
heading *what to look for*, either misleads worse than showing nothing, because
the picker will believe it. If the front has never been photographed the answer
is nothing and the screen draws the absence.

**Newest wins**, which is D132's rule for which picture of a face is current.

**Parts are out of the chain.** The pan's photograph standing for the set is
arguable and it is scope; the trigger is somebody wanting it.

**A missing file is a state, not a fault.** D132 keeps the bytes on a volume and
the rows in the database and says what that costs — a restore of one without the
other leaves rows addressing absent files, and the read answers 404. The screen
agrees: `Photo` draws the absence rather than a browser's broken-image glyph,
because at arm's length that reads as the software being broken rather than a
picture being gone.

**And evidence is filed as `detail`, never as one of the seven sides.**
Migration 84, and it is this decision that forced it: the recognition picture
resolves from `front`, so a damage photograph filed there would become what the
next person sent to that bay is shown as *what to look for*. A fact put where it
is convenient and then indistinguishable from a fact of a different kind — the
shape this project keeps finding, caught this time before it shipped.

**Where it shows.** The pick walk, `/fixtures/picking`, which is the screen that did
not exist. It is a read: `crate::picking_list` declines to record the pick,
because which carton a pick goes into — the despatch carton, a tote, a whole
wave into one cage — is a workflow question the model does not answer for
somebody else's floor, and inventing one here would be wrong on a real one. The
list says what is outstanding, which bin to walk to, in what order to walk, and
what the thing looks like when you get there.

**And the order is the floor's.** Rows sort by `location.pick_sequence`, so the
list is a route rather than a set — the first thing to read that column, which
until now only J71 had an opinion about.

### D142 — The first administrator is made once, and a token says who may

*Adopted 2026-08-17.*

**Decision.** A deployment with no person in it exposes `POST /setup`, which
creates a tenant, a site, a person, their credential and the membership tying
them together — once. It is gated on two things: zero persons anywhere, checked
inside the transaction that does the inserts, and a token minted at boot into a
state directory and printed to the log.

**The problem.** Migrations build a schema and nothing else, so a fresh
deployment has nobody to sign in as. Until this, the only ways in were loading
`fixtures/seed.sql` — whose password is committed in this repository — or
hand-writing rows with the `hash_password` example. Neither is a way to hand
somebody a deployment, and the first was used on this project's own.

**This is not an exception to D11, and the first draft of this decision said it
was.** D11 governs attribution on the ledger: `stock_movement.recorded_by_id`,
never null and never editable. Setup writes no facts about goods. Sign-on is
what *establishes* attribution and therefore cannot require it; the same is true
one step earlier. There is nothing being excepted.

**Nor does it widen the privilege boundary.** `nylonite_app` holds SELECT on
`tenant` and `person` and nothing at all on `person_credential` — the
tenant-scoped role may not mint identities, deliberately. But identity acts
already run on a raw pooled connection as the owner: that is how `sign_on`
writes a session and how passkey enrolment writes a credential, and
`credential_for_login` is a `SECURITY DEFINER` function for the same reason.
Setup joins an established category rather than inventing one.

**Why a token, when the database already says whether setup is needed.** Because
"no persons yet" is a race with strangers rather than a lock. Between a
deployment going live and an administrator existing, an endpoint gated only on
emptiness belongs to whoever finds it first — and that window was open on this
project's deployment for hours. It is the shape of CVE-2024-31218, the
PocketBase installer race, and the answer is theirs: a token only somebody who
can read the server's log or its filesystem has.

The token lives in `NYLONITE_STATE_DIR`, **not beside the images**. The image
directory holds bytes a stranger uploaded, and a path-traversal bug in that
handler must not also be a way to read a credential. Thirty minutes, mode 0600,
reused rather than rotated while somebody is halfway through the form, deleted
when setup succeeds, and deleted at boot if a person exists — that last case
being a backup restored onto a box that had been waiting, which would otherwise
leave a live credential on disk.

**The lock, the count and the inserts are one transaction**, in that order.
`pg_advisory_xact_lock` is held until the transaction ends, so a second request
waits, then sees the first one's person, and is refused. Demonstrated rather
than asserted: with the lock, two concurrent setups give one 200 and one 400;
with the lock removed, they give one 200 and one 500, because the loser reaches
a unique constraint instead of a refusal. `scripts/migrate.sh` shipped a lock
that did not lock because nothing ran two of anything at it; this one is
checked.

**Once per deployment, and not once per tenant.** The predicate is zero persons
*anywhere*. Adding a second tenant to a running deployment is provisioning, it
answers to D19 and D41, and it is deliberately not this.

**What it does not do.** Restore a backup. An unauthenticated endpoint accepting
a SQL dump on a box reachable from the internet is a pre-authentication
execution surface, and the value of it is a `psql` command somebody runs once;
`docs/deployment.md` carries that and the setup screen points at it. There is
also no hosted/self-hosted split of the kind a multi-deployment product needs,
because there is one deployment mode here — noted so the omission reads as a
decision.

**Was owed, and D143 is it.** This decision shipped with no change-password
path at all, which meant the password chosen at setup was the password forever.
D143 builds it.

### D143 — A password is changed by the person who knows it, and it takes the other sessions with it

*Adopted 2026-08-18.*

**Decision.** `POST /credentials/password` changes the caller's own password. It
requires the current one, refuses a new one under twelve characters or identical
to the old, and revokes every other live session for that person while keeping
the one that made the request. The write is `credential_change_password`, a
`SECURITY DEFINER` **compare-and-set** that only writes over the digest it is
handed.

**The problem.** D142 shipped the way into a deployment and named this as what
it owed. `nylonite_app` holds no grant on `person_credential` at all — a
deliberate boundary, and it meant nobody could change a password through
anything. On this project's own deployment that was not an inconvenience:
`app.nylonite.com` ran on `fixtures/seed.sql`, whose password is committed in
this repository, reachable from the internet, with no way to rotate it short of
`psql`.

**The current password is required, and a session is not enough.** The obvious
objection is that the caller is already signed in, so what does knowing the old
password add. It adds the difference between a stolen session and a stolen
account. A session expires, can be revoked, and dies with the browser; a
password does not. Without the check, thirty seconds at an unlocked terminal
converts a session that would have lapsed by evening into permanent access the
real owner cannot take back. OWASP asks for it for exactly that reason.

**And a wrong one is counted**, on the same per-account counter `sign_on` uses,
with a locked account refused before the digest is compared. Guessing the
current password from inside a stolen session is the attack this endpoint
creates; an endpoint that let it run unmetered would be a way *around* the
lockout rather than a second door with its own lock. Unlike sign-on the refusal
is specific — *"that is not the current password"* — because the caller has
already proved they are this person and there is nothing left to leak. Sign-on's
one generic message exists to avoid an oracle for whether somebody works here,
and that oracle is not on this side of the door.

**Compare-and-set rather than a setter**, which is the part worth arguing for.
`credential_change_password(person, expected_phc, new_phc)` updates only
`WHERE person_id = $1 AND phc = $2`, and that one clause does three jobs:

* it closes the read-verify-write race, so two changes racing cannot have the
  loser overwrite the winner with a password derived from a digest that is no
  longer there;
* it refuses a passkey-only row for free — migration 75 made `phc` nullable, and
  `NULL = anything` is never true, so a person with no password cannot have one
  set by a path claiming to *change* one. Structural rather than a branch
  somebody has to remember;
* it narrows what the application can do at all. A bare
  `credential_set_password(person, phc)` would hand `nylonite_app` the ability to
  write anybody's password. This hands it the ability to write a row whose
  current digest it has already been given, which is migration 70's mediation
  argument rather than a restatement of the check above it.

The refusal for a passkey-only person is still spelled out in the application,
because a structural refusal alone would reach the caller as *"that is not the
current password"* — true of the mechanism and a lie about the situation.

**Every other session goes.** The case that makes change-password worth having
is somebody who thinks a session was stolen, and a change that leaves the thief
signed in has not helped them. The current session survives, identified by its
own digest: signing somebody out of the screen they just used is how people
learn not to use it. The count comes back and the screen prints it, because
nought and three are different facts and *"other sessions have been ended"*
tells the operator neither.

**One password policy, not two.** Setup had its own twelve-character check and
its own message saying *"a first password"*. The moment this existed that was
two policies, one of which would drift, and a message that read wrong on the
other path. `credentials::check_password` is now the only one, and it counts
characters rather than bytes — `.len()` would accept four emoji and refuse a
legitimate password in Greek.

**Not administration.** The person comes from the resolved session and no shape
of this request takes one in the body, so there is no way to change somebody
else's. Resetting another person's password needs the role vocabulary question
176 carries, and is deliberately not this.

**What the tests are for.** The refusals, not the success: a wrong current
password refused *and* the counter advanced; a short new one refused with the
line named; an unchanged one refused rather than silently re-salted; a
passkey-only person told why; the app still unable to `UPDATE person_credential`
directly. Each test makes its own person — the first draft borrowed
`kyle@example.test` and finished with a global `DELETE FROM session`, and three
of six failed, because `cargo test` runs them at once and they were changing one
account's password out from under each other. A test whose subject is a
credential cannot borrow the credential the rest of the suite signs on with.

### D145 — Where you are working is a question the session answers, and it is asked after signing in

*Adopted 2026-08-18.*

**Decision.** A session names a site. `GET /sites` lists the warehouses a caller
could work at, and `POST /sessions/site` reissues their session against the one
they choose. A tenant with one site has it chosen silently. The question is
asked **after** authentication, on an ordinary session, not as part of signing
in.

**The problem, and it had been true since the first browser session.** The
sign-in page posts an email and a password and nothing else. `SignOnRequest` has
always accepted `site_id`, `Caller::site_id` has always carried it, and no form
ever sent one — so **every browser session in this system has had
`site_id = NULL`.**

The packing worklist filters `AND ($1::uuid IS NULL OR f.site_id = $1)` against
a doc comment claiming it is *"scoped to their site rather than to the tenant"*.
With a null site that filter disables itself. It has been latent rather than
active — the query runs inside `TenantScope` and the seed gives each tenant one
warehouse, so row-level security already produced the right list — and it
becomes a live correctness bug the moment any tenant has a second building.

What it blocked immediately was different. D112 defines every badge, and the
landing screen the interface is being built towards, as *"work waiting for you,
**at this site**, now"*. There was no site to name.

**Why this is not part of signing in, which is the load-bearing argument.** The
first draft of this plan concluded that sign-in had to be migrated first, ahead
of the recorded order. It does not. `client_events` copies `who.site_id` into
each act at write time and nothing ever re-reads the session's copy, so a site
chosen a moment *after* authenticating is exactly as sound for D11's
non-repudiable floor as one chosen during it. Separating the two questions is
what lets the sign-in page — which runs before a session exists and carries the
passkey ceremony — stay last in the migration while the thing it was blocking
ships now.

**A new session rather than an UPDATE.** `session.site_id` is documented as
"where they signed on". A row mutated under a live token would make one session
name two places over its life with nothing in the record saying when it changed.
So the old session is revoked and a new one opened through `issue_session`,
which exists precisely so there is one way to become signed in: *"a second path
that issued its own session would be a second place for the tenant check to be
forgotten."* The new session is minted **before** the old is revoked, so a
failure leaves the caller signed in rather than signed out by a question.

**Another tenant's warehouse is refused twice over.** `session_site_fk` is
`(site_id, tenant_id) REFERENCES site(id, tenant_id)`, so the database refuses
it as a constraint. The handler checks anyway, inside a `TenantScope`, because
the difference is a sentence the operator can read against a 500. `GET /sites`
reads inside a scope for the same reason `current_session` learned to: *"reading
it on a bare connection returns nothing and the answer silently loses the site
it signed on at."*

**One site is not a question.** A deployment with one warehouse per tenant — the
seed, and every deployment today — settles silently and never draws the screen.
A question with one possible answer is a screen that exists to be dismissed.

**Attached to nowhere is a real state**, distinct from loading. `site_id` is
nullable and a person can belong to a tenant with no site they may work at. The
screen says so and offers no retry, because nothing the operator does from there
changes it.

**What this replaced.** Every shell was drawing `site="MEL" who="d.stooke"` — a
literal, and an operator who is not in the seed — while every live screen
fetched one hardcoded site's data regardless of who signed in.
`GET /sessions/current` existed the whole time and nothing called it. The shells
still take `site` and `who` as props and stay presentational; a provider reads
the session and passes them down, so a fixture with no network still draws.

**The 401 policy is one seam, not nine.** `send()` notifies a handler the
session provider registers; the alternative is every hook testing
`error.status === 401` for itself, which is nine copies of one rule free to
disagree.

**And the maud sign-in's tenant choice, which was a bug rather than a gap.** The
server has always answered 409 with the list of tenants when a person belongs to
more than one; the page printed the error text and offered no picker, so such a
person could not sign in at all. It renders the list now.

**Verified against a running server rather than a gate.** The render gate serves
a static bundle with no API behind it and has therefore never loaded a live
screen — which is exactly how a made-up operator's name sat on every screen for
weeks. Signing in for real and reading the chrome back is what proves this one:
`MEL` and `KYLE PHILLIPS`, from the server.

### D154 — A shelf is a thing you measure, and there is no bay configuration screen

*Adopted 2026-08-19. Decided ahead of the build; nothing below is written yet.*

**Decision.** A `location` is an observable. Measuring a rack bay — its clear
height, its width, its depth — is the same act as measuring a carton: the same
screen, the same metrics, the same centimetres (D153), and the same provenance.
`metric.applies_to` widens to include `location` for `length`, `width` and
`height`.

**There is deliberately no screen for configuring a bay.** That is the whole
point. The brief this answers was *"intuitive based on idiomatic and pragmatic
defaults, but complex workflows should be possible through rich data handling
without complex menus and bloat"* — and a settings page for rack dimensions is
precisely the bloat. You do not configure a shelf. You measure one, with the
machinery that already exists for measuring things, and everything downstream
gets better because the data got richer.

**The schema was built for this and nobody noticed.** `observable` carries a
`location_id` with a unique index on it, and `temperature` already applies to
`location` — so the path is proven rather than theoretical. Only the three
length metrics are scoped to items today, and `applies_to` is an array, so this
is a data change rather than a structural one.

**What it buys, beyond the shelf.** A measurement of a bay carries who took it,
with what instrument, and when — because that is what an observation is. A
configuration screen would have given a number with no author. When somebody
asks why a pallet was refused from a bay, the answer is a measurement with a
name on it, which is the same standard the rest of this system holds itself to.

### D155 — Reach is derived, and says how it knows

*Adopted 2026-08-19. Decided ahead of the build.*

**Decision.** Whether a bin can be picked by hand or needs a forklift is
computed, never stored as a flag, and the answer carries its own provenance:

* **assumed** — nothing is measured, so the lowest level in the bay is
  hand-reachable and everything above it is not. True for all 2,193 Melbourne
  bins today.
* **measured** — a beam height is on file, so reach is `z_mm` against the site's
  reach height.
* **stated** — an explicit override, because a real warehouse always has one.

**This is D108's shape, applied to a different question.** That decision made a
measurement say whether it was `own`, `style` or `mixed`, on the grounds that a
screen which cannot tell them apart reports a number nobody took as though
somebody had. The same argument holds here: *ground level (assumed)* and *0.2 m
(measured)* are different claims and must not be drawn the same way.

**It is what lets the default be pragmatic without being a lie.** Assuming only
the first level is hand-pickable is right often enough to be useful and wrong
often enough to matter — in aisles A and B there are six levels, in C to G there
are four, and the operator said plainly that A–D vary. An assumption that admits
it is an assumption can be acted on; one that presents itself as fact cannot.

**It replaces a worse idea.** The first proposal was a per-aisle override table
for *which level counts as the floor*, which was machinery built around parsing
the last digits of a bin code. With a real `z_mm`, floor level is `z_mm = 0` and
the exception in A–D stops being a special case and becomes a measurement.

### D156 — The racking is described once per aisle, and that description is the map

*Adopted 2026-08-19. Decided ahead of the build.*

**Decision.** Rack geometry is a **profile per aisle** — levels, beam heights,
bay width and depth, load limit — from which every bin in that aisle derives its
`x_mm`, `y_mm`, `z_mm`, `length_mm`, `width_mm`, `height_mm` and `max_weight_g`.
Roughly fifteen profiles stand in for 2,193 rows.

**Because nobody is going to measure two thousand shelves**, and if the answer
requires that, the columns stay empty — which is what they are today, every one
of them, on every bin. A profile is a morning with a tape measure and it can be
re-measured when a bay is re-beamed, which a per-bin table cannot.

**The load limits come from the rack's own notice plate**, which Australian
racking is required to carry, rather than from anybody's estimate. That is the
authoritative source and it is already screwed to the frame.

**One artefact, four systems.** This is the decision's real weight: the same
profile feeds capacity and pallet-height limits, D155's reach, the pick-route
optimisation that today can only replay the `pick_sequence` NetSuite exported,
and the 3D warehouse map — which cannot exist at all without `x/y/z` on every
bin. `pick_sequence` is a route somebody *defined*; geometry is what lets one be
*computed*, and the two can then be compared, which is the more interesting
question.

**It also closes the loop on the measuring.** Eight thousand nine hundred and
fifty-eight items have no dimensions on file, and the reason to walk the
warehouse recording them is not freight rates alone — it is *what fits where*,
which is unanswerable until the shelves are measured too.

**Not built, and deliberately not blocking.** Capture works without any of it.
But the profiles are worth taking while somebody is standing in every aisle with
a tape anyway, rather than as a second walk later.

### D157 — Authorisation waits for its third case

*Adopted 2026-08-19.*

**Decision.** Changing a bay's measurements is **recorded, not gated**. Who
measured what, with which instrument and when, is already guaranteed by D11.
There is no permission check, and `person_tenant.role` remains the undefined
text column D19 left it as.

**The request was for a permission** — *"unless an operator with permission to
the bay configuration applies the actual height"* — and this declines to build
one yet, which is worth stating plainly rather than quietly not doing.

**Because a permission system built for one flag is the bloat the same brief
argues against.** Question 176 has held the role vocabulary open since the
authentication work, on the grounds that inventing one is a business decision
about who may do what, and D22 refuses undesigned languages everywhere else in
this system. One case is not enough to design a vocabulary against; it is enough
to design a *guess* against, and the guess would then be load-bearing.

**What makes deferring safe here is attribution.** The act has a name on it and
the ledger is append-only, so a wrong measurement is visible, attributable and
correctable. That is a weaker guarantee than prevention and a stronger one than
a role column nobody has defined the meaning of.

**When the third case arrives** — and adjusting stock and voiding a consignment
are both already candidates — the vocabulary gets designed once, against three
real cases, and this is one of them.

### D158 — A loader carries a token of its own, and its kind is its scope

*Adopted 2026-08-19, with migration 88.*

**Decision.** Reference data is imported over HTTPS by a program holding an
**import token**: a bearer credential belonging to no session, minted by a
person, prefixed `nyl_`, and accepted by the import endpoints and nowhere else.
A session is refused there. The endpoints are dry run by default and write only
when asked.

**A session token would have worked, and that is the trap.** Bearer tokens
already exist — `Authorization: Bearer` and the `__Host-` cookie name the same
`session` row, which is D5's handheld story — so the shortest path was to sign
in and hand the loader that. Three things are wrong with it, and all three are
about a credential outliving the act it was issued for.

A session is eight hours absolute and thirty minutes idle. An import is not a
sitting: `reported_stock` is replace-on-reload by construction, so loading is
something that happens again next month. A credential that must be re-minted by
typing a password is one that ends up in a script.

A session also carries a person's entire reach. Handing a loader Kyle's session
hands it every endpoint Kyle has, and withdrawing it signs Kyle out of the
floor. One credential doing two jobs cannot be withdrawn from one of them.

And a session names a person, which would put somebody's name on acts they did
not perform. `api_token.created_by_id` says who is answerable for the token
existing — a different and honest claim — and the import path writes `site`,
`location`, `item` and `reported_stock`, none of which carries
`recorded_by_id`. D11's floor is not weakened because it was never reached.

**Scope without inventing authorisation, which D157 has just declined to do.**
`auth.rs` states the absence plainly: no authorisation, and `person_tenant.role`
is an undefined text column that question 176 is holding. A token scoped by
permissions would be that vocabulary, invented in passing, for one caller.

So this is not scoped by permission. It is scoped by **kind**, which is the
shape D142 already built: the setup token authorises one endpoint and needs no
notion of what a role may do. There are now three credential kinds and each one
*is* its scope — a session reaches everything its person reaches, a setup token
reaches `POST /setup`, an import token reaches the import endpoints. That is a
capability, and a capability answers "what may this do" by being the answer.

This is why the endpoint refuses a session rather than accepting either. A kind
that a session also satisfies is a suggestion; the refusal is what makes the
sentence true, and it has its own assertion in `import_over_http`.

**The prefix is not decoration.** Both kinds arrive in the same header, so
without a mark the resolver tries one definer and then the other — two round
trips for whichever kind loses the coin toss, and behaviour that depends on the
order somebody wrote the branches in. It also makes a leaked token
*identifiable*: `nyl_` and 64 hex characters is a string a secret scanner can be
taught, and a bare 64-hex string is not — it is indistinguishable from a digest
or a commit id.

**The dry run is the real write, rolled back.** The CLI importers are dry run by
default and an HTTP import that applied by default would be the same importer
with the safety removed. But a dry run that *estimates* is a second
implementation of the write, and the numbers worth having — new bins against
corrected ones — cannot be known without asking the database anyway. So
`importing::load` always performs the writes and rolls back to a savepoint when
it was not told to apply. The report cannot disagree with what applying does,
because it is what applying does.

**The file is the interface.** The body is the CSV the system of record exports,
unaltered. Re-shaping it into JSON first puts a translation between NetSuite and
this, and a translation is a place for bugs neither side can see — while the
importer already knows how to read the export.

**The table is unreachable, and that is now checked rather than intended.**
`nylonite_app` holds no privilege on `api_token`; the `SECURITY DEFINER`
functions are the only interface, which is migration 70's arrangement. J73,
added the same day, asks Postgres whether the application can reach any table
carrying a `tenant_id` and exempts only the ones it cannot — so the day somebody
grants the application a column of this table, the check fires.

**What this does not do.** It does not scope a token to one *import* — a bin
token can load items. Doing that needs a vocabulary of what the endpoints are,
which is the same design D157 is holding for its third case. And it does not
build a screen: tokens are minted, listed and withdrawn over the API, because
the person who needs one today is the one writing the loader.

### D160 — A gerund decides, a noun reads, and the handler persists

*Adopted 2026-08-20. A convention already governing eighteen modules, written
down rather than invented.*

**Decision.** `crates/server/src` has three kinds of module and the name says
which:

- **Gerunds** — `receiving`, `observing`, `allocating`, `despatching`,
  `adjusting`, `picking`, `counting`, `moving`, `correction`, `importing`.
  The *judgement* a write makes, and **pure**: no connection, no transaction,
  no SQL. Tested without a database.
- **Nouns** — `bench`, `despatch`, `capture`, `locator`, `packing`,
  `picking_list`, `work`. A *read*, whole: the query and the shaping together,
  one vertical slice ending in a struct that goes on the wire.
- **`routes.rs`** — transport, and the persistence for every write.

**Why the split falls where it does.** The difficulty in a write is the
decision — which policy applies, what a shortfall means, whether two claims can
both be honoured — and that is exactly the part a database makes expensive to
test. Separating it buys unit tests over the hard half and leaves the easy half
in the handler. `receiving::disposition` is eleven lines of judgement with a
table test beside it; the eight hundred lines of SQL that surround it in
`record_receipt` need a fixture and a transaction and prove much less.

A read has no such seam. Its difficulty *is* the query, so splitting it would
put a function on one side and its only caller on the other. `bench.rs` owns
`cartons_on` and its SQL, and both the endpoint and — until it was retired —
the maud page called it. One definition, two callers, no second copy free to
disagree.

**The stated exception.** `receiving` reads policy (`receiving_policy`,
`policy_binding`) because resolving which policy applies *is* the judgement and
it lives in the database. Noted here so it reads as a decision rather than as
the rule breaking down.

**What this does not license.** `routes.rs` is 8,321 lines, 63 handlers, 161
structs, and 24 functions over a hundred lines — `record_receipt` is 470. That
is not this decision working; it is this decision never having been told where
to stop. The split is right and the accumulation is not, and nothing in the
register said so until now.

**When a write path is next opened**, its persistence moves out beside its
gerund module and its DTOs go with it — one path at a time, on the migration
doctrine D113 already uses for screens, not a rewrite. Two things move in the
same commit or not at all: the tests, and the fact that `check-contract.mjs`
must still see the moved structs. That gate read a hardcoded list of modules
until D160's own review found `tokens.rs` and `importing.rs` outside it, and
six wire types unchecked while it reported agreement.

### D163 — Liveness is a different question from readiness, and Docker only asks one of them

*Adopted 2026-08-24, with migration 89. Ported in shape from Nosdesk and in
reasoning from neither.*

**Decision.** `/api/live` says the process is up and does no I/O. `/api/health`
and `/api/readiness` are the same deep check — a usable connection that is not a
superuser — and report, to a caller who is signed in, the build the binary was
made from, the head of the migration ledger, and how the projections are doing.
Only the database check gates. SIGTERM flips readiness to `draining` before the
process stops accepting.

**Why this was worth a decision rather than a handler.** `/health` already
existed and already did the deep check, so the tempting change was to make it
cheap and add a second endpoint for the rest — which is Kubernetes convention
and would have been wrong here.

**Docker does not restart a container for being unhealthy.** Tested rather than
recalled: a container with `--restart unless-stopped` and a healthcheck that
exits 1 sits `unhealthy` with `RestartCount=0` indefinitely. Under Kubernetes a
failing liveness probe kills the pod, which is why liveness there must be cheap
and never touch a database. Here the same inversion costs something else
entirely — `depends_on: condition: service_healthy` gates on that signal, and
D137's whole argument is that a dependent must not start until the thing it
depends on can actually serve. **A cheap healthcheck would have marked the
server healthy while its database was unreachable, weakening the one gate this
deployment has.** So the compose healthcheck stays on the deep check, and the
cheap one exists for the orchestrator that routes conditionally rather than
because convention names it first.

**Two endpoints for one handler, and the reason is a deployment window.** The
compose healthcheck and the runbook both named `/api/health`. Moving it in the
same commit that introduced `/api/readiness` would have pointed a deployed
healthcheck at a path the running image did not have — the same shape as the
tunnel cutover, which used a second hostname rather than moving the first. The
healthcheck moved once an image carrying the route had shipped, in the commit
after; `/api/health` stays as an alias, because the README names it and a test
asserts it and two names for one handler cost nothing.

**What gates and what is merely reported, and this is the register's line
rather than a preference.** A structural failure blocks a deploy; a job-asserted
failure raises a finding and never stops the floor. J66 already reports a
projection past its `freshness_bound`. A readiness probe that turned the same
fact into a 503 would be a J-class failure stopping the floor, which is what D8
refuses. So staleness is reported and the database check alone decides.

**The detail needs a caller.** This is reachable from the internet through the
tunnel. The verdict is public — anything probing can tell a 200 from a 503 —
but the migration head, the step names and the build are internals, and an
unauthenticated reader gets none of them. `GET /images/{digest}` settled the
same question the same way: knowing a thing exists is not authority to read it.

**Migration 89 grants the application `SELECT` on the ledger**, which migration
80 deliberately granted to nobody. That reasoning holds for writing and only for
writing: an application that could record a migration it had not run is the one
lie the mechanism exists to prevent. Reading it is how a deployment answers
*which schema am I on*, and without the grant the field silently vanished from
the response — which is worse than not offering it.

**The drain is ours because actix's is too early.** `HttpServer` installs its
own SIGTERM handler and begins stopping immediately, leaving no moment in which
the process is running and reporting not-ready — and that moment is the whole of
a drain. So signals are disabled and handled here: flip, wait two seconds, then
stop gracefully. **Two seconds because Compose sends SIGKILL ten after SIGTERM**
unless `stop_grace_period` says otherwise, and a drain that waits longer than
the grace period is a drain on paper.

**What it costs, and it is honest to say it.** Nothing in this deployment reads
readiness — Compose has one healthcheck, cloudflared does not probe, and there
is one replica. The drain and the cheap endpoint are built against a caller that
does not exist yet, which this register usually refuses. They are taken here
because the cost is a dozen lines and the alternative is discovering the
distinction the first time there are two replicas, in production, with the
reasoning gone. The trigger for them mattering is a second instance or anything
in front that routes conditionally.

### D164 — A barcode says which box it is on, and a person's name is on the binding

*Adopted 2026-08-31, with migration 90. Answers the note migration 79 left at
its own foot.*

**Decision.** `item_barcode` gains `packaging_level` and `bound_by_person_id`.
`POST /items/{id}/barcodes` binds a scanned identifier to an item at a level,
behind a session rather than a machine token, and refuses a string that already
means something else rather than overwriting it.

**The gap this closes was recorded rather than discovered.** Migration 79 ends
with a section headed *what this does not do, said here so the next reader does
not go looking*, and the first thing in it is that `unit_id` does not imply a
packaging level though D34 assumed it would: *"the built vocabulary does not:
`unit` holds `ea` in the count dimension and nothing above it — no carton, no
inner, no pallet. So a carton GTIN cannot say it is a carton."*

A carton full of individually barcoded boxes is the shape that falls into it.
The box carries a GTIN, the carton carries its own, both resolve to the same
item and mean different quantities of it — and with only `unit_id` to tell them
apart, the table can hold both and distinguish them by nothing. A scan of the
carton then reads as a scan of a box, which is a stock movement of one where
twelve moved.

**The level rather than the unit, and they are not the same question.**
`unit_id` with `quantity` answers *how many base units does one scan mean*,
which is arithmetic. The level answers *what is the label stuck to*, which is a
fact about the packaging. Migration 79 names the alternative — adding `carton`
to the `unit` vocabulary, and leaves it to D23 — and it is the wrong half of the
fork: it would put packaging levels in the table of measures, beside `kg` and
`mm`, and every unit conversion would then have to know that some of its rows
are not measures at all. `packaging_level` is already an enum with five values
and `observable` already keys on it.

**Why a person may write here when a feed may not.** The same file refuses a
feed writing directly, and the reason is worth quoting because it is the reason
this endpoint is shaped as it is: *"a supplier would silently rewrite what a
scan means — the poisoning D19 exists to prevent, one level below measurements
and with a worse blast radius, because a wrong measurement produces a bad
autofill and a wrong barcode binding produces stock movements against the wrong
item."* It closes by saying an assertion lands and **something with a name
promotes it**.

An operator standing at the shelf, holding the box, with the scanner in the
other hand, is that something with a name. D11 makes the actor the
non-repudiable floor of every fact here, and `bound_by_person_id` is what makes
the distinction structural rather than conventional: an operator's binding names
them and a feed has nobody to name. NULL is therefore a real answer — the seed's
bindings and anything an importer wrote have nobody behind them — and not an
unnamed person.

**Rebinding is a refusal, not an overwrite.** `item_barcode_one_meaning_at_a_time`
already says one identifier means one thing at a time, and the endpoint answers
in those terms: the refusal names what the string currently means, because the
person scanning it is holding something and *that is already something else* is
the only answer that helps them. Correcting a binding is closing the
`effective` range on the old row, which is an act with its own name and is not
this one.

**Sending the same binding twice is not a second question.** A trigger under a
glove fires twice, so an exact repeat — same item, same level — answers the row
that exists with `already: true` rather than refusing it. That is D5's shape
reached without a `client_event_id`: the identity of a reference row is the
thing it says, so the write is idempotent on its own content.

**A shared binding cannot be overridden from the floor, and that is a decision
not yet made rather than a considered one.** `item_barcode` has a shared arm —
NULL tenant, the catalogue everybody reads — and the exclusion constraint
COALESCEs the tenant, so a shared row and a tenant's row for one string do not
conflict with each other. The endpoint nonetheless refuses when a shared row is
live, and says so in different words, because the alternative is a tenant row
that silently shadows the shared one and a resolver that quietly prefers it.
Whether an operator *should* be able to say *"that is wrong here"* about a
National Product Catalogue binding is a real question with a real answer on both
sides, and it is untouched: what exists is a refusal that names the situation
rather than a mechanism that resolves it.

**What it does not do.** It does not backfill. Every binding written before this
carries a null level and says so, and the resolver's behaviour for those is what
it always was — return the item and let the operator name the level. Writing
`each` across the table would be inventing the answer the column exists to
record, which is the failure mode this whole register keeps finding.

### D166 — A pick lands where the goods land, and picking is leaving storage

*Adopted 2026-09-01, with migration 91. Settles the carton question
`picking_list.rs` refused to answer.*

**Decision.** `POST /picks` takes a destination that is a location **or** a
package, exactly one — the same exclusive arm D24 gives the stock key. And
`picked` folds as *left storage* (`pick_face`, `bulk`, `overflow`) rather than
*left a location*.

**The question this answers was left open on purpose.** `picking_list.rs` is a
read and says why: *"which carton is a workflow question this model does not
answer — pick straight into the despatch carton, pick into a tote and
consolidate at the bench, or pick a whole wave into one cage — and each of those
is a different screen. Inventing one here would make a choice on the floor's
behalf and then be wrong on somebody's floor."* Right to refuse, and the answer
turned out not to be one of the three.

**What the floor does.** A forklift picks a large order straight onto a pallet.
A picker with a trolley takes goods off the shelf and puts them down at the
packing station, and the packer boxes them there.

The pallet always worked: a pallet is a `package`, and `PALLET-A` has been in
the fixture since the beginning. The trolley did not, and **a trolley must not
be made to work by modelling it**. It is a person's hands with wheels; naming it
as a holder would be inventing a fact to keep the model tidy, which is the
failure this register keeps finding — migration 73's *"a fact copied to where it
is convenient, and then wrong"*. What actually changed is that the goods left
the shelf and are now at the packing station. `location.kind` has had `staging`
in it since migration 1.

**The ledger always agreed.** `stock_movement` carries `to_location_id` and
`to_package_id` as an exclusive pair with whole-key CHECKs, since migration 2.
`RecordPickRequest` took `to_package_id: Uuid`, required. The write path was
narrower than the row it wrote, and that narrowness — not the model — is what
made the carton question feel unanswerable.

**Why `picked` had to change with it, which was not foreseen.** A trolley pick
has two legs, and under the old fold both counted: shelf → station left a
location, station → carton left a location, and one unit was picked twice. J56
would have raised `picked > covered` about a warehouse that had done nothing
wrong. Picking is taking goods **out of storage**; a leg from `staging` moves
goods that were picked already. `packed` and `despatched` are untouched — packed
is still *the destination package is sealed*, which a staging leg cannot
satisfy, and despatched is still *went nowhere at all*.

`location.kind` has carried its five values since migration 1 and no fold had
ever read it. This is the reading it was for.

**What it does not decide.** Whether a picker serving several orders raises a
carton each and rides them on a cage, or walks with a trolley and lets the bench
box everything, is still the floor's business — and both work now without
another decision, because containment is free: `POST /packages/{id}/contain`
writes a `package_event` and no movement, so cartons riding on a cage cost the
ledger nothing. A tote is likewise still available and needs no migration: it is
a `package_type` with `reusable = true`, and this endpoint writes into it
unchanged.


**Who names it, and when.** Not the model, and not per line. The pick walk asks
once, at the start, by scanning: the picker holds a reader at the pallet or at
the packing station's bin label, and it is held for the walk until it is
changed. Every scan after that is a different question — *is this the thing on
the row* — which is why the screen has one scan field and not two. Two inputs
on one handheld is two places to aim a reader and one of them always wrong.

That leaves one thing unchecked, and deliberately: `landing` verifies that
exactly one arm is named and that a location exists. It does not check the
*kind*, so a picker who scans a shelf gets a pick onto a shelf. That is a move
somebody means often enough that refusing it in the client would be the screen
inventing a rule the write path does not have.

**What the walk had to start returning.** `PickLine` carried `allocated`, a
bool about one bin, and it cannot answer *may I claim two more?* — which the
screen has to answer before every pick. `POST /allocations` refuses an
over-claim outright with `OverCovers`, and a pick with no claim behind it drives
`picked` past `covered` and raises J56 against a warehouse that did nothing
wrong. Both edges were reachable, so the read now carries `picked` and `covered`
and the claim is `max(0, picked + q - covered)` — usually nothing, because
planning covered the line when the order arrived.

### D167 — The design system ships patterns, not only materials

*Adopted 2026-09-01. Client only; no migration. Answers why six screens drew
the same list row six different ways.*

**Decision.** Four rules, and the fourth is the one the other three were
symptoms of.

1. **Spacing keys to the viewport; targets key to the density.** `--gap-*`
   moves with the screen's width and nothing else. `--target`, `--text-base`,
   `--readout` and the field widths move with `data-density` — floor or desk,
   the hand and the eye.
2. **The outer container owns the padding.** A `.well` inside a padded face
   sets its own padding down rather than adding a second inset.
3. **A number in a list row is a `Fact`; a number the operator walked over to
   read is a `Readout`.** One instrument per bench, not one per row.
4. **A composition three or more screens re-derive is named and lives in
   `design/patterns/`.** Materials are what a thing is made of; a pattern is
   what it *is*.

**What went wrong.** The system had given the screens materials — `Face`,
`FaceWell`, `Row`, `Spacer`, `Stack` — and no patterns, so every worklist
re-derived the row. Six screens, seventy-nine containers, six different
answers, and the differences were not decisions: the action sat right on one
row and left on the next of the same list, because `Spacer` pushes to the far
end only until the row wraps. Nobody chose that. Nobody could have chosen it,
because it is invisible in the source and visible only at 390px.

The density was worse than the inconsistency. `--gap-*` had been keyed to
`data-density`, which reads sensibly — *the floor needs room* — and means the
phone, which is where the floor is, got the most generous spacing on the
smallest screen. Beneath it every list row nested Panel → Face → FaceWell →
Stack unconditionally, each contributing its own inset, and each *correctly*:
78px of a 430px screen went to padding that no single component was wrong to
add. The capture walk drew seven rows in 2,519px.

Rules 1 and 2 took that to 1,841px. The `Record` pattern took it to **1,498px**
— the same seven rows, forty percent less screen, and now the action is
bottom-trailing on every row of every list because one grid decides it.

**Why slots rather than children.** `Record` takes `lead`, `name`, `tags`,
`facts`, `note`, `meta` and `action` as props. A call site cannot reach inside
and put the action somewhere else, which is the whole point: passing
`children` would have made the layout a suggestion, and a suggestion is what
the six screens were already ignoring. The areas are fixed, so a long
description moves nothing and an absent field costs no empty row.

**One action per row.** A row with two things to do is a row that has not been
designed; the second belongs behind the first, or on the panel. This is a
constraint the type enforces rather than a convention a reviewer has to catch.

**What this does not do.** It does not make the screens denser by making them
smaller. Nothing lost a font size and nothing lost a touch target — `--target`
is still 48px on the floor, because the hand did not change when the screen
did. The saving is entirely padding that was being added twice and structure
that was being drawn six times.

### D168 — Put-away is picking inverted, and the operator names the bin

*Adopted 2026-09-01. Client and one read; no migration — `POST /moves` has taken
`reason = 'putaway'` since migration 59 and nothing had ever called it.*

**Decision.** The dock list is stock held **directly by a `dock`-kind location**.
The operator selects the goods and scans the bin. The system does not direct.

**The inversion.** Picking is one destination and many sources: the picker names
the pallet or the station once, and every scan after it asks *is this the thing
on the row*. Put-away is one source and many destinations: everything starts on
the dock and each thing has its own home, so the goods are selected and the bin
is scanned, once per trip. The two screens are mirror images, and neither could
be the other with the arguments swapped.

**Why this step exists at all.** `POST /receipts` takes `to_location_id`,
documented as *"dock or bin"*, so receiving straight to the shelf has always
been possible and on a floor that works that way this list is correctly empty.
This floor does not: goods are checked in at the dock and put away afterwards,
often later and often by a different person. That is a fact about the floor, and
it is the only reason the step is worth a screen.

**A container moves as a container.** Package-held cells are not offered.
`crate::moving` already refuses `FromPackageHeld` because a package is relocated
by a `package_event`; the read draws the same line from the other side, or it
offers work the write path will not accept. The fixture makes it concrete:
`DOCK-1` holds three cells and one is inside `CARTON-D`, an outbound carton that
has already been despatched.

The rule is the location's *kind*, not the last movement's reason. An adjustment
landing at the dock, or a return, also needs a home; the ledger-based rule would
miss both and the kind-based one catches them.

**An inbound pallet is not a gap being declined.** D6 promised `package` would
serve putaway LPNs, and it will — but a receipt lands loose stock at a location
and has no package arm, so a received pallet is unreachable today. When
receiving grows one, this read grows a second shape and the placement goes
through `POST /packages/{id}/place`.

**The scoring engine is declined, and here is what would unblock it.**
`docs/inbound-analysis.md` sketches `putaway_policy`, a `location_occupancy`
projection and six more columns on `location`. `docs/competitor-analysis.md`
warns about that exact accretion in its own words — *"declining the engine while
accepting five small rule tables is how you get a rules engine you never
designed"* — and the location survey those scores would read has not happened.

What the screen offers instead is `homes`: the storage bins that already hold
this item, nearest on the walk first. That is the raw fact a score would be
computed from, handed over unscored — information rather than instruction, the
same species as the stock-on-hand figure on the capture walk. It turns a blind
choice into an informed one with one query and no policy table.

A scored suggestion can be added later **without changing one byte of what the
ledger records**, because the ledger records where the goods went and not why
that bin was chosen. That is what makes declining it now cost nothing, and it is
the test to apply to the next engine somebody wants: if adopting it later is
free, later is when to decide.

**Nothing blocks a put-away.** A bin's kind is not checked — `POST /moves`
verifies the location exists and is this tenant's, and refuses a move to where
the goods already are, and that is all. A put-away onto another dock is a move
somebody meant. Question 72 reached the same answer from the other direction: a
missing dimension does not block put-away, because the stock is physically on
the shelf whether or not anybody captured the date.

### D169 — A lot arrives as a code, and a stated expiry never overwrites

*Adopted 2026-09-01. One read, two arms on an existing write path; no migration —
`lot` has carried `expiry_date` since migration 2 and nothing could write a row.*

**Decision.** `POST /receipts` takes `lot_code` and `lot_expiry` as well as
`lot_id`. The code is **found or created** against
`UNIQUE (tenant_id, item_id, code)`. The expiry **fills a blank and never
overwrites**: a stated date that disagrees with the one on file leaves the held
date standing and comes back in `warnings`.

**Why it had to exist at all.** `crate::receiving::disposition` *refuses* a line
whose policy requires a lot and has none — not accepts-with-a-finding, refuses,
because accepting it puts stock on the floor that cannot be recalled by lot. One
of the two receiving policies in the fixture sets `require_lot`. And nothing
anywhere created a `lot` row: the table, `stock.lot_id`, the policy flag and the
refusal all existed, and the only way to get a lot was a fixture INSERT. **An
item under a lot-requiring policy could not be received at all.**

**Why the code and not the id.** A receiver holds a carton with a label on it.
They do not hold a uuid, and no screen could hand them one — a lot chooser would
be a list of every lot ever received for that item, which is a search where a
scan should be.

**Why find-or-create is safe here specifically.** The unique constraint does the
work: a replayed receipt finds what the first attempt made rather than raising,
and two receivers working one truck cannot make two rows for one lot. This is the
same property `pressing` gives an act, borrowed from the database rather than
re-implemented above it.

**The expiry rule is the decision, and both halves are deliberate.**

*A blank is filled*, because the first receipt to carry a date is better than no
date — a lot with no expiry is a lot no shelf-life rule can be applied to, and
refusing to fill it would preserve nothing.

*A value is never replaced*, because the held date may be the one a recall will
be run against. Letting whoever received last decide is precisely the silent
resolution D8 built the findings queue to prevent. So it is recorded as a
warning: honest, visible, attached to the receipt that raised it, and it does not
stop the goods coming in (D5).

The receipt is still **accepted**. A disagreement about a date is not a reason to
turn a truck away.

**A delivery is a session, and its header id comes from the act.** D43 and Q172
already said one delivery is one `goods_receipt` with lines joined by a
client-minted id. The screen holds it open and the operator closes it, because
only they can see the last pallet is off. The first line mints the id from
`act.id("delivery")` rather than a fresh uuid — a replayed first line has to
reach the same header, or a lost response turns one delivery into two, which is
the exact failure the shared id exists to prevent.

**What the read carries, and why each thing is on the wire.** `requires_lot`,
because discovering a refusal at the dock means the pallet is already broken
down. `levels`, because D92 and Q173 have the receiver counting cartons and the
conversion needs a config the screen cannot guess. `owners`, because the
promise's own `owner_id` is often null and `POST /receipts` then refuses the line
— and there is no marked convention for who owns received stock, so the parties
already holding that item here are offered as facts rather than one being
invented as a default. That is the mistake `bench.rs` made when it chose a dock
with *"any location at the site will do for the demo"*.

**Declined: a delivery nobody promised.** `expected_supply_id` is required, so
goods arriving against no purchase order have no path through this screen or any
other. That is a real gap on a real dock. It needs an ad-hoc supply write, and it
circles question 26 — who the goods are for — which has not been answered. Named
here so it is a known hole rather than a surprise.
