// Orders service: the behaviour the `typescript` source extracts. The same
// estate `../documentation/docs/orders-api.md` specifies in prose, so the two
// examples read as one estate.

export type OrderState = "pending" | "paid" | "shipped" | "cancelled";

export interface Line {
  productId: string;
  quantity: number;
}

export interface Order {
  id: string;
  customerId: string;
  lines: Line[];
  courierNote?: string;
  state: OrderState;
  transitions: { state: OrderState; at: Date }[];
}

export interface PlaceOrder {
  customerId: string;
  lines: Line[];
  courierNote?: string;
}

export class ValidationError extends Error {}
export class NotFoundError extends Error {}
export class ConflictError extends Error {
  constructor(readonly state: OrderState) {
    super(`order is ${state}`);
  }
}

export class OrderService {
  private readonly orders = new Map<string, Order>();
  private next = 1;

  place(request: PlaceOrder): Order {
    if (request.lines.length === 0) {
      throw new ValidationError("an order needs at least one line");
    }
    if (request.lines.some((line) => line.quantity < 1)) {
      throw new ValidationError("a line quantity is at least one");
    }
    const id = `ord_${this.next++}`;
    const order: Order = {
      id,
      customerId: request.customerId,
      lines: request.lines,
      courierNote: request.courierNote,
      state: "pending",
      transitions: [{ state: "pending", at: new Date() }],
    };
    this.orders.set(id, order);
    return order;
  }

  read(id: string): Order {
    const order = this.orders.get(id);
    if (order === undefined) {
      throw new NotFoundError(`no order ${id}`);
    }
    return order;
  }

  cancel(id: string): Order {
    const order = this.read(id);
    if (order.state === "shipped") {
      throw new ConflictError(order.state);
    }
    order.state = "cancelled";
    order.transitions.push({ state: "cancelled", at: new Date() });
    return order;
  }
}
