import pino from "pino";

export const DATABASE_URL = process.env.DATABASE_URL ?? "postgres://localhost:5432/ledger";

// A ledger row is rejected when its amount, in cents, is past this.
export const MAX_AMOUNT_CENTS = 1_000_000_00;

// Days after the due date before an invoice is swept as overdue.
export const OVERDUE_DAYS = 30;

// A bank movement matches a ledger entry within this many cents.
export const MATCH_TOLERANCE_CENTS = 1;

export const logger = pino({ level: process.env.LOG_LEVEL ?? "info" });
