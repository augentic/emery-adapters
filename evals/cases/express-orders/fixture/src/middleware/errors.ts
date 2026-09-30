import type { NextFunction, Request, Response } from "express";
import { ZodError } from "zod";
import { logger } from "@app/config";
import { ConflictError, NotFoundError, UnauthorizedError, UpstreamError, ValidationError } from "@app/lib/errors";

export function notFound(req: Request, res: Response): void {
  res.status(404).json({ error: "not-found", path: req.path });
}

export function errorHandler(error: unknown, _req: Request, res: Response, _next: NextFunction): void {
  if (error instanceof ZodError) {
    res.status(400).json({ error: "invalid-request", issues: error.issues.map((issue) => ({ path: issue.path.join("."), message: issue.message })) });
    return;
  }
  if (error instanceof ValidationError) {
    res.status(400).json({ error: "invalid-request", message: error.message });
    return;
  }
  if (error instanceof UnauthorizedError) {
    res.status(401).json({ error: "unauthorized" });
    return;
  }
  if (error instanceof NotFoundError) {
    res.status(404).json({ error: "not-found", message: error.message });
    return;
  }
  if (error instanceof ConflictError) {
    res.status(409).json({ error: "conflict", message: error.message, state: error.state });
    return;
  }
  if (error instanceof UpstreamError) {
    logger.warn({ err: error, upstream: error.upstream }, "upstream failure");
    res.status(502).json({ error: "upstream-unavailable", upstream: error.upstream });
    return;
  }
  logger.error({ err: error }, "unhandled error");
  res.status(500).json({ error: "internal" });
}
