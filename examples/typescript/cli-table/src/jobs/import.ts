import { readFile } from "node:fs/promises";
import { parse } from "csv-parse/sync";
import { logger, MAX_AMOUNT_CENTS } from "../config";
import { insertEntry } from "../lib/db";
import type { JobOptions } from "./job";

interface Row {
  posted_on: string;
  amount: string;
  reference: string;
}

const DATE = /^\d{4}-\d{2}-\d{2}$/;

// `ops import <file>`: every well-formed row of a ledger CSV export is
// inserted; a malformed one is counted and skipped; the exit code says
// whether anything was rejected.
export async function importLedger(args: string[], options: JobOptions): Promise<number> {
  const [file] = args;
  if (!file) {
    logger.error("import needs a file");
    return 64;
  }
  const rows = parse(await readFile(file, "utf8"), { columns: true, skip_empty_lines: true }) as Row[];

  let imported = 0;
  let rejected = 0;
  for (const row of rows) {
    const amountCents = Math.round(Number(row.amount) * 100);
    if (!DATE.test(row.posted_on) || !Number.isFinite(amountCents) || Math.abs(amountCents) > MAX_AMOUNT_CENTS) {
      rejected += 1;
      continue;
    }
    if (!options.dryRun) {
      await insertEntry({ postedOn: row.posted_on, amountCents, reference: row.reference.trim() });
    }
    imported += 1;
  }

  logger.info({ imported, rejected, dryRun: options.dryRun }, "import finished");
  return rejected > 0 && !options.dryRun ? 2 : 0;
}
