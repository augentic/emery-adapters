import { parse } from "csv-parse/sync";
import type { LedgerEntry } from "./db";

export const MAX_ROWS = 50000;
export const REQUIRED_COLUMNS = ["id", "account", "posted_on", "amount", "reference"] as const;
const ACCOUNT_CODE = /^\d{4}(-\d{2})?$/;
const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;

export interface RowError {
  line: number;
  message: string;
}

export interface ParsedImport {
  entries: LedgerEntry[];
  errors: RowError[];
}

export function parseLedgerCsv(text: string): ParsedImport {
  const records = parse(text, { columns: true, skip_empty_lines: true, trim: true }) as Record<string, string>[];
  if (records.length > MAX_ROWS) {
    throw new Error(`import has ${records.length} rows; the limit is ${MAX_ROWS}`);
  }
  const header = records[0] === undefined ? [] : Object.keys(records[0]);
  const missing = REQUIRED_COLUMNS.filter((column) => !header.includes(column));
  if (records.length > 0 && missing.length > 0) {
    throw new Error(`import is missing columns: ${missing.join(", ")}`);
  }

  const entries: LedgerEntry[] = [];
  const errors: RowError[] = [];
  records.forEach((record, index) => {
    const line = index + 2;
    if (!ACCOUNT_CODE.test(record.account)) {
      errors.push({ line, message: `account ${record.account} is not a four-digit code` });
      return;
    }
    if (!ISO_DATE.test(record.posted_on)) {
      errors.push({ line, message: `posted_on ${record.posted_on} is not an ISO date` });
      return;
    }
    const amountCents = Math.round(Number(record.amount) * 100);
    if (!Number.isFinite(amountCents) || amountCents === 0) {
      errors.push({ line, message: `amount ${record.amount} is not a non-zero number` });
      return;
    }
    entries.push({
      id: record.id,
      account: record.account,
      postedOn: record.posted_on,
      amountCents,
      reference: record.reference.slice(0, 140),
      source: "import",
    });
  });

  return { entries, errors };
}
