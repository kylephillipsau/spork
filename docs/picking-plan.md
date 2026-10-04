# Picking in Spork: routes, runs and the map

**Status: proposed, not adopted.** Written 2026-10-04. The proposals below have
working names, A to I, and the questions it raises are numbered 1 to 5 here
only. Each takes its D-number, and each question its Q-number, in the commit
that adopts it, as the registers require. Until then
[open-questions.md](./open-questions.md) and
[domain-model.md](./domain-model.md) win wherever this document disagrees with
them.

## The short version

Picking moves from NetSuite's handheld into Spork at Melbourne. Every route is
worked out from the layout. Spork knows where the racks are and how far apart
two bins really are on foot, using A\* on the floor. A route is the shortest
order to visit a run's bins in, from the pack station back to it.

A picker asks for the next run, and Spork offers the one that suits them: a
single order, several orders on a trolley with a tote each, or one stretch of
aisles across many orders when more than one person is picking. Each stop is
one screen asking one thing. Goods meet their order at the pack bench, which
already accounts for every unit (D202), and it says when an order is ready to
pack.

The map shows the bins, what is in them, the route, and where each picker
is.

Two things come first because everything else stands on them:

- the real layout, with a scale, which is also where the 3D bin map starts;
- a live channel between the server and the devices.

Three bugs in today's picking come first too.

A\* measures the floor: how far it is between two points, going round the
racks. A small travelling-salesman step then orders a run's stops over those
distances. The two are different jobs, and both are needed.

## The problem, in the business's terms

- **Picking happens in NetSuite.** Spork hears about a pick afterwards
  (D172) and cannot guide it.
- **The route is a number somebody typed.** `location.pick_sequence` came
  over with the bin list (D141). Spork has no idea of distance at all.
- **There is one walk for the whole site.** Today's picking screen lists every
  open line in walk order, one row per line. Nothing groups orders, sizes a
  trolley's load, or splits work between people.
- **Two people picking get in each other's way.** No claim stops two people
  doing the same line. A double pick is caught afterwards as a finding (J56),
  and there is no live channel to say who is where.
- **The orders are small and different.** Melbourne ships about 30 fulfilments
  on a busy day: a median of 3 lines and at most 32. Of 232 lines on file, 205
  are different items. Each item sits in about 1.6 bins, and up to 41.

  So little is gained from picking the same item for many orders at once.
  **The gain is proximity**: one walk past aisles K and L serves every order
  that wants something there. That is exactly what was asked for, with
  several people each taking a stretch of aisles and the orders meeting at
  the bench.
- **The crew is small.** One to four pickers. They use a trolley with totes or
  crates, a trolley with no dividers, a pallet jack or forklift for big
  orders, or just hands and a basket. All four are in use.

## What exists to build on

**The layout (D173).** Places are nested. Racks are `solid` and floors are
walk-through. Bins sit in exact cells: side, bay, level, row and position.
Melbourne's 2,191 bins sit in 15 two-sided racks, A to O.

That layout is the bin-list draft, not the real floor. The handoff's next
step 5 is to apply the real one. It is measured in cells, and nothing gives a
cell a length.

`PlacePage`'s `standing()` already works out where a person stands to reach
one bin.

**Reach (D180).** Each rack counts its levels in reach of the floor. The
levels above that need a ladder or the forklift.

**Picking.**
- `GET /sites/{id}/picking` is one list sorted by walk order
  (`picking_list.rs`).
- A pick claims what it is short of, then records the movement (`walk.ts`,
  `POST /allocations`, `POST /picks`).
- The first scan names where the goods are going: a pallet or the pack
  station (D166).

**Designed, not built (D15, D17).** These are the vocabulary for everything
below:
- `pick_batch`: the grouping of work, of kind wave, cluster, zone or single.
- `work_task`: directed work for one person at one place, with a
  `sequence`, a state and a first-claim-wins claim.
- `receptacle_assignment`: a trolley's tote slots, where each tote is a
  `package` (D6).
- One visit to a bin can serve several orders, through
  `stock_allocation.work_task_id`.

**The bench (D195 to D202).** It shows the whole order, counts every unit,
and fills the open carton first.

**Bugs that any picking work has to fix first.** All three were found in the
survey for this plan:

1. **Allocations never move past `claimed`.** Nothing writes `picked` or
   `fulfilled`, so a claim keeps counting against its bin after the goods
   have left. A bin that was picked from then shows less available than it
   holds.
2. **The walk ignores picks made in NetSuite.** A line NetSuite has picked
   stays on the walk and in the badge until the fulfilment closes
   (`work.rs`, `picking_list.rs`).
3. **"Picked" lags.** The walk reads cached `picked_quantity`, which waits
   for the scheduler, so a refresh can show a line again just after it was
   picked.

## How it should feel

The bar is the repo's own, not a slogan:
- one question per screen (W3);
- one docked action at the bottom (D134);
- the scan field asks whichever question is open (D117, D166);
- 2D answers "where", and 3D is on request (W1 to W4);
- nothing waits on a page load (D2).

A run, on the handheld:

1. **Next run.** One large card: "8 orders · 14 stops · about 12 min ·
   trolley with 8 totes". Below it, a strip of the route on the floor plan.
   Take it, or "Something else", which offers the next best.
2. **Set up the trolley.** "Scan tote 1." Each scan pairs a tote with an
   order, and the screen shows tote 1's colour. Skipped for a single order
   or a pallet run.
3. **Each stop.** The bin and its picture fill the screen, with a
   mini-map: where you are, where this is, and the way between. "Scan
   the item."
4. **Put it away.** "4 into tote 3, blue." Several orders at one bin are
   said one after another, without moving on. Short? "Only 2 here" sends
   the shortfall to the next bin with stock, re-routed on the spot.
5. **Drop off.** "Leave the trolley at PACK-1." A scan closes the run.

The packer sees the same orders arrive, each with its tote: "IF271645 is
ready to pack", or "3 of 5 lines in · 2 coming with Sam in aisle K, about 3
minutes".

A supervisor's desk screen shows today's orders, the runs, the map with
everyone on it, and what is late. It changes things only through the
planner's suggestions, never by hand-editing a route.

## The design

### Proposal A: Picking moves into Spork at Melbourne

*Proposed, not adopted. Its stock model is D212: NetSuite keeps the shelves.*

**Decision.** Pickers at Melbourne pick in Spork, not in NetSuite's handheld.
NetSuite still sends the orders and still learns what was picked, packed and
shipped (Question 1).

- **Stock (D212).** NetSuite keeps how much is in each bin; the Bridge keeps
  Spork's copy of its balance minutes old. The walk routes to the bins it
  names. A pick in Spork is custody beginning (the goods into a tote, naming
  the bin), and NetSuite is told, so its balance falls there. No opening
  balance or stocktake.

- **People.** Each picker is a person in Spork and signs in as themselves.
  Today only first-time setup creates a person, so adding people to a
  workspace comes first. NetSuite records picks under a crew name, "Casual
  Melbourne", and a crew name is no use when a question needs following up
  (architecture.md).
- **Work sessions.** Each picker's work session is declared when they sign
  in on the handheld, as D11 requires, which answers Q177 for picking. A run's
  tasks name that session.

**Why.** Routing, runs and collaboration all need the system that guides the
pick to be the one that records it. D172's path stays for picks that still
happen in NetSuite: other sites, and the changeover.

### Proposal B: The floor has a scale, and every bin face has a place to stand

*The scale is adopted as D210, in metres. The walkable grid and places to
stand are still proposed. Amends D173.*

**Decision.**
- **A scale.** A site says how long one of its layout cells is, in
  millimetres (`site.cell_mm`). One number keeps D173's relative drawing:
  nothing is measured place by place.
- **A walkable grid.** The floor is derived from the layout, never stored.
  A cell is walkable when it is inside a walk-through place and outside
  every solid place's footprint, turn included. The grid is half a cell
  fine, so a narrow aisle stays open.
- **A place to stand.** Every bin's access point is the walkable cell in
  front of its bay on its side, as `standing()` works it out today. A bin
  with no walkable cell in front of it can't be reached on foot. That is a
  finding, never a route through the rack.
- **Room for a trolley.** A trolley run counts only aisles wide enough for
  its trolley, from the trolley's width (Question 3). A basket goes anywhere.

**Why.** D173 rejected measuring every place, rightly. Distances only need
one more number, and the obstacles are already drawn. Without the real
layout (handoff step 5) every route runs through a fiction, so applying the
real layout comes first.

### Proposal C: Walking distance is A\* on the floor, kept as a matrix per site

*Adopted as D211, with distances worked out per walk rather than kept as a
matrix, and no climbing cost yet.*

**Decision.**
- **Two points.** The distance between two points is A\* on the walkable
  grid: eight neighbours, the octile heuristic, a straight step costing 1 and
  a diagonal √2, times the scale. No diagonal step cuts the corner of a solid
  cell, so a route goes round the end of a rack, not through it.
- **The whole site.** For planning, each access point and each special place
  (the pack station, the dock, the doors) gets one Dijkstra across the grid.
  That gives a matrix of every distance, with a parent map to draw any path.
  Melbourne has about 600 access points on a grid of a few thousand cells, a
  sub-second build. It is cached by layout version and rebuilt when the
  layout changes.
- **Climbing.** A stop above reach (D180) adds a fixed cost, and forklift
  work is planned as its own run (Proposal E).

**Why.** A\* is exact and fast for the question "how far from here to
there", and it can draw the path. It does not order the stops. That is Proposal D.

### Proposal D: A route visits its stops in the order that walks least

*Adopted as D211. Answers the task-ordering half of Q28 and Q42: the computed
route replaces the interim sequence.*

**Decision.**
- **The problem.** Order a run's stops to minimise the walk from its start
  (the pack station, or the picker's last scan) through every stop to its
  drop-off. It is a small travelling-salesman problem over Proposal C's matrix.
- **The method.** Up to 12 stops, solve it exactly with Held-Karp. Above
  that, use nearest insertion improved by 2-opt and Or-opt, with a budget of
  about 20 ms. The result is shown as metres and minutes.
- **The old order.** `pick_sequence` stays only as the order for bins not
  yet on the layout.

**Why.** It removes the typed number as the route. Melbourne's runs will
mostly have under 30 stops, where these methods are within a few percent of
optimal. A vehicle-routing solver is far more than this needs.

### Proposal E: Picking is done in runs

*Proposed, not adopted. Adopts D15's `pick_batch` and D17's `work_task` and
`receptacle_assignment` as written. Answers Q43.*

**Decision.** A run is a `pick_batch`: one picker, one carrier, one route,
and tasks in route order. It comes in four kinds:

| Kind | What the picker does | When |
|---|---|---|
| **single** | One order, carried to the bench | A big or urgent order, or hands and a basket |
| **cluster** | Several orders on a trolley, one tote each | The usual run: orders whose stops are close |
| **zone** | One stretch of aisles across many orders | Two or more pickers working at once |
| **batch** | Several orders onto a trolley with no dividers, sorted at the bench | Tiny orders, or many of one item |

Pallet and forklift runs are single or zone runs whose carrier is a pallet
and whose stops may be above reach.

- **A task.** One visit to a bin. Its allocations (D12) say which orders it
  serves and so which totes.
- **Tote slots.** `receptacle_assignment`, as D17 drew it.
- **Closing a run (Q43).** A run closes when every task is completed,
  failed or cancelled and its carrier is dropped at the pack station. If it
  is abandoned, its open tasks go back to the pool. Claims carry who made
  them (Q175).

**Why.** D15 and D17 already argued the shape. What was missing was a
reason to build it.

### Proposal F: The planner allocates when it builds a run

*Proposed, not adopted. Answers Q26.*

**Decision.** The planner suggests runs from the open work, and a picker
taking one is when its stock is allocated.

1. **The carrier.** Each order is sized by its recorded volume and weight
   (the D195 figures), falling back to its line count. That chooses its
   carrier: a basket, a trolley tote, or a pallet.
2. **Which bins.** For each line, the bin is chosen by what it adds to the
   route, within first-expiry for lots, reach, and what is available. Stops
   then change bins wherever that shortens the route. This is D13's travel
   weight, now computable.
3. **Which orders.** Orders are grouped by the walk their stops share,
   urgent ones first: seed with the most urgent order, add the order whose
   stops add least walking per line, and stop when the totes are full or the
   run reaches its time (about 15 minutes, learned later).
4. **Several pickers.** With two or more working, the planner offers zone
   runs: aisle stretches balanced by estimated time, so pickers rarely share
   an aisle. A picker is never fixed to a zone. Zones are only how the work
   was split this time.

The picker sees one suggestion, with "Something else" behind it. A
supervisor can hold an order back or rush it, and the planner takes it
from there.

**Why.** D13 says the score is code and only its weights are data. With
distances, travel finally has a value. One to four people makes this a
heuristic problem, not an optimisation service.

### Proposal G: Each site has a live channel

*The channel is adopted as D206. The kinds of event and claims are still
proposed. Builds D17's "real-time channel".*

**Decision.**
- **The channel (D206).** `GET /changes` is a server-sent event stream for the
  device's own site, fed by Postgres `LISTEN/NOTIFY` from triggers on the
  writes. Today it says only that something changed there, and the device
  reads again what it shows. With runs (step 4) it will also carry:
  - a task claimed or finished;
  - a run started or dropped;
  - a picker's last scan, which is their position;
  - an order ready to pack.
- **Reconnecting.** A device that was away reads once when it is back,
  because every read is live. So there are no event ids to pick up from.
  Polling is the fallback when a stream can't be held open.
- **Claims.** A claim is first-claim-wins and the loser is told at once
  (D17). It is a courtesy, not a guarantee: two picks of one task are still
  facts, and the double pick is a finding (D8).

**Why.** Nothing live exists, and collaboration without it means refreshing
and guessing. Server-sent events go one way, which is all the floor needs.
Every write still goes through the existing acts.

### Proposal H: Orders meet at the bench

*Proposed, not adopted. Extends D202.*

**Decision.**
- **Ready to pack.** An order is ready when every unit it commits is at the
  pack station: dropped in a tote, or in a run's batch. The bench says so,
  and for an order still on its way it says which run and picker has the
  rest and roughly when they will arrive.
- **Sorting a batch run.** At the bench, each scan says "IF271645, tote 3"
  or starts that order's carton. The bench is the sort, with no lights or
  put wall.
- **The whole-order view** counts what is in totes the way it counts cartons
  today (D202).

**Why.** The bench already answers "is anything missing" for one order.
Consolidation is the same question across runs.

### Proposal I: The map shows bins, stock, routes and people

*The bins, two layers and search are adopted as D208. Routes, people and
replay are still proposed. Extends D173's 3D view.*

**Decision.**
- **On the 3D map:**
  - each bin is a cell on its rack;
  - a choice of layer colours them: fill, pick frequency, count age,
    findings;
  - search flies to a bin;
  - a run's route is drawn on the floor;
  - pickers' last-known positions move live (Proposal G);
  - a finished run can be replayed.
- **On the handheld:** a 2D mini-map of the route and where to stand
  (W1, W4). 3D is only on request.

**Why.** The map is a reader of the same layer as everything else: places,
cells, stock, routes. Its bins come first, because routes need the same real
layout and scale, and walking the floor with the map is how the layout gets
checked. Its live parts (routes, people, replay) come once the routes and the
channel exist to draw.

## Rejected

- **A\* as the route optimiser.** It measures the distance between two
  points. The route is the order of the stops (Proposal D). Treating the two as
  one would make the plan wrong on its first page.
- **A vehicle-routing solver** (OR-tools or similar). Too much for one to
  four pickers and runs of under 30 stops. Proposal D's methods are a function
  with a stated contract, so they can be swapped if the scale changes.
- **Fixed zones.** With one to four people, a fixed zone means someone
  waiting. Zones are drawn for each wave (Proposal F).
- **Indoor positioning hardware** (beacons, UWB). The last scan is accurate
  enough to say where somebody is and when they will arrive.
- **Pick-to-light and lit put walls.** That is hardware for volumes Melbourne
  doesn't have. The bench's scan does the sorting (Proposal H).
- **3D as the handheld's main view.** It is slower to read than a 2D strip
  (W1 to W4).

## Risks

- **The layout has to be right.** A wrong rack position gives a confident
  wrong route. Apply the real layout and walk it once, with a handheld, to
  check access points.
- **Wifi dead spots.** Picking is the workflow most exposed (floor-devices.md).
  Picks are now kept on the device until the server has them (D207), so a dead
  spot or a reload no longer loses one. Other acts are still held in memory.
- **NetSuite has to hear what happened.** Today it is where picking happens.
  Moving picking means Spork's results must reach it (Question 1), and that path
  is still open. Under D212 it is what keeps NetSuite's shelves true.
- **Pickers trusting it.** The first runs should show the walk saved
  against the typed sequence, so trust is earned rather than assumed.

## Sequence (reasoning, not a commitment)

Each step is usable on its own and ends with something a picker or packer
can see.

1. **Groundwork.**
   - Fix the three picking bugs above.
   - Add people to a workspace, so each picker signs in as themselves.
   - Build the live channel (Proposal G).
   - Build the durable outbox for picking (D170, Question 5).

   Done when two handhelds see each other's picks within a second, and a
   pick made offline survives a reload.
2. **The real layout and the 3D bin map.**
   - Apply the real layout and set the scale (Proposal B).
   - Draw each bin as a cell on its rack in 3D, with what it holds and a
     search that flies to it (the first part of Proposal I).

   Routes need all of this, so the bin map is built first, not last.

   Done when every bin a picker might go to is on the map where it really
   is, and checked by walking the floor once.
3. **Distances and routes** (Proposals C and D). *Built as D211, on the
   drafted layout; exact once the floor is measured (D210).*
   - Build the walkable grid, A\* and the matrix.
   - Today's single walk is ordered by distance, and the path is drawn on
     the plan and on the 3D map.

   Done when the walk's metres are shown against the typed sequence's, on
   real orders.
4. **Runs for one picker** (Proposals E and F).
   - Single and cluster runs with tote setup.
   - The planner offers the next run.
   - Picking moves to Spork for those runs (Proposal A).
   - The bench knows an order is ready (Proposal H).

   Done when a trolley run of several orders is picked and packed with
   nothing unaccounted for.
5. **Several pickers.**
   - Zone and batch runs.
   - Live claims and positions.
   - Sorting at the bench.
   - The supervisor's screen.

   Done when three people work the same morning's orders without a double
   pick.
6. **The rest of the map** (Proposal I). Colour layers, the live route,
   people moving, and a run's replay, which need the live channel and the
   runs.
7. **Smarter.**
   - Learn walking speed and seconds per pick from the acts' timestamps,
     for honest time estimates.
   - Suggest re-slotting fast movers nearer the bench (a GROUP BY over
     `stock_movement`, per competitor-analysis.md).
   - Waves timed against carriers' cut-offs.

## How we'll know it works

- **Walking:** metres per line and per order, from the routes, against the
  typed sequence on the same orders.
- **Throughput:** lines picked per hour per picker.
- **Accuracy:** wrong-item scans caught at the bin, and orders packed with
  nothing unaccounted for (D202's tally).
- **Speed:** time from release to ready to pack, against promise dates.

## Questions this answers

- **Q26:** the planner allocates when a run is taken (Proposal F).
- **Q28 and Q42:** the computed route replaces the interim sequence (Proposal D).
- **Q43:** a run closes when its tasks are terminal and its carrier is
  dropped (Proposal E).
- **Q175:** claims and tasks carry who made them (Proposal E).
- **Q177:** a picker's work session is declared at sign-on, and a run's tasks
  name it (Proposal A).

## Open questions this raises

Numbered here only; each takes a Q-number in the register when the proposal
raising it is adopted.

- **Question 1. How does NetSuite learn what Spork picked, packed and shipped?**
  Today an item fulfilment is created in NetSuite when it is picked, and the
  bridge brings it here. With picking in Spork, orders have to arrive before
  any fulfilment exists, from the sales orders (the `import_orders` loader
  reads that export already). The results then go back by the packer
  marking them in NetSuite, or by the bridge writing them back. This needs a
  business answer before step 4. **D212 makes it the critical path:** NetSuite
  keeps the shelves, so it has to hear every pick Spork records. The route,
  the Bridge or a RESTlet with token-based authentication, is still open, and
  waits on NetSuite permission the user is asking for (2026-10-05).
- **Question 2. How long is a layout cell at Melbourne?** One measurement, taken
  with the real layout: a bay's width and an aisle's width.
- **Question 3. The trolleys and totes.** How many totes fit a trolley, and what
  size are they? How wide is a trolley? That sets a cluster run's capacity, by
  count and volume, and which aisles it fits.
- **Question 4. Any standing preferences for zones?** For example, does one person
  usually work the forklift? The planner assumes no fixed zones (Proposal F).
- **Question 5. Is the durable outbox a prerequisite for picking going live?**
  Yes, and it is built for picks (D207).
- **Q29, revisited.** One forklift is a resource the planner hands out. Is
  contention for it real enough to model now? Deferred until pallet runs
  exist.

## Supporting documents

- [domain-model.md](./domain-model.md): D11 to D17, D46, D166, D172, D173,
  D180, D195 to D202.
- [invariants.md](./invariants.md): J56 (picked past covered), J71
  (duplicate walk order).
- [open-questions.md](./open-questions.md): Q26, Q28, Q29, Q42, Q43, Q175,
  Q177.
- [layout.md](./layout.md) and
  [layout-interface-analysis.md](./layout-interface-analysis.md): places,
  cells, and rules W1 to W9.
- [floor-devices.md](./floor-devices.md): the handhelds and the dead spots.
- [warehouse-data-model.md](./warehouse-data-model.md): the 3D map and route
  optimiser as readers of one data layer.
