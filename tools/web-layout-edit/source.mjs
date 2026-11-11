// The site's own frontend source, as the editor changes it: a git repository of its own whose
// HEAD is the design as approved so far, one commit per approved edit over the fetched baseline.

import fs from "node:fs";

import { run } from "./run.mjs";

// The editor commits under its own name, unsigned, whatever the operator's git config says.
const GIT = ["-c", "user.name=web-layout-edit", "-c", "user.email=web-layout-edit@localhost", "-c", "commit.gpgsign=false"];

export class Source {
  constructor(dir) {
    this.dir = dir;
  }

  // The source in `dir`, fetched, committed as the baseline, and installed when `dir` is new.
  static async open(dir, site) {
    const source = new Source(dir);
    if (fs.existsSync(dir)) return source;
    fs.mkdirSync(dir, { recursive: true });
    try {
      await site.fetch(dir);
      await source.#git("init", "-q");
      await source.commit("baseline: the site as fetched");
      await site.install(dir);
    } catch (err) {
      fs.rmSync(dir, { recursive: true, force: true });
      throw err;
    }
    return source;
  }

  async clean() {
    return (await this.#git("status", "--porcelain")).trim() === "";
  }

  // Every change since HEAD, new files included, as a patch.
  async diff() {
    await this.#git("add", "-A");
    return this.#git("diff", "--cached", "--no-color");
  }

  // Drops every change since HEAD; ignored files, the install and the build among them, stay.
  async revert() {
    await this.#git("reset", "-q", "--hard");
    await this.#git("clean", "-q", "-fd");
  }

  // Commits every change and answers the commit.
  async commit(message) {
    await this.#git("add", "-A");
    await this.#git("commit", "-q", "--allow-empty", "-m", message);
    return (await this.#git("rev-parse", "HEAD")).trim();
  }

  #git(...args) {
    return run("git", [...GIT, ...args], { cwd: this.dir });
  }
}
