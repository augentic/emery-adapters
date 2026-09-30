import { randomUUID } from "node:crypto";

export const TIERS = ["standard", "silver", "gold"] as const;
export type Tier = (typeof TIERS)[number];
export type OrderState = "placed" | "shipped" | "cancelled";

export interface Customer {
  id: string;
  email: string;
  name: string;
  tier: Tier;
  createdAt: string;
}

export interface Line {
  sku: string;
  quantity: number;
  unitPriceCents: number;
}

export interface Order {
  id: string;
  customerId: string;
  lines: Line[];
  state: OrderState;
  subtotalCents: number;
  discountCents: number;
  shippingCents: number;
  taxCents: number;
  totalCents: number;
  placedAt: string;
}

// In-memory tables: the deployment is one long-lived Node server, so the maps
// persist across requests within a process and reset when it restarts.
const customerTable = new Map<string, Customer>();
const orderTable = new Map<string, Order>();

export const customers = {
  list: (): Customer[] => [...customerTable.values()],
  get: (id: string): Customer | undefined => customerTable.get(id),
  byEmail: (email: string): Customer | undefined =>
    [...customerTable.values()].find((customer) => customer.email === email),
  create(input: Omit<Customer, "id" | "createdAt">): Customer {
    const customer: Customer = { id: randomUUID(), createdAt: new Date().toISOString(), ...input };
    customerTable.set(customer.id, customer);
    return customer;
  },
};

export const orders = {
  count: (): number => orderTable.size,
  get: (id: string): Order | undefined => orderTable.get(id),
  list({ state, limit }: { state: string | null; limit: number }): Order[] {
    const newestFirst = [...orderTable.values()].sort((a, b) => b.placedAt.localeCompare(a.placedAt));
    const filtered = state ? newestFirst.filter((order) => order.state === state) : newestFirst;
    return filtered.slice(0, limit);
  },
  place(input: Omit<Order, "id" | "state" | "placedAt">): Order {
    const order: Order = { id: randomUUID(), state: "placed", placedAt: new Date().toISOString(), ...input };
    orderTable.set(order.id, order);
    return order;
  },
  cancel(id: string): void {
    const order = orderTable.get(id);
    if (order) {
      orderTable.set(id, { ...order, state: "cancelled" });
    }
  },
};
