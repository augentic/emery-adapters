// Scenario validation, before any browser starts.
//
// Same rule as rea-agents' browserScenarioValues (strict objects, a fixed
// action vocabulary, unique step ids): a scenario that names an unknown
// field, action or locator kind, or repeats a step id, is refused whole
// rather than failing half-way through a run.

const TOP = ["version", "id", "screen", "serve", "environment", "network", "storage", "settle", "browser", "start_path", "actions"];
const NESTED = {
  serve: ["dir", "spa_fallback"],
  environment: ["viewport", "device_scale_factor", "locale", "timezone", "color_scheme", "reduced_motion", "service_workers", "fixed_time", "random_seed"],
  network: ["stubs", "block_prefixes"],
  storage: ["local_storage"],
  settle: ["quiet_ms", "max_ms"],
  browser: ["headless"],
};
const STUB = ["method", "origin", "path", "status", "body", "content_type"];
const ACTION = ["step_id", "action", "locator", "value", "timeout_ms"];
const LOCATOR = { test_id: ["value"], role: ["role", "name", "exact"], css: ["value"] };
// what each action needs besides its step id
const NEEDS = {
  goto: ["value"],
  click: ["locator"],
  fill: ["locator", "value"],
  press: ["locator", "value"],
  blur: ["locator"],
  select_option: ["locator", "value"],
  check: ["locator"],
  uncheck: ["locator"],
  wait_for: ["locator"],
};
const ID = /^[a-z0-9]+(-[a-z0-9]+)*$/;

function unknown(where, value, allowed, problems) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    problems.push(`${where}: expected an object`);
    return;
  }
  for (const k of Object.keys(value)) if (!allowed.includes(k)) problems.push(`${where}: unknown field \`${k}\``);
}

/** Every rule the scenario breaks, one line each; empty when it is valid. */
export function problems(s) {
  const out = [];
  unknown("scenario", s, TOP, out);
  if (out.length && (s === null || typeof s !== "object")) return out;
  if (s.version !== 1) out.push("scenario: `version` must be 1");
  for (const key of ["id", "screen"]) if (typeof s[key] !== "string" || !ID.test(s[key])) out.push(`scenario: \`${key}\` must be a kebab-case slug`);
  if (typeof s.start_path !== "string" || !s.start_path.startsWith("/")) out.push("scenario: `start_path` must start with /");
  if (!s.serve || typeof s.serve.dir !== "string") out.push("scenario: `serve.dir` is required");
  for (const [key, allowed] of Object.entries(NESTED)) if (s[key] !== undefined) unknown(`scenario.${key}`, s[key], allowed, out);
  for (const [i, stub] of (s.network?.stubs ?? []).entries()) {
    unknown(`stub ${i}`, stub, STUB, out);
    for (const key of ["method", "path", "status"]) if (stub[key] === undefined) out.push(`stub ${i}: \`${key}\` is required`);
  }
  // no actions is allowed: the start navigation alone (a guard's redirect) is a run
  if (!Array.isArray(s.actions)) {
    out.push("scenario: `actions` must be a list");
    return out;
  }
  const seen = new Set(["start"]);
  for (const [i, a] of s.actions.entries()) {
    const where = `action ${i}${a?.step_id ? ` (${a.step_id})` : ""}`;
    unknown(where, a, ACTION, out);
    if (typeof a?.step_id !== "string" || !ID.test(a.step_id)) out.push(`${where}: \`step_id\` must be a kebab-case slug`);
    else if (seen.has(a.step_id)) out.push(`${where}: step id \`${a.step_id}\` is used twice (\`start\` is reserved)`);
    else seen.add(a.step_id);
    const needs = NEEDS[a?.action];
    if (!needs) {
      out.push(`${where}: unknown action \`${a?.action}\``);
      continue;
    }
    for (const key of needs) if (a[key] === undefined) out.push(`${where}: \`${a.action}\` needs \`${key}\``);
    if (a.locator) {
      const fields = LOCATOR[a.locator.kind];
      if (!fields) out.push(`${where}: unknown locator kind \`${a.locator.kind}\``);
      else unknown(`${where} locator`, a.locator, ["kind", "nth", ...fields], out);
    }
  }
  return out;
}

/** The scenario, or an error listing every problem. */
export function validated(s) {
  const found = problems(s);
  if (found.length) throw new Error(`invalid scenario:\n${found.join("\n")}`);
  return s;
}
