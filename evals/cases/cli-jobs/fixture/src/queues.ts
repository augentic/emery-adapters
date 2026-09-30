import { Queue } from "bullmq";
import Redis from "ioredis";
import { INVOICE_ATTEMPTS, REDIS_URL } from "./config";

export const INVOICE_QUEUE = "invoices";
export const INVOICE_BACKOFF_MS = 30000;

export interface InvoiceJob {
  invoiceId: string;
  customerId: string;
  amountCents: number;
}

export const connection = new Redis(REDIS_URL, { maxRetriesPerRequest: null });

export const invoiceQueue = new Queue<InvoiceJob>(INVOICE_QUEUE, {
  connection,
  defaultJobOptions: {
    attempts: INVOICE_ATTEMPTS,
    backoff: { type: "exponential", delay: INVOICE_BACKOFF_MS },
    removeOnComplete: 1000,
    removeOnFail: false,
  },
});
