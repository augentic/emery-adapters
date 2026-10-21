// Node id rule (ui-ir-staging/docs/node-id.md):
//   node_id(screen, path) = screen + "/n-" + first16hex(sha256(utf8(path)))

import { sha256 } from "./canonical.mjs";

export function nodeId(screen, path) {
  return `${screen}/n-${sha256(path).slice(0, 16)}`;
}
