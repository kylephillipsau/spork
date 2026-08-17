# Q89, Q90, Q91 — research and proposed decisions

**Status: proposed, not adopted.** Generated 2026-08-02 by a multi-agent pass over
six research streams (two per question), each adversarially tested against the
adopted D1-D27.

Read the cross-cutting section before adopting any of the three. It reports four
further invariants that encode the bug they were written to catch — the J8 pattern
again — and one live defect in an adopted decision (D23/D9).

**Provenance caveat.** Per-stream research and raw assessments are in the run
journal at `.claude/projects/*/subagents/workflows/wf_e86e2460-383/journal.jsonl`.
Claims about DataWedge, GS1 specifications, D365, Odoo, GitLab, Fivetran and dbt
internals are second-hand from public documentation.

---

# Q89, Q90, Q91 — decisions

---

## Q89 — Does a failed container scan need an `activity_event`?

### Decision

**Yes for identification failures, no for identification successes, and never for decode failures.** `activity_event` gains four resolution-failure kinds and typed columns for the string that failed to resolve. `scan_ok` is **deleted** from D17's adopted kind list. Scan-rate denominators come from `client_event` (D25), which already takes one row per fact-producing act and costs nothing new.

### Reasoning — the obvious answer is wrong in both directions

The obvious answer is "yes, D17's `location_empty` argument applies verbatim." It half-applies, and the half that doesn't is the expensive half.

**"Failed scan" is two populations separated by a hardware boundary, and only one is observable.** On a Zebra handheld a no-read produces *nothing*. DataWedge's intent bundle is `source, label_type, data_string, decode_data, decoded_mode` — no status field, no failure variant. The Get Scanner Status enum has six values (WAITING, SCANNING, IDLE, DISABLED, CONNECTED, DISCONNECTED) and none is "failed decode." Beam-timer expiry in multi-barcode mode discards a *partially successful* capture silently: "the scanned data is not reported." Even OPC UA's AutoID spec — the most careful public model of a reader — puts its failure enum on the **synchronous** `Scan()` method only; in trigger-driven mode the reader "throws Events at each time a transponder or code has been detected," and a no-read produces no event.

So a `no_read` kind would read zero forever and be believed. That is worse than having no metric, and it is exactly D25's falsifier approached from the other side: a kind whose row count is bounded by nothing that happened in the world.

**The population we *can* record is the benign one, and the decision text must say so.** A misread returns a valid-looking wrong value. GS1 mandates ITF-14 bearer bars specifically to reduce short scans; DataWedge ships read-redundancy security levels and Picklist mode. A misread that survives those is indistinguishable from a correct scan and surfaces *only* as a contradiction against an expectation — i.e. as `scan_mismatch` or a `containment_conflict`. Recording unresolvable identifiers does nothing to find misreads. Do not let a failure kind imply otherwise.

**`activity_event` is the right home because it is the only table in the model that can hold a subjectless fact.** This is the crux. A scan that resolved to nothing cannot be filed against the thing it failed to identify:

- not `observation` — `observable` has ten typed arms under `CHECK (num_nonnulls(...) = 1)`, and D23's licensing argument is that "nobody will ever discover an observation about no thing." An unresolvable scan is precisely that.
- not `package_event` — `package_id` is "THE SUBJECT. Always exactly one."
- not `discrepancy` alone — none of its six source arms could name it.

`activity_event` has `work_task_id` and `location_id` both nullable and no subject FK at all. openwcs reaches the same conclusion by construction: its per-handling-unit trace has `hu_id NOT NULL`, so a no-read cannot be filed there, and it needed a second table.

**A failed scan must be keyed on context, never on a subject.** Do not add a `package_id` that is NULL for the entire failure population — that is the always-NULL column D23 refused for typed subject FKs on `observation`.

### The volume argument, made rather than assumed

A site at 5,000 picks/day takes 2–4 identifier captures per pick task (location, item or LPN, tote, quantity confirm) — 10,000–20,000 from picking. Receiving, putaway, replenishment and counting roughly double it: **20,000–50,000 application-level captures/day/site.**

`scan_ok` at that rate:

| | rows/year/site | storage |
|---|---|---|
| `activity_event` @ ~300 B loaded (heap + 3 indexes) | 7.3M – 18M | **2.2 – 5.5 GB** |
| `client_event` companion (S19: every fact carries one) | 7.3M – 18M | **and this one cannot be partitioned** |

The second row is the one nobody costed. S19 requires every fact to carry a `client_event` FK, and D25 states plainly that `client_event` is the one table that **can never be partitioned** — partitioning reintroduces the per-partition-uniqueness bug it exists to prevent. A standalone confirmation scan is 1:1, so `scan_ok` mints its own `client_event` rows. Across twenty sites that is ~150M rows/year in a table with no retention decision (q101, still open) and no range-drop exit.

**`scan_ok` is currently adopted policy by omission.** It sits in D17's kind list, D17 justified `activity_event` on denominators, and read at face value that justification licenses the expensive branch. Delete it.

Failures only, at 0.1–2% of captures:

- **20–1,000 rows/day/site** = 7,300–365,000 rows/year/site = **2–110 MB/year/site**, partitionable, with a client_event companion of the same order. At twenty sites, under 2.5 GB/year total.

A ~100× ratio, and it is the whole decision.

**The denominator survives without `scan_ok`, and this is the non-obvious part.** `client_event` already carries `device_id`, `recorded_by_id`, `work_session_id`, `site_id` and `submitted_at`, one row per act. `COUNT(*) GROUP BY (device_id, hour_bucket)` is an exact attempts denominator over an existing table, on a cold path, at zero marginal cost. Its only bias is scans that produce no fact at all — which is exactly the population that should not be minting rows. Narrow D17's denominator claim in writing: `activity_event` supplies **labour-time** denominators (`idle`, `task_paused`, `skip`, `search_failed`); `client_event` supplies **scan-rate** denominators.

### Sketch

```
activity_event                 -- FACT (D17), amended. Range-partitioned on occurred_at.
  id
  tenant_id  NOT NULL          -- D18 / J20. Absent from D17's sketch.
  site_id    NOT NULL          -- likewise
  occurred_at, recorded_at
  client_event_id              -- PLAIN FK (D25). D17's "(unique)" is stale.
  device_id                    -- FK device (D27): the RECORDING device
  instrument_device_id         -- FK device: the scan engine, when it is a separate
                               --   fixed-mount unit. "Which reader is failing" is
                               --   unanswerable without it.
  recorded_by_id, work_session_id, authorised_by_id
  work_task_id, location_id                     -- both nullable: CONTEXT, not subject
  location_provenance          -- observed | context.  A failed scan's location is
                               --   a CLAIM (the app's belief), not an observation.
                               --   Mixing them makes a per-bin heatmap confidently
                               --   blame the last bin that scanned correctly.
  kind                         -- enum, widened below
  scanned_value    text        -- the decoded string, capped 128, truncated.
                               --   NOT `detail`.
  symbology_id                 -- FK symbology (shared reference, tenant_id NULL)
  parsed_ai        text        -- FIXED-WIDTH TEXT, never smallint: '00' keeps its
                               --   leading zero and 310n has a variable final digit
  expected_entity_kind         -- package | location | item | lot | none
  attempt_ordinal  smallint    -- within the client_event
  detail                       -- retained; nothing queryable may live here

symbology                      -- REFERENCE, tenant_id NULL (D19 shape)
  id, aim_code, datawedge_label_type, label
  -- a table, not an enum: DataWedge ships ~60 label_type values and they grow
  -- with firmware. An enum turns a vendor release into a migration.
```

Kinds added — all determinable in-process with **zero I/O**, from the GS1 mod-10 check digit and AI syntax:

| kind | meaning | who owns it |
|---|---|---|
| `identifier_malformed` | check digit or AI syntax failed | corrupted/misread/mistyped — challenge them now (D9) |
| `identifier_unknown` | well-formed GS1 key, matches nothing we hold | data or supplier problem |
| `identifier_unrecognised` | parses as no known scheme | supplier's internal barcode |
| `identifier_ambiguous` | more than one code resolved | normal at receipt: supplier SSCC + our LP |

`scan_mismatch` already covers "resolved, wrong thing" and stays the more valuable kind of the two.

**Grain is one row per operator-initiated capture attempt.** This belongs in the decision text, not the implementation: zxing-android-embedded decodes per preview frame and immediately requests the next, so a three-second aim is 45–90 failed decodes. At decode-attempt grain a 20,000-scan site generates ~1.3M rows/day. Four orders of magnitude turn on this sentence.

**Retries coalesce by `client_event_id`, server-side.** An operator scanning a smudged label six times in four seconds is one world event; six rows measure label quality × operator persistence, and a patient operator scores worse than one who gives up. Group by envelope with `attempt_ordinal`; do not coalesce on the client, where the discard has no audit and depends on app version.

**Findings are raised per pattern, not per attempt.** One `activity_event` per capture; N failures at one supplier, device or location within a window is what a human should see. Under this decision scan failures produce **aggregates only, no `discrepancy` rows** — see open question 112.

### Amends

- **D17** — `activity_event` gains `tenant_id`, `site_id`, `instrument_device_id`, the identification columns above, and four kinds. `scan_ok` is deleted. The stale `client_event_id (unique)` is corrected to a plain FK per D25. The denominator claim is narrowed to labour-time denominators.
- **D24** — `package_event.source` gains `keyed`. Today a hand-keyed SSCC at the dock must be recorded as `operator_scan`, which is a false fact under D24's own "the fact recorded is the fact observed." Manual key entry is the only reliable proxy for an unreadable label — GS1 mandates HRI on logistic labels precisely so the workaround exists — and it is the observable substitute for the no-read we cannot see.
- **D25** — `activity_event` is range-partitioned on `occurred_at` from the first migration, with local indexes. **Retention on a fact table is partition DDL by the owning role, not a DELETE grant.** D26 already established the owner/application split; this is compatible with S6 rather than an exception to it, and it should be written down before someone requests a DELETE grant or quietly stops recording.
- **D19** — `symbology` joins the shared reference set.
- **D23** — populate `observation_event.method` honestly at scan time (`scan` vs `keyed`). Do **not** mint a `capture_method` enum: `observation_event.method`, `observation_event.ingestion_channel` and `package_event.source` already answer this question and a third vocabulary will disagree with the other two.

### Rejects

- **`scan_ok`.** Refused with the number: 2.2–5.5 GB/year/site plus an equal volume in the one unpartitionable table.
- **`no_read` / `device_decode_failed` as a kind.** Structurally unpopulatable on the dominant handheld platform. If we ever want it, our app must own the decode session via soft trigger — which makes scanning depend on our process being healthy, and that is D5's central trade run backwards.
- **Per-decode-attempt grain.** Bounded by our frame rate, not by the world.
- **A `scan_stat` counter table** (openwcs's `(site, node, day, scans, no_reads, unknowns)`). Genuinely cheap and genuinely proven in two production WCS schemas — and it has **no role value** under D25's four-value axis. Not reference, not policy, not grouping, and not a projection, because a counter over scans that are not otherwise recorded has no source to rebuild from. It would be the first unrebuildable maintained table in the model and `projection_check` would have nothing to check it against. `client_event` gives the same denominator natively. Refused once, so it is not re-proposed by everyone who reads openwcs.
- **A derived supplier label-quality score from failure counts.** GS1's own verification report template disclaims the inference in both directions: "results are not absolute in that they do not necessarily prove or disprove that the barcode will scan." A grade-A label fails under shrinkwrap; a failure proves nothing about the grade. An inferred score has no author, and D21 rule 2 makes "names its author party" an access-control boundary rather than metadata. Failure aggregates by `(supplier, dock, day)` may **trigger** a verification; the verification is the assertion of record.
- **Barcode print-quality grading as a by-product of picking.** That is D27 instrument territory (calibrated verifier, declared aperture and wavelength, ISO 15416/15415). Note it is not merely out of scope: a 10-thousandths aperture is 0.254 mm and 660 nm is 0.00066 mm, and both round to **zero** under principle 5's millimetre canon. The two qualifiers GS1 says a grade is meaningless without are currently unstorable, and the failure is silent truncation. Resolve that before anyone builds it.
- **`detail` as the home for the scanned string.** "Which barcodes are failing, on which device" is the entire point of the row, so it is queryable, so principle 3 promotes it to a column. `detail` holding key=value strings would not trip S10 (it is `text`, not `jsonb`) and is the first step of exactly the leak principle 3 exists to stop.

---

## Q90 — What mints a package at receipt when the supplier sends no SSCC?

### Decision

**Nothing.** Goods arriving unlabelled land at a dock `location`: `holder_location_id` set, `holder_package_id` NULL, zero `package` rows. A package is minted on exactly three triggers, all of them *our* acts — we scanned a real label, we built the logistic unit, or putaway sent it to a location that requires a holder — and the identifier for an unlabelled pallet is an **internal licence plate, never an SSCC**.

### Reasoning — D24's minting rule contradicts D24's own default, and J19 cannot see it

D24 says a package exists "**when something identifies it** — an SSCC, a licence plate, a scan," and four lines later that "the default is never per-carton." Those collide, and they collide on the shape Australian grocery *mandates*.

The X12 856 offers two structures. SOTI puts the SSCC at the pallet — but only for single-SKU pallets. SOPI puts it at the **carton**, and it is required whenever a pallet holds more than one SKU: "Retailer requires that all cartons be labeled and the SSCC information be provided at the pack level when the Pallet contains multiple SKUs." A mixed pallet has no HL node, no identifier, no existence in EDI at all. So a SOPI ASN identifies 1,200 cartons/day at a reference site, and D24's rule as written **fires per carton** — 300,000 `package` rows and 600,000 `package_event` rows a year, generated from ~50 real observations. A 24:1 to 48:1 row-to-observation ratio.

That is the cheap half. The expensive half:

> If a `package` is minted from `asserted_unit.sscc`, and stock is received into it, then `stock.holder_package_id` — **a component of the six-dimension `stock` key** — was determined by a counterparty's message.

D21 rule 3 forbids an assertion projecting into `stock`. The test is **J19**: truncate every assertion table, rebuild `stock`, assert byte-identical. Run it against this case. `assertion`, `asserted_unit` and `asserted_unit_content` are truncated. `package` is not an assertion table and survives. `package_event` is a *fact* with `source = 'asn'` and survives. `stock_movement` survives. `stock` rebuilds byte-identical and **J19 passes** — while the shape of the stock key was authored by the supplier.

This is the J8 pattern again: the invariant tests the *values* after a truncation, and the laundering happened in the *keys*, through an intermediate table the truncation does not reach. A wrong invariant is worse than a missing one, and this one is currently the register's confidence in rule 3.

**The fix is one word.**

> A `package` row exists when something **we observe** identifies it. A counterparty's claim about a logistic unit lives in `asserted_unit` and becomes a `package` only when someone scans it or we build it.

Two further arguments that the research made and that are stronger than the cardinality one D24 used:

**Per-carton identification at receipt is physically unobservable, not merely expensive.** GS1 General Specifications 4.4.2: on a nested pallet, "only the SSCC barcode of the higher logistic unit SHOULD be readable. The SSCC barcodes of the lower level logistic units should be obscured." A receiver in front of a wrapped pallet *cannot* scan the cartons, because the standard says the labels are covered. Minting per carton would be minting packages nobody identified.

> **This paragraph was retracted on 2026-08-04.** GS1 Australia does not issue 12-digit company prefixes: seven to ten, allocated by turnover, and the smallest full-member prefix leaves six serial digits rather than four. The real figure is around 27,400 SSCCs/day sustained, not 274. The conclusion stands on the two arguments either side of it. See D29 in [domain-model.md](./domain-model.md), which supersedes this document, and [gs1-grocery-analysis.md](./gs1-grocery-analysis.md) for the primary sources.

**Per-carton SSCC minting is arithmetically impossible for a small tenant.** An SSCC is exactly 18 digits: extension digit + GS1 Company Prefix + serial reference + check digit. GCP and serial share 16 digits. A 12-digit GCP — what a small AU company is issued — leaves **four** serial digits: 10,000 per extension digit, 100,000 total. GS1's 1-year non-reallocation rule turns that into a sustained ceiling of **274 SSCCs/day, forever**. Carton grain at 1,200/day exhausts a small tenant's lifetime namespace in four months and breaches the reuse rule from day one. Pallet grain is ~50/day, 12,500/year, comfortable.

That kills per-carton on the standard, before labour (~17 min/day at pallet grain vs ~2.3 h/day, roughly 0.3 FTE, at carton grain) and long before the database notices. **Do not argue this on storage grounds.**

### What actually mints, and with what

**Three triggers, all ours.**

1. **We scanned a real label.** A supplier SSCC read off the pallet at the dock. GS1 4.4.1.2 read the other way round.
2. **We built the logistic unit.** GS1 4.4.1.2: "the physical builder of the logistic unit or the brand owner is responsible for the allocation of the SSCC." Re-palletising, consolidating loose cartons, rebuilding a broken pallet — a deliberate operator act. This is also the only safe way to mint from an unparseable scan, i.e. not from one at all.
3. **Putaway to a location that requires a holder.** D365's per-location-profile `Use license plate tracking` is the published precedent, and it is the right carrier: the licence plate is a property of *where the goods land*, not of the goods or the receipt. The dock needs no package; a bulk rack gets one at putaway, which is the moment the pallet first becomes a thing anyone must address later.

**The identifier is an internal LP, not an SSCC.** Every property that makes an SSCC expensive — a licensed prefix, a finite serial budget, a 12-month obligation, an implicit assertion of authorship, a fee schedule — exists to make it meaningful to *other parties*. A pallet broken down into putaway locations before anything leaves the site is never seen by another party. It pays every cost and gets no benefit. An SSCC is minted at the boundary: despatch, or re-palletisation into something that will ship.

The LP format must be **mechanically distinguishable from an SSCC in one regex** — alpha-prefixed, not an 18-digit numeric. An internal 18-digit plate with a coincidentally valid check digit will eventually be transmitted on a DESADV, and that class of error is undetectable afterwards.

### The missing machinery, and where it is scoped

Nothing in D1–D27 issues an identifier. `package.sscc` (D6) has no issuer; `party` (D20) is `(id, tenant_id, kind, name, abn, active)` with no company prefix; there is no `number_range`. That is not cosmetic: without a package, a subsequent pallet move is not the O(1) `package_event` D24 promises — it is N `stock_movement` rows, one per SKU, each asserting an inspection nobody performed. **D24's fan-out guarantee is false for rungs 0–2 of the degradation ladder, which is the common case.**

```
number_range              -- REFERENCE. OPERATOR-OWNED (tenant_id IS NULL).
  id
  issuer_party_id         -- the LEGAL ENTITY holding the GCP, not the tenant.
                          --   A GCP is licensed to a legal entity; D20 already
                          --   has site.legal_entity_id and several under one tenant.
  key                     -- 'sscc' | 'internal_lp'
  extension_digit         -- explicit row per digit; NO automatic rollover on
                          --   exhaustion (it changes the first character of every
                          --   SSCC we issue, and downstream systems pattern-match it)
  next_value  bigint
  block_size  int         -- claimed under FOR UPDATE, handed out from process memory.
                          --   Per-serial row locking is a convoy on the receipt path.
  issued_through bigint   -- high-water mark: serials consumed but never applied are
                          --   still evidenced against the 12-month reuse window
  exhausted_at            -- exhaustion raises a finding and FALLS BACK TO INTERNAL
                          --   LPs. It never blocks a print. Same code path as
                          --   "tenant has no GCP" — one fallback, exercised daily.

sscc_allocation           -- FACT. Append-only; no UPDATE, no DELETE (S6).
  id, tenant_id, client_event_id
  issuer_party_id, extension_digit, gcp, gcp_length, serial_reference
  sscc  CHAR(18) GENERATED      -- CHAR, never bigint: leading zeros are significant
  issued_at
  UNIQUE (issuer_party_id, extension_digit, serial_reference)
  -- NO package_id. The binding is package_event(kind='identified', sscc=…).
  --   A serial issued and never applied is an anti-join, not a NULL to UPDATE —
  --   and a fact table has no UPDATE grant to do it with.

party                     -- amended (D20). Intrinsic → shared row (D19).
  gln
  gs1_company_prefix      -- NULLABLE. Absence is a first-class state (D20):
                          --   an IBN member has no prefix and cannot form an SSCC
                          --   at all, while its retailer still mandates SSCC labels.
  gs1_prefix_length       -- IMMUTABLE once any allocation exists under it:
                          --   changing it silently re-partitions every past serial
  CHECK (gs1_company_prefix IS NULL
         OR (gs1_company_prefix ~ '^[0-9]{4,12}$'
             AND gs1_company_prefix !~ '^0+$'))
  -- No system-default prefix, ever. metasfresh ships '0000000' and an unconfigured
  -- install cheerfully generates check-digit-valid SSCCs in a namespace nobody
  -- owns — the same ones as every other unconfigured install.

package_identifier        -- @projection of package_event. NOT a writable table.
  tenant_id, package_id
  kind                    -- sscc | internal_lp | supplier_reference | asn_case
                          -- | carrier_tracking | giai | grai
  value, first_seen_at, superseded_at
  UNIQUE (tenant_id, kind, value) WHERE superseded_at IS NULL
  INDEX (tenant_id, value)      -- the scan-resolution path, ~15–20k hits/day/site.
                                --   NON-unique: see rejects.
```

**`package.sscc` is a live pre-D24 defect and must be fixed here.** D24 demoted `parent_package_id`, `location_id`, `resolved_location_id`, `status` and `depth` to projections of `package_event` — and said nothing about `sscc` and `barcode`. Placement was fixed; identity was left mutable. Worse, **J6's enumerated fold is `parent/location/resolved_location/status/depth`**, so a relabelled pallet whose `package.sscc` was never updated **passes** the invariant that exists to catch exactly that drift. Same shape as J8. Demote both, revoke UPDATE, extend J6.

The alias problem is real — the same pallet answers to the supplier's SSCC, our LP, the ASN case reference and a carrier tracking number. Foxfire CIMS solves it with five identifier columns on one row, which principle 4 refuses. `package_identifier` as a **projection of `package_event`** (kinds `identified` and `relabelled`) solves it with no new writable state. A directly-written table with `first_seen_at`/`superseded_at` would be mutable state with no fact behind it — the pre-D24 defect a third time.

**Ordering rule, free now and expensive later: match, then relabel.** The supplier's SSCC binds to our package via `package_event(kind='identified', source='asn')` written at match time. Relabel first and the supplier's SSCC is recoverable only from the operator's memory, and a recall or claim quoting it returns nothing.

**Minting granularity is a `putaway_policy` scalar, not a twelfth kind.** `requires_package_holder`, resolved at zone grain. D24 (supply side) already set the precedent by folding cross-dock scalars into `allocation_policy` rather than minting kinds, and `direction` is not one of D22's six dimensions — a seventh axis would change every existing depth vector.

### Volume

| policy | `package` rows/yr/site | `package_event` rows/yr/site | SSCCs/yr | `stock` effect |
|---|---|---|---|---|
| **location-held (default)** | 0 | 0 | 0 | — |
| per-pallet, internal LP | ~36k | ~73k (<20 MB) | 0 | ~2–3× on dock cells (splits a commingled cell per pallet) |
| per-pallet, SSCC | ~36k | ~73k | 36k | same |
| per-carton | ~1.1M | ~2.2M (~1 GB) | 1.1M — **exhausts a GCP-10 in 12 months** | **40× on a homogeneous pallet, on the availability path** |

The 40× is the number that matters. D24 (supply side) reduced availability to two index-only range scans; a carton-grain multiplier degrades that fold silently, for the tenants who chose it, and the failure mode is silence.

### Amends

- **D24** — the minting rule restated ("something **we observe** identifies it"); three triggers named; default is location-held; `package.sscc` and `package.barcode` demoted to `@projection`.
- **D6** — `package.sscc` as a writable column retired.
- **D20** — `party` gains `gln`, `gs1_company_prefix`, `gs1_prefix_length` as intrinsic shared attributes. `can_issue_sscc` is derived from the prefix being present, never a stored boolean.
- **D21** — J19 widened (below). `asserted_unit` gains **no** `package_id`; the binding is a `package_event`.
- **D22** — `putaway_policy.requires_package_holder`.
- **D25 / q102** — the `number_range` increment runs under the elevated maintainer role, with `FORCE ROW LEVEL SECURITY` and its own audit. In a self-hosted single-tenant deployment there is no operator and no collision to guard against; state that rather than leaving the shared row looking like a hole in D18.

### Rejects

- **Minting from an ASN.** The J19 defect above is the reason, recorded so it is refused once.
- **Per-carton minting at receipt.** Refused on the GCP arithmetic and on GS1 4.4.2's obscured child labels, not on storage.
- **A locally-minted 18-digit "internal SSCC," or a restricted-circulation SSCC range.** No such range exists — restricted circulation is a GTIN/POS mechanism. A local 18-digit numeric is indistinguishable from a real SSCC at the scanner and will escape onto a DESADV.
- **`UNIQUE (tenant_id, sscc)` on `package`.** GS1's rule is a 12-month window, not permanence; suppliers ignore it; and D5 forbids rejecting a true observation. D21's own `assertion` DDL already carries the identical comment. D365 hard-blocks receiving when an ASN's LP has on-hand elsewhere and had to ship a `Transit warehouse license plate policy` parameter to escape its own refusal — cite it as the concrete thing we are choosing not to do. A duplicate resolves to the existing package and the placement register's compare-and-set raises `containment_conflict`. Note the consequence honestly: two genuinely different physical pallets under one reused SSCC collapse into one cell. Under D5 that is correct — we record what the identifier says.
- **A global `UNIQUE (sscc)` across tenants.** Same D5 problem, plus a cross-tenant existence oracle of exactly the kind D25 closed by keying `client_event` on `(tenant_id, …)`.
- **Pre-minting `package` rows at label-print time.** CIMS's `Visible` computed column (a label-type LPN with no SKU and zero quantity is *hidden, not deleted*) and its `Archived` flag with two dedicated indexes are the evidence of what that costs: ~20k dead rows/year/site whose count is bounded by our label purchasing, not by the world. Consume the serial; let the first scan create the package.
- **A bespoke `package_minting_policy` table** with its own scope columns — the fifth parallel policy chain D22 exists to prevent.
- **GRAI for CHEP/Loscam.** Pallet pooling is fungible; serialising bases is a much heavier commitment D14 already declined.

---

## Q91 — When does the reaper run, and what is "unreferenced"?

### Decision

**"Unreferenced" is a closed, CI-asserted list of exactly one FK: `stock_allocation.stock_id`, in every state — not the enumerated live set.** `stock.id` is a handle for the life of the cell, not an archival key: no fact carries it, no report joins it, and nothing else in the schema may hold it. The reaper runs weekly, off-peak, under the projection-maintainer role, with a grace period, batched, with a predicate-repeating `DELETE` and a kill switch — and **its predicate is part of the rebuild's own definition of which cells exist.**

### Reasoning — the obvious answer is wrong twice

**First: the reaper as adopted is a near no-op, and the reasoning it rests on is false.** `stock_allocation.state` is `allocated | picking | picked | packed | fulfilled | short | released`, and nothing purges terminal rows. containment-review justified reapability with "an allocation against a zero cell should not exist anyway." A `fulfilled` allocation against a now-zero cell **is the normal end of every pick.** Under a literal read of "not deleted while referenced," every cell that ever served a pick is pinned forever and the only reapable cells are receipt cells consumed by a whole-carton move.

**Second, and this one would ship as a bug: J3 is not a reference test.** `allocated_quantity` folds only `{allocated, picking, picked, packed}`. Terminal rows contribute nothing to it and still hold the FK. A reaper written as `quantity = 0 AND allocated_quantity = 0` — the only cheap local test, and the one J3 makes look authoritative — deletes rows that live foreign keys point at. Write it as an `EXISTS` over `stock_allocation` in **any** state, and say in D24 that J3 must never be used as a reference test.

And there is no referential action that both reaps and keeps the allocation. RESTRICT/NO ACTION block; CASCADE destroys fulfilment history; SET NULL violates `CHECK (num_nonnulls(stock_id, expected_supply_id) = 1)` and aborts at statement end. Relaxing that CHECK to `<= 1` is refused twice — by D23's discriminated-union rule (an allocation bound to neither a cell nor an expected-supply row names no supply and is meaningless) and by D24 (supply side) Amendment 2, which explicitly dropped that scoping three weeks ago.

So: RESTRICT, declared, with the reaper narrowed to match.

### What "unreferenced" means, and the referencers nobody named

**The model has already answered this three times without writing it down.** `stock_count` and `discrepancy` carry the full cell key column set rather than an FK to `stock.id` — mechanism-design's stated reason is that counting a bin which turns out to hold stock is exactly the case where no cell row exists. `observable` (D23) deliberately excludes stock cells, and says why: "D24 gives `stock` a surrogate id that would make it tempting." `stock_movement` carries `from_*`/`to_*` column pairs, never a `stock_id`.

That is the rule. Write it:

> **`stock.id` is a current-state handle, not an archival key. History is `stock_movement`.**

That collapses q91's own "historical reporting that joins `stock.id`" into a rule the model already obeys — and it is why `outbox.source_id` is safe: S27 ("never dereferenced; the rendered bytes are the payload") is doing load-bearing work for the reaper that nobody wrote it for.

**Three referencers the register does not cover:**

1. **D26's schema compiler.** `record_scheme_field.field_type = 'ref'` generates **a real FK with `ON DELETE RESTRICT`**, and D26 Amendment 1 derives `attaches_to` from the code-side table registry rather than a hand-maintained list. `stock` is in that registry. So **a tenant can declare a scheme that creates a durable FK to `stock.id` at runtime**, and the reaper starts failing on rows it used to reap — silently, per tenant, as a maintenance job erroring rather than a design decision. A tenant-authored declaration disabling an operator invariant, with no privilege required. Mark `stock` non-attachable and non-subscribable.
2. **`package_content` is a VIEW that exposes `stock.id` as `package_content.id`.** Any export, report or `ref_entity` naming it persists a reapable surrogate. Same flag.
3. **`projection_check.scope_kind`/`scope_id`** (D25) is a polymorphic pair retained as a fact with no DELETE grant. Scope a check to a cell and it holds a `stock_id` forever, uncatchable by any FK — D10's argument 1 verbatim. Forbid it naming a cell.

**S2 is the invariant that licenses the bug.** "Every table naming a stock cell carries the whole key — **FK to `stock.id` or** the complete column set." Under a reapable `stock` those two are not equivalent: the column set survives the row's death and the FK does not. Remove the disjunction.

### The rebuild collision, which is the sharpest practical point

J1 asserts `stock.quantity` equals the two-sided fold of `stock_movement` over the cell key. A reaped cell folds to zero and has no row. Unless the rebuild's definition of *which cells exist* excludes them, **every reap cycle emits `projection_drift`** and D8's queue fills with noise the model generates about itself. J2, J3 and J7 have the same exposure.

State the predicate once, in the rebuild:

> A cell exists iff `quantity <> 0 OR weight_g <> 0 OR allocated_quantity <> 0 OR referenced`.

`weight_g` is easy to miss and matters: J2 folds `catch_weight_g` independently, so a cell can reach `quantity = 0` with `weight_g <> 0`. That is a catch-weight capture bug, and it is exactly the evidence a quantity-only reaper destroys while the rebuild resurrects the row.

**And `stock` needs J30's analogue, which it does not have.** D24 (supply side) forbade truncate-and-regenerate for `expected_supply` *because live allocations hold those ids*. `stock_allocation` holds `stock_id` under the same CHECK on the same table in the same decision, and `stock` got nothing. A rebuild today re-mints every id and orphans every live allocation, while J19's word "byte-identical" quietly assumes it does not.

### When it runs — and the honest benefit

Weekly, off-peak, batched, killable. The honest reason for "rarely" is that **the benefit is small and the reaper is not the fix for `stock`'s churn.**

- The availability index is already partial (`WHERE quantity <> 0`), so dead cells are already invisible to the read path. The reaper adds nothing there.
- The `UNIQUE NULLS NOT DISTINCT` arbiter index cannot be partial — it must find a zero cell to resurrect it — so it carries every cell that ever existed. Reaping trims about **one B-tree level**: roughly one page access per upsert probe.
- At 5,000 picks/day with ~2,000 cell-deaths/day against ~100k live cells, twelve months unreaped is ~730k dead rows, ~88% of the table, ~330 MB/year/site. Genuinely nothing in absolute terms. The ratio is the argument, not the megabytes.
- The real cost on `stock` is non-HOT UPDATE churn, and reaping does not touch it. `available_quantity` is `GENERATED STORED` and sits in the availability index's `INCLUDE`, so **every quantity change and every allocation state transition writes new index tuples**. D24 (supply side) already named `stock_allocation.state` the model's hottest projection trigger. Restate D24 amendment 3's benefit accurately: it bounds the arbiter index's page count. It does not "remove the unbounded-growth concern."

Mechanics that must be in the decision because the natural implementation gets them wrong:

- **The `DELETE` repeats the full predicate.** Under READ COMMITTED, `DELETE ... WHERE id = ANY($1)` deletes a cell resurrected between the SELECT and the DELETE. Batch by id **and** predicate.
- **`stock_allocation(stock_id)` plain btree must exist before the reaper does.** Postgres does not index the referencing side of an FK; without it each deleted row fires an RI trigger that sequentially scans 1.8M allocation rows.
- **Grace period.** Add `last_movement_at` to `stock` as an `@projection` — it is not in the adopted sketch — with a candidate index `(tenant_id, last_movement_at) WHERE quantity = 0 AND allocated_quantity = 0`. A cycle-count wave revisits a bin a week later; measure grace in days.
- **Per-table autovacuum.** `autovacuum_vacuum_scale_factor = 0.01` in the migration that creates `stock`, justified by UPDATE churn rather than by the reaper. A self-hosted BUSL deployment (D18) has no DBA to set it.
- The reap-vs-resurrect race is **not** a problem: `ON CONFLICT DO UPDATE` guarantees insert-or-update even against a concurrent delete. The reaper cannot make a receipt fail.

### Amends

- **D24 amendment 3** — premise corrected; predicate stated as an `EXISTS` over `stock_allocation` in every state; benefit restated as bounding the arbiter index.
- **D12 / D24** — `stock_allocation.stock_id` declared `ON DELETE RESTRICT`, with a plain btree on `(stock_id)`.
- **D24** — `stock` gains `last_movement_at` (`@projection`). `stock.id`'s comment changed from "surrogate; stable" to *"handle for the life of the cell. NOT durable: reissued if the cell is reaped and returns."*
- **D25** — DELETE revoked on **projection** tables from the app role. S5 covers UPDATE only and S6 covers fact tables only, so "nothing writes to `stock` directly, ever" is currently unenforced against DELETE.
- **D26** — `stock` and `package_content` carry neither the attachable nor the subscribable capability flag.
- **D25** — `projection_check.scope_kind` may not name a stock cell.
- **q102** — the reaper runs as the projection maintainer, so q102 blocks it.

### Rejects

- **Reaping on `allocated_quantity = 0`.**
- **Relaxing `stock_allocation`'s `= 1` CHECK** to permit SET NULL, and **CASCADE**.
- **Loose foreign keys** (GitLab's `loose_foreign_keys_deleted_records` + DELETE trigger + worker with retry and a 30-second budget). Well-evidenced, and it is a queue table, a trigger on the hottest projection and a worker to remove one FK — while deleting the guard mechanism-design adopted deliberately ("the FK is what stops a caller naming a cell that has never existed"). Revisit only if a second durable consumer is ever admitted, which S28 should make impossible.
- **A `stock_reaped` tombstone for extractors.** 30–180 MB/year/site to record having deleted 30–180 MB/year/site, recreating the unbounded table the reap exists to prevent. Extractors get the natural key or the `stock_by_item_site` roll-up, never the cell surrogate. (Fivetran soft-deletes by default and cannot even see a hard delete without a change log; dbt's hard-delete tracking is opt-in and mints its own `dbt_scd_id`. Both are wrong and neither errors — which is why the surrogate must not leave the database.)
- **`deleted_at` soft delete** — not a reap.
- **Truncate-and-regenerate**, and OpenBoxes-style scoped delete-and-rebuild.
- **Never-reap plus periodic `REINDEX CONCURRENTLY`.** Attractive on the numbers, refused on D5: a concurrent reindex of a unique index can make `INSERT ... ON CONFLICT` fail with a spurious unique violation — on `stock`'s arbiter index that is a receipt scan being rejected. That also makes any concurrent index build touching that index a floor-affecting migration, which should be a stated rule.
- **Deterministic `stock.id` (UUIDv5 over the key).** Considered seriously — it makes reap-then-resurrect recreate the identical row and would collapse this whole question. Unnecessary once nothing durable holds the id, and it puts a random-key B-tree on the hottest table in the system. Refused with the reason.
- **A `reaper_run` table or an `activity_event` kind for it.** `activity_event` exists for human productivity; a maintenance job in it poisons every metric it serves. A log line, unless someone names the question.

---

## Cross-cutting

**One rule underlies all three.** *A durable holder takes the natural key; a surrogate on a projection is a handle with the lifetime of its row.* Q89: a failed scan has no subject and must be keyed on context, never on a subject FK that is NULL for the whole population. Q90: `package.sscc` is a second independently-writable representation of an identity that belongs in the event log, and `package_identifier` must be a projection rather than a fifth table. Q91: `stock.id` is not archival. The model already obeys this rule in `stock_count`, `discrepancy`, `stock_movement` and `observable` — it has just never been stated, which is why S2 licenses its violation.

**Three invariants were found encoding their own bug**, all of the J8 shape. J19 passes while an ASN authors a `stock` key column (Q90). J6's enumerated fold omits `sscc`/`barcode`, so relabelling drift passes the check written to catch it (Q90). J3 is a quantity fold that reads as a reference test (Q91). And S2's disjunction is a fourth. The register's stated first lesson — *a wrong invariant is worse than a missing one* — earns three more entries.

**All three answers land in `client_event`, and none of them costed it.** `scan_ok` would put 7–18M unpartitionable rows/year/site there; migration imports put 30–60k per tenant on day one; Q90's minting acts add their own. q101 is open, `client_event` has no retention window, and it is the only table with no range-drop exit. **It is now the binding constraint on Q89 and should be answered in the same change.**

**Q89 and Q90 both increase traffic through `containment_conflict`, which has no `discrepancy` source arm.** The arms are capped at six and all six are causes; D24 (supply side) spent the sixth on `expected_supply_id` and noted it was already "a subject standing in for an absent cause." A scan-derived finding and a duplicate-identifier finding each want a seventh. That is the second and third subject-standing-in-for-a-cause in a row, at which point the honest reading is that `discrepancy` has become a subject union and the cap has stopped doing work. Under these decisions no scan failure raises a finding — the question is deferred, not answered.

**Retention floors are a class the register cannot check.** Q90's SSCC reuse guard needs 12 months of identifier history; q76 proposes partitioning `package_event`. Drop a partition and the duplicate check *silently starts passing*. Note the asymmetry: fold invariants (J1, J2, J6, J26) detect source deletion, because the projection stops matching. An **existence predicate** has no projection to compare against, so after a truncation both sides agree and nothing fails. Retention floors must be declared and asserted separately from the projection register — Q89's partitioning plan and Q90's reuse guard are the first two instances.

**One live defect adjacent to all three, worth fixing before scan-mismatch confidence is built on it.** D23 promoted `challenged`/`challenge_context`/`confirmed` to `observation_event` and stated "`stock_count` loses its copies." D23's own boundary rule then says a stock cell is deliberately **not** an `observable` — "quantities of cells are `stock_count`." So a bin count has no `observation_event` to carry the challenge, and D9's founding worked example (*"this location has not moved since the last count of 50, and you have entered 47"*) is currently unrecordable. An amendment consolidated a mechanism onto a table whose own boundary rule excludes the original subject.

---

## Invariants created

> **The invariants below have moved to [invariants.md](./invariants.md)**, the single register and the canonical numbering. The entries here are retained as written; the register is authoritative on text, numbering and status.


| # | Invariant | Owner | Check |
|---|---|---|---|
| **S2** *(corrected)* | Every table naming a stock cell carries **the complete column set**. The "or FK to `stock.id`" disjunction is removed — under a reapable `stock` the two are not equivalent | D24, Q91 | Catalogue diff over a declared list |
| **S28** | Exactly one foreign key in the schema targets `stock(id)`, and it is `stock_allocation.stock_id`, declared `ON DELETE RESTRICT`. `stock_allocation(stock_id)` has a plain btree | D24, Q91 | `pg_constraint` ⋈ declared list; catalogue scan |
| **S29** | `stock` and `package_content` carry neither the attachable nor the subscribable capability flag in the code-side table registry; `projection_check.scope_kind` may not name a stock cell | D26, D25, Q91 | Registry check + enumerated denylist |
| **S30** | Projection tables have no **DELETE** granted to the app role (S5 covers UPDATE; S6 covers fact tables) | D25, Q91 | `role_table_grants`; attempt, expect failure |
| **S31** | Every identification datum on `activity_event` is a typed column. `activity_event.detail` appears in no `WHERE`, `GROUP BY` or `JOIN` in the query register | Principle 3, Q89 | Query-register grep |
| **S32** | `activity_event` is range-partitioned on `occurred_at` with local indexes from the first migration, and carries `tenant_id NOT NULL` and `site_id` | D18/J20, D25, Q89 | Catalogue scan |
| **S33** | No `party.gs1_company_prefix` is all-zero or absent-yet-used; no literal default prefix appears in code; SSCC issuance is gated on an explicit `number_range` row | D20, Q90 | CHECK + grep |
| **J3** *(annotated)* | `stock.allocated_quantity` enumerates `{allocated, picking, picked, packed}`. **It is a quantity fold and must never be used as a reference test** — terminal allocations hold `stock_id` and contribute nothing to it | D12, Q91 | Stated; enforced by S28's predicate |
| **J6** *(extended)* | The `package_event` fold covers `parent`, `location`, `resolved_location`, `status`, `depth`, **`sscc`, `barcode` and `identifier_kind`** | D24, Q90 | Rebuild-and-assert |
| **J19** *(widened)* | Truncate every assertion table, rebuild `stock`, assert byte-identical — **and** assert that no column of the `stock` key is reachable from an assertion table by any path that survives truncation | D21 rule 3, Q90 | Truncate-rebuild + path analysis |
| **J32** | The reap predicate is the complement of the rebuild's existence predicate: a reap-then-rebuild-then-assert cycle produces zero `projection_drift` | D24, D25, Q91 | Suite: reap, rebuild, assert |
| **J33** | Rebuilding `stock` preserves row identity: every live cell resolves to the same `id` before and after. Truncate-and-regenerate is forbidden while any allocation holds a `stock_id`. *(J30's missing analogue.)* | D24, Q91 | Rebuild-and-diff |
| **J34** | No `package` row's earliest `package_event` has `source = 'asn'`. Minting from an assertion is structurally absent | D21, D24, Q90 | Anti-join |
| **J35** | Every `sscc_allocation` serial lies within its `number_range`'s issued span, and no serial is reissued within the reuse window (12 months + any sector extension) | D21, Q90 | Anti-join, with a declared retention floor |

---

## Open questions

> **Questions raised in this document have moved to [open-questions.md](./open-questions.md)**, the single register and the canonical numbering. The entries below are retained as written; the register is authoritative on status.


**112. The seventh `discrepancy` source arm — or the admission that `discrepancy` is a subject union.** `containment_conflict` already has no reachable source; Q89's failure aggregates and Q90's duplicate-identifier case each want one. Two of the six arms are now subjects standing in for absent causes. Either take the seventh as a recorded decision, or re-derive the cap honestly.

**113. q101, promoted.** `client_event`'s retention window is now the binding constraint on Q89 and takes 30–60k rows per migrating tenant on day one. It cannot be partitioned. Answer it with q76's partitioning plan and with the retention-floor class below, not separately.

**114. Retention floors are a distinct guarantee class.** An existence predicate has no projection to compare against, so no fold invariant can detect a breached floor. Q90's SSCC reuse window is the first instance. Where do floors get declared, and what asserts them? This collides with q70 (statutory retention vs tenant deletion).

**115. D9's challenge mechanism has no home for its founding example.** Restore `challenged`/`challenge_context`/`confirmed` to `stock_count`, or state where a bin count's challenge lives. Anything building scan-mismatch or count confidence on D9 inherits the hole.

**116. `item_barcode` is referenced twice by D19 and defined by no adopted decision.** Q89's `symbology_id` records *why* a scan failed; without `item_barcode` nothing can say what the operator should have scanned, which is where most of the diagnostic value is.

**117. q102 for the projection maintainer blocks the reaper.** The reaper needs a DELETE grant on a projection under a `SECURITY DEFINER` role with `FORCE ROW LEVEL SECURITY`. That is a wider hole than the UPDATE grant q102 was raised about.

**118. Where does the reaper's kill switch live, and who may flip it?** It is an operational knob, not a business decision — explicitly **not** a twelfth D22 policy kind. The same applies to the grace interval. Say so, or the first one sets the precedent that ends with vacuum thresholds on the scope lattice.

**119. Is the internal LP format shipped or tenant-configurable?** It must be regex-distinguishable from an SSCC in one pass, in front of the GS1 parser. A tenant-chosen format that happens to be 18 numeric digits is a defect we cannot detect afterwards.

**120. `activity_event.kind` is now large enough to re-open q44.** Fourteen-plus values across labour, task lifecycle and identification. Enum is still right (code branches, D23 says so by name), but a tenant wanting "waiting for CHEP pallets" needs our deploy — the D13 product tension arriving again, with `record_scheme` as the escape.
