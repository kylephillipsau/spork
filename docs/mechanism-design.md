# D21-D26: six mechanisms, designed in parallel and reconciled

Generated 2026-08-01. Six mechanisms were designed independently against
[cross-cutting-review.md](./cross-cutting-review.md), then each was cross-checked
against all five siblings and against D1-D20, then reconciled into one set.

All six cross-checks returned `major-conflicts`. Almost all of it was
**duplication rather than disagreement** - the six designs independently invented
the same primitives - which is section 1 of the cross-cutting review happening
live, six times over.

29 blocking conflicts were raised and resolved. The **Shared primitives** section
is the highest-value output: it is where several designs were collapsed into one.

**Status: proposed, not adopted.** This continues domain-model.md's numbering but
has not been merged into it. Read the Amendments and Invariant register sections
before accepting anything - several existing decisions change materially, and
`package_content` is deleted as a base table.

**Provenance caveat.** Per-design detail and the raw cross-checks are in the run
journal at `.claude/projects/*/subagents/workflows/wf_02cc8d29-a22/journal.jsonl`.

---

# D21–D26: reconciling six parallel mechanisms

*Written 2026-08-01, continuing [domain-model.md](./docs/domain-model.md). Six mechanisms were designed in parallel against the cross-cutting review and then cross-checked. All six cross-checks returned `major-conflicts`. This document is the reconciliation.*

---

## Verdict — do the six mechanisms form a coherent whole?

**Yes, after four of them give something up.** The conflicts were almost entirely *duplication* rather than disagreement: two designs invented `policy_binding`, three invented `observation`/`metric`, four invented `acceptance`, three invented `package_event`, three invented an idempotency mechanism, and five invented their own CI harness. That pattern is itself the finding — it is §1 of the review happening live, six times, because no artefact held the shared primitives where a parallel design would collide with them.

Where they genuinely disagreed, there were six real forks. Here is how each was decided and on what grounds.

**1. Q59 — does `package_id` join the `stock` key?** *Yes.* The competing answer (containment stays in `package_content`, container moves fan out to N `stock_movement` rows) requires a reconciliation job to defend an invariant the schema makes unenforceable: `package_content` has no `status_id` and no `owner_id`, so the assertion can only ever be partial. It also asserts forty physical events that did not happen. Holder-keyed stock makes the drift *unrepresentable* rather than *detected*, which is the stronger form. Grounds: principle 1, and D8's "the work event is the invariant" — a movement row must correspond to something a person did.

**2. Q60 — what are `stock_allocation`'s supply arms?** *Two: a cell, or an `expected_supply` row.* The five-arm proposal breaks §6 on the hottest intention table; the "never allocate against an assertion" rule and the cross-dock capability are reconciled by the fact that `expected_supply` projects into **expectation, never into `stock`**. A counterparty claim can therefore be planned against without ever reaching a balance. That is the assertion design's own rule 3 satisfied, and its "expectation namespace" finally given a table.

**3. Assertion or observation — where does a counterparty's declared weight live?** *Assertion upstream, observation downstream, and the boundary is sharper than either design drew it.* The deciding observation neither design made: `asserted_unit` is **our** row, minted when we parse; it always has an id and therefore always has an observable. The thing that may be unresolvable is `raw_gtin`, and that is a column on the body. So the weight columns come off (the observation design was right) *and* the rung-3 supplier remains storable (the assertion design was right). The rule: **an assertion body holds identifiers, structure, and the values the receipt compares line by line. Every other number with a unit is an observation.**

**4. `decision_rule` (a compiled Cedar predicate) — build it?** *No. Deferred, with the threshold written down.* Two designs amended D13 in mutually exclusive directions. The scope-lattice restatement is a grep; the predicate restatement needs a compiler to check. More decisively: once the lattice exists, `decision_rule`'s own motivating example ("vendor X's goods go to zone 3") is a `putaway` binding on the counterparty dimension, and its remaining example ("if the supplier is late twice in a month, sample every pallet") is exactly what the product position says stays code. Ship the lattice; revisit when **two tenants want different behaviour at the same decision point**. Grounds: the standing direction. Generality from a cleaner primitive (uniform addressing) beats generality from deferring evaluation to runtime.

**5. Principle 2 — how many categories?** *Four, on a provenance axis, orthogonal to a role axis.* One design fixed it at four, one proposed seven in a single CI-asserted enum, one proposed five and drove Postgres grants off it. Four-on-two-axes is the only reading where the count stops growing, and it is the only one under which `goods_receipt` (grouping), `stock` (projection) and `allocation_policy` (policy) stop looking like new categories.

**6. Where does status live?** *Derived, declared, or forbidden — decided by provenance category, not by "is the transition evidence".* The review's own test is not decidable (nobody ever loses the argument that their transition is evidence), and it has two branches where the model needs four. But the replacement as first drafted was also not total: it left `package`, `discrepancy` and `inbound_shipment` uncovered. Completed below.

**One thing I am telling you outright: `row_audit` is dropped, and `decision_rule` is not built.** Both were defensible locally and both are the shape this project exists to avoid — a polymorphic pair plus a JSONB payload in the first case, field names and operators in rows in the second. Neither survives its own document's rejected-alternatives section.

---

## The decisions

### D21 — Assertions are principle 2's fourth category, and the axis has exactly four values

**Decision.** An **assertion** is a statement of record exchanged with another party, stored exactly as exchanged, which neither side may unilaterally revise. It is a fourth value on the **provenance axis**, which is now closed at four and orthogonal to a **role axis**.

| Axis | Values | Decides |
|---|---|---|
| **Provenance** | fact \| intention \| assertion \| finding | mutability, who may author, what may project from it |
| **Role** | subject/reference \| projection \| policy \| grouping | how it is read, indexed, rebuilt |

`goods_receipt` is a grouping, not a fifth category. `allocation_policy` is policy. `stock` is a projection. Every table registers exactly one value on each axis, in code, and CI diffs the registry against `information_schema.tables`.

**Why authorship is not the right cut.** The property that generates every rule is not *who wrote it* — it is that **a copy exists outside our control**. Our own outbound DESADV is as unrevisable as a supplier's inbound one. Taking the symmetric version costs one `direction` column and buys outbound EDI, POD and quotations on machinery we build once.

**Why "intentions have an author" fails.** Six reasons, of which three are decisive: mutability is the intention category's *defining* rule and must be disabled for every counterparty claim; assertions arrive in the author's vocabulary and are normally unresolvable, where an intention with dangling FKs is a defect; and intentions project into `stock.allocated_quantity` while an ASN must not.

**The five rules.**

1. **Immutable.** No UPDATE, no DELETE, ever. A revision is a new assertion.
2. **Always names its author party.** `author_party_id NOT NULL` — an access-control boundary, not metadata.
3. **Never projects into `stock` or into commitment.** Assertions project into **expectation** (`expected_supply`, D24), maintained by the same rebuild-and-assert job.
4. **Exists to be compared.** A claim never checked is itself a finding.
5. **Recorded in the author's vocabulary. Resolution into ours is a separate, fallible, recorded step.**

```
party_message                 -- FACT. Replaces provider_exchange AND inbound_message.
  id, tenant_id, party_id
  direction                   -- inbound | outbound
  channel                     -- edi | portal | csv | email | api | webhook | print
  transport_ref, content_type
  payload bytea               -- verbatim. NEVER jsonb. Never queried structurally.
  byte_count, content_hash
  occurred_at, recorded_at
  client_event_id             -- FK client_event (D25)
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
  client_event_id             -- FK client_event
  supersedes_assertion_id     -- THEIR claim that this replaces that
  correction_of_assertion_id  -- OUR transcription fix
  CHECK (party_message_id IS NOT NULL OR captured_by_id IS NOT NULL)
  CHECK (correction_of_assertion_id IS NULL OR party_message_id IS NULL)
  UNIQUE (id, kind)                                 -- composite FK target for bodies
  UNIQUE (id, tenant_id, author_party_id, kind)     -- target for supersession
  FOREIGN KEY (supersedes_assertion_id, tenant_id, author_party_id, kind)
    REFERENCES assertion (id, tenant_id, author_party_id, kind)
    DEFERRABLE INITIALLY DEFERRED
  FOREIGN KEY (correction_of_assertion_id, tenant_id, author_party_id, kind)
    REFERENCES assertion (id, tenant_id, author_party_id, kind)
  -- NO unique on (author_reference, author_version): a duplicate resend must be
  -- STORABLE and raise a finding (D5), not be refused at the write.
  -- NO status column. Our position is assertion_stance.

assertion_stance              -- FACT: our position on a claim
  id, tenant_id, assertion_id
  stance                      -- pending | in_force | rejected | superseded
                              -- | withdrawn_by_author | expired
  reason_code, note, successor_assertion_id
  CHECK (stance <> 'superseded' OR successor_assertion_id IS NOT NULL)
  occurred_at, recorded_at, client_event_id
  recorded_by_id / automation_key      -- CHECK num_nonnulls(...) = 1
  authorised_by_id

assertion_check               -- FACT: a claim was checked against reality
  id, tenant_id, assertion_id
  asserted_unit_id, asserted_unit_content_id
  metric_id                   -- FK metric (D23). NOT a second vocabulary.
  outcome                     -- agreed | disagreed | unverifiable | unchecked_at_close
  asserted_numeric, observed_numeric        -- canonical units (D23)
  asserted_text, observed_text
  variance_numeric GENERATED
  discrepancy_id
  CHECK (outcome <> 'disagreed' OR discrepancy_id IS NOT NULL)
  checked_at, recorded_at, client_event_id
  recorded_by_id / automation_key
```

**Typed bodies, one per kind**, joined by a composite FK on `(assertion_id, kind)` with the body's `kind` a stored generated constant — so "this body belongs to an assertion of the matching kind" is declarative, not a trigger.

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
  -- NO gross_weight_g / net_weight_g / ti / hi. Those are OBSERVATIONS whose
  -- observable is this asserted_unit and whose asserted_by is the author (D23).
  -- Nesting is unbounded here (cold path); it collapses to D24's cap at receipt.

asserted_unit_content
  id, asserted_unit_id
  raw_gtin, raw_item_code, resolved_item_id
  raw_po_reference, raw_po_line_number, resolved_purchase_order_line_id
  quantity, entered_quantity, entered_unit_id      -- structural: the receipt
  lot_code, expiry_date, best_before_date          --   compares these line by line
  resolved_at, resolved_by_id, resolution_method
```

**Two column classes, and the immutability rule follows the class.** `raw_*` and every transcribed value are immutable. `resolved_*` are *our annotation* and may be written when resolution later succeeds — a GTIN unresolvable today becomes resolvable when the item is created tomorrow, and refusing that would mean discarding a claim because our catalogue was behind. But a re-resolution **freezes on first use**: once an `assertion_check` or a `goods_receipt_line` references a resolution, it may not be rewritten; a correction writes a new assertion. That closes the "resolution churn leaves a hole in the audit trail" risk, and it applies the same rule the model already uses for `goods_receipt_line.expected_quantity`.

**`inbound_shipment` is a subject, not an assertion.** Filing it as an assertion means a resend mints a second row and orphans every FK pointing at the first — the `consignment.fulfilment_id` defect D15 already deleted once.

```
inbound_shipment              -- PROJECTION (subject)
  id, tenant_id, site_id, supplier_party_id, owner_party_id
  vendor_shipment_ref
  in_force_assertion_id       -- projection: the currently effective claim
  granularity, estimated_arrival_at
  asserted_unit_count, asserted_base_quantity      -- for the gate check
  first_asserted_at, superseded_count
  vehicle_arrival_id
  UNIQUE (tenant_id, supplier_party_id, vendor_shipment_ref)
```

Nothing on it is NOT NULL that requires an assertion, so blind receipt (rung 0 of the degradation ladder) is a **schema property**, not a workflow branch.

**Amends.** Principle 2 (fourth category, two axes, count closed at four). Principle 3 (`provider_exchange` and `inbound_message` merge into `party_message` as bytes). D1 (`consignment.eta`/`.status`/`.price_minor` become projections of the in-force `carrier_status_advice`). D5 (terminology: reword "counts are assertions" to "counts are **absolute claims** — register semantics, not deltas"; `stock_count` is a fact and ours). D8 (`discrepancy` gains `assertion_check_id`; new kinds `expiry_mismatch`, `identity_mismatch`, `assertion_unresolvable`, `asserted_unit_absent`, `asserted_unit_unexpected`). D11 (machine actors: `automation_key` XOR `recorded_by_id`, on assertion-ingestion facts **only** — `stock_movement` keeps its NOT NULL person). D14 (`lot.expiry_date` remains the *accepted operational value*; a supplier-asserted expiry is an assertion, and disagreement is `expiry_mismatch`).

**Rejects.** "A counterparty's intention is still an intention" (six reasons). One table per assertion kind with author/artefact/clocks repeated. A single assertion table with a JSONB payload. An EAV bag of asserted attributes. Correcting an assertion in place where an artefact exists. Making the category asymmetric (inbound only). Treating adopted rate cards and customer shelf-life requirements as assertions — those are **policy**, carrying `adopted_from_assertion_id`, because we may change them unilaterally.

---

### D22 — Policy resolves against a scope lattice

**Decision.** One `policy_binding` table. A policy is a **typed value row bound to a point in a lattice of six ordered, tree-shaped scope dimensions, effective over a period**. Resolution: match every binding whose non-null dimensions are at-or-above the request's node on each axis; order the matches by their **depth vector**, compared lexicographically in a precedence order declared per kind **in code**; take the winner's value row; then clamp any field the value type declares as clamped. Ties are not broken — they are made structurally impossible.

| Dimension | Nodes, least to most specific | Columns |
|---|---|---|
| **Tenancy** | operator (NULL) → tenant | `tenant_id` |
| **Product** | any → `item_class` ancestors → `item_class` → `item` | `item_class_id`, `item_id` |
| **Counterparty** | any → `party_class` → `party` | `party_class_id`, `party_id` |
| **Space** | any → `site` → `zone` | `site_id`, `zone_id` |
| **Ownership** | any → `owner_party` | `owner_party_id` |
| **Metric** | any → `metric` (flat) | `metric_id` |

**Tenancy is not a declarable dimension — it is the mandatory first component of every depth vector.** A tenant's binding always beats an operator-shipped one, always, asserted in CI over the compiled registry. Without this, a per-kind precedence order that ranks Product above Tenancy lets an operator default outrank a tenant's own configuration — a correctness hole, not a support surface.

**Specificity is a vector, not a number.** Collapsing a componentwise comparison to one integer lets a large count in a low-weight component beat a small count in a high-weight one; [W3C Selectors L4](https://www.w3.org/TR/selectors-4/) says so explicitly and CSS is the proof. There is no `specificity` column. Ordering happens in code over candidate sets of tens of rows, where the precedence order is a visible `const DIMENSIONS: &[Dimension]` on the kind's Rust type.

**A scope is a conjunction, not a disjunction.** The columns are independent nullable axes, NULL meaning "any". There is deliberately **no `num_nonnulls` CHECK** — adding one would silently reverse the semantics and forbid the all-NULL operator default that shipped defaults and clamping require. An all-NULL binding is the shipped default and is the correct shape.

**The entire matching language has cardinality one:** *is this node an ancestor-or-self of that node*, applied uniformly over closure tables. No `<`, no `LIKE`, no `IN`, no boolean connectives anywhere in the data. That is the reason this is not a rules engine, and it is now a property you can grep for.

**Ties are prevented, not broken.** Within a tree dimension a request node has exactly one ancestor at each depth, so two matching bindings with identical depth vectors must name identical scopes — forbidden by `UNIQUE NULLS NOT DISTINCT`. The load-bearing dependency is that **every scope dimension is single-parent**. If `item_class` becomes many-to-many tags, resolution becomes non-deterministic and this design is unsound. Defence in depth: if the resolver ever sees equal depth vectors it takes the lower binding id (deterministic — D5, never stop the floor) and raises `discrepancy.kind = 'policy_ambiguous'`.

```
policy_binding                -- WHERE a policy applies. Scope is IMMUTABLE.
  id, tenant_id               -- NULL = operator-shipped default
  kind                        -- 11 kinds (below)
  item_class_id, item_id
  party_class_id, party_id
  site_id, zone_id
  owner_party_id
  metric_id
  supersedes_id, note, created_at, created_by_id
  UNIQUE NULLS NOT DISTINCT (tenant_id, kind, item_class_id, item_id,
                             party_class_id, party_id, site_id, zone_id,
                             owner_party_id, metric_id)
  UNIQUE (id, kind)           -- composite FK target for value tables
  INDEX (tenant_id, kind)     -- the resolver's only scan

policy_change                 -- FACT. Append-only. Mandatory reason.
  id, tenant_id, occurred_at, recorded_at
  policy_binding_id, kind
  action                      -- created | revalued | retired | reinstated | rescoped_out
  reason (NOT NULL)           -- a weight change with no reason is how tuning
                              --   becomes superstition
  recorded_by_id, authorised_by_id
  INDEX (tenant_id, recorded_at), INDEX (policy_binding_id, recorded_at)
```

Every value table has the same shape and the same three constraints:

```
<kind>_policy                 -- WHAT applies and WHEN. Append-only versions.
  id, policy_binding_id, kind
  CHECK (kind = '<its kind>')
  FOREIGN KEY (policy_binding_id, kind) REFERENCES policy_binding (id, kind)
  effective tstzrange
  EXCLUDE USING gist (policy_binding_id WITH =, effective WITH &&)
  ... typed scalars ...
  created_at, created_by_id
```

**Eleven kinds:** `allocation`, `putaway`, `receiving`, `order_tolerance`, `count_tolerance`, `shelf_life`, `sampling`, `cycle_count`, `specification`, `observation_precedence`, `observation_acceptance`. The enum, the set of `%_policy` tables and the compiled Rust registry are asserted equal in CI; any of the three drifting is a build failure.

Selected values, where they replace something:

```
allocation_policy    weight_rotation, weight_travel, weight_access,
                     rotation_tolerance_days, max_equipment_class_id,
                     allocation_expiry_hours              -- q25 gets a home
putaway_policy       weight_travel/consolidation/access/fit, allow_mixed_items,
                     allow_mixed_lots, avoid_earlier_expiry_commingle,
                     preferred_zone_id                    -- "vendor X → zone 3"
receiving_policy     default_status_id, over_receipt_pct/abs, short_closes_line,
                     require_expiry, require_country_of_origin,
                     require_photo_on_variance, blind_allowed,
                     respond_by_hours                     -- the commercial clock, q62
shelf_life_policy    min_remaining_days_ship      [clamp = Max]
                     min_remaining_pct_ship       [clamp = Max]
                     min_remaining_days_transfer          -- q39
specification_policy min_numeric, max_numeric, target_numeric,
                     min_code_ordinal, max_code_ordinal, breach_severity
                     -- scoped BY METRIC via the metric dimension: one binding,
                     -- one value row. No second key on the value table.
```

**Combination: most-specific-wins over the whole value row, plus one exception.** Whole row, not per field — you must not take `weight_rotation` from a customer binding and `weight_travel` from a site binding, because weights are only meaningful relative to each other. The exception is **per-field clamping declared on the Rust value type** (`#[policy(clamp = Max)]`): the winner's value, clamped against every less-specific match. That is what "customer × item class, plus a site floor" actually asks for, and it gives a commercial product a real governance primitive — an operator-shipped ceiling no tenant can exceed — free from a mechanism we needed anyway.

**No per-row override flag.** That is `!important`, and it exists precisely to escape the precedence order it was supposed to live in. Refused. If an operation needs both "a floor customers cannot undercut" and "a default customers can", those are two fields with two declarations.

**The one thing that outranks the resolver sits outside it.** An explicitly agreed value on an instance — `order_line.tolerance_under_pct` from an EDI order or typed by a salesperson — wins outright and the resolver is not consulted. It is a different category of thing (what was *agreed*, not what we do by default). When it arrives on an EDI order it is a projection of the in-force assertion carrying `derived_from_assertion_id` (D21); when a salesperson types it, it is our own intention on the line.

**Reproducing a past decision is a foreign key, not a replay.** Value rows are append-only and never updated, so `stock_allocation.allocation_policy_id` points at the exact immutable row that scored it. Referential integrity guarantees it exists; immutability guarantees it still says what it said. **One naming convention, asserted: `<kind>_policy_id`, always an FK to the value row, never a version integer.** "What did we believe on Tuesday" is a cold-path replay of `policy_change` filtered on `recorded_at <= T`. The recorded FK is authoritative; replay is corroboration, and disagreement is a finding.

**Scope is immutable.** Editing a binding's scope would silently change the meaning of every value row a past decision already references. Rescoping is retire-and-create, linked by `supersedes_id`.

**The resolver returns an explanation, not a value.**

```rust
struct Resolution<V> {
    value: V,
    source: Source,              // Binding(id) | InstanceAgreement | CodeDefault
    winner: Option<PolicyValueId>,
    clamped_by: Vec<(FieldName, PolicyValueId)>,
    considered: Vec<Candidate>,  // explain mode only
    rejected:   Vec<NearMiss>,   // explain mode only: WHICH dimension failed
    request: ScopeRequest,
    resolver_version: u32,
}

fn resolve_batch<V>(kind: PolicyKind, reqs: &[ScopeRequest]) -> Vec<Resolution<V>>;
```

**The batch form is the primary interface**, single-request a wrapper. D10's principle-6 argument was that the batch mechanism must be *expressible*; a closure-table join per candidate cell is not, and "nothing reads a policy value table except the resolver" does not stop the resolver being called in a loop. `considered`/`rejected` are populated in explain mode only.

The near-misses are the useful half. "Why is 90 days applying?" is answered by the winner. "Why is the 60 I configured **not** applying?" is only answerable by evaluating the bindings that did not match and naming the dimension that failed. `CodeDefault` is returned as a synthetic candidate, never as silence.

**Where the line sits.** D13 said *"if we ever find ourselves adding a table where the logic itself is rows, that is the line"* — true, and unfalsifiable as written. Sharpened:

> **Data may say where a number applies and how big it is. Only code may say what to do with it.**
> A row may contain: a scope node identifier, a period, and a typed scalar.
> A row may never contain: the name of a field, the name of an operator, a comparison, a boolean connective, the target of an action, or an ordering of steps. **The moment a table has a column whose *value* is a *column name*, we have crossed.**

That is a grep. D365 work templates fail it on the first clause; Oracle putaway priority tables fail on the second. So does a compiled predicate language, which is why `decision_rule` is deferred (D26).

**One bounded extension, fenced now rather than smuggled in later.** Stepped tolerance by order size is piecewise. A value table may have **at most one child, keyed on a single numeric axis declared in code**, with `[lower, upper)` bands and a no-overlap exclusion constraint, containing only bounds and typed scalars. `order_tolerance_band` is the only instance. The axis is named in the Rust type — **there is no `band_axis` column**, because a column whose value names an axis is precisely what the line above forbids, and the first draft of this design failed its own CI check on that column.

**Prerequisites, and one correction the parallel designs missed.** `item_class` and `party_class` must exist as per-tenant rooted trees with closure-table projections. But `item.item_class_id NOT NULL` is **wrong** and would have broken D19: a shared item (`tenant_id IS NULL`) cannot carry a mandatory FK into one tenant's private taxonomy — every other tenant reads it as an unresolvable link, which is exactly the dead end D10 exists to prevent. Classification is an association:

```
item_classification
  tenant_id, item_id, item_class_id
  PRIMARY KEY (tenant_id, item_id)      -- exactly one class per item per tenant
```

Single parentage is preserved (so the tie-freedom proof holds), D19's thin shared item is preserved, and the per-tenant `uncategorised` root becomes a default row rather than a backfill over the shared catalogue. `zone` becomes a real table (`zone(id, site_id, code, ...)`, `location.zone_id`), not a bare column — the Space dimension needs something to FK to and a depth to read.

**Amends.** D13 (`allocation_policy(scope_kind, scope_id)` → `policy_binding_id`; the scoring function stays code, unchanged and reaffirmed; the line is sharpened into a grep). D14 (`customer.min_shelf_life_days`/`_pct` are **removed** — they cannot express q33's "different requirements by category"). D20 q55 (`order_line.quantity_tolerance_pct` → instance agreement; the policy value moves to `order_tolerance_policy` + band child; **`order_line` is not a scope** — it would put instance-cardinality rows in a config table). D9/q21 (`count_tolerance_policy`, distinct from order tolerance: different numbers, different screens). D19 (a third RLS shape — see amendments). D8 (`discrepancy.kind += policy_ambiguous`; `discrepancy.respond_by` populated from `receiving_policy.respond_by_hours`). Open decisions #25, #30, #33, #38, #39, #46, #63.

**Rejects.** `policy_scope(scope_kind, scope_id, precedence)` — a polymorphic pair (D10) over the wrong logical structure, and structurally unable to express (customer AND item class). A scalar or packed `specificity`. A stored `specificity smallint[]` sort key (per-kind precedence makes one stored vector meaningless). Tie-breaking by entry order, `created_at`, or an access sequence (SAP) / document order (CSS). A per-row `is_override` flag. A `combine` column chosen per kind as data — the combining algorithm is semantics and belongs in the Rust type. Predicate rows. `policy_value(policy_id, field_name, value)`. JSONB value blobs. Many-to-many item tags. One generic `classification(kind, ...)` table. Full SQL:2011 bitemporal ranges. A version integer instead of a value-row FK. Per-tenant policy kinds or tenant-defined dimensions. A `policy_resolution` audit table (volume is per-scan; the value-row FK carries the same information).

**Not amended, deliberately:** policy is **not** a projection. The draft claimed its state was rebuildable from `policy_change`, but `policy_change` holds no values, so the claim reduces to rebuilding the value rows from the value rows. Worse, under D25's projection mechanism that claim would revoke UPDATE from the very manager the policy exists to serve. The guarantee is carried entirely by the bidirectional anti-join: every value version has a matching `policy_change` and vice versa.

---

### D23 — Observations generalise; the subject set opens on a registry, not on the fact

**Decision.** `measurement` is replaced by a two-level fact pair — `observation_event` (the act) and `observation` (one result of it) — over three reference primitives: an `observable` subject registry, a data-defined `metric` vocabulary whose *result kinds* are code-defined, and a `dimension`/`unit` pair carrying exact rational conversion to one canonical integer per dimension. Provenance decomposes into **who asserted it**, **how it was obtained**, and **through what channel**.

**Three welds, not one.** `metric` is closed by an enum — but widening it alone fails on the first temperature, because an affine unit cannot be an integer in an implied canonical unit, an ETA is not an integer at all, and a quality grade is an ordinal term. `subject_type` is closed by an enum, and q20 as posed would have frozen the subject set into the shape of the family's hottest table at the exact moment inbound opens it. And `source` conflates three orthogonal things: `carrier_actual` bundles *a carrier asserted it* with *an instrument produced it*; `operator_correction` is not a source at all but a lifecycle event about a different row.

**The subject is typed once, in a registry, rather than at every call site.**

```
observable                    -- REFERENCE. The ONLY place the subject set widens.
  id, tenant_id (NOT NULL)
  kind                        -- GENERATED: which arm is set
  item_id, packaging_level, item_packing_config_id   -- each|inner|carton|layer|pallet
  package_type_id, package_id, lot_id, location_id, consignment_id
  asserted_unit_id, asserted_unit_content_id, device_id, vehicle_arrival_id
  CHECK (num_nonnulls(<ten arms>) = 1)
  CHECK ((item_id IS NOT NULL) = (packaging_level IS NOT NULL))
  CHECK (item_id IS NULL OR packaging_level = 'each' OR item_packing_config_id IS NOT NULL)
  UNIQUE (id, tenant_id)
  ... one partial unique index per arm ...
```

Every arm is a real FK, so referential integrity holds and an investigation cannot dead-end. Batch loading is two hops with no N+1. The **fact tables are permanently stable in shape**: adding "we now observe pallet-pooling accounts" is one column on a table of ~10⁵ rows and zero change to anything holding 10⁷.

**§6's four-arm limit is amended with a boundary rule, not waived.**

> Typed nullable FKs with a mutual-exclusion CHECK are correct when the arms are **alternative identities of one referent** — a discriminated union where exactly one is structurally required and "none" is meaningless. They strain when the arms are **distinct relationships that merely happen to be exclusive today** — causes, demands, sources — because there exclusivity is a *policy*, and policies turn out to be wrong.

That explains both prior failures: `stock_movement`'s cause CHECK and `goods_receipt`'s demand CHECK both had to relax to `<= 1`. Nobody will ever discover an observation about no thing or about two things at once. This rule licenses `observable`'s ten arms and `acceptance`'s five (D25), and it *constrains* `discrepancy`'s cause arms, which stay at `<= 1` and are capped at five.

**`item` carries a `packaging_level`**, so "the carton, not the each" is expressible. The identity is `(item_id, packaging_level, item_packing_config_id)`, with the config NULL only at `each` — a carton is only a definite physical object relative to a case pack, and because `item_packing_config` is already versioned, a corrected case pack cannot silently rewrite the dimensions of cartons shipped last year. `packaging_level` is a **subject qualifier and never a unit**; a carton is not commensurable with a millimetre.

**A `stock` cell is deliberately not an observable.** It is tempting — a stocktake really is "observed quantity of item I at location L" — and the exclusion survives even though D24 gives `stock` a surrogate id. The reason is not identity, it is that admitting it would drag D4, D12, D20 and D24 into this decision for a case `stock_count` already serves. Counterparty-asserted quantities still work, because they are always properties of an identified thing.

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
                 UNIQUE (id, metric_id), UNIQUE (metric_id, code)
```

**`metric.aggregation` and `metric.higher_is_better` — one stays, one goes.** `aggregation` (last|min|max|mean) is a per-row data value selecting which fold the projection performs. That is semantics, and semantics belong in the Rust type — the same argument that refused a `combine` column on a policy-kind registry in D22. It is removed; the fold is a property of the reserved metric's code-side registration. `higher_is_better` stays: it is a display and scorecard hint, not a fold.

**The metric vocabulary is not a custom-field framework, and here is the test.** EAV is deferring *type* decisions to runtime; its signature is one `value text` column, an arbitrary attribute name, an untyped subject, and a schema that cannot be read. This has none of those: the subject is an FK; the value is one of five typed columns chosen by the metric's declared `result_kind` and enforced by a composite FK plus a CHECK, per row, by the database; the unit is enforced commensurable by a second composite FK, so recording a length in grams is a constraint violation rather than a code-review finding; nothing queryable is in JSONB; and a metric cannot add a column to another table or cause the system to branch. **The result types are code; only the vocabulary is data** — D13 one level up. Enforced by two mechanisms: reserved metrics ship with `tenant_id IS NULL, reserved = true`, and **application code may name only reserved codes**, which is a grep in CI. The day someone writes `if metric.code == "customer_special_thing"`, the build fails.

**Gross, net and tare are three metrics, not one with a modifier.** GS1 already settled this (AI 310n net, AI 330n gross). The current single `weight` metric is genuinely ambiguous today, and gross-versus-net is exactly the discrepancy a carrier re-weigh surfaces.

```
observation_event             -- THE ACT. Append-only.
  id, tenant_id, observable_id
  observed_at                 -- VALID time (device clock, D5)
  recorded_at                 -- TRANSACTION time (server clock, D5)
  client_event_id             -- FK client_event (D25)
  device_id                   -- the RECORDING device. Unconditional (D11).
  instrument_device_id        -- the MEASURING instrument. CHECK: set only when
                              --   method IN ('instrument','scan')
  recorded_by_id / automation_key      -- CHECK num_nonnulls(...) = 1
  work_session_id, authorised_by_id
  asserted_by_party_id        -- WHO claims it. NULL = us.
  method                      -- HOW: instrument|scan|keyed|derived|estimated
                              --      |transcribed|asserted
  ingestion_channel           -- THROUGH WHAT: edi|portal|csv|email|api|keyed
                              --               |scale|scanner|derived
  party_message_id, attachment_id, derived_from_event_id
  work_task_id, goods_receipt_id
  challenged, challenge_context, confirmed        -- D9, generalised
  UNIQUE (id, observable_id), UNIQUE (id, observed_at), UNIQUE (id, tenant_id)

observation                   -- ONE RESULT. Append-only. Never UPDATEd or DELETEd.
  id, tenant_id, observation_event_id
  observable_id, observed_at             -- denormalised, composite-FK'd to the event
  metric_id, result_kind, dimension_id   -- denormalised, composite-FK'd to metric
  value_numeric bigint        -- ALWAYS the dimension's canonical unit.
                              --   NO unit column: non-canonical storage is
                              --   structurally unrepresentable.
  value_instant timestamptz   -- ETA, asserted expiry
  value_code_id, value_boolean, value_text
  uncertainty_dimension_id, uncertainty_numeric   -- half-width; defaults from
                              --   instrument_device.resolution_numeric
  absent_reason               -- not_measured|not_applicable|unreadable|retracted
  entered_value numeric, entered_unit_id          -- as the counterparty gave it
  confidence smallint
  corrects_observation_id     -- the target was NEVER true (retroactive)
  retracts_observation_id     -- the target should not exist
  UNIQUE (id, observable_id, metric_id)
  UNIQUE (corrects_observation_id), UNIQUE (retracts_observation_id)
  FK (observation_event_id, observable_id) -> observation_event(id, observable_id)
  FK (metric_id, result_kind)              -> metric(id, result_kind)
  FK (metric_id, dimension_id)             -> metric(id, dimension_id)
  FK (entered_unit_id, dimension_id)       -> unit(id, dimension_id)
  FK (value_code_id, metric_id)            -> metric_code(id, metric_id)
  FK (corrects_observation_id, observable_id, metric_id)
                                           -> observation(id, observable_id, metric_id)
```

`uncertainty_dimension_id` is separate from `dimension_id` because an ETA has no dimension but "±2 hours" is a real and necessary answer — the first draft made ETA uncertainty structurally unrepresentable, which quietly killed the carrier-ETA capability it claimed.

**Provenance is three columns, and they are deliberately uncorrelated.** A carrier re-weigh is `(carrier, instrument)`. A supplier ASN is `(supplier, asserted)`. Our own eyeball is `(NULL, estimated)`. The old enum could express the first and third only by having a value per combination, which is why it ran out.

**The five-channel test, made checkable.** The same fact — pallet SSCC 393123… weighs 412.5 kg — arriving over EDI, a portal, a CSV, a dock scale and a keyboard produces five rows that are **byte-identical in `(observable_id, metric_id, value_numeric, entered_value, entered_unit_id)`**, differing only in provenance. One CI fixture per channel, one assertion. If a new adapter ever needs a content column the others do not have, the test fails on the day it is introduced. That is the interoperability requirement, made testable instead of asserted.

**Supersession, correction and retraction are three different things.** Supersession needs no mechanism — a pallet weighed 400 kg Monday and 380 kg Tuesday because a carton came off; both are true at their own times and precedence is time plus policy. Correction is an explicit link because the old row was *never* true and must stop influencing the projection **retroactively**. Retraction is an explicit link with no replacement value. All three are recorded as observations, so the table stays strictly append-only with no mutable status column on a fact.

With both clocks and corrections distinguished from supersessions, two genuinely different questions get genuinely different answers: *what did the pallet actually weigh on Monday* (`observed_at <= Monday`, excluding corrected and retracted) and *what did we believe on Monday* (`recorded_at <= Monday`, including rows later corrected). The second is what a chargeback dispute needs, and it is unanswerable if correction and supersession are the same thing.

```
observation_current           -- PROJECTION, keyed (observable_id, metric_id)
  observation_id              -- the winning row
  value_*, observed_at, recorded_at
  method, confidence, uncertainty_numeric, asserted_by_party_id
  observation_precedence_policy_id      -- WHICH policy row chose this (D22)
  in_breach                             -- against the resolved specification_policy
  cube_numeric                          -- COMPUTED here; never stored as an observation
```

**Precedence is a policy, not a number** — `observation_precedence`, one more kind on D22's resolver. That is what makes "trust supplier dimensions for items we have never measured, but never trust their weight over our scale" a configuration a manager owns rather than a branch in our code. `observation_current` records the deciding policy row, and **this generalises: any projection maintained under a policy must record the policy row that produced it, or the rebuild-and-assert job reports every policy change as drift.** That is a general finding and it is registered as such (D25).

**A derived value is stored only when the derivation was a captured act with its own provenance; otherwise it is computed.** `cube` is computed. A supplier *asserting* a cube is an assertion, and stays expressible.

**Amends.** Principle 3 (restated; the observation family is JSONB-free and asserted so). Principle 4 (amended — see below). Principle 5 (restated from a census of three conventions to a rule: every dimension has exactly one canonical unit, that unit is a **ratio scale**, stored values are integers in it, conversion is exact rational arithmetic never a float factor, and affine units get an offset that never reaches storage — canonical temperature is **millikelvin**, so `AVG`, differences and ranges are meaningful and cold-chain values never go negative. `numeric` is permitted for the preserved entered value: the prohibition is on floating point, not on exact decimal). D5 (extended, not amended). D8 (`discrepancy.observation_id`; kinds `specification_breach`, `uncalibrated_instrument` — we do not reject the reading, we record it and raise the finding). D9 (`challenged`/`challenge_context`/`confirmed` promoted to `observation_event`; **`stock_count` loses its copies** — one mechanism, and the policy that decides *when* to challenge is `count_tolerance_policy`). D10/§6 (the discriminated-union boundary rule). D13 (extended one level up). D19 (`dimension`/`unit` global; `metric`/`metric_code` shared-reference; everything else tenant-scoped). D20 (`measurement` **replaced**; `package.dimensions_source` deleted as a weaker private copy of `method`; `package_type.tare_weight_g` becomes an observation). Q20 and q64, answered. Inbound Tier-0 items 5 and 10 **supplied** — `unit` is the shared vocabulary with six consumers, `device` is the table three tables already referenced.

**`package` dimensions are frozen at seal, not live projections.** The parallel design made `package.length_mm/width_mm/height_mm/gross_weight_g` live projections of `observation_current` and thereby broke a stated invariant: *"a shipped package's dimensions are a historical fact about that consignment and must never change."* The structural defence offered (package instance and package_type class are different observables) answers only the preset-correction case. A retroactive correction against the shipped package's own weight would rewrite the number a freight invoice was computed against. So: `package` dimensions are a **snapshot at seal**, on the same argument as `goods_receipt_line.expected_quantity`, and the projection assertion applies only to unsealed packages.

**Rejects.** Widening the enums in place. A polymorphic `(subject_type, subject_id)`. Typed subject FKs on the observation row itself (correct goals, wrong location — a ten-arm CHECK on the second-largest table plus ~80 bytes of nulls per row forever). A join table between observation and subject (permits zero and two subjects, both meaningless). One `value text` column or a JSONB result. `metric` as an enum (it must carry attributes; contrast q44's `activity_event.kind`, where code branches and an enum is honest). Statistical aggregates on the row (a reefer trip is three observations sharing one event). A `procedure` table. FHIR-style comparators. Folding in `stock_count`. Money as a dimension. Storing `cube`. `confidence` as the sole ranker. A float `factor` column. Milli-degrees-Celsius as canonical. A separate lifecycle table for corrections.

---

### D24 — Containment joins the `stock` key as an arm of place; a package's position is a register

**Decision.** `package_id` joins the `stock` key as an **exclusive alternative to `location_id`**. `package_content` is deleted as a base table and re-created as a view over `stock`. A package's own position is a projection of a new append-only `package_event`. On the supply side, `stock_allocation` gains **two** typed arms — a cell or an `expected_supply` row.

**`location_id` and `package_id` answer the same question at two resolutions.** A package's location is a property of the package, so specifying both on a stock row is redundancy between two independently-writable representations — the drift Q59 named as the default outcome. The key stays at **six dimensions in seven columns**, one of which is a typed exclusive arm.

```
stock                         -- PROJECTION
  id                          -- surrogate; stable, NEVER deleted
  tenant_id, item_id
  holder_location_id          -- \ exactly one
  holder_package_id           -- /
  lot_id, status_id, owner_id
  CHECK (num_nonnulls(holder_location_id, holder_package_id) = 1)
  UNIQUE NULLS NOT DISTINCT (tenant_id, item_id, holder_location_id,
                             holder_package_id, lot_id, status_id, owner_id)
  quantity                    -- <- stock_movement
  weight_g                    -- <- stock_movement.catch_weight_g
  allocated_quantity          -- <- stock_allocation, CELL-BOUND ONLY
  available_quantity          GENERATED (quantity - allocated_quantity) STORED
  resolved_location_id        -- <- holder_location_id | package.resolved_location_id
  site_id                     -- <- resolved_location_id
  last_movement_at, last_counted_at
  INDEX (tenant_id, item_id, site_id) INCLUDE (available_quantity)
        WHERE quantity <> 0   -- partial: dead cells never leave, but never scan

CREATE VIEW package_content AS
  SELECT id, holder_package_id AS package_id, item_id, lot_id,
         quantity, weight_g AS catch_weight_g
    FROM stock WHERE holder_package_id IS NOT NULL;
```

Every capability D6 claimed for `package_content` survives, and two improve. The inbound Tier-0 item ("`package_content` gains `item_id` and typed demand FKs") is not performed — the table is retired instead, `item_id` is in the key, and the demand cause lives on the movement that put the stock there, which is where D10 says causes live. And a sealed carton's manifest acquires **history**: it is the movements that put stock into it up to `sealed_at`, a fact, non-rewritable.

**A package's position is a register, and that is a different CRDT class from stock.** D5 rejected LWW because it silently discards a pick — correct, and about *quantities*. Two concurrent picks are both true; two concurrent assertions that a carton is on P1 and on P2 cannot both be true, and picking a winner is not data loss. **Quantities are counters; relationships are registers; we implement the register ourselves over the same append-only log rather than importing a document CRDT.** Yjs stays ruled out, but not because LWW is wrong here — because we need the loser retained, ordering by *device* clock, and a finding raised, none of which `Y.Map` does.

```
package_event                 -- FACT. Append-only. Partitioned monthly on occurred_at.
  id, tenant_id, site_id
  occurred_at                 -- device clock; orders the register
  recorded_at                 -- server clock; first tiebreak
  client_event_id             -- FK client_event (D25)
  recorded_by_id, work_session_id, authorised_by_id
  work_task_id
  package_id                  -- THE SUBJECT. Always exactly one.
  kind        -- created | placed | contained | observed | identified
              -- | sealed | opened | relabelled | despatched | voided
  parent_package_id, location_id
  sscc, barcode               -- for relabelled: what it became
  assertion_id, asserted_unit_id       -- for identified: the claim this scan matched
  source      -- operator_scan | label | asn | derived | correction
  asserts_placement           -- GENERATED: kind IN (created, placed, contained)
  CHECK (parent_package_id IS NULL OR location_id IS NULL)
  CHECK (kind <> 'contained' OR parent_package_id IS NOT NULL)
  CHECK (kind <> 'placed'    OR location_id IS NOT NULL)
  BRIN (occurred_at); BTREE (package_id, occurred_at DESC, id DESC)
```

Four things in that sketch are load-bearing. **`uncontained` and `moved` collapse into `placed`** — same shape, fewer primitives, total CHECKs. **Location is a property of the root container only**, which is the constraint that answers "moving one pallet of 40 cartons either writes 40 movements or the projection is wrong": moving a pallet writes **one** event, because the cartons' parent did not change. **There is no `from_parent_id`** — a counter needs deltas, a register needs a log; a stored "from" is a second source of truth guaranteed to disagree when events arrive out of order. **`observed` changes nothing** (an EPCIS OBSERVE — a dock-door sighting is not a relocation), and `asserts_placement` being generated is why the projection query cannot forget it.

**The register, concretely.** Order by `(occurred_at, recorded_at, id)` — device clock first, because D5's founding reframe is that a scan is a delta asserted at the place and time the physical event happened. Maintenance is **compare-and-set**, not blind assignment, so a late event with an earlier `occurred_at` loses and does not touch the current value; the projection update is therefore commutative and idempotent, and shuffling a package's event arrival order is a property test. The loser stays in the log and the interval projection is repaired for that package. A losing placement raises `discrepancy.kind = 'containment_conflict'`; an implausible device clock raises `clock_skew` (accepted, never rejected — D5).

```
package                       -- amended
  ... barcode, sscc, sequence, is_mobile, package_type_id ...
  length_mm, width_mm, height_mm, gross_weight_g   -- SNAPSHOT, frozen at seal (D23)
  parent_package_id           -- @projection package_event
  location_id                 -- @projection package_event
  resolved_location_id        -- @projection: the root ancestor's location
  status                      -- @projection: open|sealed|in_transit|delivered
                              --              |emptied|void
  depth                       -- @projection; 0 = root
  placement_event_id, placement_occurred_at        -- the register's version stamp
  CHECK (depth <= 2)

package_containment           -- @projection, interval form
  package_id, parent_package_id, location_id
  valid tstzrange, source_event_id
  EXCLUDE USING gist (package_id WITH =, valid WITH &&)
```

**D6's depth cap is raised from two levels to three, deliberately.** `CHECK (depth <= 2)` admits overwrap → pallet → carton, which the inbound analysis flagged as physically real (shrink-wrap two pallets and the result gets its own SSCC). The resolution fold stays a fixed three-step join and remains non-recursive, which is all D6's cap was protecting. Asserted-unit hierarchies on the assertion side (D21) stay unbounded — cold path — and collapse to this cap at receipt.

`package_containment` uses an **exclusion constraint, not PG18's `WITHOUT OVERLAPS`**. Same guarantee, enforced by DDL rather than a job, available since PG9.x, and it makes the interval idiom identical to D22's effective-dating idiom. The deployment floor is **PostgreSQL 15** (`UNIQUE NULLS NOT DISTINCT` is genuinely required by the `stock` key), and no PG18-only feature is a dependency — which matters because D18 keeps self-hosted BUSL deployment on the table.

**The rule that decides which fact gets written:**

> A `stock_movement` is written when **custody** changes — stock enters or leaves the system, changes holder, status, owner or lot. A `package_event` is written when a **holder changes position**. One physical act produces one fact; a fact may update many projection rows.

Writing forty movements for a pallet move would assert forty physical events that did not happen, and the ledger's whole value is that every row is a real work event with an actor behind it. **Fan-out is confined to the system boundary** — receiving and despatching an ASN'd pallet genuinely writes per-line movements, because goods entered or left custody and PO variance and D14's recall trace both require it. Internal moves are O(1) facts regardless of contents.

**A full-carton pick is a container move**: one `package_event` re-parenting the carton onto the outbound pallet. No movement — custody has not changed. The link to demand is the allocation; the link to the fact of leaving comes at despatch, where the movements carry `fulfilment_line_id`. D14's recall query is unchanged and still complete, because nothing reaches a customer without crossing the boundary. This makes LPN-driven picking and putaway a single scan, which is what LPN systems are for, without a second container concept.

**The cell key travels, and two more dimensions were missing their pair.**

| Dimension | On `stock_movement` | Verdict |
|---|---|---|
| `item_id` | single | **Correct, deliberately.** Changing item is a transformation: two movements. |
| `location_id` | pair | ✓ |
| `status_id` | pair (D4) | ✓ |
| `owner_id` | **absent** | The known D20 bug. Add the pair. |
| `lot_id` | **single** | **The same bug, undetected.** Re-lotting in place changes the cell without moving anything. Add the pair. |
| `package_id` | new | pair by construction |
| `tenant_id` | single | Correct (D18). Assert both sides resolve to one tenant. |

```
stock_movement                -- amended
  id, tenant_id, occurred_at, recorded_at
  client_event_id             -- FK client_event (D25)
  recorded_by_id, work_session_id, authorised_by_id, device_id
  item_id, quantity (> 0), catch_weight_g
  from_location_id, from_package_id, from_lot_id, from_status_id, from_owner_id
  to_location_id,   to_package_id,   to_lot_id,   to_status_id,   to_owner_id
  lot_id GENERATED ALWAYS AS (COALESCE(to_lot_id, from_lot_id)) STORED
  reason, adjustment_reason_id
  fulfilment_line_id, goods_receipt_line_id, discrepancy_id
  CHECK (num_nonnulls(...) <= 1)                   -- cause (D10 corrected)
  work_task_id                                     -- orthogonal
  item_packing_config_id, entered_quantity, entered_unit_id
  CHECK (num_nonnulls(from_location_id, from_package_id) <= 1)
  CHECK (num_nonnulls(to_location_id,   to_package_id)   <= 1)
  -- a populated side must carry the WHOLE key, not just a holder:
  CHECK (num_nonnulls(from_location_id, from_package_id) = 0
         OR (from_status_id IS NOT NULL AND from_owner_id IS NOT NULL))
  CHECK (num_nonnulls(to_location_id, to_package_id) = 0
         OR (to_status_id IS NOT NULL AND to_owner_id IS NOT NULL))
  CHECK (ROW(from_*) IS DISTINCT FROM ROW(to_*))   -- must change something
```

The generated `lot_id` preserves D14's `stock_movement(lot_id, occurred_at)` index and its recall query verbatim; re-lots are found by a tiny partial index on `WHERE from_lot_id IS DISTINCT FROM to_lot_id`. The whole-key CHECKs matter more than they look: under `NULLS NOT DISTINCT` a NULL owner is a *different cell* from the site's entity, so an omitted column would not error — it would silently fork the balance.

**Movements are two-sided in space.** The spine says `quantity` is "signed" *and* gives a from/to pair; those are not compatible, and "stock on hand is the sum of these" is then not a well-defined fold. Corrected: **`quantity` is strictly positive and every movement folds into two cells** — `−quantity` at the from-cell, `+quantity` at the to-cell. A receipt has an empty from-side, a despatch an empty to-side. D5's CRDT property is untouched. This is two-sidedness in *space*, not double-entry in *value*; the financial ledger stays on the not-building list.

**`stock_count` and `discrepancy` gain the missing key columns** — `owner_id` and the holder arm — as the full column set rather than an FK to `stock.id`, because counting a bin that turns out to hold stock is precisely the case where no cell row exists.

**Expected supply is a projection, and allocation has exactly two supply arms.**

```
expected_supply               -- PROJECTION over intentions and assertions.
  id, tenant_id, site_id      -- Keyed and read the same way `stock` is.
  item_id, lot_id, owner_id, status_id
  purchase_order_line_id      \
  transfer_order_line_id       |  exactly one
  asserted_unit_content_id     |
  return_authorisation_line_id/
  refines_expected_supply_id  -- an ASN row refining a PO row
  expected_from, expected_to
  quantity_expected, quantity_refined, quantity_despatched,
  quantity_received, quantity_allocated
  quantity_available GENERATED (expected - refined - received - allocated) STORED
  closed_at
  INDEX (tenant_id, item_id, site_id, expected_from)

stock_allocation              -- amended
  stock_id                    -- \ exactly one
  expected_supply_id          -- /
  CHECK (num_nonnulls(stock_id, expected_supply_id) = 1)
  origin_expected_supply_id   -- set once at binding, never cleared
  bound_at
  allocation_policy_id        -- FK to the immutable value row (D22)
  state -- allocated | picking | picked | packed | fulfilled | short | released
```

**Q60's five arms collapse to two.** `goods_receipt_line` dissolves entirely: under this decision goods on the dock **are** stock, in a receiving LPN at a dock location, so there is a cell. The remaining three are one thing seen from three angles — a quantity of an item promised at a site in a window, with a provenance — so the allocator learns two supply kinds forever and a new source (a production order, a second ASN) is a new arm on the *projection's provenance*, not a branch in the hot path.

**`stock.allocated_quantity` is narrowed by one precise predicate, and the capability it justified is preserved by a second table of the same shape.** On-hand availability is one indexed read on `stock`; available-to-promise over future supply is one indexed read on `expected_supply`. ATP over future supply was never a single indexed read before — it was unbuildable. This is a widening dressed as a narrowing.

**Assertion-sourced expected supply never reaches `stock`.** An allocation against an ASN row records on `expected_supply.quantity_allocated` only, which satisfies D21's rule 3 in its defensible reading and makes rung-5 FEFO cross-dock possible — the ASN row is the only one carrying a lot and a tight window. It is admitted only while its assertion's stance is `in_force`. At receipt, in the same transaction as the movements, allocations are re-pointed at the new `stock_id`, `bound_at` is stamped, and `origin_expected_supply_id` is retained so "this unit was cross-docked against Coles PO 88421" stays answerable.

**The double-count trap is netted once, in the projection.** A PO promises 100; the ASN advises 60 on two pallets; naively ATP reads 160. ASN rows carry `refines_expected_supply_id` and the PO row's `quantity_refined` nets them. This is the argument for a maintained table over a view: **the netting is code, computed once, not a rule every query has to remember.** `quantity_despatched` exists for the same reason on the transfer arm — D16 defines in-transit as *despatched* not *ordered*, and sourcing `quantity_expected` from the order alone would double-count an open transfer against origin stock still on hand.

**Allocation lifecycle, corrected.** An allocation holds its quantity through `allocated | picking | picked | packed`, releasing only at despatch (when the stock leaves the system) or on explicit release. Without this, work-in-progress stock in a sealed outbound package becomes `available` again the moment picking completes — the exact `usage`-predicate cost D16 refused to pay, reintroduced by the back door.

**Amends.** D6 (`package_content` retired; containment columns demoted; depth cap raised to three levels and *enforced*). D12 (`stock_allocation` references `stock_id`; `allocated_quantity` narrowed to cell-bound; the FK is what stops a caller naming a cell that has never existed). D14 (`lot_id` pair; **pack-time serial capture re-anchors on `package`** — `unit_serial(package_id, item_id, lot_id, serial, ...)` — preserving D14's "no reach into stock, allocation or routing", which an anchor on a projection row would have broken). D16 (reinforced: in-transit stays off `stock`, and the derivation becomes a maintained projection, answering q40). D17 (`work_task.package_id` becomes load-bearing; `receptacle_assignment` finally puts picked stock on the books). D20 (`owner_id` pair — the known breakage; the key re-framed as six dimensions in seven columns). D5 (registers clause; two-sided fold). Q59, q40, q60 answered.

**Rejects.** `package_content` authoritative for containment with a job asserting agreement (the assertion can only ever be partial — no status, no owner). `package_id` alongside `location_id` as a true seventh dimension. Forty movements per container move. Five supply arms. A polymorphic supply pair. Allocations against future supply creating `stock` rows for goods that are not there. `expected_supply` as a view or UNION. Merging `package_event` into `stock_movement` (it would put a WHERE clause on the sum D5 exists to keep unconditional). Prior-state columns on `package_event`. Sentinel "no lot" / "own stock" rows instead of `NULLS NOT DISTINCT`. Modelling a package as a location (Odoo's virtual-location idiom — D16's reasoning applies verbatim). Deriving `stock`'s location by joining to `package` on every read.

**One index correction that follows:** inbound Tier-0 item 9 asked for `stock(location_id, item_id)`. It must be **`stock(resolved_location_id, item_id)`** — on `holder_location_id` it would make container-held stock invisible to every commingling and putaway check.

---

### D25 — Status is derived, declared, or forbidden — and projections are enforced by the database

**Decision.** Every status column is exactly one of three things, determined by the **provenance category** of the row it sits on. Projections are enforced by column-level grants, not by convention. One `client_event` registry owns idempotency. One `acceptance` table owns assent.

**Why the review's own test fails.** *"Derive status where the transition is evidence, store it where it is only state"* is not decidable — there is no way to lose the argument that a transition is evidence. It also answers the wrong question: the evidence in a consignment delivery is the carrier's message, not a status log. And it has two branches where the model needs four; the largest class in this schema is statuses that are neither evidence nor state but **arithmetic** over facts that already exist, for which an event table is strictly worse than the column it replaces.

**The rule, and it is total:**

| Provenance / role | Treatment |
|---|---|
| **Fact** | No status. Facts do not have lifecycles. |
| **Intention** | **Declared** — the row owns it, one timestamp per state reached, no history table. Principle 2 defines an intention as mutable; giving it an immutable event log contradicts its own category. |
| **Assertion** | **Forbidden.** The claim is immutable; *our position* is a separate fact (`assertion_stance`, D21). |
| **Finding** | **Declared.** `discrepancy.state` is ours, with D8's `resolved_at`/`resolved_by_id` as its timestamps. |
| **Grouping / projection** (role) | **Derived** — a materialised column, maintained in the same transaction, never written by the application, rebuildable, asserted. |
| **A time-varying relationship** | **Forbidden as a column.** It lives in a fact table and the current value is a projection (D24's containment). |

And the falsifier that stops event tables breeding:

> An event table earns its place only if it has columns a state transition does not. If it would be `(entity_id, from_state, to_state, changed_at, changed_by)` and nothing more, it is a changelog. **An event table's row count is bounded by things that happened in the world; a changelog's is bounded by our own code paths.**

`policy_change` (D22) passes: mandatory `reason`, `authorised_by_id`, and a valid/transaction time split. `assertion_stance` (D21) passes: `reason_code` and `successor_assertion_id`. `work_task_status_history` fails, and is refused.

**One column per source — D12's move, applied to status.** `purchase_order.status ∈ {draft, issued, partially_received, closed, cancelled}` is two facts about two different things in one enum: `draft/issued/cancelled` is *our* intention, `partially_received/closed` is arithmetic over the ledger. Storing both in one column means either the arithmetic overwrites the intention or a human overwrites the arithmetic, and NetSuite is what happens when you do that for fifteen years.

```
purchase_order   state          -- DECLARED: draft | issued | cancelled
                 issued_at, cancelled_at
                 receipt_status -- @projection: none|partial|complete|over
transfer_order   state / receipt_status               -- same split
order            state          -- DECLARED: placed | on_hold | cancelled
fulfilment       state          -- DECLARED: planned | released | cancelled
                 progress       -- @projection: allocations, movements,
                                --   packages, consignment
goods_receipt    status         -- @projection: lines + movements
consignment      status         -- @projection: in-force carrier_status_advice
inbound_shipment (projection)   -- @projection: in-force assertion + stances
package          status         -- @projection: package_event + consignment
work_task        state          -- DECLARED
stock_allocation state          -- DECLARED
discrepancy      state          -- DECLARED (finding)
```

**`order.fulfilment_status` is dropped.** It would have been a third hop (`stock_movement → fulfilment.progress → order.fulfilment_status`), breaking the two-hop cascade cap in the same document that states it. An order has few fulfilments; compute it on read.

**`work_task.state` stays declared, for stated reasons.** Not "a pick lasts ninety seconds" — that would not survive a week-long task. Three reasons: a monotone lifecycle's timestamps **are** its event log transposed (`claimed_at`, `started_at`, `completed_at`, zero extra rows, no greatest-n-per-group); the non-monotone parts already have a home in `activity_event`, which gains `task_claimed | task_released | task_reassigned`; and the hot read is the current state, which principle 6 decides. A status changelog here would be ~20,000 rows/day/site recording transitions three columns already imply.

**Enforcement is the database, not a sentence in a document.** The model already says *"Nothing writes to `stock` directly. Ever."* Postgres can make that a fact.

- **Column-level grants, issued column-wise from the first migration.** The documented trap is that granting table-wide and then revoking one column *does not work*; a single table-wide `GRANT UPDATE` would silently disarm every projection guard in the schema and nothing would fail loudly.
- **Fact tables get no `UPDATE` and no `DELETE` at all.** Append-only stops being a convention. This replaces four separate immutability-trigger families proposed in parallel (on `%_policy`, on `observation`, on assertion tables, and generically) — one mechanism, one catalogue assertion. The one refinement worth keeping is the per-column split on assertion bodies: `raw_*` immutable, `resolved_*` writable until first use, which column grants express natively where a trigger has to be parameterised.
- **Triggers exist for projection maintenance and nothing else.** One function per projection, named for it. Never business rules, defaults, validation or cascades. Unbounded trigger logic is Postgres's own accretion failure mode, and the rule that prevents it has to be written here or it exists nowhere.
- **`SECURITY DEFINER` maintainers require `FORCE ROW LEVEL SECURITY`** on every RLS-protected table involved — table owners bypass RLS by default, and without this the mechanism guarding the projections punches a hole through D18.
- **Registration is checkable.** Each projection column carries `COMMENT '@projection <source>'`, and CI diffs the commented set against the code registry of rebuild functions, bidirectionally. Sourced from **both** the code registry and `record_scheme` (D26), so a generated table does not fail the diff.

```
client_event                  -- THE idempotency registry. Unpartitioned, small.
  tenant_id, client_event_id  -- PRIMARY KEY (tenant_id, client_event_id)
  site_id, device_id, work_session_id
  recorded_by_id / automation_key      -- CHECK num_nonnulls(...) = 1
  app_version, submitted_at, received_at
```

**Every fact table carries `client_event_id` as a plain FK.** Two reasons the per-table `UNIQUE` had to go, both of which void D5's stated non-negotiable property: one physical act produces many facts (a receipt writes a `package_event` and N movements; a cubing scan writes an event and four observations), and **Postgres requires the partition key in any unique constraint**, so on a table range-partitioned by `occurred_at` a per-table `UNIQUE(client_event_id)` silently degrades to per-partition uniqueness and a replay landing in a different month is accepted. One submission inserts one `client_event` row plus all its facts in one transaction; a replay aborts on the PK and rolls back. The `(tenant_id, ...)` PK also closes a cross-tenant unique-constraint oracle. This retires the "derive `uuidv5(scan_event_id, cell_key)` per fan-out row" scheme, which collided whenever two PO lines of one item and lot landed in one LPN.

`recorded_by_id` stays denormalised on every fact row — D11 is emphatic that it is the non-repudiable floor and a join is the wrong shape for an accountability query — and CI asserts the denormalisation agrees with the envelope.

```
acceptance                    -- FACT: a person took responsibility for a state
  id, tenant_id, occurred_at, recorded_at, client_event_id
  accepted_by_person_id       -- ours (D19's global person)
  accepted_by_party_id        -- the counterparty, when external
  authorised_by_id
  goods_receipt_id, consignment_id, discrepancy_id,
  assertion_id, observation_id
  CHECK (num_nonnulls(<five arms>) = 1)      -- a subject union, per D23's rule
  decision                    -- accepted | rejected | accepted_with_exception
  accepted_state              -- the derived value AS AT acceptance. FROZEN.
  basis                       -- manual | policy_auto
  observation_acceptance_policy_id           -- which policy auto-accepted (D22)
  reason_id, note
  rejection_window_expires_at -- statutory clock, computed once and frozen
```

**One acceptance table, five arms, and the arm count is legitimate** because these are alternative identities of the thing being accepted — a subject union where "none" is meaningless — not a cause set. Four designs each invented this table; the frozen `accepted_state` is the piece only one of them had, and it is what answers **open question 13**:

> When a derived status recomputes to a value that contradicts an `acceptance`, the recomputation does not lose — but it raises `discrepancy.kind = 'accepted_state_contradicted'`. The projection is the truth about the facts; the acceptance is the truth about what a person committed to; their disagreement is the finding.

`assertion_stance` (machine ingestion triage, thousands/day, written by a parser) stays separate from `acceptance` (a legally weighty human act, tens/day), on D7's own test. They compose via the `assertion_id` arm.

```
projection_check              -- FACT: we checked a projection against its source
  id, tenant_id, projection_name, scope_kind, scope_id
  checked_at, rows_checked, rows_mismatched, duration_ms
```

Named `projection_check`, not `projection_assertion` — "assertion" now means something specific (D21) and the model would otherwise use one word for three things. Mismatches raise `discrepancy.kind = 'projection_drift'`, so the model's self-consistency lands in the same queue as every other finding.

**`row_audit` is not built.** A generic `(table_name, row_id, before jsonb, after jsonb)` changelog is a polymorphic reference plus an untyped payload — the exact shape this design's own rejected-alternatives list kills a global `events` table for, defended only by "we never read it", enforced by a grep. It is also mostly dead weight: fact tables have no UPDATE or DELETE grant, so their audit rows could only ever be inserts, doubling the write volume of the largest tables in the system to record nothing. The tables that *are* mutable are intentions and policy, and policy already has `policy_change`. If compliance later demands row-level audit for intentions, that is an infrastructure decision with its own justification, not a domain table smuggled in on a principle-3 exception.

**Amends.** D5 (registers; `client_event` registry). D8 (`discrepancy` kinds `+= containment_conflict, containment_variance, projection_drift, clock_skew, accepted_state_contradicted, policy_ambiguous, supply_withdrawn, stock_without_location, specification_breach, uncalibrated_instrument`; **`discrepancy` source arms are capped at five** — `stock_count_id`, `observation_id`, `assertion_check_id`, `work_task_id`, `stock_movement_id`, `CHECK <= 1` — and a sixth requires a recorded decision, because these are causes, not a subject union). D12 (the one-column-per-source move generalised to status). D17 (`work_task.state` confirmed, with the reason replaced; `activity_event` gains three kinds). Cross-cutting §2 (conclusion adopted, test replaced) and §7 (`acceptance` built).

**Rejects.** A global `events` table. Per-entity status-history tables. The review's own evidence test. CQRS with async projectors (the model already chose transactional projections for `stock`; async would make availability and task queues stale on exactly the paths D5 spent its coordination budget to be right). Event-sourcing ceremony — aggregates, upcasting, snapshots (Postgres tables *are* the snapshots; upcasting is what you build when you cannot migrate). System-versioned temporal shadow tables. `row_audit`. An event log for intentions.

---

### D26 — Extensibility: a schema compiler, an outbox, and a plugin surface with no document store

**Decision.** Extensibility decomposes into exactly three things a tenant can want, and each gets one primitive: **data** the product does not model (`record_scheme`, a schema compiler), **decisions** the product makes (the D22 scope lattice — nothing new), and **reactions** to things that happen (`event_subscription` + `outbox`). A fourth — presentation — is not extensibility and never touches the schema. **Three is the budget.** A fourth request for an extension mechanism means one of the three is wrong.

**The premise that changed.** Principle 4 refuses custom-field frameworks because *"adding a column is cheap and migrations are routine"*. That sentence has a hidden subject: cheap **for us**. D18 made multi-tenancy non-negotiable and pointed at a commercial product, and the subject stopped being universal. The refusal was never actually about columns — it was about **untyped attribute soup**. So the boundary moves from *who may add a column* to *what a column may be*.

**A `record_scheme` is a versioned, signed declaration of a typed field set that compiles to real DDL.** There is no attribute table and no pivot. It is a code generator whose input happens to live in a row — Nosdesk's model registry (`sync-models/*.json` → `build.rs` → typed Rust + `SCHEMA_HASH`, build fails on drift) moved from build time to install time.

```
record_scheme                 -- REFERENCE. tenant_id NULL = shipped by us (D19)
  id, tenant_id, key, version
  provenance                  -- fact | intention | assertion | finding  (D21 axis)
  role                        -- reference | grouping                   (D21 axis)
  attaches_to                 -- CLOSED enum of core entities
  cardinality                 -- one | many
  physical_table              -- 'ext_daff_biosec_discrepancy_v1'; immutable
  manifest_source bytea, manifest_hash             -- retained; never queried
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
  CHECK (parameter presence matches field_type)     -- the DB refuses an
  UNIQUE (record_scheme_id, column_name)            --   incoherent declaration
```

The generated table gets a real `tenant_id` FK, a real `NOT NULL` parent FK **with `ON DELETE RESTRICT`** (not CASCADE — a fact scheme's evidence must not be silently destroyed when a receipt is deleted, and grant revocation does not stop FK-machinery cascades), `client_event_id` for schemes with `provenance = fact`, RLS with `FORCE`, column-wise grants derived from `provenance`, and real columns with real types, CHECKs and indexes.

Against the six tests: referential integrity — real FKs. Database enforces invariants — NOT NULL, CHECK, UNIQUE, RLS, and no UPDATE grant for facts, which is the **first time principle 2's categories are mechanical rather than a naming convention**. Survives migrations — a scheme version *is* a migration, run by the same runner into the same ledger. Queryable — `contamination_g` is an integer with planner statistics. Debuggable — `\d` tells you everything; there is no interpreter. Not a language — nine type constructors, no nesting, no `any`, no user types. EAV fails all six; spare-column sidecars fail all six; JSONB-with-a-schema fails all six.

**Evolution is additive in place, otherwise a new version.** Adding a nullable column is metadata-only in Postgres and permitted; narrowing, dropping or adding NOT NULL mints version N+1 with a new table and a generated backfill, and the old table stays. That is `item_packing_config.effective_from`'s reasoning applied to schema, and D8's invariant applied to schema: **a scheme is never rewritten.**

**Ceilings are declared numbers so they are checkable:** 50 schemes per tenant, 60 fields per scheme, 100 tenant-defined metrics. Not because 51 breaks anything, but because the ceiling is what keeps this a schema *extension* rather than a schema *escape*. Salesforce's flex-column pivot is what happens when the ceiling comes off.

**The boundary against D23:** a single unit-carrying number with provenance is a `metric`; a coherent multi-field record is a `record_scheme`. Most "we need a field" requests are actually observations, which is why D23 absorbs the larger share.

```
event_subscription            -- INTENTION
  id, tenant_id, name
  source_table                -- CLOSED enum, settled AFTER the table set is
  site_id, party_id, item_class_id           -- scope filter, D22's dimensions
  delivery                    -- webhook | plugin | outbox_only
  endpoint_id, plugin_id, format_version, state

outbox                        -- FACT. Written in the SAME TRANSACTION as its fact.
  id, tenant_id, occurred_at, enqueued_at
  source_table, source_id     -- a polymorphic pair, and CORRECT here: this row is
                              --   never joined back, only replayed. D10's decisive
                              --   argument (batch loading) does not apply to a queue.
  subscription_id, party_message_id          -- the rendered bytes (D21)
  attempt_count, next_attempt_at, delivered_at, last_error
  INDEX (next_attempt_at) WHERE delivered_at IS NULL

registered_endpoint           -- SSRF guard: targets are registered, not free-form
  id, tenant_id, url, host, auth_kind, secret_ref, verified_at, active
```

**Subscription filtering uses D22's scope dimensions, not a predicate language.** *"Webhook me for discrepancies against supplier X"* is a scope, not an expression. One addressing mechanism across policy and subscriptions — which is the more general primitive by the standing direction's own test, and it removes the second of `decision_rule`'s two justifications.

**The plugin surface reuses Nosdesk's sandbox wholesale** — opaque-origin iframe on a separate registrable domain, `connect-src 'none'`, Comlink over a transferred port authenticated by holding the port, manifest-declared permissions enforced from trusted DB state, signed bundles with trust tiers, egress proxy injecting credentials the plugin never sees. **One deliberate divergence: no document store.** Nosdesk's `plugin_collection_rows.data jsonb` is right for a helpdesk, where a plugin's saved addresses are nobody's business but the plugin's. It is wrong here, because **warehouse plugin data is almost never private to the plugin** — a biosecurity form is evidence in a dispute, a quality result gates a release. In a JSONB collection it cannot be joined to a receipt, reported on, exported into a claim, or given a foreign key: the trapped-in-`measurement` failure one level down. A warehouse plugin declares a `record_scheme` and gets a real table.

**`decision_rule` is deferred, with the threshold written down.** A compiled, type-checked, total predicate (Cedar or equivalent) is a genuinely better answer than an interpreted rule table, and if this is ever built it should be adopted rather than written. But it crosses D22's sharpened D13 line on every clause — `source` holds field names, operators and connectives; `reads_set` holds attribute names as data — and its two motivating examples have both evaporated: "vendor X's goods go to zone 3" is a `putaway` binding, and subscription filtering is a scope. **Revisit when two tenants want different behaviour at the same decision point.** One tenant is a shipped vertical. Writing the threshold down is what stops "wait for real demand" becoming a permanent excuse in either direction.

**Server-side WASM is deferred too, and for a sharper reason.** Nothing in the three axes needs arbitrary computation inside a warehouse transaction, and for the one job it might serve WASM is strictly worse: Wasmtime bounds guests with fuel or epoch interruption, and neither is a **type checker**, which is the property we actually want. Adopting it would buy generality by deferring *what a plugin computes* to runtime — the exact failure the standing direction names.

**Amends.** Principle 1 (corollary: an extension mechanism is a primitive too; three is the budget). Principle 3 (see below). Principle 4 (see below). Principle 2 (categories become mechanical — a scheme's `provenance` drives its grants). D7/q14 (shared as library crates: sandbox, bridge, consent, signing. **Not** shared: the plugin collection store). D19 (nullable-tenant reference shape for `record_scheme` and `metric`). D20 (cited as the consistency check: an org with no schemes and no subscriptions encounters none of this, because there are no rows). Q47 and q70 **materially de-risked** — because tenant extension data lives in enumerable named tables rather than shared blobs, "export everything for tenant T" and "delete T except statutorily retained facts" are generated queries over `record_scheme.physical_table`, not a hunt. That is an argument *for* the compiler that has nothing to do with extensibility.

**Rejects.** EAV. Spare-column sidecars (`ext_int_1..20`) — EAV with extra steps, and the planner's statistics are on `ext_int_7` rather than on `contamination_g`. JSONB + JSON Schema. Schema-per-tenant with tenant-owned DDL (it fails on operations, not capability: if the tenant owns the DDL, *we* cannot upgrade, and the rebuild-and-assert jobs that hold this model together cannot be written once. Database-per-tenant as a *deployment topology* stays available per D18). Interpreted rule tables. Writing our own condition language. Server-side WASM (deferred, seam open). Nosdesk's plugin collection store. Shipped verticals as the *only* answer. A polymorphic `(entity_type, entity_id)` for attaching extension records — the genericity lives in the compiler, not in the row.

---

## Shared primitives

Where two or more designs invented the same thing. Collapsing these is the highest-value output of the cross-check.

| Primitive | Invented by | Resolution |
|---|---|---|
| **`policy_binding`** | 2 designs, incompatible shapes | D22's lattice. The other version had a polymorphic `value_table`/`value_id`, a scalar `specificity`, a `num_nonnulls >= 1` CHECK, and four dimensions. All three are things D22 rejects by name. |
| **`observation` + `metric`** | 3 designs | D23's family. The others had `value bigint` only (cannot hold an ETA, a grade or a boolean), subject arms on the fact table, and a `unit` enum that does not exist. |
| **`acceptance`** | 4 designs | One table, five arms (D25). Cross-cutting §7 asked for one acceptance fact; four designs each built one. |
| **`package_event`** | 3 designs, 3 kind enums | One table (D24): collapsed kinds + parent-XOR-location CHECK + `asserts_placement` + CAS/conflict findings + full D11 attribution + the `identified` kind. |
| **Idempotency** | 3 mechanisms (per-table UNIQUE, registry, derived uuidv5) | One `client_event` registry (D25). It is the only one that survives partitioning and one-act-many-facts. |
| **Unit vocabulary** | named by inbound, built by 1, assumed by 3 | D23's `unit` table. Six consumers: `item.base_unit`, `order_line.entered_unit`, `item_barcode.unit_level`, `stock_movement.entered_unit`, `asserted_unit_content.entered_unit`, `record_scheme_field.unit_id`. |
| **Metric vocabulary** | 3 (one as an inline enum on `assertion_check`) | D23's `metric`. `assertion_check.metric_id` is an FK, so a supplier scorecard and a cold-chain breach report share one vocabulary, one unit resolution and one comparison query. |
| **Provenance decomposition** | 2, independently identical | D23's `asserted_by` / `method` / `ingestion_channel`, plus `transmitted_by_party_id` (the VAN or web-EDI host) which only the assertion design had and which the observation design could not express. Two designs reaching the same decomposition independently is the strongest evidence in the set that it is right. |
| **Non-overlapping intervals** | 2 spellings (`EXCLUDE USING gist`, PG18 `WITHOUT OVERLAPS`) | One idiom: `EXCLUDE USING gist (key WITH =, period WITH &&)`. Used by `%_policy.effective`, `package_containment.valid`, and `order_tolerance_band.quantity_range`. One CI assertion template. |
| **Composite unique index as FK target** | 2 designs, ~8 uses | Named as a documented idiom, with the required schema comment: the denormalised columns these guard (`observable_id`, `result_kind`, `dimension_id`, `kind`) look redundant in the Diesel structs and someone will try to remove them. |
| **Opaque payload retention** | 8 columns, 3 types, 4 designs | One: `party_message.payload bytea`. **The model contains no `jsonb` column at all.** |
| **Immutability enforcement** | 4 trigger families | One: column-wise grants, no UPDATE/DELETE on facts, asserted from `information_schema.column_privileges`. |
| **The projection registry and rebuild-and-assert job** | 5 designs, 5 CI suites | One: `COMMENT '@projection <source>'` + bidirectional code/catalogue diff + `projection_check` + `discrepancy(projection_drift)`. This *is* the invariant register §1 asked for. |
| **The expectation namespace** | 1 named it, 1 built it | `expected_supply` (D24). |
| **The assertion envelope as a shared FK target** | 1 | Generalised: `discrepancy`, `package_event`, `acceptance` and `outbox` point at `assertion`, `observation` and `assertion_check` — the envelopes — never at each body kind. This is what keeps arm counts bounded as new kinds arrive. |

---

## Amendments to D1–D20

**Principle 1** — corollary: an extension mechanism is itself a primitive. Three is the budget (data, decision, event). A fourth request means one of the three is wrong.

**Principle 2** — restated as **two orthogonal axes**. Provenance is closed at four (fact, intention, assertion, finding); role is reference/projection/policy/grouping. Every table registers one of each in code, asserted against `information_schema`. `goods_receipt` is a grouping; `stock` is a projection; `allocation_policy` is policy. None of them is a new category.

**Principle 3** — restated from a census to a rule, and the census's last exception is removed. *Opaque payloads that crossed a party boundary may be retained verbatim for audit and are never queried structurally; compiler input is retained beside its compiled symbol table; anything queryable is a column.* Retention is **bytea**, not JSONB — EDIFACT is bytes, a photographed docket is bytes, and `content_type` says which. `provider_exchange` and the proposed `inbound_message` merge into `party_message`. **CI asserts the model contains no `jsonb` column.**

**Principle 4** — amended, and the refusal strengthened. No EAV, no untyped attribute soup — **and** a tenant may declare a **typed scheme that compiles to real columns** with real types, CHECKs, FKs and indexes, and a **data-defined metric vocabulary whose result types are code-defined**. The justification changes: "adding a column is cheap" was an argument from *our* convenience and D18 voided it. Stated cost: a scheme is a migration and a tenant's 50 schemes are 50 tables we carry forever. Ceilings are declared numbers.

**Principle 5** — restated from a census of three conventions to a rule: every dimension has exactly one canonical unit, that unit is a **ratio scale**, stored values are integers in it, conversion is **exact rational** never a float factor, affine units carry an offset that never reaches storage (canonical temperature is millikelvin), and `numeric` is permitted for the preserved entered value because the prohibition is on floating point, not exact decimal.

**Principle 6** — the batch-loading defence must be *expressible*, so the policy resolver's primary interface is `resolve_batch`, and CI asserts no resolver call appears inside a loop.

**D1** — `consignment.eta`, `.status` and `.price_minor` stop being mutable columns and become projections of the in-force `carrier_status_advice`. `provider_exchange` → `party_message`. Carrier ETA reliability becomes the same query as supplier ASN accuracy.

**D4** — the `stock` key's status arm is unchanged, but every populated side of a movement must now carry the whole key: an omitted `status_id` or `owner_id` under `NULLS NOT DISTINCT` forks the cell instead of erroring.

**D5** — three amendments. (a) **Terminology:** "counts are assertions, not deltas" becomes "counts are **absolute claims** (register semantics)". `stock_count` is *our* fact; "assertion" now means a counterparty statement of record. (b) **Registers:** quantities are counters, relationships are registers, and the register is implemented over the same append-only log. Yjs stays ruled out — we need the loser retained, ordered by device clock, and surfaced as a finding. (c) **Idempotency** moves to the `client_event` registry, because per-table `UNIQUE(client_event_id)` degrades to per-partition uniqueness on a partitioned table and silently voids the property D5 calls non-negotiable.

**D6** — `package_content` is **deleted as a base table** and re-created as a view over `stock`; the claim it never delivered (inbound pallets, non-outbound demand) is now satisfied structurally. `parent_package_id`/`location_id`/`status` demoted to projections of `package_event`. Nesting cap raised from two levels to three (`CHECK (depth <= 2)`) for the overwrap case, and enforced rather than assumed.

**D8** — `discrepancy` gains the missing cell-key columns (`owner_id`, the holder arm), five typed source arms with `CHECK <= 1` (`stock_count`, `observation`, `assertion_check`, `work_task`, `stock_movement`) and a stated cap at five; ten new kinds; `respond_by` populated from `receiving_policy.respond_by_hours`. `discrepancy.state` stays **declared**, not derived — it depends on D7's ticket, and q14 is undecided.

**D9** — `challenged`/`challenge_context`/`confirmed` move from `stock_count` to `observation_event`. `stock_count` **loses its copies**; there is one mechanism, and the policy deciding *when* to challenge is `count_tolerance_policy`.

**D10** — the four-arm limit gains a boundary rule: typed arms are correct for a **discriminated union of one referent** (`observable` at ten, `acceptance` at five) and strain for **cause sets** (`discrepancy` capped at five with `<= 1`). One licensed exception to the anti-polymorphic rule: `outbox.(source_table, source_id)`, because an outbox row is replayed, never joined back.

**D11** — extended to machine actors, narrowly: `assertion_stance`, `assertion_check`, `party_message` and machine-ingested `observation_event` carry `recorded_by_id` **XOR** `automation_key`. `stock_movement` is untouched and keeps its NOT NULL person. Stated explicitly so nobody relaxes D11 on the ledger by analogy.

**D12** — **the known breakage is fixed** (`from_owner_id`/`to_owner_id`), and a second instance nobody had noticed is fixed with it (`from_lot_id`/`to_lot_id` — re-lotting in place changes the cell without moving anything). `stock_allocation` references `stock_id` rather than key columns. `allocated_quantity` is narrowed to cell-bound allocations by an explicit predicate, with `expected_supply.quantity_allocated` carrying the other half — and the allocation holds through `picked` and `packed`, releasing at despatch, so WIP stock does not become available again.

**D13** — the line is sharpened from a judgement into a grep: *data may say where a number applies and how big it is; only code may say what to do with it.* The scoring function stays code, one implementation, testable, versioned — unchanged and reaffirmed. `allocation_policy(scope_kind, scope_id)` → `policy_binding_id` + `effective`. "We ship defaults, not hard-coded behaviour" becomes literally true: our defaults ship as `tenant_id IS NULL` bindings.

**D14** — `customer.min_shelf_life_days`/`_pct` **removed** (they cannot express "different requirements by category"); becomes `shelf_life_policy` on the lattice, with clamping so a site floor raises a customer rule. `lot_id` gains its movement pair, with D14's recall index preserved verbatim by a generated column. `lot.expiry_date` stays as the **accepted operational value**; a supplier-asserted expiry is an assertion and disagreement is `expiry_mismatch`. Pack-time serial capture re-anchors on `package`, preserving "no reach into stock, allocation or routing".

**D16** — **the known breakage is fixed**: `goods_receipt`'s `= 1` CHECK becomes `<= 1`, widened to four sources, with the demand FK on the line. This is the second time this correction has been written; the invariant register is what stops a third. In-transit stock stays off `stock` (reinforced by the `quantity > 0 ⇒ resolved_location_id IS NOT NULL` assertion), and D16's derivation becomes a maintained projection (`expected_supply` from `transfer_order_line`, netted on `quantity_despatched`), which is what makes it allocatable and answers q40.

**D17** — `work_task.state` confirmed as declared, with the reason replaced (a monotone lifecycle's timestamps *are* its event log; the hot read is the current state). `activity_event` gains `task_claimed | task_released | task_reassigned` and its `client_event_id` becomes an FK. `work_task.package_id` becomes load-bearing.

**D19** — the "two policy shapes" claim is falsified and replaced by **three**, enumerated once and asserted over `pg_policy`: (1) **global**, no `tenant_id` at all — `person`, `dimension`, `unit`; (2) **shared reference**, nullable `tenant_id`, readable by all tenants, writable only by the operator role when NULL — `item`, `metric`, `record_scheme`, `policy_binding` defaults; (3) **tenant-scoped**, `tenant_id NOT NULL` — everything observed or operational, including every assertion. Three designs each carefully named their own exception to a two-shape rule; this is that rule replaced rather than exempted.

**D20** — `owner_id`'s movement pair (above). The key re-framed as **six dimensions in seven columns**. `measurement` replaced by the observation family; `package.dimensions_source` deleted as a weaker private copy of `observation_event.method`; `package_type.tare_weight_g` becomes an observation. Catch weight's boundary restated against `stock.weight_g`: **observations describe things; movements describe changes.** `order_line.quantity_tolerance_pct` demoted to an instance agreement.

**Cross-cutting §3** — its `policy_scope` sketch is rejected in three specifics (polymorphic pair, scalar precedence, `order_line` as a scope) and replaced; only the diagnosis survives. **§6** — amended with the union/cause boundary rule. **§2** — conclusion adopted, test replaced.

---

## The invariant register

> **The invariants below have moved to [invariants.md](./invariants.md)**, the single register and the canonical numbering. The entries here are retained as written; the register is authoritative on text, numbering and status.


The artefact §1 asked for. Owner, and how it is asserted. The three known silent breakages are marked — each would have been caught here.

**Structural (CI, from `information_schema` / `pg_catalog`)**

| # | Invariant | Owner | Assertion |
|---|---|---|---|
| S1 | Every `stock` key column except `tenant_id` and `item_id` appears on `stock_movement` as a `from_`/`to_` pair, under an explicit name map | D4, D12, D20, D24 | Catalogue diff. **Caught D20-broke-D12 and the undetected `lot_id` twin.** |
| S2 | Every table naming a stock cell carries the whole key — FK to `stock.id` or the complete column set | D24 | Catalogue diff over a declared list |
| S3 | Every demand/cause CHECK on a grouping table is `<= 1`, never `= 1` | D10 corrected, D16 | Grep `pg_constraint`. **Caught D16-repeats-D10.** |
| S4 | Every table registers exactly one provenance and one role, and the registry covers `information_schema.tables` (sourced from code **and** `record_scheme`) | D21, D26 | Bidirectional diff |
| S5 | Every `@projection` column has UPDATE revoked from the app role and a registered rebuild function; every rebuild function names a live column | D25 | `column_privileges` + bidirectional registry diff |
| S6 | Every fact table has no UPDATE and no DELETE granted to the app role | D25 | `role_table_grants`; attempt both, expect failure |
| S7 | Every trigger maps to a registered projection. No trigger implements rules, validation, defaults or cascades | D25 | `pg_trigger` diff |
| S8 | Every RLS-protected table with a `SECURITY DEFINER` maintainer has `FORCE ROW LEVEL SECURITY` | D18, D25 | `pg_class.relforcerowsecurity` |
| S9 | Exactly three RLS shapes exist, selected by category | D19 amended | `pg_policy` |
| S10 | The model contains **no `jsonb` column** | Principle 3 | `information_schema.columns` |
| S11 | No `policy%`/`%_policy` column has a name in {field, attribute, column, operator, comparator, expression, condition, rule, action, target, sql, script}; the only text columns are `policy_binding.note`, `policy_change.reason`, taxonomy `code`/`name` | D22 | Column-name denylist. **This is D13 made greppable — and it is the check the first draft failed, on `band_axis`.** |
| S12 | `policy_binding` has **no** `num_nonnulls` CHECK; its uniqueness is `NULLS NOT DISTINCT` and non-deferrable | D22 | Assert absence / presence |
| S13 | The `policy_kind` enum, the `%_policy` table set and the Rust `PolicyKind` registry are the same set | D22 | Three-way diff |
| S14 | Every `%_policy` table has `CHECK (kind = ...)`, the composite FK to `(policy_binding.id, kind)`, and the effective-range exclusion constraint | D22 | Catalogue scan, generated from one template |
| S15 | Tenancy is index 0 of every kind's `DIMENSIONS` const | D22 | Compiled registry check |
| S16 | Every consuming column is named `<kind>_policy_id` and is an FK to the value table | D22 | Enumerated register + catalogue |
| S17 | Every foreign key from a non-assertion table to an assertion table is **nullable** (excluding the assertion mechanism's own tables) | D21 | `pg_constraint` ⋈ `attnotnull`. *Scoped — the first draft's version failed on its own `assertion_stance`.* |
| S18 | No table in the assertion set has a `status` or `state` column | D21, D25 | Column-name check |
| S19 | Every fact row has a `client_event` FK, and `fact.recorded_by_id` = `client_event.recorded_by_id` where the person arm is set | D5, D11, D25 | Catalogue + anti-join |
| S20 | Every generated `ext_*` table matches its scheme's compiled fields, its `manifest_hash` recomputes, its parent FK is `ON DELETE RESTRICT`, and it has RLS with FORCE | D26 | Drift job |
| S21 | Every application metric-code literal appears in the reserved seed with `reserved = true, tenant_id IS NULL` | D23 | Grep |
| S22 | No canonical unit has a non-zero offset or a factor other than 1/1 | Principle 5 | Two count-zero queries |
| S23 | No resolver call appears inside a loop | Principle 6, D22 | AST check |

**Job-asserted (the rebuild-and-assert cycle; every mismatch is a `discrepancy`, never an error)**

| # | Invariant | Owner |
|---|---|---|
| J1 | `stock.quantity` = the signed two-sided fold of `stock_movement` over the cell key | D5, D12, D24 |
| J2 | `stock.weight_g` = the same fold over `catch_weight_g` | D20 |
| J3 | `stock.allocated_quantity` = active cell-bound allocations only | D12 narrowed |
| J4 | `expected_supply.quantity_allocated` = active allocations against it | D24 |
| J5 | `stock.resolved_location_id` = holder location, or the holder package's | D24 |
| J6 | `package.parent/location/resolved_location/status/depth` = the fold of `package_event` in `(occurred_at, recorded_at, id)` order; replay in **any** arrival order is identical | D24 |
| J7 | `stock.quantity > 0 ⇒ resolved_location_id IS NOT NULL` → `stock_without_location` | D24 |
| J8 | `expected_supply.quantity_refined` = the sum of refining rows (the ATP double-count guard) | D24 |
| J9 | No active allocation references a closed `expected_supply` → `supply_withdrawn` | D24 |
| J10 | `observation_current` is exactly rebuildable from `observation` + the recorded precedence policy row | D23 |
| J11 | A counterparty-asserted observation enters `observation_current` only with an `acceptance` | D23, D25 |
| J12 | `package` dimensions equal `observation_current` **for unsealed packages only** | D23 |
| J13 | Every policy value version has a matching `policy_change`, anti-joined both ways | D22 |
| J14 | Every scope FK **and every value-table FK** resolves within the binding's tenant; an operator-shipped binding references only shared rows | D22, D18 |
| J15 | No binding names a node inconsistent with a coarser node on the same dimension (item ∉ item_class, zone ∉ site, party ∉ party_class) | D22 |
| J16 | Zero `discrepancy` rows of kind `policy_ambiguous` after the suite | D22 |
| J17 | `assertion.supersedes` is acyclic; at most one in-force assertion per `(tenant, author, kind, author_reference)` | D21 |
| J18 | `measurement`-style promotion: a counterparty observation's value still equals its assertion's | D21, D23 |
| J19 | Truncate every assertion table, rebuild `stock` and `allocated_quantity`, assert byte-identical | D21 rule 3 |
| J20 | Every `tenant_id` on a fact agrees with its subject's and its event's | D18 |
| J21 | Per-tenant ceilings hold (50 schemes, 60 fields, 100 metrics) | D26 |
| J22 | Golden snapshot: adding a scope dimension changes no existing resolution without an explicit `RESOLVER_VERSION` bump | D22 |
| J23 | The five ingestion channels produce identical content columns | D23 |
| J24 | Projection cascades are at most two hops from the originating fact | D25 |

---

## What remains open

> **Questions raised in this document have moved to [open-questions.md](./open-questions.md)**, the single register and the canonical numbering. The entries below are retained as written; the register is authoritative on status.


**Decided-adjacent, needing one line each before build:**

73. **The `event_subscription.source_table` enum must be settled last.** Three of its members are being renamed by these six decisions. Writing it before the table set is final guarantees it is wrong.
74. **`stock` cardinality moves from location-grain to LPN-grain, and rows are never deleted.** The partial index (`WHERE quantity <> 0`) handles the availability path. If it ever bites, a `stock_by_item_site` roll-up is the answer — and it should not be built until it does.
75. **The receiving screen's query must be written and `EXPLAIN`ed once, after the merge, as the acceptance test for it.** Six designs each added joins to that path (`goods_receipt → inbound_shipment → in_force_assertion → asserted_unit → asserted_unit_content`, plus observation resolution, plus a policy resolve per line, plus `client_event`). No design could measure the aggregate; the merged one must.
76. **Partitioning plan for `observation`, `stock_movement`, `package_event` and `activity_event`** before the first large tenant, not after. Retrofitting partitioning onto a table with a unique constraint is exactly the migration that would break the `client_event` guarantee.
77. **`row_audit` is dropped — is "who changed the ship-to address" a real requirement?** If yes it comes back as an infrastructure decision with its own justification, scoped to mutable tables only, and never as a projection source.

**Genuinely unresolved:**

78. **Taxonomy re-parenting silently changes specificity.** Moving `chilled_dairy` under a different parent changes its depth and therefore which bindings win, for every future resolution and for any replay. Past *decisions* are safe (they hold value-row FKs); "what should have applied" becomes wrong. The UI must surface "this move changes N active resolutions" — but the semantic hazard is real and unsolved.
79. **The per-kind precedence order is the highest-leverage undocumented number in the system.** Counterparty over product for shelf life, product over space for putaway — both defensible, both choices, and a manager who assumes the wrong one will misconfigure confidently. The explain UI must ship **with** the resolver, not after it.
80. **Resolver cache invalidation.** A missed invalidation means the floor runs on stale weights and nothing detects it. `policy_change.id` as a monotonic epoch per `(tenant, kind)`, checked on every resolution rather than a TTL — but it needs designing, not assuming.
81. **Q65 reaches `observable` and `policy_binding`.** If a 3PL client is an owner scope inside a tenant, `observable` needs `owner_id` by D19's own argument one level down (a 3PL client's case pack for the same item legitimately differs from ours). Check before the registry's unique indexes are built.
82. **Q66 (3PL storage billing) is now buildable and still undecided.** `package_containment` gives pallet-days; `order_tolerance_band`'s fenced pattern is a rate card's shape. A rate card needs a *second* axis, which is a decision to record, not a column to add. That pressure will come.
83. **Cross-tenant supplier benchmarking (q67)** is now silently decided by D19 for `party_assertion_scorecard` too. Make it a decision, not a side effect.
84. **Bitemporal queries are easy to write backwards.** *"What did the pallet weigh on Monday"* and *"what did we believe on Monday"* differ by one predicate, and getting it wrong in a chargeback dispute is worse than not having the capability. Needs a small set of named, tested query helpers, not a convention.
85. **The falsifier for new event tables is gameable** by adding a decorative column. The provenance-category rule is what actually decides; reviewers must be told to apply it first.
86. **The closed decision-point set does not exist yet, deliberately** — but if `decision_rule` is ever built, `acceptance_gate` is the one point that touches a rebuildable projection, and any projection a tenant rule can influence must record the deciding rule version or the assert job reports false drift. Decide that at the same time, not after.
87. **`assertion_stance.reason` and `policy_change.reason` are only as good as what gets typed into them.** Mandatory-not-null gets "update". The mitigation is product, not schema: short reason-code lists per kind plus free text, shown in the explain panel where absence is embarrassing.
88. **The invariant register itself needs an owner and a home.** It is the artefact that would have caught all three known breakages, and it is now 47 entries across six decisions. If it lives only in this document it will erode exactly as principle 3's census did. It should be the CI suite, with this table generated from it — not the other way round.
