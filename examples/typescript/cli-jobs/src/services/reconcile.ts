import { BANK_FEED_TOKEN, BANK_FEED_URL, FEED_TIMEOUT_MS, RECONCILE_TOLERANCE_CENTS, logger } from "../config";
import { entriesBetween, insertEntries } from "../lib/db";
import type { LedgerEntry } from "../lib/db";

interface BankTransaction {
  id: string;
  account: string;
  date: string;
  amount_cents: number;
  narrative: string;
}

export interface Reconciliation {
  from: string;
  to: string;
  matched: number;
  unmatchedLedger: LedgerEntry[];
  unmatchedBank: BankTransaction[];
  imported: number;
}

async function fetchBankTransactions(from: string, to: string): Promise<BankTransaction[]> {
  const url = `${BANK_FEED_URL}/transactions?from=${from}&to=${to}`;
  const response = await fetch(url, {
    headers: { authorization: `Bearer ${BANK_FEED_TOKEN}` },
    signal: AbortSignal.timeout(FEED_TIMEOUT_MS),
  });
  if (!response.ok) {
    throw new Error(`bank feed answered ${response.status} for ${url}`);
  }
  const body = (await response.json()) as { transactions: BankTransaction[] };
  return body.transactions;
}

function sameMovement(entry: LedgerEntry, transaction: BankTransaction): boolean {
  return (
    entry.account === transaction.account &&
    entry.postedOn === transaction.date &&
    Math.abs(entry.amountCents - transaction.amount_cents) <= RECONCILE_TOLERANCE_CENTS
  );
}

export async function reconcile(from: string, to: string, importUnmatched: boolean): Promise<Reconciliation> {
  const [ledger, bank] = await Promise.all([entriesBetween(from, to), fetchBankTransactions(from, to)]);
  const unmatchedBank = [...bank];
  const unmatchedLedger: LedgerEntry[] = [];
  let matched = 0;

  for (const entry of ledger) {
    const index = unmatchedBank.findIndex((transaction) => sameMovement(entry, transaction));
    if (index === -1) {
      unmatchedLedger.push(entry);
      continue;
    }
    unmatchedBank.splice(index, 1);
    matched += 1;
  }

  let imported = 0;
  if (importUnmatched && unmatchedBank.length > 0) {
    imported = await insertEntries(
      unmatchedBank.map((transaction) => ({
        id: `bank-${transaction.id}`,
        account: transaction.account,
        postedOn: transaction.date,
        amountCents: transaction.amount_cents,
        reference: transaction.narrative.slice(0, 140),
        source: "bank",
      })),
    );
  }

  logger.info({ from, to, matched, unmatchedLedger: unmatchedLedger.length, unmatchedBank: unmatchedBank.length, imported }, "reconciled");
  return { from, to, matched, unmatchedLedger, unmatchedBank, imported };
}
