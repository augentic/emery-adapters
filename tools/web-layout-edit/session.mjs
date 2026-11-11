// One editing session's files, under the directory `--session` names:
//
//   source/            the site's own frontend source (source.mjs)
//   facts/             the facts of the source as last recorded, the folder `emery:rendered-layout` reads
//   brief/brief.md     every approved instruction, the one file `emery:intent` reads
//   edits/<n>/         attempt n: its agent and build logs
//   records/<n>/       record n: the facts captured, `pair-<width>.svg` before and after, and its log
//   edits.jsonl        one line per decided attempt and per record

import fs from "node:fs";
import path from "node:path";

const BRIEF_HEADING = "# Design changes\n\nEach line is one approved change to the design of the element it names.\n\n";

export class Session {
  constructor(dir) {
    this.dir = dir;
    this.source = path.join(dir, "source");
    this.facts = path.join(dir, "facts");
    this.brief = path.join(dir, "brief", "brief.md");
    this.record = path.join(dir, "edits.jsonl");
  }

  // A fresh directory for the next attempt, and its number.
  attempt() {
    return numbered(path.join(this.dir, "edits"));
  }

  // A fresh directory for the next record of the facts, and its number.
  capture() {
    return numbered(path.join(this.dir, "records"));
  }

  // Appends one decided attempt or record to the record.
  log(entry) {
    fs.appendFileSync(this.record, `${JSON.stringify(entry)}\n`);
  }

  // The attempts approved since the facts were last recorded, oldest first.
  unrecorded() {
    if (!fs.existsSync(this.record)) return [];
    const entries = fs.readFileSync(this.record, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));
    const recorded = entries.findLastIndex((entry) => entry.status === "recorded");
    return entries.slice(recorded + 1).filter((entry) => entry.status === "approved").map((entry) => entry.attempt);
  }

  // Adds an approved instruction about `element` to the brief.
  note(element, instruction) {
    fs.mkdirSync(path.dirname(this.brief), { recursive: true });
    if (!fs.existsSync(this.brief)) fs.writeFileSync(this.brief, BRIEF_HEADING);
    const named = element.text ? `the <${element.tag}> "${element.text}"` : `the <${element.tag}> at ${element.path}`;
    fs.appendFileSync(this.brief, `- ${named}: ${instruction.replace(/\s+/g, " ")}\n`);
  }

  // Makes `facts` the session's facts, replacing what stood.
  adopt(facts) {
    fs.rmSync(this.facts, { recursive: true, force: true });
    fs.cpSync(facts, this.facts, { recursive: true });
  }
}

function numbered(parent) {
  fs.mkdirSync(parent, { recursive: true });
  const taken = fs.readdirSync(parent).map(Number).filter(Number.isInteger);
  const n = Math.max(0, ...taken) + 1;
  const dir = path.join(parent, String(n));
  fs.mkdirSync(dir);
  return { n, dir };
}
