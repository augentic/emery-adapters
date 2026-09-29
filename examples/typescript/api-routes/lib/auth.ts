import type { NextRequest } from "next/server";

import { HttpError } from "@/lib/errors";

// The bearer tokens admitted, comma-separated; unset admits no caller.
const API_TOKENS = (process.env.API_TOKENS ?? "")
  .split(",")
  .map((token) => token.trim())
  .filter((token) => token.length > 0);

export function authenticate(request: NextRequest): void {
  const header = request.headers.get("authorization") ?? "";
  const [scheme, token] = header.split(" ");
  if (scheme !== "Bearer" || !token || !API_TOKENS.includes(token)) {
    throw new HttpError(401, "unauthorized");
  }
}
