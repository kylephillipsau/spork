# Order fulfilment: the process as it runs today

Source: walkthrough of one real order, recorded 2026-07-30, by the person who
runs this process. Volume is **hundreds of orders per day**.

This document is the as-is record. It exists so we can (a) see the whole path in
one place instead of across a dozen NetSuite views, and (b) work out which parts
the NetSuite and MachShip API surfaces can take over. Analysis lives in the
second half; the first half is description only.

Status of claims: everything in "The process" is as described in the walkthrough.
Anything I inferred or could not verify is marked **[unverified]**.

## Context

- **Site**: Melbourne warehouse (location is checked explicitly at stage 3).
- **Picking** happens upstream on a **WMS Android scanner**, which syncs the
  picking record into NetSuite. Packing is what this process covers.
- **Systems**: NetSuite (system of record), MachShip (freight), a label printer,
  and an A4 printer.
- **Goal**: replace NetSuite as the North Star. This process is the first real
  workload to move, so it defines the minimum the replacement must do.
- **Target stack**: Rust backend (Actix-web + Diesel + PostgreSQL, following
  Nosdesk), React frontend.

## Systems and where the record lives

| Stage | System | Record |
|---|---|---|
| Pick | WMS Android scanner → NetSuite | Item Fulfilment (status `Picked`) |
| Pack | NetSuite | Item Fulfilment (status `Packed`) + WMS Transaction |
| Paperwork | NetSuite | `NLSPrintForm.nl`, A4 |
| Freight | MachShip | Pending Consignment → Consignment |
| Labels | MachShip → label printer | Label PDF |
| Manifest | MachShip | Manifest PDF (route-dependent) |

## The process

### Stage 1 — Locate the order (NetSuite)

1. Enter the **confirmation number** into the search bar.
2. Find the sales order with that confirmation number, **confirm it matches the
   contact**, and open it.

### Stage 2 — Find and validate the fulfilment (NetSuite)

3. On the sales order, open **Related Records** and find the **Item Fulfilment**
   record. It should have status **`Picked`** if picking is done.
4. Open the Item Fulfilment record and confirm the **warehouse location for the
   items is the Melbourne warehouse**. If it is, proceed.

This is a gate: wrong location or a status other than `Picked` means stop.

### Stage 3 — Record the pack (NetSuite)

5. Press **Edit** and set three fields:

   | Field | Value |
   |---|---|
   | Picked By | `Casual Melbourne` |
   | Packed By | `Daniel Stooke` |
   | Status | `Packed` |

   All three are effectively constant for this operator and site.

### Stage 4 — Record the packages (NetSuite, WMS tab)

6. Go to the **WMS tab** → **WMS Transaction** section.
7. Enter the pallet / skid / carton / box details: **dimensions and weight**.
   - Many are **"prepack" variable items** already configured, searchable and
     selectable from a **preset list**. Presets are usually named by **item
     code**, or by size (`small box` … `large box`), or by type (`PALLET`,
     `SKID`).
   - For pallets, the fields typically changed are **weight** and **height**.
     **Package type** is set to `PAL` by the preset. **Length and width are
     standard** unless the stacked pallet is oversize, in which case they are
     updated too.

This is the only stage with genuinely variable, physically-measured input.

### Stage 5 — Print the packing paperwork (NetSuite)

8. Hit **Save and Print**. This saves the record and opens **`NLSPrintForm.nl`**
   in a new tab.
9. Print on **standard A4**, fold, and attach to the pallet.

### Stage 6 — Pick up the consignment (MachShip)

10. On save, **NetSuite sends the record details to MachShip**, creating a
    **Pending Consignment**.
11. In MachShip, go to **Pending Consignments** and identify the new one by
    **delivery location**, which should match the contact on the sales order and
    the Item Fulfilment record.

There is no shared key surfaced in this step — matching is done by eye on the
delivery location. **[unverified]** whether a reference is carried through and
simply not used.

### Stage 7 — Route and configure (MachShip)

12. Open the consignment and **pick the appropriate route** (sender / courier).
13. **Subsequent steps vary substantially by route.** See the variants table.
14. Confirm the items are correct. **If there are multiple boxes of the same
    type, set the quantity here** — NetSuite records only one item for that
    product/box type, so the count is entered in MachShip.
15. Tick the declaration that the consignment contains **no dangerous goods**.
16. Press **Create and download labels**.

### Stage 8 — Labels

17. A **PDF of the labels** is saved. Open it and print to the **label printer**.
18. Labels go on the pallet / box.

### Stage 9 — Manifest (route-dependent)

For Swift Transport: take the reference number noted at stage 7, search MachShip
**Active Consignments**, find the consignment, then **cog / settings / options →
Print paperwork → Manifest**. The manifest opens as a PDF and is printed.

## Route variants

| Route | Despatch date | Caller | Reference | Labels | Manifest |
|---|---|---|---|---|---|
| **Swift Transport Services** | Next business day, **07:00** | `Alpha` | Note it (e.g. `IF263824`) — needed at stage 9 | 1 | Yes, printed per consignment via Print paperwork |
| **Direct Transport** | — | — | — | **2 if on a pallet** (front and back of pallet) | — |
| Other routes | — | — | — | 1 | — |

The walkthrough notes "a couple of cases where the MachShip process changes"
beyond these. **This table is incomplete and needs the remaining routes added.**

The reference number format `IF263824` suggests the NetSuite Item Fulfilment
internal ID with an `IF` prefix — which would mean a usable join key between the
two systems already exists. **[unverified, but worth confirming early: it would
remove the eyeball-matching at stage 6.]**

## Observations on the data

1. **Three constant fields.** `Picked By`, `Packed By`, `Status` at stage 3 carry
   no decision. They are keystrokes, not judgement.
2. **The quantity gap is a real model mismatch.** NetSuite holds one line per
   product/box type; MachShip needs a package count. The count is currently
   re-entered by hand in MachShip and exists in neither system beforehand.
   Any replacement needs a package-level model, not just a line-item model.
3. **Package presets are the useful domain data.** The prepack list (item code,
   `small box`…`large box`, `PALLET`, `SKID`, with default dimensions and
   package type `PAL`) is a small, high-value catalogue. It should be first-class
   in the replacement, not buried in a NetSuite tab.
4. **Only weight and height are genuinely variable** for pallets. Length and
   width are standard except when oversize. That is a very small input surface
   for how much UI it currently costs.
5. **Two printers, two paper paths**: A4 paperwork from NetSuite, labels from
   MachShip. Both are PDFs opened and printed by hand today.
6. **The pain is navigational, not conceptual.** The walkthrough's own summary:
   the NetSuite process is *"simple but convoluted across many interface views,
   loading states, and extremely poor application UX design."* The work is easy;
   the software makes it slow.

## Automation analysis

Mapping each stage to the API surface researched separately (see the API notes
in this branch). Feasibility is my assessment and needs validating against the
actual accounts.

| Stage | API path | Feasibility |
|---|---|---|
| 1–2 Locate SO + IF, check status and location | **SuiteQL** — one query joining sales order, item fulfilment, location, contact | **High.** Collapses a search, a record open, a related-records tab and a second record open into one query. |
| 3 Set Picked/Packed By + Status | `PATCH /record/v1/itemFulfillment/{id}` | **High.** Constant values, no input needed. |
| 4 WMS Transaction package details | **Unknown** | **The main risk.** WMS Transaction looks like a SuiteApp/custom record. Whether it is exposed to REST at all, and under what record type, has to be checked before anything else is designed. |
| 5 `NLSPrintForm.nl` A4 | Not a REST surface | Either keep the NetSuite form, or re-render the paperwork ourselves once we hold the data. |
| 6 NetSuite → MachShip pending consignment | Bypassable | We can call `createConsignment` directly and skip Pending Consignments entirely, removing the eyeball match. |
| 7 Route + despatch date + caller + quantity + DG | `POST /apiv2/routes/returnroutes` then `POST /apiv2/consignments/createConsignment` | **High.** Despatch date, caller, quantity and the DG declaration are all payload fields. Route rules per carrier become config, not memory. |
| 8 Labels | `createConsignmentwithComplexItems` lets us **supply our own barcodes** | **High**, and it routes into the existing Zebra ZPL pipeline rather than printing a PDF by hand. "2 labels for Direct Transport on a pallet" becomes a rule. |
| 9 Manifest | **Blocked as far as I can tell** | MachShip docs describe manifesting as a bulk operation in their web UI, not an API call. Needs confirming with MachShip directly. |

### What this suggests

The process decomposes into **one screen of real input** (package weight/height,
occasionally L/W, plus package count) wrapped in a lot of navigation that is
entirely mechanical. A single-screen React app over a Rust service could
plausibly reduce it to: scan/enter confirmation number → confirm the pick →
pick a preset and enter weight/height/count → done, with labels and paperwork
printing automatically.

That is worth quantifying before building: **we have no measured per-order
time.** At hundreds of orders per day, even a one-minute saving is hours daily,
but the business case should rest on a real number, not this assumption.

## Decisions taken

### D1 — Keep MachShip as the freight layer for now (2026-07-30)

**Decision.** Automate MachShip via its API rather than replacing it. Revisit
direct carrier integrations with **Direct Transport** and **Swift Transport**
later, once we have made contact with them.

**Why.** The pain in stages 6–9 is MachShip's *UI*, not MachShip's role: the
pending-consignment matching, manual route selection, remembered per-route rules,
re-entered box counts, and hand-printed PDFs. All of that is removable through
`returnroutes` + `createConsignmentwithComplexItems` while keeping their carrier
coverage (200+ Australian carriers) for free.

Replacing MachShip outright would mean owning per-carrier connections (many of
which are EDI or flat-file, not REST), carrier-conforming label formats and
consignment note number ranges, manifest transmission formats, and tracking
normalisation. None of it is hard; all of it is ongoing and breaks when someone
else changes something. Taking that on would also put the warehouse's ability to
ship behind our own integration work, on top of an already large NetSuite
replacement.

**Revisit when.** We have made contact with Direct and Swift about direct
integration. **Confirmed 2026-07-30: Swift and Direct are our highest volume
routes, and are the two to go direct with.** The end state is therefore direct
integrations for Swift and Direct Transport, with MachShip retained for the long
tail — not MachShip everywhere, and not MachShip nowhere.

**Consequence for the design.** Because two named carriers are a stated
requirement rather than a hypothetical, the freight layer gets a **carrier
abstraction from day one**: quote, create consignment, produce labels, manifest.
MachShip is the first implementation (an aggregator covering many carriers behind
one interface); Swift and Direct become sibling implementations later. This is
worth building up front only because the second and third implementations are
committed — retrofitting the seam afterwards is the expensive path. Keep the
interface thin until there is a second implementation to check it against.

**Note on rates.** MachShip appears to return the rate **calculated by the
freight company** rather than computing from an uploaded rate card
**[unverified — needs confirming]**. If that holds, two things follow: MachShip's
value is orchestration rather than pricing, and a future direct integration would
not inherit rate-card maintenance, which lowers the cost of going direct. Worth
confirming before costing that work.

### D2 — NetSuite's data model does not constrain ours (2026-07-30)

**Decision.** Build the domain model that is correct for the work, not the one
that maps cleanly onto NetSuite. Where NetSuite cannot represent something we
need, that is a NetSuite shortcoming to be improved upon, not a constraint to
inherit. NetSuite write-back is a **degradable adapter** used during coexistence,
not the shape of the system.

**Why.** The goal is to surpass NetSuite, not to reimplement it. The walkthrough
already surfaced one place where NetSuite is simply wrong for the job — the box
count (see observation 2) — and a model built to mirror NetSuite would inherit
that defect permanently.

**What this changes.**

- The **WMS Transaction reachability question is no longer a blocking design
  fork.** It drops from "decides the schema" to "decides how much fidelity we can
  write back to NetSuite during coexistence". We still want the answer, but the
  Rust models do not wait on it.
- **Packages become first-class**: a fulfilment has N packages, each with a type,
  dimensions, weight and barcode. The count is a real field, not something typed
  into MachShip at the last moment.
- **Package presets become a real, versioned catalogue** rather than a list
  buried in a NetSuite tab.
- **Route rules become data**: Swift's 07:00 next-business-day despatch, its
  `Alpha` caller value, Direct's two-labels-on-a-pallet rule. Config, not
  operator memory.
- **The fulfilment lifecycle becomes one explicit state machine** instead of
  statuses spread across several records and views.

**Quality bar.** The stated problem with NetSuite is not that it lacks features,
it is "many interface views, loading states, and extremely poor application UX
design". So the bar is concrete and testable: **one screen, no page loads,
scanner/keyboard driven, sub-second interactions.** If the replacement needs as
many views as NetSuite, it has failed even if it is functionally complete.

## Open questions

In rough priority order. Note that per **D2** none of these block the domain
model any more — they shape the NetSuite adapter and the migration path.

1. **Is the WMS Transaction record reachable via REST or SuiteQL?** Decides how
   much package fidelity we can write back to NetSuite while it is still the ERP.
   If not reachable, the options are a RESTlet or accepting that NetSuite holds a
   lower-fidelity copy than we do. No longer a schema fork (see D2).
2. **Is `IF263824` the Item Fulfilment internal ID?** If yes, we have a join key
   and stage 6's manual matching disappears.
3. **Can manifesting be triggered via the MachShip API?** Not in the public docs.
   Worth asking MachShip's integrations team, who also have to enable webhooks
   manually.
4. **What are the remaining route variants?** The table above covers Swift and
   Direct Transport; the walkthrough says there are more.
5. **Is the NetSuite account on SuiteCloud Plus?** Sets the concurrency ceiling
   (10 requests by default) and therefore how hard we can push at volume.
6. **What is the actual per-order handling time**, and how is it distributed
   between the stages?
7. **What is the consignment volume per carrier?** Decides whether direct
   integration with Direct and Swift is worth it later (see D1), and there is no
   point asking them about integration without it.
8. **Does MachShip pass through the carrier's own calculated rate**, or price
   from a rate card we maintain? Changes the cost of ever going direct (see D1).

## Next steps

Not started — listed so they are not lost:

- Add the remaining route variants to the table.
- Verify questions 1 and 2 against a real NetSuite account; they gate the design.
- Time a sample of orders per stage.
- Decide how the Zebra ZPL pipeline is shared between this repo and the new
  service (three options discussed on this branch, undecided).
