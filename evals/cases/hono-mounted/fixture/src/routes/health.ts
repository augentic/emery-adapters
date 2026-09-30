import { Hono } from "hono";
import { store } from "../lib/store";

const started = Date.now();

// Mounted at /health by the server, in front of the bearer check.
export const health = new Hono();

health.get("/", (c) => {
  c.header("Cache-Control", "no-store");
  return c.json({
    status: "ok",
    uptimeSeconds: Math.floor((Date.now() - started) / 1000),
    orders: store.orders.count(),
  });
});
