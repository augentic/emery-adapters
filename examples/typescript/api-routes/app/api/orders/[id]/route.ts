import { NextRequest, NextResponse } from "next/server";

import { authenticate } from "@/lib/auth";
import { HttpError, toResponse } from "@/lib/errors";
import { orders } from "@/lib/store";

type Params = { params: { id: string } };

export async function GET(request: NextRequest, { params }: Params) {
  try {
    authenticate(request);
    const order = orders.get(params.id);
    if (!order) {
      return NextResponse.json({ error: "not-found" }, { status: 404 });
    }
    return NextResponse.json(order);
  } catch (error) {
    return toResponse(error);
  }
}

export async function DELETE(request: NextRequest, { params }: Params) {
  try {
    authenticate(request);
    const order = orders.get(params.id);
    if (!order) {
      return NextResponse.json({ error: "not-found" }, { status: 404 });
    }
    if (order.state === "shipped") {
      throw new HttpError(409, "already-shipped");
    }
    orders.cancel(order.id);
    return new NextResponse(null, { status: 204 });
  } catch (error) {
    return toResponse(error);
  }
}
