import { useCallback, useEffect, useState } from "react";

import { useLive } from "@app/acting";
import { api, reason } from "@domain/api";
import type { WorkspacePerson } from "@domain/types";

/**
 * The workspace's people (D205): who can sign in to it, and as what. An
 * administrator adds a person with a first password they change under
 * Account, says what each is, and takes somebody out, whose history stays
 * theirs. Settings rather than acts, so each change is followed by a re-read.
 */

export type PeopleState =
  | { kind: "loading" }
  | { kind: "ready"; people: WorkspacePerson[] }
  | { kind: "failed"; message: string };

export interface PeopleBench {
  state: PeopleState;
  busy: boolean;
  problem: string | null;
  /** What the last change did, in a sentence. */
  said: string | null;
  dismiss: () => void;
  /** True when the person was added. */
  add: (input: { name: string; email: string; password: string; role: WorkspacePerson["role"] }) => Promise<boolean>;
  setRole: (person: WorkspacePerson, role: WorkspacePerson["role"]) => Promise<void>;
  remove: (person: WorkspacePerson) => Promise<void>;
}

/** The shortest first password, as the server says (credentials.rs). */
export const LEAST_PASSWORD = 12;

/**
 * A first password to read out or write down: fourteen letters and digits,
 * none that look alike. From `getRandomValues`, which a page over plain HTTP
 * has, unlike `crypto.subtle` (local.md).
 */
export function firstPassword(): string {
  const alphabet = "abcdefghjkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
  const bytes = new Uint8Array(14);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => alphabet[b % alphabet.length]).join("");
}

export function usePeople(): PeopleBench {
  const live = useLive();
  const [state, setState] = useState<PeopleState>({ kind: "loading" });
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const [said, setSaid] = useState<string | null>(null);

  const read = useCallback(async () => {
    try {
      const people = await api.people();
      if (live.current) setState({ kind: "ready", people });
    } catch (error) {
      if (live.current) setState({ kind: "failed", message: reason(error, "The server could not be reached.") });
    }
  }, [live]);

  useEffect(() => {
    void read();
  }, [read]);

  const change = useCallback(
    async (act: () => Promise<string>, failure: string): Promise<boolean> => {
      setBusy(true);
      setProblem(null);
      setSaid(null);
      try {
        const words = await act();
        if (live.current) setSaid(words);
        await read();
        return true;
      } catch (error) {
        if (live.current) setProblem(reason(error, failure));
        return false;
      } finally {
        if (live.current) setBusy(false);
      }
    },
    [read, live],
  );

  return {
    state,
    busy,
    problem,
    said,
    dismiss: () => {
      setProblem(null);
      setSaid(null);
    },
    add: ({ name, email, password, role }) =>
      change(async () => {
        const added = await api.addPerson({ display_name: name.trim(), email: email.trim(), password, role });
        const left = state.kind === "ready" && state.people.some((p) => p.person_id === added.person_id && p.left_at);
        if (left) return `${name.trim()} is back, and signs in as they did before.`;
        return added.existing
          ? `${name.trim()} already signs in to another workspace, so they keep their own password.`
          : `${name.trim()} can sign in now with that password, and change it under Account.`;
      }, "Could not add that person."),
    setRole: async (person, role) => {
      await change(async () => {
        await api.setPersonRole(person.person_id, role);
        return `${person.display_name} is ${role === "administrator" ? "an administrator" : "an operator"} now.`;
      }, "Could not change what they are.");
    },
    remove: async (person) => {
      await change(async () => {
        await api.removePerson(person.person_id);
        return `${person.display_name} can no longer sign in here. What they recorded still names them.`;
      }, "Could not take them out of the workspace.");
    },
  };
}
