import { logger, MATCH_TOLERANCE_CENTS } from "../config";
import { bankBetween, ledgerBetween, type BankMovement, type LedgerEntry } from "../lib/db";
import type { JobOptions } from "./job";

export interface Reconciliation {
  matched: number;
  unmatchedLedger: LedgerEntry[];
  unmatchedBank: BankMovement[];
}

// `ops reconcile <from> <to>`: each ledger entry in the range is matched to
// the first bank movement of the same amount, within the tolerance, not yet
// taken; what is left on either side is reported and sets the exit code.
export async function reconcile(args: string[], options: JobOptions): Promise<number> {
  const [from, to] = args;
  if (!from || !to) {
    logger.error("reconcile needs <from> and <to>");
    return 64;
  }
  const result = await match(await ledgerBetween(from, to), await bankBetween(from, to));

  logger.info(
    {
      matched: result.matched,
      unmatchedLedger: result.unmatchedLedger.length,
      unmatchedBank: result.unmatchedBank.length,
      dryRun: options.dryRun,
    },
    "reconciliation finished",
  );
  return result.unmatchedLedger.length > 0 || result.unmatchedBank.length > 0 ? 3 : 0;
}

export async function match(ledger: LedgerEntry[], bank: BankMovement[]): Promise<Reconciliation> {
  const taken = new Set<number>();
  const unmatchedLedger: LedgerEntry[] = [];
  let matched = 0;

  for (const entry of ledger) {
    const movement = bank.find(
      (candidate) =>
        !taken.has(candidate.id) && Math.abs(candidate.amountCents - entry.amountCents) <= MATCH_TOLERANCE_CENTS,
    );
    if (movement) {
      taken.add(movement.id);
      matched += 1;
    } else {
      unmatchedLedger.push(entry);
    }
  }

  return { matched, unmatchedLedger, unmatchedBank: bank.filter((movement) => !taken.has(movement.id)) };
}
