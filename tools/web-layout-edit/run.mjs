// Child processes for the editor. A command's stdout is its answer and its stderr goes to
// stderr, unless a log file is named, which takes both; a command that fails rejects with the
// tail of what it printed.

import { spawn } from "node:child_process";
import fs from "node:fs";

const TAIL_LINES = 40;

// Runs `command` with `args` in `cwd` to completion, resolving with its stdout.
export function run(command, args, { cwd, log } = {}) {
  return new Promise((resolve, reject) => {
    const file = log ? fs.createWriteStream(log, { flags: "a" }) : null;
    const child = spawn(command, args, { cwd, stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let printed = "";
    child.stdout.on("data", (chunk) => { stdout += chunk; printed += chunk; file?.write(chunk); });
    child.stderr.on("data", (chunk) => { printed += chunk; (file ?? process.stderr).write(chunk); });
    child.on("error", reject);
    child.on("close", (code, signal) => {
      file?.end();
      if (code === 0) return resolve(stdout);
      const tail = printed.trimEnd().split("\n").slice(-TAIL_LINES).join("\n");
      reject(new Error(`\`${command} ${args[0] ?? ""}\` failed (${signal ?? `exit ${code}`})${tail ? `:\n${tail}` : ""}`));
    });
  });
}

// Starts `command` in its own process group, so stopping it stops whatever it started too.
export function start(command, args, { cwd, log }) {
  const out = fs.openSync(log, "a");
  const child = spawn(command, args, { cwd, detached: true, stdio: ["ignore", out, out] });
  fs.closeSync(out);
  const exited = new Promise((resolve) => child.on("exit", resolve));
  const running = () => child.exitCode === null && child.signalCode === null;
  return {
    running,
    async stop() {
      if (!running()) return;
      try { process.kill(-child.pid, "SIGTERM"); } catch { /* already gone */ }
      await exited;
    },
  };
}
