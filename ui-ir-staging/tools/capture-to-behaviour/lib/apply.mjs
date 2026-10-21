// Attach recordings to a behaviour.json written by code-to-behaviour.
//
// 1. Match: each code element's tuple (role, text, href, name, in_overlay)
//    against the elements the runs saw. A null in the tuple matches any
//    value; a null text with `text_in` matches one of those texts. A DOM element goes to the code elements that fit it; when those
//    come from more than one place in the code, none of them claims it and
//    each is "ambiguous" (never a guess: a later semantic layer may propose
//    a resolution as an inferred claim).
// 2. Reconcile: each step that acted on a matched node adds an observation
//    to the trigger. As in rea-agents' static/runtime reconciliation
//    (docs/javascript-runtime-reconciliation.md), the run is sorted into
//    - `confirms`: effects of `does` the run showed happening; an effect
//      inside a call's on_success / on_failure is named by its path,
//      e.g. "2.on_success.0";
//    - `contradicts`: effects the code says must happen that the run shows
//      did not (only where the code leaves no other way: an unconditional
//      navigate, or a branch whose call outcome was seen, and no
//      validation message on screen);
//    - `unexplained`: requests of the step no `call` in `does` accounts for;
//    - everything else stays unknown: not seen is not absent.
//    Guards, routes and validation rules get observations the same way.
//    An action's observation includes the `wait_for` steps right after it.
// Pure function of its inputs: same files in, same bytes out.

import { canonical } from "./canonical.mjs";

const FIELD_ROLES = new Set(["textbox", "checkbox", "radio", "combobox"]);
const OVERLAY_ROLE = { dialog: "dialog", sheet: "dialog", menu: "menu", popover: "dialog", tooltip: "tooltip" };
const REQUEST = /^[A-Z]+ \S+ ([0-9]{3}|failed|blocked)$/;

const cmp = (a, b) => (a < b ? -1 : a > b ? 1 : 0);

function parseUrl(url) {
  if (!url) return null;
  try {
    return new URL(url, "http://local");
  } catch {
    return null;
  }
}
const pathOf = (url) => parseUrl(url)?.pathname ?? null;
const hashOf = (url) => parseUrl(url)?.hash ?? "";

// A route pattern from the code (`/posts/:id`, `/posts/$id`, `/files/*`)
// against a concrete path.
export function pathMatches(pattern, path) {
  if (pattern === null || path === null) return false;
  const want = pattern.split("/");
  const got = path.split("/");
  for (let i = 0; i < want.length; i += 1) {
    if (want[i] === "*" || want[i] === "$") return true;
    if (i >= got.length) return false;
    if (/^[:$]/.test(want[i]) && got[i] !== "") continue;
    if (want[i] !== got[i]) return false;
  }
  return want.length === got.length;
}

function fits(tuple, el) {
  return (
    el.role === tuple.role &&
    el.in_overlay === tuple.in_overlay &&
    (tuple.text === null ? !tuple.text_in || tuple.text_in.includes(el.text) : el.text === tuple.text) &&
    (tuple.href === null || el.href === tuple.href) &&
    (tuple.name === null || el.name === tuple.name)
  );
}

function status(nodes, ambiguous) {
  if (ambiguous) return "ambiguous";
  if (nodes.length === 0) return "none";
  return nodes.length === 1 ? "unique" : "multiple";
}

// Every effect of `does` with its address: top-level effects by index,
// nested ones by path, and whether the code makes it unconditional given
// the outcome of the call it hangs off (`outcome`: "success" | "failure").
function flatten(does, prefix = null, outcome = null) {
  return does.flatMap((e, i) => {
    const at = prefix === null ? i : `${prefix}.${i}`;
    const self = { effect: e, at, outcome, conditional: !!e.when };
    if (e.kind !== "call") return [self];
    const parent = { call: e, at };
    return [
      self,
      ...flatten(e.on_success ?? [], String(at) + ".on_success", "success").map((x) => ({ ...x, parent: x.parent ?? parent })),
      ...flatten(e.on_failure ?? [], String(at) + ".on_failure", "failure").map((x) => ({ ...x, parent: x.parent ?? parent })),
    ];
  });
}

const callMatches = (op, q) => {
  const [method, url] = q.split(" ");
  return method === op.method && pathMatches(op.url, pathOf(url));
};
const outcomeOf = (q) => {
  const code = q.split(" ").at(-1);
  return /^2/.test(code) ? "success" : "failure";
};

// A `wait_for` is the scenario's explicit readiness for the action before
// it (rea does not infer readiness either), so an action's observation runs
// to the end of the waits that follow it.
function withWaits(step, rest) {
  if (step.action === "wait_for") return step;
  const waits = [];
  for (const w of rest) {
    if (w.action !== "wait_for" || w.status !== "completed") break;
    waits.push(w);
  }
  if (!waits.length) return step;
  const last = waits.at(-1);
  const all = [step, ...waits];
  return {
    ...step,
    url_after: last.url_after,
    overlays: last.overlays,
    requests: all.flatMap((x) => x.requests),
    request_bodies: all.flatMap((x) => x.request_bodies ?? []),
    page_errors: [...new Set(all.flatMap((x) => x.page_errors ?? []))].sort(),
    texts_appeared: [...new Set(all.flatMap((x) => x.texts_appeared))].sort(),
  };
}

export function apply(input, recordings) {
  const doc = JSON.parse(JSON.stringify(input));
  const recs = recordings.filter((r) => r.screen === doc.screen).sort((a, b) => cmp(a.recording, b.recording));
  const pool = new Map();
  for (const r of recs) for (const el of r.inventory) if (!pool.has(el.node)) pool.set(el.node, el);
  const elements = [...pool.values()].sort((a, b) => cmp(a.node, b.node));

  // 1. match
  const sites = [];
  for (const t of doc.triggers) {
    sites.push({ target: t.on, key: `${t.evidence[0].path} ${canonical(t.on.match)}`, nodes: new Set(), ambiguous: false });
    for (const s of t.on.submitters ?? []) {
      sites.push({ target: s, key: `${(s.evidence ?? t.evidence)[0].path} ${canonical(s.match)}`, nodes: new Set(), ambiguous: false });
    }
  }
  for (const el of elements) {
    const fitting = sites.filter((s) => fits(s.target.match, el));
    if (!fitting.length) continue;
    const keys = new Set(fitting.map((s) => s.key));
    for (const s of fitting) {
      if (keys.size > 1) s.ambiguous = true;
      else s.nodes.add(el.node);
    }
  }
  for (const s of sites) {
    const nodes = s.ambiguous ? [] : [...s.nodes].sort();
    s.target.nodes = nodes;
    s.target.match_status = status(nodes, s.ambiguous);
  }
  for (const rule of doc.validation) {
    rule.nodes = elements.filter((el) => FIELD_ROLES.has(el.role) && el.name === rule.field).map((el) => el.node).sort();
  }

  // 2. reconcile
  const push = (entry, obs) => {
    entry.observed = [...(entry.observed ?? []), obs];
  };
  const messages = (fields) => doc.validation.filter((r) => fields.includes(r.field) && r.message).map((r) => r.message);
  const allMessages = doc.validation.filter((r) => r.message).map((r) => r.message);
  let unexplainedTotal = 0;
  for (const r of recs) {
    const done = new Set();
    let prev = null;
    for (const [i, step] of r.steps.entries()) {
      if (step.status !== "completed") break;
      const s = withWaits(step, r.steps.slice(i + 1));
      const base = { recording: r.recording, step: s.step_id, replay_digest: s.replay_digest };
      const requests = s.requests.filter((q) => REQUEST.test(q));
      const opened = (role) => s.overlays.includes(role) && !(prev?.overlays ?? []).includes(role);
      const closed = (role) => !s.overlays.includes(role) && (prev?.overlays ?? []).includes(role);
      const happened = (e) => {
        switch (e.kind) {
          case "navigate":
            return s.url_after !== s.url_before && pathMatches(e.to, pathOf(s.url_after));
          case "scroll_to":
            return hashOf(s.url_after) === e.anchor;
          case "open":
          case "toggle":
            return opened(OVERLAY_ROLE[e.overlay]);
          case "close":
            return closed(OVERLAY_ROLE[e.overlay]);
          case "set": {
            const controls = doc.state[e.state]?.controls;
            if (!controls) return false;
            const role = OVERLAY_ROLE[controls.overlay];
            return e.value === true ? s.overlays.includes(role) : e.value === false ? closed(role) : false;
          }
          case "call":
            return !!e.operation && requests.some((q) => callMatches(e.operation, q));
          case "show":
            return e.title !== null && s.texts_appeared.includes(e.title);
          case "validate":
            return messages(e.fields ?? []).some((m) => s.texts_appeared.includes(m));
          default:
            return false;
        }
      };
      const reconcile = (t) => {
        const flat = flatten(t.does);
        const confirms = flat.filter((x) => happened(x.effect)).map((x) => x.at);
        // a navigate the code makes certain, judged only when nothing on screen says the handler stopped early
        const stoppedEarly = (t.guard ?? []).length > 0 || allMessages.some((m) => s.texts_appeared.includes(m));
        const landed = pathOf(s.url_after);
        const anyNavigateFits = flat.some((x) => x.effect.kind === "navigate" && pathMatches(x.effect.to, landed));
        const certain = (x) => {
          if (x.conditional) return false;
          if (!x.parent) return x.outcome === null;
          if (x.parent.call.when || !x.parent.call.operation) return false;
          const seen = requests.filter((q) => callMatches(x.parent.call.operation, q));
          return seen.length > 0 && seen.every((q) => outcomeOf(q) === x.outcome);
        };
        const contradicts = stoppedEarly || anyNavigateFits
          ? []
          : flat.filter((x) => x.effect.kind === "navigate" && certain(x)).map((x) => x.at);
        const calls = flat.filter((x) => x.effect.kind === "call" && x.effect.operation).map((x) => x.effect.operation);
        // a document load of the page landed on is the navigation itself, not an unexplained request
        const unexplained = requests.filter((q) => !calls.some((op) => callMatches(op, q)) && !(q.startsWith("GET ") && pathOf(q.split(" ")[1]) === landed));
        unexplainedTotal += unexplained.length;
        const bodies = (s.request_bodies ?? []).filter((b) => calls.some((op) => callMatches(op, b.request))).map(({ request, content_type, fields }) => ({ request, content_type, fields }));
        return {
          ...base,
          confirms,
          ...(contradicts.length ? { contradicts } : {}),
          requests,
          ...(unexplained.length ? { unexplained } : {}),
          ...(bodies.length ? { bodies } : {}),
          ...(s.page_errors?.length ? { page_errors: s.page_errors } : {}),
          url_before: s.url_before,
          url_after: s.url_after,
        };
      };
      const node = s.target?.node;
      if (node && s.action === "click") {
        for (const t of doc.triggers) {
          const onNode = t.on.gesture === "tap" && t.on.nodes.includes(node);
          const viaSubmitter = t.on.gesture === "submit" && (t.on.submitters ?? []).some((x) => x.nodes.includes(node));
          // a click on a submit button submits its form owner (HTML), even when the
          // code renders the button somewhere the reader could not tie to the form
          const viaForm = t.on.gesture === "submit" && s.target.submitter === true && !!s.target.form && t.on.nodes.includes(s.target.form);
          if (onNode || viaSubmitter || viaForm) push(t, reconcile(t));
        }
      }
      if (s.index === 0) {
        for (const g of doc.guards) {
          if (pathMatches(g.route, pathOf(r.start_path)) && pathMatches(g.go, pathOf(s.url_after))) {
            push(g, { ...base, confirms: ["self"], requests, url_before: r.start_path, url_after: s.url_after });
          }
        }
      }
      const here = pathOf(s.url_after);
      const route = doc.routes.find((x) => x.path === here) ?? doc.routes.find((x) => pathMatches(x.path, here));
      if (route && !done.has(route.path)) {
        done.add(route.path);
        push(route, { ...base, url_after: s.url_after });
      }
      for (const rule of doc.validation) {
        if (rule.message && s.texts_appeared.includes(rule.message)) push(rule, { ...base, confirms: ["self"] });
      }
      prev = s;
    }
  }
  const counts = {};
  for (const s of sites) counts[s.target.match_status] = (counts[s.target.match_status] ?? 0) + 1;
  const observations = doc.triggers.flatMap((t) => t.observed ?? []);
  const report = {
    recordings: recs.map((r) => r.recording),
    elements: elements.length,
    match: counts,
    triggers_observed: doc.triggers.filter((t) => t.observed).length,
    triggers_confirmed: doc.triggers.filter((t) => (t.observed ?? []).some((o) => o.confirms.length)).length,
    contradictions: observations.reduce((n, o) => n + (o.contradicts?.length ?? 0), 0),
    unexplained_requests: unexplainedTotal,
    page_errors: [...new Set(recs.flatMap((r) => r.steps.flatMap((s) => s.page_errors ?? [])))].length,
    guards_observed: doc.guards.filter((g) => g.observed).length,
    rules_observed: doc.validation.filter((v) => v.observed).length,
    routes_observed: doc.routes.filter((v) => v.observed).length,
  };
  return { doc, report };
}
