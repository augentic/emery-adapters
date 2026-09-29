import pino from "pino";

export const DATABASE_URL = process.env.DATABASE_URL ?? "postgres://ledger:ledger@localhost:5432/ledger";
export const REDIS_URL = process.env.REDIS_URL ?? "redis://localhost:6379";
export const BANK_FEED_URL = process.env.BANK_FEED_URL ?? "https://feeds.bank.example/v1";
export const BANK_FEED_TOKEN = process.env.BANK_FEED_TOKEN ?? "";
export const NIGHTLY_CRON = process.env.NIGHTLY_CRON ?? "0 2 * * *";
export const TIMEZONE = process.env.TZ ?? "Pacific/Auckland";
export const INVOICE_CONCURRENCY = Number(process.env.INVOICE_CONCURRENCY ?? 4);
export const INVOICE_ATTEMPTS = Number(process.env.INVOICE_ATTEMPTS ?? 5);
export const RECONCILE_TOLERANCE_CENTS = Number(process.env.RECONCILE_TOLERANCE_CENTS ?? 100);
export const FEED_TIMEOUT_MS = Number(process.env.FEED_TIMEOUT_MS ?? 15000);

export const logger = pino({ level: process.env.LOG_LEVEL ?? "info", name: "ledger-tool" });
