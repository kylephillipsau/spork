# D24 (supply side): expected supply, netting and pre-receipt allocation

**Status: proposed, not adopted.** Generated 2026-08-01 by a multi-agent research
pass over five streams (available-to-promise, cross-docking, supply commitment and
slippage, netting across PO/ASN/receipt, and in-transit with ownership in motion),
each then adversarially tested against the adopted decisions D1-D25. 104 invariant
risks were flagged before synthesis.

Closes the half of D24 deliberately deferred at adoption. Read the Verdict first:
the deferred sketch has four defects and two false claims, and one of them is a
bug **in the invariant register** (J8 as written codified the double-subtraction
it was meant to catch).

**Provenance caveat.** Per-stream research and the raw assessments are in the run
journal at `.claude/projects/*/subagents/workflows/wf_c7a237e5-28d/journal.jsonl`.
Claims about SAP, Oracle, Microsoft, NetSuite, Odoo, ERPNext and OFBiz internals
are second-hand from public documentation and issue trackers.

---

# D24 (supply side) — expected supply, netting and pre-receipt allocation

*Proposed 2026-08-01. Closes the half of D24 deferred at adoption. Depends on D21, D22, D23, D24 (containment) and D25, all adopted.*

## Verdict — does the deferred sketch survive contact with real requirements?

Mostly. The two load-bearing choices are right and independently reinvented by the market leaders: a **row per promised supply** (Oracle `INV_SUPPLY`, 25 years in production) rather than a quantity column per supply-source × owner combination (SAP's `MARC.TRAME` / `UMLMC` / `MSLB.LBUML` / `MSKU.KUUML` proliferation); and a **refinement link plus a reduction counter** rather than depleting the parent (SAP `EKES.REF_ETENS` + `EKES.DABMG`, "Quantity Reduced (MRP)"). NetSuite states our netting rule almost verbatim. Keeping the parent's original promise intact is strictly better than every vendor that depletes it, because supplier-promise accuracy stays answerable after the fact — which is the whole reason D21 keeps the assertion.

Six things in the sketch are wrong. Four are defects, two are false claims.

**1. The generated column double-subtracts. This is a bug, not a refinement.** `quantity_available GENERATED (expected − refined − received − allocated)` goes negative the moment a receipt lands against a refining row. PO promises 100, ASN advises 60, 58 arrive: the PO row reads `100 − 60 − 58 = −18`. The same 58 units are subtracted twice, once as suppression and once as consumption. Refinement is a **transient suppression released as the refining row is consumed**, not a constant copied from the child's advised quantity. J8 as written in the register — "`quantity_refined` = the sum of refining rows" — *is* the bug, and it is the invariant that was supposed to catch it.

**2. `quantity_available` is not available-to-promise, and the decision text claims it is.** *"ATP over future supply is one indexed read on `expected_supply`"* is true for the sum of unclaimed supply and false for the question order entry asks. Microsoft defines ATP as *the minimum projected on-hand from a date through the end of the horizon* — a running minimum over a date-ordered fold. Their worked example: on-hand 20, demand 15 on the 4th, supplies 1 and 3 later; ATP on the 1st is 12, not 17. No per-row residual produces 12. A PO for 100 landing day 30 against demand for 100 due day 5 nets to zero per row and the promise gets made. Rename the column, delete the claim, and say what the read does answer.

**3. `goods_receipt_line` does not "dissolve entirely".** It is one of the three adopted cause arms on `stock_movement` (D24), and D23 cites `goods_receipt_line.expected_quantity` as its freezing precedent. Lifting the supply side out of mechanism-design.md unedited would delete a table two adopted decisions depend on. The two-arm collapse has to be re-derived without that claim — and it survives fine, because `goods_receipt_line` was never a *supply* arm.

**4. Allocating against an ASN-sourced row breaks D21 rule 3 as adopted, and the sketch hedges instead of amending.** *"Records on `expected_supply.quantity_allocated` only, which satisfies rule 3 in its defensible reading"* — a `stock_allocation` row **is** a commitment, whatever column it decrements. The hedge is doing all the work. Rule 3 must be restated in writing or the ASN arm must not be allocatable. Silently narrowing an adopted rule is the failure mode the invariant register exists to catch, and it has already happened three times.

**5. `expected_supply` cannot register one provenance under S4**, and it is a build failure on day one. It holds rows sourced from intentions (PO, transfer, RA lines) and from assertions (ASN content), governed by *different rules* — rule 3 binds one arm and not the others. `stock` gets away with mixed sourcing because D12 separates its columns by source; `expected_supply` mixes at row grain and does not inherit that precedent.

**6. Every finding the supply side needs to raise has nowhere to land.** D25 caps `discrepancy` source arms at five with *"a sixth requires a recorded decision."* Of `supply_withdrawn`, `supply_overdue`, `supply_over_refined` and `receipt_unmatched`, only the last has an arm. That cap is working exactly as designed, and it must be discharged here rather than widened in passing. Note also that `supply_withdrawn` appears in mechanism-design's D25 kind list and was **dropped** from the adopted list — correctly, because its subject did not exist. It comes back with this decision or J9 has no kind to raise.

Two smaller things the sketch is silently wrong about: `refines_expected_supply_id` must be one level only (or J8 becomes a recursive fold and the transfer arm invites chaining legs through it), and the projection rebuild must be an **identity-preserving upsert**, not truncate-and-regenerate, because live allocations hold `expected_supply_id` as a foreign key and ASN supersession is routine rather than rare.

Everything else holds. Two arms on `stock_allocation` is correct and independently arrived at by Oracle (`ORIG_SUPPLY_SOURCE_*` is `origin_expected_supply_id`; `CROSSDOCK_CRITERIA_ID` is `allocation_policy_id`). The maintained table beats the view for two independent reasons — netting computed once, and a lockable gate row that Postgres needs in the absence of gap locks. And Odoo is the control experiment for the alternative: `virtual_available` as a non-stored read-group over `stock_move` with a status predicate is the slowest field in Odoo inventory and the reason product lists time out.

---

## The design

### `expected_supply` — a promise of goods that have not arrived

```
expected_supply               -- PROJECTION (role). Folds intention + assertion + fact.
  id, tenant_id
  site_id                     -- the DESTINATION. @projection from the source line.
  item_id (NOT NULL)          -- unresolvable content produces a finding, not a row
  owner_id, status_id         -- what the goods will be, on arrival

  purchase_order_line_id      \
  transfer_order_line_id       |  exactly one (see below — this is `= 1`, deliberately)
  asserted_unit_content_id     |
  return_authorisation_line_id/
  CHECK (num_nonnulls(<the four>) = 1)

  refines_expected_supply_id  -- an ASN row refining a PO row. ONE LEVEL ONLY.
  CHECK (refines_expected_supply_id IS NULL OR asserted_unit_content_id IS NOT NULL)

  advised_lot_code            -- RAW, the supplier's string. Never resolved to lot_id.
  advised_expiry_date         -- RAW. What FEFO cross-dock sorts on.
  expected_from, expected_to  -- a window, not a point: a dock appointment has two ends
  date_confidence             -- advised | ordered | inferred | none

  quantity_expected           -- @projection, per arm (see the source table)
  quantity_refined            -- @projection: SUM of open children's outstanding
  quantity_received           -- @projection: receipts naming this row OR a child
  quantity_closed_short       -- @projection: the source line's agreed release
  quantity_allocated          -- @projection: active allocations naming this row
  quantity_outstanding  GENERATED (expected - refined - received - closed_short) STORED
  quantity_promisable   GENERATED (outstanding - allocated) STORED

  closed_at, closed_reason    -- received_in_full | short_closed | superseded
                              -- | cancelled | expired | withdrawn
  derived_from_assertion_id   -- nullable (S17)
  receiving_policy_id         -- which policy row chose status_id (D22's lifted rule)
  allocation_policy_id        -- which policy row admitted this arm

  UNIQUE (tenant_id, purchase_order_line_id)        -- one partial unique per arm.
  UNIQUE (tenant_id, transfer_order_line_id)        -- These are the idempotency guard
  UNIQUE (tenant_id, asserted_unit_content_id)      -- for message reprocessing, and
  UNIQUE (tenant_id, return_authorisation_line_id)  -- the anchor the netting needs.

  INDEX (tenant_id, item_id, site_id, owner_id, status_id, expected_from)
        INCLUDE (quantity_promisable) WHERE closed_at IS NULL
  INDEX (tenant_id, expected_to) WHERE closed_at IS NULL AND quantity_expected > 0
  INDEX (derived_from_assertion_id) WHERE derived_from_assertion_id IS NOT NULL
```

**It is read the way `stock` is read; it is not keyed the way `stock` is.** The sketch's *"keyed and read the same way"* is false in the respect that matters most. A `stock` row is a **cell** — a coordinate with at most one row, `UNIQUE NULLS NOT DISTINCT` over the whole key. An `expected_supply` row is a **promise**, and two PO lines from different POs for the same item at the same site are legitimately two rows. Uniqueness is over the *source document line*, never over `(item, owner, status)`. S2 (every table naming a stock cell carries the whole key) and D24's dead-cell reaper both apply to `stock` and **not** to this table.

**No `lot_id`, ever — and that answers q32 with "no".** A supplier's lot code is a claim in the author's vocabulary (D21 rule 5). Minting a `lot` row from it would make an assertion project into our reference data, populate expiry reports with goods that do not exist, and put a resolved FK under a live allocation that a re-resolution could silently re-point. Instead the raw code and the advised expiry sit on the projection as non-authoritative columns. FEFO cross-dock sorts on `advised_expiry_date`, which is exactly the number FEFO needs. The real `lot` is created at receipt from the goods, and advised-versus-actual is an `assertion_check` raising `advised_lot_mismatch`.

**`= 1` on the four arms, not `<= 1`, and S3 does not apply.** S3 scopes to demand/cause CHECKs on *grouping* tables, and it has fired twice for the right reason: causes are relationships that merely happen to be exclusive today. These arms are D23's other case — alternative identities of one referent. A projection row with no source is not merely unusual, it is **unrebuildable**, and relaxing to `<= 1` would destroy per-column rebuildability. Scope S3 explicitly so it does not fire here. A fifth arm requires a recorded decision, the same rule D25 wrote for `discrepancy`.

**There is no `quantity_despatched` column, because the transfer arm's row is minted at despatch.** D16 defines in-transit as *despatched*, not ordered. An approved-but-unpicked transfer's goods are still on the floor at origin and still in origin `stock.available_quantity`; minting the destination row at approval promises the same units at both ends, which is precisely the NetSuite defect (approving a transfer order raises destination on-order while the stock is physically on hand at source). Minting at despatch makes the double count **structurally unrepresentable** rather than netted, drops a column, and keeps the measure count at five — inside D365's published ceiling of nine distinct measures, which is the empirical answer to how many arms availability can have. In-transit is then `quantity_outstanding` on transfer-arm rows, which is SAP's `GLMNG − WEMNG` reached from the other side.

#### The five rebuild sources — one per column, per D12

| Column | Source |
|---|---|
| `quantity_expected` | PO / RA arm: `line.quantity`. Transfer arm: fold of despatch movements (empty to-side) naming that `transfer_order_line`. ASN arm: `asserted_unit_content.quantity`, admitted only while the assertion's stance is `in_force`. |
| `quantity_refined` | `SUM(child.quantity_outstanding)` over open children — **not** children's `quantity_expected` |
| `quantity_received` | fold of `goods_receipt_line` rows naming this row **or any row refining it** |
| `quantity_closed_short` | `line.quantity_closed_short` on the PO / RA arm; 0 elsewhere |
| `quantity_allocated` | `SUM(quantity)` over active allocations naming this row |

Five columns, five sources, five assertions. `quantity_refined` is the model's **only** projection column whose rebuild folds its own table rather than a fact log, so it is a depth-capped topologically-ordered pass rather than a fold, and that must be stated rather than assumed.

#### The netting, worked

PO promises 100 → parent: expected 100, outstanding 100.
ASN advises 60 → child row, parent `quantity_refined` = 60, parent outstanding 40, child outstanding 60. **Total promisable 100, never 160.**
58 arrive against the ASN → child received 58, child outstanding 2; parent `quantity_refined` falls to 2 *and* parent `quantity_received` rises to 58 (the receipt names a child of this row), so parent outstanding = 100 − 2 − 58 = 40. **Total promisable 42** — 2 still on the truck with a lot and a window, 40 never advised. Correct.

The promise is **partitioned** between parent and children, never summed twice, and the partition is assertable per row. Notice the receipt does *not* credit the child alone: the refinement is released and the consumption is recorded in the same transaction, which is what stops the −18.

ASN advises 120 against a PO of 100 → refined 120, outstanding −20. **Allowed.** `supply_over_refined` (D5: never clamp a counterparty's true statement; D8: the impossible state is the output).

### `stock_allocation` — two supply arms

```
stock_allocation              -- INTENTION (amended)
  fulfilment_line_id          -- the demand. Unchanged, single arm.
  stock_id                    -- \ exactly one (D23's discriminated-union rule:
  expected_supply_id          -- /  an allocation bound to nothing is meaningless)
  CHECK (num_nonnulls(stock_id, expected_supply_id) = 1)
  origin_expected_supply_id   -- set once at binding, never cleared
  binding_kind    GENERATED   -- on_hand | pre_receipt | in_transit, from which
                              --   origin arm is set. Never written freely.
  bound_at, expires_at
  firm, firmed_at, firmed_by_id, firmed_reason  -- human | strategy | wave_release
  allocation_policy_id
  state -- allocated | picking | picked | packed | fulfilled | short | released

  INDEX (expected_supply_id) WHERE expected_supply_id IS NOT NULL
  INDEX (expires_at) WHERE expected_supply_id IS NOT NULL AND state = 'allocated'
```

**Allocations are never migrated by an ingestion event.** The sketch handles the receipt migration and is silent on the earlier one: an allocation bound to the PO row when an ASN lands two days later. Every vendor rebinds; we do not, and the reason is rule 3. An ASN arriving must not rewrite one of our commitment rows — that is the supplier's message authoring our intention. Instead the ASN is an **input to the allocator**, and the allocator's output is our act, stamped with `allocation_policy_id` and a fresh `bound_at`. In practice a cross-dock allocation is *created* against the ASN row, because the allocator runs after the ASN lands with the lot and the window in hand; an older PO-bound allocation stays where it is and is satisfied by whatever arrives. The re-allocator — cold-path, advisory, non-blocking (D12) — may release and rebind, and `firm` is what tells it what it may not steal. No rebinding mechanism, no event log for intentions, no rule-3 stretch.

**Partial receipt splits the allocation, and it is best-effort.** The `= 1` CHECK forbids a row that is half cell-bound, so an allocation of 100 against an ASN row where 60 arrives becomes two rows — 60 on `stock_id`, 40 still on `expected_supply_id` — with `origin_expected_supply_id` preserved on both. This is the case that will actually occur and the sketch does not state it. Critically: **the movements commit whether or not the split does.** Rolling back a receipt because an intention could not be rewritten would be the clearest possible D5 violation in the whole model. A failed re-point raises a finding; the goods are on the dock either way.

### Cross-dock — zero new policy kinds

The cross-dock capability needs a supply window, a shelf-life tolerance, and a statement of which supply arms may be allocated against. All three are typed scalars, and they fold into the **existing** `allocation_policy` value row rather than a twelfth D22 kind:

```
allocation_policy             -- POLICY (amended)
  ... weight_rotation, weight_travel, weight_access, rotation_tolerance_days ...
  consider_expected_supply    -- one boolean, not a company-wide checkbox (D365's is)
  allow_supply_purchase_order \
  allow_supply_transfer_order  |  one typed boolean per capped arm
  allow_supply_asserted_unit   |
  allow_supply_return          /
  window_before, window_after  -- canonical intervals; the after-window applies
                               --   to expected supply only. Without these the
                               --   allocator has no upper bound on how much
                               --   future to consider.
  crossdock_min_window, crossdock_max_window
  crossdock_expiry_tolerance_days
  revalidate_on_receipt
  rotation_key_expected        -- expected supply rotates on arrival date, and
                               --   backward-from-required-by, not FEFO
```

**Not an ordered supply-source child table.** D365's cross-docking template has one and it is a straight S11 violation: `sequence` is an ordering of steps and `supply_source` is a column whose value names an arm. D22's single fenced extension permits one child keyed on a *numeric axis with `[lower, upper)` bands* — a sort list is not that. Booleans per arm express the same thing as typed scalars, and adding a fifth arm later is a column rather than a new row type. The contrast worth recording in D22: `fill_sequence ∈ {inventory_only, crossdock_only, prioritize_inventory, prioritize_crossdock}` names four **code-implemented strategies** and passes cleanly; an enum naming columns does not. That is the sharpest available illustration of where the line sits.

**Revalidation at receipt may refuse, and refusal never blocks.** If the truck is late enough that the window is missed, the goods still land — the cross-dock allocation releases, putaway proceeds, and a finding is raised. D365 documents the opposite failure (*"inventory transactions are not unregistered when cross-dock work is canceled"*), which is exactly the class of silent corruption D8 exists to convert into a row.

### Supersession, short arrival, and never arriving

| Event | Mechanism |
|---|---|
| **ASN replaced or cancelled** (X12 353 codes 01/04/05; the assertion's stance leaves `in_force`) | Every `expected_supply` row derived from that assertion closes with `closed_reason = 'superseded' \| 'cancelled'`; the parent's `quantity_refined` releases by the closed rows' outstanding in the **same transaction**. Rows derived from the replacement are created by **identity-preserving upsert** keyed on `(tenant_id, asserted_unit_content_id)` — never truncate-and-regenerate, because live allocations hold these ids. Allocations against a closed row are **not auto-released**: `supply_withdrawn` is raised and a human or the re-allocator decides. A counterparty's retraction must not silently un-promise a customer order. |
| **Duplicate resend** (code 07) | J17 already forbids two in-force assertions per `(tenant, author, kind, author_reference)`; the per-arm unique index is what stops a second set of rows. |
| **Arrives short** | `quantity_received` < `quantity_expected` on a closed row. Nothing special: the residual stays outstanding until the row closes or the fence passes. |
| **Arrives over** | `quantity_received` > `quantity_expected` permitted; `quantity_promisable` goes negative; `over_receipt`. Tolerance is an instance agreement on the line (D22), the resolver consulted only when null. |
| **Blind receipt** | A `goods_receipt_line` with no `purchase_order_line_id` and no `asserted_unit_content_id` names no `expected_supply` row, so it nets **nothing**. The PO row stays open and the overdue sweep raises it. That is D8 behaving as designed rather than a matching heuristic quietly closing the wrong row. `receipt_unmatched` makes it countable, which is a supplier-compliance metric nobody will compute otherwise. |
| **Never arrives** | The gap the whole comparison set gets wrong. D365's scheduled supply simply stops appearing once the horizon rolls past it — projected on-hand silently drops with no event anywhere, and any commitment made against it is now unbacked. Here: a sweep over `(tenant_id, expected_to) WHERE closed_at IS NULL` past `expected_to + receiving_policy.supply_overdue_hours` closes the row with `closed_reason = 'expired'` and raises `supply_overdue`; if allocations reference it, `commitment_unbacked`. Both carry `counterparty_party_id` so they aggregate into the supplier scorecard. |

`expected_supply` rows are **retained after `closed_at`**. D24's dead-cell reaper applies to `stock` only. The overdue trail, the transit-variance history and *"this unit was cross-docked against Coles PO 88421"* all live in closed rows.

---

## The availability read path

**The honest answer: it is two index-only range scans, and D12's guarantee needs restating because the version in the document was never true.**

```sql
-- (a) on hand, promisable now
SELECT sum(available_quantity) FROM stock
 WHERE tenant_id = $1 AND item_id = $2 AND site_id = $3
   AND owner_id = ANY($4) AND status_id = ANY($5)
   AND quantity <> 0;

-- (b) incoming, promisable against a need-by date
SELECT sum(quantity_promisable) FROM expected_supply
 WHERE tenant_id = $1 AND item_id = $2 AND site_id = $3
   AND owner_id = ANY($4) AND status_id = ANY($5)
   AND expected_from >= $6 - $7 AND expected_from < $6 + $8
   AND closed_at IS NULL;
```

Neither is a single row read, and (a) never was: it is a fold over the cells of one item at one site, across lot × status × owner × holder. What was actually load-bearing in D12 is this, and it survives intact:

> **No join, and no aggregate over a fact table, on the availability path.**

Two things must change on the adopted schema for that to be true, and both are corrections rather than extensions.

**`owner_id` and `status_id` must be in the index key.** The adopted D24 index is `(tenant_id, item_id, site_id) INCLUDE (available_quantity) WHERE quantity <> 0`. Neither owner nor status is in it, so every candidate row needs a heap fetch to evaluate the filter and the `INCLUDE` buys nothing. Worse, the failure mode is silence rather than slowness: a tenant with one owner never notices, and a 3PL tenant promises a vendor's units. This is documented designed behaviour in Odoo — consigned stock is excluded from valuation and **included** in forecasting, and `_get_available_quantity` takes `owner_id` as an *optional* argument defaulting to None. D20 argued that for a single-owner tenant the column is a constant and the index behaves as though it were not there; D24 then wrote the index without it. Put both in, and make `owner_id` a **mandatory** argument of the availability function so the Odoo defaulting bug is unrepresentable.

**`inventory_status.is_available_for_allocation` must not be joined.** Resolve the allocatable status-id set once per request in code and pass it as `= ANY`. A join on the availability path is the thing D12 exists to prevent.

The residual cost is honest and small: with `= ANY` on two mid-key columns the planner issues (owners × statuses) range scans rather than one. For the overwhelming majority of tenants that product is 1 and the scan is contiguous. For a 3PL it is a handful. That is the trade D20 already made for `stock`, applied consistently.

**What this read does not answer, and what we are not building.** It answers *"how much of this item, in an allocatable state, that we own, is uncommitted at this site — on hand, and promised inside the policy window."* It does **not** answer *"when can I promise 150."* Date-qualified ATP is a running minimum over a forward horizon, and no vendor computes it from row-per-supply storage: SAP's own release notes call the aggregate-on-read version *"time-intensive when processing large volumes"*, bolted on a liveCache buffer, retired it in S/4HANA 2023, and shipped it again in 2025. Oracle's answer is `MSC_HVGOP_AGGREGATED_SUPPLY` with literal `QTY1..QTY960` columns. Microsoft's is a separate microservice.

Ours is: **not built, deliberately, and named so it is a decision.** A bucketed running-minimum projection is a third hop (`observation → observation_current → expected_supply → buckets`) and breaks J24's two-hop cap; maintaining a running minimum transactionally rewrites every bucket from a change to the horizon end; and the async escape is refused by name in D25. Building it needs **both** exceptions argued, not one. Until then the floor never asks *"when can I promise"*, and the allocator's date question is answered by the window on the read above — which is a range predicate, not a fold over time.

The column is therefore named **`quantity_promisable`**, not `quantity_available`. Two near-identical names with different meanings on two same-shaped tables is exactly how availability logic starts disagreeing with itself (D14's own argument against `lot.on_hold`).

---

## Invariants created

Amending the register. J8 and J9 are **restated**; J3's predicate is written down rather than implied.

**Job-asserted**

| # | Invariant | Owner |
|---|---|---|
| J3 *(restated)* | `stock.allocated_quantity` = active allocations **with `stock_id` set**, state ∈ `{allocated, picking, picked, packed}`. The predicate is enumerated, not adjectival. | D12 narrowed, D24s |
| J4 | `expected_supply.quantity_allocated` = the same fold over allocations with `expected_supply_id` set | D24s |
| J8 *(replaced)* | Per row: `quantity_expected = quantity_refined + quantity_received + quantity_closed_short + quantity_outstanding`. **This is the check that catches the double-subtraction**; the old formulation was the bug. | D24s |
| J9 *(restated)* | `parent.quantity_refined = SUM(child.quantity_outstanding)` over children with `closed_at IS NULL` — never over children's `quantity_expected` | D24s |
| J25 | `refines_expected_supply_id` is acyclic and of depth exactly 1; violations raise `refinement_too_deep`. Asserted by a job, **never a CHECK** — a CHECK on a projection column makes the log unprojectable and wedges the rebuild (D24's own q92 ruling) | D24s |
| J26 | `expected_supply.quantity_received` = the fold of `goods_receipt_line` rows naming this row or any row refining it | D24s |
| J27 | `expected_supply.quantity_expected` on the transfer arm = the fold of despatch movements naming that `transfer_order_line`; **no unit is simultaneously counted in origin `stock.available_quantity` and destination `quantity_promisable`** | D16, D24s |
| J28 | No open `expected_supply` row with `quantity_outstanding > 0` and `expected_to + grace < now()` → `supply_overdue` | D24s |
| J29 | No active allocation references a closed or overdue `expected_supply` → `supply_withdrawn` / `commitment_unbacked`. The allocation is **not** released by the job | D24s, D5 |
| J30 | Rebuilding `expected_supply` preserves row identity: every source line resolves to the same `id` before and after. Truncate-and-regenerate is forbidden while any allocation holds an `expected_supply_id` | D12, D24s |
| J19 *(scoped)* | Truncate every assertion table, rebuild `stock` and `stock.allocated_quantity`, assert byte-identical. `expected_supply` and `stock_allocation.expected_supply_id` are **exempt and named** — that is what rule 3's amendment permits | D21 rule 3 |

**Structural (CI)**

| # | Invariant | Owner | Assertion |
|---|---|---|---|
| S3 *(scoped)* | The `<= 1` rule binds demand/cause CHECKs on **grouping** tables. A projection's provenance-arm CHECK is `= 1`, and the exception is enumerated | D10, D23, D24s | Grep + declared exception list |
| S4 *(amended)* | Every table registers exactly one role. A table with role `projection` registers the **set** of provenances it folds, and CI asserts that set equals the sources named by its registered rebuild functions | D21, D25, D24s | Bidirectional diff |
> **The invariants below have moved to [invariants.md](./invariants.md)**, the single register and the canonical numbering. The entries here are retained as written; the register is authoritative on text, numbering and status.

| S24 | `expected_supply` has one partial unique index per provenance arm, and **no** unique index over `(item_id, owner_id, status_id)` | D24s | Catalogue scan |
| S25 | The availability indexes on `stock` and `expected_supply` both carry `owner_id` and `status_id` in the key and the promisable measure in `INCLUDE`; no query on the availability path joins `inventory_status` | D12, D20, D24s | Catalogue + query register |
| S26 | `expected_supply` carries at most five maintained quantity columns. A sixth requires a recorded decision | D24s | Column count |
| S2 / D24 reaper *(scoped)* | Both apply to `stock` only. `expected_supply` rows are never reaped | D24 | Declared exclusion |

---

## Amendments to D1–D25

**D21 — rule 3's positive half, and this closes q103.** The rule as adopted reads *"Never projects into `stock` or into commitment."* Restated:

> **3. Never projects into `stock`, and never into a commitment that survives withdrawal of the claim.** Assertions project into **expectation** (`expected_supply`), and demand may be bound to an expectation row. The test is J19: truncate every assertion table and `stock` plus `stock.allocated_quantity` rebuild byte-identical, because an assertion-sourced allocation never touches a cell.

That is a real narrowing and it is written out rather than assumed. The compensating guarantee is stronger than the original wording implied: an ASN's withdrawal cannot silently reduce a balance, because it never reached one.

Also amended: **freeze-on-first-use gains a third referencer.** D21 names `assertion_check` and `goods_receipt_line`; `expected_supply` is now a fourth thing that references `asserted_unit_content.resolved_*`, and a late re-resolution must not re-key a projection row a live allocation is bound to.

**D12 — `stock.allocated_quantity` is narrowed by an enumerated predicate** (cell-bound allocations in states `allocated | picking | picked | packed`), and the allocation lifecycle gains `picked` and `packed`. Without those two states there is no point at which picked-but-not-despatched stock stops suppressing availability, so either WIP stock in a sealed outbound package reads as available again — the `usage`-predicate cost D16 refused, reintroduced by the back door — or `fulfilled` conflates picked with shipped. NetSuite is the counter-example: reallocating from a Picked fulfilment produces a commitment that looks real, cannot be acted on, and evaporates when the first order ships. *"Backorder is not an entity"* is untouched.

**D16 — q40 answered: yes, a transfer can be allocated before it arrives**, from the moment of despatch, against a destination-site `expected_supply` row. The derivation D16 described becomes a maintained projection. In-transit stays off `stock`, reinforced. The transfer arm is the one arm with **zero exposure to rule 3** — it derives from our own despatch movements, which are facts — and it is therefore the arm to build first.

**D22 — no new policy kinds.** Cross-dock windows, FEFO tolerance and per-arm admissibility are typed scalars on `allocation_policy`; supply-overdue grace is a scalar on `receiving_policy`. `fill_sequence` is recorded in D22 as the worked example of a compliant strategy-selecting enum against a non-compliant column-naming one. Incoterms and title-transfer triggers, if ever needed, are **instance agreements** on the order, not a policy kind — they are negotiated per order, not defaulted per party class.

**D25 — `discrepancy` gains its sixth source arm.** The recorded decision the cap demands: **`expected_supply_id`**, capped at six, `CHECK <= 1` unchanged. It is knowingly a *subject standing in for an absent cause* — the cause of "nothing arrived" is the absence of a movement — and D23's boundary rule is stretched deliberately rather than violated quietly.

**D8 — `discrepancy.kind +=`** `supply_withdrawn` (reinstated; dropped at D25's adoption because its subject did not exist), `supply_overdue`, `supply_over_refined`, `commitment_unbacked`, `advised_lot_mismatch`, `refinement_too_deep`, `receipt_unmatched`, `over_receipt`. `discrepancy` also gains `counterparty_party_id`, without which none of the supplier-facing ones aggregate.

**D24 (containment) — one index correction and one CHECK.** The availability index becomes `(tenant_id, item_id, site_id, owner_id, status_id) INCLUDE (available_quantity) WHERE quantity <> 0`. And `stock_movement` gains `CHECK (num_nonnulls(from_location_id, from_package_id) + num_nonnulls(to_location_id, to_package_id) >= 1)`. Today a movement with both sides empty — an ownership change while in transit — **passes every CHECK and is insertable**, but folds into two cells with no holder, which `stock`'s own CHECK forbids. That is not an unrecordable fact; it is an insertable fact the projection cannot represent, which is the wedged-rebuild failure D24 reasoned about and refused for `depth`.

**D14 / q32 answered: no.** A lot is not created before its goods arrive. The advised code and expiry ride on `expected_supply` as raw non-authoritative strings.

**Open question 38 answered.** A transfer's receipt reconciles against its despatch through the destination-site `expected_supply` row: variance is `quantity_expected − quantity_received`. **The destination site owns it**, because the row is keyed to the destination. It is a timing difference — and suppressed — until `expected_to + receiving_policy.supply_overdue_hours`, at which point it is `supply_overdue`. This is where the derivation earns its keep: ERPNext's Goods-in-Transit warehouse strands the missing 4 units as a permanent phantom balance a human must write off (issue #52221, open, and a multi-year trail behind it). Ours is arithmetic.

**Open question 60 answered.** Two arms.

---

## Rejected

- **`expected_supply` as a view or UNION.** Two independent reasons, and the sketch gives only one. Netting computed once beats a rule every query remembers; *and* the FK target is the gate row Postgres needs to serialise concurrent allocations, in the absence of the gap locks ERPNext's availability check depends on. Their source carries a comment explaining two databases' lock semantics to survive not having it.
- **A signed-adjustment ledger with caller-supplied compensation** (D365 Inventory Visibility's on-hand change schedules). Microsoft documents the burden — *"you must then revert the scheduled change by submitting a matching negative quantity"* — and it is our 160, in a shipped product, with the vendor telling the integrator that avoiding it is their job. A foreign key the database can check beats a convention nothing enforces.
- **Depleting the parent** (NetSuite, Oracle's `RCV_SHIPMENT_LINES` running totals). Conflates *"the supplier promised 100"* with *"the supplier has told me about 60"*, and supplier-promise accuracy — the whole reason D21 keeps the assertion — becomes unanswerable afterwards.
- **The movement graph as the supply binding** (Odoo's chained `move_orig_ids`/`move_dest_ids`). `stock_movement` is a fact with no UPDATE grant (S6), and `procure_method = 'make_to_order'` moves being skipped in `_action_assign` is a WHERE clause on the sum D5 exists to keep unconditional. Odoo's own failure modes are the predictable consequence: partial availability with no automatic split, no backorder split on short completion, and a PO quantity decrease that posts a message for a human rather than repairing the chain.
- **A transit location or transit warehouse.** D16 decided it; this is the evidence. D365 concedes the cost in one sentence — transit goods are *"visible in inventory reports"* and *"usable for master planning"* but not pickable, which is a predicate on every availability read. No `transit` value on `location.kind`, ever; record it as a named refusal rather than leaving it protected by reasoning.
- **A reservation as a ledger row with a zero physical delta** (OFBiz's `availableToPromiseDiff`). Genuinely attractive — J1 and J3 collapse into one fold — and refused three ways independently: `stock_movement.quantity > 0`, `recorded_by_id NOT NULL` with no automation exemption, and a conditional fold on the one sum the model has twice refused to make conditional. Recorded so it is refused once rather than re-proposed by everyone who has seen OFBiz.
- **A `quantity_short` column on `stock_allocation`** (OFBiz's `quantityNotAvailable`). A second, weaker representation of a number D12 already derives — and OFBiz's version still requires the reservation to name an inventory item it cannot satisfy, which is exactly the "`stock` rows for goods that are not there" the sketch already rejects.
- **`crossdock_sequence_key(policy_id, ordinal, column_enum, direction)`.** Fails S11 twice over: a column whose value is a column name, and an ordering of steps. The compliant version is three typed scalars and a code-side sort.
- **A stepped `stock_by_item_site` rollup** (ERPNext's `Bin`). Deferred by q74 on the right condition, and it inherits a fatal limitation regardless: `projected_qty` has no date, so a PO due in six months and a pallet on the floor contribute identically. It answers *"will I ever be short"*, not *"can I ship Thursday"*. It is also a third projection hop against J24.
- **A delta side table for `quantity_allocated`** (D365's `WHSInventReserveDelta`). It is the known remedy for write contention and it costs the generated column and the index-only read on whichever table it is applied to. Record the price and the trigger condition so *"wait for real demand"* does not become permanent in either direction.
- **Quota / allocated ATP as a column on `expected_supply`.** Entitlement is not supply. Consumption windows make one demand line consume several quota periods, so it is many-to-many and cannot be a column anywhere. Flagged only so the design does not foreclose it: `quantity_allocated` is a sum over many allocations, so nothing assumes one supply row has one claimant.
- **SAP's destructive delivery grouping**, which deletes earlier schedule lines and copies their confirmed quantities into the latest. It is why SAP installations cannot answer *"what did we originally promise"*.

---

## Open questions

> **Questions raised in this document have moved to [open-questions.md](./open-questions.md)**, the single register and the canonical numbering. The entries below are retained as written; the register is authoritative on status.


106. **Does `fulfilment_line` get a maintained `allocated_quantity`?** Three independent needs converge on it — the opportunistic matcher's open-demand scan, the replan sweep after a supply shrink, and *"is this line covered"* across two heterogeneous arms. It does not break D12's read guarantee (that is about `stock`, on the floor path) but it reverses a stated D12 sentence, and it should arrive as an amendment rather than by accretion three times over.
107. **Does title change while in transit, and do we need to record it?** Incoterms allocate risk and cost and explicitly do *not* transfer title; GS1 CBV makes `owning_party` and `possessing_party` independent source/destination types on the same event. Today `expected_supply.owner_id` is a static copy of the source line, which is not rebuildable from anything if title can move mid-flight. The mechanism is a `supply_custody_change` fact with owner and custodian pairs and `owner_id` as its projection. Deferred, not designed — but the `stock_movement` CHECK above is added now so the wrong answer is a constraint violation rather than a wedged rebuild.
108. **Are intermediate re-points reconstructable?** `origin_expected_supply_id` gives the first binding and the current arm gives the last; the middle is gone. Oracle has four distinct re-point verbs and a `transferRsvToAnotherDemand` endpoint that takes a quantity. For recall it does not matter (D14's trace runs over `stock_movement.lot_id`); for a chargeback dispute over which PO a cross-docked unit came from it might. If yes, it is an explicit exception to D25's *"an event log for intentions"*, argued in writing.
109. **Multi-PO ASN — in or out?** `refines_expected_supply_id` as a scalar FK cannot express one advised content line drawing on two PO lines, and D21's `asserted_unit_content.resolved_purchase_order_line_id` is already singular. Metcash and Coles both forbid an ASN spanning more than one PO; US/3PL 856 ORDER-loop traffic does not. Recorded as an explicit omission against q68 rather than widened speculatively — widening it to an association table would invalidate J8's partition identity.
110. **`containment_conflict` still has no source arm.** Adopted D24 raises it from `package_event` compare-and-set and D25's five arms include no `package_event_id`. That hole predates this decision and is not widened here (the cap stays at six), but it is now the only finding kind in the model whose investigation dead-ends by construction.
111. **The `expected_supply` concurrency budget.** ASN ingestion performs one parent update per child row, so a 200-line DESADV touches one parent 200 times while the allocator wants it. Batching the decrement per parent inside the ingestion transaction turns 200 updates into one and needs no schema change — but the model states a read budget and no write budget, for this table or for `stock`, and D5 only decides the floor-blocking half.
