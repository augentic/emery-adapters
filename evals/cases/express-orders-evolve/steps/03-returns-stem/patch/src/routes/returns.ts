import { Router } from "express";
import type { NextFunction, Request, Response } from "express";
import { createReturnSchema } from "@app/schemas/returns";
import type { ReturnService } from "@app/services/returns";

type Handler = (req: Request, res: Response) => Promise<void>;

function wrap(handler: Handler) {
  return (req: Request, res: Response, next: NextFunction): void => {
    handler(req, res).catch(next);
  };
}

export function returnsRouter(returns: ReturnService): Router {
  const router = Router();

  router.get(
    "/returns/:id",
    wrap(async (req, res) => {
      res.json(await returns.get(req.params.id));
    }),
  );

  router.post(
    "/returns",
    wrap(async (req, res) => {
      const request = createReturnSchema.parse(req.body);
      const created = await returns.create(request);
      res.status(201).location(`/api/returns/${created.id}`).json(created);
    }),
  );

  return router;
}
