import type { Line, Tier } from "@/lib/store";

export const TIER_DISCOUNT: Record<Tier, number> = { standard: 0, silver: 0.05, gold: 0.1 };
export const PROMO_CODES = ["LAUNCH15", "WINTER15"];
export const PROMO_DISCOUNT = 0.15;
export const FREE_SHIPPING_THRESHOLD_CENTS = 15000;
export const SHIPPING_CENTS = 850;
export const GST_RATE = 0.15;

export interface Totals {
  subtotalCents: number;
  discountCents: number;
  shippingCents: number;
  taxCents: number;
  totalCents: number;
}

// Prices the lines for a customer: one discount — the greater of the tier's
// and the promo's, never both — then shipping, then GST on what is charged.
export function priceOrder(lines: Line[], tier: Tier, promoCode?: string): Totals {
  const subtotalCents = lines.reduce((sum, line) => sum + line.quantity * line.unitPriceCents, 0);
  const promoRate = promoCode && PROMO_CODES.includes(promoCode.toUpperCase()) ? PROMO_DISCOUNT : 0;
  const rate = Math.max(TIER_DISCOUNT[tier], promoRate);
  const discountCents = Math.round(subtotalCents * rate);
  const discounted = subtotalCents - discountCents;
  const shippingCents = discounted >= FREE_SHIPPING_THRESHOLD_CENTS ? 0 : SHIPPING_CENTS;
  const taxCents = Math.round((discounted + shippingCents) * GST_RATE);
  return { subtotalCents, discountCents, shippingCents, taxCents, totalCents: discounted + shippingCents + taxCents };
}
