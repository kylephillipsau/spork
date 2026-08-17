# Cross-cutting review

What the inbound analysis surfaced about the *rest* of the design. Written
2026-07-31, after [inbound-analysis.md](./inbound-analysis.md).

The inbound-specific gaps are the smaller half. The analysis also found seven
patterns that cut across everything, and several of them are directly about the
three things that matter here: **interoperability between formats**, **more
configuration in the user's hands**, and **a coherent experience** rather than
seven mechanisms that each behave slightly differently.

---

## 1. Nothing catches a decision breaking an earlier invariant

**The meta-finding, and the one that predicts the others.**

Three instances, all from decisions taken this week:

- **D20 broke D12.** `owner_id` joined the `stock` key; `stock_movement` never
  got it. D12's stated invariant — *"each column is rebuildable from its own
  source, and the reconciliation job asserts both"* — is **false today**.
- **D16 repeated D10's error.** `goods_receipt`'s `= 1` CHECK forbids blind
  receipt, for exactly the reason the published Correction to D10 already fixed
  once. The correction was written down and then not applied to the next table.
- **D6 claimed something it did not deliver.** *"One primitive serves shipped
  parcels, pallets of cartons, picking totes, putaway LPNs and put-wall cells"* —
  true of `package`, false of `package_content`, whose only demand FK is
  outbound.

None was caught by review. All three were caught by an outside pass reading the
whole document at once.

**Recommendation — an invariant register.** The decisions state invariants in
prose, scattered across 20 entries and 1,600 lines. Extract them into an explicit
list, each with the decision that owns it and, where possible, **the assertion
that proves it**:

| Invariant | Owner | Proof |
|---|---|---|
| `stock.quantity` is rebuildable from `stock_movement` | D5, D12 | Reconciliation job |
| `stock.allocated_quantity` is rebuildable from `stock_allocation` | D12 | Reconciliation job |
| Every package on a consignment has a fulfilment | D15, D16 | CHECK / test |
| A movement has at most one demand cause | D10 corrected | CHECK |
| `stock` key dimensions all appear on `stock_movement` | D4, D20 | **Currently violated** |

The register is worth more than the tests: most of these are cheap to assert in
CI once written down, and the ones that are not are at least visible. **The
failure mode was not carelessness, it was that no artefact held the invariants
where a new decision would collide with them.**

---

## 2. Append-only about stock, casually mutable about everything else

The strongest idea in the model is applied to one table and abandoned elsewhere.

Mutable status columns now exist on `order`, `fulfilment`, `consignment`,
`work_task`, `discrepancy`, and inbound adds `goods_receipt`, `inbound_shipment`
and `package.status`. Containment is worse: `package.parent_package_id` and
`package.location_id` are overwritten, so split, merge, re-palletise and relabel
**leave no evidence at all** — the only physical relationship in the model whose
history is destroyed on update, and the one an investigation has to walk.

This was competitor gap 8. It was never elevated to a decision, and three more
entities have since acquired mutable status.

**Recommendation.** Settle it once, uniformly, before building more:

- **Containment becomes a fact.** `package_event` (append-only, D5 idempotency
  columns), with `parent_package_id`, `location_id` and `status` demoted to
  projections maintained by the same rebuild-and-assert job that guards `stock`.
- **Status becomes derived** for entities whose transitions are evidentiary —
  `consignment` (the async carrier round-trip that will need debugging at 2am),
  `inbound_shipment` (a rejected-then-resent ASN *is* the audit),
  `goods_receipt`, `discrepancy`.
- **Status stays a plain column** where transitions carry no evidentiary weight.
  `work_task.state` on a pick that lasts ninety seconds does not need a history
  table; `activity_event` already covers what happened.

The distinction to write down: **derive status where the transition is evidence,
store it where it is only state.** That is a rule, not a per-table judgement, and
it stops both the accretion and the over-engineering.

---

## 3. Policy resolution is being reinvented, and that is the configuration story

**Directly the "more tools and configuration" question.**

The model now wants most-specific-wins policy resolution for eight things:
allocation weights (D13), quantity tolerance (D20 q55), minimum shelf life (D14,
q33/39), putaway scoring, default receiving status, quality sampling, audit
tiering, and over-receipt limits. Each has been designed independently.

The competitor analysis warned in these words: *"declining the engine while
accepting five small rule tables is how you get a rules engine you never
designed."* We are at eight.

**Recommendation — one resolver, not eight chains, and not an engine.**

```
policy_scope                  -- how any policy resolves
  id, tenant_id
  policy_kind                 -- allocation | tolerance | shelf_life | putaway | ...
  scope_kind                  -- site | item_class | item | customer | supplier | order_line
  scope_id
  precedence                  -- derived from scope_kind; most specific wins
  effective_from, effective_to
```

Each policy kind keeps its **own typed value table** — `allocation_policy` stays
scalars, `tolerance` stays its own shape. What unifies is *resolution*: one
function, one precedence rule, one effective-dating mechanism, one audit trail.

This is the opposite of a rules engine, and the distinction is the same one D13
drew: **the logic is code; only the weights and scopes are data.** A rules engine
stores *what to do* as rows. This stores *which numbers apply where*.

Three things fall out, all of which are the better-experience answer:

1. **One mental model.** A manager learns precedence once and it holds
   everywhere. Eight bespoke chains means eight ways to be surprised.
2. **One UI pattern.** Every policy screen is the same screen with different
   fields — and *why is this value applying here* is answerable generically,
   which is the question configuration UIs usually cannot answer.
3. **Policy changes become facts** (open-decisions #30), once, rather than per
   policy. That makes *"did raising the rotation weight reduce spoilage?"*
   answerable — and it is the same argument for every other policy kind.

---

## 4. The provenance idiom is trapped in `measurement`

**Directly the "interoperability between formats" question.**

`measurement` is the model's most defensible original idea: an observation with a
source, a confidence and a time, superseded rather than overwritten. Nothing in
the competitor set has it.

It is also welded shut. `metric` is (length, width, height, weight, cube) and
`subject_type` is (item, package_type, package). It cannot hold a
supplier-asserted quantity, a carrier ETA with confidence, a receipt temperature
against a specification, a quality measurement, a gross/net/tare distinction, or
a dimension of a *packaging level* rather than an each.

Three independent requirements asked for the idiom and were refused by the table
shape. And open-decisions #20 — typed subject FKs — would **close the subject set
at the moment inbound opens it.**

**Recommendation.** Generalise before answering #20. An observation is
`(subject, metric, value, unit, source, confidence, observed_at, asserted_by)`,
and that shape serves dimensions, temperatures, quality measures, asserted
quantities and ETAs identically. Keep the typed-FK discipline for subjects, but
choose the subject set knowing inbound exists.

**Why this is the interoperability answer.** Ingesting the same fact from an EDI
ASN, a supplier portal, a CSV, a scale, and an operator's keyboard is *the*
integration problem. A shape that records **what was observed, by whom, how, and
how much we trust it** handles all five identically — and the differences between
formats become `source` values rather than parallel pipelines. Without it, each
new format grows its own columns.

---

## 5. Principles 3 and 4 were written for a premise D18 replaced

**Principle 3** says JSONB is permitted for *"exactly one thing"* and names
`provider_exchange.payload` as *"the only JSONB in the model"*. That is now
contradicted three times over: the outbox payload, inbound message retention, and
unrecognised GS1 application identifiers.

The problem is that it was written as a **census** rather than a **rule**. A
census erodes by exception, which is precisely the accretion the principle
exists to prevent.

> **Restated:** opaque counterparty payloads may be retained as JSONB for audit,
> never queried structurally. Anything queryable is promoted to a column. New
> cases are permitted when they meet the rule, not by amending a list.

**Principle 4** refuses custom-field frameworks on the grounds that *"adding a
column is cheap and migrations are routine"*. That was true for one company's
internal tool. **D18 made multi-tenancy non-negotiable and pointed at a
commercial product**, and the premise no longer holds automatically: a tenant
facing a biosecurity form we have not shipped cannot add a column.

The position is still defensible — one typed table per scheme, shipped by us, a
new scheme is a release. But it is now an **opinionated-product decision** with a
real cost, not an engineering convenience. D13's rules-engine refusal inherits
the same problem: *"vendor X's goods go to zone 3"* is not expressible without
our deploy.

**Recommendation.** Restate both against the D18 premise and name the cost
explicitly, so it is a chosen trade rather than an inherited assumption. This is
the honest version of "more configuration for the user": we are choosing to give
them *scoped policy values* and not *arbitrary logic*, and that line should be
drawn deliberately.

---

## 6. The typed-FK idiom strains past three arms

D10 chose typed FKs over a polymorphic pair, and the reasoning was sound —
principle 6's batch-loading defence cannot be expressed over a polymorphic
reference. But it was argued against *one* alternative at *two* arms.

It is now at four or five in several places: `goods_receipt` (PO, transfer, ASN,
RMA), `package_content`, `discrepancy` (source), `fulfilment` (order, transfer,
supplier return), `attachment`. And `external_reference` — recommended in
open-decisions #4 for exactly the format-interoperability job — is itself a
polymorphic pair, which D10 rejected, on a path (ASN party and item resolution
against a 3-hour acknowledgement SLA) where D10's batch-loading argument applies
directly.

**Recommendation.** Keep typed FKs; the reasoning holds. But **write down the
limit**: beyond about four arms, or where the arm set is genuinely open, the
honest answer is a join table rather than a widening CHECK. And re-do
`external_reference` as typed, because it is on the hot path the idiom was
adopted to protect.

---

## 7. There is no concept of acceptance

D5 says the scanner is authoritative: if 120 cartons are on the dock, 120
arrived, and rejecting the scan discards a true observation to protect a database
invariant.

That is right, and it exposes a missing concept rather than a flaw. The
commercial requirement is not to block the *count* — it is to block the
**acceptance**, and the model has no acceptance act distinct from the movement.
Under the Food and Grocery Code, acceptance extinguishes the right to reject
within 24 hours, so *"accepted at 14:32 by this person"* is a fact with legal
weight.

This is not inbound-only. A customer accepting a delivery, a quality inspector
releasing a hold, a manager accepting a stock variance — all are the same act:
**a person taking responsibility for a state, distinct from observing it.**

**Recommendation.** Add acceptance as a first-class fact wherever a
counterparty's or a supervisor's assent has consequences. It composes with D11's
`authorised_by_id` and it is the missing half of D8: a finding currently has a
resolution but no record of anyone *accepting* the outcome.

---

## What this adds up to

The design is sound. Every one of these is a **consistency** failure rather than
a conceptual one: a good idea applied to one table and not the next, a principle
stated as a list rather than a rule, an idiom adopted for two arms and stretched
to five.

That is a coherent finding in itself, and it suggests the highest-value work is
not new features but **finishing the ideas already here** — which is also what
produces the better experience, because a model where the same idea behaves the
same way everywhere is one a user can predict.

**Suggested order:**

1. The invariant register (§1) — cheap, and it stops the class of error
2. Tier 0 from the inbound analysis — eleven changes, all cheap now
3. The unified policy resolver (§3) — before the eighth bespoke chain
4. Generalise observations (§4) — before open-decisions #20 closes the set
5. Restate principles 3 and 4 (§5) — before a tenant asks
6. Append-only consistency (§2) — before three more mutable statuses exist
