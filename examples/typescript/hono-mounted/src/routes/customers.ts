import { zValidator } from "@hono/zod-validator";
import { Hono } from "hono";
import { z } from "zod";
import { nextId, store, TIERS } from "../lib/store";

const CustomerSchema = z.object({
  email: z.string().email(),
  name: z.string().min(1).max(120),
  tier: z.enum(TIERS).default("standard"),
});

// Mounted at /customers by the server.
export const customers = new Hono();

customers.get("/", (c) => {
  const email = c.req.query("email");
  if (email) {
    const found = store.customers.byEmail(email);
    return c.json({ customers: found ? [found] : [] });
  }
  return c.json({ customers: store.customers.list() });
});

customers.get("/:id", (c) => {
  const customer = store.customers.get(c.req.param("id"));
  if (!customer) {
    return c.json({ error: "not-found" }, 404);
  }
  return c.json(customer);
});

customers.post("/", zValidator("json", CustomerSchema), (c) => {
  const body = c.req.valid("json");
  const email = body.email.toLowerCase();
  if (store.customers.byEmail(email)) {
    return c.json({ error: "duplicate-email" }, 409);
  }
  const customer = { id: nextId("cus"), email, name: body.name, tier: body.tier };
  store.customers.put(customer);
  return c.json(customer, 201);
});
