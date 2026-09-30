import { Pool } from "pg";
import { DATABASE_URL } from "../config";

export interface LedgerEntry {
  id: string;
  account: string;
  postedOn: string;
  amountCents: number;
  reference: string;
  source: "import" | "manual" | "bank";
}

export interface Invoice {
  id: string;
  customerId: string;
  amountCents: number;
  dueOn: string;
  status: "draft" | "issued" | "paid" | "overdue" | "void";
}

export const pool = new Pool({ connectionString: DATABASE_URL, max: 5 });

export async function insertEntries(entries: readonly LedgerEntry[]): Promise<number> {
  if (entries.length === 0) {
    return 0;
  }
  const client = await pool.connect();
  try {
    await client.query("BEGIN");
    let inserted = 0;
    for (const entry of entries) {
      const { rowCount } = await client.query(
        `INSERT INTO ledger (id, account, posted_on, amount_cents, reference, source)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (id) DO NOTHING`,
        [entry.id, entry.account, entry.postedOn, entry.amountCents, entry.reference, entry.source],
      );
      inserted += rowCount ?? 0;
    }
    await client.query("COMMIT");
    return inserted;
  } catch (error) {
    await client.query("ROLLBACK");
    throw error;
  } finally {
    client.release();
  }
}

export async function entriesBetween(from: string, to: string): Promise<LedgerEntry[]> {
  const { rows } = await pool.query<LedgerEntry>(
    `SELECT id, account, posted_on AS "postedOn", amount_cents AS "amountCents", reference, source
     FROM ledger WHERE posted_on >= $1 AND posted_on < $2 ORDER BY posted_on`,
    [from, to],
  );
  return rows;
}

export async function invoicesDueBefore(date: string): Promise<Invoice[]> {
  const { rows } = await pool.query<Invoice>(
    `SELECT id, customer_id AS "customerId", amount_cents AS "amountCents", due_on AS "dueOn", status
     FROM invoices WHERE status = 'issued' AND due_on < $1`,
    [date],
  );
  return rows;
}

export async function markInvoice(id: string, status: Invoice["status"]): Promise<void> {
  await pool.query("UPDATE invoices SET status = $2, updated_at = now() WHERE id = $1", [id, status]);
}
