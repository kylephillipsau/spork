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
    select: () => {},
    change: () => {},
    begin: () => {},
    drag: () => {},
    add: () => {},
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
    ...read.map((d) => (d.name === "Rack G" ? { ...d, box: { ...turned(d.box, 90), x: d.box.x + 14 } } : d)),
    {
      place_id: "01990000-0000-7000-8000-0000000ba11c",
      parent_id: building.place_id,
      name: "Back wall",
      solid: true,
      box: { x: 0, y: building.box.depth - 0.5, z: 0, length: building.box.length, depth: 0.5, height: 4, turn: 0 },
      outline: null,
      sides: 1,
      bins: 0,
      fresh: true,
    },
  ];
  return deskOf(drafts, { selected: drafts.find((d) => d.name === "Rack G") ?? null, canUndo: true, cellMm: 1000, ...over });
}
