export const PORT = Number(process.env.PORT ?? 3000);

// Bearer tokens the private routes accept, comma-separated in the environment.
export const API_TOKENS = (process.env.API_TOKENS ?? "")
  .split(",")
  .map((token) => token.trim())
  .filter((token) => token.length > 0);

export const DEFAULT_PAGE_SIZE = 20;
export const MAX_PAGE_SIZE = 100;

export const FREE_SHIPPING_THRESHOLD_CENTS = 15000;
export const SHIPPING_CENTS = 850;
export const GST_RATE = 0.15;
