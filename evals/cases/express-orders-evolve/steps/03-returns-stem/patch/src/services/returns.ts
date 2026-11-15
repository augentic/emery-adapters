import { randomUUID } from "node:crypto";
import type { Order } from "@app/domain/order";
import type { OrderReturn } from "@app/domain/returns";
import { ConflictError, NotFoundError } from "@app/lib/errors";
import type { ReturnRepository } from "@app/repositories/returns";
import type { CreateReturn } from "@app/schemas/returns";
import type { OrderService } from "@app/services/orders";

export class ReturnService {
  constructor(
    private readonly repository: ReturnRepository,
    private readonly orders: OrderService,
  ) {}

  async get(id: string): Promise<OrderReturn> {
    const entry = await this.repository.find(id);
    if (entry === undefined) {
      throw new NotFoundError("return", id);
    }
    return entry;
  }

  async create(request: CreateReturn): Promise<OrderReturn> {
    const order: Order = await this.orders.get(request.orderId);
    if (order.state !== "shipped") {
      throw new ConflictError(order.state, `order ${order.id} is ${order.state} and cannot be returned`);
    }
    const created: OrderReturn = {
      id: randomUUID(),
      orderId: order.id,
      reason: request.reason.trim(),
      createdAt: new Date(),
    };
    await this.repository.insert(created);
    return created;
  }
}
