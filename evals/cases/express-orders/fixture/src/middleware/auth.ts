import type { NextFunction, Request, Response } from "express";
import { API_TOKENS } from "@app/config";
import { UnauthorizedError } from "@app/lib/errors";

const BEARER_PREFIX = "Bearer ";

export function authenticate(req: Request, _res: Response, next: NextFunction): void {
  const header = req.header("authorization");
  if (header === undefined || !header.startsWith(BEARER_PREFIX)) {
    next(new UnauthorizedError("missing bearer token"));
    return;
  }
  const token = header.slice(BEARER_PREFIX.length).trim();
  if (!API_TOKENS.includes(token)) {
    next(new UnauthorizedError("unknown token"));
    return;
  }
  next();
}
