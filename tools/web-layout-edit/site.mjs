// A site, read from its recipe under tools/web-layout-capture/sites/: the one place its source's
// fetch, install, build, and serve, and the route captured, are spelled.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { run, start } from "./run.mjs";

const SITES = fileURLToPath(new URL("../web-layout-capture/sites/", import.meta.url));
const SERVE_SECONDS = 120;

export class Site {
  constructor(name, recipe, route) {
    this.name = name;
    this.recipe = recipe;
    this.route = route;
  }

  // The recipe `name` names, once every tool it requires is on PATH.
  static async open(name) {
    const recipe = path.join(SITES, `${name}.sh`);
    if (!fs.existsSync(recipe)) {
      const known = fs.readdirSync(SITES).filter((file) => file.endsWith(".sh")).map((file) => file.slice(0, -3));
      throw new Error(`no site \`${name}\` under ${SITES}; the sites are ${known.join(", ")}`);
    }
    const read = `. "$0"; printf '%s\\n' "$ROUTE"; for tool in curl node \${REQUIRES[@]+"\${REQUIRES[@]}"}; do command -v "$tool" > /dev/null || printf 'missing %s\\n' "$tool"; done`;
    const [route, ...missing] = (await run("bash", ["-c", read, recipe])).trim().split("\n");
    if (missing.length) throw new Error(`site \`${name}\` needs ${missing.map((line) => `\`${line.slice(8)}\``).join(", ")} on PATH`);
    return new Site(name, recipe, route);
  }

  url(port) {
    return `http://127.0.0.1:${port}${this.route}`;
  }

  fetch(dir) {
    return this.#step("fetch", dir);
  }

  install(dir) {
    return this.#step("install", dir);
  }

  build(dir, log) {
    return this.#step("build", dir, [], log);
  }

  // Serves the build in `dir` at `port`, resolving with the server once the route answers.
  async serve(dir, port, log) {
    const url = this.url(port);
    if (await respond(`http://127.0.0.1:${port}/`)) throw new Error(`port ${port} is in use; free it or set CAPTURE_SITE_PORT`);
    const server = start("bash", ["-c", `. "$0"; site_serve "$1"`, this.recipe, String(port)], { cwd: dir, log });
    for (let second = 0; second < SERVE_SECONDS; second++) {
      if ((await respond(url))?.ok) return server;
      if (!server.running()) throw new Error(`the server stopped before ${url} answered; see ${log}`);
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
    await server.stop();
    throw new Error(`${url} did not answer within ${SERVE_SECONDS} seconds; see ${log}`);
  }

  #step(step, dir, args = [], log = undefined) {
    return run("bash", ["-c", `. "$0"; site_${step} "$@"`, this.recipe, ...args], { cwd: dir, log });
  }
}

// The response at `url`, or null when nothing listens there.
async function respond(url) {
  try {
    return await fetch(url);
  } catch {
    return null;
  }
}
