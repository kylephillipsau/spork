import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { api } from "@domain/api";
import { deviceStore, Outbox, type Held } from "@domain/outbox";
import type { RecordPickResponse, Uuid } from "@domain/types";

import { useChanges } from "./changes";

/**
 * THE REACT SIDE OF THE OUTBOX (D207).
 *
 * `domain/outbox.ts` keeps acts on the device and sends them; this is how an
 * outbox is made and how a screen reads it. Two are kept: picks, made where
 * the wifi is worst, and the pack bench's presses (`usePackBench.ts`).
 */

/** A pick, as kept: enough to send it again as the same act. */
export interface HeldPick {
  line: Uuid;
  stock: Uuid;
  to: { kind: "package" | "location"; id: Uuid };
  quantity: number;
  claim: number;
  /** The ids the act minted, so a second send is the same act (D5). */
  ids: { event: Uuid; allocation: Uuid };
  /** When the picker pressed, not when it was sent. */
  at: string;
  /** What and where, to say so while it waits. */
  code: string;
  where: string;
}

/** An outbox kept on this device under `name`, told when another tab on it sends or keeps one. */
export function kept<T>(name: string): Outbox<T> {
  const box = new Outbox<T>(deviceStore(name));
  if (typeof window !== "undefined") {
    window.addEventListener("storage", (e) => {
      if (e.key === name) box.changed();
    });
  }
  return box;
}

export const picks = kept<HeldPick>("spork.outbox.picks");

/** Send a kept pick as the act it was. */
export function sendPick(body: HeldPick): Promise<RecordPickResponse> {
  const ids: Record<string, Uuid> = body.ids;
  return api.pick({
    line: body.line,
    stock: body.stock,
    to: body.to,
    quantity: body.quantity,
    claim: body.claim,
    act: {
      id: (name) => {
        const id = ids[name];
        if (id === undefined) throw new Error(`No ${name} id was kept for this pick.`);
        return id;
      },
      at: body.at,
    },
    pickAnyway: true,
  });
}

/** How often to try while something waits, besides coming online and hearing a change. */
const RETRY_EVERY = 15_000;

export interface KeptView<T> {
  /** This person's acts waiting to be sent, oldest first. */
  waiting: Held<T>[];
  /** This person's acts the server refused when they were sent. */
  refused: Held<T>[];
  /** Acts somebody else made on this device, waiting for them to sign in here. */
  others: { name: string; count: number }[];
  /** Try now. */
  send: () => Promise<void>;
  /** The refusal has been read. */
  dismiss: (key: string) => void;
}

/**
 * An outbox as `owner` sees it, sent whenever there is a chance it gets
 * through: on arrival, when the device comes back online, when the live
 * channel hears anything, and every so often while something waits. `sent`
 * hears each reply before its act leaves the outbox.
 */
export function useOutbox<T, R>(
  box: Outbox<T>,
  owner: string | null,
  sendOne: (body: T, entry: Held<T>) => Promise<R>,
  sent?: (entry: Held<T>, reply: R) => void,
): KeptView<T> {
  const [all, setAll] = useState(() => box.list());
  useEffect(() => box.subscribe(() => setAll(box.list())), [box]);

  const latest = useRef(sent);
  latest.current = sent;

  const send = useCallback(async () => {
    if (!owner) return;
    await box.drain(owner, sendOne, (entry, reply) => latest.current?.(entry, reply));
  }, [box, owner, sendOne]);

  const view = useMemo(() => {
    const mine = all.filter((e) => e.owner === owner);
    const others = new Map<string, number>();
    for (const e of all) if (e.owner !== owner && e.refused === undefined) others.set(e.ownerName, (others.get(e.ownerName) ?? 0) + 1);
    return {
      waiting: mine.filter((e) => e.refused === undefined),
      refused: mine.filter((e) => e.refused !== undefined),
      others: [...others].map(([name, count]) => ({ name, count })),
    };
  }, [all, owner]);

  useEffect(() => {
    void send();
  }, [send]);
  useChanges(() => void send(), owner !== null);
  useEffect(() => {
    const online = () => void send();
    window.addEventListener("online", online);
    return () => window.removeEventListener("online", online);
  }, [send]);
  const something = view.waiting.length > 0;
  useEffect(() => {
    if (!something) return;
    const timer = setInterval(() => void send(), RETRY_EVERY);
    return () => clearInterval(timer);
  }, [something, send]);

  return { ...view, send, dismiss: (key) => box.drop(key) };
}
