export type OrderState = "pending" | "paid" | "shipped" | "cancelled";

export interface OrderLine {
  sku: string;
  description: string;
  quantity: number;
  unitCents: number;
  weightGrams: number;
}

export interface Address {
  line1: string;
  line2?: string;
  suburb: string;
  city: string;
  postcode: string;
}

export interface Transition {
  state: OrderState;
  at: Date;
  note?: string;
}

export interface Order {
  id: string;
  customerId: string;
  lines: OrderLine[];
  shipTo: Address;
  courierNote?: string;
  state: OrderState;
  subtotalCents: number;
  discountCents: number;
  shippingCents: number;
  gstCents: number;
  totalCents: number;
  transitions: Transition[];
  createdAt: Date;
  updatedAt: Date;
}

export interface Customer {
  id: string;
  email: string;
  name: string;
  tier: CustomerTier;
  createdAt: Date;
}

export type CustomerTier = "standard" | "trade" | "vip";

export interface Page<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
}

export const TERMINAL_STATES: readonly OrderState[] = ["shipped", "cancelled"];

export const TRANSITIONS: Readonly<Record<OrderState, readonly OrderState[]>> = {
  pending: ["paid", "cancelled"],
  paid: ["shipped", "cancelled"],
  shipped: [],
  cancelled: [],
};
