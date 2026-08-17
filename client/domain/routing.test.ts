import { test } from "node:test";
import assert from "node:assert/strict";
import {
  build,
  isUuid,
  match,
  normalise,
  pattern,
  resolve,
  stripBase,
  withBase,
} from "./routing.ts";

test("a literal path matches itself and nothing else", () => {
  const p = pattern("/pack");
  assert.deepEqual(match(p, "/pack"), {});
  assert.equal(match(p, "/packing"), null);
  assert.equal(match(p, "/pack/extra"), null);
  assert.equal(match(p, "/"), null);
});

test("a param takes exactly one segment", () => {
  const p = pattern("/pack/:fulfilment");
  assert.deepEqual(match(p, "/pack/f01f0000"), { fulfilment: "f01f0000" });
  // Not two. A subject with a slash in it is not a subject this route names.
  assert.equal(match(p, "/pack/one/two"), null);
  assert.equal(match(p, "/pack"), null);
});

test("params are percent-decoded, and a bad escape is a 404 rather than a throw", () => {
  const p = pattern("/find/:scan");
  // The FNC1 separator a GS1 scan carries, which must survive the round trip.
  assert.deepEqual(match(p, "/find/01093123456789%1D10ABC"), { scan: "01093123456789\x1d10ABC" });
  assert.deepEqual(match(p, "/find/%E2%9C%93"), { scan: "✓" });
  assert.deepEqual(match(p, "/find/%zz"), { scan: "%zz" });
});

test("building a path is the inverse of matching it", () => {
  const p = pattern("/findings/:finding");
  const path = build(p, { finding: "d15c0000-0000-0000-0000-000000000001" });
  assert.equal(path, "/findings/d15c0000-0000-0000-0000-000000000001");
  assert.deepEqual(match(p, path), { finding: "d15c0000-0000-0000-0000-000000000001" });
});

test("a component cannot invent a segment", () => {
  // The reason `build` exists rather than a template literal at the call site.
  const p = pattern("/find/:scan");
  assert.equal(build(p, { scan: "a/b" }), "/find/a%2Fb");
  assert.deepEqual(match(p, build(p, { scan: "a/b" })), { scan: "a/b" });
});

test("building without a param it needs is an error, not a path with a hole in it", () => {
  assert.throws(() => build(pattern("/pack/:fulfilment"), {}), /needs a fulfilment/);
});

test("normalising collapses slashes and drops a trailing one, except at the root", () => {
  assert.equal(normalise("/pack/"), "/pack");
  assert.equal(normalise("//pack//bench//"), "/pack/bench");
  assert.equal(normalise("/"), "/");
  assert.equal(normalise(""), "/");
  // Case is left alone: two identifiers differing only in case are two subjects.
  assert.equal(normalise("/pack/F01F"), "/pack/F01F");
});

test("the base comes off, and the router is correct under both mounts", () => {
  // The property that lets the mount move in one constant rather than a rewrite.
  assert.equal(stripBase("/ui/", "/ui/pack"), "/pack");
  assert.equal(stripBase("/ui/", "/ui/"), "/");
  assert.equal(stripBase("/ui/", "/ui"), "/");
  assert.equal(stripBase("/", "/pack"), "/pack");
  assert.equal(stripBase("/", "/"), "/");
  // A path outside the mount is left alone rather than mangled into one.
  assert.equal(stripBase("/ui/", "/app/packing"), "/app/packing");
  // And a name that merely starts the same way is not inside it.
  assert.equal(stripBase("/ui/", "/uixyz"), "/uixyz");
});

test("the base goes back on for an href", () => {
  for (const base of ["/ui/", "/"]) {
    assert.equal(stripBase(base, withBase(base, "/pack")), "/pack");
    assert.equal(stripBase(base, withBase(base, "/")), "/");
  }
  assert.equal(withBase("/ui/", "/pack"), "/ui/pack");
  assert.equal(withBase("/", "/pack"), "/pack");
});

test("resolve takes the first match, so a literal above a pattern wins", () => {
  const routes = [
    { id: "clear", pattern: pattern("/findings/clear") },
    { id: "one", pattern: pattern("/findings/:finding") },
  ];
  assert.equal(resolve(routes, "/findings/clear")?.route.id, "clear");
  assert.equal(resolve(routes, "/findings/abc")?.route.id, "one");
  assert.deepEqual(resolve(routes, "/findings/abc")?.params, { finding: "abc" });
  // The collision the ordering rule exists for, stated the wrong way round.
  const wrong = [routes[1]!, routes[0]!];
  assert.equal(resolve(wrong, "/findings/clear")?.route.id, "one");
});

test("nothing matching is null, and null is a 404 rather than a fixture", () => {
  const routes = [{ id: "pack", pattern: pattern("/pack") }];
  assert.equal(resolve(routes, "/nonsense"), null);
  assert.equal(resolve(routes, "/"), null);
});

test("a uuid is recognised, so a screen can refuse a subject that cannot exist", () => {
  assert.ok(isUuid("f01f0000-0000-0000-0000-000000000004"));
  assert.ok(isUuid("F01F0000-0000-0000-0000-000000000004"));
  assert.ok(!isUuid("not-a-uuid"));
  assert.ok(!isUuid(""));
});
