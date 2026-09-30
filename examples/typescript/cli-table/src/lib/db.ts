import { Pool } from "pg";
import { DATABASE_URL } from "../config";

export const pool = new Pool({ connectionString: DATABASE_URL, max: 4 });

export interface LedgerEntry {
  id: number;
  postedOn: string;
  amountCents: number;
  reference: string;
}

export interface BankMovement {
  id: number;
  bookedOn: string;
  amountCents: number;
  narrative: string;
}

export interface Invoice {
  id: number;
  dueOn: string;
  amountCents: number;
  state: "open" | "paid" | "overdue";
}

export async function insertEntry(entry: Omit<LedgerEntry, "id">): Promise<void> {
  await pool.query("insert into ledger (posted_on, amount_cents, reference) values ($1, $2, $3)", [
    entry.postedOn,
    entry.amountCents,
    entry.reference,
  ]);
}

export async function ledgerBetween(from: string, to: string): Promise<LedgerEntry[]> {
  const result = await pool.query("select * from ledger where posted_on >= $1 and posted_on < $2", [from, to]);
  return result.rows as LedgerEntry[];
}

export async function bankBetween(from: string, to: string): Promise<BankMovement[]> {
  const result = await pool.query("select * from bank_feed where booked_on >= $1 and booked_on < $2", [from, to]);
  return result.rows as BankMovement[];
}

export async function openInvoicesDueBefore(date: string): Promise<Invoice[]> {
  const result = await pool.query("select * from invoices where state = 'open' and due_on < $1", [date]);
  return result.rows as Invoice[];
}

export async function markOverdue(ids: number[]): Promise<void> {
  if (ids.length === 0) {
    return;
  }
  await pool.query("update invoices set state = 'overdue' where id = any($1)", [ids]);
}
