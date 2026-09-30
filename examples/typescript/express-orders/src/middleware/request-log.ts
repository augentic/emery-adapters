import type { NextFunction, Request, Response } from "express";
import { logger } from "@app/config";

const SLOW_REQUEST_MS = 1000;

export function requestLog(req: Request, res: Response, next: NextFunction): void {
  const startedAt = process.hrtime.bigint();
  res.on("finish", () => {
    const elapsedMs = Number(process.hrtime.bigint() - startedAt) / 1_000_000;
    const entry = { method: req.method, path: req.path, status: res.statusCode, elapsedMs: Math.round(elapsedMs) };
    if (elapsedMs > SLOW_REQUEST_MS) {
      logger.warn(entry, "slow request");
    } else {
      logger.info(entry, "request");
    }
  });
  next();
}
