# Migration 1: what has to be in it

An audit of the inbound analysis's tier-0 list, *"cheap now, ruinous later, do
before any inbound code"*, against the decisions adopted since it was written
(D21 to D44).

> **Derived, not authoritative.** [domain-model.md](./domain-model.md),
> [invariants.md](./invariants.md) and [open-questions.md](./open-questions.md)
> are the registers. This is a working list for sequencing the first migration
> and it goes stale the moment a decision lands. Re-derive it rather than trust
> it.

Generated 2026-08-04. Method: each of the eleven items checked by name against
the decision record, with the adopting decision cited or its absence recorded.

---

## The count

*Closed 2026-08-06.* **Eleven settled, one obsolete, none outstanding.** Two
prerequisites the list never had, both settled.

The outstanding four were the whole point of the exercise, because each was
unrecoverable rather than merely inconvenient: the column has to exist before the
first row is written or the history it would have carried is gone. D58 closed
three and half of the fourth; **D61 closed the rest**.

**What the last item cost is the lesson.** It was three columns on
`goods_receipt_line`. That line names `expected_supply`, which requires an origin
document, which needed a `purchase_order` nineteen places referred to and nobody
had defined. Three tables and three decisions — D59, D60, D61 — to land three
columns the list had described as a column set for two weeks. Re-derive this file
rather than trust it, which is what the banner says and what this proves.

---

## Settled since the list was written

**2. `stock_movement.from_owner_id` / `to_owner_id`.** Adopted in D24 (supply
side)'s `stock_movement` block, with a CHECK pairing status and owner. This was
the sharpest defect the inbound pass found, D20 having broken D12's invariant in
passing, and it is closed.

**3. `goods_receipt`'s demand CHECK, widened, with demand on the line.** S3 forces
`<= 1` on every grouping table and names this as the case that caught
D16-repeats-D10. D44 widened the header source set to four and added
`inbound_shipment_id`. The line reaches its own demand through `expected_supply`,
which J26 folds received quantity across.

**5. `item_barcode` and the shared unit vocabulary.** D34 defines `item_barcode`
with the exclusion constraint doing three jobs; D23 owns the unit vocabulary and
D34 names it as the consumer.

**6. `item_class`.** D22's prerequisites section defines `item_class` and
`party_class` as per-tenant rooted trees with closure-table projections, plus
`item_classification` as an association table keyed
`PRIMARY KEY (tenant_id, item_id)` so single parentage is preserved and D19's
thin shared item survives. The list's complaint that two decisions referenced it
and nothing defined it is no longer true.

**8. `number_range`.** D29 defines it scoped to the issuing legal entity, with an
explicit row per extension digit, `sscc_allocation` as the append-only fact, J35
as the reuse guard and D31 carrying the twelve-month floor. D44 records that the
same table needs a pool arm before it can issue GTINs, which is a later problem.

**9. The `stock` index, corrected on the way through.** D24 (supply side) says it
directly: *"Inbound Tier-0 asked for `stock(location_id, item_id)`. It must be
`stock(resolved_location_id, item_id)`"*, because on `holder_location_id` the
index misses everything sitting inside a package. The tier-0 item was right that
an index was needed and wrong about which one.

## Obsolete

**1. `package_content` gains `item_id` and typed demand FKs.** The list called
this *"the migration nobody wants"* and urged doing it while the table had no
history. It is moot: **D24 retired `package_content` as a base table**. It is now
a view over `stock`, and D6's containment columns were demoted to projections.

Worth pausing on, because it is the most interesting result here. The single most
expensive item on the list was not performed early, it was dissolved by getting
the model right. A structural fix removed the migration instead of scheduling it.

## Settled by D58, and each one would have lost history

**4. `entered_quantity`, `entered_unit` and `item_packing_config_id`.** ~~Outstanding.~~
**Done**: `stock_movement` in migration 18, `goods_receipt_line` in migration 21. D58 also corrected the
list's wording — the middle column is not a unit. A carton is not dimensional; its
factor varies by item and by date, which is the whole reason `item_packing_config`
is versioned, so the column is a `packaging_level` and not a `unit` foreign key. The idiom exists elsewhere, on catch
weight and on `asserted_unit_content`, but neither table the list names carries it
in an adopted block. The `item_packing_config_id` half is the one that bites:
without the FK, correcting a case pack silently rewrites what every historical
receipt meant. D23 already versions `item_packing_config` by `effective_from` for
exactly this reason, so the column is the other half of a mechanism that is
otherwise half-built.

**7. `lot.country_of_origin` and `lot.production_date`.** ~~Outstanding.~~
**Done in migration 18, D58.** Was: zero mentions in the decision record. Not backfillable, because nobody can reconstruct where a lot came
from after the pallet has gone. The GS1 pass strengthened the case: Coles requires
country of origin on every master carton, and the inbound analysis established
that it belongs on `lot` rather than `item` because it varies lot to lot for food.

**10. `stock_movement.unit_cost_minor`.** ~~Deferred.~~ **Done in migration 18,
D58**, with `cost_basis_quantity` and `cost_currency` beside it, because a cost
with no basis is ambiguous and one with no currency has no units. Was: no longer
blocked.
D44 named it while parking price; question 127 carried the boundary with D40 and
**D50 settled it**, stating in terms that a cost is not a price and that this
column stays out of the order set. So what was waiting on a decision is now
waiting on nothing, and the deferral's running price keeps accruing: history
written before the column exists has no cost on it and cannot be given one.

**11. `promised_from`, `promised_to` and `required_by` on `order`.** ~~Absent.~~
**Done in migration 9**, and demoted to `@projection` of `intention_amendment` by
D42, registered and grant-guarded by D49. This entry was stale from the day
migration 9 landed and stayed that way through six migrations, which is what the
banner at the top of this file is warning about.

## Two prerequisites the list never had

**~~`zone` becomes a real table.~~ Settled by D46.** The table is identity,
membership and a name, because everything else a zone might carry is either a
property of a location or a policy bound to the zone, which is what D22 built the
Space dimension for. Flat, matching the lattice's `any → site → zone`; nesting is
deferred to question 129. `location.zone_id` is nullable, since a dock belongs to
no zone, and its foreign key is composite on `site_id` so a location cannot join
another site's zone.

**~~`goods_receipt_line` has no adopted DDL anywhere.~~ Settled by D45.** Three
decisions depended on its columns while none defined it, which is where
`item_barcode` sat before D34 and `device` before D27.

Defining it found a defect rather than merely filling a gap. J26 folded
`expected_supply.quantity_received` across `goods_receipt_line` rows, and the
table carries no quantity, so the invariant named the right relationship over the
wrong table and could not have run. It now folds `stock_movement`. The line also
takes a single `expected_supply_id` in place of the inbound sketch's two arms,
which predated `expected_supply` and would have rebuilt its union one level down.

---

## Suggested order

1. **`zone`, `item_class`, `item_classification`.** Reference data with no
   dependencies, and the policy lattice needs them. All three are now defined,
   `zone` by D46 and the other two by D22's prerequisites.
2. ~~**Define `goods_receipt_line`.**~~ Done, D45.
3. **The four outstanding column sets**, all of which can land: `unit_cost_minor`
   waited on 127, and D50 settled it.
4. **The structural invariants.** Forty-eight of the hundred and five need no data and
   run against the empty schema, so they can be written alongside the migration
   and fail honestly until it exists.

Nothing in the decision record now blocks writing the migration. What is left on
this list is column additions and the suite.

Only after those does anything read or write rows worth keeping.

## What this does not cover

Sequencing beyond the first migration, the packing screen, and the seeded year of
history that question 122 wants. Those are unblocked by this work rather than part
of it.
