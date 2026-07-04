export type OrderStatus = "pending" | "shipped" | "delivered" | "cancelled";

export interface OrderItem {
  sku: string;
  quantity: number;
}

export interface CreateOrderRequest {
  customer_id: string;
  items: OrderItem[];
}

export interface Order {
  id: string;
  customer_id: string;
  status: OrderStatus;
  items: OrderItem[];
  created_at: string;
}

export interface ErrorResponse {
  code: string;
  message: string;
}
