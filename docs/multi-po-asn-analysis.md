# Multi-PO ASNs and the ORDER hierarchy level

Question 68. In scope, or an explicit omission? Filed on the reading that
Australian cardinality fits `goods_receipt` unchanged and that US and 3PL traffic,
with its X12 856 ORDER hierarchy level, does not. Examined here for a design that
serves both without a per-market branch.

> **Questions and invariants raised here are not owned here.** The registers are
> [open-questions.md](./open-questions.md) and [invariants.md](./invariants.md).
> The proposals at the end are deliberately unnumbered.

Generated 2026-08-04. Companion to [inbound-analysis.md](./inbound-analysis.md),
which raised 68, and to [gs1-grocery-analysis.md](./gs1-grocery-analysis.md),
which reached one of the same conclusions from the outbound side.

**Provenance.** Australian requirements are from the Metcash DESADV
implementation guideline v1.10, read in full for the GS1 pass. International
claims are from X12 856 implementation guides and EDI vendor documentation, which
is weaker evidence than a retailer specification read end to end. Two claims are
marked where they rest on a single source.

---

## Verdict

**The question's premise is half-stale, and what survives is not about
cardinality.**

*Corrected on adoption: 68 was **already settled** in the register, by D24
(supply side), alongside 109. The first draft of this document read 68 out of the
inbound analysis's tail rather than out of the register and proposed settling it,
which is the failure the register exists to prevent. The technical findings below
stand; they refine a settled question rather than closing an open one, and D43
adopts them on that basis.*

D24 (supply side) settled questions 68 and 109 on 2026-08-02: *"Multi-PO ASNs are
**in scope and supported**. The scalar FK is correct rather than a limitation."*
An ORDER-level split becomes content lines each naming one purchase order line,
which is what `asserted_unit_content.resolved_purchase_order_line_id` already
holds.

So the model already carries multi-PO advice. What 68 still holds is a
**provenance** problem rather than a capacity one, and it is genuinely
international-only:

> **X12 states the purchase order at a node above the physical tree. EDIFACT
> states it on the line. Our model has a per-line raw column.**

Populating that column from an 856 means copying an ancestor's value down into a
`raw_*` field, where D21's column-class rule says raw means as exchanged. The
value is right and the provenance claim is false, which is the quiet kind of
wrong this model keeps finding in itself.

Second residue, smaller: `goods_receipt`'s grain for a multi-PO delivery is
**undecided rather than wrong**. Two representations are legal today and nothing
chooses between them.

Neither needs a migration. The recommended answer to the first costs no schema
change at all.

---

## What 68 assumed, and what has changed under it

The question was filed against this sentence:

> Cardinality is *narrower* than the standard: Metcash and Coles both forbid an
> ASN spanning more than one PO, more than one destination or more than one truck.
> That fits `goods_receipt`'s singular demand FK comfortably. It is US/3PL 856
> traffic, with its ORDER hierarchy level, that would break it.

Three of the four load-bearing claims have moved since.

**"Singular demand FK" is no longer the state of the model.** D16 wrote
`CHECK (num_nonnulls(purchase_order_id, transfer_order_id) = 1)` and D23's
discriminated-union rule relaxed it, with S3 now asserting that every demand CHECK
on a grouping table is `<= 1` and naming this as *"D16-repeats-D10"*. A receipt
may already name no purchase order.

**The line already knows its own demand.** `goods_receipt_line` reaches its
origin through `expected_supply`, whose four arms are the purchase order line, the
transfer order line, the asserted unit content and the return authorisation line.
J26 folds `quantity_received` that way. The header FK is a convenience for the
common case, not the source of truth, so a multi-PO receipt loses nothing by
leaving it null.

**Multi-PO advice was settled by D24 (supply side).** Recorded there as *"settling
question 109, which was recorded as an omission on a misreading."* The register
migrated 68 in the same pass and marks it settled by the same decision. What was
not updated is the tail of `inbound-analysis.md` and `supply-side-design.md`,
which still describe 68 as undecided — expected, since research documents do not
own status, and the reason this document had to be corrected on adoption.

What has *not* changed is the fourth claim, and it is the one worth the pass.

---

## What the standards actually require

### The ORDER level sits above the physical levels in every defined structure

The 856's hierarchy is built from `HL` segments with a level code and a parent
pointer, and the structures in production use are named by their level sequences:

| Structure | Levels | Used for |
|---|---|---|
| SOI | Shipment, Order, Item | Standard pack, no carton detail |
| SOTI | Shipment, Order, Tare, Item | Single-SKU pallets |
| SOPI | Shipment, Order, Pack, Item | Pick and pack, carton-labelled |
| SOTPI | Shipment, Order, Tare, Pack, Item | Pallets of labelled cartons |

In all four, **O sits directly under S and above every physical level**. The
consequence is not usually stated and it decides this question:

> **A logistic unit belongs to exactly one purchase order.** There is no
> well-formed 856 in which a pallet or carton spans two orders, because the tare
> and pack nodes are descendants of a single order node.

Metcash states the identical rule as prose: *"An ASN can only relate to a single
PO. This means that goods from different orders cannot be mixed within a logistics
unit."* The second sentence is the universal rule; the first is Australia's extra.

So Australia and the United States **agree on the pallet** and differ only on the
message. The gap 68 describes is one level narrower than it reads.

### The purchase order reference is at the order node, not on the item

In X12, `PRF` carries the purchase order number and it appears in the order-level
`HL` loop. The item level carries `LIN` and `SN1` — identifier, quantity, unit of
measure — and no purchase order. When a shipment covers several orders there is
one order-level loop with its own `PRF` per purchase order number, which is the
documented structure for mixed-PO shipments.

In EDIFACT DESADV the tree is nested `CPS` packaging sequences with `LIN` under
the innermost level, and `RFF+ON` is available **at line level**. The purchase
order reference can therefore be stated per line.

**That asymmetry is the whole of what is left of 68.** Our content line has
`raw_po_reference` and `raw_po_line_number`. EDIFACT fills them from the message.
X12 cannot: the string exists on an ancestor.

### What is not a requirement, despite the question implying it

**Multi-destination is not a real case.** The guidance is that an ASN pertains to
one destination and one vendor number and may contain multiple purchase orders.
*(Single-sourced, from a vendor implementation guide. Verify before relying on
it.)* Each destination gets its own ASN, so `inbound_shipment.site_id` staying
singular is correct rather than restrictive.

**Multi-vendor consolidation does not break either.** A consolidator's truck
carries several vendors' shipments, and each vendor's ASN is its own message.
`inbound_shipment.vehicle_arrival_id` already makes the arrival one-to-many over
shipments, so the truck is modelled and the ASN is modelled and they are already
different tables.

**Depth does not break.** SOTPI is pallet → carton → item once the order node is
set aside, which is D6's depth 2 exactly. SOPI has no pallet node at all, which is
the same shape as the flat-CSV case the hierarchy tables were already required to
accept without null-hostile constraints.

---

## The finding: an inherited raw value is not an exchanged one

D21 splits the assertion body's columns into two classes and hangs immutability
off the split: `raw_*` and every transcribed value are immutable and as exchanged,
`resolved_*` are our annotation. Rule 5 says an assertion is *"recorded in the
author's vocabulary"*.

Ingesting an 856 at content-line grain requires walking up from each item node to
its ancestor order node and writing that `PRF` value into every descendant's
`raw_po_reference`. Afterwards the column holds a string that **appears nowhere on
that line in the original message**.

The value is correct. The claim the column makes about itself is not. And it fails
quietly in the way this model has now caught four times: a check that inspects
`raw_*` to prove fidelity to the received bytes would pass, because the value it
finds is one that really was in the message, just not there.

### Three ways out

**A. The order node becomes a node in `asserted_unit`.** Recommended.

`asserted_unit` already has `level_code`, already permits unbounded nesting on the
stated grounds that it is a cold path, already has a nullable `sscc`, and already
collapses to D24's depth cap at receipt. An order node is a node with
`level_code = 'order'`, no SSCC, and children that are physical. The tree stored
is then the tree the author sent, and resolution to a purchase order line walks up
at ingestion.

Cost: zero schema change. The walk is cold-path by construction — D37 put the
assertion body off the receiving screen entirely, and this walk happens once per
message at ingestion, not once per scan at the dock.

The one thing it costs is a sentence. The inbound analysis established that
*"S-O-T-P-I are not five containers: S and O are documents"*, and D24 (supply
side) reasoned from it. Under A, S remains the assertion and **O becomes a node**.

This does not reopen question 109. Content lines still resolve to exactly one
purchase order line, `refines_expected_supply_id` stays scalar, and J8's partition
identity is untouched, because refinement is an ASN row refining a PO row and has
nothing to do with where the order reference was stated.

**B. Redefine the column and admit the inheritance.** `raw_po_reference` means
"as exchanged at or above this line", with a companion column naming the level it
came from. Cheap and honest, but it adds a provenance column whose only job is to
carry an apology, and it weakens a rule that is currently absolute.

**C. An `asserted_order` sibling table.** One row per order loop, content lines
pointing at it. Faithful, and a new table for something an existing tree already
models. Rejected on the same grounds D32 rejected a second identity table.

---

## The second residue: `goods_receipt`'s grain

Nothing is wrong here. Nothing is decided either, and drift is what happens by
default.

One truck, one ASN, three purchase orders has two legal representations today:

1. **Three receipts**, each naming its purchase order in the header, all against
   one `inbound_shipment`.
2. **One receipt**, header demand null, lines carrying their own demand through
   `expected_supply`.

Both satisfy S3. Both fold correctly under J26. Nothing chooses, so both will
appear, and every query that groups by receipt will be right for one and wrong for
the other.

**Recommendation: one `goods_receipt` per delivery per demand document.** The
856's own hierarchy is then carried by tables that already exist:

| 856 level | Ours |
|---|---|
| Shipment | `inbound_shipment`, and the truck is `vehicle_arrival` |
| Order | `goods_receipt` |
| Tare, Pack | `asserted_unit`, collapsing to `package` at receipt |
| Item | `asserted_unit_content`, becoming `goods_receipt_line` |

Australia is then the degenerate case with one receipt per delivery, by contract
rather than by schema, and the international case is the same structure with N
greater than one. No branch, no mode, no per-market column.

Two things should be adopted alongside it, both already recommended and neither
adopted:

- **`goods_receipt.inbound_shipment_id`.** D37 names the chain
  `goods_receipt → inbound_shipment → in_force_assertion → asserted_unit` as the
  cold path it is designing around, but the column appears in no adopted DDL
  block. Under the recommendation above it is what makes "the three receipts off
  one delivery" a query rather than an inference.
- **The header source set widened to mirror `expected_supply`'s four arms.**
  Purchase order, transfer order, return authorisation, and none. This was the
  inbound analysis's tier-0 item 3 and half of it landed via S3.

---

## The design position, stated once for both markets

The rule that falls out, and which is worth writing down because it has now been
reached twice from opposite directions:

> **Cardinality restrictions belong to the counterparty, not to the schema.**

Metcash forbidding an ASN that spans two purchase orders, Coles requiring pallets
homogeneous by expiry date, a US retailer accepting only SOPI: all three are rules
of a trading relationship. They belong in D22's lattice with the party as a scope,
and they are checked as D8 findings rather than enforced as constraints, because a
constraint that encodes one customer's rule silently applies it to another's
goods.

The GS1 grocery pass reached this for outbound pallet composition, where Coles and
Metcash contradict each other outright. This pass reaches it for inbound message
cardinality. Two independent routes to the same rule is the reason to believe it.

The counterpart is the one restriction that **is** universal and therefore worth a
schema-level invariant rather than a policy row:

> **One order per logistic unit.** X12 enforces it structurally by putting the
> order node above the physical levels. Metcash states it as prose. It is
> checkable over `package_content` and, on the outbound side, over what we build.

---

## Structural or additive

| Residue | Verdict | Cost |
|---|---|---|
| Multi-PO advice | Already supported, D24 supply side | Nothing. Cross-reference 109 from 68 |
| Order reference stated above the line | Additive, and option A costs no schema | One `level_code` value, one reversed sentence |
| `goods_receipt` grain for multi-PO deliveries | Undecided, not wrong | A stated rule; `inbound_shipment_id`; the widened source set |
| Multi-destination, multi-vendor, depth | No gap, checked | Nothing |

Nothing here touches a table with history, and nothing here is an omission that
needs recording. **68 needed refining rather than settling**, since the register
had already settled it, which is a different answer again from the one it was
filed expecting.

---

## Proposed for the registers

**Deliberately unnumbered.** The GS1 pass has four proposals outstanding and
allocating numbers in a research document while another is in flight is precisely
how the three collisions happened. Numbers are allocated in the commit that
adopts.

**Questions to refine.** *(Adopted by D43.)*

- **68 and 109** are settled and stay settled. D24 (supply side)'s reasoning gains
  the X12 case, and the order level is stored as a node rather than flattened.

**Questions to raise.**

- Whether `raw_*` may ever hold an inherited value, stated generally rather than
  for this case. The 856 is the first instance and it will not be the last, since
  any hierarchical message can state an attribute at a level above the one we
  store.
- Whether a per-party message-cardinality profile is a `policy_kind` under D22 or
  a `party_profile` capability column under D20. Both fit; they differ in whether
  the rule is resolvable by scope.

**Invariants to propose.**

- No `package` and no `asserted_unit` subtree resolves to content lines naming
  more than one purchase order. The vacuity marker applies: it passes on an empty
  advice population and needs a companion assertion that advised receipts exist.
- Every `asserted_unit` node with a non-physical `level_code` has no `sscc` and
  contributes no `package` row at receipt. Structural, and the check that stops
  option A leaking document nodes into the physical projection.

**Corrections.**

- The inbound analysis's *"S and O are documents"* is narrowed to S under option
  A. The sentence is quoted by D24 (supply side), so the amendment names it.

---

## Sources

- [Metcash Despatch Advice Message Implementation Guideline v1.10](https://mars-metcdn-com.global.ssl.fastly.net/content/uploads/sites/101/2017/04/18110045/Metcash-DESADV-v1.10.pdf)
- [EDI WERX, the four 856 hierarchical structures](https://www.ediwerx.com/856-series-asn-hierarchical-structures/)
- [D&H 856 Ship Notice/Manifest implementation guide, X12 4010](https://www.dandh.com/docs/EDI_Guides/Vendor/Implementation%20Guide%20856,%204010.pdf)
- [Sally Beauty 856 ASN implementation guide, X12 5010](https://community.spscommerce.com/wp-content/uploads/2021/02/Sally-Beauty-Holdings-Inc-856-ASN-v5010.pdf)
- [Kmart 856 Ship Notice/Manifest guidelines, v4010](https://www.edibydesign.com/newdesignAssets/uploads/Retailers_pdf/856_kmart.pdf)
- [Orderful, 856 packing structures and errors](https://help.orderful.com/docs/856-reducing-errors-and-optimizing-packing)
- [PartnerlinQ, EDIFACT DESADV message guide](https://www.partnerlinq.com/edi-guide/what-is-the-edifact-desadv-despatch-advice-message)
- [GS1, EANCOM DESADV syntax 3](https://www.gs1.org/standards/edi-xml-gs1-eancom/eancom-desadv-s3/syntax-3)
