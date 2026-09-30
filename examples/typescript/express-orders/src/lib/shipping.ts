import { REQUEST_TIMEOUT_MS, SHIPPING_API_KEY, SHIPPING_API_URL, logger } from "@app/config";
import { UpstreamError } from "@app/lib/errors";
import { zoneFor } from "@app/data/postcodes";

export interface ShippingQuote {
  courier: string;
  service: string;
  cents: number;
  businessDays: number;
}

interface RatesResponse {
  quotes: { courier: string; service: string; price_cents: number; eta_days: number }[];
}

const MAX_ATTEMPTS = 3;
const RETRY_BACKOFF_MS = 250;

export class ShippingClient {
  async quote(postcode: string, weightGrams: number): Promise<ShippingQuote> {
    const zone = zoneFor(postcode);
    if (zone === undefined) {
      throw new UpstreamError("shipping", `postcode ${postcode} is outside every delivery zone`);
    }
    const url = `${SHIPPING_API_URL}/rates?zone=${encodeURIComponent(zone.zone)}&weight=${weightGrams}`;
    const body = await this.getWithRetry(url);
    const cheapest = body.quotes.sort((a, b) => a.price_cents - b.price_cents)[0];
    if (cheapest === undefined) {
      throw new UpstreamError("shipping", `no rate for zone ${zone.zone}`);
    }
    return {
      courier: cheapest.courier,
      service: cheapest.service,
      cents: cheapest.price_cents,
      businessDays: Math.max(cheapest.eta_days, zone.minDays),
    };
  }

  private async getWithRetry(url: string): Promise<RatesResponse> {
    let lastError: unknown;
    for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
      try {
        const response = await fetch(url, {
          headers: { authorization: `Bearer ${SHIPPING_API_KEY}`, accept: "application/json" },
          signal: controller.signal,
        });
        if (response.status >= 500) {
          throw new UpstreamError("shipping", `rates api answered ${response.status}`);
        }
        if (!response.ok) {
          throw new UpstreamError("shipping", `rates api rejected the request with ${response.status}`);
        }
        return (await response.json()) as RatesResponse;
      } catch (error) {
        lastError = error;
        const retryable = !(error instanceof UpstreamError) || error.message.includes("answered 5");
        if (!retryable || attempt === MAX_ATTEMPTS) {
          break;
        }
        logger.warn({ attempt, url }, "rates api retry");
        await new Promise((resolve) => setTimeout(resolve, RETRY_BACKOFF_MS * attempt));
      } finally {
        clearTimeout(timer);
      }
    }
    if (lastError instanceof UpstreamError) {
      throw lastError;
    }
    throw new UpstreamError("shipping", `rates api unreachable: ${String(lastError)}`);
  }
}
