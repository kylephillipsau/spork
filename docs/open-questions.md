# Open questions register

The single home for every open question. Before this file existed they lived in
five documents with a shared numbering scheme that had collided three times, and
questions raised in a proposal stayed behind when the proposal was adopted.

**The rule that keeps it true:** a question raised anywhere is migrated here in
the same commit that adopts the decision raising it. Research documents may state
questions; they do not own them. Where a research document and this register
disagree, this register wins.

Numbering is canonical here. Three collisions were resolved on consolidation: the
supply-side design's 110 and 111 duplicated numbers already used, and its
surviving live question was renumbered to 120.

---

## Live and blocking

**None.** Nothing on this list stops the first migration or the first screen. The
four that did were settled by D34, D35, D36 and D37 on 2026-08-03.

What replaces them is a measurement rather than a question: 122 below cannot be
answered without a seeded database, so it is deferred against building one rather
than left here looking like a decision nobody has made.

## Live, needing a business answer

**None.** All seven were answered on 2026-08-03.

Once answered, six of them turned out to be one principle asked in six places:
**every integration is a capability, never a dependency.** Orders, inter-company
documents and legal entities were the same question about three subjects. The
seventh, grocery business-to-business, was answered as roadmap rather than first
version, and what remains of it is a verification rather than a decision, carried
by 123.

Six of the seven were settled on 2026-08-03 by D39 to D42. What they had in
common, once answered, was one principle: **every integration is a capability,
never a dependency.** Three of them were the same question asked about orders,
about inter-company documents and about legal entities.

## Live, deferred against a trigger

| # | Question | Trigger |
|---|---|---|
| 26 | Who allocates, and when | Building the allocator |
| 28, 42 | Whether the allocator and task ordering can run before the location survey | The survey, or the interim sequence column |
| 29 | Whether equipment cost needs to model contention rather than a scalar | Forklift queuing becoming the bottleneck |
| 34 | Whether held-lot allocations auto-release or wait for a human | Building the re-allocator |
| 41 | Whether a count task locks its location | Building cycle counting |
| 43 | What closes a `pick_batch` | Building batch picking |
| 52 | Who governs the shared catalogue | The second tenant |
| 65, 81 | Whether a 3PL client is an owner scope inside a tenant | First 3PL client |
| 70 | Statutory retention against a tenant's right to deletion | First deletion request |
| 71 | Serial capture at receipt | A customer asking |
| 86 | The closed decision-point set | If `decision_rule` is ever built |
| 118 | Where the reaper's kill switch lives and who may flip it | Building the reaper (now a phase of the rebuild, D35) |
| 119 | Whether the internal licence plate format is shipped or tenant-configurable | The second tenant |
| 121 | Whether `event_subscription` needs a ceiling, and what it is | Measured outbox throughput |
| 123 | Verify the GS1 grocery surface is reachable without structural change: GTIN allocation, SSCC issuance and reuse, ITF-14 on master cartons, GS1-128 pallet labels, EDI despatch advice, National Product Catalogue. Carries the live half of 69 | Before the first grocery customer, and worth doing sooner because it is falsifiable now |
| 124 | Benchmarking's cohort floor, suppression rule and consent model. D41 set the shape and deferred the build | Enough tenants for a cohort to mean anything |
| 126 | Whether a counterparty's **message rules** are a `policy_kind` under D22 or a `party_profile` capability column under D20. Both fit; they differ in whether the rule resolves by scope. Raised by D43 as cardinality; widened by D44 to cover acknowledgement-required and structure-required, which resolve the same way | The second counterparty with a stated message rule |
| 129 | Whether `zone` needs to nest. D46 made it flat because D22's Space dimension is `any → site → zone` with no ancestors level, and a chilled area holding a chilled pick face and a chilled bulk run is the shape that would want one. It costs a closure table and one lattice row, exactly as Product already has | A tenant with sub-zones |
| 128 | What links a cancelled order to the one raised in its place. Metcash's stated amendment channel for an externally-authoritative order is cancel-and-reraise, and D44 added `order.supersedes_order_id` for the link without deciding what else the succession carries. Raised by D44 | The first EDI customer |
| 131 | What a correction to an already-reported period raises, and against what. Bitemporality means a correction recorded in September changes what we believe about August without changing the August invoice, and the resulting mismatch should be a finding rather than a silently moved number. There is no billing model yet, so the discrepancy kind has no name. Raised by D47 | A billing or period-reporting model |
| 133 | What separates a self-caught slip from a finding worth raising, and whether the queue ranks by it. A correction recorded seconds later by the person who made the mistake is categorically different from one a stocktake finds three weeks later, and nothing distinguishes them today, so both would arrive with equal weight and the queue would fill with typos. That is the packing-deviation failure arriving by another door: recorded and counted, raised only on the aggregate. `recorded_by_id` and the `recorded_at` delta are available; what is not is any record of whether anything consumed the value in between, and `work_session_id` is a bare uuid on `client_event` with no table or FK behind it, so "same session" is not a usable grouping. Cheap to state now, awkward to retrofit once corrections are flowing | Designing the findings queue |
| 137 | Whether an amendment fold's **base value** should be recorded. D42 makes the covered columns *"the original plus the amendments"*, and the original arrives on INSERT into the same column the fold then maintains. So base and result share storage: `projection_order_rebuild` is idempotent but not reconstructible, and resetting a covered column loses the original with no source to recover it from. The ledger folds do not have this property — truncate `stock.quantity` and J1 rebuilds it exactly. This is what limits J46 to the pairs an amendment determines. The candidate fix is an amendment row written at order entry carrying the initial values, which makes the fold complete at the cost of N rows per order; the alternative is a parallel `_original` column per covered column, which is the same information stored worse. Nothing is broken today, because nothing deletes the row. Raised by D51 | Before anything resets a covered column: a restore, a re-tenanting, or the reaper reaching `order` |
| 138 | How a platform-shipped policy value carries provenance. D22 requires a `policy_change` behind every value version, and a shipped default cannot have one: `policy_change` reaches `client_event` through `(tenant_scope_id, client_event_id)` and `client_event.tenant_id` is NOT NULL, so an act belonging to no tenant has no event to name. J13 exempts platform versions for that reason, which means **the one class of policy value every tenant inherits is the one with no recorded reason behind it**. The candidates are a platform-scoped `client_event` arm, a separate `platform_change` fact, or accepting that shipped defaults are release artefacts evidenced by the migration that inserts them. Raised by D52 | The first platform default a tenant disputes, or the first shipped revaluation |
| 141 | Whether primary keys move from UUIDv4 to a time-ordered UUID. Forty-three columns default to `gen_random_uuid()`, which is v4 and therefore random, so inserts scatter across B-tree pages instead of appending at the right edge. The published costs at scale are roughly 25% larger indexes, WAL amplification and order-of-magnitude slower bulk loads. UUIDv7 keeps the property this design actually needs — a handheld minting identifiers offline — while restoring insert locality. **Server half settled by D57**, migration 18: all forty-three defaults moved to `uuidv7()` and S47 keeps them there. What remains is the client half, and it is the larger one — D5 has handhelds minting identifiers offline, so the writer that produces most fact rows is the one that still generates v4. The `uuid` crate's `v7` feature is the other side. **It bites hardest where the design is already committed**: question 32 wants `activity_event` range-partitioned on `occurred_at` from its first migration, which a random key fights directly. This is `migration-1.md`'s own cheap-now-ruinous-later test applied to key generation rather than to a column. Raised by D55 | Before the first table that grows, which is `activity_event`, or a move to Postgres 18 |
| 145 | Whether `intention_amendment` reaches a purchase order. D42's sketch gave it three subject arms — `order_id | purchase_order_id | transfer_order_id`, exactly one — and migration 9 built the order arm alone. D51 then settled that one amendment names one subject and made that concrete for orders and their lines. Widening to purchase orders is the same shape a third time and is not free: `order_id` is NOT NULL, so a second subject means making it nullable, extending the subject CHECK that S41 derives from the catalogue, and deciding whether a purchase order line takes amendments the way an order line now does. Until then a purchase order's mutable columns move by UPDATE with no author, no moment and no reason — which is the state the order side was in before D51, and it was worth fixing there. Raised by D59 | The first supplier dispute about what was ordered, or the first inbound EDI purchase order acknowledgement |
| 146 | What `purchase_order.receipt_status` and `goods_receipt.status` mean. D25 adopted both as `@projection` — `none|partial|complete|over` for the first, `lines + movements` for the second — and neither is built, because neither is a fold. **What partial means for a receipt whose lines were accepted, rejected and matched in different proportions is a D15 question**, the same one D53 answered for `fulfilment.progress` by concluding that progress is three fractions and no single column can hold them. The receipt side may well have the same answer, in which case both columns stay absent and the numbers live on the lines. Raised by D61 | Before either is read by anything, or when the receiving screen needs a one-word answer |
| 151 | Whether the as-of closure reconstruction needs materialising. D76 derives the taxonomy's shape at an instant from the acts rather than storing a range per edge, on the grounds that there is no resolver to slow down, no measured scale to slow it at — `history.sql` builds no taxonomy — and that the audit it was raised for is better served by recording the decision than by replaying the world. **All three of those stop being true at the same moment**: a resolver exists, it calls the reconstruction, and something times it. The materialised form is a range per edge on both closures with a maintainer to keep it, and D22's resolver walks it on every lookup. Raised by D76 | A resolver exists and a measurement shows the reconstruction too slow for something that calls it |
| 167 | Whether a mutual-exclusion CHECK with fewer than two arms is detectable. `stock_movement_cause_ck` was `num_nonnulls(goods_receipt_line_id) <= 1` from migration 21 to migration 53 — one argument, so it returns 0 or 1 and is **always true**. It excluded nothing for thirty-two migrations. S3 read it the whole time and passed, because S3 tests the *form* of the rule, `<= 1` rather than `= 1`, and never asks whether the rule has anything to say. D99 fixed this instance by adding the second arm, and the class is untouched: the next cause set built one arm at a time does the same thing, and the check that exists to govern it will confirm it is correct. This is D45's own sentence about S3 — *"a vacuous check cannot be wrong out loud"* — restated as a property S3 could test, since the arity of `num_nonnulls` is in `pg_get_constraintdef` and countable. The register already distinguishes a vacuous invariant from a passing one and reports the count; nothing does the same for a vacuous constraint. Raised by D99 | The next `%_cause_ck` or `%_demand_ck`, which is the `discrepancy` cause set if it ever moves onto the ledger |
| 160 | Which code list a `raw_unit_code` came from. D93 keeps the author's word verbatim and does not record whether it is X12 or UN/ECE Recommendation 21, **and the two overlap in spelling without agreeing in meaning**, so a bare `CA` is not self-describing. The argument for leaving it out is that it is knowable from `party_message`, whose artefact records its own standard, and a copy here is a second place for it to be wrong; the argument against is that resolution happens per line and the join to find out is not free. This is the same shape as 125 — whether a `raw_*` column may hold a value inherited from another node — asked about provenance rather than about content. Raised by D93 | The first counterparty sending a code that means different things in the two lists, or the first resolution that guesses wrong |
| 175 | Who committed this stock. `stock_allocation` carries **no actor at all** — no `recorded_by_id`, no `recorded_at`, no `client_event_id` — so "who claimed these forty units for that order, and when" is unanswerable, and D24's re-allocator will make it worth asking: a firm claim somebody set deliberately and a speculative one a batch job made look identical. **Q174 deliberately did not widen into this.** The retry problem was solved by the client-minted identifier, which is the half of D5 that was already doing the work, and adding columns is a different decision with arguments on both sides: S19 asks *facts* for a `client_event` and D12 makes an allocation an Intention, so the envelope is not owed — but architecture.md's *"every movement and every scan records an individual"* is about acts, and somebody claiming stock performed one. The cheap half is `recorded_by_id` and `recorded_at`; the expensive half is deciding whether an Intention may name a `client_event`, which no table currently does. Raised by the Q174 endpoint work | Building the allocator (question 26), or the first dispute about a firm claim nobody will own |
| 176 | What a `person_tenant.role` means. It is a NOT NULL text column with no vocabulary behind it, and migration 70 authenticates without reading it: membership decides which tenants a person may act for and **nothing decides what they may do there**, so any signed-in member can do anything the application can. Defining the set is a business question -- packer, receiver, supervisor, whoever -- and inventing one inside an authentication migration would be exactly the undesigned language D22 refuses for policy, arriving through a different door. The shape is already there twice over: `person_tenant.role` and `work_session_member.role` in D11's sketch, which should almost certainly be one vocabulary rather than two. Raised by the Q171 work | The first person who must not be able to do something, or the first tenant with more than one kind of worker |
| 177 | Whether a work session is declared at sign-on. D11 specifies one -- *"declared at sign-on, not inferred"* -- with `work_session` and an append-only `work_session_member` carrying joins and leaves, so *"who was on this team when that movement happened"* stays answerable and nobody can be added to a past window invisibly. Migration 70 builds the session that authenticates and not the one that records a crew, so `client_event.work_session_id` is still the bare uuid question 133 complains about: with no table behind it, "same session" is not a usable grouping and the findings queue cannot rank a self-caught slip differently from a stocktake's discovery. **The two are not the same object** and conflating them would be the mistake: one is who is signed in, the other is who is working together. Raised by the Q171 work | Building the picking screens, or the first shared task without a scanner each |
| 153 | Which party is us. D21 makes assertions symmetric and `author_party_id` is NOT NULL, so an **outbound** claim must name a `party` row for the operating company — and there is no such row, nor anything that would mark one as us rather than as a counterparty. D77's fixture adds one to exercise the outbound arm, which is a fixture solving a schema problem. It reaches further than assertions: 65 and 81 ask whether a 3PL client is an owner scope inside a tenant, and both questions are about the same missing distinction between the tenant and the parties it trades with. Raised by D77 | The first outbound EDI document, or the first 3PL client |
| 178 | What the `Weight` column on an item fulfilment export actually measures. It is two different things: where `Prepack` names an item code — 78 of them — the weight is the prepack list's carton spec and never varies (`SKU-2008` is always 6.6 kg, `SKU-1230` always 17.7), and where it names a box, it varies per shipment (`Pallet` takes 146 distinct values). A per-unit weight derived by dividing the first kind by a line quantity is a catalogue constant divided by an unrelated number, which is how `DGN-0060` produced both 11.4 and 5.70 kg per unit from the same 11.4 kg carton. **A loader that did this was written, dry-run and deleted rather than shipped.** The question is whether the box-named weights are a scale reading or a typed estimate, because only the first would let this system learn what things weigh from despatched work | Somebody who knows whether the dock weighs a pallet before it goes |

## Live, wanting a written answer rather than a decision

Nothing blocks on these. They are places where the model is right and the reasoning
is undocumented, which is how the five bad invariants happened. The largest of
them, 88, was settled by D38; [invariants.md](./invariants.md) is now the same
fix applied to invariants that this file is to questions.

| # | Question |
|---|---|
| 84 | Bitemporal queries are easy to write backwards. "What did the pallet weigh on Monday" and "what did we believe on Monday" differ by one predicate, and getting it wrong in a dispute is worse than not having the capability. |
| 85 | The falsifier for new event tables is gameable by adding a decorative column. Reviewers must apply the provenance rule first. |
| 87 | Reason fields are only as good as what gets typed into them. Mandatory-not-null produces "update". |
| 15 | What the reconciliation surface for negative stock actually is. D5 tolerating negative balances is only defensible if somewhere real resolves them. |
| 105 | What an `automation_key` is. D27 narrowed it by contrast (a device is *how*, a key is *who*) without defining it. |
| 62 | Whether D8's non-blocking rule gets a counterparty carve-out, and where the statutory clock lives. |
| 125 | Whether a `raw_*` column may ever hold a value inherited from another node, stated generally rather than per standard. D43 answered it for the X12 order level by storing the node; any hierarchical message can state an attribute above the level we store, so the 856 is the first instance and not the last. |

## Live, minor

Noticed thresholds and consistency questions, none urgent: 5, 16, 22, 23, 27, 35,
36, 54, 94, 96, 97, 98, 100, 104.

---

## Settled

| # | Settled by | # | Settled by |
|---|---|---|---|
| 1, 32 | D14, D20 | 68 | D24 supply side, refined by D43 |
| 127 | D50 | 135 | D51 |
| 136 | D52 | 134 | D53 |
| 140 | D63 | 139 | D64 |
| 122 | D65 | 142 | D66 |
| 76 | D67 | 147 | D69 |
| 80 | D70 | 120 | D71 |
| 78 | D72 | 95 | D73 |
| 149 | D74 | 150 | D75 |
| 148 | D76 | 155 | D79 |
| 93 / 79 | D81 | 154 | D85 |
| 156 | D86 | 152 | D90 |
| 157 | D91 | 158 | D92 |
| 159 | D93 | 161 | D94 |
| 163 | D95 | 162 | D96 |
| 166 | D98 | 165 | D99, D100 |
| 168 | D102 | 170 | D103 |
| 169 | D104 | 143 | D105 |
| 172 | Q172 endpoint work | 173 | Q173 endpoint work |
| 174 | Q174 endpoint work | 171 | Migration 70 (D11's session, built) |
| 164 | D106 | | |
| — | D107 (mediated refresh; no prior numbered question) | | |
| 132 | D54 | | |
| 130 | D48 | | |
| 2, 3 | D15 | 72 | D33 |
| 13 | D8, D25 | 73 | D26 |
| 14 | D26 | 74 | D29, D30 |
| 17, 18, 19 | D10, D11, D9 | 89 | D28 |
| 20 | D23 | 90 | D29 |
| 21, 25, 30, 33, 39 | D22 | 91 | D30 |
| 24 | D13 | 92 | D24 |
| 31, 44 | D33 | 99 | D27 |
| 37 | D16 | 101, 113, 114, 115 | D31 |
| 38, 40, 60 | D24 supply side | 103 | D24 supply side |
| 45, 55 | D20 | 106 | D24 supply side |
| 46 | D20 | 107, 108, 109 | D24 supply side |
| 47, 48 | D18, D20 | 110 (supply-side's) | D28 |
| 50, 51 | D19 | 112 | D28 |
| 53, 57 | D32 | | |
| 59 | D24 | | |
| 61 | D21 | 116 | D34 |
| 63 | D22 | 102, 117 | D35 |
| 64 | D23 | 111 | D36 |
| | | 75 | D37, into 122 |
| | | 88 | D38 |
| | | 4, 58 | D39 |
| | | 49, 56 | D39 |
| | | 66 | D40 |
| | | 67 | D41 |
| | | 77 | D42 |
| | | 69 | answered; verification half is 123 |

**68 and 109 were settled correctly and refined rather than reopened.** D43 found
that one of the two reasons D24 (supply side) gave holds for EDIFACT and not for
X12, which states the purchase order at the order node rather than on the line.
The conclusion stands, the order level is now stored as a node, and neither
question returns to the live list. The stale text in `inbound-analysis.md` and
`supply-side-design.md` still describes 68 as undecided; this register is
authoritative, which is the rule working as intended.

## Duplicates, resolved

| Number | Duplicated | Kept |
|---|---|---|
| 79 | 93 | 93 |
| 82 | 66 | 66 |
| 83 | 67 | 67 |
| 117 | 102 | both, settled together by D35 |
| supply-side 110, 111 | domain-model 110, 111 | renumbered; the live one is 120 |

The competitor analysis carried its own sequence of eleven questions predating this
numbering. All were absorbed into 1 to 57 during adoption and are superseded.

---

## Counts

| | |
|---|---|
| Live and blocking | 0 |
| Live, business answer | 0 |
| Live, deferred with trigger | 36 |
| Live, wanting a written answer | 7 |
| Live, minor | 14 |
| **Live total** | **57** |
| Settled | 110 |

**Re-derived under D107**, which settled the mediated-refresh product gap left by
D95/D106 (no new open-question number) and raised nothing. Live totals unchanged;
decisions reach 107. Three layers: live ledger views on write/GET (no schema),
`projection_dirty` + `projection_run_dirty` for the scheduler, and rate-limited
`projection_refresh_tenant` for the app — never bare EXECUTE on `projection_run_all`.

**Re-derived under D106**, which settled 164 and raised nothing, taking the deferred
section to 32 and the live total to 53. Settled reaches 106. Full-tenant folds stay;
the year fold dropped from ~12.8 s to ~0.55 s by scoping D103's effective sum to the
roots each maintainer needs rather than joining the global view. O(delta) waits until
a step's cost is a material fraction of its freshness bound.

**Re-derived under D105**, which settled 143 and raised nothing, taking the deferred
section to 33 and the live total to 54. Settled reaches 105. The only schema change
is a CHECK on `stock_movement.reason`; the rest of 143 is the classification written
once, including that `stock_allocation.state` stays text-with-CHECK until its state
machine gains a value — 143's own trigger, left in place as a conversion note rather
than re-opened as a question.

**Re-derived under D104**, which settled 169 and raised nothing, taking the deferred
section to 34 and the live total to 55. Settled reaches 104. Structural: a projected
column has one writer (S55) and every maintainer names the migration that last
changed it (S56). The deeper half of 169 — fixture shapes rather than tables — stays
carry-forward rather than a live question, because it is a discipline rather than a
decision.

**Re-derived under D103**, which settled 170 and raised nothing, taking the deferred
section to 35 and the live total to 56. **The first decision in this run to close a
question without opening one**, which is what finishing a thread looks like: 165 to
168 to 170 was one question asked three times at increasing depth, and the third
asking had an answer that needed no fourth.

**Re-derived under D102**, which settled 168 and raised 170, so the deferred section
held at 36 and the live total at 57 for the third decision running. Settled reached
102.

**Re-derived under D101**, which raised 169 and settled nothing, taking the deferred
section to 36 and the live total to 57. **The fixture raised it, which is new**: every
other entry here came from a decision, a walk or a review, and this one came from
adding two rows of test data that reached a shape nothing had reached before.

**Re-derived under D100**, which settled the rest of 165 and raised 168, so the
deferred section held at 35 and the live total at 56 for the second decision
running: one question left and one arrived. Settled passes 101.

**165 is the first question this register carried half-settled**, from D99 to D100,
and the shape was worth the awkwardness. It kept its number, its trigger and the
outbound walk's demonstration while losing only the part that had an answer; closing
it at D99 and raising a successor would have cost the walk's evidence its question
and made 165's remainder look freshly found rather than half done.

**Re-derived after the outbound walk**, which raised 165 and 166 and settled
nothing, taking the deferred section to 35 and the live total to 56. **Both walks
have now raised more than they closed, and that is the expected shape**: a walk is
the first thing here that exercises the system rather than inspecting it, so it
reaches places no check was pointed at. What it raises comes with a demonstration
attached, which is why 162 and 163 were settled within two decisions of being
found.

**Re-derived after the inbound walk**, which raised 162 and 163 without settling
anything, and again under D95, which settled 163 and raised 164, and again under
D96, which settled 162 and raised none. The deferred section went 32 → 34 → 33 and
the live total 53 → 55 → 54.

**The walk is now the thing that raises and the thing that closes.** 162 and 163
both came from performing a delivery rather than from reading the schema, and both
were settled within two decisions of being found. That is a shorter loop than this
register has recorded before, and it is worth watching whether it holds: a question
found by executing the system has a demonstration attached, so what would settle it
is already written down.

**Re-derived under D91, D92, D93 and D94.** The first two swapped one for one —
157 out and 158 in, then 158 out and 159 in — so 32 + 7 + 14 = 53 stood unchanged
across both. D93 settled one and raised two, taking the deferred section to 33 and
the live total to 54. **D94 settled one and raised none**, which is the first time
in six decisions, and the section is back to 32 and the total to 53. Counted from
the tables rather than adjusted, which is the rule the five drifts below were
caused by breaking.

**Settling one and raising one has now happened six times**, and the last two are
the clearest instances of why. 158 was not found by thinking about the answer to
157 — it was found by the fixture, which made a subtraction in D91's new function
wrong by a factor of ten and could not have done so if it had held one receipt
line instead of two. 159 was then found by the *check* D92 wrote to settle 158,
which caught a claim asserting 400 base units against 40 in a unit of factor 1/1
that had been sitting in the fixture since the assertion tables existed.

The register keeps recording that implementation finds questions. These two
narrow it usefully: a deterministic fixture finds them, and so does a check
written to close the previous one. Neither is thinking harder.

**D94 raised nothing, and the reason is worth more than another entry.** It was
settling a question about a mechanism rather than designing one, and the two
defects it found had both been *stated correctly in this register and in the
decision record* while being absent from the database. There was nothing left to
ask. What it did instead was turn on a harness that had been reporting success
without running: three tenancy tests printing `DATABASE_URL_APP unset` on every
run since they were written. **A register that counts vacuous checks separately
from passes had three tests passing vacuously and no way to see it**, which is the
same lesson as S5 and D49 arriving at the test suite rather than at the schema.

**D93 adds a third finder and it is the least comfortable of them: reading the
standard.** 159 was raised as "can a claim say cartons" and the research answer was
that UN/ECE keeps units and package types in two separate recommendations, which
is the division D58 had already reasoned its way to — so the register had been
asking a question the schema half-answered years earlier and had failed to apply
to one table. 160 and 161 then came out of writing the migration, one from a
column deliberately left off and one from testing whether the columns added could
ever be corrected. **161 is the first question here found by running as a
different database role**, which is a check nothing in the suite performs, because
the harness that would has been skipping silently on every run.

**Fifth drift, and it was in the sentence claiming the counts are derived.** This
line read "the deferred section holds 35" while the table above it said 30, and
D72 first bumped it to 37 — adjusting the stale number rather than re-deriving it,
which is the exact mistake the paragraph warns against, made by the paragraph. The
sections were then counted under D51's convention, splitting rows that carry more
than one number and counting `93 / 79` once: **32 + 9 + 14 = 55** at that moment; 95 and 149 have since been settled, leaving 53, and the table
above is right. The deferred section holds 32.

Settling one and raising one ran three times
in a row — 136 out and 138 in, 134 out and 139 in, 132 out and 140 in — and then
D55 raised three without settling any, because an audit finds things rather than
answering them. The count is re-derived below each time rather than adjusted,
which is the whole point of having done it once.

Worth noticing rather than celebrating. Those three consecutive decisions each
closed a question and opened one, and in every case the new question was found
**by implementing the answer to the old one** rather than by thinking harder about
it: 138 from writing J13, 139 from writing S43, 140 from a draft of S45 that was
then narrowed away from it.

**141, 142 and 143 came from somewhere else, and that is the point of them.**
They are the first questions on this list raised by checking the schema against
outside practice rather than against itself. So is D55, which found a tenancy hole
that had been open since migration 1 and that every check in the suite was
structurally unable to see. A register that only ever asks itself questions will
keep answering the ones it already knows to ask.

**Fourth drift, and this time it was understating by five.** D51 came to settle
135 and raise two, so the counts were re-derived rather than adjusted: every
question number in each section's table was counted, splitting the rows that carry
more than one. The deferred section held 31 numbers against a stated 27, and the
written-answer section 9 against a stated 8. `93 / 79` is one question with a
retired duplicate number and counts once, per the duplicates table below.

So the previous total was 54 and read 49. Note what the last three corrections
have in common: **each was found by whoever next needed to touch the file, never
by the file itself.** The paragraph below this one already said the total is
derived from the sections rather than carried forward, and it was not — for the
second time. A note asserting a property is not a mechanism enforcing one, which
is the same lesson D49 drew about `@projection` and S5 drew the hard way.

**The previous total was one short of its own sections.** It read 42 against
21 + 8 + 14 = 43, and D43's two additions surfaced it rather than caused it. The
total is now derived from the sections rather than carried forward, which is the
same failure mode as the invariant numbering and has the same fix: generate it.

All four blockers were settled the day this register was written, and settling
two of them raised 121 and 122. 116 had been sitting in a research document since
the inbound pass without ever being carried across, which is the failure this
register exists to stop repeating. 121 and 122 were migrated here in the commits
that adopted the decisions raising them, which is the rule working.
