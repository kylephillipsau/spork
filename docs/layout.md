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

On **Settings › Layout**, a site with bins and no layout offers **Draft from bin
list**. It groups the bins by the shape of their codes, proposes a place for
each family with a grid and a pattern, and shows what it would make before
making anything. Bins whose codes follow no pattern (`3PL`, `ASSEMBLY-BIN`) are
listed as not on the layout, and wait to be placed by hand. Run it again later
and it only puts new bins in: into places already drawn if their pattern names
them, otherwise into new places. It never moves a bin that is already in a cell.

## Where it shows

Scan a bin label, or type it into the search box, and Spork opens the **rack
face**: the way into the place, the front of the rack with the bin's cell lit,
labelled the way the rack's own labels read, and a small plan of the building.
The plan is always drawn the same way up.

## Not built yet

- The plan editor, for drawing and arranging places, and the 3D view.
- Correcting a bin's cell by scanning it at the shelf.
- A history of who changed the layout, and roles for who may.
