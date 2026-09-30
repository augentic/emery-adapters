import { FREE_SHIPPING_THRESHOLD_CENTS, GST_RATE, SHIPPING_CENTS } from "../config";
import type { OrderLine, Tier } from "./store";

export const TIER_DISCOUNT: Record<Tier, number> = { standard: 0, silver: 0.05, gold: 0.1 };

// The total in cents: the lines less the tier's discount, shipping unless the
// discounted subtotal reaches the free-shipping threshold, GST on the lot.
export function price(lines: OrderLine[], tier: Tier): number {
  const subtotal = lines.reduce((sum, line) => sum + line.quantity * line.unitPriceCents, 0);
  const discounted = Math.round(subtotal * (1 - TIER_DISCOUNT[tier]));
  const shipping = discounted >= FREE_SHIPPING_THRESHOLD_CENTS ? 0 : SHIPPING_CENTS;
  return Math.round((discounted + shipping) * (1 + GST_RATE));
}
