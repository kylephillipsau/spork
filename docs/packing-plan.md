# Packing in Spork: checking, the board, pallets and the planner

**Status: adopted 2026-10-14 as the build plan for D244 to D252.** The
decisions are in [domain-model.md](./domain-model.md); this document is how
they get built, in what order, and how each part is known to be done. Where
it settles something the decisions left open, the settling takes its
D-number, or amends the decision it touches, in the commit that builds it.
Until then the registers win wherever this document disagrees with them.

## The short version

Feature complete is everything in D244 to D252. The work is done in the
agreed order, seven milestones:

1. **M1:** the rest of the server under D246 and D247.
2. **M2:** boxes cut to their heights, and box photos (D252).
3. **M3:** the drag-and-drop board (D245).
4. **M4:** load limits and carrier build rules (D249, D250).
5. **M5:** the planner (D251).
6. **M6:** breakdown pallets through the day (D247, D248).
7. **M7:** the finish.

Each milestone is live before the next one starts, and each ends with an
order packed on it for real.

Booking stays in NetSuite and MachShip: the bench hands over sizes and
weights (D224). These are not part of this plan:

- booking through a carrier's API;
- SSCC labels on pallets (D29);
- pricing freight by carriers' rates;
- one pallet carrying two customers' orders as one parcel. A breakdown pallet
  carries many orders, and is in scope.

## Where it stands

| | Designed | Built |
|---|---|---|
| D244 checking at the bench | yes | live 2026-10-12 |
| D246 moving units between cartons, cartons onto pallets | yes | server done in M1, 2026-10-14; the bench doesn't use it yet |
| D247 a pallet goes as one or is broken down | yes | server done in M1: what is delivered, booked and despatched; the sheet and booking line in M6 |
| D245 the board | yes, mocked | no |
| D248 a breakdown pallet through the day | yes, mocked | no |
| D249 what a box carries | yes, with research | no |
| D250 carrier rules as settings | yes, mocked | no |
| D251 one planner | outline | no |
| D252 cut-down boxes, box photos | yes, mocked | no |

The mockups are the canvas "Pack bench",
https://claude.ai/artifact/9eMaJSZBNHoRe5KHjTq5TY.

## Settled here

The decisions left these open. They are settled here as follows, and each is
recorded when it is built.

1. **Which breakdown pallet an order goes on.** The open breakdown pallet,
   which is the usual carrier's almost always, without asking. The board
   offers another breakdown pallet, or a new one, as a choice. Spork does
   not read an order's carrier from NetSuite.
2. **Taking out means onto the bench.** Taking units out of a carton moves
   them to the site's pack location, still on their line: out of the carton,
   still on the bench. They are packed again from there, or moved back to
   their bin by an ordinary move. Taking out used to reverse the pick, which
   was wrong after a repack. It reversed the repack, and so sent units back to
   the carton they came from. An undo straight after a pick is still a
   correction.
3. **A short at the check is a finding.** The packer's count is recorded, the
   plan packs what is there, and the line's shortfall goes into the findings
   queue against the line, naming who picked it, there or in NetSuite (D244).
4. **Where the plan is kept.** The board's plan is a draft per fulfilment on
   the server (`pack_plan`), overwritten as it changes and not a fact. It is
   what the packer placed: a line's units to a parcel, a parcel to a pallet, a
   preset and cut for a planned parcel, a spot on a pallet. A planned parcel
   holds the id its carton will be made with. The draft is dropped when the
   fulfilment closes.
5. **The board's handling**, which the mock shows and D245 states:
   - **A drop puts in as many as fit.** As many as fit by the planner go into
     the parcel and the rest stay where they were. Shift and drop puts in one.
     Alt and drop asks how many.
   - **Undo** goes back one step. A change to the plan is undone in the
     browser. A recorded act is undone by its opposite act: a repack back, a
     carton taken off a pallet, units taken out to the bench.
   - **The keyboard does everything a drag does:**
     - the arrows move between lines and parcels;
     - Space chooses, and Shift with the arrows chooses more;
     - M opens Move to…, where you type a count and press Enter;
     - Delete takes something out to the bench;
     - Z undoes.
   - **Several at once**: Shift or Ctrl and a click chooses more than one, and
     they are dragged together.
6. **The planner is judged on numbers** (D251):
   - **The test set** is the open and recent real orders from the local data,
     kept as fixtures, plus a set of breakdown pallets built from them.
   - **For boxes**, each order is scored on parcels, chargeable weight and
     fill.
   - **For pallets**, each is scored on:
     - its height;
     - its centre of gravity, how high and how far off centre;
     - the most loaded box against its limit;
     - support;
     - whether every must is kept.
   - **It must be at least as good** as `arrange.ts` on every order, and better
     on most. It must answer within a second for an order, and within three
     for a full pallet, in a worker.
7. **Despatch with pallets.**
   - A pallet that goes as one is consigned as itself, and its despatch is
     every carton on it and the pallet, one act each, as a carton's lines are
     today.
   - On a breakdown pallet, each carton is consigned with its order. The
     pallet is despatched when it leaves.
   - Despatch shows a pallet with the cartons on it.
   - The pallet sheet is mocked in M1 and built in M6.
8. **The data a trial needs.** The trial orders must have their items
   measured. Before each trial, the packing worklist (D197) is worked down for
   the orders chosen.

## How each milestone is known to be done

Every milestone has all of these before it goes live:

- **Server:**
  - every new act's rules are unit tested, hard and soft;
  - every new act has an HTTP test;
  - every new fact has an invariant in the register;
  - the full suite passes on a fresh database, and every migration reverses.
- **Client:**
  - `npm run verify` passes;
  - every new state has a fixture, and the render gate passes over all of
    them, light, dark, phone and wide;
  - every drag has its keyboard way.
- **Docs:** the decisions it builds are marked built; the handoff, README and
  architecture counts are brought up to date.
- **Live:** committed, pushed and running on the LAN server.
- **Tried:** an order is packed on it for real, and what the user says is
  taken into the next milestone.

## The milestones

**M1: the rest of the server (D246, D247)**

*Built 2026-10-14 (migration 140, J79 to J81, two boards on the canvas). Its
try at the bench is the short finding; the rest waits for M3's board.*

- Despatching a pallet with everything on it. The despatch read lists what is
  on each pallet, carton by carton, with their lines.
- Consignments carry what is delivered: a pallet that goes as one, or a
  broken-down pallet's cartons. Nothing is consigned both as itself and
  inside something delivered as one.
- Invariants:
  - no package inside itself;
  - three deep at most (`nesting_too_deep`);
  - nothing consigned twice that way.
- The bench read:
  - each parcel's parent;
  - each parcel's kind: a box, a pallet or skid, or one of a product as it is;
  - whether it breaks down;
  - pallet presets give their footprint and build height.
- Taking out moves units onto the bench (settled 2).
- A short at the check is a finding (settled 3).
- The despatch screen with pallets, and the pallet sheet, mocked.

**M2: boxes cut to their heights, and box photos (D252)**
- `package_type.cut_from_id`, with Workspace › Boxes' heights, adding and
  taking away. The user's list is loaded, and the half box is linked to its
  full box.
- The suggestion, the bench and the booking use the cuts. The bench says
  which height to cut a carton to.
- Box types are photographed. The observable's package-type arm is added,
  with Photos and cutting faces as items have them.
- A cut box is drawn from its full box's photographs, from the bottom edge
  up, never squashed.

**M3: the board (D245)**
- The kit's pointer drag, with its keyboard way and choosing several.
- The board replaces the forms:
  - check and To pack on the left;
  - the parcels in the middle, pallets holding cartons;
  - 3D on the right, following what is chosen, with a ghost while dragging
    and the verdict.
- Planned and made parcels, the plan kept as a draft (settled 4), Make, and
  Suggest planning round what was placed.
- Drops, undo and the keyboard as settled in 5. Moves between cartons and
  onto pallets go through M1's acts.
- A fixture for every state the mock shows, and more:
  - a drag in progress;
  - a parcel that won't take it;
  - an order pallet;
  - undo.

**M4: load limits and carrier build rules (D249, D250)**
- `subject_load` on items, said and inherited as "this way up" is, on the
  item page.
- A box type's wall and lid limits:
  - its grade, and the estimate from size and grade;
  - set from a test;
  - the warehouse's conditions, in Workspace.
- A carton is solid or hollow, said at sealing, as the plan proposes.
- `build_rules`:
  - the fixed list of rules, each must or prefer, with a weight;
  - Workspace › Build rules;
  - a carrier, or one of its services, naming its rule sets;
  - a pallet keeping the set it was started under.
- The two carriers' rule sets are entered from what the user said.

**M5: the planner (D251)**
- One engine in a worker, for goods into boxes and boxes onto pallets.
- **Inputs:** the container, the rules, the things to place, and the spots
  already fixed.
- **Musts:** footprint, overhang, height, weight, upright, support, load, and
  labels where they must show.
- **Preferences:** by weight.
- **Output:**
  - a spot for each thing;
  - a build order, bottom up and back to front;
  - a report of every rule with its figures.
- The same check runs on what the packer does by hand.
- It replaces `arrange.ts` once it passes the test set (settled 6).
- **On pallets:** a whole order planned onto pallets.

**M6: breakdown pallets through the day (D247, D248)**
- Despatch › Breakdown pallets:
  - one open pallet for each rule set, started from there or from an order;
  - the order's cartons offered the open pallet by default, another by
    choice (settled 1);
  - spots planned round what is on already, and confirmed;
  - which side to label, said at sealing;
  - "Full: wrap it and start another".
- The queue can be ordered heaviest and largest first.
- The pallet sheet.
- The booking line per thing delivered: a pallet's weight from what is on it
  plus its own, or typed; its height from the plan plus the base, confirmed.

**M7: the finish**
- A full trial: one day's orders packed on the board, one breakdown pallet
  built and despatched.
- What the trial shows is fixed.
- Speed:
  - the board answers a drag within 100 ms;
  - the bench opens within a second on the largest open order;
  - the 3D stays smooth with a full pallet on it.
- The handoff, the README, the architecture and the decisions are all marked
  built.

## How we'll know it works

- **Before a carton is sealed:** a wrong count is caught at the check or by
  the weight.
- **On the board:** an order is packed without the forms, and without
  reaching for the mouse when the keyboard is quicker.
- **On a breakdown pallet:**
  - it passes its carrier's must rules as it is built;
  - no label is hidden;
  - no box carries more than its limit.
- **The booking** is copied from the bench without anything worked out by
  hand.
- **The user** packs a day on it and signs it off.

## Risks

- **Measurements.** The planner is only as good as the sizes and weights
  recorded. Most items still lack an each size (D197), so trials depend on
  measuring.
- **Planner speed in the browser.** It is held to the limits in settled 6.
  If it can't meet them, it runs fewer tries, not slower.
- **The 3D with a full pallet** of photographed boxes. Textures are shared
  per kind and capped in size, as `pack3d.ts` does.
- **Rules not yet confirmed with the carriers.** The usual carrier's own
  rules, and whether the second carrier allows overhang front and back, start
  from what the user said. They are settings, so changing them is not a
  build.

## Still open, none blocking

- The grade stamped on the box stock, and a floor test of each box's walls
  and lid. Until then the estimates and the 10 kg lid stand (D249).
- The usual carrier's own pallet rules, and the second carrier's front and
  back overhang (D250).
