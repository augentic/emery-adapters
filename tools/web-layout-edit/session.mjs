// One editing session's files, under the directory `--session` names:
//
//   source/       the site's own frontend source (source.mjs)
//   facts/        the facts of the source as approved so far, the folder `emery specify` reads
//   edits/<n>/    attempt n: its facts, `pair-<width>.svg` before and after, and its logs
//   edits.jsonl   the record, one line per decided attempt

import fs from "node:fs";
import path from "node:path";

export class Session {
  constructor(dir) {
    this.dir = dir;
    this.source = path.join(dir, "source");
    this.facts = path.join(dir, "facts");
    this.record = path.join(dir, "edits.jsonl");
  }

  // A fresh directory for the next attempt, and its number.
  attempt() {
    const edits = path.join(this.dir, "edits");
    fs.mkdirSync(edits, { recursive: true });
    const taken = fs.readdirSync(edits).map(Number).filter(Number.isInteger);
    const n = Math.max(0, ...taken) + 1;
    const dir = path.join(edits, String(n));
    fs.mkdirSync(dir);
    return { n, dir };
  }

  // Appends one decided attempt to the record.
  log(entry) {
    fs.appendFileSync(this.record, `${JSON.stringify(entry)}\n`);
  }

  // Makes `facts` the session's facts, replacing what stood.
  adopt(facts) {
    fs.rmSync(this.facts, { recursive: true, force: true });
    fs.cpSync(facts, this.facts, { recursive: true });
  }
}
