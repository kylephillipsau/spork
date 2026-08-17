# Handoff: what changed under you, and what to do next

Written after a review of the 11–12 August application work, four repair commits
on `review/wp1-wp2-repairs`. Read this before touching `crates/server`, because
three signatures moved and one habit needs to stop.

---

## First, run this

```sh
git checkout main && git merge --ff-only review/wp1-wp2-repairs
DATABASE_URL=postgres://postgres:nylonite@localhost:55432/nylonite \
  PGPASSWORD=nylonite scripts/verify-migrations.sh
docker compose up -d --build
```

The dev database has two days of development writes committed into it — a stray
160-unit receipt movement against the fixture's `expected_supply`, and an
orphaned `client_event` — which fail J58, the correction-chain test and the
inbound walk for reasons that have nothing to do with the code. The reload above
clears them. **Everything below was verified on a database built from nothing;
none of it was verified against the instance you have been working on**, and
that difference is the single most useful thing in this note.

---

## The habit that has to stop

**Do not run the suites against the compose database.** Build one:

```sh
createdb nylonite_x
DATABASE_URL=postgres://.../nylonite_x PGPASSWORD=nylonite scripts/verify-migrations.sh
DATABASE_URL=postgres://.../nylonite_x cargo test --workspace
dropdb nylonite_x
```

Three of the five failures found in review came from this, in both directions.
Two tests leaked committed state into the shared database and broke unrelated
checks; one test *depended* on state only that database had, so it passed
locally and could never have passed anywhere else. `verify-migrations.sh` builds
a schema from empty, loads both fixtures, reverses it, and leaves a clean seeded
database behind — it has existed for weeks and nothing was calling it.

CI now calls it (`.github/workflows/ci.yml`) and runs `clippy -D warnings`
first, so a warning is a build failure from here on.

Two corollaries, both of which bit:

- **Never hard-code an id the fixture generates.** `seed.sql` folds the issued
  order into `expected_supply` rather than inserting it, so that row takes
  `uuidv7()` and has a different id on every load. Reach it the way `seed.sql`
  does — `WHERE purchase_order_line_id = '901e0000-…-001'`. Hand-written
  fixture ids (`901e…`, `17e1…`, `a517…`) are stable; anything that looks like
  a v7 is not.
- **Never commit setup rows on a fixed id.** If a test must commit outside its
  transaction, mint the ids with `Uuid::now_v7()` so a leak is an orphan rather
  than a permanent collision with another test file.

---

## Signatures that moved

Three, all in the receipt path. Fix these before writing anything new.

```rust
// crates/server/src/receiving.rs
- resolve_receiving_policy(..) -> Result<(ReceivingPolicy, Vec<String>), ApiError>
+ resolve_receiving_policy(..) -> Result<ResolvedReceiving, ApiError>
//   ResolvedReceiving { policy: ReceivingPolicy, ambiguity: Option<String> }

- ReceivingPolicy { tolerance_over_pct: f64, .. }
+ ReceivingPolicy { tolerance_over_pct: Option<f64>, .. }   // None = no bound stated

// crates/server/src/client_events.rs
- require_receipt_facts(..) -> (Uuid, Uuid, Option<Uuid>, Option<Uuid>, bool)
+ require_receipt_facts(..) -> (Uuid, Uuid, Option<Uuid>, Option<i64>, Option<Uuid>, bool)
//   the movement's quantity comes back with the movement; stop re-querying it
```

`tolerance_over_pct` being `Option` is the one with meaning behind it. The column
is nullable, so a version stating no over-delivery bound is a configuration
somebody can save, and `disposition` reads the absence rather than reading it as
zero — a policy that set no bound cannot say an overage was inside one, so the
overage goes to the queue. This is `packing_factor`'s precedent: *a made-up
number is a wrong answer rather than a missing one.* Do not reintroduce a
default for a policy field. `require_lot` now **refuses** if absent, because the
plausible default is `false` and the permissive one.

---

## Three new migrations

| # | What | Why |
|---|---|---|
| 64 | Drops `stock_count_stock_fk` | S28 permits one FK onto `stock(id)`; migration 63 added a second. `stock` is reapable, so the constraint would have wedged the first rebuild that tried to collect a counted cell. `stock_id` stays as a snapshot — S2's six key columns identify the cell and survive a reap |
| 65 | Ships the platform `receiving` binding | It lived only in `seed.sql`, so a schema-only deployment could not receive goods. **Moved, not copied** — a second all-NULL platform binding of one kind ties on every dimension and raises `policy_ambiguous` on every receipt |
| 66 | `discrepancy.goods_receipt_line_id` | Fourth source arm. The replay path was recovering the finding with `LIKE '%<line id>%'` over `detail`, which S31 forbids and which returns the wrong row once two findings mention one line |

**If you add a migration, `S28` and `S2` are worth re-reading first.** The
pattern that broke was a table naming a stock cell and reaching for the FK as
well as the key columns. Under a reapable `stock` those are not equivalent, and
only `stock_allocation` may hold the key.

---

## Behaviour that changed in `POST /receipts`

1. **`claim_act` runs before the policy resolve.** The resolution is discarded on
   a replay, but its failure paths were not, so a retired binding could turn an
   idempotent retry into a 400. Keep this order in any endpoint you add: claim
   the act, return early on replay, then resolve.
2. **An equal-specificity tie writes a `policy_ambiguous` discrepancy** naming
   the line, instead of a warning string that vanished with the response. Use
   `nylonite_policy::explain()` for the text — it exists under question 79 so the
   explanation ships with the ordering it describes. **Any endpoint you add that
   resolves a policy owes the same row.** D22 raises the tie; J16 counts it.
3. **The version lookup uses `query_opt`**, so a database error stays a 500
   instead of becoming a 400 telling the receiver their policy is misconfigured.
   `map_err(|_| Rejected(..))` over a query is a bug — it swallows the real
   error class.

---

## Where to pick up

Four questions were filed in `docs/open-questions.md`. Two of them are the
natural next endpoint work and are **deliberately not implemented**, because
both change the shape of the API rather than repair it and the receiving screen's
requirements sit underneath:

- **172 — whether one delivery is one `goods_receipt`.** D43 decided one receipt
  per delivery per demand document; `POST /receipts` mints a fresh header per
  line and takes no `goods_receipt_id`, so a twenty-line truck produces twenty
  receipts. Fixing it is a header the caller names and a line that joins it, and
  it is what lets `require_receipt_facts` say *which* line an act wrote. **This
  is the one I would do first** — everything about the receiving screen wants it.
- **173 — how a receipt says cartons.** The line writes `entered_quantity =
  quantity` and level `'each'` with no `item_packing_config_id`, so the API can
  only express base units and the entered form it preserves is a copy of the
  canonical one. Migrations 46 and 47 built those columns for exactly the case it
  cannot express. Note that J65 and J57 both *pass* on this — the guards are
  satisfied by a row that discards what they were built to protect, which is why
  it needs a question rather than a check.
- **174 — whether an allocation is an act.** `POST /allocations` is the only
  write path with no `client_event` envelope, so a handheld retry after a timeout
  commits the cell twice. S19 does not force it, because an allocation is an
  Intention rather than a Fact — that is the question, not the answer.
- **171 — who the caller is.** The tenant arrives in an unauthenticated
  `x-tenant-id` header and the actor in `recorded_by_id` in the body. Not a
  next-endpoint problem, but it is now on the register, which it was not before.

## One thing left for Kyle to decide

Migration 65 ships the platform default with `require_lot = true`, inherited
verbatim from the fixture row it replaced. Shipping `false` was discussed and
deliberately not done: `fixtures/resolver-golden.txt` records the resolution
`clamped require_lot:0->1`, and changing it needs a `RESOLVER_VERSION` bump,
whose entire purpose under J22 is to make somebody read the diff. **Do not flip
it as a side effect of other work** — it is its own commit with a regenerated
golden.

---

## The state you are inheriting

66 migrations up and down five ways. 45 structural invariants passing of 56
(2 vacuous, 9 pending), 52 job-asserted of 70 (3 vacuous, 15 pending), 135 tests,
zero failures, clippy clean under `-D warnings`. `docs/architecture.md` and
`docs/invariants.md` now agree with the code, and five new assertions in
`crates/invariants/tests/registers.rs` keep them there — including a migration
count read from the directory, so adding a migration and not updating the prose
fails the build.
