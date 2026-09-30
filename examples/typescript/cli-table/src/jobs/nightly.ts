import { logger, OVERDUE_DAYS } from "../config";
import { markOverdue, openInvoicesDueBefore } from "../lib/db";
import type { JobOptions } from "./job";

// `ops nightly`: every open invoice whose due date is more than OVERDUE_DAYS
// ago is marked overdue; a dry run only reports how many would be.
export async function runNightly(_args: string[], options: JobOptions): Promise<number> {
  const cutoff = new Date();
  cutoff.setUTCDate(cutoff.getUTCDate() - OVERDUE_DAYS);
  const overdue = await openInvoicesDueBefore(cutoff.toISOString().slice(0, 10));

  if (!options.dryRun) {
    await markOverdue(overdue.map((invoice) => invoice.id));
  }

  logger.info({ overdue: overdue.length, dryRun: options.dryRun }, "nightly sweep finished");
  return 0;
}
