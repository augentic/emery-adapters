// Canonical JSON and its SHA-256 digest.
//
// Same idea as rea-agents' canonicalDigest (sha256 over RFC 8785 JSON):
// object keys sorted by UTF-16 code units, no whitespace, numbers in the
// ECMAScript shortest form. Our own implementation.

import { createHash } from "node:crypto";

export function canonical(value) {
  if (value === null || typeof value === "boolean") return JSON.stringify(value);
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new Error("canonical: non-finite number");
    return JSON.stringify(value);
  }
  if (typeof value === "string") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (typeof value === "object") {
    const keys = Object.keys(value).filter((k) => value[k] !== undefined).sort();
    return `{${keys.map((k) => `${JSON.stringify(k)}:${canonical(value[k])}`).join(",")}}`;
  }
  throw new Error(`canonical: unsupported ${typeof value}`);
}

export function sha256(text) {
  return createHash("sha256").update(text, "utf8").digest("hex");
}

export function digest(value) {
  return `sha256:${sha256(canonical(value))}`;
}

// Pretty, key-sorted JSON for files: stable bytes for equal values.
export function stable(value) {
  return `${JSON.stringify(JSON.parse(canonical(value)), null, 2)}\n`;
}
