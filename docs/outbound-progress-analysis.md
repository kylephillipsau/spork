# Outbound progress, and where its evidence comes from

*Research pass on question 165, 2026-08-10. Raised by the outbound walk.*

> **Status: partly superseded by D99**, adopted the same day, migration 53. The cause
> arm is built and the discriminator is settled — **and settled differently from §3
> below**, which proposed reading the *to* side and is wrong. See "the discriminator"
> in D99: a to-side rule undercounts batch picking and double-counts consolidation,
> and the correct rule reads the *from* side. That paragraph is left standing rather
> than edited, because the register's rule is that a research document is superseded
> by the decision record rather than retrofitted to agree with it, and because the
> wrong version is the reason the right one had to be worked out.
>
> **Fully superseded as of D100**, migration 54, which settled the rest of 165 by
> adopting §5's option C. §4's case against `stock_allocation.state` is the case
> D100 acted on. Two things this document did not reach, both found while building
> it: `packed_quantity` must read `package.status`, a fold of `package_event`, and
> **not** `package.sealed_at`, which the application may UPDATE — §3 named the seal
> without noticing the schema offers two of them, one mutable. And a reversed
> movement and its reversal must both leave the fold, or a correction inflates the
> number it was recorded to fix; this document does not mention corrections at all.
> The decision record is authoritative over everything below.

165 asks two things:

1. Does `stock_movement` gain an outbound cause arm under D10's typed-FK rule?
2. Does outbound progress then fold from it, or stay a fold of `stock_allocation.state`?

**They have different answers, and the first is not open.** The model already decided
it, in three places, and the arm is missing because a migration built half of what its
own decision specified. The second is open, and the honest answer is neither of the two
the question offers.

---

## 1. What is actually true today

Verified against the migrations rather than the prose, because the prose and the schema
disagree and that disagreement is the finding.

| | |
|---|---|
| `stock_movement` cause arms | **one** — `goods_receipt_line_id`, migration 21 |
| `stock_movement_cause_ck` | `CHECK (num_nonnulls(goods_receipt_line_id) <= 1)` |
| `stock_movement` grants to `nylonite_app` | `SELECT, INSERT` — no UPDATE, no DELETE (S6) |
| `stock_allocation` grants to `nylonite_app` | `SELECT, INSERT, UPDATE, DELETE` |
| `stock_allocation` history | none — no event table, no append-only log |
| `stock_movement.reason` | `text NOT NULL`, **no CHECK at all** |
| `fulfilment_line`'s four quantities | all four `@projection of stock_allocation`, J31 |

`crates/invariants/tests/outbound_walk.rs` asserts the gap rather than describing it:
ten units picked into the carton, `moved = 10`, and `(picked, packed, despatched) =
(0, 0, 0)`. Its second test, `the_ledger_has_no_outbound_cause`, counts cause FKs in
`pg_constraint` and asserts `1`. Both are tripwires — they fail the day the arm lands,
which is the correct shape for a test guarding an open question.

### The cause CHECK is vacuous, in exactly the way D45 said S3 was vacuous

`num_nonnulls(goods_receipt_line_id) <= 1` takes one argument. It evaluates to 0 or 1
and is therefore **always true**. It constrains nothing.

S3 greps `pg_constraint` for `%_cause_ck` and requires the substring `<= 1`. It passes.
D45 wrote, about S3's own first outing: *"A vacuous check cannot be wrong out loud."*
That sentence now applies to the constraint S3 examines. The mutual-exclusion rule
S3 exists to enforce is, on this table, currently enforced by arity rather than by
semantics — and the moment a second arm appears the constraint becomes real without
anyone editing it.

This is not an argument for or against the arm. It is a note that the schema is already
written in anticipation of one, and that nothing today would notice if it never came.

---

## 2. Part one is already decided

**The Correction to D10** (`domain-model.md:1286`) states the corrected cause set
verbatim:

```
stock_movement
  fulfilment_line_id      -- cause
  goods_receipt_line_id   -- cause
  discrepancy_id          -- cause
  CHECK (num_nonnulls(fulfilment_line_id, goods_receipt_line_id,
                      discrepancy_id) <= 1)
```

`fulfilment_line_id` is the first line of it. Two further passages assume it exists:

- **D53** (`domain-model.md:6793`) names the absence as the *reason* progress folds the
  allocation: *"`stock_movement` carries no reference to a fulfilment or an order line,
  so there is no path from a movement to the commitment it served."* It records the
  constraint, not a preference for the state machine.
- **`mechanism-design.md:519`**, retiring `package_content`: *"the demand cause lives on
  the movement that put the stock there, which is where D10 says causes live."* A carton
  packed for an order has no such cause today, so the sentence justifying the retirement
  is not yet true of the outbound path.

So the arm is not a new decision. It is an **unbuilt** one. Migration 21 built the
inbound arm because it needed it for D45, wrote a one-argument CHECK, and left the rest.
The `discrepancy_id` arm was independently resolved in the other direction — migration 6
puts `stock_movement_id` and `resolving_movement_id` on `discrepancy`, so the cause set
on the ledger is two arms, not three.

**Cost of building it.** `ADD COLUMN … uuid` nullable is a catalogue-only change in
modern Postgres — no table rewrite, which matters on the busiest table in the schema.
The real costs are the composite FK to `fulfilment_line(id, tenant_id)` (migration 36's
convention), a partial index mirroring `stock_movement_receipt_line_idx`, and widening
the grant. Roughly eight bytes a row and one migration.

The only genuine question in part one is whether exclusivity holds — whether any
movement legitimately names both a receipt line and a fulfilment line. Under the current
model it does not: a cross-dock is an arrival movement and then a pick movement, two
rows, because D45's arrival test is already shape-based and counts only the first.

---

## 3. Part two is open, and the question's framing hides the real split

165 frames it as *ledger* versus *the commitment's own state machine*. Working through
the three fractions, **they are not homogeneous** — and neither answer is right for all
four columns.

| Column | Is it a fact or an intention? | Is it in the ledger? |
|---|---|---|
| `covered_quantity` | **intention** — how much we *mean* for this line | no, and correctly not |
| `picked_quantity` | fact | yes, but shape under-determines it |
| `packed_quantity` | fact | **no — sealing a carton moves no stock** |
| `despatched_quantity` | fact | yes, cleanly |

### Despatched is the easy one, and it is D45's mirror

D45 distinguishes an arrival from a later putaway **by shape, not by `reason`**:

```sql
AND m.from_location_id IS NULL AND m.from_package_id IS NULL   -- goods entering
```

The mirror is exact — `to_location_id IS NULL AND to_package_id IS NULL`, goods leaving
the building, which the whole-key CHECKs permit precisely so they can. Grouped by a
`fulfilment_line_id` arm this is `SUM(quantity)` and a batch load, the same sentence D45
wrote for receiving, on the same idiom, needing no new vocabulary.

It also solves the double-count for free. A pick movement and a despatch movement both
name the same line; partitioning by shape separates them, which is the identical problem
D45 solved when a putaway named the same receipt line.

### Picked is the one with real design work left

A pick is bin → carton: **both sides populated**, which is the same shape as a
replenishment, a consolidation from tote to carton, and a rebuilt pallet. Shape alone
does not identify it, so a ledger fold needs a discriminator, and the obvious candidate
is poor:

**`stock_movement.reason` is `text NOT NULL` with no CHECK.** The walk writes `'pick'`
and `'despatch'` as free strings. Branching a projection on an unconstrained text column
is strictly worse than note 143's complaint about `stock_allocation.state`, which is at
least text *with* a CHECK listing seven values. **143 should be widened to name
`reason`**, and it should be settled before, not after, anything folds on it.

The alternative discriminator is structural: a pick is a movement naming a fulfilment
line whose `to_package_id` is a package with that same `fulfilment_id`. That avoids
`reason` entirely and stays in the D45 idiom of deriving from shape and relationship.
It is not obviously complete — picking to a pallet or a pick face is not a package with
a fulfilment — and this is the piece the decision actually has to settle.

### Packed is not a ledger fact at all, and that is the finding

**Nothing moves when a carton is sealed.** There is no `stock_movement` for it. Packing
is `package_event.kind = 'sealed'`, and no cause arm on the ledger can ever produce
`packed_quantity` by grouping.

The derivation exists, and `mechanism-design.md:519` already states it: *"a sealed
carton's manifest acquires history: it is the movements that put stock into it up to
`sealed_at`, a fact, non-rewritable."* That is `packed_quantity` — the movements into a
carton, bounded by its seal event. It is a fold of two fact tables rather than one, and
it is still evidence rather than intention.

So the answer to "does progress fold from the ledger" is: **two of the three do, one
folds from the ledger and `package_event` together, and the fourth should not fold from
the ledger at all.**

---

## 4. The case against staying with `stock_allocation.state`

Not a style preference. Four specific costs.

1. **The source is mutable and deletable, and the thing it measures is not.**
   `nylonite_app` holds `UPDATE, DELETE` on `stock_allocation`, and there is no
   allocation event log. So `despatched_quantity` — the number that answers *what did we
   actually ship* — is folded from rows the application can rewrite or remove, with no
   record that it did. The inbound equivalent is folded from a table with no UPDATE and
   no DELETE granted at all (S6). 165 names *"the first dispute about what was actually
   shipped"* as its trigger; that dispute is exactly where this asymmetry lands.

2. **It contradicts D12.** An allocation is *"an intention… advisory… allowed to be
   wrong"*, and D12's founding case is the picker who finds lot B where the plan said lot
   A: *"the pick is a fact"*. Using the intention as the evidence for what was picked
   inverts that. D5's observation — the scanner is more authoritative than the database —
   is the same sentence one level up.

3. **The advancing act is unforced, which is what the walk demonstrates.** Progress reads
   zero after a real pick not because anything failed but because advancing
   `state` to `picked` is a separate write nothing requires. Every number depends on an
   operator-side bookkeeping step that the physical event does not compel.

4. **Cases D12 explicitly permits are unrepresentable.** A pick with no allocation, an
   over-pick, a pick of a substituted item — all legal under an advisory allocation, and
   none of them can move a fold whose only input is allocation rows. The ledger records
   all three natively.

**What the state machine is genuinely right for.** `covered_quantity` asks how much of
this commitment is *spoken for*, which is a question about intentions, and folding it
from `stock_allocation` is correct and should not change. D53's insight — that the cell
and the commitment ask different questions of the same word — repeats here one level in:
coverage and progress are different questions, and only one of them is about intentions.

---

## 5. The four options, honestly

| | Shape | Verdict |
|---|---|---|
| **A** | All four fold the ledger, D45's way exactly | **Impossible.** `packed` has no movement to fold. |
| **B** | Status quo — all four fold `stock_allocation.state` | Coherent and cheap, but the evidence for a physical claim is a mutable intention. Survives only until the first dispute. |
| **C** | Split by evidentiary class: `covered` ← allocation; `picked`, `despatched` ← ledger; `packed` ← ledger ∧ `package_event` | **Recommended.** Each number folds from the thing that is a fact for it. |
| **D** | Give `stock_allocation` an append-only event log and fold that | Fixes mutability, not authority. A second ledger for events the first already records — the duplication principle 1 refuses — and a picker who picks still moves nothing until someone advances a state. |

### What C costs, stated plainly

- **The pick discriminator** is unsolved (§3), and it is the reason this is a decision
  rather than a migration. It should be settled by shape and relationship, not by
  `reason`, until `reason` has a CHECK.
- **An item-agreement invariant is required.** A movement naming a fulfilment line must
  carry that line's item, or the fold credits the commitment with the wrong goods. It
  cannot be a CHECK — it spans rows — so it is a J-invariant raising a D8 finding, which
  is also the mechanism that makes D12's substituted pick *visible* rather than silently
  miscounted. This is a gain, not just a cost.
- **J31 splits.** It currently asserts all four quantities against one source. Under C it
  asserts `covered` against `stock_allocation` and the other three against the ledger,
  and the two halves have different state sets — the same trap D53 caught between J3 and
  J31, one level further on.
- **S36 wants an outbound analogue.** S36 forbids a stored received quantity *"so the
  number cannot disagree with the ledger that produces it."* Nothing says the same for
  outbound. S44 is close but forbids a rollup on the header, not a divergent source on
  the line.
- **Migration 15's four `FILTER` clauses** are replaced for three of four columns, and
  `projection_fulfilment_rebuild` grows a second source. Its `LEFT JOIN`-from-the-line
  discipline — so a line whose last allocation vanished goes to zero rather than keeping
  stale numbers — must be preserved per source.

---

## 6. Where this leaves 165

- **Part one — yes, and it is not new.** The Correction to D10 already specifies
  `fulfilment_line_id` as a cause arm; D53 and `mechanism-design.md:519` both assume it.
  Building it makes `stock_movement_cause_ck` mean something for the first time.
- **Part two — neither of the two options as posed.** `covered_quantity` stays a fold of
  intentions because that is what coverage is. The other three are claims about what
  physically happened and belong to the fact tables — two from the ledger alone, one from
  the ledger bounded by the seal event.
- **Blocked on 143, narrowly.** Not on the enum question in general, but on whether
  `stock_movement.reason` gets a constrained vocabulary — because the tempting version of
  the pick discriminator branches on it, and today it is unconstrained text.

**Rejects.** A `picked_quantity` column on `fulfilment_line` sourced from anything the
app can UPDATE directly (S36's shape, on the outbound side). A polymorphic
`cause_type`/`cause_id` pair, for D10's four reasons, argument 3 first — the fold is a
`belonging_to` batch load only because the cause is typed. Folding `picked` from
`reason = 'pick'` while `reason` has no CHECK. Deriving `packed` from a movement, which
would require inventing a movement for an act that moves nothing. A stored
`fulfilment.progress` to make any of this simpler, which S44 exists to prevent.

**Raises.** Whether `stock_allocation` should lose `DELETE` from the application role
regardless of how 165 settles — a released allocation is a fact about a decision
somebody made, and D12's advisory reading argues for retaining it rather than removing
the row.
