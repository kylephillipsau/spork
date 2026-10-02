# Handoff: capture on the phone, crops at the computer, search

Written 2026-10-02. Read this first. The earlier handoff,
[handoff-orders-bridge-3d.md](./handoff-orders-bridge-3d.md), still covers the
toolchain, tests, layout and packing.

## State

- Spork `main` is ahead of `origin/main` and not pushed.
- `warehouse-scripts` is pushed. Spork Bridge 0.4.0 is published.
- Migration 111 applies on the next `local.ps1 start`.
- The full server suite passed on a fresh database before D191. Run it again
  before pushing.

## What landed

Each item is one decision in [domain-model.md](./domain-model.md).

**Measuring items**
- D178. An item's carton is a box of N of the item. The carton card asks how
  many are in it and records the case pack before its figures
  (`POST /items/{id}/carton`, migration 101).
- D185. A carton can hold packs: 6 packs of 24. An item whose case pack has
  packs gets an Inner pack card.
- D190. On an item's page the carton card is that item's own. A family's
  figures show on it, marked as the family's, until the item is measured.
- D182, D184. A print run that looks different is a variant (a lot of the
  item) with its own card, photos and figures (`POST /items/{id}/lots`,
  migration 104). One variant can stand for the item's carton (migration 106).

**Lists and reach**
- D179. A sheet of items is a list: codes pasted in order, worked in that
  order from the item list (`POST /item-lists`, `GET /items?list=`, migration
  102).
- D180. A rack says how many levels are reachable from the floor
  (`POST /places/{id}/reach`, migration 103). The item list sends you to the
  reachable bin with the most stock. Off the layout, NetSuite's Pick bins
  count as reachable. There is no screen yet to set reach.

**Photos**
- D181. A phone takes photos and never runs the face-finder. It needs about
  1.5 GB above the page, and iOS reloads the tab. Photos to crop (`/photos`)
  finds each face at a computer. Each photo has Save, or Adjust first.
- The panel offers the next side as one button, in the order front, right,
  back, left, top, bottom, label. Photos send in the background.
- D183. A side printed like its opposite is recorded as "Same as front" and
  shares that photo and its crop (migration 105).
- D190. Photos to crop shows whose photos they are. Move refiles a whole look
  under the right item and level, and records the original as moved
  (migration 110). Use as taken keeps a photo uncropped.
- D186. Once front, right and top are cropped, the computer draws the item as
  a 3D box and the list shows that drawing (migration 107).
- D188. One item can picture its family (migration 109).
- D191. Each card says what it is packed in, as a GS1 packaging type code
  (migration 111). A case or box is photographed side by side and drawn.
  Anything else gets a photo, then back, label or close-up, and is never
  cropped or drawn.

**NetSuite**
- D187. The bridge asks Spork which NetSuite fulfilments are still open and
  reports those NetSuite packed, shipped or deleted. Spork closes them
  (migration 108). The first run closed 24 of 32.

**Search**
- D189. The header box searches items, bins and orders as you type
  (`GET /search`). Tantivy, one index per tenant, in memory, rebuilt from the
  database at most a minute after a change. On a phone it opens full screen.

## Known limits

- Firefox finds faces about 13 times slower than Chrome or Edge: 78 s against
  6 s per photo on this PC.
- Search has not been timed on the real catalogue.
- Moving a look leaves its "same as" sides on the old subject.
- The bin's reach has no screen. Set it with `POST /places/{id}/reach` or wait
  for the Warehouse setting.

## Next

1. Reach on the Warehouse screen and rack face, and a forklift mark on the
   item list's bin.
2. A completed sheet for a list: each item with its own and its carton's
   figures, as CSV.
3. Apply the real layout, so A to D can say which levels are reachable.
4. Push Spork once the whole suite passes.
