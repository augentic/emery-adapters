import { Router } from "express";
import type Redis from "ioredis";
import type { Pool } from "pg";

const PROBE_TIMEOUT_MS = 2000;

async function within<T>(promise: Promise<T>, label: string): Promise<T> {
  let timer: NodeJS.Timeout | undefined;
  const timeout = new Promise<never>((_resolve, reject) => {
    timer = setTimeout(() => reject(new Error(`${label} probe timed out`)), PROBE_TIMEOUT_MS);
  });
  try {
    return await Promise.race([promise, timeout]);
  } finally {
    clearTimeout(timer);
  }
}

export function healthRouter(pool: Pool, redis: Redis): Router {
  const router = Router();

  router.get("/live", (_req, res) => {
    res.json({ status: "ok" });
  });

  router.get("/ready", async (_req, res) => {
    const checks = await Promise.allSettled([
      within(pool.query("SELECT 1"), "postgres"),
      within(redis.ping(), "redis"),
    ]);
    const postgres = checks[0].status === "fulfilled";
    const cache = checks[1].status === "fulfilled";
    res.status(postgres ? 200 : 503).json({
      status: postgres ? "ok" : "degraded",
      postgres,
      redis: cache,
    });
  });

  return router;
}
