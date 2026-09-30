import cron from "node-cron";
import { NIGHTLY_CRON, TIMEZONE, logger } from "../config";
import { invoicesDueBefore, markInvoice } from "../lib/db";
import { invoiceQueue } from "../queues";
import { reconcile } from "../services/reconcile";

const OVERDUE_REMINDER_PRIORITY = 2;

function isoDate(date: Date): string {
  return date.toISOString().slice(0, 10);
}

export async function runNightly(now: Date = new Date()): Promise<void> {
  const today = isoDate(now);
  const yesterday = isoDate(new Date(now.getTime() - 24 * 60 * 60 * 1000));

  const result = await reconcile(yesterday, today, true);
  if (result.unmatchedLedger.length > 0) {
    logger.warn({ count: result.unmatchedLedger.length, from: yesterday }, "ledger entries without a bank movement");
  }

  const overdue = await invoicesDueBefore(today);
  for (const invoice of overdue) {
    await markInvoice(invoice.id, "overdue");
    await invoiceQueue.add(
      "remind",
      { invoiceId: invoice.id, customerId: invoice.customerId, amountCents: invoice.amountCents },
      { jobId: `remind-${invoice.id}-${today}`, priority: OVERDUE_REMINDER_PRIORITY },
    );
  }
  logger.info({ overdue: overdue.length, matched: result.matched }, "nightly run complete");
}

export function scheduleNightly(): void {
  cron.schedule(
    NIGHTLY_CRON,
    async () => {
      try {
        await runNightly();
      } catch (error) {
        logger.error({ err: error }, "nightly run failed");
      }
    },
    { timezone: TIMEZONE },
  );
  logger.info({ cron: NIGHTLY_CRON, timezone: TIMEZONE }, "nightly job scheduled");
}
