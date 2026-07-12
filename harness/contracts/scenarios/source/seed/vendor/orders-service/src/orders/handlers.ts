import { Request, Response } from "express";
import {
  CreateOrderRequest,
  ErrorResponse,
  Order,
} from "./types";
import { findOrder, persistOrder } from "./store";

export async function createOrder(req: Request, res: Response) {
  const body = req.body as CreateOrderRequest;
  if (!body.customer_id || !body.items?.length) {
    const err: ErrorResponse = {
      code: "INVALID_INPUT",
      message: "customer_id and items are required",
    };
    return res.status(400).json(err);
  }
  const order: Order = await persistOrder(body);
  return res.status(201).json(order);
}

export async function getOrder(req: Request, res: Response) {
  const order = await findOrder(req.params.orderId);
  if (!order) {
    const err: ErrorResponse = {
      code: "NOT_FOUND",
      message: "order not found",
    };
    return res.status(404).json(err);
  }
  return res.status(200).json(order);
}
