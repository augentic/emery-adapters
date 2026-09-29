import { zValidator } from "@hono/zod-validator";
import { Hono } from "hono";
import { z } from "zod";
import { DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE } from "../config";
import { price } from "../lib/pricing";
import { nextId, store } from "../lib/store";

const PlaceOrderSchema = z.object({
  customerId: z.string().min(1),
  lines: z
    .array(
      z.object({
        sku: z.string().min(1),
        quantity: z.number().int().positive(),
        unitPriceCents: z.number().int().nonnegative(),
      }),
    )
    .min(1),
});

// Mounted at /orders by the server.
export const orders = new Hono();

orders.get("/", (c) => {
  const requested = Number(c.req.query("limit") ?? DEFAULT_PAGE_SIZE);
  const limit =
    Number.isInteger(requested) && requested > 0 ? Math.min(requested, MAX_PAGE_SIZE) : DEFAULT_PAGE_SIZE;
  const state = c.req.query("state");
  const listed = store.orders.list().filter((order) => !state || order.state === state);
  return c.json({ orders: listed.slice(0, limit), total: listed.length });
});

orders.get("/:id", (c) => {
  const order = store.orders.get(c.req.param("id"));
  if (!order) {
    return c.json({ error: "not-found" }, 404);
  }
  return c.json(order);
});

orders.post("/", zValidator("json", PlaceOrderSchema), (c) => {
  const body = c.req.valid("json");
  const customer = store.customers.get(body.customerId);
  if (!customer) {
    return c.json({ error: "unknown-customer" }, 422);
  }
  const order = {
    id: nextId("ord"),
    customerId: customer.id,
    lines: body.lines,
    totalCents: price(body.lines, customer.tier),
    state: "placed" as const,
    placedAt: new Date().toISOString(),
  };
  store.orders.put(order);
  c.header("Location", `/orders/${order.id}`);
  return c.json(order, 201);
});

orders.delete("/:id", (c) => {
  const order = store.orders.get(c.req.param("id"));
  if (!order) {
    return c.json({ error: "not-found" }, 404);
  }
  if (order.state === "shipped") {
    return c.json({ error: "already-shipped" }, 409);
  }
  store.orders.put({ ...order, state: "cancelled" });
  return c.body(null, 204);
});
