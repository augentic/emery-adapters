import pino from "pino";

function required(name: string): string {
  const value = process.env[name];
  if (value === undefined || value === "") {
    throw new Error(`missing required environment variable ${name}`);
  }
  return value;
}

export const PORT = Number(process.env.PORT ?? 3000);
export const DATABASE_URL = required("DATABASE_URL");
export const REDIS_URL = process.env.REDIS_URL ?? "redis://localhost:6379";
export const SHIPPING_API_URL = process.env.SHIPPING_API_URL ?? "https://rates.courier.example/v2";
export const SHIPPING_API_KEY = process.env.SHIPPING_API_KEY ?? "";
export const REQUEST_TIMEOUT_MS = Number(process.env.REQUEST_TIMEOUT_MS ?? 5000);
export const CACHE_TTL_SECONDS = Number(process.env.CACHE_TTL_SECONDS ?? 300);
export const DEFAULT_PAGE_SIZE = Number(process.env.DEFAULT_PAGE_SIZE ?? 20);
export const MAX_PAGE_SIZE = 100;
export const API_TOKENS = (process.env.API_TOKENS ?? "").split(",").filter((token) => token.length > 0);
export const SHUTDOWN_GRACE_MS = Number(process.env.SHUTDOWN_GRACE_MS ?? 10000);
export const LOG_LEVEL = process.env.LOG_LEVEL ?? "info";

export const logger = pino({ level: LOG_LEVEL, name: "express-orders" });
