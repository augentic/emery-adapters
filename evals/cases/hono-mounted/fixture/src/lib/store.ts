export const TIERS = ["standard", "silver", "gold"] as const;
export type Tier = (typeof TIERS)[number];

export type OrderState = "placed" | "shipped" | "cancelled";

export interface Customer {
  id: string;
  email: string;
  name: string;
  tier: Tier;
}

export interface OrderLine {
  sku: string;
  quantity: number;
  unitPriceCents: number;
}

export interface Order {
  id: string;
  customerId: string;
  lines: OrderLine[];
  totalCents: number;
  state: OrderState;
  placedAt: string;
}

const customers = new Map<string, Customer>();
const orders = new Map<string, Order>();
let sequence = 0;

export function nextId(prefix: string): string {
  sequence += 1;
  return `${prefix}-${String(sequence).padStart(6, "0")}`;
}

export const store = {
  customers: {
    list: (): Customer[] => [...customers.values()],
    get: (id: string): Customer | undefined => customers.get(id),
    byEmail: (email: string): Customer | undefined =>
      [...customers.values()].find((customer) => customer.email === email.toLowerCase()),
    put: (customer: Customer): void => {
      customers.set(customer.id, customer);
    },
  },
  orders: {
    // newest first
    list: (): Order[] => [...orders.values()].sort((a, b) => b.placedAt.localeCompare(a.placedAt)),
    get: (id: string): Order | undefined => orders.get(id),
    put: (order: Order): void => {
      orders.set(order.id, order);
    },
    count: (): number => orders.size,
  },
};
