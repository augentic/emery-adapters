#!/usr/bin/env node
// capture-to-behaviour check --scenario <file> [--scenario <file>]...
// capture-to-behaviour record --scenario <file> --out <recording.json> [--chromium <path>]
// capture-to-behaviour apply --behaviour <behaviour.json> --recording <file>... --out <behaviour.json>
// capture-to-behaviour compare --recording <left.json> --recording <right.json> [--mode digest|behaviour]

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

import { apply } from "../lib/apply.mjs";
import { stable } from "../lib/canonical.mjs";
import { compare, compareBehaviour } from "../lib/compare.mjs";
import { record } from "../lib/record.mjs";
import { problems } from "../lib/scenario.mjs";

const USAGE = `usage:
  capture-to-behaviour check --scenario <file> [--scenario <file>]...
  capture-to-behaviour record --scenario <file> --out <recording.json> [--chromium <path>]
  capture-to-behaviour apply --behaviour <behaviour.json> --recording <file>... --out <behaviour.json>
  capture-to-behaviour compare --recording <left.json> --recording <right.json> [--mode digest|behaviour]`;
const REPEATABLE = new Set(["recording", "scenario"]);

function parse(argv) {
  const [command, ...rest] = argv;
  const opts = { command, recording: [], scenario: [] };
  for (let i = 0; i < rest.length; i += 2) {
    const flag = rest[i];
    const value = rest[i + 1];
    if (!flag.startsWith("--") || value === undefined) throw new Error(`bad argument ${flag}`);
    const key = flag.slice(2);
    if (REPEATABLE.has(key)) opts[key].push(value);
    else opts[key] = value;
  }
  return opts;
}

const readJson = async (path) => JSON.parse(await readFile(path, "utf8"));

async function write(path, value) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, stable(value));
}

const commands = {
  async check(o) {
    if (!o.scenario.length) return false;
    let bad = 0;
    for (const path of o.scenario) {
      const found = problems(await readJson(path));
      if (found.length) bad += 1;
      console.error(found.length ? `${path}:\n  ${found.join("\n  ")}` : `${path}: ok`);
    }
    process.exitCode = bad ? 1 : 0;
    return true;
  },
  async record(o) {
    if (o.scenario.length !== 1 || !o.out) return false;
    const rec = await record(o.scenario[0], { chromium: o.chromium });
    await write(o.out, rec);
    const failed = rec.steps.filter((s) => s.status !== "completed");
    const detail = failed.map((s) => `${s.step_id} ${s.status}${s.error ? `: ${s.error}` : ""}`).join("; ");
    console.error(`${rec.recording}: ${rec.steps.length} steps, ${rec.completeness}${detail ? `; ${detail}` : ""}`);
    process.exitCode = rec.completeness === "complete" ? 0 : 1;
    return true;
  },
  async apply(o) {
    if (!o.behaviour || !o.recording.length || !o.out) return false;
    const { doc, report } = apply(await readJson(o.behaviour), await Promise.all(o.recording.map(readJson)));
    await write(o.out, doc);
    console.error(JSON.stringify(report));
    return true;
  },
  async compare(o) {
    if (o.recording.length !== 2) return false;
    const [left, right] = await Promise.all(o.recording.map(readJson));
    const mode = o.mode ?? "digest";
    if (mode !== "digest" && mode !== "behaviour") return false;
    const result = mode === "behaviour" ? compareBehaviour(left, right) : compare(left, right);
    process.stdout.write(stable(result));
    process.exitCode = result.result === (mode === "behaviour" ? "same" : "unchanged") ? 0 : 1;
    return true;
  },
};

async function main() {
  let opts;
  try {
    opts = parse(process.argv.slice(2));
  } catch (e) {
    console.error(`${e.message}\n${USAGE}`);
    process.exit(2);
  }
  const run = Object.hasOwn(commands, opts.command) ? commands[opts.command] : null;
  if (!run || !(await run(opts))) {
    console.error(USAGE);
    process.exit(2);
  }
}

main().catch((e) => {
  console.error(e.stack || String(e));
  process.exit(1);
});
