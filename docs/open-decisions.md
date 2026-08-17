# Open decisions: recommendations

Companion to [domain-model.md](./domain-model.md). My position on each live open
question, so there is something concrete to argue with. Numbering matches that
document.

Grouped by how much of the answer the stated philosophy already decides:

- **A** — the philosophy decides it. I would just do these.
- **B** — genuine judgement calls. I have a lean, you have the context.
- **C** — one I think is being under-weighted.

---

## A. Where the philosophy decides it

**41. Count tasks do not lock a location.** *(The one I flagged.)* Locking stops
the floor to protect a number, which inverts D5. But "tolerate the noise" is not
the alternative — the ledger already knows a pick landed at 10:32 in a cell being
counted, so we can **annotate the finding**: *"you counted 47; 3 units were picked
from here during your count; accept 50?"* That is strictly better than a lock,
because it neither blocks work nor discards the explanation, and it is D9's
point-of-capture principle applied to timing rather than to quantity.

**30. `allocation_policy` changes are facts.** Append-only versions, and every
allocation records the policy version that produced it. This is the direct
consequence of D8: you want to correlate a weight change against a later shift in
findings. Without it, "did raising the rotation weight reduce spoilage?" is
unanswerable, and tuning becomes superstition.

**44. `activity_event.kind` is an enum.** A lookup table invites per-site custom
kinds, which is the rules-engine-by-accretion D13 warned about. Adding a kind is
a migration, and the set changes rarely.

**20. `measurement` gets typed subject FKs.** I argued the cold-path exception
when I raised it; I now think that is wrong. The packing station loads current
dimensions for every item on a fulfilment — that *is* a hot path, so D10's
batch-loading argument applies directly rather than by analogy.

**31. `tracking = lot` is enforced in the application, plus a periodic
assertion.** Same pattern as `stock` reconciliation, rather than a trigger.
Consistency of mechanism matters more here than strictness: one way to find
inconsistency, not two.

**15. The reconciliation UI for negative stock already exists.** It is the D8
discrepancy queue — negative balance is a `kind`, not a separate feature. Worth
noting because it means D5's tolerance of negative stock is not a loose end.

**27. Allocation does not cross sites.** Multi-site fulfilment of one order is
already expressible as multiple fulfilments (D15). Nothing to add.

**23. Movements may have no `work_session`.** Nullable, as modelled.

**43. A `pick_batch` closes when all tasks are terminal, plus an explicit
abandon.** The abandon is itself a recorded decision with a reason.

**28 + 42. Add `location.sequence` now.** Both questions have the same answer and
the same blocker. A nullable manual sequence is the interim travel proxy for
D13's scoring *and* D17's task ordering, it costs one column, and it means
neither feature waits on the survey. Peoplevox runs its whole operation on
exactly this.

**5. `base_unit` stays**, answered by `entered_quantity`/`entered_unit`. The real
exception is catch-weight items — anything sold by actual weight rather than
count — which breaks the assumption entirely. See question 45.

**32. Lots may exist before their goods arrive.** `received_at` nullable; expiry
reporting filters on stock existing. Supplier ASNs name lots ahead of delivery,
and refusing to model that would push the information into a spreadsheet.

**36. Packages from different customers may share a consignment.** The schema
permits it; whether it is commercially sensible is a `carrier_profile`-style
policy. D13's pattern.

**35. Both.** Tracking is per consignment and per package; packing lists are per
fulfilment. Forcing one answer would break either consolidated freight or
per-order paperwork.

---

## B. Judgement calls — my lean, your call

**14. Nosdesk: service boundary, shared crates.** Two services, one Cargo
workspace, with the sync transport and auth as shared library crates. The
deciding argument is blast radius: **the warehouse must keep shipping when the
helpdesk is down**, and a shared deployment makes that a coincidence rather than
a guarantee. Shared crates still avoid writing the sync layer twice.

**16. Nothing here needs a document CRDT.** Stock is ruled out (D5), and no other
surface has two people editing one structure simultaneously. If that stays true,
Nosdesk reuse is transport-only, which makes 14 easier.

**26 + 25. Allocate late; release explicitly; expire by policy.** Allocating at
batch release rather than order import minimises churn and re-planning. Add an
explicit "commit" action for orders where ATP matters commercially. Expiry is a
manager-owned duration, not a hard-coded TTL — an allocation held for a week is a
lie that suppresses availability for everything else.

**34. Held-lot allocations raise a finding and are not auto-released.**
Auto-release is convenient and quietly discards what the plan had been. Offer a
one-click re-allocate on the finding instead: same speed, evidence preserved.

**22. A `work_session` opens automatically on first scan and closes on
inactivity**, with explicit open/close available for crews. If sign-on is a
chore, it gets worked around, and D11's accountability model degrades to
whoever-was-logged-in. Auto-open with easy correction beats mandatory ceremony.

**21. Tolerances at site, item class, and item** — three levels, most specific
wins. More grain is config sprawl; fewer cannot express "controlled items are
strict, packaging is not".

**33 + 39. `min_shelf_life` at customer × item class, plus a `site` floor.**
Retailers genuinely differ by category. The site floor answers 39: a transfer has
no customer, but site B serves customers who do, so shipping it stock with 30 days
left should fail at the point of transfer, not on delivery to the end customer.

**38. A transfer variance becomes a finding only once the receipt is closed**,
not while the truck is mid-journey. Otherwise every reporting boundary generates
noise.

**29. `relative_cost` is enough.** Equipment *contention* is a scheduling problem
and modelling it properly is a large commitment for a benefit we cannot yet
measure. If forklift queuing becomes the bottleneck, it is a scheduler beside the
allocator, not a change to it.

**4. `order` is ours; NetSuite ids live in a mapping table.** An
`external_reference(system, entity_type, entity_id, external_id)` table keeps the
mirror from leaking into the model (D2), and it is the same mechanism we will
need for MachShip and, later, carriers.

**40. In-transit stock cannot be allocated yet, and that is a real limitation.**
Committing inbound supply to outbound demand is standard practice, but our
allocation targets a `stock` cell (D12) and in-transit stock is in no cell. Doing
it properly means allocating against a *supply source* — a cell, an inbound
receipt line, or a transfer — which is a genuine generalisation of D12, not a
patch. Worth doing when inbound is built, not before.

---

## C. The one I think is being under-weighted

**Multi-tenancy is not on the deliberately-not-building list, and I no longer
think it should be dismissed.**

The competitor analysis flagged it as an unstated assumption. My initial instinct
was "single tenant, `site_id` is the partition, write it down and move on".
Looking at Nosdesk, I think that is wrong.

Nosdesk is a **commercial product**: BUSL licence, `license_private.pem` /
`license_pubkey.pem`, a licensing module in the backend, hosted and RC deployment
targets. That is not the shape of an internal tool. If this warehouse system
follows the same path — and the stated goal of surpassing NetSuite is a product
ambition, not an internal-tooling one — then **tenancy is not speculative, it is
deferred.**

And it is the single most expensive thing on the list to retrofit. Every table,
every query, every index, every cache key.

Three honest options:

1. **Single tenant, stated.** Cheapest now. If it is ever wrong, it is a rewrite
   rather than a migration.
2. **Tenant column everywhere from day one.** Roughly the cost of the `site_id`
   denormalisation already recommended, applied to one more column, plus row-level
   security. Unglamorous insurance.
3. **Database-per-tenant.** Zero schema cost, real operational cost, and it
   matches how a BUSL product with self-hosting is usually deployed anyway.

I lean **3 for the product path and 1 for the internal one** — and the reason to
decide now is that option 2 is the only one that is cheap *today* and impossible
*later*. Given your stated preference for the higher-quality option on
architecture and interoperability, and given that you have already built one
product this way, I would want an explicit answer rather than an assumption.

---

## New questions these raise

45. **Are there catch-weight items** — sold by actual weight rather than count?
    If so, `base_unit` is not sufficient and quantity needs a weight dimension,
    which touches allocation, picking and invoicing. Food makes this likely.
46. **If policy edits become facts (30), does the same apply to `carrier_profile`,
    tolerances and shelf-life rules?** Consistency says yes: one
    `policy_change` fact table rather than per-policy history.
47. **Does the product path change anything else?** Tenancy is the big one, but
    per-tenant branding, data export and deletion, and upgrade cadence all follow
    from the same answer.
