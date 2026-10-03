import type { WorkspacePerson } from "@domain/types";
import type { PeopleBench, PeopleState } from "./usePeople";

const person = (n: number, over: Partial<WorkspacePerson> & Pick<WorkspacePerson, "display_name">): WorkspacePerson => ({
  person_id: `9e070000-0000-0000-0000-00000000000${n}`,
  email: `${over.display_name.toLowerCase().split(" ")[0]?.replace(/[^a-z]/g, "")}@example.test`,
  role: "operator",
  joined_at: "2026-10-04T08:00:00Z",
  left_at: null,
  password: true,
  passkeys: 0,
  you: false,
  ...over,
});

/** A small crew: the administrator, two pickers, and one who has left. */
export const PEOPLE_READY: PeopleState = {
  kind: "ready",
  people: [
    person(1, { display_name: "Dana Stooke", role: "administrator", passkeys: 1, you: true }),
    person(2, { display_name: "Sam Rivera" }),
    person(3, { display_name: "Priya Nair", passkeys: 2 }),
    person(4, { display_name: "Casey Ho", left_at: "2026-09-30T05:00:00Z" }),
  ],
};

const noop = async () => {};

export function fixturePeople(state: PeopleState, over: Partial<PeopleBench> = {}): PeopleBench {
  return { state, busy: false, problem: null, said: null, dismiss: () => {}, add: async () => true, setRole: noop, remove: noop, ...over };
}
