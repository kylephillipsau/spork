# Warehouse platform: architecture

## What it is

A warehouse management system for a distributor of food safety products (gloves,
hair nets, protective equipment) running several sites across Australia, built to
replace Oracle NetSuite. Rust and PostgreSQL on the server, React on the client,
Tauri on the handhelds. One deployment can serve more than one company, and that
is true from the first database migration rather than added later.

Most of the catalogue is non-perishable. Some of it is not: a few lines require
refrigeration, and some protective equipment carries a shelf life. Batch tracking,
expiry and stock rotation therefore cover something the business already does, and
a system without them would fall short of current requirements rather than future
ones.

Each of those is a property of a product rather than a mode the whole system runs
in. A product that needs a use-by date has one. Everything beside it on the shelf
does not, and neither pays for the other's requirements. Weight-based pricing and
holding stock on behalf of another company work the same way and are not currently
used.

That rests on one decision, covered below: what the system can do is determined by
the data rather than by a setting. It is the difference between a platform that
grows into new requirements and one that has to be forked for each.

The work it replaces is packing and despatching several hundred orders a day. That
process spans two systems and about a dozen screens. Most of it is navigation
rather than judgement. The operator already knows what to do and spends the time
telling the software about it.

## The premise

Most warehouse systems treat the database as the authority and the floor as a
source of errors to be checked. This one inverts that.

A scanner in someone's hand observes physical reality. The database only models
it. When the two disagree the scanner is usually right, and a system that rejects
the scan to protect its own consistency has thrown away the more reliable of the
two.

Three things follow, and most of the design sits downstream of them.

Scans are records of what happened, not requests for permission. A pick is written
down as something that occurred at a place and time, not as a transaction awaiting
approval. The record of stock movements is only ever added to, never edited, and
every entry carries an identifier the handheld generates. Sending the same entry
twice changes nothing, and entries can arrive in any order without changing the
result. A network dropout therefore turns into a few late arrivals rather than
into a separate offline mode with its own code to maintain.

Two pickers taking the last unit will drive the count to minus one. That is
allowed. Rejecting the second pick would mean discarding a true record of
something that happened in order to keep a number tidy, and the unit has gone
either way.

Disagreement is the most valuable thing the system produces. A count of minus one
means something physical happened that nobody wrote down: stock damaged and not
reported, a delivery that never arrived, a mislabelled pallet. Competing systems
treat these as adjustments to be quietly corrected. Here they become findings,
each with an owner, the evidence behind it and a resolution, and they raise a
ticket so a manager can look into it without stopping the floor.

That is why naming the person is built into the structure rather than added as a
feature. Every movement and every scan records an individual, the device used, and
two timestamps: when the handheld says it happened, and when the server received
it. A crew name in place of a person makes the whole apparatus decorative, because
nobody can follow up a question with "Casual Melbourne".

## Four kinds of thing

Every table in the database is classified two ways. The first says where the
information came from, and therefore what may be done with it.

| Category | Meaning | Rules |
|---|---|---|
| Fact | What happened | Only ever added to, never edited |
| Intention | What is planned | Can change or be cancelled, and is checked against what actually happened |
| Assertion | What another company stated | Cannot be edited, always names who said it, never changes a stock figure |
| Finding | Where two of those disagree | The output, not the failure |

The second classification says how a table is used: a list of reference data, a
stored summary derived from something else, a set of configurable values, or a
grouping of other records.

That second classification is what stopped the first one growing. A goods receipt
looks like a fifth kind of thing, and so does a stock balance, and so does a table
of settings. None of them is. They are the same four categories seen through a
different use, and once uses have names the list of categories stops expanding.

Assertions are the least obvious of the four and the most useful. A supplier's
despatch note is not a fact, because nobody here saw the goods being loaded. It is
not a plan either, because nobody here made it and nobody here can withdraw it. It
is a claim, and what makes it different from everything else is not who wrote it
but that somebody outside holds a copy and will quote it back. A despatch note
sent out is exactly as unretractable, so the same category covers both directions.

## The spine

Two tables carry everything, and both are only ever added to.

The first records that a quantity of a product changed hands: moved to a different
place, into a different container, reassigned to a different batch, marked
damaged, or transferred to a different owner. Every movement subtracts from one
place and adds to another. The stock figures the warehouse actually reads are
running totals derived from that record. They are stored so they can be read
quickly, updated in the same breath as the movement itself, and can be rebuilt
from scratch at any time. A scheduled job rebuilds them and reports any
disagreement.

The second records that something was measured, along with who measured it, how,
and how much it should be trusted. Dimensions, weights, temperatures, quality
grades, carrier delivery estimates and quantities a supplier claims to have sent
all go in the same place. What is being measured is named in a separate list
rather than by a code on the measurement itself, so new kinds of thing can be
measured without touching a table holding tens of millions of rows.

That second table is what makes integration tractable. The same fact arriving by
electronic data interchange, a supplier's website, a spreadsheet, a dock scale or
somebody typing it produces identical records apart from where it came from. A
test enforces that, one case per channel, so a new connection needing a field the
others lack fails on the day it is written rather than years later.

Nothing writes to the running totals directly. Database permissions prevent it, so
"nothing writes to stock" is a rule the database holds rather than a sentence in a
document that somebody eventually breaks.

## The parts worth arguing about

### Where stock sits is part of its identity

A pallet is a container, and stock sits either at a location or inside a
container, never both. Moving a pallet of forty cartons writes one record rather
than forty. Forty records would claim forty inspections that nobody carried out,
and the value of the whole ledger is that every line is work someone actually did.

Deciding which of the two records to write is settled by asking what changed
rather than what the operator intended, because a carton is both a container and a
thing inside a container, so "who has custody" has no single answer. If the
stock's identity changed, that is a movement. If a container moved, that is a
container record. Exactly one applies, every time.

### Settings are data, decisions are code

Eight separate things need the same kind of configuration: how stock is chosen for
an order, how much variance is acceptable, minimum shelf life on despatch, where
goods are put away, what condition received goods start in, how much gets
inspected, what gets audited, and how much over-delivery is tolerated. All eight
share one mechanism. A value can be set for everything, or for a category, or for
one product, customer or site, and the most specific setting wins.

The boundary between configuration and something nobody can predict is a rule that
can be checked mechanically. A setting may name what it applies to, when it
applies, and a number. A setting may never name a database field, an operator such
as "greater than", a comparison, or a sequence of steps to carry out. Once
settings start naming fields and operators, the configuration has quietly become a
programming language that nobody designed, and this system excludes that.

### Stock rotation is not fixed

Choosing between oldest stock first and the nearest pallet is a business decision
that varies by product and by situation, and most of the current catalogue has no
rotation requirement at all. Taking the nearest pallet at ground level is often
right. The same substitution is a different call when the alternative needs a
forklift to bring a pallet down, which means waiting for equipment and possibly a
second person. The system holds expiry dates, distances and the cost of reaching
each location, ranks the options, and a manager decides how much each factor
counts for.

A flexible system can always be made strict. A strict one cannot be made flexible
without rebuilding it.

### What the system can do is determined by the data

This is what the breadth rests on. Batch tracking, weight-based pricing, holding
another company's stock and operating several legal entities are all properties of
a product or a site, and all default to off. An operation that needs none of them
configures nothing and never encounters them. There is no settings screen where
switching something on changes how the whole system behaves.

A global switch per feature is what forces a vendor to maintain separate versions
for separate industries. Here one product can be batch-tracked and rotated while
everything beside it on the shelf is neither. The same mechanism covers a
catalogue with a few refrigerated lines and a catalogue where most of it is
refrigerated, without either paying for the other's requirements.

### Every connection to another system is optional

The system is complete on its own. Every external system it talks to is one
implementation of a join that has a working default behind it, so an operation
running nothing else loses no function it needs.

Orders are created here and are complete here. A business that would rather
create them in an accounting system can, and that connection is a channel rather
than a dependency, with one system of record per order recorded on the order
itself. Two systems that both accept edits to the same order need a conflict
rule nobody can explain to a person on a dock, so that is refused outright.

The same applies to paperwork for stock moving between two companies in a group.
The system raises it, and a business with a finance system that would rather do
it there can have the movement raise a flag instead. Both work and neither is
required.

This is the difference between replacing a dependency and moving it. A warehouse
system that cannot run without the accounting system beside it has not reduced
anything, it has added a second thing to keep running.

### Old data is archived, never deleted

Anything the running totals are derived from is kept indefinitely, because the
check that makes stock trustworthy is that the total equals the sum of the whole
record. Delete part of it and the check cannot run.

Records nothing is derived from, such as scan failures, can age out. They are moved
to cold storage rather than removed, because the failure mode of removing them is
silent: a check that asks whether an identifier has been used in the last twelve
months has nothing to compare itself against, so truncating its history makes it
quietly start passing.

How far back each kind of record must reach is written down with the rule that
requires it, whether that is a carrier's liability window, a supplier claim period,
or the twelve months a barcode identifier cannot be reissued within. A check
confirms the data actually reaches those floors.

### Customers can add fields, and they become real ones

A customer can define a set of fields and the system builds them a real database
table, with real types and real validation, rather than putting everything in one
untyped bucket.

The usual argument against customer-defined fields is that adding a column is
cheap. That is true when one company runs the software for itself. It stops being
true for a product, where the company that needs the field and the company that
can add it are different. The objection was never really about adding columns. It
was about storing data in a form where nothing knows what it holds until something
tries to read it, and that objection still stands.

## What it does that NetSuite cannot

These fall out of the design rather than being features added to it.

It can say what is physically in a carton. Cartons are real records and their
contents link back to order lines, so a packing list is per carton, a damage claim
can name the carton the item was in, and a declared weight can be checked against
its contents before a carrier charges for the difference. NetSuite holds one line
per product with no carton count, so that number exists nowhere until an operator
types it into the freight system.

It can say where freight went and what it cost. The carrier and the service used
to book them are recorded separately, so Swift booked through MachShip today and
Swift booked directly tomorrow are the same carrier, and the cost history survives
the change.

It can say which of its own records are wrong. Every measurement records how it
was obtained and how well it has held up since. When a carrier reweighs a pallet
and charges accordingly, that reweight is stored as their measurement, so
comparing what was predicted against what was invoiced is a single question. It
finds both the bad records and what they are costing.

It can say what a supplier actually did. A despatch note is kept exactly as sent,
and the goods received are compared against it line by line. Because the original
claim is never overwritten, how reliable a supplier's promises are stays
answerable months later, which is what every system that overwrites the original
figure gives up.

## Deliberate exclusions

Recorded as decisions rather than gaps: a configurable rules engine, a general
purpose "any field on anything" store, one document type that tries to be every
document, a workflow designer, screens customers can rearrange, purchase invoice
matching and stock valuation, yard and dock scheduling, running the transport
itself, tracking individual serial numbers through stock, and forward-looking
delivery promising.

Two of those warrant an explanation. Forward-looking delivery promising means
answering "what can I commit to for Thursday" by looking across everything due to
arrive between now and then. Doing it correctly means finding the lowest point the
stock level reaches anywhere in that window, and no vendor computes that from
records held one row per expected delivery. It would need another layer of derived
totals, updated in the background rather than immediately, and this design
excludes background updates by name. That is two exceptions rather than one.

Yard management stays out while recording a vehicle arrival and its pallet count
stays in, because Australian grocery contractually requires a signed paper docket
with a pallet count on every delivery, and that is not the yard.

## Current state

One hundred and sixty-nine recorded decisions, a hundred and twenty-nine rules the design must always
satisfy, and a database that ninety-one migrations build and reverse cleanly, both
from empty and with data in them.

That list of rules is what the design gets checked against, and it is now a test
suite rather than a document. Each rule carries its own metadata and its own
identifier, so allocating the same number twice is a compile error rather than a
mistake anyone can make. One hundred and seven rules are implemented and none fails.

Seventy-one of the rules state that something does not happen, and a rule of
that shape passes when there is nothing to check. The suite reports those
separately: a check that examined nothing is recorded as proving nothing, rather
than counted as a pass.

The rules have already earned their place. Three of them caught defects in the
schema that a reading would not have: a fact table granted the right to be
edited, a fact table with no link to the act that produced it, and a policy
column named after the rules engine this design refuses to be. A fourth caught
one in the application era: a migration adding a second foreign key onto the
reapable stock projection, which S28 forbids and which would have wedged the
first rebuild that tried to collect the cell it named.

The schema covers the two tables that carry everything, the projections folded
from them, containment in both current and historical form, findings, and the
policy resolver. Above it sits an API of sixty-seven endpoints and a projection
scheduler, and above that a React client of fifteen screens: the pack bench, the
picking list, despatch, capture and weighing, the findings queue, order search,
and the administrative screens behind them. There is no separate handheld build;
a phone gets the same client, laid out for the width it has.

The gap worth naming is on the writing half of that API. Fourteen of its
thirty-seven writing endpoints have never been exercised over HTTP, and four of
those are among the six paths that write the ledger. The pure modules they call
are covered; the handlers wrapping them are not, so the transaction handling,
the idempotency claim and the ledger insert itself are asserted nowhere. That is
the first thing being fixed, endpoint by endpoint in order of what each writes.

Fifty-seven questions remain
open: thirty-five deferred against a stated trigger, seven wanting a written
answer rather than a decision, and fourteen minor.

## Reading further

- [domain-model.md](./domain-model.md), the domain decision record; the shared register runs D1 to D169
- [invariants.md](./invariants.md), the rules the design must always satisfy
- [open-questions.md](./open-questions.md), everything still open
- [order-fulfilment-process.md](./order-fulfilment-process.md), the process being
  replaced, as it runs today
- [warehouse-data-model.md](./warehouse-data-model.md), the shared data layer and
  the features that sit on it
- [competitor-analysis.md](./competitor-analysis.md),
  [inbound-analysis.md](./inbound-analysis.md) and
  [supply-side-design.md](./supply-side-design.md), the supporting research
