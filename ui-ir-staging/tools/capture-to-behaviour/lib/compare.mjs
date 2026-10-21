// Compare two recordings of one scenario, step by step.
//
// After rea-agents' scenario comparison (docs/browser-scenario-contract.md
// "Scenario comparison", browserScenarioDiffValues.ts): steps are aligned by
// step_id, never by position; a step is `unchanged` when its replay digest
// is equal, `changed` when both runs completed it and the digests differ
// (with the fields that differ), and `unknown` when either run lacks it or
// did not complete it. Missing evidence is never read as equality.

const FIELDS = ["status", "url_after", "dom_sha256", "aria_sha256", "overlays", "texts_appeared", "requests", "request_bodies", "page_errors", "inventory", "target"];

const same = (a, b) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);

export function compare(left, right) {
  const byId = (rec) => new Map(rec.steps.map((s) => [s.step_id, s]));
  const l = byId(left);
  const r = byId(right);
  const ids = [...new Set([...left.steps.map((s) => s.step_id), ...right.steps.map((s) => s.step_id)])];
  const steps = ids.map((id) => {
    const a = l.get(id);
    const b = r.get(id);
    if (!a || !b) return { step_id: id, result: "unknown", reason: `missing in ${a ? "right" : "left"}` };
    if (a.status !== "completed" || b.status !== "completed") return { step_id: id, result: "unknown", reason: `status ${a.status} / ${b.status}` };
    if (a.replay_digest === b.replay_digest) return { step_id: id, result: "unchanged" };
    return { step_id: id, result: "changed", fields: FIELDS.filter((f) => !same(a[f], b[f])) };
  });
  const results = new Set(steps.map((s) => s.result));
  return {
    left: left.recording,
    right: right.recording,
    same_scenario: left.scenario_digest === right.scenario_digest,
    result: results.has("changed") ? "changed" : results.has("unknown") ? "unknown" : "unchanged",
    steps,
  };
}

// Behaviour mode: what a user and the backend observe, for two builds of one
// screen that need not share a DOM (one in React, one in Vue). Per step:
// `url_after`, the API requests (a request with an origin; the page's own
// document and asset loads are not compared), the request bodies' content
// type and field names (values are never recorded), the visible texts that
// appeared, and page errors. Once the left run has left the screen's start
// path, the next screen is out of scope: its texts and its GET requests are
// listed under `off_screen` and not compared. A step is `same`, `differs`
// (with each field's left and right), or `unknown` as above.

const api = (requests) => (requests ?? []).filter((r) => /^[A-Z]+ https?:\/\//.test(r)).sort();
const bodies = (list) => (list ?? []).map((b) => ({ request: b.request, content_type: b.content_type, fields: b.fields })).sort((x, y) => (x.request < y.request ? -1 : 1));

export function compareBehaviour(left, right) {
  const start = left.start_path;
  const r = new Map(right.steps.map((s) => [s.step_id, s]));
  const ids = [...new Set([...left.steps.map((s) => s.step_id), ...right.steps.map((s) => s.step_id)])];
  const l = new Map(left.steps.map((s) => [s.step_id, s]));
  // texts_appeared is a delta against the previous step; when that step was off start_path the
  // baseline page is one the port need not reproduce, so the delta is not comparable either.
  const prevOff = new Map(left.steps.map((s, i) => [s.step_id, i > 0 && left.steps[i - 1].url_after !== start]));
  const steps = ids.map((id) => {
    const a = l.get(id);
    const b = r.get(id);
    if (!a || !b) return { step_id: id, result: "unknown", reason: `missing in ${a ? "right" : "left"}` };
    if (a.status !== "completed" || b.status !== "completed") return { step_id: id, result: "unknown", reason: `status ${a.status} / ${b.status}`, ...(b.error ? { right_error: b.error } : {}) };
    const off = a.url_after !== start;
    const baselineOff = !off && prevOff.get(id) === true;
    const view = (s) => ({
      url_after: s.url_after,
      requests: off ? api(s.requests).filter((q) => !q.startsWith("GET ")) : api(s.requests),
      request_bodies: bodies(s.request_bodies),
      texts_appeared: off || baselineOff ? null : [...(s.texts_appeared ?? [])].sort(),
      page_errors: s.page_errors ?? [],
    });
    const va = view(a);
    const vb = view(b);
    const differs = Object.keys(va).filter((k) => !same(va[k], vb[k])).map((k) => ({ field: k, left: va[k], right: vb[k] }));
    const out = { step_id: id, result: differs.length ? "differs" : "same" };
    if (differs.length) out.differs = differs;
    if (off) out.off_screen = ["texts_appeared", "GET requests"];
    if (baselineOff) out.off_screen_baseline = ["texts_appeared"];
    return out;
  });
  const results = new Set(steps.map((s) => s.result));
  return {
    mode: "behaviour",
    left: left.recording,
    right: right.recording,
    start_path: start,
    result: results.has("differs") ? "differs" : results.has("unknown") ? "unknown" : "same",
    counts: { same: steps.filter((s) => s.result === "same").length, differs: steps.filter((s) => s.result === "differs").length, unknown: steps.filter((s) => s.result === "unknown").length },
    steps,
  };
}
