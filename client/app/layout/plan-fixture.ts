import { changeCount, draftsOf, planOf, turned, type Draft } from "./edit";
import { DRAFTED_SITE } from "./fixture";
import type { PlanDesk } from "./usePlanEditor";

/**
 * The plan editor, reachable with no network (D209): the drafted site as
 * read, and the same with Rack G turned a quarter and moved, and a wall drawn
 * along the back, so Save has something to say.
 */
function deskOf(drafts: Draft[], over: Partial<PlanDesk> = {}): PlanDesk {
  return {
    read: { kind: "ready", value: DRAFTED_SITE },
    drafts,
    plan: planOf(drafts),
    selected: null,
    chosen: [],
    select: () => {},
    selectAll: () => {},
    change: () => {},
    place: () => {},
    begin: () => {},
    drag: () => {},
    add: () => {},
    tray: {
      kind: "ready",
      value: ["PACK", "DOCK-1", "3PL"].map((code, i) => ({
        location_id: `0b1d0000-0000-7000-8000-00000000f0${String(i).padStart(2, "0")}`,
        code,
        kind: "staging",
        place_id: null,
        place_name: null,
        cell: null,
        whereabouts: null,
        pick_sequence: null,
        within_reach: true,
        reported: [],
        reported_items: 0,
        held: 0,
      })),
    },
    placeBin: () => {},
    remove: () => {},
    undo: () => {},
    redo: () => {},
    canUndo: false,
    canRedo: false,
    count: changeCount(DRAFTED_SITE, drafts),
    cellMm: null,
    measureInMetres: async () => {},
    save: async () => {},
    discard: () => {},
    busy: false,
    problem: null,
    said: null,
    dismiss: () => {},
    ...over,
  };
}

export function fixturePlan(over: Partial<PlanDesk> = {}): PlanDesk {
  return deskOf(draftsOf(DRAFTED_SITE), over);
}

export function fixturePlanEdited(over: Partial<PlanDesk> = {}): PlanDesk {
  const read = draftsOf(DRAFTED_SITE);
  const building = read.find((d) => d.parent_id === null)!;
  const drafts: Draft[] = [
    ...read.map((d) => (d.name === "Rack G" ? { ...d, box: { ...turned(d.box, 90), x: d.box.x + 14 }, from_right: true } : d)),
    {
      place_id: "01990000-0000-7000-8000-0000000ba11c",
      parent_id: building.place_id,
      name: "Back wall",
      solid: true,
      box: { x: 0, y: building.box.depth - 0.5, z: 0, length: building.box.length, depth: 0.5, height: 4, turn: 0 },
      outline: null,
      sides: 1,
      from_right: false,
      bays: 1,
      levels: 1,
      bins: 0,
      fresh: true,
    },
  ];
  const g = drafts.find((d) => d.name === "Rack G") ?? null;
  return deskOf(drafts, { selected: g, chosen: g ? [g] : [], canUndo: true, cellMm: 1000, ...over });
}

/** Three racks chosen together, to set out in a row or size from their bays. */
export function fixturePlanChosen(over: Partial<PlanDesk> = {}): PlanDesk {
  const drafts = draftsOf(DRAFTED_SITE);
  const chosen = drafts.filter((d) => ["Rack E", "Rack F", "Rack G"].includes(d.name));
  return deskOf(drafts, { chosen, cellMm: 1000, ...over });
}
