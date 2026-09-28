# Writing a site's layout

A site's layout is two CSV files, loaded on the Import screen as **Racks** and
**Floor areas** (D173). Both are dry-run first, like every import, and both can
be loaded again: a rack or area is matched by its site and code, and a file
that changes nothing reports nothing.

The site must already exist: load its bin list first. A layout never creates a
warehouse, and racks never create bins. They place the bins the bin list has.

## The frame

Pick one corner of the building as the origin and measure everything in
millimetres from it: `x` to the right on the plan, `y` up the page. Heights are
from the floor. Every number is a whole number of millimetres.

## Racks

One row per run of bays along one aisle face. Back-to-back racks are two rows.

| Column | Required | Meaning |
|---|---|---|
| `site` | yes | The site's code, as the Workspace shows it. |
| `rack` | yes | This rack's code, unique within the site. |
| `aisle` | yes | The aisle its bins' codes start with. Every code the template makes must read as this aisle. |
| `x_mm`, `y_mm` | yes | The corner at the left end of the rack's front, standing in the aisle facing it. |
| `rotation` | yes | `0`, `90`, `180` or `270`, counter-clockwise. At `0` the rack runs to the right along `x` and is faced from lower `y`. |
| `depth_mm`, `height_mm` | yes | Front to back, and floor to top. |
| `upright_mm` | no, `0` | The frame between bays and at each end. Bay widths are clear widths. |
| `first_bay` | no, `1` | The number of the leftmost bay, facing the rack. |
| `bay_step` | no, `1` | How the number changes bay to bay: `2` when odd bays face one side of the aisle and even the other, `-1` when they count down. |
| `bay_widths_mm` | yes | Each bay's width, left to right. `12x2700` is twelve bays of 2700; `12x2700 1800` adds a narrow one. |
| `first_level` | no, `1` | The number of the bottom level. |
| `level_z_mm` | yes | Each level's floor height, lowest first: `0 1500 3000`. A level reaches up to the next; the top one reaches `height_mm`. |
| `positions` | no, `1` | How many bins share a bay at each level, one per level (`3 1 1`), or one number for all of them. |
| `template` | yes | How the bins are named. See below. |

**Templates** are the bin code with the numbers replaced by fields: `{aisle}`,
`{bay}`, `{level}` and `{position}`. Pad a number with a leading-zero width
(`{bay:02}` makes `07`) or letter it (`{level:A}` makes 1 `A`, 2 `B`). A
template needs `{bay}` and `{level}`, and `{position}` when any level holds more
than one bin per bay.

```csv
site,rack,aisle,x_mm,y_mm,rotation,depth_mm,height_mm,upright_mm,first_bay,bay_step,bay_widths_mm,level_z_mm,positions,template
NORTH,C-S,C,59890,7100,180,1100,6500,90,39,-2,20x2700,0 1600 3200 4800,3 1 1 1,{aisle}-{bay:02}-{level}-{position}
NORTH,C-N,C,4000,9100,0,1100,6500,90,2,2,20x2700,0 1600 3200 4800,3 1 1 1,{aisle}-{bay:02}-{level}-{position}
```

Aisle C is 2 m wide, between `y` 7100 and 9100, with a rack on each side
running from `x` 4000 to 59890 (20 bays of 2700 and 21 uprights of 90). C-N is
faced from the aisle below it, so it takes rotation 0 and its left end is the
west end. C-S is faced from the aisle above it, so it is turned half round, and
its left end, as you face it, is the **east** end: that is its `x_mm`. Its bays
count down from 39 so that bay 1 is opposite bay 2. Positions within a bay also
run left to right as you face it, so on C-S position 1 is at the bay's east end.

**What the report says.** For each site: how many slots the racks make, how many
bins were placed, moved or left alone because they were measured, and two lists
worth reading before applying:

- slots whose code is no bin on file, which usually means the template spells
  codes differently from the bin list, or the racks describe bays the bin list
  does not have;
- bins in an aisle a rack runs along that no rack names, which is J77.

A rack loaded again with fewer bays takes back the boxes it gave; a bin with a
measured box keeps it.

## Floor areas

One row per area.

| Column | Required | Meaning |
|---|---|---|
| `site` | yes | The site's code. |
| `area` | yes | This area's code, unique within the site. |
| `kind` | yes | `dock`, `staging`, `packing`, `walkway`, `wall`, `floor_stack`, `office` or `restricted`. |
| `outline` | yes | Its corners in order, as `x,y` pairs separated by spaces. Quote the cell, because it contains commas. At least three corners, and no edges crossing. |
| `height_mm` | no | How tall, for anything drawn standing: a wall, a floor stack. |
| `location` | no | The code of the location this area is, such as a staging bin, so what is held there can be drawn there. |

```csv
site,area,kind,outline,height_mm,location
NORTH,STAGE-1,staging,"500,500 6500,500 6500,3000 500,3000",,STAGE-1
NORTH,WALL-S,wall,"0,0 60000,0 60000,200 0,200",6000,
```
