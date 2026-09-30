import type { Context, Next } from "hono";
import { API_TOKENS } from "../config";

// Every private route is behind a bearer token from API_TOKENS.
export async function bearer(c: Context, next: Next): Promise<Response | void> {
  const header = c.req.header("authorization") ?? "";
  const token = header.startsWith("Bearer ") ? header.slice("Bearer ".length).trim() : "";
  if (token.length === 0 || !API_TOKENS.includes(token)) {
    return c.json({ error: "unauthorized" }, 401);
  }
  await next();
}
