import { readFile } from "node:fs/promises";
import { logger } from "../config";
import { parseLedgerCsv } from "../lib/csv";
import { insertEntries } from "../lib/db";

export interface ImportSummary {
  file: string;
  parsed: number;
  inserted: number;
  rejected: number;
  dryRun: boolean;
}

export async function importLedger(file: string, dryRun: boolean): Promise<ImportSummary> {
  const text = await readFile(file, "utf8");
  const { entries, errors } = parseLedgerCsv(text);
  for (const error of errors) {
    logger.warn({ file, line: error.line }, error.message);
  }
  const inserted = dryRun ? 0 : await insertEntries(entries);
  const summary = { file, parsed: entries.length, inserted, rejected: errors.length, dryRun };
  logger.info(summary, dryRun ? "import previewed" : "import committed");
  return summary;
}
