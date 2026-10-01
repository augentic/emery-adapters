import asyncio
import logging

import httpx

from ..data.postcodes import zone_for
from ..lib.errors import UpstreamError
from ..settings import settings

log = logging.getLogger("shopapi.shipping")

MAX_ATTEMPTS = 3
RETRY_BACKOFF_S = 0.25


class ShippingClient:
    """Quotes delivery through the courier's rates API, cheapest service first."""

    def __init__(self) -> None:
        self.http = httpx.AsyncClient(
            base_url=settings.shipping_api_url,
            timeout=settings.request_timeout_s,
            headers={"authorization": f"Bearer {settings.shipping_api_key}", "accept": "application/json"},
        )

    async def close(self) -> None:
        await self.http.aclose()

    async def quote(self, postcode: str, weight_grams: int) -> dict:
        zone = zone_for(postcode)
        if zone is None:
            raise UpstreamError("shipping", f"postcode {postcode} is outside every delivery zone")
        body = await self._get_with_retry("/rates", {"zone": zone.zone, "weight": weight_grams})
        quotes = sorted(body.get("quotes", []), key=lambda q: q["price_cents"])
        if not quotes:
            raise UpstreamError("shipping", f"no rate for zone {zone.zone}")
        cheapest = quotes[0]
        return {
            "courier": cheapest["courier"],
            "service": cheapest["service"],
            "cents": cheapest["price_cents"],
            "business_days": max(cheapest["eta_days"], zone.min_days),
        }

    async def _get_with_retry(self, path: str, params: dict) -> dict:
        last: Exception | None = None
        for attempt in range(1, MAX_ATTEMPTS + 1):
            try:
                response = await self.http.get(path, params=params)
            except httpx.HTTPError as error:
                last = error
            else:
                if response.status_code >= 500:
                    last = UpstreamError("shipping", f"rates api answered {response.status_code}")
                elif response.status_code >= 400:
                    raise UpstreamError("shipping", f"rates api rejected the request with {response.status_code}")
                else:
                    return response.json()
            if attempt < MAX_ATTEMPTS:
                log.warning("rates api retry %s for %s", attempt, path)
                await asyncio.sleep(RETRY_BACKOFF_S * attempt)
        raise UpstreamError("shipping", f"rates api unreachable: {last}")
