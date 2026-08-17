# GS1 grocery surface: research and verification

Question 123. Verify that the GS1 grocery surface is reachable without structural
change: GTIN allocation, SSCC issuance and reuse, ITF-14 on master cartons,
GS1-128 pallet labels, EDI despatch advice, National Product Catalogue. Carries
the live half of question 69, which D39 to D42 answered as roadmap rather than
first version.

> **Questions and invariants raised here are not owned here.** The registers are
> [open-questions.md](./open-questions.md) and [invariants.md](./invariants.md),
> and they carry the canonical numbering. The proposals at the end of this
> document are proposals until a decision migrates them.

Generated 2026-08-03. The direction under test is **outbound**: the tenant is a
supplier to Woolworths, Coles or Metcash, not a receiver from them. Inbound was
covered by [inbound-analysis.md](./inbound-analysis.md) and settled by D24, D29,
D34 and D37.

**Provenance.** Unlike the inbound pass, the load-bearing claims here are from
primary documents read in full rather than from summaries: GS1 Australia's
*2025-2026 Subscription Benefits and Entitlements* (effective 1 July 2025), GS1
Australia's *GS1 Logistic Label and SSCC* fact sheet (August 2022), *Coles Carton
Logistics Labelling Requirements v5.2*, and *Metcash Despatch Advice Message
Implementation Guideline v1.10* (EANCOM 2002, June 2016). Where a claim rests on
a search result rather than a document, it says so. Retailer specifications
change without notice and none of this should be treated as contractual.

---

## Verdict

**Reachable. Nothing in the six surfaces requires rewriting a table that has
history.** The three gaps are additive, and the model is already right about the
two things that would have been expensive to retrofit: identity is separated from
symbology (D34), and per-packaging-level attributes have a typed subject (D23's
`observable`).

The finding worth acting on is not a gap. **One of D29's three arguments is
false in Australia, and it is the one D29 tells the reader to rely on.** That is
what makes 123 worth doing now: it is falsifiable today and it gets more
expensive to falsify once anyone has quoted the number.

Three things are actually missing:

1. **No acknowledgement lifecycle for an outbound assertion.** Metcash's rule is
   that a supplier who receives no functional acknowledgement *should not despatch
   against that ASN*, and rejection arrives at both header and line level with
   reasons. The model has our stance on their claim and our parse status on their
   message. It has nothing for their disposition of ours.
2. **No issuer for a GTIN.** `item_barcode` holds GTINs; nothing allocates one.
   This is the position `package.sscc` was in before D29, one table later.
3. **The National Product Catalogue's outbound direction is unmodelled**, and the
   recommendation is that it stays that way and is written down as an omission.

Everything else is code, rows, or a policy kind that already exists.

---

## One correction, cheap today

D29 refuses per-carton minting at receipt on three arguments. The second is
arithmetic:

> An SSCC is 18 digits: extension digit + company prefix + serial reference +
> check digit, with prefix and serial sharing 16. A 12-digit prefix — what a small
> Australian company is issued — leaves **four** serial digits: 100,000 total.
> GS1's one-year non-reallocation rule turns that into a sustained ceiling of
> **274 SSCCs/day, forever**.

**GS1 Australia does not issue 12-digit company prefixes.** Its own fact sheet
tabulates SSCC construction for seven-, eight-, nine- and ten-digit prefixes, and
its membership FAQ sets the allocation by turnover: eight digits under $50M,
seven digits over. The smallest prefix any full member holds is ten digits, which
is the *100 GTINs* tier of the entitlements table.

Recomputing on the real allocations:

| GS1 AU prefix | Serial digits | Per extension digit | Across ten digits | Sustained ceiling under the 12-month rule |
|---|---|---|---|---|
| 10 (smallest full member) | 6 | 1,000,000 | 10,000,000 | ~27,400/day |
| 8 (standard, under $50M) | 8 | 100,000,000 | 1,000,000,000 | ~2,700,000/day |
| 7 (over $50M) | 9 | 1,000,000,000 | 10,000,000,000 | ~27,000,000/day |

D29's reference site is 1,200 cartons/day, or 438,000 a year. Against the
smallest full-member namespace that is **4.4% a year**, not the four-month
exhaustion D29 describes. The figure is wrong by two orders of magnitude in the
worst case and by four in the normal one.

**The decision survives; the argument does not.** The other two arguments are
untouched and both are stronger:

- Per-carton identification at receipt is **physically unobservable**. GS1
  General Specifications 4.4.2 obscures the child labels on a wrapped pallet, so
  minting per carton is minting packages nobody identified.
- The expensive half, unchanged: a `package` minted from `asserted_unit.sscc`
  puts a counterparty's message into a component of the `stock` key, which D21
  rule 3 forbids and which J19 passed anyway.

This matters because D29 says, of the arithmetic specifically, *"That kills
per-carton on the standard, before labour and long before the database notices.
Do not argue this on storage grounds."* A reader who checks the load-bearing
claim finds it false and has no way to know the conclusion is still right.

**There is a real Australian failure mode, and it is worse than a tight
ceiling.** GS1 Australia's entitlements table marks *"Allocation & use of GS1
Company Prefix (eg: so you can create SSCC)"* with a cross for the **Individual
Barcode Number** subscription — turnover under $1M, one to ten GTINs, up to nine
more on application. That member has no prefix, and therefore **cannot form an
SSCC at all**, while Woolworths, Coles and Metcash all mandate SSCC pallet
labels. Not a small serial budget: no capacity.

D29 already handles this correctly. `number_range` exhaustion *"raises a finding
and FALLS BACK to internal LPs. It never blocks a print. Same code path as
'tenant has no prefix' — one fallback, exercised daily."* The mechanism is right
and only its justification needs restating. The honest version is:

> Carton grain is refused because the cartons on a wrapped pallet cannot be
> observed and because minting from an assertion launders a counterparty's claim
> into the `stock` key. Serial capacity is not the constraint in Australia. The
> constraint that does bind is categorical: a tenant on an Individual Barcode
> Number subscription has no company prefix and can issue no SSCC at any grain,
> which is why the internal-LP fallback is a daily path rather than an edge case.

---

## Surface by surface

### 1. GTIN allocation

**What binds.** GTIN allocation is governed by the GTIN Management Standard.
Two rules decide when a new GTIN is required and both are mechanical: any change
to the legally-declared net content requires a new GTIN with **no threshold**,
and a change of **over 20% to any single axis** of a physical dimension or to
gross weight requires one, with sub-20% changes at the brand owner's discretion.
The 20% test is per axis, not on volume, and GS1 names cumulative sub-threshold
changes as an unacceptable avoidance practice.

**Reuse is gone.** The January 2019 revision eliminated GTIN reuse in every
sector: a GTIN allocated to a trade item is not reallocated to another. The
surviving exception is narrow, and it is for an item that was allocated a GTIN
but never produced, which may be reused twelve months after deletion from the
catalogue. (Search-sourced; verify against the current standard before anything
depends on it.)

**What the model holds.** `item_barcode` (D34) with `scheme = 'gtin'`, GTINs
normalised to 14 on write, one exclusion constraint over `effective daterange`.
`party.gs1_company_prefix` and `gs1_prefix_length` exist from D29.

**Gap: a GTIN has no issuer.** `number_range.key` is `'sscc' | 'internal_lp'`.
Allocating an item reference out of our own company prefix is the same act as
allocating a serial reference — a claimed slot in a finite namespace with a
non-reuse obligation attached — and today nothing performs it. The fix is one
enum value and one fact table shaped like `sscc_allocation`, which is additive.
Note the obligation is *stronger* than the SSCC's: non-reuse is permanent, so
the retention floor has no expiry and S34's history-depth companion cannot be
satisfied by any finite archive window. That is worth stating explicitly rather
than discovering.

**Stale claim in D34.** D34 justifies the range shape twice on a reuse waiting
period:

> GTIN reuse after GS1's waiting period is a later non-overlapping range, which
> the same constraint permits while continuing to forbid the overlap.

and in its rejects, *"`active boolean`: it cannot express GS1's reuse waiting
period"*. There has been no general GTIN reuse waiting period since 1 January
2019. **The constraint shape is still right, for reasons D34 also gives**: a
`supplier_reference` genuinely rebinds when a supplier reuses its own code, a
tenant-scoped row supersedes a shared one, an internal code is retired, and a
scan recorded in March must stay explainable in September. Only the GTIN example
is wrong, and it is the one the text leads with.

**Capacity is the real constraint, and it is GTINs rather than SSCCs.** A
ten-digit prefix yields 100 item references. Because indicator digits 1 to 8
produce higher packaging levels from the same item reference, that is 100 base
products with up to eight levels each rather than 100 barcodes — a distinction
D34 already draws and which stops this looking worse than it is. Still, a small
grocery supplier with 100 references is one range extension away from a fee, and
`number_range.exhausted_at` raising a finding is the right shape for it.

### 2. SSCC issuance and reuse

**What binds.** Eighteen digits under AI (00): extension digit, company prefix,
serial reference, check digit. The extension digit is assigned by the company
constructing the SSCC and exists to increase serial capacity. The prefix used
*"should belong to the brand owner or physical builder of the logistic unit"*,
which is the same allocation rule D29 built trigger 2 on. An SSCC *"must not be
reassigned within one year of the shipment date from the SSCC assignor to a
trading partner"*, and industry requirements may extend that.

**It is enforced, not merely specified.** Metcash validates SSCCs on an inbound
ASN against SSCCs already receipted into stock, and an SSCC used by that supplier
in the past twelve months *"will trigger an exception for review"* at line level.
The GS1 hygiene rule and the commercial consequence are the same rule.

**What the model holds.** D29 is complete here: `number_range` scoped to the
issuing legal entity with an explicit row per extension digit and no automatic
rollover, `sscc_allocation` as an append-only fact with `sscc CHAR(18)`
generated, J35 asserting no serial is reissued inside the window, S33 gating
issuance on an explicit range row, and D31's `retention_floor` carrying the
twelve-month identifier-reuse floor with GS1 named as the authority. J35 is also
the register's exemplar of the vacuity problem, with S34 as its companion.

**No gap.** This surface is reachable as designed. The only correction is the
arithmetic above, which changes no schema.

### 3. ITF-14 on master cartons

**What binds** (Coles v5.2, read in full). GTIN-14 is mandatory on each master
carton in both human-readable and barcode format, illustrated in ITF-14
symbology, at 122.43mm wide excluding quiet zones and 32mm minimum height
excluding the bearer bar at 100% magnification. Also mandatory on every carton:
manufacturer name, address and country of origin; SKU description; pack size;
inners per outer where applicable; and handling information covering storage,
warnings, carton orientation and pallet configuration. Expiry is mandatory on
each carton for date-sensitive SKUs, catch weight is mandatory on each carton for
catch-weight items, and batch number is conditional on being used. SSCC is
mandatory on the pallet label for B2B-compliant suppliers.

**D34 called this correctly in advance.** ITF-14 is a symbology and not an
identity, so a carton GTIN is one `item_barcode` row at the carton unit and the
same GTIN read from a GS1-128 beside it resolves through the same row. Nothing
structural.

**Printing is not scanning, and the parts are already facts.** Country of origin
lands on `lot` rather than `item` because it varies lot to lot for food. Net
content and catch weight are `observation` rows against metrics that exist
because D23 split gross, net and tare on GS1's authority. Pack size and inners
per outer are `item_packing_config`. What does not exist is a label composition
surface, and it should stay code plus a template. A `carton_label` table would be
a projection with no fact behind it.

**The sharp one: pallet homogeneity is a per-customer rule.** Coles requires that
*"pallets must be homogenous where each carton on the pallet has the same
expiration date"*. Metcash explicitly supports the opposite, having clarified in
v1.8 that a mixed batch (multi-code) pallet requires `DTM+36/361` on each `LIN`
segment. Two customers, contradictory constraints on the same physical act.

That is D22's lattice with the customer party as a scope, not a rule in code. If
it were hardcoded, one customer's rule would silently govern the other's pallets.
And per D5 the model does not block the build: a pallet violating the in-force
rule raises a finding at pack time, which is the answer D8 gives everywhere else.

### 4. GS1-128 pallet labels

**What binds.** The SSCC is *"the one mandatory requirement"* on a logistic
label. GS1-128 *"must be used for all information on the GS1 Logistic Label"*,
with concatenation used to minimise symbol count except for the SSCC itself,
which stands alone because of its larger recommended magnification. A GS1
DataMatrix or QR *"MAY be included in addition to"* the GS1-128 symbol, which
confirms the inbound pass's reading of Sunrise 2027: 2D is additive on logistic
labels and a 2D-only receiving path is out of scope.

**What the model holds.** Parsing is settled — the vendor `gs1-syntax-engine`
with the GS1 Syntax Dictionary as read-only seed data, unrecognised application
identifiers stored opaquely and never rejected. Emission is the mirror image and
is simply unbuilt. Additive.

**One emission rule needs a declared source.** D34 normalises GTINs to 14 on
write. Metcash requires that the GTIN on our ASN match the one on their purchase
order *"e.g. including leading zero if it is present on EDI PO"*. The two are
mechanically reconcilable, since a GTIN-13 padded to 14 with indicator 0 is the
same trade item and the padding is reversible. What is not automatic is knowing
which form a given counterparty expects. The verbatim bytes are retained on
`party_message.payload`, so the answer is recoverable but not queryable, and
re-parsing a stored EDIFACT message to decide how to render an outbound field is
not a mechanism anyone should build. The cheap correct answer is a capability
column on `party_profile`, which is D20's shape and D20's argument.

### 5. EDI despatch advice

**What binds** (Metcash v1.10, read in full; the Australian Retail Industry
EANCOM 2002 guideline behind it is GS1 Australia's). Cardinality first, because
it is narrower than the standard and narrower than the inbound pass recorded:

- *"An ASN can only relate to a single PO. This means that goods from different
  orders cannot be mixed within a logistics unit. However, multiple ASNs from
  separate purchase orders can be consolidated within a delivery."*
- *"A PO can be split over multiple deliveries... Each PO delivery must have a
  separate ASN. An ASN cannot be split over multiple deliveries or trucks."*
  Separate invoices must be issued against each ASN of a split shipment.
- Ti/Hi (`MEA+PD+ULY`, `MEA+PD+LAY`) are required for Metcash.
- *"An Accepted ASN number cannot be re-used by a supplier for a 24 month
  period."*

**Our grouping absorbs this without bending.** D15's three groupings already
separate the commitment from the shipment: one ASN per `fulfilment`, several
fulfilments consolidated into one `consignment`. The rule that goods from
different orders may not be mixed within a logistic unit is a predicate over
`package_content` → `fulfilment_line` → `fulfilment` → `order`, so it is
checkable and therefore a finding rather than a schema change.

**Gap: nothing records the counterparty's disposition of our assertion.**
Metcash returns a CONTRL functional acknowledgement within three hours, and *"if
the supplier does not receive a FA from Metcash after sending an ASN, the
supplier should not despatch stock against that ASN."* Beyond that, rejection is
structured and two-tiered. The whole ASN is rejected if it is structurally
invalid, relates to more than one purchase order, spans more than one delivery,
duplicates an accepted message, reuses an ASN number inside 24 months, names an
invalid or closed PO, omits the shipment reference, names the wrong delivery
location, or has a delivery date later than the PO requested. A line is excepted
if the GTIN is not on the PO, the SSCC was used in the past twelve months, the
quantity exceeds the PO, Ti/Hi or unit of measure differ from the PO, or shelf
life is outside agreed limits.

The model has `assertion_stance` for our position on their claim and
`party_message.parse_status` for our parsing of their message. Neither is theirs
of ours. D21 took the symmetric version deliberately — *"our own outbound
despatch advice is as unrevisable as a supplier's inbound one... costs one
`direction` column"* — and the direction column is there, but the return path
is not. One new fact against the outbound assertion, carrying acknowledgement,
disposition and per-line rejection reasons, is the whole of it. Additive, and
missing rather than merely unbuilt.

**It also raises a D5 question that should be decided rather than inherited.**
"Do not despatch until acknowledged" is a counterparty-imposed block on our own
physical act. D5 forbids the ledger blocking on coordination; it does not forbid
recording that a customer's rule was breached. The consistent answer is a finding
at despatch rather than a lock, but D5 was written about our own ledger and this
is the first case where the coordination requirement is contractual.

**And a second retention floor with a named authority.** The 24-month ASN-number
non-reuse rule is the same vacuity class as J35: an existence predicate with no
projection to compare against, which starts passing silently once history is
truncated. It is a `retention_floor` row with Metcash as the authority, and it is
the second instance of the class question 114 named.

**One thing the inbound pass got right and is worth confirming.** The despatch
advice body is shaped for inbound: `despatch_advice.inbound_shipment_id` is its
subject. An outbound one has a `fulfilment` as its subject. Under D21 the typed
body per kind is joined by a composite FK on `(assertion_id, kind)`, so this is
a nullable sibling column with a CHECK keyed on direction, not a second table.

### 6. National Product Catalogue

**What it is.** GS1 Australia's GDSN-certified data pool. Publication is
organised into Trade Item Groups that must align to the supplier IDs each
retailer maintains, attributes are mandatory or conditionally mandatory (the
documented example being `SellingUnitOfMeasure`, conditional on consumer units),
and the target data set was last updated February 2026. It is a paid
subscription in GS1 Australia's entitlements table, separate from barcode
membership. The inbound pass's reading — GTIN barcodes and SSCC pallet labels
mandatory, NPC and EDI in the preferred tier — is consistent with everything
found here and is not contradicted.

**The inbound direction is already refused, correctly.** D34: an NPC feed, a
supplier price file or a wholesaler's catalogue export is an assertion, never a
write to reference data, because *"a feed writes here directly [and] a supplier
silently rewrites what a scan means"*. That refusal stands and this document adds
nothing to it.

**The outbound direction is what 123 asks about, and it is not covered.**
Publishing means being the source: a hierarchy of trade items, each packaging
level its own GTIN carrying its own dimensions, gross weight, net content and
classification, grouped per retailer, with prices.

What is already right is the part that would have been expensive:
`observable`'s `(item_id, packaging_level, item_packing_config_id)` triple is
exactly the per-level subject NPC needs, versioned so that a corrected case pack
cannot silently rewrite the dimensions of cartons shipped last year. Gross, net
and tare are three metrics because GS1 settled it. Canonical units are exact
rationals. Ti and Hi are `item_packing_config`. **The logistics half of an NPC
publication is derivable from the model today.**

What is not there: a published value per attribute is a projection with a named
maintainer under D35, and the version history of what we told a retailer in March
is a second one; price is not in the model at all; the retailer-scoped Trade Item
Group is a party-scoped grouping; and the marketing and classification attributes
are not warehouse facts. Under D26 they would land in the extension mechanism,
and D36's ceiling then becomes the arithmetic that matters: 60 fields per scheme
with `CHECK (ordinal BETWEEN 1 AND 60)`, against an NPC target data set for a
food item that is larger than that. It would take several schemes, and a
warehouse model growing several schemes to hold marketing copy is the shape of a
mistake.

**Recommendation: declare it out of scope with the reason, rather than approach
it.** The obligation this system should accept is to hold and export the
logistics attributes it observes — dimensions per level, Ti/Hi, gross and net
weight, catch weight, country of origin. Product content belongs to whatever
holds product content. This is the same move D40 made for the invoice: produce
the lines and what they were computed from, and let a system whose job it is do
the rendering. Written as an omission with a reason, per the standing objection
to omissions inherited from dead premises.

---

## Structural or additive

| Surface | Verdict | Cost |
|---|---|---|
| GTIN allocation | Additive | One `number_range` key, one allocation fact, one permanent retention floor |
| SSCC issuance and reuse | Reachable as designed | Nothing. Correct D29's arithmetic |
| ITF-14 on master cartons | Additive | Label composition in code; pallet homogeneity as a D22 policy kind |
| GS1-128 pallet labels | Additive | Emission code; one `party_profile` capability column for GTIN rendering |
| EDI despatch advice | Additive | One acknowledgement fact; one nullable subject column on the body; one retention floor |
| National Product Catalogue | Out of scope, stated | A paragraph in the non-goals |

Nothing on this list touches a table that has history. `package_content` still
has none, which was question 69's stated good case and remains true.

---

## Proposed for the registers

**Deliberately unnumbered.** Numbers are allocated in the commit that adopts, not
here. The first draft of this section numbered these 125 to 128; D43 has since
taken 125 and 126 for other questions, which is exactly why a research document
must not allocate.

**Questions.**

- **Where does a counterparty's disposition of our outbound assertion live?**
  Functional acknowledgement, header rejection, per-line exception with reasons.
  Trigger: the first outbound despatch advice.
- **Does D5's non-blocking rule survive a contractual "do not despatch until
  acknowledged"?** Recommendation is a finding at despatch, not a lock, but D5
  was written about our own ledger. Related to question 62, which asks the same
  shape about D8. Trigger: the first EDI customer.
- **Is a retailer-issued purchase order an assertion as well as an order?**
  D39 settled provenance for orders arriving through our own channels. A
  retailer's PO is a document a counterparty holds a copy of and will quote back
  in a chargeback, which is D21's defining property, and `assertion.kind` has no
  value for it. Trigger: the first EDI customer.
- **Is publishing to the National Product Catalogue in scope?**
  Recommendation is no, with the logistics attribute export in scope and the
  reason recorded. Trigger: decide before anyone builds an NPC adapter.

**Invariants.**

- Every GTIN issued from our own company prefix has an allocation row, and no
  item reference is ever reallocated. The reuse guard has no expiry, so S34's
  history-depth companion cannot be satisfied by a finite window and the floor
  must say so.
- No outbound assertion of kind `despatch_advice` has a `fulfilment` whose
  packages resolve to more than one `order`. The vacuity marker applies.

**Corrections.** *(Both adopted 2026-08-04, into D29 and D34 directly.)*

- D29's serial-capacity argument is retracted and replaced by the categorical
  one: an Individual Barcode Number subscription has no company prefix and can
  issue no SSCC at any grain.
- D34's two references to a GTIN reuse waiting period are restated. The
  constraint shape is unchanged and the other justifications for it stand.
- `architecture.md` and `proposal.md` counts are re-derived from the registers
  rather than adjusted, per the standing rule.

---

## Sources

- [GS1 Australia, 2025-2026 Subscription Benefits and Entitlements](https://assets.ctfassets.net/9uypwcnuzbqi/RLvQrTe2ljBIroNVhnzqw/d8640ae3bd702702d25bb7e4b23bb164/GS1au-form-Membership_benefits_entitlement-2025.pdf)
- [GS1 Australia, GS1 Logistic Label and Serial Shipping Container Code fact sheet](https://assets.ctfassets.net/9uypwcnuzbqi/7hnreiHAB5x36TMFbNLxUO/848a78f1ece644af54ed28689ae4e76b/GS1au-fact-sheet-suppliers-SSCC-and-logistics-label.pdf)
- [GS1 Australia, membership FAQs](https://www.gs1au.org/resources/faqs/membership)
- [GS1 Australia, National Product Catalogue](https://www.gs1au.org/services/data-and-content/national-product-catalogue)
- [GS1 Australia, NPC user guide cookbook](https://assets.ctfassets.net/9uypwcnuzbqi/5fMqxY0gu08HR6K1Qz5dbw/46824a9461af45927c2e4c128913c90e/GS1au-cookbook-npc-complete.pdf)
- [Coles Carton Logistics Labelling Requirements v5.2](https://www.supplierportal.coles.com.au/wps/wcm/connect/d4b9f300477f5e02bf3affe161441723/Coles+Logistics+Carton+Labelling+requirements+v5+2.pdf?MOD=AJPERES)
- [Metcash Despatch Advice Message Implementation Guideline v1.10](https://mars-metcdn-com.global.ssl.fastly.net/content/uploads/sites/101/2017/04/18110045/Metcash-DESADV-v1.10.pdf)
- [GS1, SSCC identification key](https://www.gs1.org/standards/id-keys/sscc)
- [GS1, GTIN Management Standard](https://ref.gs1.org/standards/gtin-management/)
- [GS1, dimensional or gross weight change rule](https://www.gs1.org/1/gtinrules/en/rule/265/dimensional-or-gross-weight-change)
- [GS1, declared net content rule](https://www.gs1.org/1/gtinrules/en/rule/266/)
- [GS1 Global Office, can a GTIN be reused](https://support.gs1.org/support/solutions/articles/43000734390-can-a-gtin-be-reused-)
- [GS1 Logistic Label Guideline](https://www.gs1.org/standards/gs1-logistic-label-guideline/current-standard)
