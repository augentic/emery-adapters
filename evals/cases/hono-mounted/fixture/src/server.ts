import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { logger } from "hono/logger";
import { PORT } from "./config";
import { bearer } from "./lib/auth";
import { customers } from "./routes/customers";
import { health } from "./routes/health";
import { orders } from "./routes/orders";

const app = new Hono();

app.use(logger());

// Each resource is a sub-application mounted under its own prefix; the
// routes inside are registered at "/" and "/:id" relative to it.
app.route("/health", health);
app.use("/orders/*", bearer);
app.use("/customers/*", bearer);
app.route("/orders", orders);
app.route("/customers", customers);

const server = serve({ fetch: app.fetch, port: PORT });
console.log(`hono-mounted listening on ${PORT}`);

const shutdown = (): void => {
  server.close();
  process.exit(0);
};
process.on("SIGTERM", shutdown);
process.on("SIGINT", shutdown);
