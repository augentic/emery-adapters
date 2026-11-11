// The served page's facts, through the `web-layout-capture` CLI: what the facts are, how they
// are digested, and how they are drawn is the Rust crates', never this tool's.

import { fileURLToPath } from "node:url";

import { run } from "./run.mjs";

const MANIFEST = fileURLToPath(new URL("../../Cargo.toml", import.meta.url));

function cli(args, log) {
  return run("cargo", ["run", "-q", "--manifest-path", MANIFEST, "-p", "web-layout-capture", "--", ...args], { log });
}

// Captures the page at `url` into `out`, replacing it.
export function capture(url, out, log) {
  return cli(["capture", "--url", url, "--out", out], log);
}

// The `sha256:` digest a spec names the facts in `dir` by.
export async function digest(dir) {
  return (await cli(["digest", "--facts", dir])).trim();
}

// Writes `pair-<width>.svg` into `out` at each review width: `before` and `after` side by side.
export function pair(before, after, out) {
  return cli(["pair", "--before", before, "--after", after, "--out", out]);
}
