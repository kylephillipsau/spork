import { strict as assert } from "node:assert";
import { test } from "node:test";

import { b64urlToBytes, bytesToB64url, declined } from "./webauthn.ts";

/**
 * The transcoding, which lived twice as inlined JavaScript in two maud pages
 * and was never tested in either. It is the one part of a passkey ceremony
 * whose failure looks like a rejected key rather than like a bug.
 */

test("a round trip returns the bytes it was given", () => {
  const bytes = Uint8Array.from({ length: 256 }, (_, i) => i);
  assert.deepEqual(b64urlToBytes(bytesToB64url(bytes)), bytes);
});

test("the alphabet is url-safe, so no + or / survives", () => {
  // 0xFB 0xFF encodes to `+/` in standard base64, which is the pair that has to
  // change. A credential id travels in JSON and sometimes in a path.
  const encoded = bytesToB64url(Uint8Array.from([0xfb, 0xff, 0xbf]));
  assert.equal(encoded.includes("+"), false);
  assert.equal(encoded.includes("/"), false);
  assert.deepEqual(b64urlToBytes(encoded), Uint8Array.from([0xfb, 0xff, 0xbf]));
});

test("padding is dropped on the way out and restored on the way in", () => {
  // One, two and three byte inputs are the three tail lengths, and the two that
  // need padding are exactly what `atob` refuses without it.
  for (const n of [1, 2, 3, 4, 5]) {
    const bytes = Uint8Array.from({ length: n }, (_, i) => i + 1);
    const encoded = bytesToB64url(bytes);
    assert.equal(encoded.includes("="), false, `${n} bytes encoded with padding`);
    assert.deepEqual(b64urlToBytes(encoded), bytes, `${n} bytes did not survive`);
  }
});

test("an empty buffer is empty rather than an error", () => {
  assert.equal(bytesToB64url(new Uint8Array()), "");
  assert.deepEqual(b64urlToBytes(""), new Uint8Array());
});

test("a large attestation object does not overflow the call stack", () => {
  // `String.fromCharCode(...bytes)` throws somewhere around 100k arguments, and
  // an attestation object carrying a certificate chain is the realistic way to
  // get there. This is the reason the encoder appends a byte at a time.
  const big = Uint8Array.from({ length: 200_000 }, (_, i) => i % 256);
  const encoded = bytesToB64url(big);
  assert.deepEqual(b64urlToBytes(encoded), big);
});

test("saying no is not a failure, and is told apart from one", () => {
  // Two screens read this, and both of them used to draw a dismissed sheet as
  // an error — the maud pages caught everything into one paragraph, so
  // cancelling left "Could not start." on the screen. `InvalidStateError` is
  // deliberately not here: it means this authenticator already holds a key,
  // which is something to say rather than something to swallow.
  assert.equal(declined(Object.assign(new Error("no"), { name: "NotAllowedError" })), true);
  assert.equal(declined(Object.assign(new Error("no"), { name: "AbortError" })), true);
  assert.equal(declined(Object.assign(new Error("no"), { name: "InvalidStateError" })), false);
  assert.equal(declined(new Error("the network went away")), false);
  // Anything that is not an Error at all: a rejected fetch, a string thrown
  // from somewhere. Not a decline, so it gets reported.
  assert.equal(declined("NotAllowedError"), false);
  assert.equal(declined(null), false);
});

test("it reads what the server writes", () => {
  // A challenge as the Rust side emits it: base64url, unpadded.
  assert.deepEqual(
    b64urlToBytes("AQIDBAU"),
    Uint8Array.from([1, 2, 3, 4, 5]),
    "an unpadded challenge is decodable without the client adding anything",
  );
});
