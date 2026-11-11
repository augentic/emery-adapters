#!/usr/bin/env node
// Experiment / non-production. Point-and-edit: change the design of one element of a site by
// having an agent edit the site's own frontend source, and record the facts after every change.
//
//   node tools/web-layout-edit/edit.mjs --site <site> --session DIR --agent COMMAND
//
// <site> names a recipe under tools/web-layout-capture/sites/. A new session fetches the site's
// source into DIR/source and captures its facts into DIR/facts; a later run carries on from
// where the session stands. The page's build is served at CAPTURE_SITE_PORT or 4173, the port
// capture-site.sh serves at, and opened in a browser:
//
// 1. Right-click an element to select it; "Select parent" widens the selection.
// 2. Write how its design should change and apply. COMMAND runs in DIR/source with the prompt
//    (prompt.md: the instruction and the selected element) as its last argument, for example
//    `cursor-agent -p`. It edits the source, never the facts.
// 3. The page is rebuilt and reloaded, so the browser shows the edit, and the panel shows the
//    source diff.
// 4. Approve captures the edited page's facts into DIR/edits/<n>/facts, commits the change in
//    DIR/source, makes its facts DIR/facts, and sets the facts before and after side by side.
//    Reject reverts the source and reopens the selection, so the instruction can be revised and
//    applied again. Facts are captured for an approved edit alone, since a capture takes a while.
//
// Every decided attempt is one line of DIR/edits.jsonl: the instruction, the element, the source
// diff, its status, how long each step took, and for an approved edit the facts' digests before
// and after and its commit.
//
// The editor settles a design before Emery sees it. Once it is settled, run `emery specify` over
// a config whose `emery:rendered-layout` source `path` names DIR/facts, then `emery apply`. A
// later change is edited here again, then `emery amend` and `emery apply`.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

import { chromium } from "playwright";

import { capture, digest, pair } from "./facts.mjs";
import { run } from "./run.mjs";
import { Session } from "./session.mjs";
import { Site } from "./site.mjs";
import { Source } from "./source.mjs";

const PANEL = new URL("./panel.js", import.meta.url);
const PROMPT = fs.readFileSync(new URL("./prompt.md", import.meta.url), "utf8");
const USAGE = "usage: node edit.mjs --site <site> --session DIR --agent COMMAND";

const { values } = parseArgs({ options: { site: { type: "string" }, session: { type: "string" }, agent: { type: "string" } } });
if (!values.site || !values.session || !values.agent) {
  console.error(USAGE);
  process.exit(2);
}

const port = Number(process.env.CAPTURE_SITE_PORT ?? 4173);
const site = await Site.open(values.site);
const session = new Session(path.resolve(values.session));
const url = site.url(port);
const log = (name) => path.join(session.dir, `${name}.log`);

say(`opening ${session.source}`);
const source = await Source.open(session.source, site);
if (!(await source.clean())) {
  throw new Error(`${session.source} holds changes no edit made; commit or revert them first`);
}
say(`building and serving the page (${log("build")})`);
await site.build(session.source, log("build"));
let server = await site.serve(session.source, port, log("serve"));
if (!fs.existsSync(session.facts)) {
  say(`capturing the page's facts (${log("capture")})`);
  await capture(url, session.facts, log("capture"));
}

const browser = await chromium.launch({ headless: false });
const page = await browser.newPage({ viewport: null });
let state = { phase: "idle" };
let pending = null;
await page.addInitScript({ path: fileURLToPath(PANEL) });
await page.exposeBinding("__layoutEditIntent", (_, intent) => {
  handle(intent).catch((err) => show({ phase: "idle", message: `the editor failed: ${err.message}` }));
});
page.on("load", () => show(state));
browser.on("disconnected", () => shutdown(0));
process.on("SIGINT", () => shutdown(130));
await page.goto(url);
say(`editing ${url}; right-click an element in the browser`);

async function handle(intent) {
  if (intent.type === "close" && (state.phase === "select" || state.phase === "approved")) return show({ phase: "idle" });
  if (intent.type === "apply" && (state.phase === "idle" || state.phase === "select")) return attempt(intent.element, intent.instruction);
  if (intent.type === "approve" && state.phase === "review") return approve();
  if (intent.type === "reject" && state.phase === "review") return reject();
}

// Steps 2 and 3: the agent's edit, and the page rebuilt onto it for the operator to look at.
async function attempt(element, instruction) {
  const { n, dir } = session.attempt();
  const entry = { attempt: n, at: new Date().toISOString(), instruction, element, diff: "", ms: {} };
  let rebuilt = false;
  try {
    const prompt = PROMPT.replace("{{instruction}}", () => instruction)
      .replace("{{url}}", () => url)
      .replace("{{element}}", () => JSON.stringify(element, null, 2));
    await timed(entry, "agent", "The agent is editing the source…", () =>
      run("bash", ["-c", `${values.agent} "$1"`, "web-layout-edit", prompt], { cwd: session.source, log: path.join(dir, "agent.log") }));
    entry.diff = await source.diff();
    if (!entry.diff) throw new Error("the agent changed nothing in the source");
    rebuilt = true;
    await timed(entry, "build", "Building and serving the edited page…", () => republish(path.join(dir, "build.log")));
    pending = { entry, dir };
    await show(review());
  } catch (err) {
    session.log({ ...entry, status: "failed", error: err.message });
    await restore(rebuilt);
    await show({ phase: "select", element, instruction, error: `Edit ${n} failed and was reverted: ${err.message}` });
  }
}

// Step 4, approved: the edited page's facts are captured, the change is committed, and its
// facts become the session's. A capture that fails commits nothing and leaves the review open.
async function approve() {
  const { entry, dir } = pending;
  const facts = path.join(dir, "facts");
  try {
    await timed(entry, "capture", "Capturing the approved page's facts…", () => capture(url, facts, path.join(dir, "capture.log")));
    entry.facts = { before: await digest(session.facts), after: await digest(facts) };
    await pair(session.facts, facts, dir);
  } catch (err) {
    return show(review(`Capturing the facts failed; nothing was committed: ${err.message}`));
  }
  pending = null;
  const commit = await source.commit(`edit ${entry.attempt}: ${entry.instruction}`);
  session.adopt(facts);
  session.log({ ...entry, status: "approved", commit });
  await show({ phase: "approved", attempt: entry.attempt, element: entry.element, instruction: entry.instruction, facts: entry.facts, pairs: pairs(dir) });
}

function review(error = undefined) {
  const { entry } = pending;
  return { phase: "review", attempt: entry.attempt, element: entry.element, instruction: entry.instruction, diff: entry.diff, error };
}

async function timed(entry, name, message, work) {
  await show({ phase: "busy", message, instruction: entry.instruction });
  const started = Date.now();
  try {
    return await work();
  } finally {
    entry.ms[name] = Date.now() - started;
  }
}

// Step 4, rejected: the source goes back to the last approved design, the selection reopens.
async function reject() {
  const { entry } = pending;
  pending = null;
  session.log({ ...entry, status: "rejected" });
  await show({ phase: "busy", message: "Reverting the source…", instruction: entry.instruction });
  await restore(true);
  await show({ phase: "select", element: entry.element, instruction: entry.instruction });
}

async function restore(rebuilt) {
  await source.revert();
  if (rebuilt) await republish(log("build"));
}

// The source's build served again and the page reloaded onto it.
async function republish(buildLog) {
  await server.stop();
  await site.build(session.source, buildLog);
  server = await site.serve(session.source, port, log("serve"));
  await page.reload();
}

function pairs(dir) {
  return fs.readdirSync(dir)
    .map((name) => /^pair-(\d+)\.svg$/.exec(name))
    .filter(Boolean)
    .map(([name, width]) => ({ width: Number(width), file: path.join(dir, name), svg: fs.readFileSync(path.join(dir, name), "utf8") }))
    .sort((a, b) => a.width - b.width);
}

async function show(next) {
  state = next;
  await page.evaluate((shown) => window.__layoutEdit?.show(shown), state).catch(() => {});
}

async function shutdown(code) {
  await server.stop();
  await browser.close().catch(() => {});
  process.exit(code);
}

function say(message) {
  console.error(`web-layout-edit: ${message}`);
}
