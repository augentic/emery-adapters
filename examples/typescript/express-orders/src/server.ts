import express from "express";
import { Pool } from "pg";
import Redis from "ioredis";
import { DATABASE_URL, PORT, REDIS_URL, SHUTDOWN_GRACE_MS, logger } from "@app/config";
import { authenticate } from "@app/middleware/auth";
import { errorHandler, notFound } from "@app/middleware/errors";
import { requestLog } from "@app/middleware/request-log";
import { healthRouter } from "@app/routes/health";
import { customersRouter } from "@app/routes/customers";
import { ordersRouter } from "@app/routes/orders";
import { Cache } from "@app/lib/cache";
import { ShippingClient } from "@app/lib/shipping";
import { CustomerRepository } from "@app/repositories/customers";
import { OrderRepository } from "@app/repositories/orders";
import { CustomerService } from "@app/services/customers";
import { OrderService } from "@app/services/orders";

const pool = new Pool({ connectionString: DATABASE_URL, max: 10, idleTimeoutMillis: 30000 });
const redis = new Redis(REDIS_URL, { lazyConnect: true, maxRetriesPerRequest: 2 });
const cache = new Cache(redis);
const shipping = new ShippingClient();

const customers = new CustomerService(new CustomerRepository(pool));
const orders = new OrderService(new OrderRepository(pool), customers, shipping, cache);

const app = express();
app.disable("x-powered-by");
app.use(express.json({ limit: "256kb" }));
app.use(requestLog);
app.use("/health", healthRouter(pool, redis));
app.use("/api", authenticate);
app.use("/api", customersRouter(customers));
app.use("/api", ordersRouter(orders));
app.use(notFound);
app.use(errorHandler);

async function main(): Promise<void> {
  await redis.connect();
  await pool.query("SELECT 1");
  const server = app.listen(PORT, () => {
    logger.info({ port: PORT }, "orders api listening");
  });

  const shutdown = (signal: string) => {
    logger.warn({ signal }, "shutting down");
    server.close(async () => {
      await pool.end();
      redis.disconnect();
      process.exit(0);
    });
    setTimeout(() => process.exit(1), SHUTDOWN_GRACE_MS).unref();
  };
  process.on("SIGTERM", () => shutdown("SIGTERM"));
  process.on("SIGINT", () => shutdown("SIGINT"));
}

main().catch((error: Error) => {
  logger.fatal({ err: error }, "startup failed");
  process.exit(1);
});
