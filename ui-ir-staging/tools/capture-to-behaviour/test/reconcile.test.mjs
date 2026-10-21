import assert from "node:assert/strict";
import { test } from "node:test";

import { apply, pathMatches } from "../lib/apply.mjs";
import { compare, compareBehaviour } from "../lib/compare.mjs";
import { problems } from "../lib/scenario.mjs";

const D = (n) => `sha256:${String(n).repeat(64)}`;
const ev = [{ path: "a.tsx#L1" }];

function login(stepAfter) {
  const behaviour = {
    version: 1,
    screen: "s",
    routes: [{ path: "/posts/$id", declared: true, evidence: ev }],
    triggers: [
      {
        id: "t-000000000001",
        on: { gesture: "submit", match: { role: "form", text: null, href: null, name: null, in_overlay: false }, nodes: [], match_status: "unresolved", submitters: [{ match: { role: "button", text: "Log in", href: null, name: null, in_overlay: false }, nodes: [], match_status: "unresolved", evidence: ev }] },
        does: [{ kind: "call", callee: "api:login", operation: { method: "POST", url: "/api/login" }, on_success: [{ kind: "navigate", to: "/home", evidence: ev }], evidence: ev }],
        evidence: ev,
      },
    ],
    guards: [],
    state: {},
    validation: [{ field: "email", rule: "required", message: "Required", nodes: [], evidence: ev }],
    transitions: [],
  };
  const recording = {
    recording: "r",
    screen: "s",
    start_path: "/login",
    inventory: [{ node: "s/n-1", path: "p1", role: "button", text: "Log in", href: null, name: null, in_overlay: false }],
    steps: [
      { index: 0, step_id: "start", status: "completed", url_before: null, url_after: "/login", requests: [], texts_appeared: [], overlays: [], replay_digest: D(0) },
      { index: 1, step_id: "submit", action: "click", target: { node: "s/n-1" }, status: "completed", url_before: "/login", overlays: [], replay_digest: D(1), ...stepAfter },
    ],
  };
  return { behaviour, recording };
}

test("nested on_success effects are confirmed by path, bodies kept as shape", () => {
  const { behaviour, recording } = login({
    url_after: "/home",
    requests: ["POST /api/login 200", "GET /api/me 200"],
    request_bodies: [{ request: "POST /api/login", content_type: "application/json", fields: ["email", "password"], body_sha256: D(9) }],
    texts_appeared: [],
  });
  const { doc, report } = apply(behaviour, [recording]);
  const [obs] = doc.triggers[0].observed;
  assert.deepEqual(obs.confirms, [0, "0.on_success.0"]);
  assert.equal(obs.contradicts, undefined);
  assert.deepEqual(obs.unexplained, ["GET /api/me 200"]);
  assert.deepEqual(obs.bodies, [{ request: "POST /api/login", content_type: "application/json", fields: ["email", "password"] }]);
  assert.equal(report.contradictions, 0);
  assert.equal(report.unexplained_requests, 1);
});

test("a certain navigate that did not happen is a contradiction", () => {
  const { behaviour, recording } = login({ url_after: "/elsewhere", requests: ["POST /api/login 200"], texts_appeared: [] });
  const [obs] = apply(behaviour, [recording]).doc.triggers[0].observed;
  assert.deepEqual(obs.contradicts, ["0.on_success.0"]);
});

test("no contradiction when the call failed, or a validation message shows", () => {
  const failed = login({ url_after: "/login", requests: ["POST /api/login 401"], texts_appeared: [] });
  assert.equal(apply(failed.behaviour, [failed.recording]).doc.triggers[0].observed[0].contradicts, undefined);
  const invalid = login({ url_after: "/login", requests: [], texts_appeared: ["Required"] });
  assert.equal(apply(invalid.behaviour, [invalid.recording]).doc.triggers[0].observed[0].contradicts, undefined);
});

test("route patterns", () => {
  assert.ok(pathMatches("/posts/$id", "/posts/3"));
  assert.ok(pathMatches("/posts/:id", "/posts/3"));
  assert.ok(pathMatches("/files/*", "/files/a/b"));
  assert.ok(!pathMatches("/posts/$id", "/posts"));
  assert.ok(!pathMatches("/a", "/b"));
});

const valid = () => ({
  version: 1,
  id: "x-login",
  screen: "login",
  serve: { dir: "dist", spa_fallback: true },
  start_path: "/login",
  actions: [{ step_id: "fill-email", action: "fill", locator: { kind: "css", value: "input" }, value: "a@b.c" }],
});

test("scenario validation is strict", () => {
  assert.deepEqual(problems(valid()), []);
  const bad = valid();
  bad.extra = 1;
  bad.actions.push({ step_id: "fill-email", action: "hover", locator: { kind: "xpath", value: "//a" } });
  bad.actions.push({ step_id: "start", action: "click", locator: { kind: "role", role: "button", label: "x" } });
  assert.deepEqual(problems(bad), [
    "scenario: unknown field `extra`",
    "action 1 (fill-email): step id `fill-email` is used twice (`start` is reserved)",
    "action 1 (fill-email): unknown action `hover`",
    "action 2 (start): step id `start` is used twice (`start` is reserved)",
    "action 2 (start) locator: unknown field `label`",
  ]);
});

test("compare aligns by step id; missing evidence is unknown", () => {
  const rec = (steps) => ({ recording: "r", scenario_digest: D(1), steps });
  const a = rec([{ step_id: "start", status: "completed", replay_digest: D(1) }, { step_id: "go", status: "completed", replay_digest: D(2), url_after: "/a" }]);
  const b = rec([{ step_id: "go", status: "completed", replay_digest: D(3), url_after: "/b" }, { step_id: "start", status: "completed", replay_digest: D(1) }]);
  const c = rec([{ step_id: "start", status: "completed", replay_digest: D(1) }, { step_id: "go", status: "cancelled" }]);
  assert.equal(compare(a, a).result, "unchanged");
  const ab = compare(a, b);
  assert.equal(ab.result, "changed");
  assert.deepEqual(ab.steps.find((s) => s.step_id === "go").fields, ["url_after"]);
  assert.equal(compare(a, c).result, "unknown");
});

test("behaviour compare ignores the DOM, keeps API requests, scopes texts to the screen", () => {
  const rec = (steps) => ({ recording: "r", start_path: "/login", steps });
  const step = (id, o) => ({ step_id: id, status: "completed", replay_digest: D(Math.random()), dom_sha256: D(Math.random()), page_errors: [], ...o });
  const a = rec([
    step("start", { url_after: "/login", requests: ["GET /login 200"], texts_appeared: ["Log In", "Email"] }),
    step("submit", { url_after: "/", requests: ["POST http://api/x 200", "GET http://api/me 200"], request_bodies: [{ request: "POST http://api/x", content_type: "f", fields: ["a"], body_sha256: D(1) }], texts_appeared: ["Dashboard"] }),
  ]);
  const b = rec([
    step("start", { url_after: "/login", requests: ["GET /login 200", "GET /assets/a.js 200"], texts_appeared: ["Email", "Log In"] }),
    step("submit", { url_after: "/", requests: ["POST http://api/x 200"], request_bodies: [{ request: "POST http://api/x", content_type: "f", fields: ["a"], body_sha256: D(2) }], texts_appeared: ["Home"] }),
  ]);
  const ab = compareBehaviour(a, b);
  assert.equal(ab.result, "same");
  assert.deepEqual(ab.steps.find((s) => s.step_id === "submit").off_screen, ["texts_appeared", "GET requests"]);
  const c = rec([step("start", { url_after: "/login", requests: [], texts_appeared: ["Log In"] }), step("submit", { url_after: "/login", requests: [], texts_appeared: [] })]);
  const ac = compareBehaviour(a, c);
  assert.equal(ac.result, "differs");
  assert.deepEqual(ac.steps.find((s) => s.step_id === "start").differs.map((d) => d.field), ["texts_appeared"]);
  // back on start_path after an off-screen step: the texts delta has an off-screen baseline
  const back = (t) => rec([step("go", { url_after: "/signup", requests: [], texts_appeared: [] }), step("back", { url_after: "/login", requests: [], texts_appeared: t })]);
  const bb = compareBehaviour(back(["Log In"]), back(["Log In", "Email"]));
  assert.equal(bb.result, "same");
  assert.deepEqual(bb.steps.find((s) => s.step_id === "back").off_screen_baseline, ["texts_appeared"]);
});

test("text_in narrows a null text to the code's literal alternatives", () => {
  const ev1 = [{ path: "a.tsx#L1" }];
  const behaviour = {
    version: 1, screen: "s", routes: [], guards: [], state: {}, validation: [], transitions: [],
    triggers: [
      { id: "t-000000000001", on: { gesture: "tap", match: { role: "button", text: null, text_in: ["Hide", "Show"], href: null, name: null, in_overlay: false }, nodes: [], match_status: "unresolved" }, does: [], evidence: ev1 },
      { id: "t-000000000002", on: { gesture: "tap", match: { role: "button", text: "Sign in", href: null, name: null, in_overlay: false }, nodes: [], match_status: "unresolved" }, does: [], evidence: [{ path: "b.tsx#L1" }] },
    ],
  };
  const el = (node, text) => ({ node, path: node, role: "button", text, href: null, name: null, in_overlay: false });
  const recording = { recording: "r", screen: "s", start_path: "/", inventory: [el("s/n-1", "Show"), el("s/n-2", "Sign in")], steps: [] };
  const [toggle, submit] = apply(behaviour, [recording]).doc.triggers;
  assert.deepEqual([toggle.on.match_status, toggle.on.nodes], ["unique", ["s/n-1"]]);
  assert.deepEqual([submit.on.match_status, submit.on.nodes], ["unique", ["s/n-2"]]);
});

test("a click on a submit button is a submit of its form, even with no submitter in the code", () => {
  const ev1 = [{ path: "form.tsx#L1" }];
  const behaviour = {
    version: 1, screen: "s", routes: [], guards: [], state: {}, transitions: [],
    validation: [],
    triggers: [{ id: "t-000000000001", on: { gesture: "submit", match: { role: "form", text: null, href: null, name: null, in_overlay: false }, nodes: [], match_status: "unresolved", submitters: [] }, does: [{ kind: "validate", fields: ["email"], evidence: ev1 }], evidence: ev1 }],
  };
  const el = (node, role, extra) => ({ node, path: node, role, text: null, href: null, name: null, in_overlay: false, ...extra });
  const recording = {
    recording: "r", screen: "s", start_path: "/login",
    inventory: [el("s/n-f", "form"), el("s/n-b", "button", { text: "Log in", submitter: true, form: "s/n-f" })],
    steps: [
      { index: 0, step_id: "start", status: "completed", url_before: null, url_after: "/login", requests: [], texts_appeared: [], overlays: [], replay_digest: D(0) },
      { index: 1, step_id: "submit", action: "click", target: { node: "s/n-b", submitter: true, form: "s/n-f" }, status: "completed", url_before: "/login", url_after: "/login", requests: [], texts_appeared: ["Required"], overlays: [], replay_digest: D(1) },
    ],
  };
  const [form] = apply(behaviour, [recording]).doc.triggers;
  assert.deepEqual(form.on.nodes, ["s/n-f"]);
  assert.equal(form.observed.length, 1);
  assert.equal(form.observed[0].step, "submit");
});
