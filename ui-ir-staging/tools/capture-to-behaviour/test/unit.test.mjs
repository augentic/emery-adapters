import assert from "node:assert/strict";
import { test } from "node:test";

import { apply } from "../lib/apply.mjs";
import { canonical, digest } from "../lib/canonical.mjs";
import { nodeId } from "../lib/node-id.mjs";

test("node id vectors (ui-ir-staging/docs/node-id.md)", () => {
  assert.equal(nodeId("login", "html"), "login/n-e5e239560bc125a2");
  assert.equal(
    nodeId("login", "html > body > div > div > div:nth-of-type(2) > div:nth-of-type(1) > div > button"),
    "login/n-cf2331bab5423b71",
  );
  assert.equal(nodeId("index", "html > body"), "index/n-b364c0b0dce72fd0");
});

test("canonical json sorts keys and drops undefined", () => {
  assert.equal(canonical({ b: 1, a: [true, null, "x"], c: undefined }), '{"a":[true,null,"x"],"b":1}');
  assert.equal(digest({ a: 1, b: 2 }), digest({ b: 2, a: 1 }));
});

const tuple = (role, text, href = null, in_overlay = false) => ({ role, text, href, name: null, in_overlay });
const el = (node, role, text, href = null, extra = {}) => ({ node, path: node, role, text, href, name: null, in_overlay: false, ...extra });

function fixture() {
  const behaviour = {
    version: 1,
    screen: "s",
    routes: [{ path: "/next", declared: false, evidence: [{ path: "a.tsx#L1" }] }],
    triggers: [
      { id: "t-000000000001", on: { gesture: "tap", match: tuple("link", "Next", "/next"), nodes: [], match_status: "unresolved" }, does: [{ kind: "navigate", to: "/next", evidence: [{ path: "a.tsx#L1" }] }], evidence: [{ path: "a.tsx#L1" }] },
      { id: "t-000000000002", on: { gesture: "tap", match: tuple("button", null), nodes: [], match_status: "unresolved" }, does: [], evidence: [{ path: "a.tsx#L2" }] },
      { id: "t-000000000003", on: { gesture: "tap", match: tuple("button", "Go"), nodes: [], match_status: "unresolved" }, does: [], evidence: [{ path: "a.tsx#L3" }] },
    ],
    guards: [],
    state: {},
    validation: [{ field: "email", rule: "email", message: "Bad email", nodes: [], evidence: [{ path: "a.tsx#L4" }] }],
    transitions: [],
  };
  const recording = {
    recording: "r",
    screen: "s",
    start_path: "/",
    inventory: [el("s/n-1", "link", "Next", "/next"), el("s/n-2", "button", "Go"), el("s/n-3", "textbox", null, null, { name: "email" })],
    steps: [
      { index: 0, step_id: "start", status: "completed", url_before: null, url_after: "/", requests: ["GET / 200"], texts_appeared: [], overlays: [], replay_digest: `sha256:${"0".repeat(64)}` },
      { index: 1, step_id: "go", action: "click", target: { node: "s/n-1" }, status: "completed", url_before: "/", url_after: "/next", requests: [], texts_appeared: ["Bad email"], overlays: [], replay_digest: `sha256:${"1".repeat(64)}` },
    ],
  };
  return { behaviour, recording };
}

test("apply matches, flags ambiguity, observes", () => {
  const { behaviour, recording } = fixture();
  const { doc } = apply(behaviour, [recording]);
  const [link, wild, go] = doc.triggers;
  assert.deepEqual(link.on.nodes, ["s/n-1"]);
  assert.equal(link.on.match_status, "unique");
  assert.equal(wild.on.match_status, "ambiguous");
  assert.equal(go.on.match_status, "ambiguous");
  assert.deepEqual(link.observed, [{ recording: "r", step: "go", replay_digest: `sha256:${"1".repeat(64)}`, confirms: [0], requests: [], url_before: "/", url_after: "/next" }]);
  assert.deepEqual(doc.validation[0].nodes, ["s/n-3"]);
  assert.equal(doc.validation[0].observed[0].step, "go");
  assert.equal(doc.routes[0].observed[0].url_after, "/next");
  assert.equal(behaviour.triggers[0].on.match_status, "unresolved", "input not mutated");
});

test("apply is a pure function", () => {
  const { behaviour, recording } = fixture();
  assert.equal(canonical(apply(behaviour, [recording]).doc), canonical(apply(behaviour, [recording]).doc));
});
