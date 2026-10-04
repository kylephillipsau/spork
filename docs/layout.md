# How the layout works

A site's layout is **places**: boxes drawn inside other boxes (D173). Nothing is
measured. This page says what a place is, how bins get into one, and what is
built so far.

## Places

A place is a box inside another place, or standing on the site. It has:

- **A name**, which is whatever people call it: *Main*, *Rack C*, *Side wall
  shelving*, *Returns (reserved)*, *Mezzanine*.
- **Walk-through or solid.** A room, a walkway or a reserved area is
  walk-through; racking, a wall or a column is solid. It is the only thing about
  a place the system reasons with.
- **A box in its parent's cells**: where its corner is, how long, deep and tall
  it is, how far up (a mezzanine), and how far it is turned. Positions are
  estimates. Only relationships are exact.
- **An outline**, optionally, when a walk-through place is not a rectangle: an
  L-shaped building, a notch, an angled wall.

A place's position on the site is never stored. It is worked out from its
parent's, and its parent's, when it is read, so moving a building moves
everything in it.

## Grids and bins

A place can hold a **grid**: bays along its length, levels up, rows into its
depth, and, level by level, how many bins share a bay. **Every bin sits in one
cell of one place**, and one cell holds one bin. A single floor spot is a place
with one cell.

A **naming pattern** says what a place's bins are called, so the bins from the
bin list drop into the right cells:

| Pattern | Names bay 7, level 3 |
|---|---|
| `C-{bay:02}-{level}` | `C-07-3` |
| `C-{bay}.{level:A}` | `C-7.C` |
| `D-{bay:02}-{level}-{position}` | `D-07-3-1` (position 1) |

`{bay:02}` pads to two digits and `{level:A}` letters the level (1 is A). The
numbers on the labels start where the place says and can step by two, for racks
numbered odd on one side of an aisle and even on the other, or count down.

## A first layout, from the bin list

On **Inventory › Warehouse**, a site with bins and no layout offers **Draft from bin
list**. It groups the bins by the shape of their codes, proposes a place for
each family with a grid and a pattern, and shows what it would make before
making anything. Bins whose codes follow no pattern (`3PL`, `ASSEMBLY-BIN`) are
listed as not on the layout, and wait to be placed by hand.

Each place in the preview has a tick box. A lone code can look like a rack of
its own when it is no rack at all. Untick that place and it is not made: its
bins join the ones not on the layout, and the counts and the button follow the
ticks.

## A rack with two sides

Racking often has bins on both faces, numbered round the rack: `E-01` to
`E-18` along the front, then `E-19` to `E-36` back along the other side, so
`E-36` is behind `E-01`. Walk round to the back and it still reads left to
right, `19` to `36`.

A place's grid says how many **sides** it has, one or two, and a bin's cell
says which side it is on. A cell's bay is its column along the rack, counted
from the front's left as you face the front, so the bin behind another is the
same column on the other side. The back's labels follow from the front's:
numbered round, they run on from the front's last, the other way along. Rack E
is one place eighteen columns long, and `E-36` is the back of its first column.

The bin codes cannot say a rack has two sides, so the draft's preview asks.
Tick **Two sides** on a rack, or press **Two sides for every rack**. With an
odd number of bays the front takes the one over.

Scan `E-36-01` and the rack face opens on the back, drawn as you would see it
standing there: 19 to 36 left to right, `E-36` lit at the end behind `E-01`.
**Front** and **Back** switch between the sides, and the plan below marks the
aisle to stand in. The bins list says "back, bay 36, level 01". Run it again later
and it only puts new bins in: into places already drawn if their pattern names
them, otherwise into new places. It never moves a bin that is already in a cell.

## Where it shows

**Inventory › Warehouse** lists the site's places, each under the one it is
inside, beside a plan of the whole site. Choose a place on either and its bins
are listed with their cell in the rack's own words ("bay 03, level 2"), what
NetSuite's last inventory balance put on the shelf, and what Spork's own ledger
holds there. The two are separate columns because they are separate records. A
search finds a bin anywhere on the site, and the bins no pattern fitted are
listed as not on the layout.

Beside the plan, **Show 3D** draws the same site in three dimensions. Solid
places stand up as blocks with their bays and levels drawn on them, and
walk-through places lie flat as floor. Choosing a place in either view chooses
it in both and on the list. It turns by dragging, and moves across the floor by
dragging with the middle button, or with Ctrl (⌘ on a Mac) held. It zooms by scrolling over it, by
pinching, or with the buttons. Nothing is edited in 3D. It is there to confirm what the
plan says, the way a glance across the floor would. The browser remembers
whether it was shown, and it starts hidden on a narrow screen.

Scan a bin label, or type it into the search box, and Spork opens the **rack
face**: the way into the place, the front of the rack with the bin's cell lit,
labelled the way the rack's own labels read, and a small plan of the building.
The plan is always drawn the same way up.

## Editing the layout

**Inventory › Warehouse › Edit layout** shows the site from above. Drag a
place to move it, half a cell at a time, or nudge it with the arrow keys. Turn
it a quarter at a time with R or the buttons, and it turns about its middle.
Its front is marked, and a rack with two sides has its back marked too.
Beside the plan are its name, what it is (solid or walk-through), and its
position and size in cells.

You can add a wall, a column, a dock, a packing station or an area. You can
take away a place that holds no bins.

Nothing is saved until **Save**, and **Undo** takes back one change at a
time. Bins move with their racks, because each is in a bay and a level of its
rack wherever the rack stands. A rack's bays and levels can't be changed here.
If somebody else changed the layout since you opened it, the save says so,
and you open it again to see their change.

Every change is kept: what the place was, what it became, who did it, and
when.

## Measuring the floor

The draft's racks are in the right order but not the right places, and there
is no floor plan to copy. So the floor is built from measurements. Press
**Measure in metres** once: from then on every number in the editor is in
metres, from the front left corner of the place it is inside.

1. **The building.** Choose it and type its inside length and width.
2. **Each rack make.** Choose a rack and, under **Size from its bays**, type a
   bay's width from upright centre to upright centre, one side's depth, and a
   level's height. Tick the box to size every rack of the same make too.
3. **Each row.** Choose the racks in it (Shift-click, or **Choose every
   rack**). Under **Set out in a row**, type how far the first is from the
   left and the front walls, and how wide the aisles are.
4. **The odd one out.** Choose it and type what the tape says under **Room
   round it**: to the wall, or to the rack across the aisle.
5. **Which way each rack faces.** The chosen rack's front is marked. If its
   first bay opens onto the other aisle, press **Face the other way**.
6. **Docks, packing stations, columns.** Add them and type where they are.
   A bin on no layout, like the packing bench's `PACK`, is listed under **On
   no layout**: press **Place** and it goes on the plan as a spot of its own.
   The picking walk starts and ends at the packing bench once it is there.

Then **Save**, and walk the floor with the bin map to check it.

## Not built yet

- Changing a rack's bays, levels or naming pattern, which moves bins.
- Correcting a bin's cell by scanning it at the shelf.
- Roles for who may change the layout. For now, anyone signed in may.
