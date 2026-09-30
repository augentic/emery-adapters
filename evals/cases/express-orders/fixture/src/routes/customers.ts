import { Router } from "express";
import type { NextFunction, Request, Response } from "express";
import { createCustomerSchema, updateTierSchema } from "@app/schemas/customers";
import type { CustomerService } from "@app/services/customers";

type Handler = (req: Request, res: Response) => Promise<void>;

function wrap(handler: Handler) {
  return (req: Request, res: Response, next: NextFunction): void => {
    handler(req, res).catch(next);
  };
}

export function customersRouter(customers: CustomerService): Router {
  const router = Router();

  router.get(
    "/customers/:id",
    wrap(async (req, res) => {
      res.json(await customers.get(req.params.id));
    }),
  );

  router.post(
    "/customers",
    wrap(async (req, res) => {
      const request = createCustomerSchema.parse(req.body);
      const customer = await customers.create(request);
      res.status(201).location(`/api/customers/${customer.id}`).json(customer);
    }),
  );

  router.put(
    "/customers/:id/tier",
    wrap(async (req, res) => {
      const { tier } = updateTierSchema.parse(req.body);
      res.json(await customers.setTier(req.params.id, tier));
    }),
  );

  return router;
}
