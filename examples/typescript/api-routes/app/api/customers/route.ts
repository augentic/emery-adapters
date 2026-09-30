import { NextRequest, NextResponse } from "next/server";
import { z } from "zod";

import { authenticate } from "@/lib/auth";
import { HttpError, toResponse } from "@/lib/errors";
import { TIERS, customers } from "@/lib/store";

const CustomerSchema = z.object({
  email: z.string().email(),
  name: z.string().min(1).max(120),
  tier: z.enum(TIERS).default("standard"),
});

export async function GET(request: NextRequest) {
  try {
    authenticate(request);
    const email = request.nextUrl.searchParams.get("email");
    if (email) {
      const found = customers.byEmail(email.toLowerCase());
      return NextResponse.json(found ? [found] : []);
    }
    return NextResponse.json(customers.list());
  } catch (error) {
    return toResponse(error);
  }
}

export async function POST(request: NextRequest) {
  try {
    authenticate(request);
    const parsed = CustomerSchema.safeParse(await request.json());
    if (!parsed.success) {
      return NextResponse.json({ error: "invalid-customer", issues: parsed.error.issues }, { status: 400 });
    }
    const email = parsed.data.email.toLowerCase();
    if (customers.byEmail(email)) {
      throw new HttpError(409, "duplicate-email");
    }
    const customer = customers.create({ ...parsed.data, email });
    return NextResponse.json(customer, { status: 201 });
  } catch (error) {
    return toResponse(error);
  }
}
