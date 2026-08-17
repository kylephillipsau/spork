# The warehouse data layer, and the features that sit on it

Companion to [order-fulfilment-process.md](./order-fulfilment-process.md).
Working document — this is a design being worked out, not a settled schema.

## The point

The planned features look like four separate projects:

1. 3D warehouse map
2. Picking route optimisation
3. Optimal packing strategy calculator
4. Fulfilment processing (predefined packages, autofilled WMS dimensions)

They are not. They are **four readers of one data layer**. Every one of them
needs some combination of: where things physically are, what things physically
are, and how much of each is where. Build that spine once and the four features
become comparatively thin; build them separately and the same data gets modelled
four incompatible ways.

That also sets the build order. The spine comes first, and it is worth getting
right, because everything else inherits its mistakes.

## What the walkthrough already told us

Worth noticing before designing anything. From the fulfilment walkthrough, on
pallets:

> "usually I change the **weight** and **height** … length and width are standard
> unless the stacked pallet is oversize"

That is the data model stating itself out loud:

- **Length and width are constant** because the pallet footprint is fixed.
  **[unverified: confirm the actual footprint — Australian standard is
  1165 × 1165 mm, but CHEP/Loscam and 1200 × 1000 stock all exist.]**
- **Height varies** because the number of stacked layers varies.
- **Weight varies** because the contents vary.

So height and weight are not independent facts that must be measured. They are
**derived from contents plus packing configuration**:

```
height = pallet base height + (layers x carton height)
weight = pallet tare weight + sum(item weight x quantity)
```

If we hold item dimensions and a packing configuration per item, stage 4 of the
fulfilment process stops being data entry and becomes **confirm or correct a
computed value**. That is the single highest-leverage thing in this document,
and it falls out of reference data we would need for the packing calculator
anyway.

## Core entities

Sketch, not a migration. Field lists are indicative.

### Physical space

**Site** — Melbourne today; assume more later, because location was an explicit
check in the fulfilment process.

**Location / bin** — the addressable unit.
- Location code (the human/scanner identifier, typically encoding aisle-bay-level)
- Aisle, bay, level, position — parsed, not just a string, so we can sort and route
- Physical origin `(x, y, z)` and dimensions — **required by the 3D map and the
  route optimiser**; a code alone is not enough
- Type: pick face, bulk, overflow, staging, dock
- Constraints: max weight, max height, equipment needed to reach it

Getting coordinates is a **warehouse survey**, not a data import. See risks.

**Traversable graph** — aisles and cross-aisles as edges with distances. The
route optimiser needs a graph; the 3D map needs geometry. Derive the graph from
the layout rather than maintaining it separately.

### Products

**Stock item (SKU)**
- Item code, description
- **Each-level** dimensions and weight
- **Unit-of-measure hierarchy**: each → inner → carton → layer → pallet.
  This is the part that makes autofill and packing possible.
- Cartons per layer, layers per pallet (the classic Ti/Hi pair)
- Handling: stackable, max stack height, fragile, temperature, orientation
  constraints ("this way up")
- Dangerous goods: DG class, UN number, packing group — feeds the MachShip
  declaration directly rather than a per-consignment tickbox

**Packing configuration** — how a SKU is presented for shipping. The
walkthrough's "prepack variable items … usually named by item code" are exactly
this, already existing as presets in NetSuite. Worth extracting rather than
reinventing.

### Packaging

**Package preset** — the catalogue behind "select a predefined package".
- Name (`PALLET`, `SKID`, `small box` … `large box`, or an item code)
- Default L/W/H, tare weight
- Package type code (`PAL` etc.) and its mapping per carrier
- Whether dimensions are fixed or expected to be overridden (a pallet's height
  always varies; a box's rarely does)

Versioned, because dimensions get corrected and old consignments must still
explain themselves.

### Inventory

**Stock on hand** — item x location x quantity, plus lot/batch, expiry, and
serial where applicable. Expiry is a live requirement, not a future one: most
food safety products are non-perishable, but a few lines require refrigeration
and some protective equipment carries a shelf life. Modelling it properly for
those also means the model can serve a perishables operation without reopening
the core.
**[unverified — confirm whether lot/expiry tracking is in scope.]**

**Stock movement** — the append-only log. Every feature that reasons about
history or reconciles against NetSuite needs this, and it cannot be added
retroactively.

## Feature to data dependency map

| Feature | Locations w/ coords | Traversable graph | Item dims + UoM | Packing config | Package presets | Live stock |
|---|---|---|---|---|---|---|
| 3D warehouse map | **Required** | — | — | — | — | For occupancy view |
| Picking route optimiser | **Required** | **Required** | — | — | — | **Required** (where to pick from) |
| Packing calculator | — | — | **Required** | **Required** | **Required** | — |
| Fulfilment: preset selection | — | — | — | — | **Required** | — |
| Fulfilment: WMS autofill | — | — | **Required** | **Required** | **Required** | — |

Two things fall out:

1. **Package presets are the cheapest win.** One small catalogue, no coordinates,
   no survey, and it unblocks preset selection immediately.
2. **Item dimensions plus packing configuration unlock three features at once**
   — autofill, the packing calculator, and accurate freight costing. This is the
   highest-value reference data in the system.

Coordinates and the traversable graph are needed by the two most visible
features (map, route optimiser) and by nothing else. They can be sequenced later
without blocking fulfilment work.

## Why packing accuracy is worth money

Australian freight is generally charged on **cubic weight or dead weight,
whichever is greater**. That means how well an order is packed changes what the
consignment costs, not just how tidy it is. A packing calculator that reduces
cube directly reduces freight spend, and it can be measured against actual
invoices.

**[unverified: confirm the cubic conversion factor used by Swift and Direct.
It varies by carrier and materially affects the optimisation target.]**

This also connects to the direct carrier integrations in D1 — once we hold
accurate dimensions, we can validate what we are being charged.

## Dimensional data is a living asset, not a data-entry project

The SKU range only grows with scale. So any plan that reads "measure everything,
then build on it" is wrong by construction: it finishes behind, and it makes the
whole system's usefulness hostage to a backlog.

The design principle instead: **dimensional data is captured and corrected by
the workflows that already touch the product.** Four feeds, none of which is a
project:

**1. Capture at receipt.** A SKU's first physical contact with us is goods
receipt and put-away. Measuring and weighing once at the dock, as part of that
existing flow, means new SKUs arrive with dimensions already attached. This is
the primary feed and the only one that needs new operator behaviour — and it
scales with intake rather than with catalogue size.

**2. Learn from fulfilment corrections.** Autofill computes a height and weight;
the operator confirms or corrects. **A correction is a measurement.** If a SKU's
computed weight is consistently under by the same amount, the record is wrong and
the system should say so rather than being quietly corrected forever. This turns
every one of the hundreds of daily orders into a data point.

**3. Reconcile against carrier actuals.** Carriers re-weigh and re-cube
consignments, and bill discrepancies back. Those figures are a free, independent
measurement of what we shipped. Comparing predicted against carrier-measured
does two useful things at once: it finds bad records, and it quantifies what
inaccurate data is currently costing in discrepancy charges. Worth pulling as
soon as we hold predictions to compare against.

**4. Bootstrap from what exists.** NetSuite's prepack presets already encode
packing configurations. Supplier data covers some dimensions. Neither will be
complete or fully trustworthy, which is fine — they are a starting position, not
a source of truth.

**Confidence is a first-class field.** Every dimensional record carries how it
was obtained (measured / supplier / derived / estimated), when, and how well it
has held up against feeds 2 and 3. Autofill uses it to decide whether to present
a value as confirmed or as a suggestion needing a look. This is what lets the
system be useful on day one with partial data instead of waiting for
completeness — an estimate flagged as an estimate is useful; an estimate
presented as fact is worse than nothing.

The consequence for sequencing: **step 2 below is not "populate the item
catalogue" but "build the capture and confidence model".** Coverage then grows
on its own, fastest for the SKUs that ship most, which are the ones where
accuracy pays.

## Static reference vs live state

Worth separating early, because they have completely different needs:

| | Change rate | Source | Consequence of being wrong |
|---|---|---|---|
| Layout, coordinates | Rarely | Survey | Map and routes wrong |
| Item dims, packing config | Occasionally | Measurement + supplier data | Autofill and quotes wrong, freight overcharged |
| Package presets | Occasionally | Extract from NetSuite | Wrong labels/dimensions |
| Stock on hand | Constantly | Scanner / NetSuite | Picks fail |

Only the last row is genuinely live. The first three are reference data that can
be edited by hand, versioned, and reviewed — which means a decent chunk of this
is an admin UI over slow-moving tables, not a real-time system.

## Risks

**1. Data acquisition is continuous, not a project.** See "Dimensional data is a
living asset" below — this was originally written as a one-off measurement
exercise gated on SKU count, which is the wrong shape.

**2. Two sources of truth for inventory.** NetSuite is the ERP and the WMS
Android scanner already syncs picking into it. If we stand up our own inventory
tables, stock exists in two places. Per **D2** we should not let NetSuite shape
the model, but inventory is the one area where being wrong has immediate physical
consequences. Suggested stance: **mirror NetSuite inventory read-only at first**,
prove the model against reality, and only take ownership once the scanner path
moves to us. That needs deciding explicitly rather than by drift.

**3. The survey is real work.** Coordinates for every bin do not exist anywhere
today. This gates the map and the route optimiser, and nothing else — so it
should not be allowed to gate the fulfilment work.

**4. Scope.** This document describes a WMS. That is a much larger system than
the fulfilment tool, and the fulfilment tool is what currently hurts. The
sequencing below is deliberately biased toward shipping something useful early.

## Suggested sequencing

Reasoning, not a commitment:

1. **Package presets** — small catalogue, immediately replaces a NetSuite lookup,
   validates the preset model against real use.
2. **The item model plus the capture and confidence machinery** — not a populated
   catalogue. Unlocks WMS autofill for whatever is covered, degrading honestly to
   a suggestion where it is not, and starts accumulating coverage from day one.
3. **Fulfilment processing** on top of both, per the process doc.
4. **Packing calculator** — same data as step 2, now used to decide rather than
   describe.
5. **Locations, coordinates, inventory** — the survey and the live stock model.
6. **3D map and route optimiser** — the visible payoff, once the spine exists.

Steps 1–4 need no survey and no coordinates. Steps 5–6 are where this becomes a
WMS proper.

## Open questions

1. Is lot/batch/expiry tracking in scope? (FEFO changes the inventory model and
   the picking logic.)
2. What does NetSuite already hold for item dimensions and packing
   configurations, and how complete and trustworthy is it?
3. What is the actual pallet footprint in use, and are there several?
4. Is there a weigh/cube station at goods receipt today, or would capture at
   receipt need equipment as well as a process change?
5. Does the WMS Android scanner have an API or export we could read, or is
   NetSuite the only way to see picking data? (It is also the obvious device for
   capture at receipt.)
6. Cubic conversion factors for Swift and Direct.
7. Do carrier invoices expose re-weigh/re-cube figures per consignment, and in
   what form? Determines how quickly feed 3 can be switched on.
