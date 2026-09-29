import { NextResponse } from "next/server";

export class HttpError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string,
  ) {
    super(code);
  }
}

// Every route handler's catch: an HttpError answers with its status and code,
// a body that is not JSON is the caller's fault, anything else is logged and
// hidden behind a 500.
export function toResponse(error: unknown): NextResponse {
  if (error instanceof HttpError) {
    return NextResponse.json({ error: error.code }, { status: error.status });
  }
  if (error instanceof SyntaxError) {
    return NextResponse.json({ error: "malformed-json" }, { status: 400 });
  }
  console.error("unhandled", error);
  return NextResponse.json({ error: "internal" }, { status: 500 });
}
