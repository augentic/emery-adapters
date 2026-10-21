// Record one scripted run of a built app.
//
// The recording model follows rea-agents 6.7.0 (MIT, Copyright (c) 2026
// morluto; https://github.com/morluto/rea), re-implemented here:
// - a declarative scenario, validated whole before the run: start path,
//   then actions with css / role / test_id locators, no caller code
//   (rea browserScenarioValues.ts, PlaywrightScenarioActions.ts);
// - a fresh browser context per run with a pinned environment: viewport,
//   device scale, locale, timezone, colour scheme, reduced motion,
//   service workers blocked, downloads off (rea PlaywrightScenarioBrowser.ts);
// - step 0 is the start navigation; every step records url before and
//   after, status, and digests of the DOM and the accessibility snapshot
//   (rea PlaywrightScenarioArtifacts.ts);
// - network transactions, page errors and console errors are attributed to
//   the step they started in (rea PlaywrightScenarioEvents.ts); a request
//   body is kept as its shape (media type, field names, digest), never its
//   values (rea keeps redacted bytes);
// - after a failed step the rest are cancelled, not run
//   (rea PlaywrightBrowserScenarioProvider.ts);
// - the run is made comparable by dropping what varies between equal runs:
//   elapsed times, event order within a step, the local server origin
//   (rea browserScenarioNormalization.ts); each step's replay digest is
//   sha256 over canonical JSON (rea canonicalDigest.ts).
// Additions of ours: a local static server, network stubs with every other
// origin blocked, a fixed clock, a seeded Math.random, and an element
// inventory per step so
// observations name ui-ir node ids.

import { readFile, readdir } from "node:fs/promises";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { chromium } from "playwright-core";

import { digest, sha256 } from "./canonical.mjs";
import { install } from "./inventory.mjs";
import { nodeId } from "./node-id.mjs";
import { validated } from "./scenario.mjs";
import { serve } from "./server.mjs";

const RECORDED_TYPES = new Set(["document", "xhr", "fetch"]);
const DEFAULT_ENV = {
  viewport: { width: 1280, height: 800 },
  device_scale_factor: 1,
  locale: "en-US",
  timezone: "UTC",
  color_scheme: "light",
  reduced_motion: "reduce",
  service_workers: "block",
  fixed_time: "2026-01-01T00:00:00.000Z",
  random_seed: 1,
};

// Math.random as mulberry32 from a fixed seed: apps that draw demo data at
// random render the same DOM on every run (crypto.getRandomValues is left alone)
function seedRandom(seed) {
  let a = seed >>> 0;
  Math.random = () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export async function findChromium(explicit) {
  if (explicit) return explicit;
  if (process.env.CAPTURE_CHROMIUM) return process.env.CAPTURE_CHROMIUM;
  const roots = [join(homedir(), "Library/Caches/ms-playwright"), join(homedir(), ".cache/ms-playwright")];
  const tails = [
    "chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
    "chrome-mac/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
    "chrome-linux64/chrome",
    "chrome-linux/chrome",
  ];
  for (const root of roots) {
    if (!existsSync(root)) continue;
    const dirs = (await readdir(root)).filter((d) => /^chromium-\d+$/.test(d)).sort((a, b) => Number(b.slice(9)) - Number(a.slice(9)));
    for (const d of dirs) {
      for (const tail of tails) {
        const p = join(root, d, tail);
        if (existsSync(p)) return p;
      }
    }
  }
  throw new Error("no Chromium found: pass --chromium <path> or set CAPTURE_CHROMIUM");
}

function locate(page, locator) {
  let l;
  switch (locator.kind) {
    case "test_id":
      l = page.getByTestId(locator.value);
      break;
    case "role":
      l = page.getByRole(locator.role, { name: locator.name, exact: locator.exact ?? true });
      break;
    default:
      l = page.locator(locator.value);
  }
  return locator.nth === undefined ? l : l.nth(locator.nth);
}

const relativeUrl = (url, origin) => (url.startsWith(origin) ? url.slice(origin.length) || "/" : url);
const firstLine = (text) => String(text).split("\n")[0];

// The shape of a request body: what was sent, not the values.
function bodyShape(req) {
  const text = req.postData();
  if (!text) return null;
  const type = (req.headers()["content-type"] ?? "").split(";")[0].trim() || null;
  let fields = null;
  if (type === "application/json") {
    try {
      const v = JSON.parse(text);
      if (v && typeof v === "object" && !Array.isArray(v)) fields = Object.keys(v).sort();
    } catch {
      fields = null;
    }
  } else if (type === "application/x-www-form-urlencoded") {
    fields = [...new Set(new URLSearchParams(text).keys())].sort();
  }
  return { content_type: type, fields, body_sha256: `sha256:${sha256(text)}` };
}

async function capture(page, origin, screen) {
  const inv = await page.evaluate(() => window.__c2b.inventory());
  const elements = inv.elements.map((e) => ({ node: nodeId(screen, e.path), ...e })).sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  // the local server's port changes per run: written as "local:" so equal runs hash equal
  const dom = (await page.content()).split(origin).join("local:");
  let aria = "";
  try {
    aria = await page.locator("html").ariaSnapshot({ timeout: 5000 });
  } catch {
    aria = "";
  }
  return {
    url: relativeUrl(page.url(), origin),
    dom_sha256: `sha256:${sha256(dom)}`,
    aria_sha256: `sha256:${sha256(aria)}`,
    elements,
    overlays: inv.overlays.map((o) => o.role).sort(),
    text: inv.text,
  };
}

async function settle(page, net, { quiet_ms = 300, max_ms = 6000 } = {}) {
  const until = Date.now() + max_ms;
  let quietSince = null;
  while (Date.now() < until) {
    await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))).catch(() => {});
    if (net.inflight === 0) {
      quietSince ??= Date.now();
      if (Date.now() - quietSince >= quiet_ms) return;
    } else {
      quietSince = null;
    }
    await page.waitForTimeout(50);
  }
}

async function perform(page, action, origin) {
  const timeout = action.timeout_ms ?? 5000;
  const target = () => locate(page, action.locator);
  switch (action.action) {
    case "goto":
      return page.goto(origin + action.value, { waitUntil: "load", timeout });
    case "click":
      return target().click({ timeout });
    case "fill":
      return target().fill(action.value, { timeout });
    case "press":
      return target().press(action.value, { timeout });
    case "blur":
      return target().blur({ timeout });
    case "select_option":
      return target().selectOption(action.value, { timeout });
    case "check":
      return target().check({ timeout });
    case "uncheck":
      return target().uncheck({ timeout });
    default:
      return target().waitFor({ state: action.value ?? "visible", timeout });
  }
}

// Per-step event buckets, keyed by the step an event started in.
class Events {
  constructor() {
    this.step = 0;
    this.inflight = 0;
    this.byStep = new Map();
  }

  push(kind, value, step = this.step) {
    const bucket = this.byStep.get(step) ?? { requests: [], bodies: [], errors: [] };
    bucket[kind].push(value);
    this.byStep.set(step, bucket);
  }

  of(step) {
    const b = this.byStep.get(step) ?? { requests: [], bodies: [], errors: [] };
    const key = (x) => JSON.stringify(x);
    return {
      requests: [...b.requests].sort(),
      request_bodies: [...b.bodies].sort((x, y) => (key(x) < key(y) ? -1 : key(x) > key(y) ? 1 : 0)),
      page_errors: [...new Set(b.errors)].sort(),
    };
  }
}

function stepRecord(index, action, target, status, error, before, after, events) {
  const seen = new Set(before ? before.text : []);
  const appeared = [...new Set(after.text.filter((l) => !seen.has(l)))].sort();
  return {
    index,
    step_id: action.step_id,
    action: action.action,
    locator: action.locator ?? null,
    value: action.action === "fill" ? `sha256:${sha256(String(action.value))}` : (action.value ?? null),
    target,
    status,
    error,
    url_before: before ? before.url : null,
    url_after: after.url,
    dom_sha256: after.dom_sha256,
    aria_sha256: after.aria_sha256,
    overlays: after.overlays,
    texts_appeared: appeared,
    ...events,
    inventory: digest(after.elements),
  };
}

export async function record(scenarioPath, { chromium: chromiumPath } = {}) {
  const scenario = validated(JSON.parse(await readFile(scenarioPath, "utf8")));
  const base = dirname(resolve(scenarioPath));
  const env = { ...DEFAULT_ENV, ...(scenario.environment ?? {}) };
  // `${NAME}` in serve.dir comes from the environment, so scenarios carry no machine-specific paths
  const dir = scenario.serve.dir.replace(/\$\{([A-Z_][A-Z0-9_]*)\}/g, (_, name) => {
    if (process.env[name] === undefined) throw new Error(`serve.dir needs $${name}`);
    return process.env[name];
  });
  const server = await serve(resolve(base, dir), { spaFallback: scenario.serve.spa_fallback ?? true });
  const origin = server.origin;
  const browser = await chromium.launch({ executablePath: await findChromium(chromiumPath), headless: scenario.browser?.headless ?? true });
  const events = new Events();
  const steps = [];
  const inventory = new Map();
  try {
    const context = await browser.newContext({
      viewport: env.viewport,
      deviceScaleFactor: env.device_scale_factor,
      locale: env.locale,
      timezoneId: env.timezone,
      colorScheme: env.color_scheme,
      reducedMotion: env.reduced_motion,
      serviceWorkers: env.service_workers,
      acceptDownloads: false,
    });
    await context.clock.setFixedTime(new Date(env.fixed_time));
    await context.addInitScript(seedRandom, env.random_seed);
    await context.addInitScript(install);
    const local = scenario.storage?.local_storage;
    if (local) {
      await context.addInitScript((items) => {
        for (const [k, v] of Object.entries(items)) if (localStorage.getItem(k) === null) localStorage.setItem(k, v);
      }, local);
    }

    // network: stubs answer, the app's own origin is served, everything else is blocked
    const stubs = scenario.network?.stubs ?? [];
    const blocked = scenario.network?.block_prefixes ?? [];
    const cors = (u) => (`${u.protocol}//${u.host}` === origin ? {} : {
      "access-control-allow-origin": origin,
      "access-control-allow-credentials": "true",
      "access-control-allow-headers": "*, authorization, content-type",
      "access-control-allow-methods": "GET, POST, PUT, PATCH, DELETE, OPTIONS",
    });
    await context.route("**/*", async (route) => {
      const req = route.request();
      const url = new URL(req.url());
      const here = `${url.protocol}//${url.host}`;
      const matching = stubs.filter((s) => (s.origin ?? origin) === here && s.path === url.pathname);
      if (req.method() === "OPTIONS" && matching.length) return route.fulfill({ status: 204, headers: cors(url), body: "" });
      const stub = matching.find((s) => s.method === req.method());
      if (stub) {
        const body = typeof stub.body === "string" ? stub.body : JSON.stringify(stub.body ?? null);
        return route.fulfill({ status: stub.status, headers: cors(url), contentType: stub.content_type ?? "application/json", body });
      }
      if (here !== origin) return route.abort("blockedbyclient");
      if (blocked.some((p) => url.pathname.startsWith(p))) return route.abort("blockedbyclient");
      return route.continue();
    });

    // events, attributed to the step they started in
    const tag = new WeakMap();
    const finish = (req, outcome) => {
      const t = tag.get(req);
      if (!t || t.done) return;
      t.done = true;
      events.inflight -= 1;
      if (!t.recorded) return;
      const line = `${req.method()} ${relativeUrl(req.url(), origin)}`;
      events.push("requests", `${line} ${outcome}`, t.step);
      if (t.body) events.push("bodies", { request: line, ...t.body }, t.step);
    };
    context.on("request", (req) => {
      events.inflight += 1;
      const recorded = RECORDED_TYPES.has(req.resourceType());
      tag.set(req, { step: events.step, recorded, body: recorded ? bodyShape(req) : null, done: false });
    });
    context.on("requestfinished", async (req) => {
      const res = await req.response().catch(() => null);
      finish(req, res ? String(res.status()) : "failed");
    });
    context.on("requestfailed", (req) => {
      finish(req, /BLOCKED_BY_CLIENT/i.test(req.failure()?.errorText ?? "") ? "blocked" : "failed");
    });
    const page = await context.newPage();
    page.setDefaultTimeout(10000);
    page.on("pageerror", (e) => events.push("errors", `pageerror: ${firstLine(e.message).split(origin).join("local:")}`));
    // a failed load is already a network transaction of the step: only other console errors are kept
    page.on("console", (m) => {
      const text = firstLine(m.text());
      if (m.type() === "error" && !text.startsWith("Failed to load resource")) events.push("errors", `console: ${text.split(origin).join("local:")}`);
    });

    // step 0: the start navigation
    const start = { step_id: "start", action: "goto", value: scenario.start_path };
    let status = "completed";
    let error = null;
    try {
      await page.goto(origin + scenario.start_path, { waitUntil: "load" });
      await settle(page, events, scenario.settle);
    } catch (e) {
      status = "failed";
      error = firstLine(e.message);
    }
    let state = await capture(page, origin, scenario.screen);
    // the inventory keeps what was seen on the screen's own route: a page navigated to is another screen
    const onScreen = (st) => st.url.split(/[?#]/)[0] === scenario.start_path.split(/[?#]/)[0];
    const keep = (st) => onScreen(st) && st.elements.forEach((e) => inventory.has(e.node) || inventory.set(e.node, e));
    keep(state);
    steps.push(stepRecord(0, start, null, status, error, null, state, events.of(0)));

    // the actions
    let failed = status === "failed";
    for (const [i, action] of scenario.actions.entries()) {
      const index = i + 1;
      if (failed) {
        steps.push({ index, step_id: action.step_id, action: action.action, status: "cancelled" });
        continue;
      }
      events.step = index;
      let target = null;
      status = "completed";
      error = null;
      try {
        if (action.locator) {
          const el = await locate(page, action.locator).first().evaluate((node) => window.__c2b.describe(node), null, { timeout: action.timeout_ms ?? 5000 });
          const seen = state.elements.find((e) => e.path === el.path);
          target = { node: nodeId(scenario.screen, el.path), path: el.path, tag: el.tag, ...(seen ? { role: seen.role, text: seen.text, href: seen.href, name: seen.name, in_overlay: seen.in_overlay, submitter: seen.submitter, form: seen.form ? nodeId(scenario.screen, seen.form) : null } : {}) };
        }
        await perform(page, action, origin);
        await settle(page, events, scenario.settle);
      } catch (e) {
        status = "failed";
        error = firstLine(e.message);
        failed = true;
      }
      const after = await capture(page, origin, scenario.screen);
      keep(after);
      steps.push(stepRecord(index, action, target, status, error, state, after, events.of(index)));
      state = after;
    }
    await context.close();
  } finally {
    await browser.close();
    await server.close();
  }
  for (const s of steps) if (s.status !== "cancelled") s.replay_digest = digest(s);
  const out = {
    version: 1,
    recording: scenario.id,
    screen: scenario.screen,
    scenario_digest: digest(scenario),
    environment: env,
    start_path: scenario.start_path,
    completeness: steps.every((s) => s.status === "completed") ? "complete" : "partial",
    steps,
    inventory: [...inventory.values()].sort((a, b) => (a.node < b.node ? -1 : a.node > b.node ? 1 : 0)),
  };
  out.run_digest = digest({ ...out, run_digest: undefined });
  return out;
}
