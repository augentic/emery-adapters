import { Worker } from "bullmq";
import type { Job } from "bullmq";
import { INVOICE_CONCURRENCY, logger } from "../config";
import { markInvoice } from "../lib/db";
import { INVOICE_QUEUE, connection } from "../queues";
import type { InvoiceJob } from "../queues";

const REMINDER_SUBJECT = "Invoice overdue";
const MAX_REMINDER_AMOUNT_CENTS = 5_000_000;

async function sendReminder(job: Job<InvoiceJob>): Promise<void> {
  if (job.data.amountCents > MAX_REMINDER_AMOUNT_CENTS) {
    logger.warn({ invoiceId: job.data.invoiceId, amountCents: job.data.amountCents }, "amount above the reminder ceiling; routing to collections");
    await markInvoice(job.data.invoiceId, "void");
    return;
  }
  const response = await fetch(`${process.env.MAIL_API_URL ?? "https://mail.example/v1"}/send`, {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${process.env.MAIL_API_TOKEN ?? ""}` },
    body: JSON.stringify({
      to: job.data.customerId,
      subject: REMINDER_SUBJECT,
      template: "invoice-overdue",
      variables: { invoiceId: job.data.invoiceId, amount: (job.data.amountCents / 100).toFixed(2) },
    }),
  });
  if (!response.ok) {
    throw new Error(`mail api answered ${response.status}`);
  }
}

export function startInvoiceWorker(): Worker<InvoiceJob> {
  const worker = new Worker<InvoiceJob>(
    INVOICE_QUEUE,
    async (job) => {
      switch (job.name) {
        case "remind":
          await sendReminder(job);
          break;
        case "void":
          await markInvoice(job.data.invoiceId, "void");
          break;
        default:
          throw new Error(`unknown invoice job ${job.name}`);
      }
    },
    { connection, concurrency: INVOICE_CONCURRENCY },
  );

  worker.on("failed", (job, error) => {
    logger.error({ jobId: job?.id, attempts: job?.attemptsMade, err: error }, "invoice job failed");
  });
  worker.on("completed", (job) => {
    logger.info({ jobId: job.id, name: job.name }, "invoice job completed");
  });
  return worker;
}
