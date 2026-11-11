#!/usr/bin/env node
// Experiment / non-production. Point-and-edit: change the design of one element of a site at a
// time by having an agent edit the site's own frontend source, then record the facts once.
//
//   node tools/web-layout-edit/edit.mjs --site <site> --session DIR --agent COMMAND
//
// <site> names a recipe under tools/web-layout-capture/sites/. A new session fetches the site's
// source into DIR/source and captures its facts into DIR/facts; a later run carries on from
// where the session stands. The page's build is served at CAPTURE_SITE_PORT or 4173, the port
// capture-site.sh serves at, and opened in a browser:
//
// 1. Right-click an element to select it; "Select parent" widens the selection.
// 2. Write how its design should change and queue it. Queue as many as wanted, on any element,
//    without waiting; they run one at a time in the order they were queued.
// 3. For each queued instruction in turn, COMMAND runs in DIR/source with the prompt (prompt.md:
//    the instruction and the element) as its last argument, for example `cursor-agent -p`. It
//    edits the source, never the facts. The page is rebuilt and reloaded, so the browser shows
//    the edit, and the panel shows the source diff.
// 4. Approve commits the change in DIR/source and adds the instruction to DIR/brief/brief.md;
//    the next queued instruction then runs. Reject reverts the source and drops the edit.
//    Neither captures facts, so an edit costs the agent and a build alone.
// 5. Finish, once every edit is approved, captures the page's facts into DIR/records/<n>/facts,
//    makes them DIR/facts, and sets the facts before and after side by side.
//
// DIR/edits.jsonl holds one line per decided attempt (the instruction, the element, the source
// diff, its status, how long each step took, and an approved one's commit) and one per record
// (the attempts it covers and the facts' digests before and after).
//
// The editor settles a design before Emery sees it. Once it is settled, run `emery specify` over
// a config whose `emery:rendered-layout` source `path` names DIR/facts/ and whose
// `emery:intent` source `path` names DIR/brief/, then `emery apply`. A later change is edited
// here again and finished, then `emery amend` and `emery apply`.

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
let pending = null; // the edit awaiting a decision, or null
let busy = false; // an agent or build is running
const queue = []; // the instructions waiting their turn, oldest first
await page.addInitScript({ path: fileURLToPath(PANEL) });
await page.exposeBinding("__layoutEditIntent", (_, intent) => {
  handle(intent).catch((err) => show(idle(`The editor failed: ${err.message}`)));
});
page.on("load", () => show(state));
browser.on("disconnected", () => shutdown(0));
process.on("SIGINT", () => shutdown(130));
await page.goto(url);
say(`editing ${url}; right-click an element in the browser`);

async function handle(intent) {
  const settled = !pending && !busy;
  if (intent.type === "queue") return enqueue(intent.element, intent.instruction);
  if (intent.type === "approve" && pending) return approve();
  if (intent.type === "reject" && pending) return reject();
  if (intent.type === "close" && settled) return show(idle());
  if (intent.type === "finish" && settled && session.unrecorded().length) return finish();
}

// Step 2: an instruction joins the queue and, if nothing is running, starts the queue moving.
async function enqueue(element, instruction) {
  queue.push({ element, instruction });
  await show(state);
  await pump();
}

// Runs queued instructions one at a time: the next starts only once the one before it is decided.
async function pump() {
  if (busy || pending || !queue.length) return;
  busy = true;
  const { element, instruction } = queue.shift();
  const started = await attempt(element, instruction);
  busy = false;
  if (!started) await pump();
}

// Step 3: the agent's edit, and the page rebuilt onto it. Returns whether a review is now open;
// a failed attempt is reverted and recorded, and lets the queue move on.
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
    return true;
  } catch (err) {
    session.log({ ...entry, status: "failed", error: err.message });
    await restore(rebuilt);
    await show(idle(`Edit ${n} failed and was reverted: ${err.message}`));
    return false;
  }
}

// Step 4, approved: the change is committed and its instruction joins the brief; the queue moves on.
async function approve() {
  const { entry } = pending;
  pending = null;
  const commit = await source.commit(`edit ${entry.attempt}: ${entry.instruction}`);
  session.note(entry.element, entry.instruction);
  session.log({ ...entry, status: "approved", commit });
  await show(idle(`Edit ${entry.attempt} approved.`));
  await pump();
}

// Step 5: the page's facts captured once over every approved edit, and made the session's.
// A capture that fails leaves the session's facts as they stood.
async function finish() {
  const edits = session.unrecorded();
  const { n, dir } = session.capture();
  const entry = { record: n, at: new Date().toISOString(), edits, ms: {} };
  const facts = path.join(dir, "facts");
  try {
    await timed(entry, "capture", "Capturing the page's facts…", () => capture(url, facts, path.join(dir, "capture.log")));
    entry.facts = { before: await digest(session.facts), after: await digest(facts) };
    await pair(session.facts, facts, dir);
  } catch (err) {
    return show(idle(`Capturing the facts failed; ${session.facts} is unchanged: ${err.message}`));
  }
  session.adopt(facts);
  session.log({ ...entry, status: "recorded" });
  await show({ phase: "finished", edits, facts: entry.facts, pairs: pairs(dir) });
}

function idle(message = undefined) {
  return { phase: "idle", message };
}

function review() {
  const { entry } = pending;
  return { phase: "review", attempt: entry.attempt, element: entry.element, instruction: entry.instruction, diff: entry.diff };
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

// Step 4, rejected: the source goes back to the last approved design; the queue moves on.
async function reject() {
  const { entry } = pending;
  pending = null;
  session.log({ ...entry, status: "rejected" });
  await show({ phase: "busy", message: "Reverting the source…", instruction: entry.instruction });
  await restore(true);
  await show(idle(`Edit ${entry.attempt} rejected and reverted.`));
  await pump();
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
  state = {
    ...next,
    unrecorded: session.unrecorded().length,
    queued: queue.map(({ element, instruction }) => ({ tag: element.tag, text: element.text, instruction })),
  };
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
