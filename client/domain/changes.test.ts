import { strict as assert } from "node:assert";
import { test } from "node:test";

import { backoff, Channel, FrameReader, type Timings } from "./changes.ts";

test("frames are read whole however the stream is cut", () => {
  const reader = new FrameReader();
  assert.deepEqual(reader.push("retry: 3000\nevent: hel"), []);
  assert.deepEqual(reader.push("lo\ndata: {}\n\n: keepalive\n\nevent: changed\n"), [{ event: "hello", data: "{}" }]);
  assert.deepEqual(reader.push("data: {}\n\n"), [{ event: "changed", data: "{}" }]);
});

test("a comment is not a frame, and a frame without an event is a message", () => {
  const reader = new FrameReader();
  assert.deepEqual(reader.push(": keepalive\n\n"), []);
  assert.deepEqual(reader.push("data: a\ndata: b\r\n\r\n"), [{ event: "message", data: "a\nb" }]);
});

test("the wait before trying again doubles, to half a minute", () => {
  assert.deepEqual([0, 1, 2, 3, 4, 5, 9].map(backoff), [1000, 2000, 4000, 8000, 16000, 30000, 30000]);
});

const FAST: Timings = { helloWithin: 200, settle: 5, pollEvery: 15, retry: () => 10_000 };
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** A stream the test writes to, as the server would. */
function stream() {
  let push!: (text: string) => void;
  let end!: () => void;
  const body = new ReadableStream<Uint8Array>({
    start(controller) {
      const bytes = new TextEncoder();
      push = (text) => controller.enqueue(bytes.encode(text));
      end = () => controller.close();
    },
  });
  return { response: new Response(body, { status: 200 }), push: (t: string) => push(t), end: () => end() };
}

test("a burst of changes is one read, and the first hello is not one", async () => {
  const s = stream();
  const channel = new Channel(async () => s.response, FAST);
  let reads = 0;
  const stop = channel.subscribe(() => reads++);
  s.push("event: hello\ndata: {}\n\n");
  await wait(20);
  assert.equal(channel.standing, "live");
  assert.equal(reads, 0, "the screen has just read for itself");
  s.push("event: changed\ndata: {}\n\nevent: changed\ndata: {}\n\nevent: changed\ndata: {}\n\n");
  await wait(30);
  assert.equal(reads, 1);
  stop();
  assert.equal(channel.standing, "off");
});

test("a stream that never says hello is no stream, and the device asks on a timer", async () => {
  const channel = new Channel(async () => stream().response, FAST);
  let reads = 0;
  const stop = channel.subscribe(() => reads++);
  await wait(300);
  assert.equal(channel.standing, "polling");
  assert.ok(reads >= 2, `told on the timer, ${reads} times`);
  stop();
});

test("a stream that ends is opened again, and the gap is read", async () => {
  const streams = [stream(), stream()];
  let opened = 0;
  const channel = new Channel(async () => streams[opened++]!.response, FAST);
  let reads = 0;
  const stop = channel.subscribe(() => reads++);
  streams[0]!.push("event: hello\ndata: {}\n\n");
  await wait(20);
  streams[0]!.end();
  await wait(20);
  assert.equal(opened, 2, "straight back");
  streams[1]!.push("event: hello\ndata: {}\n\n");
  await wait(20);
  assert.equal(reads, 1, "whatever happened between the two was read once");
  stop();
});
