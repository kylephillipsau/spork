# Warehouse platform: proposal

## Summary

A replacement for the warehouse side of NetSuite. Designed and specified in
full, with the database and the rules that check it now built and running.

Packing and despatching an order currently spans two systems and about a dozen
screens. Almost none of that is judgement. The operator already knows what to do
and spends the time telling the software about it. The one decision in the
process is how many cartons there are and what they weigh, and even that is mostly
derivable from data the business already holds. The design collapses the whole
sequence to a single screen.

The larger gain is harder to measure. Systems that treat the database as the
authority and the floor as a source of errors generate reactive work without end:
chasing discrepancies, reconciling counts, explaining why the system says one
thing when the shelf says another. This design starts from the
opposite assumption. The scanner is more reliable than the record, so a
disagreement becomes a finding with the evidence attached rather than an
adjustment nobody sees, and most of that chasing never starts.

The design is specified to a level it could be built from: seventy-one recorded
decisions, a hundred and nine rules it must always satisfy, research against six
comparable products, and the labelling and trading standards Australian grocery
imposes. What it does not have is code or costings for the alternatives. Code is
being addressed. The costings need figures from the business.

## The problem

The current process is documented step by step in a separate note. In outline: find
the order by confirmation number, check it against the contact, open it, find the
related fulfilment record, check it was picked and picked at the right site, edit
three fields that are the same every time, switch to a different tab, enter the
carton dimensions and weight, save, print an A4 sheet from a page that opens in a
new tab, then move to the freight system, find the matching consignment by
eyeballing the delivery address, choose the carrier, apply that carrier's
rules from memory, correct the carton count because the first system
never recorded it, tick a declaration, download a PDF, print it, and for some
carriers go back afterwards and print a manifest separately. That sequence runs
several hundred times a day.

Two costs sit inside that sequence, and they behave differently.

The first is navigation: loading screens, switching tabs, re-entering things one
system already knows. Timed with a stopwatch, a standard order takes one minute
forty-five seconds when every detail has been entered correctly and the
consignment is going to Direct Transport.

That is the clean case on one carrier, so it is a floor. A missing detail pushes
it up. So does a carrier whose rules have to be recalled, and so does a manifest
that has to be printed separately afterwards. The floor is enough to make the
case: at three hundred orders a day it comes to more than eight hours of
someone's day, spent telling two systems what the operator already knew. The
design removes almost all of it, because the sequence exists to satisfy two
systems rather than to do the work.

The second is reactive work, which is the larger cost and much harder to put a
number on. When the software insists it is right and the floor is wrong, every
physical event the software did not expect turns into an investigation with
no evidence attached. Stock that does not match. A carton count nobody recorded. A
carrier invoice that disagrees with what was declared. A supplier delivery that was
short, discovered weeks later with no record of what was originally promised.

The software's design generates that work, not the warehouse. A system that
records what happened, keeps the evidence and raises the disagreement as a finding
with a named person and a timestamp does not make that work faster. It stops most
of it from arising.

## Goals

1. Collapse packing and despatch to one screen, with the carton dimensions and
   weight computed from what the business already knows and confirmed rather than
   typed.
2. Make every discrepancy investigable: who, when, which device, and what the
   evidence was. Today most of them are not.
3. Make freight cost verifiable by comparing what was predicted against what was
   invoiced.
4. Keep supplier promises answerable after the fact, by never overwriting what was
   originally advised.
5. Serve more than one site and more than one company from one deployment.

## Non-goals

This does not do purchase invoice matching, stock valuation or general ledger. It
hands accurate, timestamped, non-rewritable figures to a finance system instead.

NetSuite fills that role here, and that is a property of this deployment rather
than of the design. Every external system it connects to is a capability with a
working default behind it, so an operation running nothing else loses no
necessary function. That rule keeps the seam thin, and it stops this work from
rebuilding the dependency it is meant to reduce.

Also excluded: a configurable rules engine, a general purpose "any field on
anything" store, a workflow designer, screens customers can rearrange, yard and
dock scheduling, running the transport itself, tracking individual serial numbers
through stock, rendering invoices, forward-looking delivery promising, and
publishing a product catalogue to retailers. Each of those is a decision
with reasoning behind it, and several are the features that make comparable systems
slow and hard to change.

The catalogue exclusion is a scope decision rather than a limit. Australian
grocery retailers take product data through a shared catalogue, and the logistics
part of what they ask for is already derivable here: dimensions and weights per
packaging level, cartons per layer and layers per pallet, each recorded with how
it was obtained. The catalogue also carries price, classification and marketing
copy, which are not warehouse facts. The system exports what it observes and a
product information system publishes it.

## Alternatives considered

### Stay on NetSuite

The cheapest option and the honest baseline. The process works today and people are
trained on it.

What it does not do cannot be fixed by configuration. NetSuite records one line per
product with no carton count, so what is physically in a box exists nowhere until
somebody types it into the freight system. It has no cartonisation. Its discrepancy
handling adjusts the figure and keeps no finding. All three follow from its data
model, and none of them is a setting anyone can change.

The cost of staying is the reactive work, which continues and grows with volume.

### Buy a warehouse system

Six products were researched in depth: NetSuite's own warehouse module, Manhattan
Associates, CartonCloud, ShipHero, Odoo and Peoplevox. CartonCloud is the closest
fit on market and scale, being Australian and aimed at operations this size.

Several findings apply across the whole set. None of them models package contents
in a way that answers what is physically in a carton. Cartonisation is either
absent or, in the one case where it is marketed, a historical guess that can be a
day stale. Every one of them settles a discrepancy by correcting a number, leaving
no record of what disagreed. All six ignore pallet accounts, which are an
uncontrolled loss line.

Buying also does not remove the integration work. Any of these has to be connected
to NetSuite and to the freight system, which is a substantial part of the effort
either way.

This option needs a number the business has and this document does not: what the
alternatives cost, and what NetSuite currently costs. That should be established
before any decision.

### Extend NetSuite

Building on top of it with custom records and scripts keeps everything in one
system.

The limits are the same as staying, because the data model is the constraint.
Adding a carton count to a system with nowhere to put it means adding a custom
record, which is where part of the current process already lives. It also deepens
the reliance on the system this exercise exists to reduce.

### Build

The alternatives are not bad. They share one gap: every one of them treats the
database as the authority. That is a structural property, not a feature anybody
forgot, which is why no amount of configuration reaches it. The design work is
largely done and the research behind it is documented. Where the design looks
unusual, it is because that stance is unusual.

The argument against it is real and stated in the risks below.

## The approach

### The premise

A scanner in someone's hand observes physical reality. The database only models it.
When the two disagree the scanner is usually right, and a system that rejects the
scan to protect its own consistency has thrown away the more reliable of the two.

Scans are therefore records of what happened, not requests for permission. The
record of stock movements is only ever added to, never edited, and every entry
carries an identifier the handheld generates. Sending the same entry twice changes
nothing, and entries can arrive in any order without changing the result. A network
dropout turns into a few late arrivals rather than into a separate offline mode
with its own code to maintain.

Two pickers taking the last unit will drive the count to minus one. That is
allowed, because rejecting the second pick would mean discarding a record of
something that happened to keep a number tidy, and the unit has gone either way.

That makes disagreement useful. A count of minus one means something physical
happened that nobody wrote down. Competing systems treat it as an adjustment to be
made and forgotten. Here it becomes a finding with an owner, the evidence behind it
and a resolution, and it raises a ticket so a manager can look into it without
stopping the floor.

That is why the structure names the person. Every movement records an individual,
the device used, and two timestamps.
A crew name in place of a person makes the whole apparatus decorative, because
nobody can follow up a question with "Casual Melbourne".

### How the data is organised

Every table is classified two ways. The first says where the information came from,
and therefore what may be done with it.

| Category | Meaning | Rules |
|---|---|---|
| Fact | What happened | Only ever added to, never edited |
| Intention | What is planned | Can change or be cancelled, and is checked against what actually happened |
| Assertion | A statement of record exchanged with another company, in either direction | Cannot be edited, always names who said it, never changes a stock figure |
| Finding | Where two of those disagree | The output, not the failure |

The second classification says how a table is used: reference data, a stored
summary derived from something else, configurable values, or a grouping of other
records.

Two tables carry everything, and both are only ever added to. The first records
that a quantity of a product changed hands. Stock figures are running totals
derived from it, stored so they can be read quickly and rebuilt from scratch at any
time, with a scheduled job that rebuilds them and reports any disagreement.

The second records that something was measured, along with who measured it, how,
and how much it should be trusted. Dimensions, weights, temperatures, carrier
delivery estimates and quantities a supplier claims to have sent all go in the same
place. That is what makes integration tractable, because the same fact arriving by
electronic data interchange, a supplier's website, a spreadsheet, a dock scale or
somebody typing it produces identical records apart from where it came from.

### What the system can do is determined by the data

Batch tracking, expiry, stock rotation, weight-based pricing, holding another
company's stock and operating several legal entities are all properties of a
product or a site, and all default to off. An operation that needs none of them
configures nothing and never encounters them.

That matters here. Most of the catalogue is non-perishable, but a few lines require
refrigeration and some protective equipment carries a shelf life, so batch tracking
and expiry cover something the business already does. The same mechanism would
serve a catalogue where most of it is refrigerated, without the current one paying
for that. A global switch per feature is what forces a vendor to maintain separate
versions for separate industries.

## What this makes possible that NetSuite cannot

It can say what is physically in a carton. Cartons are records in their own right, and their
contents link back to order lines, so a packing list is per carton, a damage claim
can name the carton the item was in, and a declared weight can be checked against
its contents before a carrier charges for the difference.

It can say where freight went and what it cost. The carrier and the service used to
book them are recorded separately, so a carrier booked through an intermediary
today and booked directly tomorrow is the same carrier, and the cost history
survives the change.

It can say which of its own records are wrong. Every measurement records how it was
obtained and how well it has held up since. When a carrier reweighs a pallet and
charges accordingly, that reweight is stored as their measurement, so comparing
what was predicted against what was invoiced becomes a single question. It finds
both the bad records and what they are costing.

It can also say what a supplier actually did. A despatch note is kept exactly as
sent, and the goods received are compared against it line by line. Because the
original claim is never overwritten, how reliable a supplier's promises are stays
answerable months later, which is what every system that overwrites the original
figure gives up.

None of those are features added on top. They fall out of the way the data is
organised, which is why they are hard to retrofit into a system that organised it
differently.

## Risks

It is currently one person's work. The design, the research and the build sit in
one head and one pair of hands. That is the main risk to the business, and the
one most affected by how the work is resourced.

The estimates are still estimates. The database exists and the rules that check
it run against it, which was the first real validation and has already paid for
itself by catching three schema defects. What does not exist is the application:
no server, no screen, no handheld, and therefore no timing of the new process to
set against the old one.

The floor has to change two habits: individual logins rather than a shared crew
name, and measuring goods once when they first arrive. Both are small in isolation,
and both are the kind of change that does not happen unless somebody owns it.

The boundary with NetSuite has to hold. NetSuite is the finance system in this
deployment, so there is a seam, and seams need maintaining. The design keeps it
thin and does not depend on it, but it does not remove it either.

Data migration is unscoped. Moving product data, locations and open orders across
has not been planned in any detail.

The baseline is a single timed run. One minute forty-five seconds was measured on
a standard order, and the daily figure multiplies it by an order count that is
itself approximate. Timing more orders, including the ones that go wrong, would
either confirm it or move it. The improvement is measured against that number
whichever way it lands, so it is worth having before the first slice is built.

## Where it stands

There are seventy-one recorded decisions, a hundred and nine rules the design must
always satisfy, and research against six comparable products alongside the freight
standards and Australian grocery requirements that apply. Twenty-nine migrations build
the database and reverse it cleanly, and sixty-nine of the rules run against it
as tests.

The design has been checked against real requirements rather than against itself,
and the specification is detailed enough to build from directly. No technical
question now blocks the first database work, and none is waiting on an answer
only the business has. Forty-eight remain open, all of them in one register
instead of scattered through the research: twenty-five deferred against a stated
trigger, nine where the model is settled and the reasoning wants writing down,
and fourteen minor. Nothing is waiting on a decision nobody has identified.

The next step is the first working slice: the database, the rules that check it,
and the packing screen. Building that would also replace the estimate above with a
measurement.

## Supporting documents

- `architecture.md`, the design in more detail
- `domain-model.md`, the full decision record, fifty decisions with the
  reasoning and what each rejected
- `invariants.md`, the rules the design must always satisfy, and which of them
  can pass while checking nothing
- `open-questions.md`, everything still open, with what each one blocks
- `order-fulfilment-process.md`, the current process recorded step by step
- `competitor-analysis.md`, `inbound-analysis.md` and `supply-side-design.md`, the
  research the decisions rest on
- `gs1-grocery-analysis.md`, `multi-po-asn-analysis.md` and
  `outbound-edi-analysis.md`, the research behind the grocery and trading
  standards work
