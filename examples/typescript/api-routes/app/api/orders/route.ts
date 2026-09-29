import { NextRequest, NextResponse } from "next/server";
import { z } from "zod";

import { authenticate } from "@/lib/auth";
import { HttpError, toResponse } from "@/lib/errors";
import { priceOrder } from "@/lib/pricing";
import { customers, orders } from "@/lib/store";

export const dynamic = "force-dynamic";

const DEFAULT_PAGE_SIZE = 20;
const MAX_PAGE_SIZE = 100;

const LineSchema = z.object({
  sku: z.string().min(1),
  quantity: z.number().int().positive(),
  unitPriceCents: z.number().int().nonnegative(),
});

const PlaceOrderSchema = z.object({
  customerId: z.string().uuid(),
  lines: z.array(LineSchema).min(1),
  promoCode: z.string().optional(),
});

export async function GET(request: NextRequest) {
  try {
    authenticate(request);
    const params = request.nextUrl.searchParams;
    const requested = Number(params.get("limit") ?? DEFAULT_PAGE_SIZE);
    const limit =
      Number.isFinite(requested) && requested > 0 ? Math.min(requested, MAX_PAGE_SIZE) : DEFAULT_PAGE_SIZE;
    const page = orders.list({ state: params.get("state"), limit });
    return NextResponse.json({ items: page, count: page.length });
  } catch (error) {
    return toResponse(error);
  }
}

export async function POST(request: NextRequest) {
  try {
    authenticate(request);
    const parsed = PlaceOrderSchema.safeParse(await request.json());
    if (!parsed.success) {
      return NextResponse.json({ error: "invalid-order", issues: parsed.error.issues }, { status: 400 });
    }
    const customer = customers.get(parsed.data.customerId);
    if (!customer) {
      throw new HttpError(422, "unknown-customer");
    }
    const totals = priceOrder(parsed.data.lines, customer.tier, parsed.data.promoCode);
    const order = orders.place({ customerId: customer.id, lines: parsed.data.lines, ...totals });
    return NextResponse.json(order, { status: 201, headers: { Location: `/api/orders/${order.id}` } });
  } catch (error) {
    return toResponse(error);
  }
}
