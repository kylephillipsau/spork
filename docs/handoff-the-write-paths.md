# Handoff: the write half of the server has no tests, and one of them now does

Written 2026-08-23, covering a review and a session's work on 22 August. Read
this before touching `crates/server/src/routes.rs`, `client/domain/api.ts` or
anything under `crates/server/tests/`.

The short version: **the design is holding; the shell around it is where the
rush shows.** A read-only pass over the repo against its own documentation found
six things, the largest being that fifteen of thirty-seven writing endpoints had
never been executed over HTTP. `POST /receipts` now has tests. Fourteen do not.

This does not supersede [handover.md](./handover.md), which is still current on
the interface, the deployment and the conventions. It adds the server's write
paths, which that document does not cover.

---

## First, run this

```sh
git pull

# A database that has never seen this project. Not the compose one, and not
# the dev one either — see "the fixture has a two-day shelf life" below.
docker exec -e PGPASSWORD=nylonite nylonite-postgres-1 \
  psql -U postgres -d postgres -c 'DROP DATABASE IF EXISTS nylonite_verify' \
                               -c 'CREATE DATABASE nylonite_verify'

# What CI's database actually is: every up.sql in order, then seed.sql. This is
# the tail of verify-migrations.sh, without the four phases before it.
for m in $(ls -1 migrations | sort); do
  grep -qiE 'ALTER +TYPE.*ADD +VALUE' "migrations/$m/up.sql" && F="" || F="-1"
  docker exec -i -e PGPASSWORD=nylonite nylonite-postgres-1 \
    psql -U postgres -d nylonite_verify -q -v ON_ERROR_STOP=1 $F -f - \
    < "migrations/$m/up.sql" || { echo "FAIL $m"; break; }
done
docker exec -i -e PGPASSWORD=nylonite nylonite-postgres-1 \
  psql -U postgres -d nylonite_verify -q -v ON_ERROR_STOP=1 -f - < fixtures/seed.sql

DATABASE_URL=postgres://postgres:nylonite@localhost:55432/nylonite_verify \
  cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

`DATABASE_URL` is not optional and its absence is silent — handover.md's *First,
run this* explains why, and it has not stopped being true.

**Why the recipe above rather than `verify-migrations.sh`.** That script is
still the right thing for proving the migration set reverses, and it is what CI
runs. It is the wrong thing to point at a database you are keeping: it drops the
schema, and it leaves the ledger empty so `migrate.sh --status` afterwards
reports all eighty-eight as pending. The recipe above builds the same end state
without either.

---

## What changed

**`crates/server/tests/receipt_http.rs` is new** — three tests, thirty-six
assertions, the first HTTP coverage `POST /receipts` has ever had.

- `a_receipt_writes_a_header_a_line_and_the_movement_that_names_it` — entered
  cartons convert to base units with both forms stored (D92 / Q173), the
  movement carries `goods_receipt_line_id` back to its cause, and a second line
  naming the same `goods_receipt_id` joins the header rather than opening a
  second one (D43 / Q172).
- `the_same_receipt_sent_twice_writes_one_movement` — the one that matters. Same
  body twice: same ids returned, replay warning present, `header_created` still
  answering for the act being replayed, and one movement, one line and one
  header in the database afterwards.
- `a_replay_that_disagrees_about_the_quantity_is_refused` — 400, naming the
  figure it is holding, and the refused send writes nothing.

Both replay tests were checked by mutation before being trusted: with
`if act.is_replay()` in `record_receipt` changed to `if false`, both fail. The
happy path was checked the same way by making the movement stop naming its line.
Ten consecutive runs green on the dev database, six on a freshly built one.

**`docs/architecture.md`'s "Current state" and `crates/server/src/main.rs`'s
module doc** both described a server with no screens and no write paths. They
now describe the one that exists. The load-bearing sentences in architecture.md
— the ones `registers.rs` reads numbers out of — are untouched.

---

## Six things that will bite you

### 1. Nothing below the application stops a receipt being recorded twice

This is the largest finding in the review and it was only visible by mutation.
With `record_receipt`'s replay branch disabled, an identical second submission
**returned 200 and wrote a second `goods_receipt_line` and a second
`stock_movement`.** No unique constraint objected, no exclusion constraint, no
trigger.

D25 settles that *"one `client_event` registry owns idempotency"*, and it does —
but it owns it alone. One `if`, in one handler, is the entire defence against
double-receiving a delivery, on the table the architecture calls the spine.

**The obvious fix is wrong.** A unique index on
`stock_movement (tenant_id, client_event_id)` is the reflex, and three acts in
the fixture legitimately write two movements each: `ce000000-…0001`, a receipt
covering two lines of one delivery; `ce000000-…0004`, an adjustment that
subtracts in one place and adds in another; and `ce000000-…000a`, which
despatches and adjusts together. That constraint would reject all three. Whether
something narrower holds — unique per `goods_receipt_line_id`, or partial by
reason — is a real schema question, not a patch.

Worth noting beside it: `require_receipt_facts` refuses to replay an act owning
more than one receipt line, and `ce000000-…0001` is exactly such an act. It was
seeded directly rather than written through the handler, so nothing has ever hit
that path.

### 2. The client mints a new idempotency key on every attempt

`client/domain/api.ts:42` states the contract the whole dropout story rests on:

> Every write carries a `client_event_id` the client mints … so the identifier
> is generated here, before the request, **and reused if it has to be sent
> again.**

Nothing reuses it. All twelve call sites write `client_event_id: uuid()` inline
in the request body, evaluated fresh per call, and there is no retry or
memoisation anywhere in the client. `api.consign` has the same defect with its
`id`, under a comment saying the identifier is *"the only thing that makes a
retry a replay rather than a second booking."*

Today nothing retries, so the key is never reused and the bug never fires. The
case it leaves open is the ordinary one on a handheld: the server commits, the
reply is lost, the operator presses the button again — and with finding 1, there
is nothing underneath to catch it.

**The comment in `api.ts` was deliberately left saying what it says.** It states
the intended contract correctly; the code is what is wrong. Annotating the bug
into the comment would be churn against a fix that is already the next step.

Fix shape: mint the id once per act in the hook that owns the act, pass it into
`api`, and reuse it across attempts of the same act.

### 3. The fixture has a two-day shelf life

`fixtures/seed.sql` sets `promised_to = (CURRENT_DATE + 2)::timestamptz` under a
comment that argues, correctly, that a promise must be relative because *"a
fixture loaded today that promises a date last August is stating something
false."* The intent is right and `CURRENT_DATE` evaluates at load time, so the
date materialises and pins.

The result: `the_queue_is_the_way_in` in `pack_walk_http.rs` fails on any
database seeded more than two days ago, asserting `due today` against a fixture
that now reads overdue. CI never sees it because CI reseeds every run. It is not
a defect in the code and it is not worth bisecting.

If it wants fixing, the fix is in the fixture — a promise stored relative and
resolved at read time, or a reseed step — not in the test.

### 4. A failed run leaves an observable behind, and it poisons the next one

handover.md §9 already records that the suite passes once per database and that
`what_a_carton_of_something_measures_is_a_fact_about_the_kind` answers `mixed`
where the fixture expects `style`, because a sibling test in the same binary
posts an *own* carton observation concurrently.

**The delta is that it persists.** When a test earlier in `pack_walk_http.rs`
panics, the file's cleanup never runs, and the own-carton `observable` stays in
the database — so the failure is no longer a same-run race but a permanent
property of that database until somebody removes it. The dev database had five
such events accumulated between 17 and 22 August. It compounds with finding 3:
the stale fixture makes a test panic, the panic leaks, the leak fails a
*different* test on every subsequent run.

The cleanup, which has to go in dependency order:

```sql
BEGIN;
CREATE TEMP TABLE leaked AS
  SELECT id FROM observable
   WHERE item_id = '17e10000-0000-0000-0000-000000000002'
     AND packaging_level = 'carton' AND id::text NOT LIKE '0b5e%';
CREATE TEMP TABLE ev AS
  SELECT id FROM observation_event WHERE observable_id IN (SELECT id FROM leaked);
DELETE FROM observation_current WHERE observable_id IN (SELECT id FROM leaked);
DELETE FROM observation WHERE observable_id IN (SELECT id FROM leaked)
                           OR observation_event_id IN (SELECT id FROM ev);
DELETE FROM observation_image WHERE observation_event_id IN (SELECT id FROM ev);
DELETE FROM observation_event WHERE observable_id IN (SELECT id FROM leaked);
DELETE FROM observable WHERE id IN (SELECT id FROM leaked);
COMMIT;
```

`receipt_http.rs` is written not to have this problem: every act id is minted
and named up front, and the cleanup runs on ids rather than on shapes. It still
leaks if it panics mid-test, which is the same structural cost `pack_walk_http`
pays for committing.

### 5. `warnings` mixes what the act meant with what the server was doing

`RecordReceiptResponse::warnings` carries two unrelated kinds of sentence:
semantic outcomes (this was a replay; the disposition raised a finding) and
operational ones (`projection_refresh_tenant` was rate-limited, so the claim
handover is deferred). Under concurrency the second kind appears on a perfectly
ordinary first send.

This cost real time: the first draft of the replay test asserted the first send
returned no warnings and failed about one run in three. It now matches on the
substring `replay`, which is what any caller wanting to tell an operator *"that
was already recorded"* would also have to do. A typed field — or a `replayed:
bool` like `ConsignmentResponse` already has — is worth doing before more
callers grow the habit.

### 6. Two register numbers drift with nothing watching them

`docs/invariants.md` marks **71** rows with the ● vacuity marker; the Rust specs
carry `asserts_absence: true` on **69**. The two that disagree are **J22** and
**J73**, ● in the document and `false` in code.

`registers.rs` checks the ● count against the prose and the row count against
`ALL.len()`, but never checks ● against `asserts_absence`, so that field drifts
freely. Nothing is currently mis-reported — the verdict comes from
`examined == 0`, deliberately and correctly — but this is the same shape as
every other gate in this repo that failed by passing.

Two edits: reconcile the flags, and add the assertion so it cannot happen again.

---

## Where to pick up

**First, lift the harness into `tests/common/`.** `receipt_http.rs` copies
`ok_json` from `pack_walk_http.rs` verbatim and adds a `bearer` helper that file
also has inline. That is copy two of a shape whose own module header describes
killing at forty-nine copies. Writing the next test file makes it copy three, so
the lift goes first, in its own commit.

**Then the remaining ledger writers, largest first**, each on the pattern
`receipt_http.rs` sets — one happy path asserting what only a running handler
can, and one duplicate-submission-replays, both checked by mutation before being
believed:

| lines | endpoint | why this order | |
|---:|---|---|---|
| 315 | `POST /adjustments` | writes the ledger; `adjusting::check` is covered, the handler is not | ✅ |
| 298 | `POST /moves` | writes the ledger; simplest of the four, good second | ✅ |
| 284 | `POST /packages/{id}/despatch` | writes the ledger *and* a package event | ✅ |
| 257 | `POST /counts` | writes an assertion and a finding, not stock — different shape | ✅ |

**All four covered on 2026-08-31**, in `crates/server/tests/ledger_http.rs`, on
the pattern predicted above. The prediction that they would find something held:
`POST /adjustments` could not replay, because its claim sat behind a delta that
a successful adjustment makes zero. See handover.md.

Ten more writing endpoints have no HTTP test after those four: the package
lifecycle (`place`, `contain`, `open`), `allocations/{id}/release`, both
`discrepancies/{id}` actions, `passkeys/authentication/finish`,
`DELETE /passkeys/{id}`, `projections/refresh` and `import/items`.

**`DELETE /passkeys/{id}` deserves a note.** It is the only endpoint in the
repo declared `#[actix_web::delete(...)]` rather than `#[delete(...)]`, so it is
invisible to any grep written against the short form — which is how it stayed
off the first version of the coverage table. It is the credential-revocation
path.

**Then the client event-id fix** (finding 2). With findings 1 and 4 understood,
it is the highest-value single change on the list.

---

## Known debts this adds

**`routes.rs` is 8,137 lines and duplication has re-accreted.** `203a935`
removed forty-nine copies of three functions; the layer that grew since has the
same shape back. The `claim_act(tx, &NewClientEvent { … })` block appears
**fifteen times**, every field of it from `who` and `body`. The package-handler
preamble — `caller`, tenant, `path.into_inner()`, `body.into_inner()`, the
`"operator_scan"` default, `TenantScope::begin` — is duplicated **five times**
across place / contain / seal / open / despatch. The replay-mismatch arm at
lines 1605, 1923, 2082 and 5201 is **four byte-identical blocks** differing only
in a literal: `"placed"`, `"sealed"`, `"opened"`, `"voided"`.

That last one is the same setup as the bug `203a935` describes — copies
drifting, the newest already wrong — and four of those five package handlers
have no HTTP test, so the duplicated code and the untested code are the same
code. The extraction is safe once the coverage exists, which is the order above.

**The client test runner is an enumerated list again.**

```json
"test": "node --test domain/*.test.ts app/nav/*.test.ts app/routing/*.test.ts app/measurement/capture/*.test.ts"
```

All six test files match today, so it passes honestly. A `*.test.ts` added under
`app/outbound/`, `app/session/` or `app/scan/` is silently not run, and the
failure is green. `990939b` was *"a glob that had quietly narrowed again"* and
the fix widened the enumeration rather than removing it.
`client/scripts/check-contract.mjs` is the model for how this repo builds a gate
that cannot shrink: a recursive directory read plus `everyInterfaceIsPaired`.
The npm script never got the same treatment.

**`check-contract.mjs` checks one direction only.** Every TypeScript interface
must be paired with a Rust struct; a serialised Rust response struct with no
TypeScript interface is invisible to it. Lower severity than the above — the
field-level comparison still catches drift within a pair — but it is the same
family.

**Nothing gates endpoint-has-test.** There is no check anywhere that a route
registered in `routes::configure` is reached by anything. The coverage table in
this handover was built by hand and will be wrong the moment somebody adds an
endpoint. `api_mount.rs` proves the prefix; nothing proves the surface.

---

## The state you are inheriting

CI on `b2a270e` is green across all three jobs. The working tree at the time of
writing has `receipt_http.rs` untracked and edits to `architecture.md` and
`main.rs` uncommitted; nothing else is touched.

The **dev database** (`nylonite`, port 55432) was three migrations behind and is
now at eighty-eight, applied forward-only by `migrate.sh`'s own method. Its
leaked observable has been purged twice and will come back the next time a test
in `pack_walk_http.rs` panics — which, until the fixture is reseeded, is every
run. The one red test on it is `the_queue_is_the_way_in`, and that is finding 3
rather than a defect. On a freshly built database the whole workspace passes,
new tests included, which is how findings 3 and 4 were separated from real
failures.
