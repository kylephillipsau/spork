import { strict as assert } from "node:assert";
import { test } from "node:test";

import { Outbox, transient, type Held, type Store } from "./outbox.ts";

/** A store that outlives the outbox, as a device's storage outlives a reload. */
function memory(): Store & { raw: () => string | null } {
  let value: string | null = null;
  return { get: () => value, set: (v) => (value = v), raw: () => value };
}

const SAM = "sam:ws:mel";
const held = (key: string, owner = SAM): Held<{ n: number }> => ({ key, owner, ownerName: "Sam", body: { n: Number(key.slice(1)) } });

/** An error as `ApiError` makes one, with a status, or a network failure without. */
const answered = (status: number, message = "no") => Object.assign(new Error(message), { status });
const unreachable = () => new TypeError("Failed to fetch");

test("what is kept before sending survives a reload", () => {
  const store = memory();
  new Outbox(store).put(held("p1"));
  assert.deepEqual(
    new Outbox<{ n: number }>(store).list().map((e) => e.key),
    ["p1"],
    "a new outbox on the same store still has it",
  );
});

test("a press kept twice is one entry, in its first place", () => {
  const box = new Outbox<{ n: number }>(memory());
  box.put(held("p1"));
  box.put(held("p2"));
  box.put({ ...held("p1"), body: { n: 9 } });
  assert.deepEqual(box.list().map((e) => [e.key, e.body.n]), [["p1", 9], ["p2", 2]]);
});

test("sent acts leave, oldest first, and each reply is heard before its act leaves", async () => {
  const box = new Outbox<{ n: number }>(memory());
  box.put(held("p1"));
  box.put(held("p2"));
  const order: string[] = [];
  const left = await box.drain(
    SAM,
    async (body) => body.n * 10,
    (entry, reply) => order.push(`${entry.key}:${reply}:${box.list().length}`),
  );
  assert.equal(left, 0);
  assert.deepEqual(order, ["p1:10:2", "p2:20:1"], "the reply arrives while the act is still kept");
});

test("no connection holds everything, and stops at the first", async () => {
  const box = new Outbox<{ n: number }>(memory());
  box.put(held("p1"));
  box.put(held("p2"));
  let tries = 0;
  const left = await box.drain(SAM, async () => {
    tries += 1;
    throw unreachable();
  });
  assert.equal(left, 2);
  assert.equal(tries, 1, "the second would not have got through either");
});

test("a refusal is kept to be read, and does not hold up the rest", async () => {
  const box = new Outbox<{ n: number }>(memory());
  box.put(held("p1"));
  box.put(held("p2"));
  const left = await box.drain(SAM, async (body) => {
    if (body.n === 1) throw answered(400, "That line is cancelled.");
    return body.n;
  });
  assert.equal(left, 0, "nothing is waiting");
  assert.deepEqual(box.list(), [{ ...held("p1"), refused: "That line is cancelled." }]);
  const again = await box.drain(SAM, async () => assert.fail("a refused act is not sent again"));
  assert.equal(again, 0);
  box.drop("p1");
  assert.deepEqual(box.list(), []);
});

test("somebody else's acts wait for them", async () => {
  const box = new Outbox<{ n: number }>(memory());
  box.put(held("p1", "priya:ws:mel"));
  box.put(held("p2"));
  const sent: string[] = [];
  await box.drain(SAM, async (body) => sent.push(String(body.n)));
  assert.deepEqual(sent, ["2"]);
  assert.deepEqual(box.list().map((e) => e.key), ["p1"], "Priya's is still kept");
});

test("a lapsed session or a server error is worth trying again; a refusal is not", () => {
  for (const error of [unreachable(), answered(401), answered(408), answered(429), answered(500), answered(503)]) {
    assert.ok(transient(error), String((error as { status?: number }).status ?? "no answer"));
  }
  for (const error of [answered(400), answered(403), answered(404), answered(409), answered(422)]) {
    assert.ok(!transient(error), String(error.status));
  }
});

test("a store that can't be read is an empty outbox, not an error", () => {
  const box = new Outbox({ get: () => "{not json", set: () => {} });
  assert.deepEqual(box.list(), []);
});
