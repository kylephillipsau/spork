# The outbound EDI boundary: four questions

The four proposals left over from [gs1-grocery-analysis.md](./gs1-grocery-analysis.md)
after D43 took the two corrections. Where a counterparty's disposition of our
outbound assertion lives; whether D5's non-blocking rule survives a contractual
"do not despatch until acknowledged"; whether a retailer-issued purchase order is
an assertion as well as an order; and whether publishing to the National Product
Catalogue is in scope.

> **Questions and invariants raised here are not owned here.** The registers are
> [open-questions.md](./open-questions.md) and [invariants.md](./invariants.md).
> The proposals at the end are unnumbered; numbers are allocated in the commit
> that adopts.

Generated 2026-08-04.

**Provenance.** The Australian material is primary: *Metcash Purchase Order
Response v2.0* (EANCOM 2002, April 2018) and *Metcash Despatch Advice v1.10*, both
read in full. Standards-level claims about acknowledgement layering are from
vendor and Oracle documentation rather than from the X12 and EDIFACT specs
themselves. The NPC attribute count could not be established from public sources
and the recommendation does not rest on it.

---

## Verdict

Three of the four answer cleanly onto tables that already exist. The fourth
should be refused in writing.

**But the research surfaced something none of the four asked about, and it is
larger than any of them.**

> **Price is on the customer's purchase order, we are contractually required to
> confirm it, and the model has no place to put it.**

Metcash: *"The POA is used to confirm the price on the PO"*, and *"the amount
payable is calculated by reference to POA confirmed price and the quantity
received."* `order_line` is `id, order_id, item_id, quantity_ordered`. Confirming
a price we cannot store is not possible, and this is not the invoicing that D40
put out of scope — it is agreeing a commercial term on an intention, which is
what an order is.

That is the finding to act on. The other three are cheap.

---

## 1. The acknowledgement lifecycle is two layers, and each has a home already

The question assumed one thing. There are two, they arrive by different
mechanisms, and conflating them is how this gets built wrong.

**Layer 1, transport and syntax.** EDIFACT `CONTRL`, X12 `997` or `999`. It says
the interchange arrived and parsed. Metcash: *"An automated Functional
Acknowledgement (FA) at interchange level is expected for all B2B documents
exchanged between Metcash and Suppliers"*, in both directions and for every
message type, returned within three hours.

This is a property of **the message**, not of the claim inside it. `party_message`
already carries `parse_status` and `parser_version` — our parsing of an inbound
message. Its mirror is their parsing of our outbound one, and the acknowledgement
itself arrives as its own inbound `party_message`. So the linkage is
message-to-message: one nullable self-referencing FK,
`acknowledges_party_message_id`, plus the acknowledgement's own outcome.

**Layer 2, application disposition.** EDIFACT `APERAK`, X12 `824`, or in Metcash's
case a proprietary exception process. It says the business accepted or rejected
the claim, with reasons, at header and line level. From the DESADV guide: the
whole ASN is rejected if it relates to more than one purchase order, spans more
than one delivery, reuses an ASN number inside 24 months, names an invalid or
closed PO, or omits the shipment reference; a line is excepted if the GTIN is not
on the PO, the SSCC was used in the past twelve months, the quantity exceeds the
PO, Ti/Hi or unit of measure differ, or shelf life is outside agreed limits.

This is **a counterparty's statement of record about our claim**, authored by
them, held by them, and quoted back in a chargeback. That is D21's definition
without amendment. It is an assertion, inbound, with `author_party_id` set to the
retailer and a body naming the assertion it responds to.

**The symmetry is worth stating because it is load-bearing.** `assertion_stance`
is *our* position on *their* claim, and it is a fact because it is ours. Their
position on *our* claim is *their* assertion, and it is immutable because it is
theirs. The model already has both halves of the pattern and has only ever used
one of them.

### D5 survives, and the rule it needs is D8's

Metcash: *"If the supplier does not receive a FA from Metcash after sending an
ASN, the supplier should not despatch stock against that ASN."*

That is a counterparty-imposed block on our own physical act, and D5 forbids the
ledger blocking on coordination. The two do not actually collide. D5 is about
what the ledger refuses to record; this is about what the floor should do. The
consistent answer is the one D8 gives everywhere: **despatching against an
unacknowledged advice raises a finding, it does not raise a lock.**

Two reasons beyond consistency. A lock is unenforceable — the truck leaves whether
or not the software agrees — so a lock would produce an unrecorded despatch, which
is strictly worse than a recorded one with a finding attached. And the requirement
is per-counterparty: Metcash states it, and it has no force for a customer who
sends no acknowledgement at all.

Which makes it the same shape as question 126, already in the register: a
counterparty's message rules are either a `policy_kind` under D22 or a
`party_profile` capability under D20. **Acknowledgement-required belongs in
whichever answer 126 gets**, and it is a reason to widen 126's wording from
"message-cardinality rules" to counterparty message rules generally.

---

## 2. A retailer's purchase order is both, and the model already has the shape

D39 settled that orders are ours: created here, complete here, with
`source_channel` and `external_ref` recording provenance and one system of record
per order. D21's cut is that **a copy exists outside our control**.

For a retailer-issued purchase order, both are true and neither displaces the
other. Coles' `ORDERS` message is a document Coles authored, holds, and will quote
back in a dispute. The `order` row we create from it is our own intention, which
we plan and fulfil against.

**This is exactly the `inbound_shipment` pattern, one direction over.** D21
already reasoned it through: *"`inbound_shipment` is a subject, not an assertion.
Filing it as an assertion means a resend mints a second row and orphans every FK
pointing at the first."* A customer's purchase order has the identical failure
mode, and the identical fix: the document is an assertion, the order is the
subject, and the assertion body carries `resolved_order_id` the way
`despatch_advice` carries `inbound_shipment_id`.

**The kind set already contains the reply and not the message.**
`assertion.kind` is `despatch_advice | carrier_status | equipment_docket |
delivery_receipt | order_response | price_advice`. `order_response` is the
outbound ORDRSP. Nothing holds the ORDERS it responds to, which is a gap the
enumeration itself points at.

### Two things fall out that the question did not ask

**Order amendments arrive as cancel-and-reraise, not as amendments.** Metcash:
*"Metcash will NOT be implementing an EDI Purchase Order Change (POC) message"*,
and *"there may be a need for the Stock Controller to cancel the PO and re-raise a
new PO in its place."*

D42 made an amendment to an intention a fact, folding `intention_amendment` into
the order's covered columns. That is right for orders we author. For an
externally-authoritative order it is the wrong channel by contract, and D39
already said so: *"an order whose record of authority is external is amended
through that system, not here."* What is missing is the link — `order` has no
successor or predecessor reference, so a cancel-and-reraise pair is two unrelated
rows and the second one's history starts from nothing. One nullable FK, and it is
the same shape as `assertion.supersedes_assertion_id`.

**A refused line is a zero, and it carries meaning.** *"If a PO line item cannot
be fulfilled it must be returned on the POA with a zero quantity."* That is the
`OutstandingReason` idea the inbound analysis wanted to steal from Peppol,
arriving from the other direction: "none of this is coming" is a counterparty
intention no arithmetic over our facts produces. It belongs on the outbound
order-response body, not on `order_line`.

---

## 3. Price is the actual gap

Neither of the four questions is about price. All the primary evidence is.

Metcash's POA validations name the line-item fields we are confirming: **GTIN, PO
Line Item Number, Price, Quantity, Pack Size, Ti-Hi, Unit of Measure, Shelf
Life**. Accepting in full *"means that all information that was sent to the
supplier was correct in every aspect and can be delivered in full, i.e. Price,
Quantity, TUN, TiHi, Pack Size, Unit of Measure, Delivery Date."*

Against the model:

| Field we must confirm | Where it lives |
|---|---|
| GTIN | `item_barcode` (D34) |
| Quantity, unit of measure | `order_line`, D23's unit vocabulary |
| Pack size, Ti/Hi | `item_packing_config` |
| Shelf life | `lot`, and the receiving policy |
| Delivery date | `order.promised_*`, still an unadopted recommendation |
| **Price** | **Nowhere** |

`order_line` is `id, order_id, item_id, quantity_ordered`. `consignment` carries a
freight `price_minor`. `stock_movement.unit_cost_minor` was recommended and
deferred. Nothing holds what the customer agreed to pay for a line.

**This is not the scope D40 declined.** D40 kept invoice rendering out and said
the system produces the charge lines and what they were computed from. A price on
an order line is upstream of that: it is a term of the intention, which is what
the counterparty is asking us to confirm and what the payable amount is later
computed from. Metcash: *"Unless otherwise agreed by Metcash, the amount payable
is calculated by reference to POA confirmed price and the quantity received."*

Two smaller confirmations sit alongside it, both pleasing:

- *"Backorders will not exist under eTrade, but a linked Purchase Order will be
  used to retain any existing Price."* D12 eliminated backorder as an entity and
  the reasoning holds against a real trading partner's rules.
- *"No substitutes, pack size, UOM or pallet configuration changes will be
  accepted without prior authorisation."* Pack-size correction is exactly what
  `item_packing_config`'s versioning exists to make safe, and here it is
  contractual rather than merely prudent.

**Recommendation: raise price as its own question rather than smuggling it into
the order-response work.** It touches D40's boundary, the unadopted
`unit_cost_minor`, and whatever a rate card becomes, and answering it inside an
EDI decision would settle a commercial question by implication. The immediate,
cheap half is that `order_line` needs a price and a currency before any grocery
order response is built, and that history before the column exists is not
recoverable.

---

## 4. The GTIN issuer, and the assumption that a namespace is a sequence

`item_barcode` holds GTINs and nothing allocates one. `number_range.key` is
`'sscc' | 'internal_lp'`. The obvious fix is a third value and a fact table
shaped like `sscc_allocation`, and it is nearly right.

**What breaks it: not every identifier namespace is a sequence.** D29's
`number_range` is `next_value` plus `block_size` claimed under `FOR UPDATE`, which
assumes a computable serial space. Australian GTIN namespaces are not all that
shape:

| Namespace | How obtained | Shape |
|---|---|---|
| GTIN-13 from our company prefix | Derived from the GCP | Sequence |
| Higher packaging levels | Indicator digits 1 to 8 on the same item reference | Enumeration, not a sequence |
| GTIN-8 | Individually allocated by GS1 Australia | **Pool of granted numbers** |
| Individual Barcode Number tier | One to ten GTINs, nine more on application | **Pool of granted numbers** |
| Variable measure (VMN-13) | Restricted-circulation prefixes, per member-organisation rules | Separate namespace |
| UPC company prefix | A separate entitlement, applied for | Second sequence |

A pool is not a range with a next value. It is a set of numbers granted to us,
each either issued or not. Modelling it as a sequence means inventing a
`next_value` over numbers we were handed, and the first tenant on the Individual
Barcode Number tier breaks it.

The cheap answer is that granted numbers are **pre-created allocation rows marked
unissued**, so the fact table is the pool and `number_range` covers only the
namespaces that really are sequences. That keeps D29's row-locked claim path for
SSCCs untouched.

**Indicator digits are the part that stops this looking worse than it is.** A
ten-digit prefix yields 100 item references, and because indicators 1 to 8
produce higher packaging levels from the same reference, that is 100 base products
with up to eight levels each rather than 100 barcodes. D34 already draws the
distinction; the issuer has to respect it, because allocating a fresh item
reference for a carton would burn the scarce namespace eight times faster.

**The obligation is permanent, which S34 cannot satisfy.** Recorded already in
D34's correction: GTIN non-reuse has no expiry, so the `retention_floor` it needs
has no `minimum_age` that any finite archive window satisfies. D31's floor table
assumes an interval. That is a small but real amendment, and it is the second
time the retention-floor class has needed widening.

**The issuer is the brand owner, which may not be the tenant.** `sscc_allocation.
issuer_party_id` is already the legal entity holding the prefix rather than the
tenant, and the same applies here. For a 3PL holding a client's stock the brand
owner is the client, which is question 65 arriving from a new direction rather
than a new problem.

---

## 5. The National Product Catalogue: refuse it in writing

**Recommendation: out of scope, stated as an omission with the reason, with the
logistics attribute export in scope.**

What is already right is the expensive part. `observable`'s
`(item_id, packaging_level, item_packing_config_id)` triple is the per-level
subject NPC needs, versioned so a corrected case pack cannot rewrite the
dimensions of cartons shipped last year. Gross, net and tare are three metrics
because GS1 settled it. Ti and Hi are `item_packing_config`. **The logistics half
of an NPC publication is derivable from the model today**, and that is worth
saying plainly, because it is what makes the refusal a scope decision rather than
a capability gap.

What makes it the wrong subsystem to own:

- **It carries price.** The dictionary is the *Item and Price* Data Dictionary,
  and price is the gap section 3 just opened. Publishing a price we do not hold is
  not a near miss.
- **A published value is not a fold.** NPC needs the value we told the retailer,
  and the history of what we told them and when. That is a projection with a named
  maintainer under D35 plus a publication log — two mechanisms serving an external
  catalogue rather than the warehouse.
- **The attribute surface is not warehouse facts.** GPC classification, marketing
  copy, allergens, imagery. Under D26 those land in the extension mechanism, and
  D36 caps a scheme at 60 fields with `CHECK (ordinal BETWEEN 1 AND 60)`. A food
  item's target data set exceeds that, so it would take several schemes. A
  warehouse model growing several extension schemes to hold marketing copy is the
  shape of a mistake, and D36's ceiling exists precisely to make that visible
  rather than gradual.

*(The exact attribute count could not be established from public sources. The
recommendation does not depend on a threshold: price and publication versioning
decide it.)*

**The precedent is D40.** Produce the lines and what they were computed from, and
let the system whose job it is do the rendering. Here: hold and export the
logistics attributes we observe, and let a product-information system publish.

---

## Structural or additive

| Question | Answer | Cost |
|---|---|---|
| Acknowledgement lifecycle | Two layers, two existing homes | One self-FK on `party_message`; one assertion kind and body |
| D5 versus "do not despatch until acknowledged" | Finding, not lock | Folds into question 126, widened |
| Retailer PO as assertion | Both, mirroring `inbound_shipment` | One assertion kind and body; one successor FK on `order` |
| GTIN issuer | Additive, with a shape correction | Pool arm; a floor with no expiry |
| NPC publishing | Out of scope, stated | A paragraph in the non-goals |
| **Price on the order line** | **Not asked, and the real gap** | **A column pair, before any history exists** |

Nothing here requires rewriting a table that has history. The price column is the
only item with a deadline attached, and the deadline is the first grocery order
rather than the first migration.

---

## Proposed for the registers

Unnumbered deliberately.

**Questions to raise.**

- **Does `order_line` carry a price, and where does the boundary with D40 sit?**
  The immediate half is cheap and the commercial half is not. Trigger: before the
  first customer order arrives by EDI, and before `order_line` has history.
- **What links a cancelled order to the one raised in its place?** Metcash's
  stated amendment channel for an externally-authoritative order. Trigger: the
  first EDI customer.

**Questions to widen.**

- **126** from message-cardinality rules to counterparty message rules generally,
  so acknowledgement-required and structure-required resolve the same way.

**Questions to answer, pending a decision.**

- The acknowledgement lifecycle: two layers, `party_message` for transport and an
  inbound assertion for application disposition.
- The retailer purchase order: an assertion whose subject is our `order`, the
  `inbound_shipment` pattern applied one direction over.
- The National Product Catalogue: out of scope, with the logistics export in and
  the reason recorded.

**Invariants to propose.**

- Every outbound `party_message` on a channel whose party profile requires
  acknowledgement has one, or a finding naming it. Vacuity-marked, and it needs
  the companion assertion that outbound messages exist at all.
- No `assertion` of an application-disposition kind names a subject assertion of
  the opposite `direction`. Their disposition is always of our claim.

**Amendments to propose.**

- **D31** — `retention_floor.minimum_age` admits a floor with no expiry. GTIN
  non-reuse is permanent, and an interval cannot say so.
- **D42** — the amendment fold is for orders we author; an externally-authoritative
  order is amended through its own system, and the local record is a succession
  link rather than an amendment.

---

## Sources

- [Metcash Purchase Order Response v2.0](https://mars-metcdn-com.global.ssl.fastly.net/content/uploads/sites/109/2019/06/27113041/Metcash-ORDRSP-v2.0.pdf)
- [Metcash Despatch Advice Message Implementation Guideline v1.10](https://mars-metcdn-com.global.ssl.fastly.net/content/uploads/sites/101/2017/04/18110045/Metcash-DESADV-v1.10.pdf)
- [Oracle, EDI acknowledgement documents: 997/CONTRL and 824/APERAK](https://docs.oracle.com/cd/E16582_01/doc.91/e15102/edi_acknowledgements.htm)
- [Seeburger, EDIFACT CONTRL technical acknowledgement](https://www.seeburger.com/resources/good-to-know/edifact-contrl-technical-acknowledgement-message)
- [GS1, EANCOM ORDRSP syntax 3](https://www.gs1.org/standards/edi-xml-gs1-eancom/eancom-ordrsp-s3/syntax-3)
- [GS1 Australia, 2025-2026 Subscription Benefits and Entitlements](https://assets.ctfassets.net/9uypwcnuzbqi/RLvQrTe2ljBIroNVhnzqw/d8640ae3bd702702d25bb7e4b23bb164/GS1au-form-Membership_benefits_entitlement-2025.pdf)
- [GS1 Australia, GTIN-8 how-to guide](https://assets.ctfassets.net/9uypwcnuzbqi/3FPTbuA03cCX3YnKHdL5vZ/d7c70dfb996a0a109fc94e9ee39c03ef/GS1au-how-to-guide-GTIN-8.pdf)
- [GS1 Australia, variable measure retail fact sheet](https://assets.ctfassets.net/9uypwcnuzbqi/PlyyuvpxJtvubTH7KoIJO/96be9181fda515d6d39b525cb0d5d3e2/GS1au-fact-sheet-variable-measure-retail.pdf)
- [GS1 Australia, National Product Catalogue FAQs](https://www.gs1au.org/resources/faqs/npc)
