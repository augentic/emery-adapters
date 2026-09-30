import { Router } from "express";
import type { NextFunction, Request, Response } from "express";
import type { OrderService } from "@app/services/orders";
import { amendLinesSchema, cancelSchema, listQuerySchema, placeOrderSchema } from "@app/schemas/orders";

type Handler = (req: Request, res: Response) => Promise<void>;

function wrap(handler: Handler) {
  return (req: Request, res: Response, next: NextFunction): void => {
    handler(req, res).catch(next);
  };
}

export function ordersRouter(orders: OrderService): Router {
  const router = Router();

  router.get(
    "/orders",
    wrap(async (req, res) => {
      const query = listQuerySchema.parse(req.query);
      const page = await orders.list(query);
      res.setHeader("x-total-count", String(page.total));
      res.json(page);
    }),
  );

  router.get(
    "/orders/:id",
    wrap(async (req, res) => {
      const order = await orders.get(req.params.id);
      res.json(order);
    }),
  );

  router.post(
    "/orders",
    wrap(async (req, res) => {
      const request = placeOrderSchema.parse(req.body);
      const order = await orders.place(request);
      res.status(201).location(`/api/orders/${order.id}`).json(order);
    }),
  );

  router.put(
    "/orders/:id/lines",
    wrap(async (req, res) => {
      const request = amendLinesSchema.parse(req.body);
      const order = await orders.amend(req.params.id, request);
      res.json(order);
    }),
  );

  router.post(
    "/orders/:id/pay",
    wrap(async (req, res) => {
      const order = await orders.transition(req.params.id, "paid");
      res.json(order);
    }),
  );

  router.post(
    "/orders/:id/ship",
    wrap(async (req, res) => {
      const order = await orders.transition(req.params.id, "shipped");
      res.json(order);
    }),
  );

  router.post(
    "/orders/:id/cancel",
    wrap(async (req, res) => {
      const { reason } = cancelSchema.parse(req.body);
      const order = await orders.transition(req.params.id, "cancelled", reason);
      res.json(order);
    }),
  );

  return router;
}
