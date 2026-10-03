/**
 * THE DURABLE OUTBOX (D207).
 *
 * A picker in a dead spot presses Take, and the request goes nowhere. Until
 * now the act lived in the screen's memory (`pressing` in acts.ts), so a
 * reload or a flat battery lost it, and the goods in the tote had no record.
 * Here an act is written to the device before it is sent, and stays until the
 * server has it.
 *
 * **Sending again is safe because of D5.** An act carries the ids it minted
 * and the moment it was pressed, so a second copy of one that did land is a
 * replay the server answers with what it recorded, not a second pick.
 *
 * **What the server refuses is not retried.** A network failure, a server
 * error, or a session that has lapsed holds the act for later. Any other
 * refusal is the server's answer, and is kept beside the queue for the person
 * to read, not sent again and again. One refused act does not hold up the ones
 * behind it.
 *
 * **An act is sent as the person who did it.** Each one names its owner, the
 * person, workspace and site of the session that recorded it, and is sent only
 * under that session. On a shared handheld somebody else's waiting picks are
 * shown, never sent as theirs.
 *
 * Pure apart from the store it is given, so it runs under `node --test`.
 */

/** Somewhere to keep a string across a reload. */
export interface Store {
  get: () => string | null;
  set: (value: string) => void;
}

export interface Held<T> {
  /** The act's key: the same press is the same entry. */
  key: string;
  /** `person:tenant:site` of the session that recorded it. */
  owner: string;
  /** Who that is, to say whose picks are waiting. */
  ownerName: string;
  /** Everything needed to send it again, its ids and its moment included. */
  body: T;
  /** The server's refusal, once it gave one. Kept until dismissed. */
  refused?: string;
}

/** Whether a failure is worth trying again: no answer, or not this act's fault. */
export function transient(error: unknown): boolean {
  const status = (error as { status?: unknown } | null)?.status;
  if (typeof status !== "number" || status === 0) return true;
  return status === 401 || status === 408 || status === 425 || status === 429 || status >= 500;
}

/** The message a refusal carries. */
function said(error: unknown): string {
  return error instanceof Error && error.message ? error.message : "The server refused it.";
}

/**
 * The browser's own storage, falling back to memory where that is blocked or
 * full. A device that can't keep acts across a reload still keeps them until
 * one, which is what it did before.
 */
export function deviceStore(name: string): Store {
  let memory: string | null = null;
  return {
    get: () => {
      try {
        return globalThis.localStorage?.getItem(name) ?? memory;
      } catch {
        return memory;
      }
    },
    set: (value) => {
      memory = value;
      try {
        globalThis.localStorage?.setItem(name, value);
      } catch {
        /* memory has it */
      }
    },
  };
}

export class Outbox<T> {
  private readonly store: Store;
  private readonly listeners = new Set<() => void>();
  private sending = false;

  constructor(store: Store) {
    this.store = store;
  }

  /** Everything waiting or refused, oldest first. Read afresh, because another tab may have sent some. */
  list(): Held<T>[] {
    try {
      const parsed: unknown = JSON.parse(this.store.get() ?? "[]");
      return Array.isArray(parsed) ? (parsed as Held<T>[]) : [];
    } catch {
      return [];
    }
  }

  /** Keep an act before sending it. The same key replaces, and keeps its place. */
  put(entry: Held<T>): void {
    const all = this.list();
    const at = all.findIndex((e) => e.key === entry.key);
    if (at >= 0) all[at] = entry;
    else all.push(entry);
    this.save(all);
  }

  /** The server has it, or the person has read why not. */
  drop(key: string): void {
    const all = this.list();
    if (!all.some((e) => e.key === key)) return;
    this.save(all.filter((e) => e.key !== key));
  }

  refuse(key: string, reason: string): void {
    this.save(this.list().map((e) => (e.key === key ? { ...e, refused: reason } : e)));
  }

  /** Be told when anything is kept, sent, refused or dropped. */
  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** Another tab changed the store. */
  changed(): void {
    this.tell();
  }

  /**
   * Send this owner's waiting acts, oldest first, one at a time. Stops at the
   * first that can't be sent for want of a connection, because the ones behind
   * it won't be either. `sent` hears each reply before its act is dropped, so a
   * screen can show what the server recorded without a gap.
   *
   * `busy` names acts a screen is sending itself right now. They are left to
   * it, so its answer is the only one: a press shows the server's refusal,
   * and the same act sent from here at the same moment must not record
   * what the press was told was refused.
   *
   * Returns how many of this owner's are still waiting.
   */
  async drain<R>(
    owner: string,
    send: (body: T) => Promise<R>,
    sent?: (entry: Held<T>, reply: R) => void,
    busy?: (key: string) => boolean,
  ): Promise<number> {
    if (!this.sending) {
      this.sending = true;
      try {
        for (const entry of this.list()) {
          if (entry.owner !== owner || entry.refused !== undefined || busy?.(entry.key)) continue;
          // Sent or dropped elsewhere while this one was on its way.
          if (!this.list().some((e) => e.key === entry.key && e.refused === undefined)) continue;
          try {
            const reply = await send(entry.body);
            sent?.(entry, reply);
            this.drop(entry.key);
          } catch (error) {
            if (transient(error)) break;
            this.refuse(entry.key, said(error));
          }
        }
      } finally {
        this.sending = false;
      }
    }
    return this.list().filter((e) => e.owner === owner && e.refused === undefined).length;
  }

  private save(all: Held<T>[]): void {
    this.store.set(JSON.stringify(all));
    this.tell();
  }

  private tell(): void {
    for (const listener of [...this.listeners]) listener();
  }
}
