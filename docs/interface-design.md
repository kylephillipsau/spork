# Interface design

The frontend, planned as a whole rather than accreted a page at a time. Companion
to [architecture.md](./architecture.md), [floor-devices.md](./floor-devices.md)
and [order-fulfilment-process.md](./order-fulfilment-process.md). Decisions here
take numbers from the shared register: D109 to D131 and D134 to D135 here, beside
D132, D133 and D136 in the domain record.

The bar this document is measured against is already written down, in D2:

> **one screen, no page loads, scanner/keyboard driven, sub-second interactions.**
> If the replacement needs as many views as NetSuite, it has failed even if it is
> functionally complete.

That is a statement about the interface, and until this document nothing had been
designed against it.

---

## Where we actually are

> **Status.** The plan below is largely built. `client/` holds the design
> package and the Pack screen, five gates run in CI, and the bundle is deployed
> beside the API. What follows is the design as it stands, with the corrections
> that got it there recorded rather than smoothed over — the material was wrong
> about its own finish four times, about its light three times, and about its
> edges three times, and every correction came from checking the physics or
> rendering the page.

When this document was written there were twelve server-rendered pages, one
two-hundred-line stylesheet, and a header bar with four links in it. That
understated the good parts and overstated the whole.

**What is genuinely right and survives unchanged.** Amber is reserved for findings
and nothing else, so a screen with amber on it is a screen with something to
chase. Every string a person reads off a label or says down a phone is monospace
with tabular figures, and prose is not. The base size is 17px and targets are 44px
because this is not a desk application. The packing page opens on the work rather
than on a search box, and its three groups are computed from quantities rather
than stored as a status. Each of those is a rule with a reason behind it, which is
rarer than it sounds.

**What is missing is the entire middle layer.** There is no navigation
architecture — four flat links is a list, not a system. There is no component
vocabulary beyond `.docket` and `.rows`, so every page reaches for inline `style=`
attributes to say anything the stylesheet did not anticipate. There is one shell,
`main { max-width: 62rem }`, serving what are really four different ergonomic
situations. Multi-tenancy has been in the schema since migration 1 and has never
had a pixel drawn for it. Findings are described as the most valuable output the
system produces and appear as the second of four undifferentiated links.

---

## Four surfaces, not one app

The single biggest structural error available here is to treat this as one
responsive application. It is four, and they differ in ergonomics rather than in
screen width — which is why breakpoints alone cannot express the difference.

| Surface | Device | Posture | Input | Governing constraint |
|---|---|---|---|---|
| **Bench** | Desktop browser | Standing, stationary, both hands sometimes free | Scanner wedge, keyboard, occasional mouse | Repetition. Several hundred a day, so every interaction is paid for hundreds of times |
| **Floor** | Honeywell handheld | Walking, gloved, one hand, low grip | Scanner trigger, thumb | Attention. The operator is looking at the rack, not the screen |
| **Desk** | Desktop browser | Seated, analytical | Keyboard, mouse | Density. Comparing evidence, deciding, moving on |
| **Document** | A4, label printer | Not a screen at all | — | Fidelity. It gets folded onto a pallet and read by a stranger |

Two consequences.

**The primary action sits at the bottom on Floor and at the top on Bench.** A
handheld is held at waist height and driven with a thumb; the reachable third of
the screen is the bottom third. A bench monitor is read top-down. Most warehouse
software puts the button in the same place on both and is wrong on one of them.

**Document is a first-class surface, not a print stylesheet.** Stage 5 of the
recorded process is an A4 sheet folded onto a pallet, and that observation is the
one real argument the current server-rendered pages make for themselves. It
survives intact. See D113.

---

## Navigation: three mechanisms, each with one job

Navigation is the thing NetSuite loses on — *"simple but convoluted across many
interface views, loading states, and extremely poor application UX design."*
Beating it is not a matter of having fewer screens. It is a matter of having three
well-chosen ways to move instead of one generic one.

### 1. The work rail — organised by job, not by entity

The entity-oriented menu is the disease. *Orders. Fulfilments. Packages. Items.
Locations. Stock.* It is a rendering of the schema, it requires the operator to
know which noun holds the thing they want, and it is why finding one order takes
four navigations today.

```
INBOUND      Receive          ← expected supply, arriving now
             Put away

OUTBOUND     Pick
             Pack       (4)   ← the count is work waiting, at your site

             Despatch

INTEGRITY    Findings   (7)   ← the output, not a tab
             Count
             Weigh     (12)

REFERENCE    Items · Presets · Locations · Policies · Carriers

YOU          Session, site, keys
```

Five groups, and the grouping is load-bearing rather than decorative. It means a
new screen has an obvious home, and it means the rail can be read by someone who
knows the warehouse but not the software. `Integrity` is the group that does not
exist in comparable products, and it is where this system's premise lives.

**Badges follow one rule, stated in D112: a badge counts work waiting for you, at
this site, now.** Never a total.

### 2. The scan bar — the universal locator

**This is the navigation system, and the rail is the fallback.**

One always-present input accepts anything the system can resolve — an SSCC, an
item barcode, a location code, a package barcode, an order or fulfilment reference
— and goes there. Three outcomes, and the third is the interesting one.

- **One subject.** Navigate. No intermediate results page, ever.
- **Several.** A disambiguation list, with the scanned string echoed verbatim in
  mono at the top.
- **Nothing.** D28 says *record resolution failures, never decode failures*. So a
  scan that resolves to nothing should not be an error toast — it should be a
  record the system writes. A warehouse full of unresolvable barcodes is a
  finding about the label estate, and today that information is thrown away at
  the point where somebody shrugs and types it.

  **It is still thrown away.** The locator built in D136 reports the outcome and
  keeps nothing, because the place to write it is `activity_event` and that table
  does not exist. The screen says so rather than implying otherwise, and it is
  on the build list below as the debt it is.

**Four outcomes rather than three, and the fourth is the one that costs.** A
well-formed identifier nobody holds is `identifier_unknown`; a string that is not
an identifier at all is `identifier_unrecognised`. Collapsing them tells an
operator "no such item" about a smudge, and — worse in the other direction —
tells them the same thing about two items when the truth is `identifier_ambiguous`.
That last is how a scan gets recorded against the wrong product, so nothing is
ever resolved by silent preference.

**Focus is claimed, never stolen** (D117).

### 3. The evidence panel — inspect without leaving

A finding is *"an owner, the evidence behind it and a resolution"*. You cannot act
on that from a list, and you should not have to lose your place to see it. A
right-hand panel on Bench and Desk, a bottom sheet on Floor, holding the pair that
disagrees: expected against observed, the difference, who saw it, when, on which
device, and the actions available. It opens over any amber row anywhere — findings
are an annotation the whole system carries, not a destination.

### What this replaces

The recorded process spans nine stages, two systems and about a dozen screens.
Under these three mechanisms it is: scan the confirmation number, land on the pack
screen, work, print. Stages 1, 2, 6 and 9 are navigation that stops existing
rather than navigation made faster.

---

## The screen map

Screens marked ○ have no endpoint yet.

### Outbound

| Screen | Surface | Does | Behind it |
|---|---|---|---|
| Pack queue | Bench | The work at your site, grouped by computed progress | `/sites/{id}/open-lines` |
| **Pack** | Bench | The whole of stages 3–8 on one screen. **Built** | `/fulfilments/{id}/bench` |
| Packing list | Document | A4, folded onto the pallet | `/app/packing-list/{id}` |
| Labels | Document | ZPL to the label printer | ○ |
| Pick list | Floor | One line at a time, in walk order | `/picks`, `bin.pick_sequence` |
| Allocate | Desk | Commit supply to demand, and release it | `/allocations` |
| Despatch | Bench | Consignment, manifest, what left today | `/consignments` |

### Inbound

| Screen | Surface | Does | Behind it |
|---|---|---|---|
| Expected | Desk | What is promised and when | ○ |
| Receive | Floor | Scan against the claim, record what arrived | `/receipts` |
| Disposition | Floor | Where it goes and in what condition | `/receipts` |
| Put away | Floor | Move it to a cell | `/moves` |
| Supplier reliability | Desk | Claim against receipt, over time | ○ |

### Integrity

| Screen | Surface | Does | Behind it |
|---|---|---|---|
| **Findings** | Desk | The queue. Evidence on the row, not behind it | `/discrepancies` |
| Finding | Desk | Investigate and accept, in the rail beside the queue | `/discrepancies/{id}/*` |
| Count | Floor | Count a cell, see the annotation | `/counts` |
| Correct | Bench/Floor | Net against what it corrects | `/corrections` |
| Adjust | Desk | With a reason, always | `/adjustments` |
| Projections | Desk | Freshness, and rebuild a tenant | `/projections/refresh` |

### Measurement

| Screen | Surface | Does | Behind it |
|---|---|---|---|
| Worklist | Bench | Never-measured first, then stale, ranked by demand | `/revalidation` |
| **Weigh** | Bench | One thing on a scale at a time | `/weighings` |
| **Capture** | Floor | Scan, weigh, measure, photograph seven faces | `/capture`, `/observations`, `/observations/{id}/images/{face}` |
| Measurements | Desk | The history, and what disagreed | `/items/{id}/measurements` |

### Reference and account

Items, presets (`/package-types`), locations and zones, policies (the D22 scope
lattice, which needs a real interface and does not have one), carriers, devices,
people and keys (`/passkeys`), sites and tenant switching, sign in and out.

**Roughly thirty screens.** Four are load-bearing — Pack, Findings, Weigh, Pick —
and that is where the design effort goes.

---

## The material system: Hard Anodise

> This section supersedes the paper-first system this document originally
> specified — condensed stencil type, two rule weights, kraft bands. That
> system was competent and anonymous, and it lost to a simpler test: it could
> have been any warehouse product. What replaced it is a **material** rather
> than a stylesheet, and the difference is that a material has physics, so its
> rules can be derived rather than asserted.
>
> Several of them were then derived wrongly, and the corrections are recorded
> here rather than smoothed over. Every one was found by checking the physics
> or by rendering the page, never by looking harder.

### What it is

**Bead-blasted, Type III hardcoat anodised aluminium**, lit by one lamp fixed
above and slightly left. The finish on a machine-tool spindle: matte,
near-black, isotropic, expensive without being shiny. Beside it one textile —
**ripstop nylon** — because the company sells protective equipment and is called
Nylonite.

And one reconciliation the whole thing turns on: **the chassis is metal and the
data is paper.** The enclosure can be as rich as we like because nobody reads
it; a gloved operator at arm's length under warehouse lighting reads quantities
and identifiers, so those sit on a bright matte instrument face with the
contrast of paper. Not a compromise between two ideas — what a Fluke meter, a
weighbridge terminal and an aircraft panel each arrived at independently.

**It is not skeuomorphism, and the line between the two is worth stating.** The
first drafts carried a relief groove, a bezel catch, chamfer rings and a 3mm
riser under every key: literal hardware, traced from a photograph of hardware,
and the most dated thing on the screen. Those are gone. The light model, the
grain and the occlusion stayed, because they are how a surface behaves rather
than what a surface has bolted to it. See D125.

### The six layers

Every element is one of these, and its layer decides its physics.

| | Layer | Physics |
|---|---|---|
| **L0** | Ground | The isometric hangar: hairline grid, racking at the aisle edges, the lamp's pool, a vignette. Drawn once to canvas. Never holds content |
| **L1** | Panel | Blasted, anodised, opaque. Carries the light model and both textures |
| **L1b** | Nylon | Ripstop weave, satin. Boots, straps, tags. Never carries a number |
| **L2** | Well | A pocket: shadow where the wall cuts the light, a hairline of catch where it comes back |
| **L3** | Face | The instrument face. Every number and identifier lives here |
| **L4** | Key | A control. Elevation and a lit top edge, and it moves one pixel when struck |
| **L5** | Lamp | Emitted light, never paint |

**One panel, several windows.** A bench is one instrument, and a fascia with a
row of readouts seated into it is what one looks like. Sections do not carry
their own metal: the chassis is continuous and the gaps between faces are that
metal showing through. A chassis is one piece of material, and drawing it as
five is drawing something that could not be machined.

### The finish

Four drafts built a *brushed* surface — directional grain, an anisotropic
highlight smeared across the lay — before the finish spec was checked. A
hardcoat-anodised fascia is **bead-blasted** before anodising, which is
isotropic: uniform matte micro-roughness, no lay at all.

**Micro-roughness.** Fractal noise at **two octaves, not four**. More octaves
clump into cloud, which reads as dirt on a lens rather than an even blast field.

**Micro-sparkle.** The same field thresholded to its top sixteenth, so only the
pits survive, held **at the perimeter**: the chamfer reaches the specular
direction far more readily than the flat field does, and the flat field is where
the reading happens. Sparkle in the middle is sparkle in the way.

**Two materials, two grains.** The chassis is blasted metal; a face is a printed
panel at a finer grain and half the strength. Two surfaces sharing a light
should still differ under it.

**And the metal is tuned for the strip, not the slab.** The chassis is almost
entirely covered by faces, so the only metal anyone sees is the gap between
them. Grain set for a full panel blows a twelve-pixel gap into a bright sanded
band.

### The light

**One lamp, in viewport space**, at 38% across and just above the fold. Every
panel reflects that one source, which is what makes the page read as one object
rather than thirty with private suns. **Highlights are cool and shadows are
warm** — how metal photographs under a key light with warm ambient, and the
absence of that split is the most reliable tell of an unconsidered dark theme.

**The body is uniform; only the edges move.** Brightness on a matte surface is a
function of the angle between its normal and the light, and every point on a
flat panel shares one normal — so a flat matte panel under a distant lamp is
*evenly lit*. The travelling lobe earlier drafts painted across it was a
point-light falloff: the model for a bulb held an inch off the surface, and the
reason it read as unnatural however carefully it was tuned.

**The pointer moves the viewer, not the lamp.** Hanging the light off the cursor
is a torch in your hand rather than a lit room. The lamp rests where it rests;
the pointer applies a bounded parallax — at most 15% of the viewport across and
9% down — standing in for a viewer leaning a few degrees. **Scrolling is the
primary motion**, which also works on a device with no pointer at all.

**Damped, not lerped.** A critically damped second-order spring at ω ≈ 4.6
rad/s, integrated against real elapsed time. A fixed-factor lerp snaps hard,
trails for a long time, and runs at whatever speed the frame rate happens to be.
That substitution is most of the difference between an effect that feels
reactive and one that feels deliberate.

### Edges

**Occlusion, not outline.** This took three passes and the corrections are the
substance of it.

A 1px solid border runs corner to corner at full strength on all four sides,
which is what a *line* does and not what *light* does — so the borders went, and
edges became gradients masked to a hairline with `mask-composite: exclude`.

Then the gradient traced all four sides, which is the solid border again drawn
more expensively. So it became a single catch along the top, gone by a third of
the way down.

Then the silhouette went with it, and panels floated. **The fix was not a
brighter line but a dark one**: the contact shadow a raised object casts into
what it sits on. Occlusion defines a shape at any strength and can never read as
a glowing border, which is the failure every bright version kept producing.

**And no edge carries colour.** The cover glass had a chromatic rim — cool where
light enters, warm where it leaves — which is what glass does and which, at any
visible strength, is an orange line down one side and a white one down the
other. A coloured border does not stop being a coloured border because it was
derived from optics.

### Form

**Square.** Radii are zero; controls keep 2px, which is not a radius anybody
reads as one and stops an input's corner pixel looking like a defect.

Squircles were tried and removed. "Organic" means light that falls off, not
corners that round over — and rounding them made this the same soft UI as
everything else. Sharp geometry with soft light is a coherent thing and a rarer
one.

### Colour: four channels

| Channel | Means | Where |
|---|---|---|
| **Nitrile violet** | *Act* | The live key. One per screen |
| **Steel** | *State* | Sealed, open, how much is free, how much is left |
| **Amber** | *A finding* | Nothing else, ever |
| **Green** | *Confirmation* | An action that just completed. Never a steady state |

**Steel exists because amber was about to be borrowed.** There was no way to say
sealed, or nearly out, without reaching for the one hue that already means
something else — and "amber is convenient for a warning" is verbatim why D115
exists. A missing channel is a real gap; filling it with the reserved colour is
how a rule that has held for a whole project stops holding. See D126.

### Typography

| Role | Face | Used for |
|---|---|---|
| **Display** | Archivo, **expanded** width, uppercase, wide-tracked | Nameplates, section heads |
| **UI** | Archivo, normal width | Everything read as prose |
| **Readout** | Archivo, tabular figures | Weights, counts, quantities, dimensions |
| **Identifier** | System mono, tabular figures | Anything read off a label or said down a phone |

Display is **expanded, not condensed**: an engraved nameplate is wide-tracked,
and condensed reads as newsprint where expanded reads as machined.

**Readout carries three sizes and the small one matters.** A stated figure
sitting beside fields being typed into is not the number anyone came to read, so
it matches the inputs it sits with and differs by not being one. The footprint
was briefly the largest thing in a carton, next to the empty weight box that is
the only figure anybody is there to enter.

Descriptions carry a little extra weight and tighter tracking rather than more
size. A description should stay quieter than the code above it; it just needs
enough stroke to survive a dark ground.

### Ornament, and drawn absence

Two devices, and both measure something.

**Dimension lines.** Hairlines with tick ends and a mono figure, carrying real
values off the record.

**Drawn empty states.** A container with nothing in it is still a container. An
empty carton was a line of faint text in a tall dark gap, so the fullest state
on the screen and the emptiest one occupied the same amount of nothing — and an
empty box on a bench does not read as nothing. It reads as a box, waiting.

So the box is drawn: an isometric wireframe at hairline weight with the occluded
far corner **dashed the way an engineering drawing dashes what it cannot see**,
over a faintly hatched ground. **And drawn to the preset's own proportions**, so
a pallet comes out squat and a small box does not. Height is eased rather than
scaled straight — a skid is 150mm on a 1165mm footprint, which in true
proportion draws a flat diamond nobody reads as a container. See D127.

### Density

Two modes over one token set, selected by surface rather than viewport.

```
[data-density="floor"]   17px base · 48px targets · gaps  6 12 18 24 36 48
[data-density="desk"]    15px base · 34px targets · gaps  4  8 12 16 24 32
```

**The scale is stated, not derived**, and that is a correction. It was six gaps
multiplied off a `--step` of 4px at desk, and the arithmetic was the defect:
`--gap-4` is what panel and well padding use, and it came out at **8px**. Every
panel had eight pixels of breathing room and every table sat against an edge. A
4px step is a sub-unit for optical nudges; it is not a base to multiply padding
out of.

Field widths are named roles for the same reason — `inline` for a table cell,
`measure` for a weight or a height — so a call site says what a field is *for*
and the density decides how wide that is.

### Motion

Mechanical. Keys move one pixel and lose most of their elevation in 90ms;
nothing springs except the lamp, which is critically damped and therefore does
not overshoot either. **Nothing animates that blocks work.**

---

## The laws

The design system's actual deliverable. Eight of them are enforced by
`client/scripts/check-laws.mjs`; the rest are enforced by review, and the ones
that are not checkable are the ones that have broken.

| Law | Why it is a law and not a preference |
|---|---|
| **Every element declares a layer** ✓ | The layer decides chamfer direction, ink treatment, and whether it takes the light model. An element styled outside the stack is the crack the whole material comes apart at |
| **A decorative layer never escapes what it decorates** ✓ | `position: absolute; inset: 0` resolves against the nearest *positioned* ancestor, so a parent missing `position: relative` silently hands the layer everything the parent does not cover. Valid CSS, no warning, and the symptom looks like a design decision — which is how it survived four rounds of being tuned as one |
| **A state nothing reaches is a state nobody has seen** ✓ | However carefully it was built. The drawn empty container rendered on zero screens because no fixture reached it |
| **A token nothing reads is drift** ✓ | Every dead token was live once and outlived what used it. It is also the check that catches a rule rewritten out of existence, because its tokens get left behind |
| **Materials own their CSS; primitives take no class or style** ✓ | There is no subset of the anodise recipe that is still anodised aluminium |
| **Spacing belongs to the parent** ✓ | Components own padding; nothing owns margin. A component that can push its neighbours around cannot be reused |
| **No raw colour outside tokens.css** ✓ | A hex in a component is a value that exists in one theme and was never considered in the other |
| **Metal is opaque** ✓ | The defining property. Translucency buys depth by contradicting the material, and it is the version that fails on a handheld |
| **Nothing translucent is laid over text** | Gloss bought with legibility is the worst trade available on this screen. It has broken once |
| **The chassis never holds a number** | Quantities and identifiers live on a face, because a gloved operator at arm's length needs paper contrast |
| **Edges are occlusion, not outline** | A bright line tracing a shape is a border however it was drawn. Dark reveal defines a silhouette at any strength |
| **No edge carries colour** | Dispersion is real optics and an orange line down one side is still an orange line |
| **The body is evenly lit; only edges move** | Brightness is a function of the normal, and a flat panel has one normal everywhere |
| **The pointer moves the viewer, not the lamp** | Hanging the light off the cursor is a torch in your hand. Scroll is the primary motion, and the one that works with no pointer |
| **The finish is blasted, so nothing has a lay** | A directional stripe reintroduced for texture is a brushed panel: a different part, a different specular, a far more common look |
| **Noise is two octaves, not four** | More octaves clump into cloud, which is dirt on a lens |
| **Glint lives at the perimeter** | The chamfer reaches the specular direction more readily, and the flat field is where the reading happens |
| **Metal encloses; nylon protects and labels** | Two materials, one lamp, a boundary neither crosses. Nylon never carries a number |
| **Violet acts · steel reports · amber finds · green confirms** | Four channels, no overlap. Amber is reserved and a missing channel is not a reason to borrow it |
| **Absence is drawn** | An empty container still occupies the bench. Rendering it as nothing makes the emptiest and fullest states the same size |
| **Ornament measures something** | A system about weights and measures can afford ornament only if the ornament is doing the measuring |
| **On the night face the well is lit, not sunk** | A recess in an already-dark field is invisible. Same component, opposite direction |
| **Motion is mechanical and never blocks work** | A panel transition is fine; a number transitioning while somebody reads it is not |

---

## Component inventory

Built and exported from `client/design/index.ts`:

**Materials.** `Panel`, `Well`, `Nylon`, `Boot`, `Tag`, `Face`, `FaceWell`,
`Key`, `Lamp`.

**Instruments.** `Readout` (three sizes), `Band`, `Pill` (three tones), `Code`,
`Soft`, `Faint`, `Steel`, `Field` (three widths, commits on Enter, `secret` for a password), `Chooser`,
`Dim`, `EmptySlot`.

**Structures.** `Table`, `Num`, `NumHead`, `Action`, `Finding`, `EvidencePair`.

**Layout.** `Stack`, `Row`, `Grid`, `Spacer` — gap only, from the token scale.

**Room.** `LightRoom`, `startLightSolver`.

**Shells.** `BenchShell` (key at the top), `FloorShell` (dock at the bottom,
D134), `DeskShell` (an evidence rail beside the work), `PlainShell` (one narrow
column and no chrome, for the screens that are about the account rather than the
work — setting a deployment up, which runs before there is a site or a person to
name, and changing your own password, which says whose it is in its own content
rather than in a corner as a tag).

**Locator.** `ScanInput` — one input, claims focus on mount and takes it back
after each scan (D111, D117). A wedge, not a camera: a warehouse scanner is a
keyboard.

Still to build: `Stamp`, `Sheet`, `Toast`, `Badge`, `Numpad`, and
the compositions — `WorkQueue`, `GateHeader`, `CartonBuilder`, `WorklistRow`,
`PickCard`, `ReceiptLine`, `MeasurementHistory`, `PolicyScope`.

A screen built from anything not on this list has found a gap worth naming.

## Frontend architecture

Per D113: React for every interactive surface, maud for documents.

```
crates/server/
  src/bench.rs           the pack bench as data — one definition, two renderers
  src/assets.rs          the bundle, same origin, and the cache contract
  src/routes.rs          JSON API, + GET /fulfilments/{id}/bench
  src/web/               narrows to documents: packing list, A4, labels

client/
  design/                tokens · materials · light · primitives · layout
  domain/                types, API client, formatting
  app/
    shells/              BenchShell (FloorShell, DeskShell to come)
    outbound/pack/       usePackBench · PackBench · fixture
  scripts/               gen-tokens · check-contract · check-laws · render
```

### One definition, two renderers

The bench reads were written inside `web/` when the bench was a server-rendered
page. Giving the screen to React meant either writing those queries a second
time against the same tables, or moving them where both renderers can read them.

**This codebase has a register for what the first option costs.** Every drift it
records has one shape: two things stating one fact, free to disagree. A packing
list computing "what is left" one way while the screen that filled the carton
computed it another is that shape exactly. So `crate::bench` holds the queries,
`GET /fulfilments/{id}/bench` returns them, and a test asserts the page draws the
reference and every line the endpoint reports — the agreement checked, rather
than the extraction assumed.

One request rather than five: a screen that opens by fetching a header, then
lines, then cartons, then contents, then presets has five chances to be slow.

### The bundle ships with the API

Same origin is the auth story rather than a convenience: the session is a cookie
and `api.ts` sends `credentials: "same-origin"`, so hosting the client anywhere
else trades a working sign-on for a CORS and SameSite problem. A node stage
builds `client/`, the server image carries `dist/`, and the mount is `/ui`.

> **Superseded by D144 and D146.** The mount is the root, and the API answers
> under `/api`. The argument below is why `/ui` was right while the API held the
> root namespace, and it is kept because the hazard it describes is real: what
> made the root mount safe was moving the API, not deciding the incident had
> stopped mattering.

**Not `/`, and the reason is a diagnostic.** A single-page app needs a fallback
so a deep link returns `index.html`. Mounted at the root, that fallback swallows
every unmatched path — including the API's — and answers a request for a route
that does not exist with a *page*. That is not hypothetical: an empty-bodied 404
is what identified a stale build once, and a root-mounted SPA would have returned
200 and an HTML document.

**And the document is never cached.** Assets are content-hashed and therefore
immutable by construction; `index.html` is one fixed name whose whole job is
naming today's hashes. Served with no `Cache-Control`, it invites heuristic
caching and a browser serves an old copy naming old hashes — so a deploy that
built correctly is invisible to anyone who has visited before, and redeploying
changes nothing. See D130.

### The four rules

**Materials own their CSS, and primitives accept no `className` or `style`.**
The only variation is a closed enum. The solver binds to `data-material`, so the
CSS Modules hash is never load-bearing.

**Children never set their own outer spacing.** Padding belongs to the material
that owns the surface; separation is the parent's `gap`.

**Logic and presentation split by what changes.** `usePackBench.ts` fetches and
returns data; `PackBench.tsx` takes data and emits primitives. The
presentational half renders from a fixture with no network — which is what makes
both densities and both faces reviewable, and what makes the render gate
possible at all.

**Every act re-reads rather than patching state, including after a failure.** A
write can fail having partly landed. A screen left showing what it hoped
happened is how a bench starts disagreeing with the ledger it exists to
describe.

### Four gates

`npm run verify` is the first three; CI adds the fourth after the build.

| Gate | Asks |
|---|---|
| `typecheck` | Does it compile, under `strict` and `noUncheckedIndexedAccess` |
| `tokens:check` | Is `tokens.gen.ts` generated from `tokens.css`, and does every themed token exist in **both** night blocks |
| `contract` | Does `domain/types.ts` still agree with `crates/server/src/bench.rs` about which fields exist |
| `laws` | Eight of the laws above, as static checks |
| `render` | Runs the bundle in a browser and measures what came out |

**The fifth exists because the first four read source.** Typecheck, the token
contract and the laws all answer questions about *text*, and all stayed green
through a component that rendered nowhere, a layer painting across the whole
chassis, and a stylesheet rule shipping the opposite of its own comment. None of
those is visible to a grep and all three are obvious to a
`getBoundingClientRect`. See D131.

The token generator uses the same bidirectional diff D25 applies to
`@projection` columns, and the type contract is the cheap half of D113's
"generated, not hand-mirrored" until a generator exists.

### Migration

Route by route. The maud pages stay until their React equivalent ships. Nothing
is deleted for tidiness.

---

## What is built

- **The design package.** Tokens, two materials, the light solver, the hangar,
  and the primitives listed above at both densities.
- **All five gates**, including the render check, which visits every screen in
  both faces — the Floor ones at a handheld viewport (D134) — and keeps the
  screenshots as a CI artefact.
- **The Bench shell**, **Pack** and **Despatch**, each against the live endpoint
  or a fixture, behind a route table in `main.tsx`.
- **The Floor shell** and **Capture**: the worklist, the figures and the seven
  faces, three stages against `/capture`, `/observations` and the image
  endpoint. D133, D134 and D135.
- **The locator**, on the capture screen: `ScanInput` over `GET /resolve`, which
  is D34's resolution function built at last. A scan resolving to one thing with
  one capture target opens the session; anything less certain draws the options.
  The failure record D111 asks for is deferred and the screen says so (D136).
- **Setup**, at `/setup`: the way into a deployment that has nobody in
  it, gated by a token the server prints to its log (D142). Four fixture routes,
  because a real deployment stops needing it the moment somebody does it.
- **The Desk shell** and **Findings**: the queue with its evidence on the row,
  the evidence panel in the rail, and investigate and accept. The third of the
  four surfaces, and the first screen to use amber for what amber is for.
- **The deployment**: node stage, same-origin bundle, cache contract, root
  redirect, and a link from the maud pages.
- **The photograph model** — `observation_image`, content-addressed storage and
  the two endpoints, now with a screen that drives them.

## What to build next

**The standing direction is that every view and component runs on the design
system.** Three screens do; eleven server-rendered maud pages do not, and they
are the worklist. Route by route, per the migration note above — nothing is
deleted for tidiness, and `/app/packing-list/{id}` stays maud because it is
genuinely a document.

1. **Weigh**, the last of the four load-bearing screens with nothing built.
   Bench surface, so the shell exists; `/revalidation` and `/weighings` are both
   built, and the worklist logic is already a tested pure module.
2. **The rest of the maud pages** — orders, fulfilment, the packing worklist,
   keys — and then sign-in, which moves last because it runs before a session
   exists and carries the passkey ceremony.
3. **The resolution-failure record** — D28's `activity_event` and the
   `symbology` table beside it. The locator resolves and reports and keeps
   nothing, which is the one part of D111 still owed (D136).
4. **The locator in the chrome.** It is on the capture screen and belongs on
   every surface, which is what D111 actually asks for. That needs somewhere to
   navigate to, so it arrives with the router.
5. **Pick**, where the offline question stops being theoretical.
6. **A real router**, which Capture turned out not to force (D135) and
   **Findings does**: its state is on the server, so a deep link to a selected
   finding restores everything it names, which is exactly the test D135 set.

Open question 4 — whether the material survives a Honeywell handheld — is a
hardware question rather than a scheduling one, and stays in the register.

## Decisions

### D109 — Four surfaces, one design system, two densities

**Decision.** Bench, Floor, Desk and Document are separate surfaces with separate
shells and separate ergonomic rules, served by one token set and one component
library at two densities.

**Why.** The differences are postural, not dimensional: gloved and walking versus
seated and comparing. A responsive layout expresses width and cannot express that.
But the differences do not reach the components — a panel is a panel in both — so
two design systems would be two things to keep in agreement, which is the failure
mode this repository has documented a dozen times.

### D110 — Navigation is by work, not by entity

**Decision.** The primary rail groups screens by job — Inbound, Outbound,
Integrity, Reference, You — not by table.

**Why.** The entity menu is a rendering of the schema and requires the operator to
know which noun holds the thing they want. It is a substantial part of why the
recorded process takes a dozen screens. Grouping by work also gives `Integrity` a
name, and that group is where this system's premise lives.

### D111 — The scan bar is the primary locator, and a failed resolution is a record

**Decision.** One always-present input resolves any scannable identifier to its
subject and navigates. A scan resolving to nothing writes a resolution-failure
record and tells the operator it has.

**Why.** The fastest path to a record is the barcode on the thing in your hand. And
D28 already requires that resolution failures are recorded; the locator is where
they arise, so it is where they get written.

### D112 — A badge counts work waiting for you, at this site, now

**Decision.** Every count in the chrome is scoped to the caller's site and to work
that is actionable. Zero hides. Totals never appear.

**Why.** Already the rule behind the weigh badge — *"a badge showing nothing to do
is a badge people stop reading"* — generalised rather than invented. A badge
reading 1,247 is a decoration; a badge reading 4 is an instruction.

### D113 — React for interactive surfaces, server-rendered for documents

**Decision.** Every interactive surface is React over the existing JSON API. maud
narrows to what is genuinely a document: the A4 packing list, paperwork, labels.

**Why.** D2's bar is reachable and the current form-post-and-reload pages are not.
The Tauri handheld path requires a web client not tied to the session-rendered
server. And the argument `web/mod.rs` makes for server rendering is narrower than
it looks: it is an argument about *documents*, and it is correct about those, which
is why they stay.

**What it costs.** An asset pipeline, which the current design explicitly avoided,
and a second deployable to host. Both were always implied by the React and Tauri
commitments in architecture.md.

### D114 — The client formats values; the server phrases judgements

**Decision.** Grouping, units, decimals and dates are formatted client-side.
Anything reading as a judgement — "due tomorrow", "overdue", "three weeks ago" — is
phrased by the server and sent as text.

**Why.** Formatting is presentation. Phrasing a promise against today is a business
rule with tests behind it already, and a second implementation in TypeScript is a
second thing that can disagree.

### D115 — Amber is exclusive; green is confirmation, not status

**Decision.** Amber marks findings and nothing else. Green marks an action that
just completed and never a steady state.

**Why.** One colour, one meaning, which erodes the moment amber is convenient for
something else. The green half pre-empts the same argument: a status dot on every
healthy row trains people to ignore the colour channel the finding rule depends on.

**Amended by D118 and D126.** Both are now emitted light rather than paint,
which gives breaking the rule a visible cost. And the reason amber survived
intact is that D126 gave state its own channel: the pressure on this rule was
never carelessness, it was a missing colour.

### D116 — Two rule weights, no radii

**Superseded by D118 and D120.** The identity is no longer carried by rules. Edges
are chamfers with a light model, and the enforceable constraint is that every
element declares a layer. Zero radius survives, as a property of the panels rather
than as a rule of its own.

### D117 — Scan focus is claimed by a screen, never stolen by the chrome

**Decision.** A screen declares its scan target. The locator receives scans only
when nothing has claimed them. Scan input sits behind one abstraction from the
first commit.

**Why.** Under keyboard wedge a scan goes wherever the caret is, which
floor-devices.md correctly identified as a real source of bugs on a busy screen. An
explicit claim removes the class rather than fixing instances of it.

### D118 — The chassis is metal and the data is paper

**Decision.** The enclosure is a material with physics — blasted hardcoat anodise,
one lamp, chamfers, occlusion. Every number and identifier sits on a bright matte
instrument face seated into it. The face is themed day and night; the chassis is
not.

**Why.** A dark, textured, richly lit interface and a screen read at arm's length by
a gloved operator under warehouse lighting are both requirements, and they are only
in conflict if one surface has to do both jobs. Separating them is what a Fluke
meter, a weighbridge terminal and an aircraft panel each arrived at independently.
It also gives theming an obvious home: an enclosure that changes colour with the
time of day is not an enclosure.

**What it costs.** Two palettes to maintain rather than one, and a rule that has to
be enforced rather than remembered — hence D124.

### D119 — Nothing translucent is laid over text

**Decision.** No translucent overlay, wash, streak or gradient may sit above running
text or a readout. The cover lens is confined to a masked corner region.

**Why.** It has already broken once: a draft ran a white wash diagonally across the
whole instrument face and measurably cost contrast. Gloss bought with legibility is
the worst trade available on this screen, and the pressure to make that trade
recurs every time something needs to look richer.

### D120 — The finish is blasted: isotropic, matte, sparkling at the edges

**Decision.** Bead-blasted Type III hardcoat. No directional grain anywhere. Noise
at two octaves. Micro-sparkle thresholded from the same field and held at the
perimeter. Peak specular 5%.

**Why.** Four drafts built a brushed surface before the finish was checked, and
brushed is the wrong part: an anodised instrument fascia is blasted, which is
isotropic. The consequences are not cosmetic — they decide whether the highlight is
a band or a field, whether the texture has direction, and where the sparkle sits.

**What it costs.** The striated anisotropic band, which was the most obviously
impressive thing any draft produced. A matte surface cannot make one.

### D121 — One fixed lamp; only edges move; the pointer moves the viewer

**Decision.** One lamp in viewport space. Panel bodies are uniformly lit; the four
chamfers carry a Fresnel baseline plus a swing weighted by how squarely each faces
the lamp. The pointer applies a bounded parallax standing in for a viewer leaning,
never relocating the light. Scroll is the primary motion. The lamp is driven by a
critically damped spring integrated against real elapsed time.

**Why.** Brightness on a matte surface is a function of the surface normal, and a
flat panel has one normal everywhere — so a travelling highlight across the body is
a point light held an inch away, and no amount of tuning makes it look otherwise.
Hanging the lamp off the cursor is a torch in your hand rather than a lit room. Both
errors shipped in earlier drafts and both were diagnosable from first principles
rather than from taste.

**What it settles.** Every "should this react to the mouse?" question afterwards.
The answer is that the room does not move; at most the viewer does.

### D122 — Two materials, one lamp: metal encloses, nylon protects and labels

**Decision.** Anodised aluminium and ripstop nylon, lit by the same lamp. Nylon
appears where the system is padded, carried or tagged — boots, straps, woven tags —
and never carries a number.

**Why.** The company sells nitrile gloves, hair nets and protective equipment and
is called Nylonite; a textile in the system is the trade rather than a flourish.
What keeps it from becoming decoration is a boundary with no exceptions. Sharing the
lamp is what makes them read as two finishes in one room rather than two unrelated
textures, and their different sheen behaviour — a fibre bundle is a cylinder, a
blast pit is not — tells them apart in motion.

### D123 — Materials own their CSS; primitives accept no class or style

**Decision.** No utility-class framework. Tokens are plain CSS custom properties;
each material is one stylesheet owning its whole recipe; primitives expose closed
enums and never a `className` or `style` prop. The light solver binds to
`data-material` attributes.

**Why.** A utility framework's affordance is composition at the call site, which is
exactly what a material must forbid — there is no subset of the anodise recipe that
is still anodised. Adopting one would mean hand-writing the recipe anyway and
configuring a second spacing and colour vocabulary to duplicate tokens that already
exist. Runtime CSS-in-JS is ruled out separately: custom properties are already the
dynamic channel and are far cheaper at sixty frames a second.

**What it costs.** Layout ergonomics, answered by a small set of `Stack` / `Row` /
`Grid` primitives taking gap values from the token scale only.

### D124 — Spacing belongs to the parent, and the laws are lint rules

**Decision.** Components set no outer margin; separation is the parent's `gap`. The
material system's laws are enforced in CI, and the TypeScript token constants are
generated from `tokens.css` with a test that fails when the committed file differs.

**Why.** Every law in this document is one somebody will have a good local reason to
break — amber is convenient for warnings, a wash makes a panel look richer, a
margin fixes one screen. Written down, they last about a quarter. The checks worth
having first:

- every element under `design/` carries a `data-layer`
- `--amber` and `--face-amber` are referenced only under `app/integrity/`
- no `margin` in any component stylesheet; no raw hex outside `tokens.css`
- nothing with `opacity` or a translucent background is an ancestor of face content
- every token in `:root` has a counterpart in both face themes

The generated-constants-with-a-diff-test is the same bidirectional mechanism D25
uses for `@projection` columns and that the invariant register is moving toward. It
makes "the night face is missing a token" a build failure rather than something
somebody notices in a screenshot.

### D125 — The literal hardware comes off; the physics stays

**Decision.** No relief groove, no bezel catch, no chamfer rings, no riser under
a key. The material keeps its light model, its grain, its occlusion and its
edge behaviour.

**Why.** Those four were hardware traced from photographs of hardware, and they
are what made a considered material read as a decade-old interface. The line
worth holding is between *how a surface behaves* and *what a surface has bolted
to it*: a blasted finish scattering light is the first, a moulded key cap is the
second. Everything that survived is derivable from a light and a roughness;
everything removed had to be drawn from memory of an object.

**What it settles.** Every future "should this look more physical" question. The
answer is more physics, not more furniture.

### D126 — Four colour channels, and amber is not one to borrow

**Decision.** Nitrile violet means *act*. Steel means *state*. Amber means *a
finding*. Green means *an action that just completed*. No channel does two jobs
and none is reassigned.

**Why.** There was no way to say sealed, or open, or nearly out, and the obvious
fill was a muted amber — which is verbatim the erosion D115 exists to prevent.
Both halves of that are true at once: the gap was real and the reserved colour
was the wrong thing to fill it with. Steel is cool and industrial and cannot be
mistaken for the violet that means act.

**What it costs.** One more hue to keep coherent across both faces, and a rule
that sealed reads steel rather than green — green confirms an action just taken,
and a carton sealed before the page loaded is a state.

### D127 — Absence is drawn, and the drawing is dimensioned

**Decision.** Empty and low-content states carry material weight. A container
with nothing in it is drawn as a container, from the dimensions the record
actually holds.

**Why.** An empty carton rendered as a line of faint text made the emptiest
state and the fullest state occupy the same amount of nothing, which is not what
an empty box on a bench looks like. And ornament measures something or it is
decoration: drawing a *generic* carton would have been a picture, where drawing
the preset's own proportions is a measurement.

**What it costs.** A height easing, because a skid at 150mm on a 1165mm
footprint is true in proportion and illegible as a drawing. The exponent keeps
the ordering and lifts the extreme, and that compromise is stated rather than
hidden.

### D128 — Edges are occlusion; no edge carries colour

**Decision.** A raised surface is defined by the dark contact shadow it casts,
plus at most one catch along the lit edge. No bright line traces a shape, and no
edge is tinted.

**Why.** Three passes each failed the same way. A solid border runs corner to
corner at full strength, which is what a line does. A gradient masked to a
hairline around all four sides is that border drawn more expensively. And
dispersion on the cover glass — cool where light enters, warm where it leaves —
is real optics and also an orange line down one side. **A coloured border does
not stop being a coloured border because it was derived.** Occlusion defines a
silhouette at any strength and can never glow.

### D129 — Square corners with soft light

**Decision.** Radii are zero. Controls keep two pixels. `corner-shape` is not
used.

**Why.** Squircles were the wrong reading of a request for something organic:
what was wanted was light that falls off, not corners that round over — and
rounding them as well made this the same soft UI as everything else. Sharp
geometry under soft light is coherent and rarer. Two pixels on an input is not a
radius anybody reads as one; it stops a corner pixel looking like a defect.

### D130 — The bundle ships with the API, and the document is never cached

**Decision.** The client is built into the server image and served from the same
origin, at the root. Content-hashed assets are cached for a year and marked
`immutable`; `index.html` and every client route are `no-cache`.

**Why.** The session is a cookie and the client sends `credentials:
"same-origin"`, so a separately-hosted bundle trades a working sign-on for a
SameSite problem. The mount is not `/` because a root SPA fallback answers an
unmatched API path with a page instead of a 404, and that 404 has already been
load-bearing in a diagnosis.

The cache half is the one that bit twice. A document served with only
`Last-Modified` invites heuristic caching: the browser invents a lifetime and
serves an old copy, which names old hashes, which loads the old application.
**A deploy can be entirely correct and entirely invisible**, and redeploying
does not help because the document was never the thing being refetched.

### D131 — A gate that renders the page

**Decision.** CI runs the built bundle in a headless browser and asserts what
came out: every material carries a layer, no decorative layer escapes what it
decorates, the states the design system draws are reachable from a fixture, and
both themes resolve. Screenshots are kept as an artefact.

**Why.** The other gates read source and all stayed green through three defects
at once — a component that rendered nowhere, a layer painting across the whole
chassis, and a rule shipping the opposite of its own comment. Two of those had
been *reported as fixed*.

**The containment assertion is the one worth having.** `position: absolute;
inset: 0` resolves against the nearest positioned ancestor, so a parent missing
`position: relative` silently hands the layer everything the parent does not
cover. The CSS is valid, nothing warns, and the symptom looks like a design
decision — which is how it survived four rounds of being tuned as one.

**What it costs.** A browser download in CI, and a fixture that has to reach
every state the design system draws. The second is a feature: a state no fixture
reaches is a state nobody has seen.

### D134 — The Floor shell docks the primary action, and the gate renders it at the size of the device

**Decision.** `FloorShell` is a column that fills the viewport: a thin header, a
scrolling body, and a **dock** pinned to the bottom holding the primary action.
The dock is a slot, it is opaque, and the render gate visits every Floor screen
at a handheld viewport rather than at the bench's.

**Why the bottom.** This is the whole of the postural difference D109 asserts.
A bench monitor is read top-down at eye level; a handheld is held at waist
height and driven with the thumb of the hand holding it, so the reachable third
of the screen is the bottom third and the top is where the least reachable
control on the device lives. Putting the difference in the shell is what stops
it being decided again, differently, on every screen.

**Why a slot and not a `primary` prop.** The action at the bottom of a capture
session changes as the session moves — record the figures, then finish. A shell
taking a label and a callback would be modelling one step of a flow it does not
know about, and the second screen would break it.

**Why the viewport is part of the decision.** The dock was transparent, and at
1600×1500 that looked deliberate. At 430×932 the worklist scrolled underneath
it and the row passing behind the primary key collided with the label beside
it — both unreadable, and invisible at bench width. **A Floor screen reviewed at
desk width is a screen nobody will see.** D109's claim that the surfaces differ
posturally rather than dimensionally is a reason to render the handheld at the
handheld's size, not a licence to skip doing so.

So the dock is opaque, and it is `--ano-950` rather than a face token: the
chassis is a fixed material that does not change with the time of day (D118),
because the dock is part of the device rather than paper laid on it. Depth comes
from the ground and an occluding edge, never from a blur.

**The gate now asserts the density too**, which it could not before: `--target`
was being read off `document.documentElement`, where it is not declared, so an
empty string was compared against nothing and passed. It reads the density
element and asserts 48px on the floor against 34px at the bench. A Floor screen
mounted inside `density="desk"` would look almost right at a monitor and be
wrong on the device, and nothing but this gate can see that. **A check that
always passes is worse than no check**, and it is the third time in this
repository that the checker was the thing that was wrong.

### D135 — A capture session is not in the URL

**Decision.** The subject being captured lives in `useCapture` and not in the
path. The route table stays; the router does not arrive with this screen.

**Why, given this was the screen expected to force one.** *"The first screen
needing an identifier in its URL replaces it. Capture will be that screen."*
It turns out to be the screen that argues the other way. D133 makes the whole
session one act, so nothing the operator types is durable until Record — and a
link that reopened `/capture/17e1…` after a refresh would faithfully restore the
subject while silently dropping the three figures beside it. **A URL that
promises a resumability the model deliberately refuses is worse than no URL**,
because it is a promise the operator has no way to check.

The subject is also not something anybody types. It is chosen from a worklist or
resolved from a scan, both of which are already on the screen.

**What this defers rather than settles.** The route table is still not a router
and still says so. The screen that replaces it is one whose state is on the
server — a fulfilment, a receipt, a finding — where a deep link restores
everything it names. That is a stronger test than "has an identifier".

**Met on 2026-08-31, by the finding.** `/findings/:finding` is the route, and the
finding is the only thing in it. The tab is *derived* from the finding's state
rather than carried beside it — a tab is a filter and a filter is not a place,
which is the same argument `/orders` makes about `?reference=` — and it moves
only when the tab you are on cannot show the finding you were sent. So the path
alone restores the queue, the tab and the panel, and the test D135 set is the
one that was run: a refresh comes back to the same evidence, and the link goes
to a colleague.

It also surfaced what a route table with no subjects in it could hide.
`subscribe` listened for `popstate` directly, while the snapshot React then read
was written only by `go()` — so Back notified React with the path from *before*
the navigation, React compared it as unchanged, and the address bar moved while
the screen did not. Nothing had noticed, because until a route had a subject in
it there was nothing anybody would press Back out of.

### D144 — The API answers under `/api`, so the screens can have the root

*Adopted 2026-08-18.*

**Decision.** Every JSON endpoint moves under `/api`. The server-rendered
documents stay at `/app` until they narrow to `/print` (D113). The client will
take the root.

**The problem.** The bundle answers under `/ui` and the screens are named
`/pack`, `/findings`. Nobody types that, nobody reads it down a
phone, and — the report that prompted this — it reads as a demo rather than as
software. The obvious fix is to move the client to the root, and the obvious
fix was blocked: `/capture` and `/setup` were already endpoints, and a rule
saying *new endpoints must avoid the page names* is a rule somebody eventually
forgets in the direction that breaks a screen.

**Why a prefix rather than careful naming.** Seven of the nine names the client
wants are free at the root today, so this could have been solved by choosing
different words twice. That solution decays: it holds only while everyone adding
an endpoint remembers to check a list kept somewhere else. A prefix makes the
collision impossible to write, which is the difference between a convention and
a structure.

**It is also what makes a root-mounted client safe**, and that is the part worth
recording. `assets.rs` argued for `/ui` on a real incident: a single-page
application needs a catch-all returning `index.html`, and at the root that
catch-all swallows every unmatched path — including the API's. A route missing
from a stale build answered 404 with an empty body, and *"the emptiness is what
identified it"*; a page returned as 200 would have made the diagnosis much
longer. With the API in its own scope terminating its own misses, an unmatched
`/api/*` never reaches the catch-all and stays diagnosable. The hazard is
removed rather than tolerated.

**The prefix is written down once on each side.** `send()` in `domain/api.ts`,
and the `post()` helper in the maud pages' inline scripts. The forty endpoint
definitions still read as the paths they are. A prefix repeated at forty call
sites is forty chances to leave one behind.

**One design consequence, and it is an improvement.** `Photo` built
`/images/{digest}` itself — the one place under `design/` that knew an endpoint's
shape, in a package that otherwise imports nothing from `domain/`. It now takes
a URL and the caller builds it, so the design system knows only how to draw a
picture.

**What it does not do.** Move the client. That is a separate commit, so that a
URL problem and a behaviour problem cannot arrive in the same bisect, and so
this can sit on the deployment for a release first. The test that guards it —
`crates/server/tests/api_mount.rs`, asserting that an unmatched `/api/*` answers
404 with an empty body — is written now, where it passes trivially, because a
guard written after a deploy has already gone quiet is a guard written too late.

**Nor does it move the ninety test URIs.** The suite mounts `routes::configure`
at the root and asks whether a handler works; the prefix is not its subject.
Rewriting ninety paths to restate one fact is churn that can only introduce
mistakes, so the fact is asserted once, in the shape a running server has.

---

## Open questions

1. **Does the pack bench have a fixed monitor size?** It changes whether Bench is
   designed to a known viewport or to a range, and a known one is worth a lot.
2. **Which screens genuinely need offline**, per floor-devices.md's question 3. It
   is a design question before it is an engineering one: an operator's mental model
   of "did that save?" has to be honest, and that is interface work.
3. **What does the policy scope lattice (D22) look like as a screen?** The only part
   of the domain with no obvious interface precedent, and the one most likely to
   become a form nobody can use.
4. **Does the material survive a Honeywell handheld?** The blend modes, masks and
   SVG-filter textures are cheap on a desktop GPU and unmeasured on the fleet. The
   likely answer is a `[data-density="floor"]` variant that drops the sparkle layer
   and the hangar, which is worth designing deliberately rather than discovering.
   The render gate can measure it once there is a device profile to render at.
5. **Do handhelds need a distinct sign-in ceremony?** Passkeys with no email already
   suit a shared device; whether shift handover is a session boundary is a D11
   question with an interface answer.
6. **What is the tenant-switching model in the chrome?** Multi-tenancy has been in
   the schema since migration 1 and has never had a pixel drawn for it.
7. **Is there a brand beyond the product name?** Nothing here has been checked
   against anything the business already puts on paper.
8. **Does the fixture owe a state per screen?** D131 makes reachability a gate,
   and the honest generalisation is that every state a screen can be in wants a
   fixture reaching it. That is a larger commitment than one empty carton and it
   is not yet decided how far it goes.

### D146 — The screens have the root, and fixtures are a build mode

*Adopted 2026-08-18.*

**Decision.** The bundle mounts at `/`. Screens are `/pack`, `/findings`,
`/capture`, `/weigh`, `/despatch`, `/picking`, `/where`, `/account`, `/setup`.
Fixtures move to `/fixtures/*` and exist only in a **review build**, which the
render gate measures and the deployment never receives. An unmatched path draws
a real not-found screen. `assets.rs` keeps the maud documents' place and the
packing list moves to `/print/packing-list/{id}`, which is where D113 always
said it was going.

**The problem.** The route table was a `Record` of thirty-nine literal
pathnames, thirty-one of them fixtures — including `/`, the front door — and
it ended `?? FixturePack`, so every typo and every stale link drew a pack bench
full of invented data. Every real screen hid behind `/live/*`. There was not
one `<a>` element in the client, so no screen could reach another. That is not a
cosmetic complaint: it is what made the whole thing read as a demo, and all five
gates stayed green throughout because each measured something else.

**Why the router arrives now.** D135 deferred it until *"a screen whose state is
on the server — a fulfilment, a receipt, a finding — where a deep link restores
everything it names."* Findings is that screen and it is built.

**Written rather than depended on**, and the reasoning is specific to this
client rather than general minimalism. The fraction of a router library this
would use is the fraction that is cheap to write: match, params, and a 404.
There are no nested layouts — a shell is chosen per route rather than composed —
and its data APIs would be a second fetching idiom beside the one this codebase
already decided on. The two edge cases that do bite are both handled explicitly:
`stripBase`, so the matcher is correct under `/` and under `/` and the mount
could move in one constant; and match order, which is declaration order and is
asserted by a test rather than left to a sorting rule nobody can see.

The matcher is pure and has twelve tests. `node --test` runs TypeScript
directly, so the client gained a test runner without gaining a dependency.

**What made the root mount safe.** This file argued for `/ui` on a real
incident, and the argument was sound: a single-page application's catch-all,
mounted at the root, swallows unmatched API paths and turns a diagnostic empty
404 into a 200 and an HTML document. D144 removed the hazard by giving the API
its own scope with its own `default_service`. The guard is
`crates/server/tests/api_mount.rs`, written a commit early while it still passed
trivially, because a guard written after a deploy has gone quiet is written too
late.

**Fixtures are a build mode, not a route.** `import.meta.env.MODE` is
substituted with a literal, so a production build compiles
`if ("production" === "review")` and Rollup drops the branch, the dynamic import
and the whole module. Verified by grepping the built assets: no fixture chunk,
no `d.stooke`, and `/fixtures/pack` draws the not-found screen on a real server.

**The cost, stated rather than buried:** the render gate now measures
`dist-review` and not the artefact that ships. That is the same smell as
everything else this work is fixing, and it is accepted because the alternative
is shipping invented data to a customer. It is bounded three ways — the two
bundles differ in exactly one thing, the route table; law 9 asserts the
production entry imports no fixture; and the gate's route list moves in the same
commit as any rename.

**Two new laws, both verified by breaking them.** Law 9: the production entry
imports no fixture. Its first version passed over every file, because it reused
an `exists()` helper that answers for directories — a file always came back
false through it, and the check silently did nothing. Law 10: anchors come from
`Link`. That is what keeps the click interceptor total: it hands a path to the
router only when it resolves, so the eleven maud pages still standing at `/app`
stay reachable by a full page load, and *"nothing is deleted for tidiness"*
survives.

**`Link` is the first anchor in this client**, and it is never lit. A `Key` with
an `onClick` would have drawn the same thing and given up middle-click,
open-in-new-tab and a screen reader saying *link* — and it would have added a
button to every screen, invalidating the twenty-nine hand-counted `litKeys` the
render gate holds. Its colour is inherited rather than fixed, because D118 puts
metal and paper in one system with opposite inks: the first landing screen drew
four grey words on white before that was fixed.

### D147 — The front door is what is waiting for you, at this site, now

*Adopted 2026-08-18.*

**Decision.** `/` reads `GET /api/work` and draws the work waiting at the
caller's site, grouped by job as D110 groups the rail. One endpoint serves both
this screen and the rail's badges.

**Why one endpoint.** Two would be two computations of one fact, free to
disagree — the failure this repository has recorded more times than any other.
And scoping server-side means **the client cannot render a total it was never
sent**, which is a stronger guarantee than a rule about what the client ought to
filter.

**Zero hides, and nothing waiting is not the same as no site.** D112's rule
drives the whole screen: a group with nothing in it is drawn as a sentence
rather than a row with a nought beside it. A session naming no site draws
something different again, because *go home* and *the software does not know
where you are standing* are opposite instructions and must not look alike.

**No weigh count, and the omission is the point.** What is due for weighing is
not a SQL predicate: `revalidation` reads every current gross weight and decides
in Rust, from the method that produced it and how long ago. A `count(*)` here
would be a second definition of "stale" free to disagree with the screen it
labels. The fix is to extract that worklist so both call it, and it belongs with
the weigh screen's port. Until then there is no weigh badge — a gap somebody can
see rather than a number nobody can trust.

### D148 — The rail is drawn from one list, and the list is checked

*Adopted 2026-08-18.*

**Decision.** D110's work rail is built. Its destinations are one exported list
grouped by job — Outbound, Integrity, You — carried on Bench and Desk, absent on
Floor, with badges from the same `GET /api/work` the landing screen reads.

**Absent, not disabled.** `Inbound` and `Reference` have no screens behind them
and so are not in the rail at all. A greyed row is a promise, and a rail of
promises is a rail people stop reading — the same argument D112 makes about a
badge showing nought, which `Badge` now enforces by returning nothing.

**Floor has no rail, and this is the decision most likely to be questioned.** A
handheld is 430px wide and D134 has already spoken for the dock as the reachable
third. A squeezed column and an overlaid sheet are both worse answers than the
screen the product already needed: **the landing screen is Floor's rail**, drawn
as full-width rows, and the way to it is the wordmark, which every shell header
now carries as a link. One extra tap, and it buys no overlay (D119 territory),
no focus trap and no second component.

**`external` is what keeps the migration honest.** Eleven maud pages are still
standing and D113's doctrine is that each lives until its replacement ships. A
rail entry pointing at one is marked external, so the router's click interceptor
lets the browser have it. Without that, Orders and Passkeys would be
client-side 404s on pages that work.

**The list is checked rather than trusted**, which is the part that matters.
Three tests ask questions a hand-written rail cannot answer for itself: does
every entry point at a route that exists; is every screen reachable, or does it
say how it is reached instead; and does every badge name a count the endpoint
returns. The second forces a decision when a screen is added, which is the only
way *a screen nobody can reach* stops being invisible.

That required splitting the route table into a manifest of plain data and the
components that draw it — `node --test` cannot load a module importing React and
a stylesheet. The split earned its keep immediately: pairing the manifest with
the renderers now throws if either side has an entry the other lacks, which is
the *component rendering nowhere* defect the render gate was built after finding.

**`DeskShell`'s `rail` prop is now `evidence`.** D110 calls navigation the work
rail and that shell used the same word for the panel beside the work; two
meanings for one name in one file is a misreading waiting to happen. The rename
caught a live bug on its first run — the findings fixtures were still passing the
evidence panel as `rail`, so it drew inside the navigation chrome, and the gate
said so.

**The chrome is chassis, and the gate now asserts it.** Every screen counter
looks inside `[data-region="work"]`; without that the rail would inflate the
twenty-nine hand-counted expectations at once, and a chrome scan bar would make
`minScans` trivially true everywhere and destroy the check that says *capture
has a locator*. Four new universal assertions: no instrument face in the chrome
(D118), exactly one link home, at most one destination marked current, and no
badge showing a nought.

**Still owed:** D111's locator in the chrome. Presence should be decided
statically from the route table so the gate can assert it, and focus arbitrated
at runtime so the chrome never steals what a screen has claimed (D117) — Capture
owns its scanner and must keep it.

### D149 — Presence is static, focus is claimed: the locator reaches the chrome

*Adopted 2026-08-18.*

**Decision.** D111's locator is in the chrome on every screen behind a session,
except the one that owns the scanner. **Whether it is drawn is decided by the
route manifest; whether it holds the caret is decided at runtime, and the answer
is always no.**

**The tension, and why it is only apparent.** D111 wants *"one always-present
input"* on every surface; D117 says *"focus is claimed by a screen, never stolen
by the chrome."* They read as contradictory until presence and focus are
separated, at which point both hold exactly.

*Presence* is a flag on the manifest. A claim registered in a mount effect would
draw the chrome's locator for one frame and then remove it — a flash, and a gate
assertion that depends on timing. Declared statically, it is settled before
anything renders and a test can ask about it.

*Focus* is arbitrated by a small store, and the rule is one sentence: **the
chrome's locator never calls for the caret.** `ScanInput` gained a `focus` prop
for it, because two inputs on one page both using `autoFocus` would fight and
the last mounted would win — a caret landing where the operator did not put it,
which is D117's own bug wearing different clothes.

**Two screens do not carry it, for two different reasons.** Capture owns the
scanner, and two locators stacked on a 430px handheld is a worse answer to D111
than one — `FloorShell` already argues that vertical space is the scarce
resource there. Setup runs on a deployment with nobody in it, so resolving would
answer 401; a scan bar that cannot answer is worse than no scan bar.

**The four outcomes are a pure function with a table test.** D111 is emphatic
that collapsing them is the failure — *"tells an operator 'no such item' about a
smudge, and, worse in the other direction, tells them the same thing about two
items when the truth is `identifier_ambiguous`."* As a `switch` inside a
component, only a browser could reach them and only a warehouse with the wrong
labels in it could produce three. As `destinationFor()` it is six assertions.

A fifth outcome is drawn that the register did not name: **resolved to something
real that no screen takes as a subject** — a location, or a package. Sending it
to Capture because that is the only screen built would be the silent preference
D111 forbids, one level up. It says *the scan was read correctly; the
destination is missing*, which is a different sentence from *no such thing*.

**What the gate can and cannot see.** It asserts the chrome never draws two
locators and never draws one on a screen that claimed the scanner. It cannot
assert *which* screens should have one, because a fixture that does not model
the chrome cannot answer that — so the manifest answers it, in
`app/routing/manifest.test.ts`, along with no duplicate ids or paths and every
path resolving to its own entry.

**Still owed:** the resolution-failure record. D28 wants an unresolvable scan
written down, because a warehouse full of unreadable labels is a finding about
the label estate. The screen says so; `activity_event` still does not exist
(D136).

### D150 — Finding an order is a screen, and a commitment is a path

*Adopted 2026-08-18.*

**Decision.** `/orders` replaces `/app/orders`, and `/pack/:fulfilment` carries a
commitment in the URL. `const FULFILMENT` is gone.

**The first port, and it was chosen for that.** `GET /orders` already existed, so
nothing had to be designed on the server — which makes it the cheapest way to
find out whether the router, the rail and the chrome hold up on a real screen
against real data. They do: rail → `/orders` → search → a commitment → its
bench, and the deep link restores it.

**It answers with a list, and that is not a lookup dressed up.** D44 says an
externally-authoritative order is amended by cancel-and-reraise, so **one
reference can legitimately name a cancelled order and the one that replaced
it**. Showing only the newest would hide exactly what somebody ringing about a
changed order needs. The fixture draws that pair, because it takes an amended
order to produce and nobody would otherwise see it.

**Progress is quantities, never a status**, per S44: no table in the fulfilment
set carries a stored progress column, because any label it held would be a
function of four coverage quantities that can disagree with it. So a partly
picked commitment says *6 of 24 picked* rather than failing a gate, and `as_at`
distinguishes *never projected* from *projected* (D95) rather than drawing a
stale number as a fresh one.

**`/pack/:fulfilment` is what D135 was waiting for.** It deferred the router
until *"a screen whose state is on the server — a fulfilment, a receipt, a
finding — where a deep link restores everything it names"*, and this is
literally that. `/pack` with nothing named draws a sentence and a way to find
one; it does not pick a fulfilment, which is what the constant it replaces did.

**The parity gap was caught before it shipped, not after.** The first version of
this screen showed everything the maud page showed except the link through to
the bench — and the bench link is what stage 1 exists for. `docs/handover.md`
warns about exactly this: *"the thing to watch on a port is that the React
version does not quietly lose something the maud page showed."* It was found by
reading the page it replaces rather than by using the new one.

**The rail's Orders entry stopped being external**, and a test failed on that
day — which is what it is for. An entry that stops pointing at maud must stop
being treated as one, or the click interceptor keeps handing it to the browser.

**`/app/orders` still stands.** Parity is met, so it *may* go; it is left for one
release of real use first. Nothing is deleted for tidiness, and nothing is
deleted the same hour its replacement first ran either.

### D151 — The pack queue moves, and its grouping is one function

*Adopted 2026-08-18.*

**Decision.** `/pack` is the queue and `/pack/:fulfilment` is the bench. The four
groups come from `packing::stage`, a pure function of two numbers that **the
server-rendered page and the JSON endpoint both call**.

**Why one function rather than one endpoint.** The obvious port copies the
grouping into the client. That would be two definitions of "ready to pack",
free to disagree, and which one an operator saw would depend on which screen
they opened. `stage(picked, committed)` is tested beside itself, the maud page
now calls it, and the two screens were checked against real data giving the same
answer — three ready and one packed, on both.

**The ordering inside it is the whole bug it prevents.** `NothingCommitted` is
tested first, because with zero committed `picked >= committed` is *true* and an
empty commitment would file under Packed — which reads as *that went out*. That
is a test rather than a comment.

**"Overdue" is the server's word.** D114 puts the line at judgement: *in 3 days*
is arithmetic, *overdue* is an opinion about it. `due()` moved across with the
queue rather than being re-derived from a date on the client.

**It opens on the work, not on a search box**, which is one of the few things
the pages it replaces got right. The search narrows a list already there.

**A group with nothing in it is not drawn.** Four empty headings is a screen
telling you four times that there is nothing to do — the same argument D112
makes about a badge showing nought.

**The packing-list link was the real find.** Porting `/app/fulfilment/{id}` as
its own screen would have duplicated the carton list the bench already draws —
so instead the bench gained what it was actually missing: the link to the
printable packing list, which the maud bench offered and the React one did not.
That is stage 5 of the recorded process, and losing it silently is exactly what
`handover.md` warns a port does. A screen that duplicates one you have is not a
port, it is a second place to fix things.

### D152 — The gate enforces, and a phone gets the work before the menu

*Adopted 2026-08-19.*

**Decision.** `/sign-in` is React, every screen behind a session is gated, and
signing out is an act rather than a page. The rail is drawn once and follows the
work on a narrow screen instead of preceding it.

**The gate decided and did not enforce.** `SessionContext` has resolved
`anonymous` since D145 and **nothing acted on it** — an unsigned-in visit to
`/pack` drew the pack screen and let every call under it fail one at a time.
There was no `/sign-in` in the client at all, so the only way in was the
server-rendered page nothing linked to. A state that is computed and ignored is
worse than one that is not computed: it reads, in the source, as though somebody
had thought about it.

**Signing in is a feature rather than a port**, which `docs/handover.md`
predicted. D19 makes a person global and membership per-tenant, so somebody who
works for two businesses must say which they are acting for; the server has
always answered 409 with the list and the maud page printed the error text at
somebody with no field to answer in. Here the 409 is a question. That also
needed `ApiError` to keep the parsed body — a refusal that carries a question is
not fully described by its message.

**Replace, never push**, and `?next=` carries the destination. Otherwise Back
bounces between the form and the screen that rejected it, and a bookmarked
finding does not survive an expired session — which is most of what the deep
links D135 waited for are for. `next` is refused unless it is a path of this
application, because an absolute one is an open redirect.

**Loading is not refused.** The gate draws nothing while the session resolves
rather than redirecting, or a reload signs everybody out.

**The phone fix took two attempts and the first one was wrong.** Below the
breakpoint the rail was stacked *above* the work, so a phone scrolled through
eleven navigation rows to reach a queue. The first fix hid the shell's rail and
drew a second one on the landing screen — which left two `nav` landmarks in the
document, twenty links where there are ten destinations, and a hidden Sign out
that a click found before the visible one. **That was found by a test clicking
it, not by looking at a screenshot.**

What replaced it is ordering rather than duplication: one rail, after the work
on a narrow screen and beside it on a wide one. Content first and navigation
after is also where a thumb is, which is D134's argument for the Floor dock
applied to a page.

**Responsiveness is now measured.** Every desk-shaped route renders a third time
at 390px, and the assertion is that nothing runs past the window —
`scrollWidth > clientWidth` is the failure a screenshot at 1600px cannot show,
and sideways scrolling on a phone is the difference between an application and a
website somebody did not finish. Screenshots are named for the visit rather than
the colour scheme, because the pocket render was silently overwriting the light
one.

**`Badge` inherits its colour**, like `Link` before it. Pinned to `--legend` it
read correctly in the chassis rail and washed out to pale grey the moment the
same component was drawn on an instrument face. A count nobody can read is a
count that is not there.

### D153 — The field asks in the unit the instrument reads

*Adopted 2026-08-19.*

**Decision.** Capture asks for lengths in **centimetres** and weight in
**kilograms**, sends both with their unit, and shows stored lengths back in
centimetres. Nothing is scaled on the client.

**The defect this fixes was silent and it was a tenth.** The screen asked for
millimetres. The tape in the warehouse is marked in centimetres, and so is the
printed sheet this replaces, whose own header reads *"measure the selling unit
shown in Unit · centimetres and kilograms, one decimal"*. So `23.5` off the tape
went in as 23.5mm and was stored as 23mm — no error, nothing on screen, and a
box recorded at a tenth of its length. Weight was already kilograms and right,
which is what made the mismatch easy to miss.

**Verified against a running server rather than reasoned about:** `23.5cm`
entered stores 235, `0.896kg` stores 896, and both keep the entered pair.

**Nothing is converted on the client**, and that is Principle 5 rather than
laziness: `unit` travels with the value and the writer applies
`unit.factor_num/factor_den` — cm is already 10/1 to canonical mm. A client that
divided by ten would be a second place the conversion lives, and the two would
disagree the day one of them changed.

**A unit cannot live only in a `suffix`.** `measurementsOf` moved into a module
with no React in it so `node --test` can read it, and five assertions pin the
units, the absence of client-side scaling, that a blank is not a measurement of
zero, and D138's rule that declaring no dimensions declares all three.

**The readouts say cm too.** They showed a bare number, which was ambiguous
while the field said millimetres and wrong the moment it said centimetres. A
stored value shown in a different unit from the one the field asks for is how a
number gets retyped at a tenth of its size — which is the same failure from the
other end.

### D159 — The chrome is one component, and below 48rem it stacks rather than wraps

*Adopted 2026-08-19.*

**Decision.** All four shells draw the same `Chrome`. Below 48rem it stacks —
identity, then where-and-who, then the locator across the full width — and
**nothing is dropped to make room**. Above 48rem it is one line, which is where
it already was.

**It was written four times.** Every shell hand-rolled the same row and carried
its own `.mark` and `.title` rules; three were byte-identical and the fourth
differed by two font sizes. The wordmark's `letter-spacing: 0.24em` sat in all
four beside a `--track-legend` token doing the same job one line down, which is
now `--track-mark`. None of that was a decision anybody made — it is what four
copies become.

**A `Spacer` in a wrapping flex line is the bug.** It takes the remaining width
and pushes everything after it onto the next line, so at 390px the bar came out
as the wordmark, then the scan bar, then the site and operator tags stranded on
a third line, left-aligned, reading as two stray buttons. `margin-left: auto`
on a tag *group* replaces it: when there is room the pair sits right, and when
there is not the pair wraps together. **Together** is the point — where you are
and who you are is one statement, and half of it alone on a line is worse than
both of it a line down.

Floor had this shape already, because at 430px it had no choice. The other three
inherited it rather than the reverse.

**Nothing is hidden, and that is the load-bearing half.** The obvious way to fit
a phone is to drop the locator or the site tag. D111 puts the locator on every
surface; the site tag is what tells an operator which warehouse their acts will
be recorded against. A chrome that drops either is a chrome that lies at exactly
the width where it matters most — so the cost is paid in rows, which is a
cheaper currency than truth.

**Three rows on a phone, not two.** The first draft of this said two, and the
measurements did not allow it: the wordmark, a screen name and both tags do not
fit in 390px once the wordmark's tracking is counted. Three rows that each say
one thing beat two rows with something missing.

**Floor's chrome stays smaller** — one override rather than a fourth copy. It
reads backwards against the density scale, whose whole point is that Floor's
type is *larger*; the exception is that this is the one element on a Floor
screen nobody reads at arm's length.

**`PlainShell` centres only on request.** Sign in is two fields and sat in the
top third of a phone with two-thirds of bare chassis beneath it. Centring is
`align="centre"` rather than the default, because passkeys and the site chooser
are lists, and centring a list moves every row when a row arrives. Auto margins
rather than `justify-content`, so a form taller than the space pins to the top
and scrolls instead of putting its first field above the top edge.

**Verified narrow, on every shell.** Bench, Desk and Plain at the 390px pocket;
Floor at the 430px handheld it is actually for. Both are below the breakpoint,
so no shell ships this unmeasured.

### D161 — The frame is mounted once, and a screen draws only its work

*Adopted 2026-08-21.*

**Decision.** `LightRoom`, `SessionProvider`, `Gate`, the chrome and the shell
are rendered above the route switch, in `Router` and `Framed`. A screen's
`render` returns its work and nothing else. The two regions a shell owns but a
screen's state fills — Floor's dock (D109/D134) and Desk's evidence panel (D111)
— are drawn by the screen and portalled into containers the shell always draws.

**What was wrong.** Every route returned a complete shell, and each shell mounted
its own `LightRoom` — whose own doc comment says *"mounted once, at the app
root"*. React reconciles by element type at a position, so navigating from
`/pack` to `/despatch` swapped `LivePack` for `LiveDespatch` at the top of the
tree and unmounted everything beneath it. Every navigation therefore:

- destroyed the room's canvas and restarted the light solver's critically damped
  spring, which its own comment says *"settles in a little under a second"* —
  the fade nobody designed and everybody saw;
- refetched `GET /sessions/current`, because `live()` wrapped each screen in its
  own `SessionProvider`;
- emptied the rail's badges to `{}` and refetched `GET /work`, so the counts
  blinked out and back on every screen change;
- rebuilt the chrome, taking the locator's focus and any half-typed scan with it.

None of that was a decision. It is what a route table returning whole pages
becomes once the pages acquire a room, a session and a rail.

**Why a portal rather than a prop.** The shell now outlives the screen, and the
dock's buttons and the evidence panel's contents are the *screen's* state. Passing
them upward would mean either the shell holding state it does not own, or the
router calling a screen for its return value rather than rendering it — and the
second is not a thing React does, because a function that calls hooks and is not
rendered as a component has no instance to keep them in. A portal moves the DOM
and nothing else: the dock is still a child of the screen for context, for events
and for `useCapture`, and it lands where the shell put the container.

**The containers are unconditional, and the stylesheet hides them empty.** A
shell cannot know whether the screen inside it will fill a region, and a screen
cannot portal into a container that only exists once it has asked for one. So
both are always drawn and `:empty` takes them out of the layout — one code path,
and an empty dock costs a handheld nothing. Fixtures still pass `dock` and
`evidence` as props into the same containers, because a fixture has no frame
above it.

**A stale scan is cleared on arrival.** `useChromeScan` used to be emptied by the
navigation destroying it. Now it survives, so an ambiguous scan's panel would
follow the operator to the next screen; it clears on a path change, explicitly.

**Gates:** a sixth, `npm run frame`. The render gate serves `dist-review` and
every route it visits is a fixture drawing its own shell, so the live frame was
checked by nothing. `check-frame.mjs` serves `dist` — what the Dockerfile copies
— answers the API with canned JSON, and asserts that the chrome, the rail and
the canvas are the *same DOM nodes* after a navigation, that the session is
fetched once, and that both slot containers are present and collapsed while
empty. Every one of those assertions was false a commit earlier.

### D162 — Below 62rem the rail is a sheet, and the chrome that opens it is pinned

*Adopted 2026-08-21.*

**Decision.** On Bench and Desk below 62rem, the work rail is drawn as a
full-screen opaque sheet opened by a **Menu** key in the chrome, and the chrome
is `position: sticky` at those widths. Above 62rem nothing changes: the rail is
the column it has always been and neither the key nor the dialog semantics
exist. Floor and Plain have no rail and are untouched.

**The rail was reachable and useless.** It stacked *under* the work
(`order: 2`), chosen over stacking above it because that put eleven navigation
rows between the top of a phone and the thing anybody came for. Both are the
same failure from opposite ends: to navigate, you scroll the entire worklist.
Fourteen destinations in five groups rule out a tab bar, and a bottom bar would
sit exactly where D134 puts the Floor dock.

**Pinning the chrome is not a separate nicety — it is what makes the sheet
worth anything.** A Menu key at the top of a list somebody has scrolled to the
bottom of is the rail-below problem with a new name. So wherever the rail is a
sheet, the bar that opens it stays. This extends D159 rather than contradicting
it: still three rows on a phone, still nothing dropped, and now they do not
scroll away. The bar covers the page's horizontal padding with a spread shadow —
a negative margin is forbidden by law 3 and a `100vw` width reintroduces the
sideways scroll this repository has already chased once.

**One rail in the document at every width.** Not a second copy revealed at
narrow. That was tried once for the landing screen and left two `nav` landmarks,
twenty links where there are ten destinations, and a hidden Sign out that a click
found before the visible one. The shell draws one `WorkRail`; `useNavSheet`
decides how it is presented, and the stylesheet gives it the full width of the
sheet so every destination is a thumb-width target.

**Opaque and full-screen, so D119 is not in play.** A translucent menu over a
worklist is exactly the case that law was written for — the rows of both showing
through each other. `--ano-950`, the same material as the Floor dock: chassis,
not paper, and depth from an occluding edge rather than a blur.

**"Menu", not "Work".** The landmark stays `Work` because that is what the list
is — D110 navigates by job rather than by entity. The key says what pressing it
does, which is what a control's label is for, and it is the word somebody holding
a scanner recognises without reading. It is an ordinary unlit `Key`: the rail
already puts one in the chrome for Sign out, so this is not a new kind of
control, and every hand-counted render expectation is untouched.

**Close at the bottom**, for the reason D134 docks Floor's primary action there,
with the same inset edge above it — Sign out is a key too and the second one is
destructive. Escape does the same thing for anyone with a keyboard, and focus
returns to whatever opened the sheet.

**Desk's rail breakpoint moves from 72rem to 62rem.** Two questions were sharing
one number: how wide before the *evidence panel* sits beside the work, and how
wide before the *rail* is a column. The first is still 72rem — the panel is 26rem
and needs the room. The second is the question Bench already answered, and it is
also where the sheet gives way, so it now has one answer. Between 62 and 72rem a
Desk screen is a rail and one work column, which is what Bench has been at that
width all along.

**62rem is written in four files** — both shells' stylesheets,
`nav-sheet.module.css` and `useNavSheet.ts` — because a media query cannot read a
custom property and this client has no CSS build step. Each says so.

**Gates.** Twenty assertions added to `npm run frame`, at 1440px, 800×900 and
390×844 — the middle one because between 48 and 62rem the chrome is a single row
*and* the rail is a sheet, which is a band neither of the other two measures:
that no dialog and no Menu key exist where the rail is a column; that at pocket
width the rail is out of the way, the chrome stays at `top: 0` after scrolling,
the sheet covers the viewport, focus lands inside it and returns to the key on
Escape, there is exactly one `nav[aria-label="Work"]` in the document, and that
choosing a destination both navigates *and* leaves nothing over the screen —
the sheet is closed by the path changing, not by the click — and that the screen
arrived at starts at the top, which the frame no longer gets for free from being
rebuilt.

### D165 — The app is a shell around the client, not a second client

*Adopted 2026-08-31. No migration. `mobile/`.*

**Decision.** The iOS and Android app is a Tauri 2 shell that runs the same
React bundle a browser runs. Three seams are rebound for a device — where the
server is, which HTTP client reaches it, and what credential it carries — and
nothing else. React Native is refused.

**What the alternative actually costs.** A React Native client is a second
interface: every screen, the four shells, the work rail, the scan bar, and the
whole of `design/` — 28 files, `LightRoom`'s solver, the material tokens, D128's
occlusion-not-paint rule. The render gate's 68 fixture screens at two densities
in light and dark would cover none of it, and this register's decisions from
D109 onward would describe one of the two clients. Two renderings of one state,
free to disagree, is the failure this repository has recorded more times than
any other.

**What the shell costs instead, and it is not nothing.** A webview is not a
native app: no native navigation transitions, no native scroll physics, and a
camera reachable only through what a plugin exposes. The judgement is that
Floor screens are lists, fields and a scan bar drawn on a design system built
for gloves at 430px, and that none of the three losses lands on them.

**The seams were already there, and one of them was written for this.**
`SignOnResponse.token` has said so since the day it existed — *"Returned once,
for clients that cannot hold a cookie — D5's handhelds. A browser ignores this
and uses the cookie."* `credentials: "same-origin"` appeared in exactly two
places and the base URL in one. That is the whole of what a device changes about
this client, and it is the strongest evidence available that the shell is the
right shape: a port that needed more would have been arguing with the design.

**A build mode, not a runtime check.** The obvious thing is
`"__TAURI_INTERNALS__" in window` and a lazy import, which ships the host to
every browser as a chunk relied upon not to load. D146 already settled how this
repository feels about that, for fixtures, in the commit that found the front
door pointing at one. So `vite build --mode mobile` produces what the shell
embeds, `import.meta.env.MODE` is a literal everywhere else, and law 9b asserts
that nothing on the live path imports `@tauri-apps/*` — the deployed bundle
contains no native code at all rather than none that runs.

**Why Tauri and not something else.** Because `~/dev/Nosdesk/mobile` is the same
shape, ships to TestFlight and Play, and carries two hand-written native plugins
of about two hundred lines each. The barcode scanner this exists for is a third
one. Choosing the stack somebody has already debugged on two stores is worth
more than choosing the stack with the better documentation.

**No store pipeline, and that is this register's own rule applied to itself.**
Fastlane lanes were written and removed in the same day: they uploaded to
TestFlight and Play, neither had been run, and there is no developer account for
this product yet. D163 says of its own readiness endpoint that it is *"built
against a caller that does not exist yet, which this register usually refuses"*
and takes it anyway for a stated reason. There is no such reason here — a
working pair of lanes is one directory away when there is an account to point
them at. Nothing committed ties the app to a developer account: the bundle
identifier is its own and the team ID is never in the repository, so moving
Nylonite onto a separate account later costs one environment variable.
