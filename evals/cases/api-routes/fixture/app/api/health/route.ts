import { NextResponse } from "next/server";

import { orders } from "@/lib/store";

export const dynamic = "force-dynamic";

const STARTED_AT = Date.now();

// Unauthenticated: the platform's probe carries no token.
export function GET() {
  const uptimeSeconds = Math.floor((Date.now() - STARTED_AT) / 1000);
  return NextResponse.json(
    { status: "ok", uptimeSeconds, orders: orders.count() },
    { headers: { "Cache-Control": "no-store" } },
  );
}
