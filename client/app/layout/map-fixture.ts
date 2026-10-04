import type { BinRow, MapBin, MapBins } from "@domain/types";

import { DRAFTED_SITE } from "./fixture";
import type { MapDesk, MapSite } from "./useMap";

/**
 * The bin map, reachable with no network (D208): the drafted site's racks,
 * every cell holding a bin, and NetSuite's count spread across them so the
 * layer has something to say. Only the floor level is in reach, as the
 * drafted racks say.
 */
function binsOf(): MapBin[] {
  const out: MapBin[] = [];
  for (const place of DRAFTED_SITE.places) {
    if (!place.solid || !place.pattern) continue;
    const letter = place.name.slice(-1);
    for (let side = 1; side <= place.sides; side++) {
      for (let bay = 1; bay <= place.bays; bay++) {
        for (let level = 1; level <= place.levels; level++) {
          const label = side === 2 ? 2 * place.bays - bay + 1 : bay;
          const items = (bay * 7 + level * 3 + side * 5 + place.name.length) % 6;
          out.push({
            location_id: `0b1d0000-0000-7000-8000-${place.place_id.slice(-4)}${String(side)}${String(bay).padStart(3, "0")}${String(level).padStart(4, "0")}`,
            code: `${letter}-${String(label).padStart(2, "0")}-${level}`,
            place_id: place.place_id,
            side,
            bay,
            level,
            row: 1,
            position: 1,
            within_reach: level <= place.reach_levels,
            reported_items: items > 3 ? 0 : items,
            reported_on_hand: items > 3 ? 0 : items * 12,
            held: 0,
          });
        }
      }
    }
  }
  return out;
}

// NetSuite's count from a few minutes ago, as the Bridge keeps it (D212).
const BINS: MapBins = { bins: binsOf(), unplaced: 42, reported_as_at: new Date(Date.now() - 4 * 60_000).toISOString() };

export const MAP_SITE: MapSite = { layout: DRAFTED_SITE, bins: BINS };

/** A bin on the back of Rack G, holding two items. */
export const MAP_CHOSEN: MapBin =
  BINS.bins.find((b) => b.code.startsWith("G-") && b.side === 2 && b.level === 2 && b.reported_items === 2) ?? BINS.bins[0]!;

export const MAP_CHOSEN_DETAIL: BinRow = {
  location_id: MAP_CHOSEN.location_id,
  code: MAP_CHOSEN.code,
  kind: "pick_face",
  place_id: MAP_CHOSEN.place_id,
  place_name: "Rack G",
  cell: { bay: MAP_CHOSEN.bay, level: MAP_CHOSEN.level, row: 1, position: 1, side: MAP_CHOSEN.side },
  whereabouts: `back, bay ${MAP_CHOSEN.code.split("-")[1]}, level ${MAP_CHOSEN.level}`,
  pick_sequence: null,
  within_reach: MAP_CHOSEN.within_reach,
  reported: [
    { item_id: "17e10000-0000-0000-0000-000000000003", item_code: "GLOVE-M", on_hand: "24" },
    { item_id: "17e10000-0000-0000-0000-000000000004", item_code: "GLOVE-L", on_hand: "12" },
  ],
  reported_items: 2,
  held: 0,
};

export function fixtureMap(over: Partial<MapDesk> = {}): MapDesk {
  return {
    read: { kind: "ready", value: MAP_SITE },
    layer: "stock",
    setLayer: () => {},
    chosen: null,
    place: null,
    choose: () => {},
    find: () => {},
    flight: 0,
    detail: { kind: "idle" },
    setReach: async () => {},
    busy: false,
    problem: null,
    dismiss: () => {},
    showWalk: false,
    setShowWalk: () => {},
    walk: { kind: "idle" },
    ...over,
  };
}

/** The same, with a bin chosen and its card filled in. */
export function fixtureMapChosen(over: Partial<MapDesk> = {}): MapDesk {
  return fixtureMap({
    chosen: MAP_CHOSEN,
    place: DRAFTED_SITE.places.find((p) => p.place_id === MAP_CHOSEN.place_id) ?? null,
    detail: { kind: "ready", value: MAP_CHOSEN_DETAIL },
    ...over,
  });
}

/**
 * Today's walk on the drafted site: from the bench at the front, up the left
 * side, along an aisle between two racks and back.
 */
export const MAP_WALK = {
  from: "pack" as const,
  cell_mm: 1000,
  walked: 62.5,
  typed: 97,
  stops: 6,
  off_route: 0,
  path: [
    [0.5, 0.5],
    [0.5, 34.5],
    [12, 34.5],
    [12, 37.5],
    [3, 37.5],
    [0.5, 35],
    [0.5, 0.5],
  ] as [number, number][],
};
