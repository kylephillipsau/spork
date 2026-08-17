# Review: does the containment model match what actually happens?

Focused review of two points in [D24](./mechanism-design.md) before it is adopted:
the `package_content` restructure and the `lot_id` breakage. Written 2026-08-01.

The question being answered is not "is this clever" but **does the fact model
correspond, one to one, to physical acts — and is the rule that decides which
fact gets written decidable by someone at 6am who has not read this document.**

Conclusion: the model is right, the *rule* as written is not, and one of my own
objections was wrong.

---

## 1. The rule needs restating. "Custody" is the actual defect.

D24 says:

> A `stock_movement` is written when **custody** changes — stock enters or leaves
> the system, changes holder, status, owner or lot. A `package_event` is written
> when a **holder changes position**.

That is not decidable, because **a carton is both a holder and a thing with a
holder.** When a carton moves from pallet A to pallet B, has custody changed? The
carton changed hands. The goods inside did not. Two competent people will answer
differently, and the whole value of the split is that they must not.

**Restated, exactly:**

> - **`stock_movement`** — a stock cell's **key** changed: holder, lot, status or
>   owner; or quantity entered or left the system.
> - **`package_event`** — a package's **placement** changed: its parent, or its
>   location.

The two are disjoint by construction because **they have different subjects**. A
movement is about a quantity of an item. An event is about a container. No act
can qualify for both, and no act can fall between them. "Custody" invited the
ambiguity by naming neither subject.

## 2. Tested against what actually happens

| Physical act | What changed | Fact |
|---|---|---|
| Pick loose units from a bin into a tote | stock's holder: bin → tote | `stock_movement` |
| Pick 6 of 12 units out of a carton | stock's holder: carton → tote | `stock_movement` |
| Pick a whole carton onto an outbound pallet | the carton's parent | `package_event` |
| Forklift moves a pallet, bay A → bay B | the pallet's location | `package_event` |
| Shrink-wrap two pallets together | the pallets' parent | `package_event` |
| Goods arrive off a truck | stock enters the system | `stock_movement` |
| Goods leave on a truck | stock leaves the system | `stock_movement` |
| Quarantine a pallet's contents | stock's status | `stock_movement` |
| 3PL client sells stock in situ | stock's owner | `stock_movement` |
| Re-key a mis-recorded lot | stock's lot | `stock_movement` |
| Count a bin | nothing — an observation | `stock_count` |

**One physical act, one fact, every time.** No act produces two, none produces
none, and the assignment is forced rather than chosen.

**The subtle one is the third row, and it is the model earning its keep.** Picking
a whole carton writes no movement because **nothing happened to the goods** — they
never left their container, nobody counted them, nobody saw them. Recording a
stock movement would assert an inspection of the contents that did not occur. The
old model would have written forty movements for a pallet move, every one of them
a claim about goods nobody touched, which is precisely the lie D8 exists to
prevent.

**This is what makes it well defined:** the fact recorded is the fact observed.

## 3. My "two fact tables" objection was wrong

I said *"what did Daniel pick today"* becomes a UNION across `stock_movement` and
`package_event`, and called it an unstated cost. That was wrong, and the design
already answers it.

D25's `client_event` registry exists for exactly this — *"one physical act
produces many facts (a receipt writes a `package_event` and N movements; a cubing
scan writes an event and four observations)"*. One submission inserts one
`client_event` plus all its facts in one transaction.

So the work question is asked at the act layer:

```
client_event WHERE recorded_by_id = :person AND submitted_at::date = :day
```

and each act joins out to whatever facts it produced. That is **better** than a
UNION, and more honest: the **act** is the unit of work, and facts are its
consequences. It is also D8's "the work event is the invariant" implemented
properly rather than by scattering the work across the tables that record its
effects.

Two fact tables is not a fragmentation. It is one act layer over two
consequence tables, which is the correct shape when one act can have two kinds of
consequence.

## 4. Cardinality is a policy choice, not a forced consequence

Question 74 admits `stock` moves to LPN grain and waves at a partial index. That
undersells the answer.

**LPN grain is not inherently larger than location grain.** A pallet holds roughly
a location's worth of stock. Five SKUs on a mixed pallet is five `stock` rows; the
same five SKUs in a bin is also five rows. The keys differ; the counts do not.

Cardinality multiplies only if we mint a `package` row **per carton**. And a
package row exists only when something **identifies** it — an SSCC, a licence
plate, a scan. Cartons sitting in bulk on a pallet are not individually
identified and do not become packages until they are labelled or picked.

> **So cardinality is governed by when we mint packages, which is a policy we
> own — not a consequence of the key change.**

That is worth stating as a rule, because the failure mode is minting a package
per carton at receipt "for completeness" and discovering the cost later.

**And "rows are never deleted" is over-strong.** `stock` is a **projection**;
it is rebuildable from the ledger by definition. A zero-quantity cell with no
allocation referencing it carries no information that the ledger does not already
hold. The `never deleted` rule exists to protect `stock_allocation.stock_id`, and
an allocation against a zero cell should not exist anyway.

Corrected: **dead cells are reapable once unreferenced.** That is a maintenance
job, not an architectural constraint, and it removes the unbounded-growth concern
entirely.

## 5. The `lot_id` breakage — confirmed, with one rule still missing

Genuine, and the same class as `owner_id`: `lot_id` is in the `stock` key but
appears once on `stock_movement`, so re-lotting in place is inexpressible.

The audit that found it checked **all six key dimensions** and gave a reasoned
verdict on each — `item_id` deliberately single (changing item is a
transformation: two movements), `tenant_id` correctly single. That is the
invariant register working on its first run, and it is the strongest argument for
building it.

**The fix is right; the semantics need one sentence.** The generated
`lot_id = COALESCE(to_lot_id, from_lot_id)` preserves D14's recall index and query
verbatim — but it preserves the *index* while changing the *trace* through a
re-lot. Lot A re-lotted to B, then shipped: the despatch movement carries
`from_lot_id = B`, so *"which customers received lot A"* returns nothing.

Whether that is correct depends on **why** the re-lot happened, and nobody has
said:

- **Correction** — "we keyed A, it was always B". The trace **must not** follow.
  Those goods were never lot A, and following would produce a false recall.
- **Transformation** — a genuine merge or split. The trace **must** follow, or a
  recall misses stock.

**Recommendation: permit only the correction, and forbid transformation.** Lot
merging is already unacceptable in food traceability, and forbidding it keeps the
recall query exactly as D14 wrote it with no chain-walking to remember. If a
genuine split is ever needed it is two movements through a transformation, the
same treatment `item_id` already gets — which is a pleasing consistency rather
than a special case.

Then `stock_movement.adjustment_reason_id` distinguishes the two on the row, and
the partial index on `WHERE from_lot_id IS DISTINCT FROM to_lot_id` becomes a
correction audit rather than a required leg of every recall.

## Verdict

**Adopt the `lot_id` pair now.** It is small, certainly right, currently a live
defect, and the correction-only rule above closes it cleanly.

**Adopt the containment restructure**, with three amendments:

1. Replace the "custody / position" wording with the subject-based rule in §1.
   This is the only part that was genuinely unclean, and it is a wording fix to a
   correct design.
2. Record that **package minting granularity is a policy** — with the default
   being that a package exists when something identifies it, never "for
   completeness".
3. Soften "rows are never deleted" to **"not deleted while referenced"**, and
   schedule the reaper as a maintenance job.

Question 74 is then answered, and question 59's drift concern is answered
structurally rather than by a reconciliation job that could only ever be partial.

## Still open

89. **Does a `package_event` need an `activity_event` sibling for a failed
    container scan?** A carton scanned onto the wrong pallet and corrected leaves
    a `containment_conflict` finding, but a scan that resolved to nothing leaves
    no trace. D17's `location_empty` reasoning applies verbatim.
90. **What mints a package at receipt when the supplier sends no SSCC?** We
    generate one to have a holder — but §4 says minting is a policy, so the
    default for an unlabelled pallet needs stating rather than defaulting to
    one-per-carton by accident.
