import type { CustomerTier, OrderLine } from "@app/domain/order";
import type { ShippingQuote } from "@app/lib/shipping";

export const GST_RATE = 0.15;
export const FREE_SHIPPING_THRESHOLD_CENTS = 10000;
export const TRADE_DISCOUNT_RATE = 0.1;
export const VIP_DISCOUNT_RATE = 0.15;
export const BULK_DISCOUNT_MIN_UNITS = 24;
export const BULK_DISCOUNT_RATE = 0.05;

export interface Totals {
  subtotalCents: number;
  discountCents: number;
  shippingCents: number;
  gstCents: number;
  totalCents: number;
}

export function subtotal(lines: readonly OrderLine[]): number {
  return lines.reduce((sum, line) => sum + line.unitCents * line.quantity, 0);
}

export function weight(lines: readonly OrderLine[]): number {
  return lines.reduce((sum, line) => sum + line.weightGrams * line.quantity, 0);
}

export function discountRate(tier: CustomerTier, units: number): number {
  const tierRate = tier === "vip" ? VIP_DISCOUNT_RATE : tier === "trade" ? TRADE_DISCOUNT_RATE : 0;
  const bulkRate = units >= BULK_DISCOUNT_MIN_UNITS ? BULK_DISCOUNT_RATE : 0;
  return Math.max(tierRate, bulkRate);
}

export function totals(lines: readonly OrderLine[], tier: CustomerTier, quote: ShippingQuote): Totals {
  const subtotalCents = subtotal(lines);
  const units = lines.reduce((sum, line) => sum + line.quantity, 0);
  const discountCents = Math.round(subtotalCents * discountRate(tier, units));
  const discounted = subtotalCents - discountCents;
  const shippingCents = discounted >= FREE_SHIPPING_THRESHOLD_CENTS ? 0 : quote.cents;
  const gstCents = Math.round((discounted + shippingCents) * GST_RATE);
  return {
    subtotalCents,
    discountCents,
    shippingCents,
    gstCents,
    totalCents: discounted + shippingCents + gstCents,
  };
}
