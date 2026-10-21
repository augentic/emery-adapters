#!/usr/bin/env node
// run-samples <out-dir> [--only <id>,...]
//
// Every sample of samples.json, end to end: code-to-behaviour (static
// facts), one recording per scenarios/<id>/*.json, apply, schema check, and
// a coverage report when a ui-ir capture exists. Writes
//   <out>/<id>/static/       behaviour.json + behaviour.unmapped.json
//   <out>/<id>/recordings/   one file per scenario
//   <out>/<id>/behaviour.json  code + recordings
//   <out>/<id>/coverage.json   when <capture>/computed.json exists
//   <out>/summary.json
// `${NAME}` in samples.json and in scenarios comes from the environment;
// EXPERIMENT is required; SITES and CAPTURES default under it.

import { execFileSync, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { stable } from "../lib/canonical.mjs";

const HERE = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const ROOT = resolve(HERE, "../../..");
const CLI = join(HERE, "bin/capture-to-behaviour.mjs");

if (!process.env.EXPERIMENT) {
  console.error("run-samples: set EXPERIMENT to the directory holding the sample apps");
  process.exit(2);
}
process.env.SITES ??= join(process.env.EXPERIMENT, "behaviour-sites");
process.env.CAPTURES ??= join(process.env.EXPERIMENT, "ui-ir/captures");
const expand = (text) => text.replace(/\$\{([A-Z_][A-Z0-9_]*)\}/g, (_, n) => process.env[n] ?? "");

const run = (cmd, args) => spawnSync(cmd, args, { encoding: "utf8", cwd: ROOT });
const readJson = async (p) => JSON.parse(await readFile(p, "utf8"));

async function sample(s, out, bins) {
  const dir = join(out, s.id);
  const result = { id: s.id, screen: s.screen, entry: s.entry };
  const c2b = run(bins.code, ["--source", expand(s.source), "--entry", s.entry, "--screen", s.screen, "--out", join(dir, "static")]);
  if (c2b.status !== 0) return { ...result, error: `code-to-behaviour: ${c2b.stderr.trim()}` };
  const doc = await readJson(join(dir, "static/behaviour.json"));
  const unmapped = await readJson(join(dir, "static/behaviour.unmapped.json"));
  result.static = {
    triggers: doc.triggers.length,
    effects: doc.triggers.reduce((n, t) => n + t.does.length, 0),
    routes: doc.routes.length,
    guards: doc.guards.length,
    state: Object.keys(doc.state).length,
    validation: doc.validation.length,
    transitions: doc.transitions.length,
    unmapped: unmapped.counts,
  };

  const scenarios = (await readdir(join(HERE, "scenarios", s.id))).filter((f) => f.endsWith(".json")).sort();
  const recordings = [];
  result.recordings = [];
  for (const f of scenarios) {
    const path = join(dir, "recordings", f);
    const rec = run("node", [CLI, "record", "--scenario", join(HERE, "scenarios", s.id, f), "--out", path]);
    const line = rec.stderr.trim().split("\n").at(-1);
    if (!existsSync(path)) {
      result.recordings.push({ scenario: f, error: line });
      continue;
    }
    const r = await readJson(path);
    recordings.push(path);
    result.recordings.push({
      scenario: r.recording,
      completeness: r.completeness,
      steps: r.steps.length,
      failed: r.steps.filter((x) => x.status === "failed").map((x) => `${x.step_id}: ${x.error}`),
      requests: r.steps.reduce((n, x) => n + (x.requests?.length ?? 0), 0),
      page_errors: [...new Set(r.steps.flatMap((x) => x.page_errors ?? []))],
      run_digest: r.run_digest,
    });
  }
  if (!recordings.length) return result;
  const applied = run("node", [CLI, "apply", "--behaviour", join(dir, "static/behaviour.json"), ...recordings.flatMap((p) => ["--recording", p]), "--out", join(dir, "behaviour.json")]);
  if (applied.status !== 0) return { ...result, error: `apply: ${applied.stderr.trim()}` };
  result.apply = JSON.parse(applied.stderr.trim().split("\n").at(-1));
  const check = run(bins.check, ["check", join(dir, "static/behaviour.json"), join(dir, "behaviour.json")]);
  result.schema = { ok: check.status === 0, output: (check.stdout + check.stderr).trim() };
  const computed = join(expand(s.capture), "computed.json");
  if (existsSync(computed)) {
    const cov = run(bins.check, ["coverage", join(dir, "behaviour.json"), computed]);
    if (cov.status === 0) {
      await writeFile(join(dir, "coverage.json"), cov.stdout);
      const c = JSON.parse(cov.stdout);
      result.coverage = { event_holes: c.event_holes, matched_resolvable: c.matched_resolvable, matched_without_effects: c.matched_without_effects.length, unclaimed: c.unclaimed.length, trigger_nodes_outside_capture: c.trigger_nodes_outside_capture };
    } else {
      result.coverage = { error: cov.stderr.trim() };
    }
  } else {
    result.coverage = null;
  }
  return result;
}

async function main() {
  const [out, flag, only] = process.argv.slice(2);
  if (!out || (flag && flag !== "--only")) {
    console.error("usage: run-samples <out-dir> [--only <id>,...]");
    process.exit(2);
  }
  execFileSync("cargo", ["build", "-q", "-p", "code-to-behaviour", "-p", "ui-ir-behaviour", "--features", "ui-ir-behaviour/validate"], { cwd: ROOT, stdio: "inherit" });
  const bins = { code: join(ROOT, "target/debug/code-to-behaviour"), check: join(ROOT, "target/debug/ui-ir-behaviour") };
  const wanted = only ? new Set(only.split(",")) : null;
  const samples = (await readJson(join(HERE, "samples.json"))).filter((s) => !wanted || wanted.has(s.id));
  await mkdir(out, { recursive: true });
  const results = [];
  for (const s of samples) {
    console.error(`== ${s.id}`);
    results.push(await sample(s, resolve(out), bins));
  }
  await writeFile(join(out, "summary.json"), stable(results));
  console.error(`wrote ${join(out, "summary.json")}`);
}

main().catch((e) => {
  console.error(e.stack || String(e));
  process.exit(1);
});
