import { randomUUID } from "node:crypto";
import { logger } from "@app/config";
import type { Order, OrderLine, OrderState, Page } from "@app/domain/order";
import { TERMINAL_STATES, TRANSITIONS } from "@app/domain/order";
import type { Cache } from "@app/lib/cache";
import { ConflictError, NotFoundError, ValidationError } from "@app/lib/errors";
import type { ShippingClient } from "@app/lib/shipping";
import type { OrderRepository } from "@app/repositories/orders";
import type { AmendLines, ListQuery, PlaceOrder } from "@app/schemas/orders";
import type { CustomerService } from "@app/services/customers";
import { totals, weight } from "@app/services/pricing";

const ORDER_CACHE_TTL_SECONDS = 60;

export class OrderService {
  constructor(
    private readonly repository: OrderRepository,
    private readonly customers: CustomerService,
    private readonly shipping: ShippingClient,
    private readonly cache: Cache,
  ) {}

  async get(id: string): Promise<Order> {
    const cached = await this.cache.get<Order>(`order:${id}`);
    if (cached !== undefined) {
      return cached;
    }
    const order = await this.repository.find(id);
    if (order === undefined) {
      throw new NotFoundError("order", id);
    }
    await this.cache.set(`order:${id}`, order, ORDER_CACHE_TTL_SECONDS);
    return order;
  }

  list(query: ListQuery): Promise<Page<Order>> {
    return this.repository.list(query);
  }

  async place(request: PlaceOrder): Promise<Order> {
    const customer = await this.customers.get(request.customerId);
    const lines = await this.priceLines(request.lines);
    const quote = await this.shipping.quote(request.shipTo.postcode, weight(lines));
    const now = new Date();
    const order: Order = {
      id: randomUUID(),
      customerId: customer.id,
      lines,
      shipTo: request.shipTo,
      courierNote: request.courierNote,
      state: "pending",
      ...totals(lines, customer.tier, quote),
      transitions: [{ state: "pending", at: now }],
      createdAt: now,
      updatedAt: now,
    };
    await this.repository.insert(order);
    logger.info({ orderId: order.id, customerId: customer.id, totalCents: order.totalCents }, "order placed");
    return order;
  }

  async amend(id: string, request: AmendLines): Promise<Order> {
    const order = await this.get(id);
    if (order.state !== "pending") {
      throw new ConflictError(order.state, `order ${id} is ${order.state} and its lines are fixed`);
    }
    const customer = await this.customers.get(order.customerId);
    const lines = await this.priceLines(request.lines);
    const quote = await this.shipping.quote(order.shipTo.postcode, weight(lines));
    const amended: Order = {
      ...order,
      lines,
      ...totals(lines, customer.tier, quote),
      updatedAt: new Date(),
    };
    await this.repository.update(amended);
    await this.cache.invalidate(`order:${id}`);
    return amended;
  }

  async transition(id: string, to: OrderState, note?: string): Promise<Order> {
    const order = await this.get(id);
    if (TERMINAL_STATES.includes(order.state)) {
      throw new ConflictError(order.state, `order ${id} is already ${order.state}`);
    }
    if (!TRANSITIONS[order.state].includes(to)) {
      throw new ConflictError(order.state, `order ${id} cannot move from ${order.state} to ${to}`);
    }
    const at = new Date();
    const moved: Order = {
      ...order,
      state: to,
      transitions: [...order.transitions, { state: to, at, note }],
      updatedAt: at,
    };
    await this.repository.update(moved);
    await this.cache.invalidate(`order:${id}`);
    logger.info({ orderId: id, from: order.state, to }, "order transitioned");
    return moved;
  }

  private async priceLines(requested: PlaceOrder["lines"]): Promise<OrderLine[]> {
    const skus = [...new Set(requested.map((line) => line.sku))];
    const catalogue = await this.repository.catalogue(skus);
    const bySku = new Map(catalogue.map((item) => [item.sku, item]));
    const lines: OrderLine[] = [];
    for (const line of requested) {
      const item = bySku.get(line.sku);
      if (item === undefined) {
        throw new ValidationError(`unknown sku ${line.sku}`);
      }
      if (!item.active) {
        throw new ValidationError(`sku ${line.sku} is discontinued`);
      }
      const existing = lines.find((priced) => priced.sku === line.sku);
      if (existing !== undefined) {
        existing.quantity += line.quantity;
        continue;
      }
      lines.push({
        sku: item.sku,
        description: item.description,
        quantity: line.quantity,
        unitCents: item.unit_cents,
        weightGrams: item.weight_grams,
      });
    }
    return lines;
  }
}
